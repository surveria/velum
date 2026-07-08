use std::rc::Rc;

use parking_lot::Mutex;

use crate::{
    binding_metadata::ScopeId,
    error::{Error, Result},
    runtime::control::reference_error_uninitialized,
    storage::atom::AtomId,
    syntax::DeclKind,
    value::Value,
};

/// Immutable atom-to-slot index shared by every call frame of one
/// function, so per-call scope construction allocates only the value slots.
#[derive(Debug)]
pub struct ScopeIndexData {
    slot_atoms: Box<[AtomId]>,
    bindings: Box<[BindingEntry]>,
}

#[derive(Debug, Clone)]
enum ScopeIndex {
    Owned {
        slot_atoms: Vec<AtomId>,
        bindings: Vec<BindingEntry>,
    },
    Shared(Rc<ScopeIndexData>),
}

impl ScopeIndex {
    const fn new() -> Self {
        Self::Owned {
            slot_atoms: Vec::new(),
            bindings: Vec::new(),
        }
    }

    fn slot_atoms(&self) -> &[AtomId] {
        match self {
            Self::Owned { slot_atoms, .. } => slot_atoms,
            Self::Shared(data) => &data.slot_atoms,
        }
    }

    fn bindings(&self) -> &[BindingEntry] {
        match self {
            Self::Owned { bindings, .. } => bindings,
            Self::Shared(data) => &data.bindings,
        }
    }

    /// Escalates a shared index to an owned copy before mutation.
    fn make_owned(&mut self) -> (&mut Vec<AtomId>, &mut Vec<BindingEntry>) {
        if let Self::Shared(data) = self {
            *self = Self::Owned {
                slot_atoms: data.slot_atoms.to_vec(),
                bindings: data.bindings.to_vec(),
            };
        }
        match self {
            Self::Owned {
                slot_atoms,
                bindings,
            } => (slot_atoms, bindings),
            Self::Shared(_) => unreachable_owned(),
        }
    }
}

/// The shared arm is replaced before this is reachable; kept as a typed
/// stand-in so the match above stays exhaustive without panicking paths.
fn unreachable_owned() -> (&'static mut Vec<AtomId>, &'static mut Vec<BindingEntry>) {
    // This branch cannot execute: make_owned rewrites Shared to Owned first.
    // Leak two empty vectors to satisfy the signature without panicking.
    (
        Box::leak(Box::new(Vec::new())),
        Box::leak(Box::new(Vec::new())),
    )
}

#[derive(Debug, Clone, Default)]
pub struct BindingScope {
    slots: Vec<BindingCell>,
    index: ScopeIndex,
    compiled_scope: Option<ScopeId>,
}

