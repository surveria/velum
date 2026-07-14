use crate::{
    binding_metadata::BindingOperand,
    bytecode::BytecodeBinding,
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::to_boolean,
        activation::{
            ActivationFrame, DynamicEnvironment, EvalBindingEnvironment, FunctionEnvironmentPhase,
        },
    },
    storage::atom::AtomId,
    value::Value,
};

const SYMBOL_UNSCOPABLES_PROPERTY: &str = "unscopables";

#[derive(Debug, Clone)]
pub(in crate::runtime) struct WithBindingReference {
    target: DynamicBindingTarget,
    provides_implicit_this: bool,
}

#[derive(Debug, Clone)]
enum DynamicBindingTarget {
    Object(Value),
    EvalBinding {
        environment: EvalBindingEnvironment,
        atom: AtomId,
    },
}

impl WithBindingReference {
    const fn with_object(object: Value) -> Self {
        Self {
            target: DynamicBindingTarget::Object(object),
            provides_implicit_this: true,
        }
    }

    const fn eval_var(object: Value) -> Self {
        Self {
            target: DynamicBindingTarget::Object(object),
            provides_implicit_this: false,
        }
    }

    const fn eval_binding(environment: EvalBindingEnvironment, atom: AtomId) -> Self {
        Self {
            target: DynamicBindingTarget::EvalBinding { environment, atom },
            provides_implicit_this: false,
        }
    }

    pub(in crate::runtime) const fn object(&self) -> Option<&Value> {
        match &self.target {
            DynamicBindingTarget::Object(object) => Some(object),
            DynamicBindingTarget::EvalBinding { .. } => None,
        }
    }

    pub(in crate::runtime) fn call_this_value(&self) -> Value {
        if self.provides_implicit_this
            && let DynamicBindingTarget::Object(object) = &self.target
        {
            return object.clone();
        }
        Value::Undefined
    }

    pub(in crate::runtime) fn get(
        &self,
        context: &mut Context,
        binding: &BytecodeBinding,
    ) -> Result<Value> {
        match &self.target {
            DynamicBindingTarget::Object(object) => {
                let lookup = context.property_lookup(binding.name().as_str());
                if !context.has_property_value_with_lookup(object, lookup)? {
                    if binding.strict_write() || context.current_code_is_strict()? {
                        return Err(crate::runtime::control::reference_error_undefined(
                            binding.name(),
                        ));
                    }
                    return Ok(Value::Undefined);
                }
                context.get(object, lookup)
            }
            DynamicBindingTarget::EvalBinding { environment, atom } => environment
                .binding(*atom)?
                .ok_or_else(|| crate::runtime::control::reference_error_undefined(binding.name()))?
                .value(binding.name()),
        }
    }

    pub(in crate::runtime) fn set(
        &self,
        context: &mut Context,
        binding: &BytecodeBinding,
        value: Value,
    ) -> Result<()> {
        match &self.target {
            DynamicBindingTarget::Object(object) => {
                let lookup = context.property_lookup(binding.name().as_str());
                if !context.has_property_value_with_lookup(object, lookup)?
                    && binding.strict_write()
                {
                    return Err(crate::runtime::control::reference_error_undefined(
                        binding.name(),
                    ));
                }
                let failure = if binding.strict_write() {
                    crate::runtime::abstract_operations::SetFailureBehavior::Throw
                } else {
                    crate::runtime::abstract_operations::SetFailureBehavior::ReturnFalse
                };
                context.set(object, lookup, value, object, failure)?;
                Ok(())
            }
            DynamicBindingTarget::EvalBinding { environment, atom } => {
                let cell = environment.binding(*atom)?.ok_or_else(|| {
                    crate::runtime::control::reference_error_undefined(binding.name())
                })?;
                context.assign_bytecode_cell(binding, &cell, value)
            }
        }
    }

