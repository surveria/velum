use std::rc::Rc;

use crate::value::Value;

use super::{FunctionUpvalues, function::FunctionSuperBinding};

/// One VM-owned synchronous execution activation.
///
/// Call frames own every value needed to make the current invocation
/// resumable. Temporary `this` frames preserve class-field evaluation
/// semantics, while an evaluation boundary hides caller lexical state from
/// generated Function-constructor source without removing its roots.
#[derive(Debug)]
pub(in crate::runtime) enum ActivationFrame {
    Call {
        local_base: usize,
        upvalues: FunctionUpvalues,
        this_value: Value,
        new_target: Value,
        super_binding: Option<Rc<FunctionSuperBinding>>,
    },
    TemporaryThis {
        this_value: Value,
    },
    EvalBoundary {
        local_base: usize,
    },
}

impl ActivationFrame {
    pub(in crate::runtime) const fn call(
        local_base: usize,
        upvalues: FunctionUpvalues,
        this_value: Value,
        new_target: Value,
        super_binding: Option<Rc<FunctionSuperBinding>>,
    ) -> Self {
        Self::Call {
            local_base,
            upvalues,
            this_value,
            new_target,
            super_binding,
        }
    }

    pub(in crate::runtime) const fn temporary_this(this_value: Value) -> Self {
        Self::TemporaryThis { this_value }
    }

    pub(in crate::runtime) const fn eval_boundary(local_base: usize) -> Self {
        Self::EvalBoundary { local_base }
    }

    pub(in crate::runtime) const fn local_base(&self) -> Option<usize> {
        match self {
            Self::Call { local_base, .. } | Self::EvalBoundary { local_base } => Some(*local_base),
            Self::TemporaryThis { .. } => None,
        }
    }

    pub(in crate::runtime) const fn upvalues(&self) -> Option<&FunctionUpvalues> {
        match self {
            Self::Call { upvalues, .. } => Some(upvalues),
            Self::TemporaryThis { .. } | Self::EvalBoundary { .. } => None,
        }
    }

    pub(in crate::runtime) const fn this_value(&self) -> Option<&Value> {
        match self {
            Self::Call { this_value, .. } | Self::TemporaryThis { this_value } => Some(this_value),
            Self::EvalBoundary { .. } => None,
        }
    }

    pub(in crate::runtime) const fn new_target(&self) -> Option<&Value> {
        match self {
            Self::Call { new_target, .. } => Some(new_target),
            Self::TemporaryThis { .. } | Self::EvalBoundary { .. } => None,
        }
    }

    pub(in crate::runtime) const fn super_binding(&self) -> Option<&Rc<FunctionSuperBinding>> {
        match self {
            Self::Call { super_binding, .. } => super_binding.as_ref(),
            Self::TemporaryThis { .. } | Self::EvalBoundary { .. } => None,
        }
    }

    pub(in crate::runtime) const fn is_call(&self) -> bool {
        matches!(self, Self::Call { .. })
    }

    pub(in crate::runtime) const fn is_temporary_this(&self) -> bool {
        matches!(self, Self::TemporaryThis { .. })
    }

    pub(in crate::runtime) const fn is_eval_boundary(&self) -> bool {
        matches!(self, Self::EvalBoundary { .. })
    }
}
