//! Native Switchboot UI. Firmware services are supplied by the host through [`Backend`].
mod app_icon;
mod demo;
mod drag;
mod model;
mod motion;
mod movement;
mod style;
mod update_controller;
mod update_view;
mod updates;

use std::sync::Arc;

pub use demo::DemoBackend;
use iced::widget::{
    Column, button, checkbox, column, container, mouse_area, row, scrollable, text, text_input,
};
use iced::{Element, Length, Subscription, Task};
pub use model::{Backend, BootTarget, Entry, Shortcut, Snapshot};
use model::{Change, DropTarget, Model, Operation};
pub use updates::{UpdateProgress, UpdateRelease};

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
    let window = iced::window::Settings {
        size: iced::Size::new(650.0, 650.0),
        min_size: Some(iced::Size::new(500.0, 360.0)),
        icon: Some(app_icon::window_icon()),
        ..iced::window::Settings::default()
    };
    #[cfg(target_os = "linux")]
    let window = iced::window::Settings {
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: "dev.zidane.switchboot".into(),
            ..Default::default()
        },
        ..window
    };
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
    .theme(App::theme)
    .subscription(App::subscription)
    .window(window)
    .centered()
    .run()
}

struct App {
    updates: updates::State,
    update_progress: UpdateProgress,
    backend: Arc<dyn Backend>,
    model: Model,
    dialog: Option<Dialog>,
    notice: Option<String>,
    drag: Option<DragState>,
    activation: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    entry_actions: Option<BootTarget>,
    restart_confirmation: Option<BootTarget>,
    positions: movement::Positions,
    system_theme: Option<iced::theme::Mode>,
}

#[derive(Clone, Copy)]
struct LayoutDensity {
    narrow: bool,
}

impl LayoutDensity {
    fn for_width(width: f32) -> Self {
        Self {
            narrow: width < 600.0,
        }
    }

    fn control_size(self) -> f32 {
        30.0
    }
    fn gap(self) -> f32 {
        6.0
    }
}

#[derive(Debug, Clone, Copy)]
struct DragState {
    entry: u16,
    target: Option<DropTarget>,
    scroll_velocity: f32,
}

#[derive(Debug, Clone)]
enum Dialog {
    Shortcut {
        target: BootTarget,
        name: String,
        reboot: bool,
    },
}

#[derive(Debug, Clone)]
enum Message {
    UpdateChecked(Result<Option<UpdateRelease>, String>),
    InstallUpdate,
    UpdateInstalled(Result<(), String>),
    UpdateProgress(UpdateProgress),
    PollUpdate,
    UpdatePolled(Result<bool, String>),
    PollUpgradeShutdown,
    Loaded(Result<Snapshot, String>),
    Discovered(Result<Vec<Entry>, String>),
    Completed(Result<(), String>),
    Refresh,
    SystemThemeChanged(iced::theme::Mode),
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
    RequestRestart(BootTarget),
    UndoNext(BootTarget),
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
                .map_err(|error| format!("Background task failed: {error}"))?
        },
        complete,
    )
}

impl App {
    fn new(backend: Arc<dyn Backend>) -> (Self, Task<Message>) {
        let mut app = Self {
            updates: updates::State::new(backend.updates_supported()),
            update_progress: UpdateProgress::default(),
            backend,
            model: Model::default(),
            dialog: None,
            notice: None,
            drag: None,
            activation: None,
            entry_actions: None,
            restart_confirmation: None,
            positions: movement::Positions::default(),
            system_theme: None,
        };
        let task = Task::batch([
            app.load(),
            app.check_update(),
            iced::system::theme().map(Message::SystemThemeChanged),
        ]);
        (app, task)
    }

    fn load(&mut self) -> Task<Message> {
        self.positions.clear();
        self.entry_actions = None;
        self.model.loading = true;
        self.model.error = None;
        let backend = self.backend.clone();
        worker(move || backend.load(), Message::Loaded)
    }

