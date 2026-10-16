//! Application update tasks and handoff routing, separate from firmware messages.
use crate::{App, Message, UpdateProgress, updates, worker};
use iced::{Subscription, Task};
use std::sync::{Arc, Mutex};

impl Message {
    pub(super) fn is_update(&self) -> bool {
        matches!(
            self,
            Self::UpdateChecked(_)
                | Self::InstallUpdate
                | Self::UpdateInstalled(_)
                | Self::UpdateProgress(_)
                | Self::PollUpdate
                | Self::UpdatePolled(_)
                | Self::PollUpgradeShutdown
        )
    }
}

fn update_progress_reporter(
    output: iced::futures::channel::mpsc::Sender<Message>,
) -> Arc<dyn Fn(UpdateProgress) + Send + Sync> {
    let output = Mutex::new(output);
    Arc::new(move |progress| {
        // Reuse the sender: cloning per callback creates a new reserved queue
        // slot and bypasses the channel bound. Intermediate progress may drop.
        if let Ok(mut output) = output.lock() {
            let _ = output.try_send(Message::UpdateProgress(progress));
        }
    })
}

impl App {
    pub(super) fn can_install_update(&self) -> bool {
        !self.model.busy()
            && !self.model.discovering
            && !self.model.changed()
            && self.dialog.is_none()
            && self.drag.is_none()
    }

    pub(super) fn check_update(&mut self) -> Task<Message> {
        if !self.updates.begin_check() {
            return Task::none();
        }
        let backend = self.backend.clone();
        worker(move || backend.check_update(), Message::UpdateChecked)
    }

    pub(super) fn update_subscription(&self) -> Subscription<Message> {
        let mut subscriptions = Vec::new();
        if self.updates.setup_running() {
            subscriptions.push(
                iced::time::every(std::time::Duration::from_millis(500))
                    .map(|_| Message::PollUpdate),
            );
        }
        if !matches!(
            self.updates,
            updates::State::Disabled | updates::State::ShuttingDown(_)
        ) {
            subscriptions.push(
                iced::time::every(std::time::Duration::from_millis(250))
                    .map(|_| Message::PollUpgradeShutdown),
            );
        }
        Subscription::batch(subscriptions)
    }

