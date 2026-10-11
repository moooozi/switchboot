//! Native Switchboot UI. Firmware services are supplied by the host through [`Backend`].
mod drag;
mod model;
mod style;

use std::sync::{Arc, Mutex};

use iced::widget::{
    Column, button, checkbox, column, container, mouse_area, row, scrollable, text, text_input,
};
use iced::{Element, Length, Subscription, Task};
pub use model::{Backend, BootTarget, Entry, Shortcut, Snapshot};
use model::{Change, DropTarget, Model, Operation};

/// Opens the native window. Run this on the process's main thread.
pub fn run(backend: Arc<dyn Backend>) -> iced::Result {
    run_application(backend, None)
}

/// Run with a lightweight activation callback supplied by the process singleton.
pub fn run_with_events(
    backend: Arc<dyn Backend>,
    activation: Arc<dyn Fn() -> bool + Send + Sync>,
) -> iced::Result {
    run_application(backend, Some(activation))
}

fn run_application(
    backend: Arc<dyn Backend>,
    activation: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
) -> iced::Result {
    iced::application(
        move || {
            let (mut app, task) = App::new(backend.clone());
            app.activation = activation.clone();
            (app, task)
        },
        App::update,
        App::view,
    )
    .title("Switchboot")
    .theme(style::theme())
    .subscription(App::subscription)
    .window(iced::window::Settings {
        size: iced::Size::new(960.0, 740.0),
        min_size: Some(iced::Size::new(800.0, 520.0)),
        ..iced::window::Settings::default()
    })
    .centered()
    .run()
}

struct App {
    backend: Arc<dyn Backend>,
    model: Model,
    dialog: Option<Dialog>,
    notice: Option<String>,
    drag: Option<DragState>,
    activation: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    entry_actions: Option<BootTarget>,
}

#[derive(Debug, Clone, Copy)]
struct DragState {
    entry: u16,
    target: Option<DropTarget>,
    scroll_velocity: f32,
}

#[derive(Debug, Clone)]
enum Dialog {
    Restart,
    Shortcut {
        target: BootTarget,
        name: String,
        reboot: bool,
    },
}

#[derive(Debug, Clone)]
enum Message {
    Loaded(Result<Snapshot, String>),
    Discovered(Result<Vec<Entry>, String>),
    Completed(Result<(), String>),
    Refresh,
    Move(u16, isize),
    MakeDefault(u16),
    Include(u16),
    Exclude(u16),
    Save,
    Discard,
    Undo,
    Redo,
    BootNext(Option<u16>),
    Firmware(bool),
    OpenShortcut(BootTarget),
    ShortcutName(String),
    ShortcutReboot(bool),
    CreateShortcut,
    RequestRestart,
    ConfirmRestart,
    CancelDialog,
    Keyboard(iced::keyboard::Event),
    DragStart(u16),
    DragHover(DropTarget),
    DragDrop(DropTarget),
    DragCancel,
    DragScrollVelocity(f32),
    DragScrollTick,
    PollActivation,
    ToggleEntryActions(BootTarget),
}

fn worker<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
    complete: impl FnOnce(Result<T, String>) -> Message + Send + 'static,
) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(work)
                .await
                .map_err(|error| format!("Firmware worker failed: {error}"))?
        },
        complete,
    )
}

impl App {
    fn new(backend: Arc<dyn Backend>) -> (Self, Task<Message>) {
        let mut app = Self {
            backend,
            model: Model::default(),
            dialog: None,
            notice: None,
            drag: None,
            activation: None,
            entry_actions: None,
        };
        let task = app.load();
        (app, task)
    }

    fn load(&mut self) -> Task<Message> {
        self.entry_actions = None;
        self.model.loading = true;
        self.model.error = None;
        let backend = self.backend.clone();
        worker(move || backend.load(), Message::Loaded)
    }