    fn operation(&mut self, operation: Operation) -> Task<Message> {
        if self.updates.installing() || self.drag.is_some() || !self.model.begin(operation.clone())
        {
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

    fn theme(&self) -> Option<iced::Theme> {
        self.system_theme
            .map(|mode| style::theme(mode == iced::theme::Mode::Light))
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            iced::system::theme_changes().map(Message::SystemThemeChanged),
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
        subscriptions.push(self.update_subscription());
        if self.drag.is_some_and(|drag| drag.scroll_velocity != 0.0) {
            subscriptions.push(
                iced::time::every(std::time::Duration::from_millis(32))
                    .map(|_| Message::DragScrollTick),
            );
        }
        Subscription::batch(subscriptions)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.update_message(message);
        self.positions
            .sync(&self.display_order(), self.drag.map(|drag| drag.entry));
        task
    }

    fn display_order(&self) -> Vec<u16> {
        self.drag
            .and_then(|drag| {
                drag.target
                    .and_then(|target| self.model.preview_drop(drag.entry, target))
            })
            .unwrap_or_else(|| self.model.order.clone())
    }

    fn update_message(&mut self, message: Message) -> Task<Message> {
        if message.is_update() {
            return self.update_updates(message);
        }
        // Keep firmware and draft operations excluded while Setup is running.
        // The handoff event authorizes GUI exit after Setup finishes preflight.
        if self.updates.installing()
            && !matches!(
                message,
                Message::PollActivation | Message::SystemThemeChanged(_)
            )
        {
            return Task::none();
        }
        // Confirmation belongs to one target and ends when another action is taken.
        if !matches!(
            message,
            Message::RequestRestart(_) | Message::PollActivation | Message::SystemThemeChanged(_)
        ) {
            self.restart_confirmation = None;
        }
        match message {
            Message::SystemThemeChanged(mode) => {
                self.system_theme = Some(mode);
                return Task::none();
            }
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
                let saved = matches!(self.model.pending, Some(Operation::Save(_)));
                self.model.complete(result);
                if succeeded {
                    self.notice = if shortcut {
                        Some("Shortcut created.".into())
                    } else if saved {
                        Some("Boot order saved.".into())
                    } else {
                        None
                    };
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
                    Message::UndoNext(target) if self.is_selected(target) => {
                        return self.undo_next(target);
                    }
                    Message::RequestRestart(target) if self.is_selected(target) => {
                        if self.restart_confirmation == Some(target) {
                            self.restart_confirmation = None;
                            return self.operation(Operation::Restart);
                        }
                        self.restart_confirmation = Some(target);
                    }
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
            ("linpus", "linux"),
            ("linux", "linux"),
            ("gnu", "linux"),
        ]
        .into_iter()
        .find(|(pattern, _)| description.contains(pattern))
        .map(|(_, icon)| icon)
        .unwrap_or("generic")
    }

    fn is_selected(&self, target: BootTarget) -> bool {
        match target {
            BootTarget::Entry(id) => self.model.boot_next == Some(id),
            BootTarget::FirmwareSetup => self.model.firmware_setup,
        }
    }

    /// Undo this selection without undoing unrelated order edits made afterwards.
    fn undo_next(&mut self, target: BootTarget) -> Task<Message> {
        let selection = self.model.history[..self.model.cursor]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, change)| {
                matches!(
                    (target, change),
                    (BootTarget::Entry(_), Change::BootNext { .. })
                        | (BootTarget::FirmwareSetup, Change::Firmware { .. })
                )
            });
        if let Some((index, change)) = selection {
            if index + 1 == self.model.cursor {
                return self.operation(Operation::Undo(change.clone()));
            }
            let message = match change {
                Change::BootNext { before, .. } => Message::BootNext(*before),
                Change::Firmware { before, .. } => Message::Firmware(*before),
                Change::Order { .. } => unreachable!(),
            };
            return self.update(message);
        }
        // A target already selected when the app loaded can still be canceled.
        self.update(match target {
            BootTarget::Entry(_) => Message::BootNext(None),
            BootTarget::FirmwareSetup => Message::Firmware(false),
        })
    }

    fn next_controls(&self, target: BootTarget, enabled: bool) -> Element<'_, Message> {
        let size = 12;
        let padding = [6, 8];
        if self.is_selected(target) {
            let restart_content: Element<'_, Message> = if self.restart_confirmation == Some(target)
            {
                row![
                    vector_icon(
                        "refresh",
                        if enabled {
                            style::IconTone::OnColor
                        } else {
                            style::IconTone::Disabled
                        },
                        14
                    ),
                    text("Confirm?").size(size),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center)
                .into()
            } else {
                text("Restart").size(size).into()
            };
            row![
                motion::button(
                    button(text("Undo").size(size))
                        .padding(padding)
                        .on_press_maybe(enabled.then_some(Message::UndoNext(target))),
                    enabled,
                    style::undo_next,
                ),
                motion::button(
                    button(container(restart_content).center_x(Length::Fill))
                        .padding(padding)
                        .width(Length::Fixed(88.0))
                        .on_press_maybe(enabled.then_some(Message::RequestRestart(target))),
                    enabled,
                    style::restart,
                ),
            ]
            .spacing(6)
            .into()
        } else {
            motion::button(
                button(text("BootNext").size(size))
                    .padding(padding)
                    .on_press_maybe(enabled.then_some(match target {
                        BootTarget::Entry(id) => Message::BootNext(Some(id)),
                        BootTarget::FirmwareSetup => Message::Firmware(true),
                    })),
                enabled,
                style::button,
            )
        }
    }

