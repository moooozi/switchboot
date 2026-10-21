//! Update presentation and transitions. Installation is exclusive with firmware work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateRelease {
    pub version: String,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

#[derive(Debug, Default)]
pub(crate) enum State {
    #[default]
    Disabled,
    Idle,
    Checking(Option<UpdateRelease>),
    Current,
    Available(UpdateRelease),
    Installing(UpdateRelease),
    Failed {
        release: Option<UpdateRelease>,
        error: String,
    },
    SetupRunning {
        release: UpdateRelease,
        polling: bool,
    },
    ShuttingDown(Option<UpdateRelease>),
    Finished,
}

impl State {
    pub fn new(supported: bool) -> Self {
        if supported {
            Self::Idle
        } else {
            Self::Disabled
        }
    }

    pub fn checking(&self) -> bool {
        matches!(self, Self::Checking(_))
    }

    pub fn installing(&self) -> bool {
        matches!(
            self,
            Self::Installing(_) | Self::SetupRunning { .. } | Self::ShuttingDown(_)
        )
    }

    pub fn setup_running(&self) -> bool {
        matches!(self, Self::SetupRunning { .. })
    }

    pub(super) fn can_check(&self) -> bool {
        !matches!(
            self,
            Self::Disabled
                | Self::Checking(_)
                | Self::Installing(_)
                | Self::SetupRunning { .. }
                | Self::ShuttingDown(_)
        )
    }

    pub fn release(&self) -> Option<&UpdateRelease> {
        match self {
            Self::Available(release) | Self::Installing(release) => Some(release),
            Self::SetupRunning { release, .. } => Some(release),
            Self::Checking(release)
            | Self::Failed { release, .. }
            | Self::ShuttingDown(release) => release.as_ref(),
            _ => None,
        }
    }

    pub fn begin_check(&mut self) -> bool {
        if !self.can_check() {
            return false;
        }
        *self = Self::Checking(self.release().cloned());
        true
    }

    pub fn begin_shutdown(&mut self, allowed: bool) -> bool {
        if !allowed || matches!(self, Self::Disabled | Self::ShuttingDown(_)) {
            return false;
        }
        *self = Self::ShuttingDown(self.release().cloned());
        true
    }

    pub fn checked(&mut self, result: Result<Option<UpdateRelease>, String>) {
        if !self.checking() {
            return;
        }
        *self = match result {
            Ok(Some(release)) => Self::Available(release),
            Ok(None) => Self::Current,
            Err(error) => Self::Failed {
                release: self.release().cloned(),
                error,
            },
        };
    }

    pub fn begin_install(&mut self, allowed: bool) -> Option<String> {
        if !allowed || !self.can_check() {
            return None;
        }
        let release = self.release()?.clone();
        let version = release.version.clone();
        *self = Self::Installing(release);
        Some(version)
    }

    /// Keep controls locked until Setup requests GUI exit or finishes early.
    pub fn launched(&mut self, result: Result<(), String>) {
        let Self::Installing(release) = self else {
            return;
        };
        *self = match result {
            Ok(()) => Self::SetupRunning {
                release: release.clone(),
                polling: false,
            },
            Err(error) => Self::Failed {
                release: Some(release.clone()),
                error,
            },
        };
    }

    pub fn begin_poll(&mut self) -> bool {
        if let Self::SetupRunning { polling, .. } = self
            && !*polling
        {
            *polling = true;
            return true;
        }
        false
    }

    pub fn polled(&mut self, result: Result<bool, String>) {
        let Self::SetupRunning { release, polling } = self else {
            return;
        };
        match result {
            Ok(false) => *polling = false,
            Ok(true) => *self = Self::Finished,
            Err(error) => {
                *self = Self::Failed {
                    release: Some(release.clone()),
                    error,
                }
            }
        }
    }

    #[cfg(test)]
    pub fn status(&self) -> String {
        match self {
            Self::Disabled | Self::Idle => String::new(),
            Self::Checking(_) => "Checking for updates…".into(),
            Self::Current => "Switchboot is up to date".into(),
            Self::Available(release) => format!("Switchboot {} is available", release.version),
            Self::Installing(_) => {
                "Downloading and verifying update… Approve Windows Setup when prompted.".into()
            }
            Self::Failed { error, .. } => error.clone(),
            Self::SetupRunning { .. } => {
                "Windows Setup is running. It will close and reopen Switchboot.".into()
            }
            Self::Finished => "Windows Setup finished.".into(),
            Self::ShuttingDown(_) => "Closing Switchboot for upgrade...".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release() -> UpdateRelease {
        UpdateRelease {
            version: "1.2.3".into(),
            notes: String::new(),
        }
    }

    #[test]
    fn shutdown_requires_a_clean_idle_model_and_remains_exclusive() {
        let mut state = State::Available(release());
        assert!(!state.begin_shutdown(false));
        assert!(matches!(state, State::Available(_)));
        assert!(state.begin_shutdown(true));
        assert!(state.installing());
        assert!(!state.begin_check());
        assert!(!state.begin_shutdown(true));
        assert!(state.begin_install(true).is_none());
        state.polled(Ok(true));
        state.launched(Err("late launch result".into()));
        assert!(matches!(state, State::ShuttingDown(_)));
        assert!(!State::Disabled.begin_shutdown(true));
        // Setup can signal before its launch completion reaches the UI.
        let mut state = State::Installing(release());
        assert!(state.begin_shutdown(true));
        state.launched(Ok(()));
        assert!(matches!(state, State::ShuttingDown(_)));
    }

    #[test]
    fn stale_completions_and_disabled_actions_do_not_enable_updates() {
        let mut state = State::Disabled;
        state.checked(Ok(Some(release())));
        assert!(!state.begin_check());
        assert!(state.begin_install(true).is_none());
        state.launched(Ok(()));
        assert!(matches!(state, State::Disabled));
    }

    #[test]
    fn failure_preserves_release_for_retry_and_setup_owns_application_exit() {
        let mut state = State::Available(release());
        assert!(state.begin_check());
        assert!(!state.begin_check());
        assert!(state.begin_install(true).is_none());
        state.checked(Err("Offline".into()));
        assert_eq!(state.release(), Some(&release()));
        assert!(state.begin_install(false).is_none());
        assert_eq!(state.begin_install(true).as_deref(), Some("1.2.3"));
        state.checked(Ok(None));
        assert!(state.installing());
        state.launched(Err("Canceled".into()));
        assert_eq!(state.status(), "Canceled");
        assert!(state.begin_install(true).is_some());
        state.launched(Ok(()));
        assert!(state.installing());
        assert!(!state.begin_check());
        assert!(state.begin_poll());
        assert!(!state.begin_poll());
        state.polled(Ok(false));
        assert!(state.begin_poll());
        state.polled(Err("UAC canceled".into()));
        assert!(!state.installing());
        assert!(state.begin_install(true).is_some());
        state.launched(Ok(()));
        state.polled(Ok(true));
        assert!(!state.installing());
        assert!(state.begin_check());
    }
}