    fn operation(&mut self, operation: Operation) -> Task<Message> {
        if self.drag.is_some() || !self.model.begin(operation.clone()) {
            return Task::none();
        }
        self.notice = None;
        self.entry_actions = None;
        let backend = self.backend.clone();
        worker(
            move || operation.execute(backend.as_ref()),
            Message::Completed,
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            iced::keyboard::listen().map(Message::Keyboard),
            iced::event::listen_with(|event, status, _| match event {
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) if status == iced::event::Status::Ignored => Some(Message::DragCancel),
                iced::Event::Mouse(
                    iced::mouse::Event::CursorLeft
                    | iced::mouse::Event::ButtonPressed(iced::mouse::Button::Right),
                )
                | iced::Event::Window(iced::window::Event::Unfocused)
                | iced::Event::Touch(
                    iced::touch::Event::FingerLost { .. } | iced::touch::Event::FingerLifted { .. },
                ) => Some(Message::DragCancel),
                _ => None,
            }),
        ];
        if self.activation.is_some() {
            subscriptions.push(
                iced::time::every(std::time::Duration::from_millis(150))
                    .map(|_| Message::PollActivation),
            );
        }
        if self.drag.is_some_and(|drag| drag.scroll_velocity != 0.0) {
            subscriptions.push(
                iced::time::every(std::time::Duration::from_millis(32))
                    .map(|_| Message::DragScrollTick),
            );
        }
        Subscription::batch(subscriptions)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PollActivation => {
                if self.activation.as_ref().is_some_and(|poll| poll()) {
                    return iced::window::oldest().then(|window| match window {
                        Some(id) => {
                            iced::window::minimize(id, false).chain(iced::window::gain_focus(id))
                        }
                        None => Task::none(),
                    });
                }
            }
            Message::DragCancel => self.drag = None,
            Message::DragScrollVelocity(velocity) => {
                if let Some(drag) = &mut self.drag {
                    drag.scroll_velocity = velocity;
                }
            }
            Message::DragScrollTick => {
                if let Some(drag) = self.drag
                    && !self.model.busy()
                    && drag.scroll_velocity != 0.0
                {
                    return iced::widget::operation::scroll_by(
                        "boot-entries",
                        iced::widget::operation::AbsoluteOffset {
                            x: 0.0,
                            y: drag.scroll_velocity,
                        },
                    );
                }
            }
            Message::DragStart(entry)
                if !self.model.busy()
                    && self.dialog.is_none()
                    && self.drag.is_none()
                    && self.model.entries.contains_key(&entry) =>
            {
                self.entry_actions = None;
                self.drag = Some(DragState {
                    entry,
                    target: None,
                    scroll_velocity: 0.0,
                });
            }
            Message::DragHover(target) if !self.model.busy() && self.dialog.is_none() => {
                if let Some(drag) = &mut self.drag {
                    drag.target = Some(target);
                }
            }
            Message::DragDrop(target) => {
                if let Some(drag) = self.drag.take()
                    && self.dialog.is_none()
                {
                    self.model.drop_entry(drag.entry, target);
                }
            }
            Message::Loaded(result) => {
                self.model.loading = false;
                match result.and_then(|snapshot| self.model.initialize(snapshot)) {
                    Ok(()) => {
                        self.model.discovering = true;
                        let backend = self.backend.clone();
                        return worker(move || backend.discover(), Message::Discovered);
                    }
                    Err(error) => self.model.error = Some(error),
                }
            }
            Message::Discovered(result) => {
                self.model.discovering = false;
                match result {
                    Ok(entries) => {
                        for entry in entries {
                            self.model.entries.entry(entry.id).or_insert(entry);
                        }
                    }
                    Err(error) => {
                        self.model.error = Some(format!("Entry discovery failed: {error}"))
                    }
                }
            }
            Message::Completed(result) => {
                let succeeded = result.is_ok();
                let shortcut = matches!(self.model.pending, Some(Operation::Shortcut(_)));
                self.model.complete(result);
                if succeeded {
                    self.notice = Some(
                        if shortcut {
                            "Shortcut created."
                        } else {
                            "Operation completed."
                        }
                        .into(),
                    );
                    if shortcut {
                        self.dialog = None;
                    }
                }
            }
            Message::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) {
                    self.drag = None;
                    self.entry_actions = None;
                    return self.update(Message::CancelDialog);
                }
                if self.dialog.is_none()
                    && modifiers.command()
                    && let iced::keyboard::Key::Character(character) = key
                {
                    let action = match character.as_str().to_ascii_lowercase().as_str() {
                        "s" => Some(Message::Save),
                        "z" if modifiers.shift() => Some(Message::Redo),
                        "z" => Some(Message::Undo),
                        "y" => Some(Message::Redo),
                        _ => None,
                    };
                    if let Some(action) = action {
                        return self.update(action);
                    }
                }
            }
            Message::Refresh
                if !self.model.loading
                    && self.model.pending.is_none()
                    && !self.model.discovering
                    && !self.model.changed()
                    && self.dialog.is_none()
                    && self.drag.is_none() =>
            {
                return self.load();
            }
            Message::CancelDialog if self.model.pending.is_none() => self.dialog = None,
            Message::ShortcutName(name) if !self.model.busy() => {
                if let Some(Dialog::Shortcut { name: value, .. }) = &mut self.dialog {
                    *value = name;
                }
            }
            Message::ShortcutReboot(reboot) if !self.model.busy() => {
                if let Some(Dialog::Shortcut { reboot: value, .. }) = &mut self.dialog {
                    *value = reboot;
                }
            }
            Message::CreateShortcut if !self.model.busy() => {
                if let Some(Dialog::Shortcut {
                    target,
                    name,
                    reboot,
                }) = &self.dialog
                {
                    if name.trim().is_empty() {
                        return Task::none();
                    }
                    let shortcut = Shortcut {
                        name: name.trim().into(),
                        target: *target,
                        reboot: *reboot,
                        icon_id: Some(self.icon_id(*target).into()),
                    };
                    return self.operation(Operation::Shortcut(shortcut));
                }
            }
            Message::ConfirmRestart
                if !self.model.busy() && matches!(self.dialog, Some(Dialog::Restart)) =>
            {
                self.dialog = None;
                return self.operation(Operation::Restart);
            }
            message if !self.model.busy() && self.dialog.is_none() && self.drag.is_none() => {
                match message {
                    Message::ToggleEntryActions(target) => {
                        self.entry_actions = if self.entry_actions == Some(target) {
                            None
                        } else {
                            Some(target)
                        };
                    }
                    Message::Move(id, direction) => self.model.move_entry(id, direction),
                    Message::MakeDefault(id) => self.model.make_default(id),
                    Message::Include(id) => self.model.include(id),
                    Message::Exclude(id) => self.model.exclude(id),
                    Message::Discard => self.model.record_order(self.model.baseline.clone()),
                    Message::Save if self.model.changed() => {
                        return self.operation(Operation::Save(self.model.order.clone()));
                    }
                    Message::Undo if self.model.cursor > 0 => {
                        let change = self.model.history[self.model.cursor - 1].clone();
                        return self.operation(Operation::Undo(change));
                    }
                    Message::Redo if self.model.cursor < self.model.history.len() => {
                        let change = self.model.history[self.model.cursor].clone();
                        return self.operation(Operation::Redo(change));
                    }
                    Message::BootNext(next)
                        if next != self.model.boot_next
                            && next.is_none_or(|id| self.model.entries.contains_key(&id)) =>
                    {
                        return self.operation(Operation::Change(Change::BootNext {
                            before: self.model.boot_next,
                            after: next,
                        }));
                    }
                    Message::Firmware(after) if after != self.model.firmware_setup => {
                        return self.operation(Operation::Change(Change::Firmware {
                            before: self.model.firmware_setup,
                            after,
                        }));
                    }
                    Message::OpenShortcut(target) if !self.model.portable => {
                        let description = match target {
                            BootTarget::Entry(id) => self
                                .model
                                .entries
                                .get(&id)
                                .map(|entry| entry.description.as_str())
                                .unwrap_or("boot entry"),
                            BootTarget::FirmwareSetup => "EFI Firmware Setup",
                        };
                        self.dialog = Some(Dialog::Shortcut {
                            target,
                            name: format!("Reboot to {description}"),
                            reboot: true,
                        });
                    }
                    Message::RequestRestart => self.dialog = Some(Dialog::Restart),
                    _ => {}
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn icon_id(&self, target: BootTarget) -> &'static str {
        let BootTarget::Entry(id) = target else {
            return "generic";
        };
        let Some(entry) = self.model.entries.get(&id) else {
            return "generic";
        };
        let description = entry.description.to_lowercase();
        [
            ("windows boot manager", "windows"),
            ("ubuntu", "ubuntu"),
            ("buntu", "ubuntu"),
            ("neon", "neon"),
            ("pop", "popos"),
            ("debian", "debian"),
            ("fedora", "fedora"),
            ("suse", "suse"),
            ("cachyos", "cachyos"),
            ("manjaro", "manjaro"),
            ("arch", "arch"),
            ("nixos", "nixos"),
            ("deepin", "deepin"),
            ("macos", "macos"),
            ("mac os", "macos"),
            ("darwin", "macos"),
            ("apple", "macos"),
            ("linux", "linux"),
            ("gnu", "linux"),
        ]
        .into_iter()
        .find(|(pattern, _)| description.contains(pattern))
        .map(|(_, icon)| icon)
        .unwrap_or("generic")
    }

    fn control<'a>(
        &self,
        label: impl Into<String>,
        message: Message,
        enabled: bool,
    ) -> Element<'a, Message> {
        let mut control = button(text(label.into()).size(14))
            .padding([9, 16])
            .style(style::button);
        if enabled {
            control = control.on_press(message);
        }
        control.into()
    }

    fn icon_control<'a>(
        &self,
        icon: &'static str,
        label: impl Into<String>,
        message: Message,
        enabled: bool,
    ) -> Element<'a, Message> {
        let mut control = button(
            container(action_icon(icon, enabled))
                .center_x(38)
                .center_y(38),
        )
        .padding(0)
        .style(move |theme, status| {
            if matches!(icon, "more" | "undo" | "redo") {
                style::quiet(theme, status)
            } else {
                style::button(theme, status)
            }
        });
        if enabled {
            control = control.on_press(message);
        }
        iced::widget::tooltip(
            control,
            container(text(label.into()).size(13))
                .padding([6, 10])
                .style(|_| style::card(false)),
            iced::widget::tooltip::Position::Bottom,
        )
        .into()
    }

    fn entry_view(&self, entry: &Entry, position: Option<usize>) -> Element<'_, Message> {
        let enabled = !self.model.busy() && self.dialog.is_none() && self.drag.is_none();
        let target = BootTarget::Entry(entry.id);
        let mut badges = row![].spacing(8).align_y(iced::Alignment::Center);
        if self.model.current == Some(entry.id) {
            badges = badges.push(badge("Current", style::Badge::Current));
        }
        if self.model.baseline.first() == Some(&entry.id) {
            badges = badges.push(badge("Default", style::Badge::Default));
        }
        if self.model.boot_next == Some(entry.id) {
            badges = badges.push(badge("BootNext", style::Badge::Next));
        }
        let mut handle = mouse_area(action_icon("grip", enabled));
        if enabled && (position.is_none() || self.model.order.len() > 1) {
            handle = handle
                .on_press(Message::DragStart(entry.id))
                .interaction(iced::mouse::Interaction::Grab);
        }
        let mut title = row![handle].spacing(12).align_y(iced::Alignment::Center);
        if let Some(index) = position {
            title = title.push(
                text(format!("{}.", index + 1))
                    .size(15)
                    .color(style::MUTED)
                    .width(22),
            );
        }
        title = title
            .push(os_icon(self.icon_id(target)))
            .push(text(entry.description.clone()).size(19).width(Length::Fill))
            .push(badges);
        if let Some(index) = position {
            title = title
                .push(self.icon_control(
                    "up",
                    "Move up",
                    Message::Move(entry.id, -1),
                    enabled && index > 0,
                ))
                .push(self.icon_control(
                    "down",
                    "Move down",
                    Message::Move(entry.id, 1),
                    enabled && index + 1 < self.model.order.len(),
                ))
                .push(self.icon_control(
                    "remove",
                    "Remove from boot order",
                    Message::Exclude(entry.id),
                    enabled && self.model.order.len() > 1,
                ));
        } else {
            title = title.push(self.icon_control(
                "add",
                "Add to boot order",
                Message::Include(entry.id),
                enabled,
            ));
        }
        title = title.push(self.icon_control(
            "more",
            "Entry actions",
            Message::ToggleEntryActions(target),
            enabled,
        ));
        let mut body = column![title].spacing(14);
        if self.entry_actions == Some(target) {
            let mut actions = row![
                self.control(
                    if self.model.boot_next == Some(entry.id) {
                        "Cancel BootNext"
                    } else {
                        "Boot next"
                    },
                    Message::BootNext(if self.model.boot_next == Some(entry.id) {
                        None
                    } else {
                        Some(entry.id)
                    }),
                    enabled,
                ),
                self.control(
                    "Create shortcut",
                    Message::OpenShortcut(target),
                    enabled && !self.model.portable
                ),
            ]
            .spacing(8);
            if let Some(index) = position {
                actions = actions.push(self.control(
                    "Make default",
                    Message::MakeDefault(entry.id),
                    enabled && index > 0,
                ));
            }
            body = body.push(actions);
        }
        let current = self.model.current == Some(entry.id);
        let card = container(body)
            .padding([20, 16])
            .width(Length::Fill)
            .style(move |_| style::card(current));
        if position.is_some() {
            drag::zone(
                card,
                drag::Zone::Row(entry.id),
                self.drag.is_some() && !self.model.busy(),
                self.drag.and_then(|state| state.target),
            )
        } else {
            card.into()
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let enabled = !self.model.busy() && self.dialog.is_none() && self.drag.is_none();
        let undo = self
            .model
            .history
            .get(self.model.cursor.wrapping_sub(1))
            .map(|change| format!("Undo {}", change.label()))
            .unwrap_or_else(|| "Undo".into());
        let redo = self
            .model
            .history
            .get(self.model.cursor)
            .map(|change| format!("Redo {}", change.label()))
            .unwrap_or_else(|| "Redo".into());
        let toolbar = row![
            text("Switchboot")
                .size(34)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                })
                .width(Length::Fill),
            button(text("Save").size(16))
                .padding([13, 28])
                .style(style::primary)
                .on_press_maybe((enabled && self.model.changed()).then_some(Message::Save)),
            button(text("Discard").size(16))
                .padding([13, 24])
                .style(style::button)
                .on_press_maybe((enabled && self.model.changed()).then_some(Message::Discard)),
            self.icon_control(
                "undo",
                undo,
                Message::Undo,
                enabled && self.model.cursor > 0
            ),
            self.icon_control(
                "redo",
                redo,
                Message::Redo,
                enabled && self.model.cursor < self.model.history.len()
            ),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center);
        let mut content = column![toolbar].spacing(24);
        if let Some(error) = &self.model.error {
            content = content.push(
                container(text(error).color(style::DANGER).size(14))
                    .padding(14)
                    .width(Length::Fill)
                    .style(|_| style::card(false)),
            );
        }
        if let Some(notice) = &self.notice {
            content = content.push(text(notice).size(14).color(style::MUTED));
        }
        if self.model.loading {
            content = content.push(text("Reading firmware boot entries…"));
        }
        if self.model.pending.is_some() {
            content = content.push(text("Applying operation…"));
        }
        if let Some(dialog) = &self.dialog {
            content = content.push(self.dialog_view(dialog));
        } else if self.model.initialized {
            let mut entries = Column::new()
                .spacing(12)
                .push(section_header("Boot Order", self.model.order.len()));
            entries = entries.push(
                text(if self.drag.is_some() {
                    "Drop between entries or into Others. Escape cancels."
                } else if self.model.changed() {
                    "Unsaved changes. Save to update firmware."
                } else {
                    "Drag to reorder, or use the arrows. Changes stay local until Save."
                })
                .size(13)
                .color(style::MUTED),
            );
            for (index, id) in self.model.order.iter().enumerate() {
                if let Some(entry) = self.model.entries.get(id) {
                    entries = entries.push(self.entry_view(entry, Some(index)));
                }
            }
            let append = container(
                text(if self.drag.is_some() {
                    "Drop here to add to the end of the boot order"
                } else {
                    ""
                })
                .size(13)
                .color(style::MUTED),
            )
            .height(32)
            .center_y(32)
            .width(Length::Fill);
            // Keep this zone in the layout when idle so beginning a drag does not move rows.
            entries = entries.push(drag::zone(
                append,
                drag::Zone::End,
                self.drag.is_some() && !self.model.busy(),
                self.drag.and_then(|state| state.target),
            ));
            let other_count = self
                .model
                .entries
                .values()
                .filter(|entry| !self.model.order.contains(&entry.id))
                .count()
                + 1;
            let mut other_entries = Column::new()
                .spacing(12)
                .push(section_header("Others", other_count));
            let mut firmware_body = column![
                row![
                    container(action_icon("firmware", true)).center_x(44),
                    text("EFI Firmware Setup").size(19).width(Length::Fill),
                    button(
                        text(if self.model.firmware_setup {
                            "Cancel setup"
                        } else {
                            "Boot to setup"
                        })
                        .size(14)
                    )
                    .padding([8, 14])
                    .style(style::button)
                    .on_press_maybe(
                        enabled.then_some(Message::Firmware(!self.model.firmware_setup))
                    ),
                    self.icon_control(
                        "more",
                        "Firmware actions",
                        Message::ToggleEntryActions(BootTarget::FirmwareSetup),
                        enabled
                    ),
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center)
            ]
            .spacing(14);
            if self.entry_actions == Some(BootTarget::FirmwareSetup) {
                firmware_body = firmware_body.push(
                    row![self.control(
                        "Create shortcut",
                        Message::OpenShortcut(BootTarget::FirmwareSetup),
                        enabled && !self.model.portable
                    ),]
                    .spacing(8),
                );
            }
            let firmware = container(firmware_body)
                .padding([20, 16])
                .width(Length::Fill)
                .style(|_| style::card(false));
            other_entries = other_entries.push(firmware);
            if self.model.discovering {
                other_entries = other_entries.push(text("Discovering other boot entries…"));
            }
            let mut others: Vec<_> = self
                .model
                .entries
                .values()
                .filter(|entry| !self.model.order.contains(&entry.id))
                .collect();
            others.sort_by_key(|entry| entry.id);
            for entry in others {
                other_entries = other_entries.push(self.entry_view(entry, None));
            }
            let can_remove = self
                .drag
                .is_some_and(|state| self.model.order.contains(&state.entry))
                && self.model.order.len() > 1
                && !self.model.busy();
            entries = entries.push(drag::zone(
                other_entries,
                drag::Zone::Others,
                can_remove,
                self.drag.and_then(|state| state.target),
            ));
            content = content.push(drag::viewport(
                scrollable(container(entries).padding(iced::Padding {
                    right: 16.0,
                    ..Default::default()
                }))
                .id("boot-entries")
                .height(Length::Fill),
                self.drag.is_some(),
                self.drag.map_or(0.0, |drag| drag.scroll_velocity),
            ));
        }
        content = content.push(
            row![
                self.control(
                    "Refresh",
                    Message::Refresh,
                    !self.model.loading
                        && self.model.pending.is_none()
                        && !self.model.discovering
                        && !self.model.changed()
                        && self.dialog.is_none()
                        && self.drag.is_none()
                ),
                self.control("Restart…", Message::RequestRestart, enabled),
                iced::widget::Space::new().width(Length::Fill),
                text("Ctrl+S save · Ctrl+Z undo")
                    .size(12)
                    .color(style::MUTED),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
        );
        container(content)
            .padding(32)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn dialog_view(&self, dialog: &Dialog) -> Element<'_, Message> {
        let idle = !self.model.busy();
        let body = match dialog {
            Dialog::Restart => column![
                text("Restart this computer?").size(24),
                text("Save your work in other applications before restarting. Unsaved boot order changes will be lost."),
                row![self.control("Cancel", Message::CancelDialog, idle), self.control("Restart now", Message::ConfirmRestart, idle)].spacing(12),
            ].spacing(16),
            Dialog::Shortcut { name, reboot, .. } => {
                let mut input = text_input("Shortcut name", name);
                let mut restart = checkbox(*reboot).label("Restart immediately when this shortcut runs");
                if idle {
                    input = input.on_input(Message::ShortcutName);
                    restart = restart.on_toggle(Message::ShortcutReboot);
                }
                column![
                    text("Create shortcut").size(24),
                    input,
                    text("The shortcut selects this target for the next boot."),
                    restart,
                    row![self.control("Cancel", Message::CancelDialog, idle), self.control("Create shortcut", Message::CreateShortcut, idle && !name.trim().is_empty())].spacing(12),
                ].spacing(16)
            }
        };
        container(body)
            .padding(24)
            .width(Length::Fill)
            .style(|_| style::card(false))
            .into()
    }
}

