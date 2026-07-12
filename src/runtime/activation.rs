use std::rc::Rc;

use crate::value::{FunctionId, Value};

use super::{
    FunctionActivationEnvironment, FunctionUpvalues, bytecode::BytecodeContinuationFrame,
    function::FunctionSuperBinding, private::PrivateEnvironment,
};

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
        with_environments: Vec<Value>,
        this_value: Value,
        new_target: Value,
        super_binding: Option<Rc<FunctionSuperBinding>>,
        private_environment: Option<Rc<PrivateEnvironment>>,
        continuation: Option<BytecodeContinuationFrame>,
    },
    TemporaryThis {
        this_value: Value,
        private_environment: Option<Rc<PrivateEnvironment>>,
        continuation: Option<BytecodeContinuationFrame>,
    },
    EvalBoundary {
        local_base: usize,
        with_environments: Vec<Value>,
        private_environment: Option<Rc<PrivateEnvironment>>,
        continuation: Option<BytecodeContinuationFrame>,
    },
    Bytecode {
        with_environments: Vec<Value>,
        private_environment: Option<Rc<PrivateEnvironment>>,
        continuation: Option<BytecodeContinuationFrame>,
    },
}

impl ActivationFrame {
    pub(in crate::runtime) fn call(
        function: FunctionId,
        local_base: usize,
        environment: FunctionActivationEnvironment,
        this_value: Value,
        new_target: Value,
        super_binding: Option<Rc<FunctionSuperBinding>>,
        private_environment: Option<Rc<PrivateEnvironment>>,
    ) -> Self {
        let (upvalues, with_environments) = environment;
        Self::Call {
            local_base,
            upvalues,
            with_environments,
            this_value,
            new_target,
            super_binding,
            private_environment,
            continuation: Some(BytecodeContinuationFrame::function(function)),
        }
    }

    pub(in crate::runtime) const fn temporary_this(
        this_value: Value,
        private_environment: Option<Rc<PrivateEnvironment>>,
    ) -> Self {
        Self::TemporaryThis {
            this_value,
            private_environment,
            continuation: None,
        }
    }

    pub(in crate::runtime) const fn eval_boundary(local_base: usize) -> Self {
        Self::EvalBoundary {
            local_base,
            with_environments: Vec::new(),
            private_environment: None,
            continuation: None,
        }
    }

    pub(in crate::runtime) const fn bytecode(
        continuation: BytecodeContinuationFrame,
        with_environments: Vec<Value>,
    ) -> Self {
        Self::Bytecode {
            with_environments,
            private_environment: None,
            continuation: Some(continuation),
        }
    }

    pub(in crate::runtime) const fn private_environment(&self) -> Option<&Rc<PrivateEnvironment>> {
        match self {
            Self::Call {
                private_environment,
                ..
            }
            | Self::TemporaryThis {
                private_environment,
                ..
            }
            | Self::EvalBoundary {
                private_environment,
                ..
            }
            | Self::Bytecode {
                private_environment,
                ..
            } => private_environment.as_ref(),
        }
    }

    pub(in crate::runtime) const fn private_environment_mut(
        &mut self,
    ) -> &mut Option<Rc<PrivateEnvironment>> {
        match self {
            Self::Call {
                private_environment,
                ..
            }
            | Self::TemporaryThis {
                private_environment,
                ..
            }
            | Self::EvalBoundary {
                private_environment,
                ..
            }
            | Self::Bytecode {
                private_environment,
                ..
            } => private_environment,
        }
    }

    pub(in crate::runtime) const fn local_base(&self) -> Option<usize> {
        match self {
            Self::Call { local_base, .. } | Self::EvalBoundary { local_base, .. } => {
                Some(*local_base)
            }
            Self::TemporaryThis { .. } | Self::Bytecode { .. } => None,
        }
    }

    pub(in crate::runtime) const fn rebase_local_base(
        &mut self,
        local_base: usize,
    ) -> Result<(), ()> {
        match self {
            Self::Call {
                local_base: base, ..
            }
            | Self::EvalBoundary {
                local_base: base, ..
            } => {
                *base = local_base;
                Ok(())
            }
            Self::TemporaryThis { .. } | Self::Bytecode { .. } => Err(()),
        }
    }

    pub(in crate::runtime) const fn upvalues(&self) -> Option<&FunctionUpvalues> {
        match self {
            Self::Call { upvalues, .. } => Some(upvalues),
            Self::TemporaryThis { .. } | Self::EvalBoundary { .. } | Self::Bytecode { .. } => None,
        }
    }

    pub(in crate::runtime) fn with_environments(&self) -> Option<&[Value]> {
        match self {
            Self::Call {
                with_environments, ..
            }
            | Self::Bytecode {
                with_environments, ..
            }
            | Self::EvalBoundary {
                with_environments, ..
            } => Some(with_environments),
            Self::TemporaryThis { .. } => None,
        }
    }

    pub(in crate::runtime) const fn with_environments_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            Self::Call {
                with_environments, ..
            }
            | Self::Bytecode {
                with_environments, ..
            }
            | Self::EvalBoundary {
                with_environments, ..
            } => Some(with_environments),
            Self::TemporaryThis { .. } => None,
        }
    }

    pub(in crate::runtime) const fn function_id(&self) -> Option<FunctionId> {
        match self {
            Self::Call { continuation, .. } => match continuation {
                Some(continuation) => continuation.function_id(),
                None => None,
            },
            Self::TemporaryThis { .. } | Self::EvalBoundary { .. } | Self::Bytecode { .. } => None,
        }
    }

    pub(in crate::runtime) const fn this_value(&self) -> Option<&Value> {
        match self {
            Self::Call { this_value, .. } | Self::TemporaryThis { this_value, .. } => {
                Some(this_value)
            }
            Self::EvalBoundary { .. } | Self::Bytecode { .. } => None,
        }
    }

    pub(in crate::runtime) const fn new_target(&self) -> Option<&Value> {
        match self {
            Self::Call { new_target, .. } => Some(new_target),
            Self::TemporaryThis { .. } | Self::EvalBoundary { .. } | Self::Bytecode { .. } => None,
        }
    }

    pub(in crate::runtime) const fn super_binding(&self) -> Option<&Rc<FunctionSuperBinding>> {
        match self {
            Self::Call { super_binding, .. } => super_binding.as_ref(),
            Self::TemporaryThis { .. } | Self::EvalBoundary { .. } | Self::Bytecode { .. } => None,
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

    pub(in crate::runtime) const fn is_bytecode(&self) -> bool {
        matches!(self, Self::Bytecode { .. })
    }

    pub(in crate::runtime) const fn continuation(&self) -> Option<&BytecodeContinuationFrame> {
        match self {
            Self::Call { continuation, .. }
            | Self::TemporaryThis { continuation, .. }
            | Self::EvalBoundary { continuation, .. }
            | Self::Bytecode { continuation, .. } => continuation.as_ref(),
        }
    }

    pub(in crate::runtime) const fn continuation_mut(
        &mut self,
    ) -> &mut Option<BytecodeContinuationFrame> {
        match self {
            Self::Call { continuation, .. }
            | Self::TemporaryThis { continuation, .. }
            | Self::EvalBoundary { continuation, .. }
            | Self::Bytecode { continuation, .. } => continuation,
        }
    }
}
