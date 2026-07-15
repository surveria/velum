use crate::storage::atom::AtomId;

use super::{BindingScope, ScopeIndex};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum EvalVarConflictPolicy {
    #[default]
    Reject,
    AllowSimpleCatchParameter,
}

impl BindingScope {
    pub(in crate::runtime) const fn new_simple_catch_parameter() -> Self {
        Self {
            slots: Vec::new(),
            index: ScopeIndex::new(),
            compiled_scope: None,
            eval_var_conflict: EvalVarConflictPolicy::AllowSimpleCatchParameter,
            storage_ledger: None,
            resource_stacks: Vec::new(),
        }
    }

    pub(crate) fn conflicts_with_eval_var(&self, atom: AtomId) -> bool {
        self.contains(atom)
            && self.eval_var_conflict != EvalVarConflictPolicy::AllowSimpleCatchParameter
    }

    pub(crate) fn shadows_redeclared_eval_var(&self, atom: AtomId) -> bool {
        self.contains(atom)
            && self.eval_var_conflict == EvalVarConflictPolicy::AllowSimpleCatchParameter
    }
}
