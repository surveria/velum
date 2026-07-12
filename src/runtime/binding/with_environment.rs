use crate::{
    bytecode::BytecodeBinding,
    error::{Error, Result},
    runtime::{Context, abstract_operations::to_boolean},
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
        let updated = context.set(&self.object, lookup, value.clone(), &self.object, failure)?;
        if updated
            && let Value::Object(id) = &self.object
            && context.is_global_object_id(*id)
        {
            context.sync_global_object_property_binding(binding.name().as_str(), value)?;
        }
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

    pub(in crate::runtime) fn current_with_environments(&self) -> &[Value] {
        for frame in self.activation_frames.iter().rev() {
            if frame.is_eval_boundary() {
                return &[];
            }
            if let Some(environments) = frame.with_environments() {
                return environments;
            }
        }
        &[]
    }

    fn current_with_environments_mut(&mut self) -> Result<&mut Vec<Value>> {
        let Some(index) = self
            .activation_frames
            .iter()
            .rposition(|frame| frame.is_eval_boundary() || frame.with_environments().is_some())
        else {
            return Err(Error::runtime("with environment activation is missing"));
        };
        let frame = self
            .activation_frames
            .get_mut(index)
            .ok_or_else(|| Error::runtime("with environment activation disappeared"))?;
        if frame.is_eval_boundary() {
            return Err(Error::runtime("with environment crossed an eval boundary"));
        }
        frame
            .with_environments_mut()
            .ok_or_else(|| Error::runtime("with environment activation is unavailable"))
    }

    pub(in crate::runtime) fn push_with_environment(&mut self, object: Value) -> Result<()> {
        self.current_with_environments_mut()?.push(object);
        Ok(())
    }

    pub(in crate::runtime) fn pop_with_environment(&mut self) -> Result<Value> {
        self.current_with_environments_mut()?
            .pop()
            .ok_or_else(|| Error::runtime("with environment disappeared"))
    }

    pub(in crate::runtime) fn resolve_with_binding(
        &mut self,
        binding: &BytecodeBinding,
    ) -> Result<Option<WithBindingReference>> {
        let count = usize::try_from(binding.with_environment_count())
            .map_err(|_| Error::limit("with environment count exceeded addressable range"))?;
        if count == 0 {
            return Ok(None);
        }
        let environments = self.current_with_environments();
        if count > environments.len() {
            return Err(Error::runtime(
                "captured with environment chain is incomplete",
            ));
        }
        let candidates = environments
            .get(environments.len().saturating_sub(count)..)
            .ok_or_else(|| Error::runtime("with environment range is invalid"))?
            .to_vec();
        for object in candidates.into_iter().rev() {
            if self.with_object_has_binding(&object, binding.name().as_str())? {
                return Ok(Some(WithBindingReference::new(object)));
            }
        }
        Ok(None)
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
        Ok(!to_boolean(&blocked))
    }
}
