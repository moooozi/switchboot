//! Short visual transitions driven by redraws, without application messages or timers.
use std::time::{Duration, Instant};

use iced::advanced::renderer::Renderer as _;
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer, widget};
use iced::widget::{Button, button as button_widget};
use iced::{Animation, Color, Element, Event, Length, Rectangle, Renderer, Size, Theme};

use crate::{Message, style};

const DURATION: Duration = Duration::from_millis(180);
type ButtonStyle = fn(&Theme, button_widget::Status) -> button_widget::Style;

fn transition(target: bool) -> Animation<bool> {
    Animation::new(target)
        .duration(DURATION)
        .easing(iced::animation::Easing::EaseOutCubic)
}

pub fn button<'a>(
    content: Button<'a, Message>,
    enabled: bool,
    style: ButtonStyle,
) -> Element<'a, Message> {
    Element::new(Motion {
        content: content
            .style(move |theme, status| {
                let mut appearance = button_appearance(style, theme, status, enabled);
                appearance.background = None;
                appearance.border.width = 0.0;
                appearance.shadow = Default::default();
                appearance
            })
            .into(),
        kind: Kind::Button { enabled, style },
    })
}

pub fn card<'a>(
    content: impl Into<Element<'a, Message>>,
    current: bool,
    selected: bool,
) -> Element<'a, Message> {
    Element::new(Motion {
        content: content.into(),
        kind: Kind::Card { current, selected },
    })
}

pub fn reveal<'a>(content: impl Into<Element<'a, Message>>, open: bool) -> Element<'a, Message> {
    Element::new(Motion {
        content: content.into(),
        kind: Kind::Reveal { open },
    })
}

#[derive(Clone, Copy)]
enum Kind {
    Button { enabled: bool, style: ButtonStyle },
    Card { current: bool, selected: bool },
    Reveal { open: bool },
}

impl Kind {
    fn target(self) -> bool {
        match self {
            Self::Button { .. } => false,
            Self::Card { selected, .. } => selected,
            Self::Reveal { open } => open,
        }
    }
}

struct State {
    transition: Animation<bool>,
    layout_progress: f32,
    pressed: bool,
}

impl State {
    fn new(target: bool) -> Self {
        Self {
            transition: transition(target),
            layout_progress: if target { 1.0 } else { 0.0 },
            pressed: false,
        }
    }

    fn progress(&self, now: Instant) -> f32 {
        self.transition.interpolate(0.0, 1.0, now)
    }
}

struct Motion<'a> {
    content: Element<'a, Message>,
    kind: Kind,
}

impl Motion<'_> {
    fn interactive(&self) -> bool {
        !matches!(self.kind, Kind::Reveal { open: false })
    }
}

impl Widget<Message, Theme, Renderer> for Motion<'_> {
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::new(self.kind.target()))
    }

    fn size(&self) -> Size<Length> {
        let mut size = self.content.as_widget().size();
        if matches!(self.kind, Kind::Reveal { .. }) {
            size.height = Length::Shrink;
        }
        size
    }

    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut widget::Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
        let state = tree.state.downcast_mut::<State>();
        match self.kind {
            Kind::Button { enabled: false, .. } => {
                state.transition = transition(false);
                state.pressed = false;
            }
            Kind::Button { .. } => {}
            _ => state.transition.go_mut(self.kind.target(), Instant::now()),
        }
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let child = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        let mut size = child.size();
        if matches!(self.kind, Kind::Reveal { .. }) {
            let state = tree.state.downcast_mut::<State>();
            state.layout_progress = state.progress(Instant::now());
            size.height *= state.layout_progress;
        }
        layout::Node::with_children(size, vec![child])
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        if self.interactive() {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout.children().next().unwrap(),
                renderer,
                operation,
            );
        }
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if self.interactive() {
            let clip = if matches!(self.kind, Kind::Reveal { .. }) {
                layout.bounds().intersection(viewport).unwrap_or_default()
            } else {
                *viewport
            };
            let cursor = if cursor.is_over(clip) {
                cursor
            } else {
                mouse::Cursor::Unavailable
            };
            self.content.as_widget_mut().update(
                &mut tree.children[0],
                event,
                layout.children().next().unwrap(),
                cursor,
                renderer,
                clipboard,
                shell,
                &clip,
            );
        }
        let state = tree.state.downcast_mut::<State>();
        let now = Instant::now();
        if let Kind::Button { enabled, .. } = self.kind {
            let hovered = enabled
                && cursor.is_over(layout.bounds())
                && cursor.is_over(*viewport)
                && !matches!(event, Event::Window(iced::window::Event::Unfocused));
            state.transition.go_mut(hovered, now);
            match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                    state.pressed = hovered;
                }
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                | Event::Window(iced::window::Event::Unfocused) => state.pressed = false,
                _ => {}
            }
        }
        if matches!(self.kind, Kind::Reveal { .. }) && state.layout_progress != state.progress(now)
        {
            shell.invalidate_layout();
        }
        if state.transition.is_animating(now) {
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        renderer_style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let progress = state.progress(Instant::now());
        let appearance = match self.kind {
            Kind::Button { enabled, style } => {
                let active = style(theme, button_widget::Status::Active);
                let hovered = style(theme, button_widget::Status::Hovered);
                let mut appearance = if !enabled {
                    style(theme, button_widget::Status::Disabled)
                } else if state.pressed {
                    style(theme, button_widget::Status::Pressed)
                } else {
                    active
                };
                if enabled && !state.pressed {
                    let color = |background| match background {
                        Some(iced::Background::Color(color)) => color,
                        _ => Color::TRANSPARENT,
                    };
                    appearance.background = Some(
                        mix(
                            color(active.background),
                            color(hovered.background),
                            progress,
                        )
                        .into(),
                    );
                }
                Some((appearance.background, appearance.border, appearance.shadow))
            }
            Kind::Card { current, .. } => {
                let mut appearance = style::target_card(theme, current, false);
                appearance.border.color = mix(
                    appearance.border.color,
                    style::target_card(theme, current, true).border.color,
                    progress,
                );
                Some((appearance.background, appearance.border, appearance.shadow))
            }
            Kind::Reveal { .. } => None,
        };
        if let Some((background, border, shadow)) = appearance {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border,
                    shadow,
                    ..Default::default()
                },
                background.unwrap_or(Color::TRANSPARENT.into()),
            );
        }
        let child_layout = layout.children().next().unwrap();
        if matches!(self.kind, Kind::Reveal { .. }) {
            if let Some(clip) = layout.bounds().intersection(viewport)
                && clip.height > 0.0
            {
                renderer.with_layer(clip, |renderer| {
                    self.content.as_widget().draw(
                        &tree.children[0],
                        renderer,
                        theme,
                        renderer_style,
                        child_layout,
                        cursor,
                        &clip,
                    )
                });
            }
        } else {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                renderer_style,
                child_layout,
                cursor,
                viewport,
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.interactive() {
            let clip = if matches!(self.kind, Kind::Reveal { .. }) {
                layout.bounds().intersection(viewport).unwrap_or_default()
            } else {
                *viewport
            };
            let cursor = if cursor.is_over(clip) {
                cursor
            } else {
                mouse::Cursor::Unavailable
            };
            self.content.as_widget().mouse_interaction(
                &tree.children[0],
                layout.children().next().unwrap(),
                cursor,
                &clip,
                renderer,
            )
        } else {
            mouse::Interaction::default()
        }
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut widget::Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: iced::Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        if !self.interactive() {
            return None;
        }
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}