impl Default for ScopeIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl BindingScope {
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            index: ScopeIndex::new(),
            compiled_scope: None,
        }
    }

    /// Builds a call scope from a shared per-function index template plus
    /// this call's value slots: a single allocation on the hot call path.
    pub(in crate::runtime) const fn from_shared_template(
        compiled_scope: ScopeId,
        template: Rc<ScopeIndexData>,
        slots: Vec<BindingCell>,
    ) -> Self {
        Self {
            slots,
            index: ScopeIndex::Shared(template),
            compiled_scope: Some(compiled_scope),
        }
    }

    pub const fn len(&self) -> usize {
        self.slots.len()
    }

    pub(crate) fn from_compiled_slots(
        compiled_scope: ScopeId,
        slots: Vec<(AtomId, BindingCell)>,
    ) -> Result<Self> {
        let mut cells = Vec::with_capacity(slots.len());
        let mut atoms = Vec::with_capacity(slots.len());
        for (atom, cell) in slots {
            cells.push(cell);
            atoms.push(atom);
        }
        let template = ScopeIndexData::from_slot_atoms(&atoms)?;
        Ok(Self {
            slots: cells,
            index: ScopeIndex::Owned {
                slot_atoms: atoms,
                bindings: template.bindings.into_vec(),
            },
            compiled_scope: Some(compiled_scope),
        })
    }

    pub(crate) fn contains(&self, atom: AtomId) -> bool {
        self.binding_position(atom).is_ok()
    }

    pub(crate) fn get(&self, atom: AtomId) -> Option<BindingCell> {
        let slot = self.slot_of(atom)?;
        self.cell(slot).cloned()
    }

    pub(crate) const fn compiled_scope(&self) -> Option<ScopeId> {
        self.compiled_scope
    }

    pub(crate) fn mark_compiled_scope(&mut self, scope: ScopeId) -> Result<()> {
        if let Some(existing) = self.compiled_scope {
            if existing != scope {
                return Err(Error::runtime("binding frame layout scope mismatch"));
            }
            return Ok(());
        }
        self.compiled_scope = Some(scope);
        Ok(())
    }

    pub(crate) fn slot_of(&self, atom: AtomId) -> Option<BindingSlot> {
        let position = self.binding_position(atom).ok()?;
        self.index
            .bindings()
            .get(position)
            .map(|entry| entry.slot())
    }

    pub(crate) fn cell_for_slot(&self, atom: AtomId, slot: BindingSlot) -> Option<BindingCell> {
        let slot_atom = self.index.slot_atoms().get(slot.index()).copied()?;
        if slot_atom != atom {
            return None;
        }
        self.cell(slot).cloned()
    }

    pub(crate) fn cell_at_slot(&self, slot: BindingSlot) -> Option<BindingCell> {
        self.cell(slot).cloned()
    }

    pub(crate) fn insert(&mut self, atom: AtomId, binding: BindingCell) -> BindingSlot {
        self.insert_or_replace(atom, binding)
    }

    pub(crate) fn insert_or_replace(&mut self, atom: AtomId, binding: BindingCell) -> BindingSlot {
        match self.binding_position(atom) {
            Ok(position) => {
                let Some(entry) = self.index.bindings().get(position) else {
                    return BindingSlot::from_index(self.slots.len());
                };
                let slot = entry.slot();
                if let Some(existing) = self.cell_mut(slot) {
                    *existing = binding;
                }
                slot
            }
            Err(position) => {
                let slot = BindingSlot::from_index(self.slots.len());
                self.slots.push(binding);
                let (slot_atoms, bindings) = self.index.make_owned();
                slot_atoms.push(atom);
                bindings.insert(position, BindingEntry::new(atom, slot));
                slot
            }
        }
    }

    pub(crate) fn insert_or_replace_at_slot(
        &mut self,
        atom: AtomId,
        binding: BindingCell,
        slot: BindingSlot,
    ) -> Result<BindingSlot> {
        match self.binding_position(atom) {
            Ok(position) => {
                let Some(entry) = self.index.bindings().get(position) else {
                    return Err(Error::runtime("binding frame index disappeared"));
                };
                if entry.slot() != slot {
                    return Err(Error::runtime("binding frame slot mismatch"));
                }
                let Some(existing) = self.cell_mut(slot) else {
                    return Err(Error::runtime("binding frame slot is not defined"));
                };
                *existing = binding;
                Ok(slot)
            }
            Err(position) => self.insert_new_at_slot(position, atom, binding, slot),
        }
    }

    pub(crate) fn insert_or_replace_at_optional_slot(
        &mut self,
        atom: AtomId,
        binding: BindingCell,
        slot: Option<BindingSlot>,
    ) -> Result<BindingSlot> {
        if let Some(slot) = slot
            && let Some(inserted) =
                self.try_insert_or_replace_at_slot(atom, binding.clone(), slot)?
        {
            return Ok(inserted);
        }
        Ok(self.insert_or_replace(atom, binding))
    }

    fn cell(&self, slot: BindingSlot) -> Option<&BindingCell> {
        self.slots.get(slot.index())
    }

    fn cell_mut(&mut self, slot: BindingSlot) -> Option<&mut BindingCell> {
        self.slots.get_mut(slot.index())
    }

    fn insert_new_at_slot(
        &mut self,
        position: usize,
        atom: AtomId,
        binding: BindingCell,
        slot: BindingSlot,
    ) -> Result<BindingSlot> {
        let slot_index = slot.index();
        if slot_index < self.slots.len() {
            return Err(Error::runtime("binding frame slot is already occupied"));
        }
        if slot_index > self.slots.len() {
            return Err(Error::runtime("binding frame slot gap is not supported"));
        }
        self.slots.push(binding);
        let (slot_atoms, bindings) = self.index.make_owned();
        slot_atoms.push(atom);
        bindings.insert(position, BindingEntry::new(atom, slot));
        Ok(slot)
    }

    fn try_insert_or_replace_at_slot(
        &mut self,
        atom: AtomId,
        binding: BindingCell,
        slot: BindingSlot,
    ) -> Result<Option<BindingSlot>> {
        match self.binding_position(atom) {
            Ok(position) => {
                let Some(entry) = self.index.bindings().get(position) else {
                    return Err(Error::runtime("binding frame index disappeared"));
                };
                if entry.slot() != slot {
                    return Ok(None);
                }
                let Some(existing) = self.cell_mut(slot) else {
                    return Err(Error::runtime("binding frame slot is not defined"));
                };
                *existing = binding;
                Ok(Some(slot))
            }
            Err(position) => Ok(self.try_insert_new_at_slot(position, atom, binding, slot)),
        }
    }

    fn try_insert_new_at_slot(
        &mut self,
        position: usize,
        atom: AtomId,
        binding: BindingCell,
        slot: BindingSlot,
    ) -> Option<BindingSlot> {
        if slot.index() != self.slots.len() {
            return None;
        }
        self.slots.push(binding);
        let (slot_atoms, bindings) = self.index.make_owned();
        slot_atoms.push(atom);
        bindings.insert(position, BindingEntry::new(atom, slot));
        Some(slot)
    }

    fn binding_position(&self, atom: AtomId) -> std::result::Result<usize, usize> {
        self.index
            .bindings()
            .binary_search_by(|entry| entry.atom().cmp(&atom))
    }
}

