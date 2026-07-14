use crate::{
    binding_metadata::BindingOperand,
    bytecode::BytecodeBinding,
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::to_boolean,
        activation::{ActivationFrame, DynamicEnvironment, FunctionEnvironmentPhase},
    },
    value::Value,
};

const SYMBOL_UNSCOPABLES_PROPERTY: &str = "unscopables";

#[derive(Debug, Clone)]
pub(in crate::runtime) struct WithBindingReference {
    object: Value,
}

impl WithBindingReference {
    pub(in crate::runtime) const fn new(object: Value) -> Self {
        Self { object }
    }

    pub(in crate::runtime) const fn object(&self) -> &Value {
        &self.object
    }

    pub(in crate::runtime) fn get(
        &self,
        context: &mut Context,
        binding: &BytecodeBinding,
    ) -> Result<Value> {
        let lookup = context.property_lookup(binding.name().as_str());
        if !context.has_property_value_with_lookup(&self.object, lookup)? {
            if binding.strict_write() || context.current_code_is_strict()? {
                return Err(crate::runtime::control::reference_error_undefined(
                    binding.name(),
                ));
            }
            return Ok(Value::Undefined);
        }
        context.get(&self.object, lookup)
    }

    pub(in crate::runtime) fn set(
        &self,
        context: &mut Context,
        binding: &BytecodeBinding,
        value: Value,
    ) -> Result<()> {
        let lookup = context.property_lookup(binding.name().as_str());
        if !context.has_property_value_with_lookup(&self.object, lookup)? && binding.strict_write()
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
        context.set(&self.object, lookup, value, &self.object, failure)?;
        Ok(())
    }

    pub(in crate::runtime) fn delete(
        &self,
        context: &mut Context,
        binding: &BytecodeBinding,
    ) -> Result<bool> {
        let lookup = context.property_lookup(binding.name().as_str());
        context.delete_property_value_with_lookup(&self.object, lookup)
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
        let count = usize::try_from(binding.with_environment_count())
            .map_err(|_| Error::limit("with environment count exceeded addressable range"))?;
        let environments = self.current_dynamic_environments().to_vec();
        let available_with_environments = environments
            .iter()
            .filter(|environment| matches!(environment, DynamicEnvironment::With(_)))
            .count();
        if count > available_with_environments {
            return Err(Error::runtime(
                "captured with environment chain is incomplete",
            ));
        }
        let resolves_eval_var = matches!(
            binding.operand(),
            BindingOperand::Global { .. } | BindingOperand::Unresolved
        );
        let mut remaining_with_environments = count;
        for environment in environments.into_iter().rev() {
            match environment {
                DynamicEnvironment::With(object) if remaining_with_environments > 0 => {
                    remaining_with_environments = remaining_with_environments.saturating_sub(1);
                    if self.with_object_has_binding(&object, binding.name().as_str())? {
                        return Ok(Some(WithBindingReference::new(object)));
                    }
                }
                DynamicEnvironment::EvalVar(object) if resolves_eval_var => {
                    let lookup = self.property_lookup(binding.name().as_str());
                    if self.has_property_value_with_lookup(&object, lookup)? {
                        return Ok(Some(WithBindingReference::new(object)));
                    }
                }
                DynamicEnvironment::With(_) | DynamicEnvironment::EvalVar(_) => {}
            }
        }
        Ok(None)
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
                DynamicEnvironment::With(_) => None,
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
