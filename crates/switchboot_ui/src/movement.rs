//! Entry positions survive section changes; model edits commit immediately while rows travel.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use iced::advanced::renderer::Renderer as _;
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer, widget};
use iced::{Animation, Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector};

use crate::Message;

const DURATION: Duration = Duration::from_millis(220);
pub const FIRMWARE: u32 = 0x10000;
pub const OTHERS_HEADING: u32 = 0x10001;

#[derive(Clone, Default)]
pub struct Positions(Rc<RefCell<Registry>>);

impl Positions {
    pub fn sync(&self, order: &[u16], dragging: Option<u16>) {
        self.0.borrow_mut().sync(order, dragging, Instant::now());
    }

    pub fn clear(&self) {
        *self.0.borrow_mut() = Registry::default();
    }
}

#[derive(Default)]
struct Registry {
    order: Vec<u16>,
    revision: u64,
    rows: HashMap<u32, Position>,
    cursor: Option<Point>,
    drag: Option<Drag>,
}

struct Position {
    destination: Point,
    revision: u64,
    x: Animation<f32>,
    y: Animation<f32>,
}

struct Drag {
    id: u16,
    origin: Point,
    cursor_origin: Point,
}

fn animation(value: f32) -> Animation<f32> {
    Animation::new(value)
        .duration(DURATION)
        .easing(iced::animation::Easing::EaseOutCubic)
}

impl Position {
    fn offset(&self, now: Instant) -> Vector {
        Vector::new(
            self.x.interpolate_with(|value| value, now),
            self.y.interpolate_with(|value| value, now),
        )
    }

    fn moving(&self, now: Instant) -> bool {
        self.x.is_animating(now) || self.y.is_animating(now)
    }
}

impl Registry {
    fn sync(&mut self, order: &[u16], dragging: Option<u16>, now: Instant) {
        let changed = self.order != order;
        let previous_drag = self.drag.as_ref().map(|drag| drag.id);
        if previous_drag != dragging {
            if let Some(drag) = self.drag.take()
                && let Some(cursor) = self.cursor
                && let Some(row) = self.rows.get_mut(&u32::from(drag.id))
            {
                // The drop animation starts at the pointer, including a canceled drag.
                row.destination = drag.origin + (cursor - drag.cursor_origin);
                row.x = animation(0.0);
                row.y = animation(0.0);
            }
            if let Some(id) = dragging
                && let Some(cursor) = self.cursor
                && let Some(row) = self.rows.get(&u32::from(id))
            {
                let origin = row.destination + row.offset(now);
                self.drag = Some(Drag {
                    id,
                    origin,
                    cursor_origin: cursor,
                });
                // Drop targets and row locations agree before a new drag begins.
                for row in self.rows.values_mut() {
                    row.x = animation(0.0);
                    row.y = animation(0.0);
                }
            }
        }
        if changed || (previous_drag.is_some() && previous_drag != dragging) {
            self.order = order.to_vec();
            self.revision += 1;
        }
    }

    fn observe(&mut self, id: u32, destination: Point, now: Instant) {
        let revision = self.revision;
        let row = self.rows.entry(id).or_insert_with(|| Position {
            destination,
            revision,
            x: animation(0.0),
            y: animation(0.0),
        });
        if row.revision != revision {
            let delta = row.destination + row.offset(now) - destination;
            row.x = animation(delta.x).go(0.0, now);
            row.y = animation(delta.y).go(0.0, now);
            row.revision = revision;
        }
        // Scrolling, resizing and action expansion update geometry without starting a move.
        row.destination = destination;
    }

    fn offset(&self, id: u32, now: Instant) -> Vector {
        let Some(row) = self.rows.get(&id) else {
            return Vector::ZERO;
        };
        if let Some(drag) = &self.drag
            && u32::from(drag.id) == id
            && let Some(cursor) = self.cursor
        {
            return drag.origin + (cursor - drag.cursor_origin) - row.destination;
        }
        row.offset(now)
    }

    fn dragging(&self, id: u32) -> bool {
        self.drag
            .as_ref()
            .is_some_and(|drag| u32::from(drag.id) == id)
    }
}

pub fn item<'a>(
    content: impl Into<Element<'a, Message>>,
    id: u32,
    positions: &Positions,
) -> Element<'a, Message> {
    Element::new(Moving {
        content: content.into(),
        id,
        positions: positions.clone(),
    })
}

struct Moving<'a> {
    content: Element<'a, Message>,
    id: u32,
    positions: Positions,
}