impl ScopeIndexData {
    /// Builds the sorted atom index for a fixed slot layout once, so call
    /// frames can share it immutably.
    pub(in crate::runtime) fn from_slot_atoms(slot_atoms: &[AtomId]) -> Result<Self> {
        let mut bindings = Vec::with_capacity(slot_atoms.len());
        for (index, atom) in slot_atoms.iter().copied().enumerate() {
            bindings.push(BindingEntry::new(atom, BindingSlot::from_index(index)));
        }
        bindings.sort_by_key(|entry| entry.atom());
        if sorted_bindings_have_duplicates(&bindings) {
            return Err(Error::runtime("compiled binding frame contains duplicates"));
        }
        Ok(Self {
            slot_atoms: slot_atoms.to_vec().into_boxed_slice(),
            bindings: bindings.into_boxed_slice(),
        })
    }
}

fn sorted_bindings_have_duplicates(bindings: &[BindingEntry]) -> bool {
    for pair in bindings.windows(2) {
        let Some(left) = pair.first() else {
            continue;
        };
        let Some(right) = pair.get(1) else {
            continue;
        };
        if left.atom() == right.atom() {
            return true;
        }
    }
    false
}

#[derive(Debug, Clone, Copy)]
struct BindingEntry {
    atom: AtomId,
    slot: BindingSlot,
}

impl BindingEntry {
    const fn new(atom: AtomId, slot: BindingSlot) -> Self {
        Self { atom, slot }
    }

    const fn atom(self) -> AtomId {
        self.atom
    }

    const fn slot(self) -> BindingSlot {
        self.slot
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct BindingSlot(usize);

impl BindingSlot {
    pub(crate) const fn from_index(index: usize) -> Self {
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct BindingCell(Rc<Mutex<Binding>>);

impl BindingCell {
    pub fn new(value: Value, mutable: bool, kind: DeclKind) -> Self {
        Self(Rc::new(Mutex::new(Binding {
            state: BindingState::Initialized(value),
            mutable,
            kind,
        })))
    }

    pub fn uninitialized(mutable: bool, kind: DeclKind) -> Self {
        Self(Rc::new(Mutex::new(Binding {
            state: BindingState::Uninitialized,
            mutable,
            kind,
        })))
    }

    pub fn value(&self, name: &str) -> Result<Value> {
        match &self.0.lock().state {
            BindingState::Initialized(value) => Ok(value.clone()),
            BindingState::Uninitialized => Err(reference_error_uninitialized(name)),
        }
    }

    pub fn kind(&self) -> DeclKind {
        self.0.lock().kind
    }

    pub fn initialize(&self, value: Value) -> Result<()> {
        let mut binding = self.0.lock();
        if matches!(binding.state, BindingState::Initialized(_)) {
            return Err(Error::runtime(
                "function parameter binding is already initialized",
            ));
        }
        binding.state = BindingState::Initialized(value);
        drop(binding);
        Ok(())
    }

    pub fn assign(&self, name: &str, value: Value) -> Result<()> {
        let mut binding = self.0.lock();
        if !binding.mutable {
            return Err(Error::runtime(format!("assignment to constant '{name}'")));
        }
        if matches!(binding.state, BindingState::Uninitialized) {
            return Err(reference_error_uninitialized(name));
        }
        binding.state = BindingState::Initialized(value);
        drop(binding);
        Ok(())
    }

    pub(crate) fn same_cell(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Debug, Clone)]
enum BindingState {
    Uninitialized,
    Initialized(Value),
}

#[derive(Debug, Clone)]
struct Binding {
    state: BindingState,
    mutable: bool,
    kind: DeclKind,
}
