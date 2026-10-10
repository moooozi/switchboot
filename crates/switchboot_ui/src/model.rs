use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: u16,
    pub description: String,
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub entries: Vec<Entry>,
    pub order: Vec<u16>,
    pub boot_next: Option<u16>,
    pub current: Option<u16>,
    pub firmware_setup: bool,
    pub portable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootTarget {
    Entry(u16),
    FirmwareSetup,
}

#[derive(Debug, Clone)]
pub struct Shortcut {
    pub name: String,
    pub target: BootTarget,
    pub reboot: bool,
    pub icon_id: Option<String>,
}

/// Privileged services are implemented by the host, independently of iced.
/// Calls may block: the UI dispatches them to a blocking worker thread.
pub trait Backend: Send + Sync + 'static {
    fn load(&self) -> Result<Snapshot, String>;
    fn discover(&self) -> Result<Vec<Entry>, String>;
    fn set_order(&self, order: Vec<u16>) -> Result<(), String>;
    fn set_next(&self, next: Option<u16>) -> Result<(), String>;
    fn set_firmware(&self, enabled: bool) -> Result<(), String>;
    fn create_shortcut(&self, shortcut: Shortcut) -> Result<(), String>;
    fn restart(&self) -> Result<(), String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Change {
    Order {
        before: Vec<u16>,
        after: Vec<u16>,
    },
    BootNext {
        before: Option<u16>,
        after: Option<u16>,
    },
    Firmware {
        before: bool,
        after: bool,
    },
}