fn section_header(title: &str, count: usize) -> Element<'_, Message> {
    row![
        text(title)
            .size(22)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..Default::default()
            })
            .width(Length::Fill),
        text(format!(
            "{count} {}",
            if count == 1 { "entry" } else { "entries" }
        ))
        .size(14)
        .color(style::MUTED),
    ]
    .align_y(iced::Alignment::Center)
    .padding([8, 4])
    .into()
}

fn badge(label: &'static str, kind: style::Badge) -> Element<'static, Message> {
    container(text(label).size(12).font(iced::Font {
        weight: iced::font::Weight::Semibold,
        ..Default::default()
    }))
    .padding([6, 10])
    .style(move |_| style::badge(kind))
    .into()
}

/// Vector controls avoid platform-dependent arrow/ellipsis font glyphs.
fn action_icon(name: &str, enabled: bool) -> Element<'static, Message> {
    let path = match name {
        "up" => "<path d='M12 19V5m-5 5 5-5 5 5'/>",
        "down" => "<path d='M12 5v14m-5-5 5 5 5-5'/>",
        "remove" => "<path d='m6 6 12 12M18 6 6 18'/>",
        "add" => "<path d='M12 5v14M5 12h14'/>",
        "undo" => "<path d='M8 5 3 10l5 5M3 10h10a7 7 0 0 1 7 7v2'/>",
        "redo" => "<path d='m16 5 5 5-5 5m5-5H11a7 7 0 0 0-7 7v2'/>",
        "grip" => {
            "<path d='M8 6h.01M16 6h.01M8 12h.01M16 12h.01M8 18h.01M16 18h.01' stroke-width='3'/>"
        }
        "firmware" => {
            "<rect x='6' y='6' width='12' height='12' rx='2'/><path d='M9 9h6v6H9zM9 3v3m6-3v3m-6 12v3m6-3v3M3 9h3m-3 6h3m12-6h3m-3 6h3'/>"
        }
        _ => "<path d='M12 5h.01M12 12h.01M12 19h.01' stroke-width='3'/>",
    };
    let color = if enabled { "#bcbcbc" } else { "#686868" };
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='{color}' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'>{path}</svg>"
    );
    iced::widget::svg(iced::widget::svg::Handle::from_memory(svg.into_bytes()))
        .width(20)
        .height(20)
        .into()
}