    pub(super) fn update_updates(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PollUpgradeShutdown => match self.backend.upgrade_shutdown_requested() {
                Ok(true) => {
                    if self.updates.begin_shutdown(self.can_install_update()) {
                        return iced::exit();
                    }
                    self.model.error = Some("Upgrade shutdown refused: save or discard edits and finish pending operations first.".into());
                }
                Ok(false) => {}
                Err(error) => self.model.error = Some(error),
            },

            Message::UpdateChecked(result) => self.updates.checked(result),
            Message::InstallUpdate => {
                let allowed = self.can_install_update();
                if let Some(version) = self.updates.begin_install(allowed) {
                    self.update_progress = UpdateProgress::default();
                    self.entry_actions = None;
                    let backend = self.backend.clone();
                    return Task::run(
                        iced::stream::channel(1, async move |mut output| {
                            use iced::futures::SinkExt;
                            let progress_output = update_progress_reporter(output.clone());
                            let result = tokio::task::spawn_blocking(move || {
                                backend.install_update_with_progress(version, progress_output)
                            })
                            .await
                            .unwrap_or_else(|error| {
                                Err(format!("Background task failed: {error}"))
                            });
                            let _ = output.send(Message::UpdateInstalled(result)).await;
                        }),
                        |message| message,
                    );
                }
            }
            Message::UpdateProgress(progress) => {
                if matches!(self.updates, updates::State::Installing(_)) {
                    self.update_progress = progress;
                }
            }
            Message::UpdateInstalled(result) => {
                self.updates.launched(result);
            }
            Message::PollUpdate => {
                if self.updates.begin_poll() {
                    let backend = self.backend.clone();
                    return worker(move || backend.poll_update(), Message::UpdatePolled);
                }
            }
            Message::UpdatePolled(result) => self.updates.polled(result),
            _ => {}
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UpdateRelease;
    use crate::tests::app;

    #[test]
    fn progress_flood_stays_bounded_and_reporting_resumes_after_drain() {
        let (output, mut input) = iced::futures::channel::mpsc::channel(1);
        let report = update_progress_reporter(output);
        for downloaded in 0..1_000 {
            report(UpdateProgress {
                downloaded,
                total: None,
            });
        }
        let mut queued = 0;
        while input.try_recv().is_ok() {
            queued += 1;
        }
        assert!((1..=2).contains(&queued));
        let progress = UpdateProgress {
            downloaded: 1_000,
            total: Some(1_000),
        };
        report(progress);
        assert!(
            matches!(input.try_recv(), Ok(Message::UpdateProgress(value)) if value == progress)
        );
    }

    #[test]
    fn unsupported_updates_are_disabled_and_installation_requires_a_clean_idle_model() {
        let mut app = app();
        app.updates = updates::State::Disabled;
        assert!(matches!(app.updates, updates::State::Disabled));
        let _ = app.check_update();
        assert!(!app.updates.checking());
        let _ = app.update(Message::InstallUpdate);
        assert!(!app.updates.installing());
        app.updates = updates::State::Idle;
        let _ = app.check_update();
        let _ = app.update(Message::UpdateChecked(Ok(Some(UpdateRelease {
            version: "0.4.0".into(),
            notes: "Changes".into(),
        }))));
        let _ = app.update(Message::Move(0, 1));
        let _ = app.update(Message::InstallUpdate);
        assert!(!app.updates.installing());
        let _ = app.update(Message::Discard);
        let _ = app.update(Message::BootNext(Some(0)));
        let _ = app.update(Message::InstallUpdate);
        assert!(!app.updates.installing());
        let _ = app.update(Message::Completed(Ok(())));
        let _ = app.update(Message::InstallUpdate);
        assert!(app.updates.installing());
        let _ = app.update(Message::BootNext(Some(1)));
        let _ = app.update(Message::Move(0, 1));
        assert_eq!(app.model.boot_next, Some(0));
        assert_eq!(app.model.order, vec![0, 1]);
        assert!(app.model.pending.is_none());
        let _ = app.update(Message::UpdateInstalled(Err("UAC canceled".into())));
        assert!(!app.updates.installing());
        assert_eq!(app.updates.status(), "UAC canceled");
        assert!(app.updates.release().is_some());
    }

    #[test]
    fn progress_modal_stays_exclusive_and_closes_on_failure() {
        let mut app = app();
        app.updates = updates::State::Available(UpdateRelease {
            version: "1.2.3".into(),
            notes: String::new(),
        });
        assert!(app.updates.modal(app.update_progress).is_none());
        let _ = app.update(Message::InstallUpdate);
        let progress = UpdateProgress {
            downloaded: 1_500_000,
            total: Some(3_200_000),
        };
        let _ = app.update(Message::UpdateProgress(progress));
        assert_eq!(app.update_progress, progress);
        assert!(app.updates.modal(app.update_progress).is_some());
        let order = app.model.order.clone();
        let _ = app.update(Message::Move(0, 1));
        assert_eq!(app.model.order, order);
        let _ = app.update(Message::UpdateInstalled(Err("Download failed".into())));
        assert!(app.updates.modal(app.update_progress).is_none());
        assert!(app.updates.release().is_some());
        let _ = app.update(Message::UpdateProgress(UpdateProgress::default()));
        assert_eq!(app.update_progress, progress);
    }

    #[test]
    fn update_checks_allow_retry_without_changing_firmware_or_draft() {
        let mut app = app();
        app.updates = updates::State::Idle;
        let _ = app.check_update();
        assert!(app.updates.checking());
        let _ = app.update(Message::UpdateChecked(Err("Offline".into())));
        assert!(!app.updates.checking());
        assert_eq!(app.updates.status(), "Offline");
        let _ = app.check_update();
        assert!(app.updates.checking());
        let _ = app.update(Message::UpdateChecked(Ok(None)));
        assert!(!app.updates.checking());
        assert!(app.updates.release().is_none());
        assert_eq!(app.model.order, vec![0, 1]);
        assert!(app.model.history.is_empty());
        // Demo checks always return a local sample; clicking Install preserves
        // the notification and cannot download, elevate, launch Setup or exit.
        let _ = app.check_update();
        let _ = app.update(Message::UpdateChecked(app.backend.check_update()));
        assert!(app.updates.release().is_some());
        let version = app.updates.release().unwrap().version.clone();
        let _ = app.update(Message::InstallUpdate);
        let _ = app.update(Message::UpdateInstalled(
            app.backend.install_update(version),
        ));
        assert!(!app.updates.installing());
        assert!(app.updates.release().is_some());
    }

    #[test]
    fn setup_launch_keeps_app_open_and_cancellation_releases_operation_guard() {
        let mut app = app();
        app.updates = updates::State::Available(UpdateRelease {
            version: "0.4.0".into(),
            notes: String::new(),
        });
        let _ = app.update(Message::InstallUpdate);
        let _ = app.update(Message::UpdateInstalled(Ok(())));
        assert!(app.updates.setup_running());
        let _ = app.update(Message::BootNext(Some(1)));
        assert!(app.model.pending.is_none());
        let _ = app.update(Message::PollUpdate);
        let _ = app.update(Message::UpdatePolled(Ok(false)));
        assert!(app.updates.setup_running());
        let _ = app.update(Message::PollUpdate);
        let _ = app.update(Message::UpdatePolled(Err("Setup canceled".into())));
        assert!(!app.updates.installing());
        assert_eq!(app.updates.status(), "Setup canceled");
        let _ = app.update(Message::BootNext(Some(1)));
        assert!(app.model.pending.is_some());
    }
}
