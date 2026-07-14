use std::{cell::RefCell, rc::Rc};

use crate::{
    error::{Error, Result as EngineResult},
    runtime::binding::scope::BindingCell,
    storage::atom::AtomId,
    value::{FunctionId, Value},
};

use super::{
    FunctionActivationEnvironment, FunctionUpvalues, bytecode::BytecodeContinuationFrame,
    function::FunctionSuperBinding, private::PrivateEnvironment,
};

#[derive(Clone, Debug)]
pub(in crate::runtime) enum DynamicEnvironment {
    With(Value),
    EvalVar(Value),
    EvalBindings(EvalBindingEnvironment),
    CapturedLexical(EvalBindingEnvironment),
}

impl DynamicEnvironment {
    pub(in crate::runtime) fn storage_binding_count(&self) -> EngineResult<usize> {
        match self {
            Self::With(_) | Self::EvalVar(_) => Ok(1),
            Self::EvalBindings(environment) | Self::CapturedLexical(environment) => environment
                .len()?
                .checked_add(1)
                .ok_or_else(|| Error::limit("eval environment binding count overflowed")),
        }
    }

    pub(in crate::runtime) fn for_each_value(
        &self,
        mut visit: impl FnMut(&Value) -> EngineResult<()>,
    ) -> EngineResult<()> {
        match self {
            Self::With(value) | Self::EvalVar(value) => visit(value),
            Self::EvalBindings(environment) | Self::CapturedLexical(environment) => {
                environment.for_each_value(visit)
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(in crate::runtime) struct EvalBindingEnvironment(Rc<RefCell<Vec<EvalBindingEntry>>>);

#[derive(Clone, Debug)]
struct EvalBindingEntry {
    atom: AtomId,
    cell: BindingCell,
    deletable: bool,
    active: bool,
}

impl EvalBindingEnvironment {
    pub(in crate::runtime) fn contains(&self, atom: AtomId) -> EngineResult<bool> {
        self.binding(atom).map(|binding| binding.is_some())
    }

    pub(in crate::runtime) fn insert(
        &self,
        atom: AtomId,
        cell: BindingCell,
        deletable: bool,
    ) -> EngineResult<bool> {
        let mut entries = self
            .0
            .try_borrow_mut()
            .map_err(|_| Error::runtime("eval binding environment is already borrowed"))?;
        match entries.binary_search_by_key(&atom, |entry| entry.atom) {
            Ok(position) => {
                let Some(entry) = entries.get_mut(position) else {
                    return Err(Error::runtime("eval binding entry disappeared"));
                };
                if !entry.cell.same_cell(&cell) {
                    return Err(Error::runtime("eval binding cell identity changed"));
                }
                return Ok(false);
            }
            Err(position) => entries.insert(
                position,
                EvalBindingEntry {
                    atom,
                    cell,
                    deletable,
                    active: true,
                },
            ),
        }
        Ok(true)
    }

    pub(in crate::runtime) fn binding(&self, atom: AtomId) -> EngineResult<Option<BindingCell>> {
        let entries = self
            .0
            .try_borrow()
            .map_err(|_| Error::runtime("eval binding environment is already mutably borrowed"))?;
        let Ok(position) = entries.binary_search_by_key(&atom, |entry| entry.atom) else {
            return Ok(None);
        };
        Ok(entries
            .get(position)
            .filter(|entry| entry.active)
            .map(|entry| entry.cell.clone()))
    }

    pub(in crate::runtime) fn delete(&self, atom: AtomId) -> EngineResult<bool> {
        let mut entries = self
            .0
            .try_borrow_mut()
            .map_err(|_| Error::runtime("eval binding environment is already borrowed"))?;
        let Ok(position) = entries.binary_search_by_key(&atom, |entry| entry.atom) else {
            return Ok(true);
        };
        let Some(entry) = entries.get_mut(position) else {
            return Err(Error::runtime("eval binding entry disappeared"));
        };
        if !entry.active {
            return Ok(true);
        }
        if !entry.deletable {
            return Ok(false);
        }
        entry.cell.mark_deleted()?;
        entry.active = false;
        Ok(true)
    }

    pub(in crate::runtime) fn assign_annex_b(
        &self,
        atom: AtomId,
        name: &str,
        value: Value,
    ) -> EngineResult<bool> {
        let mut entries = self
            .0
            .try_borrow_mut()
            .map_err(|_| Error::runtime("eval binding environment is already borrowed"))?;
        let Ok(position) = entries.binary_search_by_key(&atom, |entry| entry.atom) else {
            return Ok(false);
        };
        let Some(entry) = entries.get_mut(position) else {
            return Err(Error::runtime("eval binding entry disappeared"));
        };
        if entry.active {
            entry.cell.assign(name, value)?;
        } else {
            entry.cell.restore_deleted(value)?;
            entry.active = true;
        }
        Ok(true)
    }

    pub(in crate::runtime) fn same_environment(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub(in crate::runtime) fn len(&self) -> EngineResult<usize> {
        self.0
            .try_borrow()
            .map(|entries| entries.len())
            .map_err(|_| Error::runtime("eval binding environment is already mutably borrowed"))
    }

    fn for_each_value(
        &self,
        mut visit: impl FnMut(&Value) -> EngineResult<()>,
    ) -> EngineResult<()> {
        let entries = self
            .0
            .try_borrow()
            .map_err(|_| Error::runtime("eval binding environment is already mutably borrowed"))?;
        for entry in entries.iter() {
            if !entry.active {
                continue;
            }
            if let Some(result) = entry.cell.with_initialized_value(&mut visit) {
                result?;
            }
        }
        Ok(())
    }
}

/// One VM-owned synchronous execution activation.
///
/// Call frames own every value needed to make the current invocation
/// resumable. Temporary `this` frames preserve class-field evaluation
/// semantics, while an evaluation boundary hides caller lexical state from
/// generated Function-constructor source without removing its roots.
#[derive(Debug)]
pub(in crate::runtime) enum ActivationFrame {
    Call {
        function: FunctionId,
        local_base: usize,
        environment_phase: FunctionEnvironmentPhase,
        upvalues: FunctionUpvalues,
        dynamic_environments: Vec<DynamicEnvironment>,
        captured_dynamic_environment_count: usize,
        this_value: Value,
        new_target: Value,
        super_binding: Option<Rc<FunctionSuperBinding>>,
        private_environment: Option<Rc<PrivateEnvironment>>,
        continuation: Option<BytecodeContinuationFrame>,
    },
    TemporaryThis {
        this_value: Value,
        new_target: Value,
        super_binding: Rc<FunctionSuperBinding>,
        private_environment: Option<Rc<PrivateEnvironment>>,
        class_field_initializer: bool,
        continuation: Option<BytecodeContinuationFrame>,
    },
    EvalBoundary {
        local_base: usize,
        dynamic_environments: Vec<DynamicEnvironment>,
        private_environment: Option<Rc<PrivateEnvironment>>,
        continuation: Option<BytecodeContinuationFrame>,
    },
    Bytecode {
        dynamic_environments: Vec<DynamicEnvironment>,
        private_environment: Option<Rc<PrivateEnvironment>>,
        continuation: Option<BytecodeContinuationFrame>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum FunctionEnvironmentPhase {
    Setup,
    ParameterInitialization,
    SharedBody,
    SeparateBody,
}

impl FunctionEnvironmentPhase {
    pub(in crate::runtime) const fn has_separate_body_scope(self) -> bool {
        matches!(self, Self::SeparateBody)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::runtime) struct ActivationFrameStorageFootprint {
    binding_count: usize,
    execution_frame_count: usize,
}

pub(in crate::runtime) struct FunctionCallActivation {
    pub(in crate::runtime) function: FunctionId,
    pub(in crate::runtime) environment: FunctionActivationEnvironment,
    pub(in crate::runtime) captured_dynamic_environment_count: usize,
    pub(in crate::runtime) this_value: Value,
    pub(in crate::runtime) new_target: Value,
    pub(in crate::runtime) super_binding: Option<Rc<FunctionSuperBinding>>,
    pub(in crate::runtime) private_environment: Option<Rc<PrivateEnvironment>>,
}

impl ActivationFrameStorageFootprint {
    pub(in crate::runtime) const fn binding_count(self) -> usize {
        self.binding_count
    }

    pub(in crate::runtime) const fn execution_frame_count(self) -> usize {
        self.execution_frame_count
    }

    pub(in crate::runtime) fn checked_add(self, other: Self) -> crate::error::Result<Self> {
        let binding_count = self
            .binding_count
            .checked_add(other.binding_count)
            .ok_or_else(activation_storage_overflow)?;
        let execution_frame_count = self
            .execution_frame_count
            .checked_add(other.execution_frame_count)
            .ok_or_else(activation_storage_overflow)?;
        Ok(Self {
            binding_count,
            execution_frame_count,
        })
    }
}

impl ActivationFrame {
    pub(in crate::runtime) fn storage_footprint(
        &self,
    ) -> crate::error::Result<ActivationFrameStorageFootprint> {
        let binding_count = self
            .upvalues()
            .map_or(0, |upvalues| upvalues.len())
            .checked_add(self.dynamic_environments().map_or(Ok(0), |environments| {
                environments.iter().try_fold(0_usize, |count, environment| {
                    count
                        .checked_add(environment.storage_binding_count()?)
                        .ok_or_else(activation_storage_overflow)
                })
            })?)
            .ok_or_else(activation_storage_overflow)?;
        let control_count = self
            .continuation()
            .map_or(0, super::bytecode::BytecodeContinuationFrame::control_count);
        let execution_frame_count = 1_usize
            .checked_add(control_count)
            .ok_or_else(activation_storage_overflow)?;
        Ok(ActivationFrameStorageFootprint {
            binding_count,
            execution_frame_count,
        })
    }

    pub(in crate::runtime) fn call(local_base: usize, activation: FunctionCallActivation) -> Self {
        let FunctionCallActivation {
            function,
            environment,
            captured_dynamic_environment_count,
            this_value,
            new_target,
            super_binding,
            private_environment,
        } = activation;
        let (upvalues, dynamic_environments) = environment;
        Self::Call {
            function,
            local_base,
            environment_phase: FunctionEnvironmentPhase::Setup,
            upvalues,
            dynamic_environments,
            captured_dynamic_environment_count,
            this_value,
            new_target,
            super_binding,
            private_environment,
            continuation: Some(BytecodeContinuationFrame::function(function)),
        }
    }

    pub(in crate::runtime) const fn temporary_this(
        this_value: Value,
        super_binding: Rc<FunctionSuperBinding>,
        private_environment: Option<Rc<PrivateEnvironment>>,
        class_field_initializer: bool,
    ) -> Self {
        Self::TemporaryThis {
            this_value,
            new_target: Value::Undefined,
            super_binding,
            private_environment,
            class_field_initializer,
            continuation: None,
        }
    }

    pub(in crate::runtime) const fn eval_boundary(local_base: usize) -> Self {
        Self::EvalBoundary {
            local_base,
            dynamic_environments: Vec::new(),
            private_environment: None,
            continuation: None,
        }
    }

    pub(in crate::runtime) const fn bytecode(
        continuation: BytecodeContinuationFrame,
        dynamic_environments: Vec<DynamicEnvironment>,
    ) -> Self {
        Self::Bytecode {
            dynamic_environments,
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

    pub(in crate::runtime) const fn class_field_initializer_context(&self) -> Option<bool> {
        match self {
            Self::TemporaryThis {
                class_field_initializer,
                ..
            } => Some(*class_field_initializer),
            Self::EvalBoundary { .. } => Some(false),
            Self::Call { .. } | Self::Bytecode { .. } => None,
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

    pub(in crate::runtime) const fn function_environment_phase(
        &self,
    ) -> Option<FunctionEnvironmentPhase> {
        match self {
            Self::Call {
                environment_phase, ..
            } => Some(*environment_phase),
            Self::TemporaryThis { .. } | Self::EvalBoundary { .. } | Self::Bytecode { .. } => None,
        }
    }

    pub(in crate::runtime) const fn function_environment_phase_mut(
        &mut self,
    ) -> Option<&mut FunctionEnvironmentPhase> {
        match self {
            Self::Call {
                environment_phase, ..
            } => Some(environment_phase),
            Self::TemporaryThis { .. } | Self::EvalBoundary { .. } | Self::Bytecode { .. } => None,
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

    pub(in crate::runtime) fn dynamic_environments(&self) -> Option<&[DynamicEnvironment]> {
        match self {
            Self::Call {
                dynamic_environments,
                ..
            }
            | Self::Bytecode {
                dynamic_environments,
                ..
            }
            | Self::EvalBoundary {
                dynamic_environments,
                ..
            } => Some(dynamic_environments),
            Self::TemporaryThis { .. } => None,
        }
    }

    pub(in crate::runtime) const fn dynamic_environments_mut(
        &mut self,
    ) -> Option<&mut Vec<DynamicEnvironment>> {
        match self {
            Self::Call {
                dynamic_environments,
                ..
            }
            | Self::Bytecode {
                dynamic_environments,
                ..
            }
            | Self::EvalBoundary {
                dynamic_environments,
                ..
            } => Some(dynamic_environments),
            Self::TemporaryThis { .. } => None,
        }
    }

    pub(in crate::runtime) const fn function_id(&self) -> Option<FunctionId> {
        match self {
            Self::Call { function, .. } => Some(*function),
            Self::TemporaryThis { .. } | Self::EvalBoundary { .. } | Self::Bytecode { .. } => None,
        }
    }

    pub(in crate::runtime) const fn captured_dynamic_environment_count(&self) -> Option<usize> {
        match self {
            Self::Call {
                captured_dynamic_environment_count,
                ..
            } => Some(*captured_dynamic_environment_count),
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
            Self::Call { new_target, .. } | Self::TemporaryThis { new_target, .. } => {
                Some(new_target)
            }
            Self::EvalBoundary { .. } | Self::Bytecode { .. } => None,
        }
    }

    pub(in crate::runtime) const fn super_binding(&self) -> Option<&Rc<FunctionSuperBinding>> {
        match self {
            Self::Call { super_binding, .. } => super_binding.as_ref(),
            Self::TemporaryThis { super_binding, .. } => Some(super_binding),
            Self::EvalBoundary { .. } | Self::Bytecode { .. } => None,
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

fn activation_storage_overflow() -> Error {
    Error::limit("activation frame storage footprint overflowed")
}