fn os_icon(icon: &str) -> Element<'static, Message> {
    let bytes: &[u8] = match icon {
        "windows" => include_bytes!("../../../static/os-icons/windows.svg"),
        "ubuntu" => include_bytes!("../../../static/os-icons/ubuntu.svg"),
        "neon" => include_bytes!("../../../static/os-icons/neon.svg"),
        "popos" => include_bytes!("../../../static/os-icons/popos.svg"),
        "debian" => include_bytes!("../../../static/os-icons/debian.svg"),
        "fedora" => include_bytes!("../../../static/os-icons/fedora.svg"),
        "suse" => include_bytes!("../../../static/os-icons/suse.svg"),
        "cachyos" => include_bytes!("../../../static/os-icons/cachyos.svg"),
        "manjaro" => include_bytes!("../../../static/os-icons/manjaro.svg"),
        "arch" => include_bytes!("../../../static/os-icons/arch.svg"),
        "nixos" => include_bytes!("../../../static/os-icons/nixos.svg"),
        "deepin" => include_bytes!("../../../static/os-icons/deepin.svg"),
        "macos" => include_bytes!("../../../static/os-icons/macos.svg"),
        "linux" => include_bytes!("../../../static/os-icons/linux.svg"),
        _ => return text("EFI").size(13).width(32).into(),
    };
    iced::widget::svg(iced::widget::svg::Handle::from_memory(bytes))
        .width(34)
        .height(34)
        .into()
}