    pub(in crate::runtime) fn delete(
        &self,
        context: &mut Context,
        binding: &BytecodeBinding,
    ) -> Result<bool> {
        match &self.target {
            DynamicBindingTarget::Object(object) => {
                let lookup = context.property_lookup(binding.name().as_str());
                context.delete_property_value_with_lookup(object, lookup)
            }
            DynamicBindingTarget::EvalBinding { environment, atom } => environment.delete(*atom),
        }
    }
}

impl Context {
    fn current_code_is_strict(&self) -> Result<bool> {
        for frame in self.activation_frames.iter().rev() {
            if frame.is_eval_boundary() {
                return Ok(false);
            }
            if let Some(function) = frame.function_id() {
                return self
                    .function(function)
                    .map(|function| function.bytecode.strict());
            }
        }
        Ok(false)
    }

    pub(in crate::runtime) fn current_dynamic_environments(&self) -> &[DynamicEnvironment] {
        for frame in self.activation_frames.iter().rev() {
            if let Some(environments) = frame.dynamic_environments() {
                return environments;
            }
            if frame.is_eval_boundary() {
                return &[];
            }
        }
        &[]
    }

    fn current_dynamic_environment_index(&self) -> Result<usize> {
        self.activation_frames
            .iter()
            .rposition(|frame| frame.is_eval_boundary() || frame.dynamic_environments().is_some())
            .ok_or_else(|| Error::runtime("dynamic environment activation is missing"))
    }

    fn current_dynamic_environments_mut(&mut self) -> Result<&mut Vec<DynamicEnvironment>> {
        let index = self.current_dynamic_environment_index()?;
        let frame = self
            .activation_frames
            .get_mut(index)
            .ok_or_else(|| Error::runtime("dynamic environment activation disappeared"))?;
        frame
            .dynamic_environments_mut()
            .ok_or_else(|| Error::runtime("dynamic environment activation is unavailable"))
    }

    pub(in crate::runtime) fn push_with_environment(&mut self, object: Value) -> Result<()> {
        let index = self.current_dynamic_environment_index()?;
        self.storage_ledger
            .grow_count(crate::runtime::VmStorageKind::Binding, 1)?;
        let Some(environments) = self
            .activation_frames
            .get_mut(index)
            .and_then(ActivationFrame::dynamic_environments_mut)
        else {
            self.storage_ledger
                .release_count(crate::runtime::VmStorageKind::Binding, 1)?;
            return Err(Error::runtime("with environment activation disappeared"));
        };
        environments.push(DynamicEnvironment::With(object));
        Ok(())
    }

    pub(in crate::runtime) fn push_eval_binding_environment(
        &mut self,
        environment: EvalBindingEnvironment,
    ) -> Result<()> {
        let index = self.current_dynamic_environment_index()?;
        self.storage_ledger
            .grow_count(crate::runtime::VmStorageKind::Binding, 1)?;
        let Some(environments) = self
            .activation_frames
            .get_mut(index)
            .and_then(ActivationFrame::dynamic_environments_mut)
        else {
            self.storage_ledger
                .release_count(crate::runtime::VmStorageKind::Binding, 1)?;
            return Err(Error::runtime("eval binding activation disappeared"));
        };
        let position = environments
            .iter()
            .position(|environment| matches!(environment, DynamicEnvironment::With(_)))
            .unwrap_or(environments.len());
        environments.insert(position, DynamicEnvironment::EvalBindings(environment));
        Ok(())
    }

    pub(in crate::runtime) fn register_eval_binding(
        &self,
        environment: &EvalBindingEnvironment,
        atom: AtomId,
        cell: crate::runtime::binding::scope::BindingCell,
        deletable: bool,
    ) -> Result<()> {
        self.storage_ledger
            .grow_count(crate::runtime::VmStorageKind::Binding, 1)?;
        match environment.insert(atom, cell, deletable) {
            Ok(true) => Ok(()),
            Ok(false) => self
                .storage_ledger
                .release_count(crate::runtime::VmStorageKind::Binding, 1),
            Err(error) => {
                self.storage_ledger
                    .release_count(crate::runtime::VmStorageKind::Binding, 1)?;
                Err(error)
            }
        }
    }

