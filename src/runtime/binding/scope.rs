use std::rc::Rc;

use parking_lot::Mutex;

use crate::{
    binding_metadata::ScopeId,
    error::{Error, Result},
    runtime::{
        VmStorageKind,
        control::reference_error_uninitialized,
        storage_ledger::{VmStorageLedger, VmStorageReservation},
    },
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

impl ScopeIndexData {
    pub(in crate::runtime) fn storage_entry_count(&self) -> Result<usize> {
        self.slot_atoms
            .len()
            .checked_add(self.bindings.len())
            .ok_or_else(|| Error::limit("shared scope index entry count overflowed"))
    }
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

    fn storage_entry_count(&self) -> Result<usize> {
        self.slot_atoms()
            .len()
            .checked_add(self.bindings().len())
            .ok_or_else(|| Error::limit("binding scope index entry count overflowed"))
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

#[derive(Debug, Default)]
pub struct BindingScope {
    slots: Vec<BindingCell>,
    index: ScopeIndex,
    compiled_scope: Option<ScopeId>,
    storage_ledger: Option<VmStorageLedger>,
    resource_stacks: Vec<BindingResourceStack>,
}

#[derive(Debug)]
pub(in crate::runtime) enum BindingResourceStack {
    Sync(Value),
    Async(Value),
}

impl BindingResourceStack {
    pub(crate) const fn value(&self) -> &Value {
        match self {
            Self::Sync(value) | Self::Async(value) => value,
        }
    }
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
            storage_ledger: None,
            resource_stacks: Vec::new(),
        }
    }

    pub(in crate::runtime) const fn new_active(storage_ledger: VmStorageLedger) -> Self {
        Self {
            slots: Vec::new(),
            index: ScopeIndex::new(),
            compiled_scope: None,
            storage_ledger: Some(storage_ledger),
            resource_stacks: Vec::new(),
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
            storage_ledger: None,
            resource_stacks: Vec::new(),
        }
    }

    pub const fn len(&self) -> usize {
        self.slots.len()
    }

    pub(crate) fn index_entry_count(&self) -> Result<usize> {
        self.index.storage_entry_count()
    }

    pub(crate) fn cells(&self) -> impl Iterator<Item = &BindingCell> {
        self.slots.iter()
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
            storage_ledger: None,
            resource_stacks: Vec::new(),
        })
    }

    pub(in crate::runtime) fn resource_stacks(
        &self,
    ) -> impl Iterator<Item = &BindingResourceStack> {
        self.resource_stacks.iter()
    }

    pub(in crate::runtime) fn push_resource_stack(&mut self, stack: BindingResourceStack) {
        self.resource_stacks.push(stack);
    }

    pub(in crate::runtime) fn take_resource_stacks(&mut self) -> Vec<BindingResourceStack> {
        std::mem::take(&mut self.resource_stacks)
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

    pub(crate) fn insert(&mut self, atom: AtomId, binding: BindingCell) -> Result<BindingSlot> {
        self.insert_or_replace(atom, binding)
    }

    pub(crate) fn insert_or_replace(
        &mut self,
        atom: AtomId,
        binding: BindingCell,
    ) -> Result<BindingSlot> {
        match self.binding_position(atom) {
            Ok(position) => {
                let Some(entry) = self.index.bindings().get(position) else {
                    return Err(Error::runtime("binding index entry disappeared"));
                };
                let slot = entry.slot();
                if let Some(existing) = self.cell_mut(slot) {
                    *existing = binding;
                }
                Ok(slot)
            }
            Err(position) => {
                let reservations = self.reserve_new_binding()?;
                let slot = BindingSlot::from_index(self.slots.len());
                Self::commit_new_binding(reservations)?;
                self.slots.push(binding);
                let (slot_atoms, bindings) = self.index.make_owned();
                slot_atoms.push(atom);
                bindings.insert(position, BindingEntry::new(atom, slot));
                Ok(slot)
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
        self.insert_or_replace(atom, binding)
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
        let reservations = self.reserve_new_binding()?;
        Self::commit_new_binding(reservations)?;
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
            Err(position) => self.try_insert_new_at_slot(position, atom, binding, slot),
        }
    }

    fn try_insert_new_at_slot(
        &mut self,
        position: usize,
        atom: AtomId,
        binding: BindingCell,
        slot: BindingSlot,
    ) -> Result<Option<BindingSlot>> {
        if slot.index() != self.slots.len() {
            return Ok(None);
        }
        let reservations = self.reserve_new_binding()?;
        Self::commit_new_binding(reservations)?;
        self.slots.push(binding);
        let (slot_atoms, bindings) = self.index.make_owned();
        slot_atoms.push(atom);
        bindings.insert(position, BindingEntry::new(atom, slot));
        Ok(Some(slot))
    }

    pub(in crate::runtime) fn activate_storage(
        &mut self,
        storage_ledger: VmStorageLedger,
    ) -> Result<()> {
        if self.storage_ledger.is_some() {
            return Err(Error::runtime("binding scope storage is already active"));
        }
        storage_ledger.grow_count(VmStorageKind::Binding, self.len())?;
        let cache_entries = self.index_entry_count()?;
        if let Err(error) = storage_ledger.grow_count(VmStorageKind::CacheEntry, cache_entries) {
            storage_ledger.release_count(VmStorageKind::Binding, self.len())?;
            return Err(error);
        }
        self.storage_ledger = Some(storage_ledger);
        Ok(())
    }

    pub(in crate::runtime) fn deactivate_storage(&mut self) -> Result<()> {
        let Some(storage_ledger) = self.storage_ledger.take() else {
            return Ok(());
        };
        storage_ledger.release_count(VmStorageKind::Binding, self.len())?;
        storage_ledger.release_count(VmStorageKind::CacheEntry, self.index_entry_count()?)
    }

    fn reserve_new_binding(
        &self,
    ) -> Result<(Option<VmStorageReservation>, Option<VmStorageReservation>)> {
        let Some(storage_ledger) = &self.storage_ledger else {
            return Ok((None, None));
        };
        let binding = storage_ledger.reserve_count(VmStorageKind::Binding, 1)?;
        let cache = storage_ledger.reserve_count(VmStorageKind::CacheEntry, 2)?;
        Ok((Some(binding), Some(cache)))
    }

    fn commit_new_binding(
        reservations: (Option<VmStorageReservation>, Option<VmStorageReservation>),
    ) -> Result<()> {
        if let Some(binding) = reservations.0 {
            binding.commit()?;
        }
        if let Some(cache) = reservations.1 {
            cache.commit()?;
        }
        Ok(())
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
            immutable_assignment: ImmutableAssignment::AlwaysThrow,
            kind,
        })))
    }

    pub(in crate::runtime) fn named_function(value: Value) -> Self {
        Self(Rc::new(Mutex::new(Binding {
            state: BindingState::Initialized(value),
            mutable: false,
            immutable_assignment: ImmutableAssignment::ThrowIfStrict,
            kind: DeclKind::Const,
        })))
    }

    pub fn uninitialized(mutable: bool, kind: DeclKind) -> Self {
        Self(Rc::new(Mutex::new(Binding {
            state: BindingState::Uninitialized,
            mutable,
            immutable_assignment: ImmutableAssignment::AlwaysThrow,
            kind,
        })))
    }

    pub fn value(&self, name: &str) -> Result<Value> {
        match &self.0.lock().state {
            BindingState::Initialized(value) => Ok(value.clone()),
            BindingState::Uninitialized => Err(reference_error_uninitialized(name)),
        }
    }

    pub(crate) fn with_initialized_value<R>(&self, visit: impl FnOnce(&Value) -> R) -> Option<R> {
        let binding = self.0.lock();
        match &binding.state {
            BindingState::Initialized(value) => Some(visit(value)),
            BindingState::Uninitialized => None,
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

    pub(in crate::runtime) fn assign_bytecode(
        &self,
        name: &str,
        value: Value,
        strict: bool,
    ) -> Result<()> {
        let mut binding = self.0.lock();
        if !binding.mutable {
            if binding.immutable_assignment == ImmutableAssignment::ThrowIfStrict && !strict {
                return Ok(());
            }
            return Err(Error::type_error(format!(
                "assignment to constant '{name}'"
            )));
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
    immutable_assignment: ImmutableAssignment,
    kind: DeclKind,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ImmutableAssignment {
    AlwaysThrow,
    ThrowIfStrict,
}
