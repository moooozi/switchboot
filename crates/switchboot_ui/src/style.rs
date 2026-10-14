//! Shared visual language: neutral surfaces, rounded controls and semantic accents.
use iced::widget::{
    button::{Status, Style},
    container,
};
use iced::{Border, Color, Theme, color};

pub const BACKGROUND: Color = color!(0x171717);

pub const TEXT: Color = color!(0xf5f5f5);

pub const ACCENT: Color = color!(0x00b9e8);
pub const DANGER: Color = color!(0xf39494);

#[test]
fn both_modes_style_surfaces_text_and_icons() {
    for light in [false, true] {
        let theme = theme(light);
        assert_eq!(theme.extended_palette().is_dark, !light);
        assert_eq!(card(&theme, false).text_color, Some(theme.palette().text));
        assert_ne!(
            icon(&theme, IconTone::Muted).color,
            icon(&theme, IconTone::Disabled).color
        );
        assert_eq!(icon(&theme, IconTone::OnColor).color, Some(Color::WHITE));
        assert_eq!(restart(&theme, Status::Active).text_color, Color::WHITE);
    }
    assert_ne!(
        card(&theme(false), false).background,
        card(&theme(true), false).background
    );
    assert_ne!(muted(&theme(false)).color, muted(&theme(true)).color);
}

pub fn theme(light: bool) -> Theme {
    if light {
        return Theme::custom(
            "Switchboot Light",
            iced::theme::Palette {
                background: color!(0xf5f5f5),
                text: color!(0x202020),
                primary: color!(0x007b9b),
                success: color!(0x00785b),
                warning: color!(0x995000),
                danger: color!(0xb42323),
            },
        );
    }
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

struct Palette {
    surface: Color,
    border: Color,
    muted: Color,
    disabled_icon: Color,
    button: Color,
    button_hover: Color,
    button_pressed: Color,
    button_disabled: Color,
    disabled_text: Color,
    primary_hover: Color,
    primary_text: Color,
    current_badge: (Color, Color, Color),
    default_badge: (Color, Color, Color),
}

impl Palette {
    fn for_theme(theme: &Theme) -> Self {
        if theme.extended_palette().is_dark {
            Self {
                surface: color!(0x252525),
                border: color!(0x404040),
                muted: color!(0xaaaaaa),
                disabled_icon: color!(0x686868),
                button: color!(0x383838),
                button_hover: color!(0x484848),
                button_pressed: color!(0x303030),
                button_disabled: color!(0x303030),
                disabled_text: color!(0x777777),
                primary_hover: color!(0x65d8f4),
                primary_text: BACKGROUND,
                current_badge: (color!(0x4b2068), color!(0xddb3fa), color!(0x9848d8)),
                default_badge: (color!(0x183c78), color!(0xa2c8ff), color!(0x518aff)),
            }
        } else {
            Self {
                surface: Color::WHITE,
                border: color!(0xd4d4d4),
                muted: color!(0x606060),
                disabled_icon: color!(0x999999),
                button: color!(0xe8e8e8),
                button_hover: color!(0xd8d8d8),
                button_pressed: color!(0xcccccc),
                button_disabled: color!(0xeeeeee),
                disabled_text: color!(0x999999),
                primary_hover: color!(0x006580),
                primary_text: Color::WHITE,
                current_badge: (color!(0xf0e2fc), color!(0x65238c), color!(0xb47ad4)),
                default_badge: (color!(0xe3edff), color!(0x244f91), color!(0x7e9fcf)),
            }
        }
    }
}

#[derive(Clone, Copy)]
pub enum IconTone {
    Muted,
    Disabled,
    OnColor,
}

impl IconTone {
    pub fn for_enabled(enabled: bool) -> Self {
        if enabled { Self::Muted } else { Self::Disabled }
    }
}

pub fn muted(theme: &Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(Palette::for_theme(theme).muted),
    }
}

pub fn danger(theme: &Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(theme.palette().danger),
    }
}

pub fn icon(theme: &Theme, tone: IconTone) -> iced::widget::svg::Style {
    let palette = Palette::for_theme(theme);
    iced::widget::svg::Style {
        color: Some(match tone {
            IconTone::Muted => palette.muted,
            IconTone::Disabled => palette.disabled_icon,
            IconTone::OnColor => Color::WHITE,
        }),
    }
}

pub fn card(theme: &Theme, highlighted: bool) -> container::Style {
    let palette = Palette::for_theme(theme);
    container::Style {
        background: Some(palette.surface.into()),
        text_color: Some(theme.palette().text),
        border: Border {
            color: if highlighted {
                theme.palette().primary
            } else {
                palette.border
            },
            width: 1.0,
            radius: 16.0.into(),
        },
        ..Default::default()
    }
}

pub fn target_card(theme: &Theme, current: bool, selected: bool) -> container::Style {
    let mut style = card(theme, current);
    if selected {
        style.border.color = theme.palette().success;
    }
    style
}

#[derive(Clone, Copy)]
pub enum Badge {
    Current,
    Default,
}

pub fn badge(theme: &Theme, kind: Badge) -> container::Style {
    let palette = Palette::for_theme(theme);
    let (background, text, border) = match kind {
        Badge::Current => palette.current_badge,
        Badge::Default => palette.default_badge,
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

pub fn button(theme: &Theme, status: Status) -> Style {
    let palette = Palette::for_theme(theme);
    let (background, text) = match status {
        Status::Active => (palette.button, theme.palette().text),
        Status::Hovered => (palette.button_hover, theme.palette().text),
        Status::Pressed => (palette.button_pressed, theme.palette().text),
        Status::Disabled => (palette.button_disabled, palette.disabled_text),
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
                Status::Hovered => Palette::for_theme(theme).primary_hover,
                Status::Pressed => color!(0x00536a),
                _ => theme.palette().primary,
            }
            .into(),
        ),
        text_color: Palette::for_theme(theme).primary_text,
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
        text_color: Color::WHITE,
        ..button(theme, status)
    }
}
