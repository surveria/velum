use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

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
    fn make_owned(&mut self) -> Result<(&mut Vec<AtomId>, &mut Vec<BindingEntry>)> {
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
            } => Ok((slot_atoms, bindings)),
            Self::Shared(_) => Err(Error::runtime(
                "scope index remained shared after copy-on-write",
            )),
        }
    }
}

#[derive(Debug, Default)]
pub struct BindingScope {
    slots: Vec<BindingCell>,
    index: ScopeIndex,
    compiled_scope: Option<ScopeId>,
    storage_ledger: Option<VmStorageLedger>,
    resource_stacks: Vec<BindingResourceStack>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::runtime) struct BindingScopeStorageFootprint {
    binding_count: usize,
    cache_entry_count: usize,
}

impl BindingScopeStorageFootprint {
    pub(in crate::runtime) const fn binding_count(self) -> usize {
        self.binding_count
    }

    pub(in crate::runtime) const fn cache_entry_count(self) -> usize {
        self.cache_entry_count
    }

    pub(in crate::runtime) fn checked_add(self, other: Self) -> Result<Self> {
        let binding_count = self
            .binding_count
            .checked_add(other.binding_count)
            .ok_or_else(binding_scope_storage_overflow)?;
        let cache_entry_count = self
            .cache_entry_count
            .checked_add(other.cache_entry_count)
            .ok_or_else(binding_scope_storage_overflow)?;
        Ok(Self {
            binding_count,
            cache_entry_count,
        })
    }
}