    fn entry_action<'a>(
        &self,
        icon: &'static str,
        label: &'static str,
        message: Message,
        enabled: bool,
    ) -> Element<'a, Message> {
        motion::button(
            button(
                row![
                    vector_icon(icon, style::IconTone::for_enabled(enabled), 16),
                    text(label).size(12),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .padding([6, 10])
            .on_press_maybe(enabled.then_some(message)),
            enabled,
            style::button,
        )
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
        motion::button(control, enabled, style::button)
    }

    fn icon_control_sized<'a>(
        &self,
        icon: &'static str,
        label: impl Into<String>,
        message: Message,
        enabled: bool,
        size: f32,
    ) -> Element<'a, Message> {
        let mut control = button(
            container(action_icon(icon, enabled))
                .center_x(size)
                .center_y(size),
        )
        .padding(0);
        if enabled {
            control = control.on_press(message);
        }
        iced::widget::tooltip(
            motion::button(
                control,
                enabled,
                if matches!(icon, "more" | "undo" | "redo" | "refresh") {
                    style::quiet
                } else {
                    style::button
                },
            ),
            container(text(label.into()).size(13))
                .padding([6, 10])
                .style(|theme| style::card(theme, false)),
            iced::widget::tooltip::Position::Bottom,
        )
        .into()
    }

    fn entry_view(
        &self,
        entry: &Entry,
        position: Option<usize>,
        density: LayoutDensity,
    ) -> Element<'_, Message> {
        let current = self.model.current == Some(entry.id);
        let selected = self.model.boot_next == Some(entry.id);
        let card = motion::card(
            container(self.entry_body(entry, position, density)).width(Length::Fill),
            current,
            selected,
        );
        let card = movement::item(card, u32::from(entry.id), &self.positions);
        if position.is_some() {
            drag::zone(
                card,
                if self.drag.is_some_and(|drag| drag.entry == entry.id) {
                    drag::Zone::Placeholder(entry.id)
                } else {
                    drag::Zone::Row(entry.id)
                },
                self.drag.is_some() && !self.model.busy(),
                self.drag.and_then(|state| state.target),
            )
        } else {
            card
        }
    }

    fn entry_body(
        &self,
        entry: &Entry,
        position: Option<usize>,
        density: LayoutDensity,
    ) -> Element<'_, Message> {
        let enabled = !self.updates.installing()
            && !self.model.busy()
            && self.dialog.is_none()
            && self.drag.is_none();
        let target = BootTarget::Entry(entry.id);
        let mut badges = row![].spacing(6).align_y(iced::Alignment::Center);
        if self.model.current == Some(entry.id) {
            badges = badges.push(badge("Current", style::Badge::Current));
        }
        if self.model.baseline.first() == Some(&entry.id) {
            badges = badges.push(badge("Default", style::Badge::Default));
        }
        let mut leading = row![]
            .spacing(density.gap())
            .align_y(iced::Alignment::Center);
        if let Some(index) = position {
            leading = leading.push(
                text(format!("{}.", index + 1))
                    .size(12)
                    .style(style::muted)
                    .width(16),
            );
        }
        leading = leading.push(os_icon(self.icon_id(target), &entry.description));
        let name = text(entry.description.clone())
            .size(16)
            .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
            .width(Length::Fill);
        let mut title = row![container(leading).center_y(30)]
            .spacing(density.gap())
            .width(Length::Fill)
            .align_y(iced::Alignment::Center);
        let has_badges =
            self.model.current == Some(entry.id) || self.model.baseline.first() == Some(&entry.id);
        title = title.push(name);
        if has_badges {
            title = title.push(badges);
        }
        let mut handle = mouse_area(container(title).width(Length::Fill).padding(iced::Padding {
            top: 16.0,
            right: 6.0,
            bottom: 16.0,
            left: 12.0,
        }));
        if enabled && (position.is_none() || self.model.order.len() > 1) {
            handle = handle
                .on_press(Message::DragStart(entry.id))
                .interaction(iced::mouse::Interaction::Grab);
        }
        let mut controls = row![self.next_controls(target, enabled)]
            .spacing(6)
            .align_y(iced::Alignment::Center);
        let order_controls = |index: usize| {
            let mut controls = row![];
            if !density.narrow {
                controls = controls
                    .push(self.icon_control_sized(
                        "up",
                        "Move up",
                        Message::Move(entry.id, -1),
                        enabled && index > 0,
                        density.control_size(),
                    ))
                    .push(self.icon_control_sized(
                        "down",
                        "Move down",
                        Message::Move(entry.id, 1),
                        enabled && index + 1 < self.model.order.len(),
                        density.control_size(),
                    ));
            }
            controls
                .push(self.icon_control_sized(
                    "remove",
                    "Remove from boot order",
                    Message::Exclude(entry.id),
                    enabled && self.model.order.len() > 1,
                    density.control_size(),
                ))
                .spacing(6)
        };
        if let Some(index) = position {
            controls = controls.push(order_controls(index));
        } else {
            controls = controls.push(self.icon_control_sized(
                "add",
                "Add to boot order",
                Message::Include(entry.id),
                enabled,
                density.control_size(),
            ));
        }
        controls = controls.push(self.icon_control_sized(
            "more",
            "Entry actions",
            Message::ToggleEntryActions(target),
            enabled,
            density.control_size(),
        ));
        let mut body = column![
            row![
                handle,
                container(controls).padding(iced::Padding {
                    top: 16.0,
                    right: 12.0,
                    bottom: 16.0,
                    left: 0.0,
                }),
            ]
            .align_y(iced::Alignment::Center)
        ];
        {
            let mut actions = row![self.entry_action(
                "shortcut",
                "Create shortcut",
                Message::OpenShortcut(target),
                enabled && !self.model.portable,
            )]
            .spacing(8);
            if let Some(index) = position {
                actions = actions.push(self.entry_action(
                    "default",
                    "Make default",
                    Message::MakeDefault(entry.id),
                    enabled && index > 0,
                ));
            }
            body = body.push(motion::reveal(
                container(actions).padding(iced::Padding {
                    top: 0.0,
                    right: 12.0,
                    bottom: 16.0,
                    left: 12.0,
                }),
                self.entry_actions == Some(target),
            ));
        }
        body.into()
    }

    fn view(&self) -> Element<'_, Message> {
        iced::widget::responsive(move |size| self.view_sized(LayoutDensity::for_width(size.width)))
            .into()
    }

    fn view_sized(&self, density: LayoutDensity) -> Element<'_, Message> {
        let display_order = self.display_order();
        let enabled = !self.updates.installing()
            && !self.model.busy()
            && self.dialog.is_none()
            && self.drag.is_none();
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
        let mut title = column![text("Switchboot").size(30).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        })]
        .spacing(2)
        .width(Length::Fill);
        if let Some(link) = self
            .updates
            .link(self.can_install_update(), self.model.changed())
        {
            title = title.push(link);
        }
        let mut toolbar = row![
            title,
            motion::button(
                button(text("Save").size(14))
                    .padding([10, 16])
                    .on_press_maybe((enabled && self.model.changed()).then_some(Message::Save)),
                enabled && self.model.changed(),
                style::primary
            ),
            motion::button(
                button(text("Discard").size(14))
                    .padding([10, 16])
                    .on_press_maybe((enabled && self.model.changed()).then_some(Message::Discard)),
                enabled && self.model.changed(),
                style::button
            ),
        ]
        .spacing(density.gap())
        .align_y(iced::Alignment::Center);
        toolbar = toolbar
            .push(self.icon_control_sized(
                "undo",
                undo,
                Message::Undo,
                enabled && self.model.cursor > 0,
                30.0,
            ))
            .push(self.icon_control_sized(
                "redo",
                redo,
                Message::Redo,
                enabled && self.model.cursor < self.model.history.len(),
                30.0,
            ));
        toolbar = toolbar.push(self.icon_control_sized(
            "refresh",
            "Refresh boot entries",
            Message::Refresh,
            !self.model.loading
                && self.model.pending.is_none()
                && !self.model.discovering
                && !self.model.changed()
                && self.dialog.is_none()
                && self.drag.is_none(),
            30.0,
        ));
        let mut content = column![toolbar].spacing(20);
        if let Some(error) = &self.model.error {
            content = content.push(
                container(text(error).style(style::danger).size(14))
                    .padding(14)
                    .width(Length::Fill)
                    .style(|theme| style::card(theme, false)),
            );
        }
        if self.model.loading {
            content = content.push(text("Reading firmware boot entries…"));
        }
        if let Some(dialog) = &self.dialog {
            content = content.push(scrollable(self.dialog_view(dialog)).height(Length::Fill));
        } else if self.model.initialized {
            let mut entries = Column::new()
                .spacing(12)
                .push(section_header("Boot Order", display_order.len()))
                .push(
                    text("Drag items to reorder or move between sections.")
                        .size(12)
                        .style(style::muted),
                );
            entries = entries.push(
                iced::widget::keyed_column(display_order.iter().enumerate().filter_map(
                    |(index, id)| {
                        self.model
                            .entries
                            .get(id)
                            .map(|entry| (*id, self.entry_view(entry, Some(index), density)))
                    },
                ))
                .spacing(12),
            );
            let append = container(
                text(if self.drag.is_some() {
                    "Drop here to add to the end of the boot order"
                } else {
                    ""
                })
                .size(13)
                .style(style::muted),
            )
            .height(20)
            .center_y(20)
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
                .filter(|entry| !display_order.contains(&entry.id))
                .count()
                + 1;
            let mut other_entries = Column::new().spacing(12).push(movement::item(
                section_header("Others", other_count),
                movement::OTHERS_HEADING,
                &self.positions,
            ));
            let mut firmware_body = column![
                row![
                    container(action_icon("firmware", true)).center_x(28),
                    text("EFI Firmware Setup").size(16).width(Length::Fill),
                    row![
                        self.next_controls(BootTarget::FirmwareSetup, enabled),
                        self.icon_control_sized(
                            "add",
                            "Firmware setup cannot be added to the boot order",
                            Message::Firmware(true),
                            false,
                            density.control_size()
                        ),
                        self.icon_control_sized(
                            "more",
                            "Firmware actions",
                            Message::ToggleEntryActions(BootTarget::FirmwareSetup),
                            enabled,
                            density.control_size()
                        ),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                ]
                .spacing(density.gap())
                .align_y(iced::Alignment::Center)
            ];
            {
                firmware_body = firmware_body.push(motion::reveal(
                    container(
                        row![self.entry_action(
                            "shortcut",
                            "Create shortcut",
                            Message::OpenShortcut(BootTarget::FirmwareSetup),
                            enabled && !self.model.portable
                        ),]
                        .spacing(6),
                    )
                    .padding(iced::Padding {
                        top: 14.0,
                        ..Default::default()
                    }),
                    self.entry_actions == Some(BootTarget::FirmwareSetup),
                ));
            }
            let firmware = motion::card(
                container(firmware_body)
                    .padding([16, 12])
                    .width(Length::Fill),
                false,
                self.model.firmware_setup,
            );
            other_entries = other_entries.push(movement::item(
                firmware,
                movement::FIRMWARE,
                &self.positions,
            ));
            if self.model.discovering {
                other_entries = other_entries.push(text("Discovering other boot entries…"));
            }
            let mut others: Vec<_> = self
                .model
                .entries
                .values()
                .filter(|entry| !display_order.contains(&entry.id))
                .collect();
            others.sort_by_key(|entry| entry.id);
            other_entries = other_entries.push(
                iced::widget::keyed_column(
                    others
                        .into_iter()
                        .map(|entry| (entry.id, self.entry_view(entry, None, density))),
                )
                .spacing(12),
            );
            let can_remove = self.drag.is_some_and(|state| {
                !self.model.order.contains(&state.entry) || self.model.order.len() > 1
            }) && !self.model.busy();
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
        let status = if self.model.pending.is_some() {
            "Applying operation..."
        } else if let Some(notice) = self.notice.as_deref() {
            notice
        } else if self.model.changed() {
            "Unsaved changes. Save to update firmware."
        } else {
            ""
        };
        let footer = row![
            text(status)
                .size(12)
                .style(style::muted)
                .width(Length::Fill),
        ];
        content = content.push(footer.align_y(iced::Alignment::Center));
        let base: Element<'_, Message> = container(content)
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
        if let Some(modal) = self.updates.modal(self.update_progress) {
            let overlay = container(iced::widget::opaque(modal))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .padding(24)
                .style(|_| container::Style {
                    background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.65).into()),
                    ..Default::default()
                });
            iced::widget::stack![base, iced::widget::opaque(overlay)].into()
        } else {
            base
        }
    }

    fn dialog_view(&self, dialog: &Dialog) -> Element<'_, Message> {
        let idle = !self.model.busy();
        let body = match dialog {
            Dialog::Shortcut { name, reboot, .. } => {
                let mut input = text_input("Shortcut name", name);
                let mut restart =
                    checkbox(*reboot).label("Restart immediately when this shortcut runs");
                if idle {
                    input = input.on_input(Message::ShortcutName);
                    restart = restart.on_toggle(Message::ShortcutReboot);
                }
                column![
                    text("Create shortcut").size(24),
                    input,
                    text("The shortcut selects this target for the next boot."),
                    restart,
                    row![
                        self.control("Cancel", Message::CancelDialog, idle),
                        self.control(
                            "Create shortcut",
                            Message::CreateShortcut,
                            idle && !name.trim().is_empty()
                        )
                    ]
                    .spacing(12),
                ]
                .spacing(16)
            }
        };
        container(body)
            .padding(24)
            .width(Length::Fill)
            .style(|theme| style::card(theme, false))
            .into()
    }
}

