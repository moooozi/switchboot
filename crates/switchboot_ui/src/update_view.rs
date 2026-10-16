//! Update link and modal rendering.
use super::{Message, style};
use crate::updates::{State, UpdateProgress};
use iced::{
    Element, Length,
    widget::{button, column, container, progress_bar, text, tooltip},
};

impl UpdateProgress {
    fn label(self) -> String {
        let downloaded = self.downloaded as f64 / 1_000_000.0;
        match self.total.filter(|total| *total > 0) {
            Some(total) => format!(
                "Downloaded {downloaded:.1} MB of {:.1} MB ({:.1}%)",
                total as f64 / 1_000_000.0,
                (self.downloaded as f64 / total as f64 * 100.0).min(100.0)
            ),
            None if self.downloaded > 0 => format!("Downloaded {downloaded:.1} MB"),
            None => "Preparing download...".into(),
        }
    }
}

impl State {
    pub fn link(&self, can_install: bool, dirty: bool) -> Option<Element<'_, Message>> {
        let release = self.release()?;
        let label = format!("Upgrade to v{}", release.version.trim_start_matches('v'));
        let action = (can_install && self.can_check()).then_some(Message::InstallUpdate);
        let link = button(text(label).size(14))
            .padding(0)
            .style(|theme, status| {
                let mut appearance = button::text(theme, status);
                if status != button::Status::Disabled {
                    appearance.text_color = if theme.extended_palette().is_dark {
                        iced::color!(0x009fff)
                    } else {
                        iced::color!(0x006fce)
                    };
                }
                appearance
            })
            .on_press_maybe(action);
        let hint = if dirty {
            "Save or discard your boot order changes before updating."
        } else {
            "Download and install the latest version"
        };
        let mut content = column![tooltip(
            link,
            container(text(hint).size(12))
                .padding(8)
                .style(container::rounded_box),
            tooltip::Position::Bottom
        )]
        .spacing(4);
        if let Self::Failed { error, .. } = self {
            content = content.push(text(error).size(12).style(style::danger));
        }
        Some(content.into())
    }

    pub fn modal(&self, progress: UpdateProgress) -> Option<Element<'_, Message>> {
        if !self.installing() {
            return None;
        }
        let label = if matches!(self, Self::ShuttingDown(_)) {
            "Closing Switchboot...".into()
        } else if self.setup_running() {
            "Opening installer...".into()
        } else if progress
            .total
            .is_some_and(|total| total > 0 && progress.downloaded >= total)
        {
            "Verifying update...".into()
        } else {
            progress.label()
        };
        let fraction = if self.setup_running() {
            1.0
        } else {
            progress
                .total
                .filter(|total| *total > 0)
                .map_or(0.0, |total| {
                    (progress.downloaded as f32 / total as f32).min(1.0)
                })
        };
        Some(
            container(
                column![
                    text("Updating Switchboot").size(22).font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    }),
                    text(label).size(16).style(style::muted),
                    progress_bar(0.0..=1.0, fraction).girth(8),
                ]
                .spacing(24),
            )
            .padding(30)
            .max_width(480)
            .width(Length::Fill)
            .style(|theme| style::card(theme, false))
            .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UpdateRelease;

    #[test]
    fn successful_startup_check_without_update_hides_the_link() {
        let mut state = State::new(true);
        assert!(state.begin_check());
        state.checked(Ok(None));
        assert!(state.link(true, false).is_none());
    }

    #[test]
    fn startup_check_and_failure_are_silent() {
        let mut state = State::new(true);
        assert!(state.link(true, false).is_none());
        assert!(state.begin_check());
        assert!(state.link(true, false).is_none());
        state.checked(Err("Offline".into()));
        assert!(state.link(true, false).is_none());
    }

    #[test]
    fn offered_update_and_user_initiated_failure_remain_visible() {
        let mut state = State::new(true);
        assert!(state.begin_check());
        state.checked(Ok(Some(UpdateRelease {
            version: "0.4.0".into(),
            notes: String::new(),
        })));
        assert!(state.link(true, false).is_some());
        assert!(state.begin_install(true).is_some());
        state.launched(Err("Download failed".into()));
        assert!(state.link(true, false).is_some());
    }
}