/// Safe in-memory backend for UI development. It never touches firmware or restarts.
pub struct DemoBackend {
    state: Mutex<Snapshot>,
}

impl Default for DemoBackend {
    fn default() -> Self {
        Self {
            state: Mutex::new(Snapshot {
                entries: vec![
                    Entry {
                        id: 0,
                        description: "Windows Boot Manager".into(),
                    },
                    Entry {
                        id: 1,
                        description: "Fedora".into(),
                    },
                    Entry {
                        id: 2,
                        description: "Ubuntu".into(),
                    },
                ],
                order: vec![0, 1],
                boot_next: None,
                current: Some(0),
                firmware_setup: false,
                portable: false,
            }),
        }
    }
}

impl Backend for DemoBackend {
    fn load(&self) -> Result<Snapshot, String> {
        self.state
            .lock()
            .map(|state| state.clone())
            .map_err(|error| error.to_string())
    }
    fn discover(&self) -> Result<Vec<Entry>, String> {
        Ok(self.load()?.entries)
    }
    fn set_order(&self, order: Vec<u16>) -> Result<(), String> {
        self.state.lock().map_err(|error| error.to_string())?.order = order;
        Ok(())
    }
    fn set_next(&self, next: Option<u16>) -> Result<(), String> {
        self.state
            .lock()
            .map_err(|error| error.to_string())?
            .boot_next = next;
        Ok(())
    }
    fn set_firmware(&self, enabled: bool) -> Result<(), String> {
        self.state
            .lock()
            .map_err(|error| error.to_string())?
            .firmware_setup = enabled;
        Ok(())
    }
    fn create_shortcut(&self, _: Shortcut) -> Result<(), String> {
        Ok(())
    }
    fn restart(&self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let backend = Arc::new(DemoBackend::default());
        let snapshot = backend.load().unwrap();
        let (mut app, _) = App::new(backend);
        app.model.loading = false;
        app.model.initialize(snapshot).unwrap();
        app
    }