fn section_header(title: &str, count: usize) -> Element<'_, Message> {
    row![
        text(title)
            .size(20)
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
        .style(style::muted),
    ]
    .align_y(iced::Alignment::Center)
    .padding([8, 4])
    .into()
}

fn badge(label: &'static str, kind: style::Badge) -> Element<'static, Message> {
    container(text(label).size(11).font(iced::Font {
        weight: iced::font::Weight::Semibold,
        ..Default::default()
    }))
    .padding([4, 6])
    .style(move |theme| style::badge(theme, kind))
    .into()
}

/// Vector controls avoid platform-dependent arrow/ellipsis font glyphs.
fn action_icon(name: &str, enabled: bool) -> Element<'static, Message> {
    if name == "firmware" {
        return iced::widget::svg(iced::widget::svg::Handle::from_memory(include_bytes!(
            "../../../static/os-icons/firmware.svg"
        )))
        .width(20)
        .height(20)
        .style(move |theme, _| style::icon(theme, style::IconTone::for_enabled(enabled)))
        .into();
    }
    vector_icon(name, style::IconTone::for_enabled(enabled), 20)
}

fn vector_icon(name: &str, tone: style::IconTone, size: u32) -> Element<'static, Message> {
    let path = match name {
        "up" => "<path d='M12 19V5m-5 5 5-5 5 5'/>",
        "down" => "<path d='M12 5v14m-5-5 5 5 5-5'/>",
        "remove" => "<path d='m6 6 12 12M18 6 6 18'/>",
        "add" => "<path d='M12 5v14M5 12h14'/>",
        "undo" => "<path d='M8 5 3 10l5 5M3 10h10a7 7 0 0 1 7 7v2'/>",
        "redo" => "<path d='m16 5 5 5-5 5m5-5H11a7 7 0 0 0-7 7v2'/>",
        "refresh" => "<path d='M20 11a8 8 0 1 0-2 7M20 4v7h-7'/>",
        "shortcut" => {
            "<path d='M14 4h6v6m0-6L10 14M10 5H5a1 1 0 0 0-1 1v13a1 1 0 0 0 1 1h13a1 1 0 0 0 1-1v-5'/>"
        }
        "default" => {
            "<path d='m12 3 2.8 5.7 6.2.9-4.5 4.4 1.1 6.2-5.6-3-5.6 3 1.1-6.2L3 9.6l6.2-.9z'/>"
        }

        _ => "<path d='M12 5h.01M12 12h.01M12 19h.01' stroke-width='3'/>",
    };
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'>{path}</svg>"
    );
    iced::widget::svg(iced::widget::svg::Handle::from_memory(svg.into_bytes()))
        .width(size)
        .height(size)
        .style(move |theme, _| style::icon(theme, tone))
        .into()
}