fn binding_scope_storage_overflow() -> Error {
    Error::limit("binding scope storage footprint overflowed")
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

    pub(in crate::runtime) fn storage_footprint(&self) -> Result<BindingScopeStorageFootprint> {
        Ok(BindingScopeStorageFootprint {
            binding_count: self.len(),
            cache_entry_count: self.index_entry_count()?,
        })
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
                let slot = BindingSlot::from_index(self.slots.len());
                self.insert_new_binding(position, atom, binding, slot)?;
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
        self.insert_new_binding(position, atom, binding, slot)?;
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
        self.insert_new_binding(position, atom, binding, slot)?;
        Ok(Some(slot))
    }

    fn insert_new_binding(
        &mut self,
        position: usize,
        atom: AtomId,
        binding: BindingCell,
        slot: BindingSlot,
    ) -> Result<()> {
        let Self {
            slots,
            index,
            storage_ledger,
            ..
        } = self;
        let (slot_atoms, bindings) = index.make_owned()?;
        if slot.index() != slots.len() {
            return Err(Error::runtime("binding frame slot changed before commit"));
        }
        if position > bindings.len() {
            return Err(Error::runtime(
                "binding index position changed before commit",
            ));
        }
        let reservations = Self::reserve_new_binding(storage_ledger.as_ref())?;
        Self::commit_new_binding(reservations)?;
        slots.push(binding);
        slot_atoms.push(atom);
        bindings.insert(position, BindingEntry::new(atom, slot));
        Ok(())
    }

    pub(in crate::runtime) fn activate_storage(
        &mut self,
        storage_ledger: VmStorageLedger,
    ) -> Result<()> {
        if self.storage_ledger.is_some() {
            return Err(Error::runtime("binding scope storage is already active"));
        }
        let footprint = self.storage_footprint()?;
        storage_ledger.grow_count(VmStorageKind::Binding, footprint.binding_count())?;
        if let Err(error) =
            storage_ledger.grow_count(VmStorageKind::CacheEntry, footprint.cache_entry_count())
        {
            storage_ledger.release_count(VmStorageKind::Binding, footprint.binding_count())?;
            return Err(error);
        }
        self.storage_ledger = Some(storage_ledger);
        Ok(())
    }

    pub(in crate::runtime) fn deactivate_storage(&mut self) -> Result<()> {
        let Some(storage_ledger) = self.storage_ledger.take() else {
            return Ok(());
        };
        let footprint = self.storage_footprint()?;
        storage_ledger.release_count(VmStorageKind::Binding, footprint.binding_count())?;
        storage_ledger.release_count(VmStorageKind::CacheEntry, footprint.cache_entry_count())
    }

    fn reserve_new_binding(
        storage_ledger: Option<&VmStorageLedger>,
    ) -> Result<(Option<VmStorageReservation>, Option<VmStorageReservation>)> {
        let Some(storage_ledger) = storage_ledger else {
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
pub struct BindingCell(Rc<BindingCellInner>);

impl BindingCell {
    pub fn new(value: Value, mutable: bool, kind: DeclKind) -> Self {
        Self::from_binding(
            BindingState::Initialized(value),
            mutable,
            ImmutableAssignment::AlwaysThrow,
            kind,
        )
    }

    pub(in crate::runtime) fn named_function(value: Value) -> Self {
        Self::from_binding(
            BindingState::Initialized(value),
            false,
            ImmutableAssignment::ThrowIfStrict,
            DeclKind::Const,
        )
    }

    pub(in crate::runtime) fn immutable_global(value: Value) -> Self {
        Self::from_binding(
            BindingState::Initialized(value),
            false,
            ImmutableAssignment::ThrowIfStrict,
            DeclKind::Const,
        )
    }

    pub fn uninitialized(mutable: bool, kind: DeclKind) -> Self {
        Self::from_binding(
            BindingState::Uninitialized,
            mutable,
            ImmutableAssignment::AlwaysThrow,
            kind,
        )
    }

    fn from_binding(
        state: BindingState,
        mutable: bool,
        immutable_assignment: ImmutableAssignment,
        kind: DeclKind,
    ) -> Self {
        Self(Rc::new(BindingCellInner {
            binding: RefCell::new(Binding {
                state,
                mutable,
                immutable_assignment,
                is_terminal_alias_target: false,
            }),
            kind,
        }))
    }

    pub fn value(&self, name: &str) -> Result<Value> {
        let target = {
            let binding = self.borrow()?;
            match &binding.state {
                BindingState::Initialized(value) => return Ok(value.clone()),
                BindingState::Uninitialized => return Err(reference_error_uninitialized(name)),
                BindingState::Alias(target) => target.clone(),
            }
        };
        let target_binding = target.borrow()?;
        match &target_binding.state {
            BindingState::Initialized(value) => Ok(value.clone()),
            BindingState::Uninitialized => Err(reference_error_uninitialized(name)),
            BindingState::Alias(_) => Err(Error::runtime(
                "import binding alias target is not terminal",
            )),
        }
    }

    pub(crate) fn with_initialized_value<R>(&self, visit: impl FnOnce(&Value) -> R) -> Option<R> {
        let value = self.value("<binding>").ok()?;
        Some(visit(&value))
    }

    pub fn kind(&self) -> DeclKind {
        self.0.kind
    }

    pub fn initialize(&self, value: Value) -> Result<()> {
        let mut binding = self.borrow_mut()?;
        if !matches!(binding.state, BindingState::Uninitialized) {
            return Err(Error::runtime(
                "function parameter binding is already initialized",
            ));
        }
        binding.state = BindingState::Initialized(value);
        drop(binding);
        Ok(())
    }

    pub fn assign(&self, name: &str, value: Value) -> Result<()> {
        let mut binding = self.borrow_mut()?;
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
        let mut binding = self.borrow_mut()?;
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

    pub(in crate::runtime) fn alias_to(&self, target: Self) -> Result<()> {
        if self.same_cell(&target) {
            return Err(Error::runtime("binding cannot alias itself"));
        }
        let mut binding = self.borrow_mut()?;
        if !matches!(binding.state, BindingState::Uninitialized) {
            return Err(Error::runtime("import binding is already linked"));
        }
        if binding.is_terminal_alias_target {
            return Err(Error::runtime(
                "terminal import binding cannot become an alias",
            ));
        }
        let mut target_binding = target.borrow_mut()?;
        if matches!(target_binding.state, BindingState::Alias(_)) {
            return Err(Error::runtime(
                "import binding alias target is not terminal",
            ));
        }
        target_binding.is_terminal_alias_target = true;
        drop(target_binding);
        binding.state = BindingState::Alias(target);
        binding.mutable = false;
        binding.immutable_assignment = ImmutableAssignment::AlwaysThrow;
        drop(binding);
        Ok(())
    }

    pub(crate) fn same_cell(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    fn borrow(&self) -> Result<Ref<'_, Binding>> {
        self.0
            .binding
            .try_borrow()
            .map_err(|_| Error::runtime("binding is already mutably borrowed"))
    }

    fn borrow_mut(&self) -> Result<RefMut<'_, Binding>> {
        self.0
            .binding
            .try_borrow_mut()
            .map_err(|_| Error::runtime("binding is already borrowed"))
    }
}

#[derive(Debug)]
struct BindingCellInner {
    binding: RefCell<Binding>,
    kind: DeclKind,
}

#[derive(Debug, Clone)]
enum BindingState {
    Uninitialized,
    Initialized(Value),
    Alias(BindingCell),
}

#[derive(Debug, Clone)]
struct Binding {
    state: BindingState,
    mutable: bool,
    immutable_assignment: ImmutableAssignment,
    is_terminal_alias_target: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ImmutableAssignment {
    AlwaysThrow,
    ThrowIfStrict,
}