    #[test]
    fn entry_menus_are_local_and_respect_operation_and_drag_guards() {
        let mut app = app();
        let target = BootTarget::Entry(0);
        let _ = app.update(Message::ToggleEntryActions(target));
        assert_eq!(app.entry_actions, Some(target));
        assert_eq!(app.model.order, vec![0, 1]);
        assert!(app.model.history.is_empty());
        assert!(app.model.pending.is_none());

        let _ = app.update(Message::BootNext(Some(0)));
        assert!(app.entry_actions.is_none());
        let _ = app.update(Message::ToggleEntryActions(BootTarget::FirmwareSetup));
        assert!(app.entry_actions.is_none());
        let _ = app.update(Message::Completed(Err("Mock firmware failure".into())));
        assert_eq!(app.model.boot_next, None);
        assert!(app.model.history.is_empty());

        let _ = app.update(Message::ToggleEntryActions(target));
        let _ = app.update(Message::DragStart(0));
        assert!(app.entry_actions.is_none());
        let _ = app.update(Message::ToggleEntryActions(target));
        assert!(app.entry_actions.is_none());
        let _ = app.update(Message::DragCancel);
        let _ = app.update(Message::ToggleEntryActions(target));
        let _ = app.update(Message::ToggleEntryActions(target));
        assert!(app.entry_actions.is_none());
    }