#[test]
fn system_theme_changes_only_affect_appearance() {
    let (mut app, _) = App::new(Arc::new(DemoBackend::default()));
    let order = app.model.order.clone();
    let target = BootTarget::FirmwareSetup;
    app.restart_confirmation = Some(target);
    assert!(app.theme().is_none());
    for mode in [iced::theme::Mode::Light, iced::theme::Mode::Dark] {
        let _ = app.update(Message::SystemThemeChanged(mode));
        assert_eq!(app.system_theme, Some(mode));
        assert_eq!(
            app.theme().unwrap().extended_palette().is_dark,
            mode != iced::theme::Mode::Light
        );
        assert_eq!(app.model.order, order);
        assert!(app.model.pending.is_none());
        assert_eq!(app.restart_confirmation, Some(target));
    }
}

#[test]
fn efi_icons_match_device_descriptions() {
    for (description, expected) in [
        ("EFI Network", "network"),
        ("UEFI PXE IPv4", "network"),
        ("EFI DVD/CDROM", "optical"),
        ("EFI USB Device", "usb"),
        ("EFI USB Device (Samsung SSD 970 EVO 500G)", "usb"),
        ("Unknown boot entry", "drive"),
    ] {
        assert_eq!(efi_icon(description), expected);
    }
}

fn efi_icon(description: &str) -> &'static str {
    let description = description.to_lowercase();
    if description.contains("usb") {
        "usb"
    } else if ["network", "pxe", "ipv4", "ipv6", "ethernet"]
        .iter()
        .any(|pattern| description.contains(pattern))
    {
        "network"
    } else if ["dvd", "cdrom", "cd-rom", "optical"]
        .iter()
        .any(|pattern| description.contains(pattern))
    {
        "optical"
    } else {
        "drive"
    }
}

