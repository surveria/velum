use crate::{
    atom::AtomId,
    binding_layout::{BindingOperand, ScopeId},
    runtime::scope::BindingSlot,
};

#[derive(Debug, Clone, Copy)]
pub(super) enum BindingLocation {
    Global {
        atom: AtomId,
        slot: BindingSlot,
        validation: BindingLocationValidation,
    },
    ExactGlobal {
        slot: BindingSlot,
    },
    BuiltinGlobal {
        atom: AtomId,
        slot: BindingSlot,
        validation: BindingLocationValidation,
    },
    Local {
        atom: AtomId,
        scope: LocalScopeIndex,
        slot: BindingSlot,
        validation: BindingLocationValidation,
    },
    ExactLocal {
        frame: LocalScopeIndex,
        compiled_scope: ScopeId,
        slot: BindingSlot,
    },
    Upvalue {
        slot: BindingSlot,
    },
}

impl BindingLocation {
    pub(super) const fn global(atom: AtomId, slot: BindingSlot) -> Self {
        Self::Global {
            atom,
            slot,
            validation: BindingLocationValidation::Guarded,
        }
    }

    pub(super) const fn exact_global(slot: BindingSlot) -> Self {
        Self::ExactGlobal { slot }
    }

    pub(super) const fn builtin_global(atom: AtomId, slot: BindingSlot) -> Self {
        Self::BuiltinGlobal {
            atom,
            slot,
            validation: BindingLocationValidation::Guarded,
        }
    }

    pub(super) const fn local(atom: AtomId, scope: LocalScopeIndex, slot: BindingSlot) -> Self {
        Self::Local {
            atom,
            scope,
            slot,
            validation: BindingLocationValidation::Guarded,
        }
    }

    pub(super) const fn upvalue(slot: BindingSlot) -> Self {
        Self::Upvalue { slot }
    }

    pub(super) const fn needs_shadow_guard(self) -> bool {
        matches!(
            self,
            Self::Global {
                validation: BindingLocationValidation::Guarded,
                ..
            } | Self::BuiltinGlobal {
                validation: BindingLocationValidation::Guarded,
                ..
            } | Self::Local {
                validation: BindingLocationValidation::Guarded,
                ..
            }
        )
    }

    pub(super) const fn for_compiled_operand(self, operand: BindingOperand) -> Self {
        match (operand, self) {
            (
                BindingOperand::Local {
                    scope: compiled_scope,
                    ..
                },
                Self::Local {
                    scope: frame, slot, ..
                },
            ) => Self::ExactLocal {
                frame,
                compiled_scope,
                slot,
            },
            (BindingOperand::Local { .. }, Self::ExactLocal { .. }) => self,
            (BindingOperand::Upvalue { .. }, Self::Upvalue { slot }) => Self::Upvalue { slot },
            (_, location) => location,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum BindingLocationValidation {
    Guarded,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LocalScopeIndex(usize);

impl LocalScopeIndex {
    pub(super) const fn new(index: usize) -> Self {
        Self(index)
    }

    pub(super) const fn index(self) -> usize {
        self.0
    }
}