    #[test]
    fn restart_requires_confirmation_and_cancel_does_not_schedule_backend() {
        let mut app = app();
        let _ = app.update(Message::ConfirmRestart);
        assert!(app.model.pending.is_none());
        let _ = app.update(Message::RequestRestart);
        assert!(matches!(app.dialog, Some(Dialog::Restart)));
        assert!(app.model.pending.is_none());
        let _ = app.update(Message::CancelDialog);
        assert!(app.dialog.is_none());
        assert!(app.model.pending.is_none());
        let _ = app.update(Message::RequestRestart);
        let _ = app.update(Message::ConfirmRestart);
        assert!(matches!(app.model.pending, Some(Operation::Restart)));
    }

    #[test]
    fn failed_shortcut_keeps_form_for_retry_and_blocks_double_submission() {
        let mut app = app();
        let _ = app.update(Message::OpenShortcut(BootTarget::Entry(0)));
        let _ = app.update(Message::ShortcutName("Boot Windows".into()));
        let _ = app.update(Message::CreateShortcut);
        assert!(
            matches!(app.model.pending, Some(Operation::Shortcut(ref shortcut)) if shortcut.name == "Boot Windows")
        );
        let _ = app.update(Message::CancelDialog);
        assert!(app.dialog.is_some());
        let _ = app.update(Message::Completed(Err(
            "Shortcut directory is read only".into()
        )));
        assert!(
            matches!(app.dialog, Some(Dialog::Shortcut { ref name, .. }) if name == "Boot Windows")
        );
        assert!(app.model.error.as_deref().unwrap().contains("read only"));
        let _ = app.update(Message::CreateShortcut);
        let _ = app.update(Message::Completed(Ok(())));
        assert!(app.dialog.is_none());
    }