fn os_icon(icon: &str, description: &str) -> Element<'static, Message> {
    let logo: Option<&[u8]> = match icon {
        "windows" => Some(include_bytes!("../../../static/os-icons/windows.svg")),
        "ubuntu" => Some(include_bytes!("../../../static/os-icons/ubuntu.svg")),
        "neon" => Some(include_bytes!("../../../static/os-icons/neon.svg")),
        "popos" => Some(include_bytes!("../../../static/os-icons/popos.svg")),
        "debian" => Some(include_bytes!("../../../static/os-icons/debian.svg")),
        "fedora" => Some(include_bytes!("../../../static/os-icons/fedora.svg")),
        "suse" => Some(include_bytes!("../../../static/os-icons/suse.svg")),
        "cachyos" => Some(include_bytes!("../../../static/os-icons/cachyos.svg")),
        "manjaro" => Some(include_bytes!("../../../static/os-icons/manjaro.svg")),
        "arch" => Some(include_bytes!("../../../static/os-icons/arch.svg")),
        "nixos" => Some(include_bytes!("../../../static/os-icons/nixos.svg")),
        "deepin" => Some(include_bytes!("../../../static/os-icons/deepin.svg")),
        "macos" => Some(include_bytes!("../../../static/os-icons/macos.svg")),
        "linux" => Some(include_bytes!("../../../static/os-icons/linux.svg")),
        _ => None,
    };
    let (bytes, tone) = if let Some(bytes) = logo {
        (bytes, None)
    } else {
        let bytes: &[u8] = match efi_icon(description) {
            "network" => include_bytes!("../../../static/os-icons/network.svg"),
            "optical" => include_bytes!("../../../static/os-icons/optical.svg"),
            "usb" => include_bytes!("../../../static/os-icons/usb.svg"),
            _ => include_bytes!("../../../static/os-icons/drive.svg"),
        };
        (bytes, Some(style::IconTone::Muted))
    };
    iced::widget::svg(iced::widget::svg::Handle::from_memory(bytes))
        .width(28)
        .height(28)
        .style(move |theme, _| {
            tone.map_or_else(iced::widget::svg::Style::default, |tone| {
                style::icon(theme, tone)
            })
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn app() -> App {
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
    fn contextual_undo_restores_previous_selection_only_after_success() {
        let mut app = app();
        app.model.boot_next = Some(1);
        let _ = app.update(Message::BootNext(Some(0)));
        let _ = app.update(Message::Completed(Ok(())));
        let cursor = app.model.cursor;
        let _ = app.update(Message::UndoNext(BootTarget::Entry(0)));
        assert!(matches!(
            app.model.pending,
            Some(Operation::Undo(Change::BootNext {
                before: Some(1),
                after: Some(0)
            }))
        ));
        let _ = app.update(Message::Completed(Err("Mock EFI denial".into())));
        assert_eq!(app.model.boot_next, Some(0));
        assert_eq!(app.model.cursor, cursor);
        let _ = app.update(Message::UndoNext(BootTarget::Entry(0)));
        let _ = app.update(Message::Completed(Ok(())));
        assert_eq!(app.model.boot_next, Some(1));
        assert_eq!(app.model.cursor, cursor - 1);
    }

    #[test]
    fn contextual_undo_preserves_later_order_edits_and_separate_setup_flag() {
        let mut app = app();
        let _ = app.update(Message::BootNext(Some(0)));
        let _ = app.update(Message::Completed(Ok(())));
        let _ = app.update(Message::Firmware(true));
        let _ = app.update(Message::Completed(Ok(())));
        let _ = app.update(Message::Move(0, 1));
        let _ = app.update(Message::UndoNext(BootTarget::Entry(0)));
        let _ = app.update(Message::Completed(Ok(())));
        assert_eq!(app.model.boot_next, None);
        assert!(app.model.firmware_setup);
        assert_eq!(app.model.order, vec![1, 0]);
        let _ = app.update(Message::UndoNext(BootTarget::FirmwareSetup));
        let _ = app.update(Message::Completed(Ok(())));
        assert!(!app.model.firmware_setup);
        assert_eq!(app.model.order, vec![1, 0]);
    }

    #[test]
    fn loaded_selection_can_be_canceled_and_pending_selection_cannot_restart() {
        let mut app = app();
        let setup = BootTarget::FirmwareSetup;
        let _ = app.update(Message::Firmware(true));
        let _ = app.update(Message::RequestRestart(setup));
        assert!(app.restart_confirmation.is_none());
        let _ = app.update(Message::Completed(Err("Mock EFI denial".into())));
        let _ = app.update(Message::RequestRestart(setup));
        assert!(app.restart_confirmation.is_none());
        app.model.firmware_setup = true;
        let _ = app.update(Message::RequestRestart(setup));
        assert_eq!(app.restart_confirmation, Some(setup));
        assert!(app.dialog.is_none());
        let _ = app.update(Message::CancelDialog);
        let _ = app.update(Message::UndoNext(setup));
        assert!(matches!(
            app.model.pending,
            Some(Operation::Change(Change::Firmware {
                before: true,
                after: false
            }))
        ));
        let _ = app.update(Message::Completed(Ok(())));
        assert!(!app.model.firmware_setup);
    }

    #[test]
    fn restart_requires_confirmation_and_cancel_does_not_schedule_backend() {
        let mut app = app();
        let target = BootTarget::Entry(0);
        let _ = app.update(Message::RequestRestart(target));
        assert!(app.restart_confirmation.is_none());
        assert!(app.model.pending.is_none());
        let _ = app.update(Message::BootNext(Some(0)));
        let _ = app.update(Message::Completed(Ok(())));
        let _ = app.update(Message::RequestRestart(target));
        assert_eq!(app.restart_confirmation, Some(target));
        assert!(app.dialog.is_none());
        assert!(app.model.pending.is_none());
        let _ = app.update(Message::CancelDialog);
        assert!(app.restart_confirmation.is_none());
        assert!(app.model.pending.is_none());
        let _ = app.update(Message::RequestRestart(target));
        assert!(app.model.pending.is_none());
        let _ = app.update(Message::RequestRestart(target));
        assert!(matches!(app.model.pending, Some(Operation::Restart)));
        assert!(app.restart_confirmation.is_none());
        let _ = app.update(Message::RequestRestart(target));
        assert!(app.restart_confirmation.is_none());
    }

    #[test]
    fn inline_restart_confirmation_is_bound_to_target_and_canceled_by_other_actions() {
        let mut app = app();
        let entry = BootTarget::Entry(0);
        let setup = BootTarget::FirmwareSetup;
        app.model.boot_next = Some(0);
        app.model.firmware_setup = true;
        let _ = app.update(Message::RequestRestart(entry));
        let _ = app.update(Message::RequestRestart(setup));
        assert_eq!(app.restart_confirmation, Some(setup));
        assert!(app.model.pending.is_none());
        let _ = app.update(Message::Move(0, 1));
        assert!(app.restart_confirmation.is_none());
        let _ = app.update(Message::RequestRestart(setup));
        assert!(app.model.pending.is_none());
        let _ = app.update(Message::UndoNext(setup));
        assert!(app.restart_confirmation.is_none());
        let _ = app.update(Message::RequestRestart(setup));
        assert!(matches!(
            app.model.pending,
            Some(Operation::Change(Change::Firmware { .. }))
        ));
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
        assert_eq!(app.display_order(), vec![1, 0]);
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
        assert_eq!(app.display_order(), vec![0, 1]);
        assert!(app.model.history.is_empty());
    }

    #[test]
    fn preview_across_sections_changes_only_the_display_until_drop() {
        let mut app = app();
        let _ = app.update(Message::DragStart(0));
        let _ = app.update(Message::DragHover(DropTarget::Others));
        assert_eq!(app.display_order(), vec![1]);
        assert_eq!(app.model.order, vec![0, 1]);
        assert!(app.model.history.is_empty());
        let _ = app.update(Message::DragDrop(DropTarget::Others));
        assert_eq!(app.model.order, vec![1]);
        assert_eq!(app.model.history.len(), 1);
        assert!(app.model.pending.is_none());

        let _ = app.update(Message::DragStart(0));
        let _ = app.update(Message::DragHover(DropTarget::Before(1)));
        assert_eq!(app.display_order(), vec![0, 1]);
        assert_eq!(app.model.order, vec![1]);
        let _ = app.update(Message::DragCancel);
        assert_eq!(app.display_order(), vec![1]);
        assert_eq!(app.model.history.len(), 1);
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