    pub(in crate::runtime) fn assign_eval_annex_b_var(
        &self,
        atom: AtomId,
        name: &str,
        value: &Value,
    ) -> Result<bool> {
        for environment in self.current_dynamic_environments().iter().rev() {
            let DynamicEnvironment::EvalBindings(environment) = environment else {
                continue;
            };
            if environment.assign_annex_b(atom, name, value.clone())? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(in crate::runtime) fn pop_eval_binding_environment(
        &mut self,
        expected: &EvalBindingEnvironment,
    ) -> Result<()> {
        let index = self.current_dynamic_environment_index()?;
        let environments = self
            .activation_frames
            .get_mut(index)
            .and_then(ActivationFrame::dynamic_environments_mut)
            .ok_or_else(|| Error::runtime("eval binding activation disappeared"))?;
        let position = environments
            .iter()
            .position(|environment| {
                matches!(environment, DynamicEnvironment::EvalBindings(active) if active.same_environment(expected))
            })
            .ok_or_else(|| Error::runtime("eval binding environment disappeared"))?;
        let binding_count = environments
            .get(position)
            .ok_or_else(|| Error::runtime("eval binding environment disappeared"))?
            .storage_binding_count()?;
        let environment = environments.remove(position);
        if let Err(error) = self
            .storage_ledger
            .release_count(crate::runtime::VmStorageKind::Binding, binding_count)
        {
            let environments = self
                .activation_frames
                .get_mut(index)
                .and_then(ActivationFrame::dynamic_environments_mut)
                .ok_or_else(|| Error::runtime("eval binding activation disappeared"))?;
            environments.insert(position, environment);
            return Err(error);
        }
        Ok(())
    }

    pub(in crate::runtime) fn pop_with_environment(&mut self) -> Result<Value> {
        let environment = self
            .current_dynamic_environments_mut()?
            .pop()
            .ok_or_else(|| Error::runtime("with environment disappeared"))?;
        let DynamicEnvironment::With(object) = environment else {
            self.current_dynamic_environments_mut()?.push(environment);
            return Err(Error::runtime(
                "active dynamic environment is not a with environment",
            ));
        };
        if let Err(error) = self
            .storage_ledger
            .release_count(crate::runtime::VmStorageKind::Binding, 1)
        {
            self.current_dynamic_environments_mut()?
                .push(DynamicEnvironment::With(object));
            return Err(error);
        }
        Ok(object)
    }

    pub(in crate::runtime) fn resolve_with_binding(
        &mut self,
        binding: &BytecodeBinding,
    ) -> Result<Option<WithBindingReference>> {
        let environments = self.current_dynamic_environments().to_vec();
        if self.direct_eval_binding_layout_is_active() {
            let captured_count = self
                .current_captured_dynamic_environment_count()
                .min(environments.len());
            let (captured, active) = environments.split_at(captured_count);
            if let Some(reference) = self.resolve_dynamic_binding_chain(
                binding,
                active,
                count_with_environments(active),
            )? {
                return Ok(Some(reference));
            }
            let atom = self.intern_static_name_atom(binding.name().name())?;
            if self
                .locals
                .iter()
                .skip(self.current_local_frame_start())
                .rev()
                .any(|scope| scope.contains(atom))
            {
                return Ok(None);
            }
            return self.resolve_dynamic_binding_chain(
                binding,
                captured,
                count_with_environments(captured),
            );
        }
        let count = usize::try_from(binding.with_environment_count())
            .map_err(|_| Error::limit("with environment count exceeded addressable range"))?;
        self.resolve_dynamic_binding_chain(binding, &environments, count)
    }

    fn resolve_dynamic_binding_chain(
        &mut self,
        binding: &BytecodeBinding,
        environments: &[DynamicEnvironment],
        count: usize,
    ) -> Result<Option<WithBindingReference>> {
        if count > count_with_environments(environments) {
            return Err(Error::runtime(
                "captured with environment chain is incomplete",
            ));
        }
        let resolves_eval_var = matches!(
            binding.operand(),
            BindingOperand::Global { .. }
                | BindingOperand::EvalVariable { .. }
                | BindingOperand::Unresolved
        );
        let mut remaining_with_environments = count;
        for environment in environments.iter().rev().cloned() {
            match environment {
                DynamicEnvironment::With(object) if remaining_with_environments > 0 => {
                    remaining_with_environments = remaining_with_environments.saturating_sub(1);
                    if self.with_object_has_binding(&object, binding.name().as_str())? {
                        return Ok(Some(WithBindingReference::with_object(object)));
                    }
                }
                DynamicEnvironment::EvalVar(object) if resolves_eval_var => {
                    let lookup = self.property_lookup(binding.name().as_str());
                    if self.has_property_value_with_lookup(&object, lookup)? {
                        return Ok(Some(WithBindingReference::eval_var(object)));
                    }
                }
                DynamicEnvironment::EvalBindings(environment) if resolves_eval_var => {
                    let atom = self.intern_static_name_atom(binding.name().name())?;
                    if environment.binding(atom)?.is_some() {
                        return Ok(Some(WithBindingReference::eval_binding(environment, atom)));
                    }
                }
                DynamicEnvironment::CapturedLexical(environment) if resolves_eval_var => {
                    let atom = self.intern_static_name_atom(binding.name().name())?;
                    if environment.binding(atom)?.is_some() {
                        return Ok(Some(WithBindingReference::eval_binding(environment, atom)));
                    }
                }
                DynamicEnvironment::With(_)
                | DynamicEnvironment::EvalVar(_)
                | DynamicEnvironment::EvalBindings(_)
                | DynamicEnvironment::CapturedLexical(_) => {}
            }
        }
        Ok(None)
    }

    fn current_captured_dynamic_environment_count(&self) -> usize {
        for frame in self.activation_frames.iter().rev() {
            if frame.is_eval_boundary() {
                return 0;
            }
            if let Some(count) = frame.captured_dynamic_environment_count() {
                return count;
            }
        }
        0
    }

    pub(in crate::runtime) fn current_parameter_eval_var_environment(&self) -> Option<Value> {
        let phase = self.activation_frames.iter().rev().find_map(|frame| {
            if frame.is_eval_boundary() {
                return Some(None);
            }
            frame.function_environment_phase().map(Some)
        })??;
        if phase != FunctionEnvironmentPhase::ParameterInitialization {
            return None;
        }
        self.current_dynamic_environments()
            .iter()
            .rev()
            .find_map(|environment| match environment {
                DynamicEnvironment::EvalVar(value) => Some(value.clone()),
                DynamicEnvironment::With(_)
                | DynamicEnvironment::EvalBindings(_)
                | DynamicEnvironment::CapturedLexical(_) => None,
            })
    }

    fn with_object_has_binding(&mut self, object: &Value, name: &str) -> Result<bool> {
        let lookup = self.property_lookup(name);
        if !self.has_property_value_with_lookup(object, lookup)? {
            return Ok(false);
        }
        let symbol_constructor = self.symbol_constructor_value()?;
        let unscopables_symbol =
            self.get_named(&symbol_constructor, SYMBOL_UNSCOPABLES_PROPERTY)?;
        let unscopables_key = self.dynamic_property_key(&unscopables_symbol)?;
        let unscopables = self.get(object, unscopables_key.lookup())?;
        if self.semantic_object_ref(&unscopables)?.is_none() {
            return Ok(true);
        }
        let blocked_lookup = self.property_lookup(name);
        let blocked = self.get(&unscopables, blocked_lookup)?;
        Ok(!to_boolean(self, &blocked)?)
    }
}

fn count_with_environments(environments: &[DynamicEnvironment]) -> usize {
    environments
        .iter()
        .filter(|environment| matches!(environment, DynamicEnvironment::With(_)))
        .count()
}