impl Widget<Message, Theme, Renderer> for Moving<'_> {
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
        let offset = {
            let mut registry = self.positions.0.borrow_mut();
            let now = Instant::now();
            if let Some(position) = cursor.position() {
                registry.cursor = Some(position);
            }
            registry.observe(self.id, layout.position(), now);
            if registry.rows[&self.id].moving(now)
                || (registry.dragging(self.id)
                    && matches!(event, Event::Mouse(mouse::Event::CursorMoved { .. })))
            {
                shell.request_redraw();
            }
            registry.offset(self.id, now)
        };
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor - offset,
            renderer,
            clipboard,
            shell,
            &shifted(*viewport, -offset),
        );
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
        let (offset, floating) = {
            let registry = self.positions.0.borrow();
            let now = Instant::now();
            (
                registry.offset(self.id, now),
                registry.dragging(self.id)
                    || registry
                        .rows
                        .get(&self.id)
                        .is_some_and(|row| row.moving(now)),
            )
        };
        let draw = |renderer: &mut Renderer| {
            renderer.with_translation(offset, |renderer| {
                self.content.as_widget().draw(
                    &tree.children[0],
                    renderer,
                    theme,
                    style,
                    layout,
                    cursor - offset,
                    &shifted(*viewport, -offset),
                );
            })
        };
        if floating {
            // One top layer keeps the grabbed row above its neighbors and inside the viewport.
            renderer.with_layer(*viewport, draw);
        } else {
            draw(renderer);
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
        let registry = self.positions.0.borrow();
        if registry.dragging(self.id) {
            return mouse::Interaction::Grabbing;
        }
        let offset = registry.offset(self.id, Instant::now());
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor - offset,
            &shifted(*viewport, -offset),
            renderer,
        )
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut widget::Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        let offset = self.positions.0.borrow().offset(self.id, Instant::now());
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation + offset,
        )
    }
}

fn shifted(mut rectangle: Rectangle, offset: Vector) -> Rectangle {
    rectangle.x += offset.x;
    rectangle.y += offset.y;
    rectangle
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_rows(now: Instant) -> Registry {
        let mut registry = Registry::default();
        registry.sync(&[0, 1], None, now);
        registry.observe(0, Point::new(20.0, 100.0), now);
        registry.observe(1, Point::new(20.0, 180.0), now);
        registry
    }

    #[test]
    fn reordered_rows_start_at_their_previous_positions_and_settle() {
        let now = Instant::now();
        let mut registry = two_rows(now);
        registry.sync(&[1, 0], None, now);
        registry.observe(1, Point::new(20.0, 100.0), now);
        registry.observe(0, Point::new(20.0, 180.0), now);
        assert_eq!(registry.offset(0, now), Vector::new(0.0, -80.0));
        assert_eq!(registry.offset(1, now), Vector::new(0.0, 80.0));
        for id in [0, 1] {
            assert_eq!(registry.offset(id, now + DURATION), Vector::ZERO);
            assert!(!registry.rows[&id].moving(now + DURATION));
        }
    }

    #[test]
    fn repeated_moves_continue_from_the_current_visual_position() {
        let now = Instant::now();
        let mut registry = two_rows(now);
        registry.sync(&[1, 0], None, now);
        registry.observe(0, Point::new(20.0, 180.0), now);
        let halfway = now + DURATION / 2;
        let visual = registry.rows[&0].destination + registry.offset(0, halfway);
        registry.sync(&[0, 1], None, halfway);
        registry.observe(0, Point::new(20.0, 100.0), halfway);
        assert_eq!(
            registry.rows[&0].destination + registry.offset(0, halfway),
            visual
        );
        assert_eq!(registry.offset(0, halfway + DURATION), Vector::ZERO);
    }

    #[test]
    fn drag_follows_pointer_and_cancel_returns_without_changing_order() {
        let now = Instant::now();
        let mut registry = two_rows(now);
        registry.cursor = Some(Point::new(50.0, 120.0));
        registry.sync(&[0, 1], Some(0), now);
        registry.cursor = Some(Point::new(90.0, 170.0));
        assert_eq!(registry.offset(0, now), Vector::new(40.0, 50.0));
        registry.sync(&[0, 1], None, now);
        registry.observe(0, Point::new(20.0, 100.0), now);
        assert_eq!(registry.order, [0, 1]);
        assert_eq!(registry.offset(0, now), Vector::new(40.0, 50.0));
        assert_eq!(registry.offset(0, now + DURATION), Vector::ZERO);
    }

    #[test]
    fn cross_section_drop_starts_at_pointer_and_preserves_entry_zero() {
        let now = Instant::now();
        let mut registry = two_rows(now);
        registry.observe(2, Point::new(20.0, 400.0), now);
        registry.cursor = Some(Point::new(50.0, 420.0));
        registry.sync(&[0, 1], Some(2), now);
        registry.cursor = Some(Point::new(50.0, 150.0));
        registry.sync(&[0, 2, 1], None, now);
        registry.observe(2, Point::new(20.0, 180.0), now);
        assert_eq!(
            registry.rows[&2].destination + registry.offset(2, now),
            Point::new(20.0, 130.0)
        );
        assert_eq!(registry.offset(2, now + DURATION), Vector::ZERO);
        assert!(registry.rows.contains_key(&0));
    }

    #[test]
    fn scrolling_and_resizing_do_not_start_reorder_animations() {
        let now = Instant::now();
        let mut registry = two_rows(now);
        registry.observe(0, Point::new(16.0, 60.0), now);
        assert_eq!(registry.offset(0, now), Vector::ZERO);
        assert!(!registry.rows[&0].moving(now));
        // A scroll changes content-space cursor coordinates while the grabbed row stays under it.
        registry.cursor = Some(Point::new(50.0, 80.0));
        registry.sync(&[0, 1], Some(0), now);
        registry.cursor = Some(Point::new(50.0, 120.0));
        assert_eq!(registry.offset(0, now), Vector::new(0.0, 40.0));
    }
}
