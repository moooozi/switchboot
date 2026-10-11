//! Shared visual language: neutral surfaces, rounded controls and semantic accents.
use iced::widget::{
    button::{Status, Style},
    container,
};
use iced::{Border, Color, Theme, color};

pub const BACKGROUND: Color = color!(0x171717);
pub const SURFACE: Color = color!(0x252525);
pub const BORDER: Color = color!(0x404040);
pub const TEXT: Color = color!(0xf5f5f5);
pub const MUTED: Color = color!(0xaaaaaa);
pub const ACCENT: Color = color!(0x00b9e8);
pub const DANGER: Color = color!(0xf39494);

pub fn theme() -> Theme {
    Theme::custom(
        "Switchboot",
        iced::theme::Palette {
            background: BACKGROUND,
            text: TEXT,
            primary: ACCENT,
            success: color!(0x8bcda3),
            warning: color!(0xe6bd76),
            danger: DANGER,
        },
    )
}

pub fn card(highlighted: bool) -> container::Style {
    container::Style {
        background: Some(SURFACE.into()),
        text_color: Some(TEXT),
        border: Border {
            color: if highlighted { ACCENT } else { BORDER },
            width: 1.0,
            radius: 16.0.into(),
        },
        ..Default::default()
    }
}

pub fn target_card(current: bool, selected: bool) -> container::Style {
    let mut style = card(current);
    if selected {
        style.border.color = color!(0x00c391);
    }
    style
}

#[derive(Clone, Copy)]
pub enum Badge {
    Current,
    Default,
}

pub fn badge(kind: Badge) -> container::Style {
    let (background, text, border) = match kind {
        Badge::Current => (color!(0x4b2068), color!(0xddb3fa), color!(0x9848d8)),
        Badge::Default => (color!(0x183c78), color!(0xa2c8ff), color!(0x518aff)),
    };
    container::Style {
        background: Some(background.into()),
        text_color: Some(text),
        border: Border {
            color: border,
            width: 1.0,
            radius: 20.0.into(),
        },
        ..Default::default()
    }
}

pub fn button(_: &Theme, status: Status) -> Style {
    let (background, text) = match status {
        Status::Active => (color!(0x383838), TEXT),
        Status::Hovered => (color!(0x484848), TEXT),
        Status::Pressed => (color!(0x303030), TEXT),
        Status::Disabled => (color!(0x303030), color!(0x777777)),
    };
    Style {
        background: Some(background.into()),
        text_color: text,
        border: Border {
            radius: 24.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn primary(theme: &Theme, status: Status) -> Style {
    if status == Status::Disabled {
        return button(theme, status);
    }
    Style {
        background: Some(
            match status {
                Status::Hovered => color!(0x65d8f4),
                Status::Pressed => color!(0x009dc5),
                _ => ACCENT,
            }
            .into(),
        ),
        text_color: BACKGROUND,
        ..button(theme, status)
    }
}

pub fn quiet(theme: &Theme, status: Status) -> Style {
    let mut style = button(theme, status);
    if matches!(status, Status::Active | Status::Disabled) {
        style.background = None;
    }
    style
}

pub fn undo_next(theme: &Theme, status: Status) -> Style {
    colored(theme, status, color!(0x00866a), color!(0x00a580))
}

pub fn restart(theme: &Theme, status: Status) -> Style {
    colored(theme, status, color!(0xba4c00), color!(0xd45c0a))
}

fn colored(theme: &Theme, status: Status, base: Color, hover: Color) -> Style {
    if status == Status::Disabled {
        return button(theme, status);
    }
    Style {
        background: Some(
            if status == Status::Hovered {
                hover
            } else {
                base
            }
            .into(),
        ),
        ..button(theme, status)
    }
}
