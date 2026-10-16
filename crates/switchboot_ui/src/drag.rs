//! Layout-aware native drop targets. Scrollable clipping is respected when hit testing.
use iced::advanced::renderer::Renderer as _;
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, renderer, widget};
use iced::{Color, Element, Event, Length, Point, Rectangle, Renderer, Size, Theme};

use crate::{Message, model::DropTarget};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Zone {
    Row(u16),
    Placeholder(u16),
    End,
    Others,
}

impl Zone {
    fn target(self, bounds: Rectangle, position: Point) -> DropTarget {
        match self {
            Self::Row(id) if position.y < bounds.center_y() => DropTarget::Before(id),
            Self::Row(id) => DropTarget::After(id),
            Self::Placeholder(id) => DropTarget::Before(id),
            Self::End => DropTarget::End,
            Self::Others => DropTarget::Others,
        }
    }

    fn contains(self, target: DropTarget) -> bool {
        match (self, target) {
            (Self::Row(id), DropTarget::Before(anchor) | DropTarget::After(anchor)) => id == anchor,
            (Self::End, DropTarget::End) | (Self::Others, DropTarget::Others) => true,
            _ => false,
        }
    }
}

pub(crate) fn zone<'a>(
    content: impl Into<Element<'a, Message>>,
    kind: Zone,
    active: bool,
    highlight: Option<DropTarget>,
) -> Element<'a, Message> {
    Element::new(DropZone {
        content: content.into(),
        role: Role::Target(kind),
        active,
        highlight,
    })
}

pub(crate) fn viewport<'a>(
    content: impl Into<Element<'a, Message>>,
    active: bool,
    velocity: f32,
) -> Element<'a, Message> {
    Element::new(DropZone {
        content: content.into(),
        role: Role::Viewport { velocity },
        active,
        highlight: None,
    })
}

#[derive(Clone, Copy)]
enum Role {
    Target(Zone),
    Viewport { velocity: f32 },
}

struct DropZone<'a> {
    content: Element<'a, Message>,
    role: Role,
    active: bool,
    highlight: Option<DropTarget>,
}

impl Widget<Message, Theme, Renderer> for DropZone<'_> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }
    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut widget::Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }
    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
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
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        if !self.active {
            return;
        }
        let kind = match self.role {
            Role::Target(kind) => kind,
            Role::Viewport { velocity } => {
                if matches!(event, Event::Mouse(mouse::Event::CursorMoved { .. })) {
                    let next = edge_velocity(layout.bounds(), cursor.position());
                    if velocity != next {
                        shell.publish(Message::DragScrollVelocity(next));
                    }
                }
                return;
            }
        };
        let Some(position) = cursor.position() else {
            return;
        };
        let mut hit_bounds = layout.bounds();
        // Zones have a 10px gap. Each zone owns half of the adjacent insertion gap.
        hit_bounds.y -= 5.0;
        hit_bounds.height += 10.0;
        if !hit_test(hit_bounds, *viewport, position) {
            return;
        }
        if let Zone::Placeholder(id) = kind {
            if matches!(
                event,
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            ) {
                // The moving gap retains the last target rather than targeting itself.
                shell.publish(Message::DragDrop(
                    self.highlight.unwrap_or(DropTarget::Before(id)),
                ));
                shell.capture_event();
            }
            return;
        }
        let target = kind.target(layout.bounds(), position);
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. })
            | Event::Window(iced::window::Event::RedrawRequested(_)) => {
                if self.highlight != Some(target) {
                    shell.publish(Message::DragHover(target));
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                shell.publish(Message::DragDrop(target));
                shell.capture_event();
            }
            _ => {}
        }
    }
    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        let Role::Target(kind) = self.role else {
            return;
        };
        let Some(target) = self
            .highlight
            .filter(|target| self.active && kind.contains(*target))
        else {
            return;
        };
        if matches!(target, DropTarget::Before(_) | DropTarget::After(_)) {
            return;
        }
        let bounds = layout.bounds();
        let color = theme.palette().primary;
        renderer.with_layer(*viewport, |renderer| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: iced::Border {
                        color,
                        width: 2.0,
                        radius: 4.0.into(),
                    },
                    ..renderer::Quad::default()
                },
                Color::TRANSPARENT,
            )
        });
    }
    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.active {
            return mouse::Interaction::Grabbing;
        }
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }
}

fn hit_test(bounds: Rectangle, viewport: Rectangle, position: Point) -> bool {
    bounds.contains(position)
        && position.y < bounds.y + bounds.height
        && viewport.contains(position)
}

fn edge_velocity(bounds: Rectangle, cursor: Option<Point>) -> f32 {
    let Some(position) = cursor.filter(|position| bounds.contains(*position)) else {
        return 0.0;
    };
    let edge = 40.0_f32.min(bounds.height / 3.0);
    if position.y < bounds.y + edge {
        -12.0
    } else if position.y > bounds.y + bounds.height - edge {
        12.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_midpoint_selects_before_or_after() {
        let bounds = Rectangle {
            x: 20.0,
            y: 40.0,
            width: 300.0,
            height: 100.0,
        };
        assert_eq!(
            Zone::Row(0).target(bounds, Point::new(30.0, 89.0)),
            DropTarget::Before(0)
        );
        assert_eq!(
            Zone::Row(0).target(bounds, Point::new(30.0, 90.0)),
            DropTarget::After(0)
        );
    }

    #[test]
    fn clipped_rows_cannot_receive_drops_outside_scroll_viewport() {
        let bounds = Rectangle {
            x: 20.0,
            y: 40.0,
            width: 300.0,
            height: 100.0,
        };
        let viewport = Rectangle {
            x: 20.0,
            y: 80.0,
            width: 300.0,
            height: 200.0,
        };
        assert!(!hit_test(bounds, viewport, Point::new(30.0, 50.0)));
        assert!(hit_test(bounds, viewport, Point::new(30.0, 100.0)));
        assert!(!hit_test(bounds, viewport, Point::new(330.0, 100.0)));
    }

    #[test]
    fn auto_scroll_only_runs_near_visible_viewport_edges() {
        let bounds = Rectangle {
            x: 10.0,
            y: 40.0,
            width: 300.0,
            height: 200.0,
        };
        assert_eq!(edge_velocity(bounds, Some(Point::new(20.0, 50.0))), -12.0);
        assert_eq!(edge_velocity(bounds, Some(Point::new(20.0, 230.0))), 12.0);
        assert_eq!(edge_velocity(bounds, Some(Point::new(20.0, 140.0))), 0.0);
        assert_eq!(edge_velocity(bounds, Some(Point::new(20.0, 250.0))), 0.0);
        assert_eq!(edge_velocity(bounds, None), 0.0);
    }
}