impl Change {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Order { .. } => "boot order",
            Self::BootNext { .. } => "BootNext",
            Self::Firmware { .. } => "firmware setup",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Operation {
    Save(Vec<u16>),
    Change(Change),
    Undo(Change),
    Redo(Change),
    Shortcut(Shortcut),
    Restart,
}

impl Operation {
    pub(crate) fn execute(&self, backend: &dyn Backend) -> Result<(), String> {
        match self {
            Self::Save(order) => backend.set_order(order.clone()),
            Self::Change(change) | Self::Redo(change) => apply_remote(change, false, backend),
            Self::Undo(change) => apply_remote(change, true, backend),
            Self::Shortcut(shortcut) => backend.create_shortcut(shortcut.clone()),
            Self::Restart => backend.restart(),
        }
    }
}

fn apply_remote(change: &Change, undo: bool, backend: &dyn Backend) -> Result<(), String> {
    match change {
        Change::Order { .. } => Ok(()),
        Change::BootNext { before, after } => backend.set_next(if undo { *before } else { *after }),
        Change::Firmware { before, after } => {
            backend.set_firmware(if undo { *before } else { *after })
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Model {
    pub entries: HashMap<u16, Entry>,
    pub order: Vec<u16>,
    pub baseline: Vec<u16>,
    pub boot_next: Option<u16>,
    pub current: Option<u16>,
    pub firmware_setup: bool,
    pub portable: bool,
    pub initialized: bool,
    pub loading: bool,
    pub discovering: bool,
    pub pending: Option<Operation>,
    pub error: Option<String>,
    pub history: Vec<Change>,
    pub cursor: usize,
}

impl Model {
    pub fn busy(&self) -> bool {
        self.loading || self.discovering || self.pending.is_some() || !self.initialized
    }

    pub fn changed(&self) -> bool {
        self.order != self.baseline
    }

    pub fn initialize(&mut self, snapshot: Snapshot) -> Result<(), String> {
        let catalog: HashMap<_, _> = snapshot
            .entries
            .into_iter()
            .map(|entry| (entry.id, entry))
            .collect();
        if snapshot.order.iter().any(|id| !catalog.contains_key(id)) {
            return Err("The firmware boot order contains an unavailable entry".into());
        }
        if snapshot.order.iter().copied().collect::<HashSet<_>>().len() != snapshot.order.len() {
            return Err("The firmware boot order contains duplicate entries".into());
        }
        self.entries = catalog;
        self.order = snapshot.order.clone();
        self.baseline = snapshot.order;
        self.boot_next = snapshot.boot_next;
        self.current = snapshot.current;
        self.firmware_setup = snapshot.firmware_setup;
        self.portable = snapshot.portable;
        self.initialized = true;
        self.history.clear();
        self.cursor = 0;
        self.error = None;
        Ok(())
    }

    pub fn record_order(&mut self, after: Vec<u16>) {
        if self.busy()
            || after == self.order
            || after.iter().any(|id| !self.entries.contains_key(id))
        {
            return;
        }
        if after.iter().copied().collect::<HashSet<_>>().len() != after.len() {
            return;
        }
        let before = self.order.clone();
        self.order = after.clone();
        self.record(Change::Order { before, after });
    }

    pub fn move_entry(&mut self, id: u16, displacement: isize) {
        let Some(index) = self.order.iter().position(|value| *value == id) else {
            return;
        };
        let Some(destination) = index.checked_add_signed(displacement) else {
            return;
        };
        if destination >= self.order.len() {
            return;
        }
        let mut after = self.order.clone();
        after.swap(index, destination);
        self.record_order(after);
    }

    pub fn make_default(&mut self, id: u16) {
        if !self.order.contains(&id) {
            return;
        }
        let mut after = self.order.clone();
        after.retain(|value| *value != id);
        after.insert(0, id);
        self.record_order(after);
    }

    pub fn include(&mut self, id: u16) {
        if self.order.contains(&id) {
            return;
        }
        let mut after = self.order.clone();
        after.push(id);
        self.record_order(after);
    }

    pub fn exclude(&mut self, id: u16) {
        if self.order.len() <= 1 {
            return;
        }
        self.record_order(
            self.order
                .iter()
                .copied()
                .filter(|value| *value != id)
                .collect(),
        );
    }

    pub fn begin(&mut self, operation: Operation) -> bool {
        if self.busy() {
            return false;
        }
        self.pending = Some(operation);
        self.error = None;
        true
    }

    pub fn complete(&mut self, result: Result<(), String>) {
        let Some(operation) = self.pending.take() else {
            return;
        };
        if let Err(error) = result {
            self.error = Some(error);
            return;
        }
        match operation {
            Operation::Save(order) => self.baseline = order,
            Operation::Change(change) => {
                self.apply(&change, false);
                self.record(change);
            }
            Operation::Undo(change) => {
                self.apply(&change, true);
                self.cursor -= 1;
            }
            Operation::Redo(change) => {
                self.apply(&change, false);
                self.cursor += 1;
            }
            Operation::Shortcut(_) | Operation::Restart => {}
        }
    }

    fn apply(&mut self, change: &Change, undo: bool) {
        match change {
            Change::Order { before, after } => {
                self.order = if undo { before } else { after }.clone()
            }
            Change::BootNext { before, after } => {
                self.boot_next = if undo { *before } else { *after }
            }
            Change::Firmware { before, after } => {
                self.firmware_setup = if undo { *before } else { *after }
            }
        }
    }

    fn record(&mut self, change: Change) {
        self.history.truncate(self.cursor);
        self.history.push(change);
        if self.history.len() > 50 {
            self.history.remove(0);
        }
        self.cursor = self.history.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Model {
        let mut model = Model::default();
        model
            .initialize(Snapshot {
                entries: (0..3)
                    .map(|id| Entry {
                        id,
                        description: format!("Entry {id}"),
                    })
                    .collect(),
                order: vec![0, 1],
                boot_next: Some(0),
                ..Snapshot::default()
            })
            .unwrap();
        model
    }

    #[test]
    fn failed_firmware_change_and_undo_keep_state_and_history() {
        let mut model = model();
        let change = Change::BootNext {
            before: Some(0),
            after: Some(1),
        };
        assert!(model.begin(Operation::Change(change.clone())));
        model.complete(Err("Denied".into()));
        assert_eq!(model.boot_next, Some(0));
        assert_eq!(model.cursor, 0);
        assert!(model.begin(Operation::Change(change.clone())));
        model.complete(Ok(()));
        assert_eq!(model.cursor, 1);
        assert!(model.begin(Operation::Undo(change.clone())));
        model.complete(Err("Denied".into()));
        assert_eq!(model.boot_next, Some(1));
        assert_eq!(model.cursor, 1);
        assert!(model.begin(Operation::Undo(change.clone())));
        model.complete(Ok(()));
        assert_eq!(model.boot_next, Some(0));
        assert_eq!(model.cursor, 0);
        assert!(model.begin(Operation::Redo(change)));
        model.complete(Err("Denied".into()));
        assert_eq!(model.cursor, 0);
    }

    #[test]
    fn pending_operation_excludes_edits_and_other_operations() {
        let mut model = model();
        assert!(model.begin(Operation::Save(model.order.clone())));
        model.move_entry(0, 1);
        model.include(2);
        model.exclude(0);
        assert!(!model.begin(Operation::Restart));
        assert_eq!(model.order, vec![0, 1]);
        assert_eq!(model.cursor, 0);
    }

    #[test]
    fn discovery_blocks_firmware_mutations_until_catalog_is_complete() {
        let mut model = model();
        model.discovering = true;
        assert!(!model.begin(Operation::Restart));
        model.include(2);
        assert_eq!(model.order, vec![0, 1]);
        model.discovering = false;
        assert!(model.begin(Operation::Restart));
    }

    #[test]
    fn empty_original_order_can_be_restored_by_discard() {
        let mut model = Model::default();
        model
            .initialize(Snapshot {
                entries: vec![Entry {
                    id: 0,
                    description: "Boot0000".into(),
                }],
                ..Snapshot::default()
            })
            .unwrap();
        model.include(0);
        assert!(model.changed());
        model.record_order(model.baseline.clone());
        assert!(model.order.is_empty());
        assert!(!model.changed());
    }

    #[test]
    fn save_commits_submitted_snapshot_and_errors_preserve_baseline() {
        let mut model = model();
        model.move_entry(0, 1);
        assert!(model.begin(Operation::Save(model.order.clone())));
        model.complete(Err("Denied".into()));
        assert_eq!(model.baseline, vec![0, 1]);
        assert!(model.begin(Operation::Save(model.order.clone())));
        // A completion must carry the snapshot even if external state changes.
        model.order = vec![0, 1];
        model.complete(Ok(()));
        assert_eq!(model.baseline, vec![1, 0]);
    }

    #[test]
    fn removed_entries_and_boot_zero_survive_undo_redo() {
        let mut model = model();
        model.include(2);
        model.exclude(0);
        assert_eq!(model.order, vec![1, 2]);
        let change = model.history[model.cursor - 1].clone();
        assert!(model.begin(Operation::Undo(change.clone())));
        model.complete(Ok(()));
        assert_eq!(model.order, vec![0, 1, 2]);
        assert!(model.entries.contains_key(&0));
        assert!(model.begin(Operation::Redo(change)));
        model.complete(Ok(()));
        assert_eq!(model.order, vec![1, 2]);
    }

    #[test]
    fn new_edit_truncates_redo_and_history_is_bounded() {
        let mut model = model();
        for _ in 0..60 {
            model.move_entry(0, if model.order[0] == 0 { 1 } else { -1 });
        }
        assert_eq!(model.history.len(), 50);
        let change = model.history[model.cursor - 1].clone();
        model.begin(Operation::Undo(change));
        model.complete(Ok(()));
        model.include(2);
        assert_eq!(model.cursor, model.history.len());
        assert!(
            matches!(model.history.last(), Some(Change::Order { after, .. }) if after.contains(&2))
        );
    }
}