fn mix(from: Color, to: Color, progress: f32) -> Color {
    Color {
        r: from.r + (to.r - from.r) * progress,
        g: from.g + (to.g - from.g) * progress,
        b: from.b + (to.b - from.b) * progress,
        a: from.a + (to.a - from.a) * progress,
    }
}

fn button_appearance(
    style: ButtonStyle,
    theme: &Theme,
    status: button_widget::Status,
    enabled: bool,
) -> button_widget::Style {
    // A rebuilt native button has no status until its next redraw event. Its fallback
    // is Disabled, even when it has an on_press handler; relayout must not dim it.
    let status = if enabled && status == button_widget::Status::Disabled {
        button_widget::Status::Active
    } else {
        status
    };
    style(theme, status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_actions_reject_input_even_while_still_visible() {
        let mut state = State::new(true);
        state.transition.go_mut(false, Instant::now());
        let closing = Motion {
            content: iced::widget::text("Action").into(),
            kind: Kind::Reveal { open: false },
        };
        assert_eq!(state.layout_progress, 1.0);
        assert!(!closing.interactive());

        let opening = Motion {
            kind: Kind::Reveal { open: true },
            ..closing
        };
        assert!(opening.interactive());
    }

    #[test]
    fn disabling_a_hovered_pressed_button_resets_visual_state() {
        let active = button(iced::widget::button("Action"), true, style::button);
        let mut tree = widget::Tree::new(&active);
        let state = tree.state.downcast_mut::<State>();
        state.transition = transition(true);
        state.pressed = true;

        let disabled = button(iced::widget::button("Action"), false, style::button);
        disabled.as_widget().diff(&mut tree);
        let state = tree.state.downcast_ref::<State>();
        assert_eq!(state.progress(Instant::now()), 0.0);
        assert!(!state.pressed);
        assert!(!state.transition.is_animating(Instant::now()));
    }

    #[test]
    fn reveal_can_reverse_without_jumping_and_settles_at_zero() {
        let opened_at = Instant::now();
        let mut state = State::new(false);
        state.transition.go_mut(true, opened_at);
        let reversed_at = opened_at + DURATION / 2;
        let halfway = state.progress(reversed_at);
        assert!(halfway > 0.0 && halfway < 1.0);
        state.transition.go_mut(false, reversed_at);
        assert!((state.progress(reversed_at) - halfway).abs() < 0.001);
        let settled_at = reversed_at + DURATION;
        assert_eq!(state.progress(settled_at), 0.0);
        assert!(!state.transition.is_animating(settled_at));
    }

    #[test]
    fn relayout_does_not_dim_enabled_buttons_but_preserves_disabled_styling() {
        let theme = style::theme(false);
        for style in [style::button as ButtonStyle, style::primary, style::restart] {
            assert_eq!(
                button_appearance(style, &theme, button_widget::Status::Disabled, true).text_color,
                style(&theme, button_widget::Status::Active).text_color,
            );
            assert_eq!(
                button_appearance(style, &theme, button_widget::Status::Disabled, false).text_color,
                style(&theme, button_widget::Status::Disabled).text_color,
            );
        }
    }
}