    #[test]
    fn initialization_failure_allows_retry_without_fabricated_firmware_state() {
        let (mut app, _) = App::new(Arc::new(DemoBackend::default()));
        let _ = app.update(Message::Loaded(Err("EFI access denied".into())));
        assert!(!app.model.initialized);
        assert!(!app.model.loading);
        assert!(app.model.busy());
        assert_eq!(app.model.error.as_deref(), Some("EFI access denied"));
        let _ = app.update(Message::Refresh);
        assert!(app.model.loading);
    }

    #[test]
    fn no_op_firmware_messages_do_not_create_history_or_tasks() {
        let mut app = app();
        let _ = app.update(Message::BootNext(None));
        let _ = app.update(Message::Firmware(false));
        assert!(app.model.pending.is_none());
        assert!(app.model.history.is_empty());
    }

    #[test]
    fn drag_preview_is_transactional_and_cancel_preserves_draft_and_history() {
        let mut app = app();
        let _ = app.update(Message::DragStart(0));
        let _ = app.update(Message::DragHover(DropTarget::After(1)));
        assert_eq!(app.model.order, vec![0, 1]);
        assert!(app.model.history.is_empty());
        let _ = app.update(Message::Save);
        let _ = app.update(Message::Move(0, 1));
        let _ = app.update(Message::BootNext(Some(1)));
        let _ = app.update(Message::Undo);
        assert_eq!(app.model.order, vec![0, 1]);
        assert!(app.model.pending.is_none());
        let _ = app.update(Message::DragCancel);
        assert!(app.drag.is_none());
        assert_eq!(app.model.order, vec![0, 1]);
        assert!(app.model.history.is_empty());
    }

    #[test]
    fn drop_commits_exactly_once_and_never_starts_firmware_operation() {
        let mut app = app();
        let _ = app.update(Message::DragStart(0));
        let _ = app.update(Message::DragDrop(DropTarget::After(1)));
        let _ = app.update(Message::DragDrop(DropTarget::Before(1)));
        assert_eq!(app.model.order, vec![1, 0]);
        assert_eq!(app.model.baseline, vec![0, 1]);
        assert_eq!(app.model.cursor, 1);
        assert!(app.model.pending.is_none());
        assert!(app.drag.is_none());
    }

    #[test]
    fn delayed_drop_is_ignored_after_cancel_and_busy_state_rejects_drag() {
        let mut app = app();
        let _ = app.update(Message::DragStart(0));
        let _ = app.update(Message::DragCancel);
        let _ = app.update(Message::DragDrop(DropTarget::After(1)));
        assert_eq!(app.model.order, vec![0, 1]);
        app.model.begin(Operation::Restart);
        let _ = app.update(Message::DragStart(0));
        assert!(app.drag.is_none());
    }
}
