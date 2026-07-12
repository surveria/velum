use crate::{
    bytecode::{BytecodeArrayIndex, BytecodeProperty},
    error::Result,
    runtime::{Context, property::string_length_value_if_string},
    syntax::{BinaryOp, StaticPropertyAccessId, UpdateOp},
    value::Value,
};

enum ArrayIndexMutation {
    Updated { old_value: Value, new_value: Value },
    NeedsGenericSet { old_value: Value, new_value: Value },
}

impl Context {
    pub(super) fn eval_bytecode_array_length(
        &mut self,
        object: &Value,
        property: &BytecodeProperty,
    ) -> Result<Value> {
        if self.optional_optimizations_enabled() {
            if let Some(value) = self.get_array_length_property_value(object)? {
                return Ok(value);
            }
            if let Some(value) = string_length_value_if_string(object, property.name().as_str())? {
                return Ok(value);
            }
        }
        self.get_static_property_value(object, property.name(), property.access())
    }

    pub(super) fn eval_bytecode_array_index_member(
        &mut self,
        object: &Value,
        property: &BytecodeProperty,
        index: BytecodeArrayIndex,
    ) -> Result<Value> {
        if self.optional_optimizations_enabled()
            && let Value::Object(id) = object
            && let Some(value) = self
                .objects
                .array_index_value_if_array(*id, index.index()?)?
        {
            return self.runtime_value(value);
        }
        self.get_static_property_value(object, property.name(), property.access())
    }

    pub(super) fn eval_dynamic_array_index_member(
        &mut self,
        object: &Value,
        property: &Value,
    ) -> Result<Option<Value>> {
        if !self.optional_optimizations_enabled() {
            return Ok(None);
        }
        let Value::Object(id) = object else {
            return Ok(None);
        };
        let Some(index) = self.objects.dynamic_array_index_if_array(*id, property)? else {
            return Ok(None);
        };
        self.objects
            .array_index_value_if_array(*id, index)?
            .map(|value| self.runtime_value(value))
            .transpose()
    }

    pub(super) fn set_bytecode_array_index_property(
        &mut self,
        object: &Value,
        property: &BytecodeProperty,
        index: BytecodeArrayIndex,
        value: Value,
        strict: bool,
    ) -> Result<()> {
        let value = self.runtime_value(value)?;
        if self.optional_optimizations_enabled()
            && (!strict || self.bytecode_target_is_typed_array(object)?)
            && let Value::Object(id) = object
            && self.set_array_or_typed_array_index(*id, index.index()?, value.clone())?
        {
            return Ok(());
        }
        self.set_bytecode_static_property_reference(
            object,
            property.name(),
            property.access(),
            value,
            strict,
        )
    }

    pub(super) fn set_dynamic_array_index_property(
        &mut self,
        object: &Value,
        property: &Value,
        value: Value,
        strict: bool,
    ) -> Result<bool> {
        if !self.optional_optimizations_enabled() {
            return Ok(false);
        }
        let Value::Object(id) = object else {
            return Ok(false);
        };
        if strict && self.objects.typed_array(*id)?.is_none() {
            return Ok(false);
        }
        let Some(index) = self.objects.dynamic_array_index_if_array(*id, property)? else {
            return Ok(false);
        };
        let value = self.runtime_value(value)?;
        self.set_array_or_typed_array_index(*id, index, value)
    }

    pub(super) fn eval_bytecode_array_index_update(
        &mut self,
        object: &Value,
        property: &BytecodeProperty,
        index: BytecodeArrayIndex,
        op: UpdateOp,
        prefix: bool,
        strict: bool,
    ) -> Result<Value> {
        if !self.optional_optimizations_enabled()
            || (strict && !self.bytecode_target_is_typed_array(object)?)
        {
            return self.eval_bytecode_update_static_property(
                object,
                property.name(),
                property.access(),
                op,
                prefix,
                strict,
            );
        }
        let Some(mutation) = self.try_array_index_read_modify_write(
            object,
            index.index()?,
            |context, old_value| context.bytecode_update_values(old_value, op),
        )?
        else {
            return self.eval_bytecode_update_static_property(
                object,
                property.name(),
                property.access(),
                op,
                prefix,
                strict,
            );
        };
        self.array_index_mutation_result(object, property, mutation, prefix, strict)
    }

    pub(super) fn eval_dynamic_array_index_update(
        &mut self,
        object: &Value,
        property: &Value,
        access: StaticPropertyAccessId,
        op: UpdateOp,
        prefix: bool,
        strict: bool,
    ) -> Result<Option<Value>> {
        if !self.optional_optimizations_enabled() {
            return Ok(None);
        }
        let Value::Object(id) = object else {
            return Ok(None);
        };
        if strict && self.objects.typed_array(*id)?.is_none() {
            return Ok(None);
        }
        let Some(index) = self.objects.dynamic_array_index_if_array(*id, property)? else {
            return Ok(None);
        };
        let Some(mutation) =
            self.try_array_index_read_modify_write(object, index, |context, old_value| {
                context.bytecode_update_values(old_value, op)
            })?
        else {
            return Ok(None);
        };
        self.dynamic_array_index_mutation_result(object, property, access, mutation, prefix, strict)
            .map(Some)
    }

    pub(super) fn eval_bytecode_array_index_compound_assignment(
        &mut self,
        op: BinaryOp,
        object: &Value,
        property: &BytecodeProperty,
        index: BytecodeArrayIndex,
        right: &Value,
        strict: bool,
    ) -> Result<Value> {
        if !self.optional_optimizations_enabled()
            || (strict && !self.bytecode_target_is_typed_array(object)?)
        {
            return self.eval_bytecode_static_compound_assignment(
                op,
                object,
                property.name(),
                property.access(),
                right,
                strict,
            );
        }
        let Some(mutation) = self.try_array_index_read_modify_write(
            object,
            index.index()?,
            |context, old_value| {
                context
                    .eval_bytecode_compound_value(op, old_value, right)
                    .map(|new_value| (old_value.clone(), new_value))
            },
        )?
        else {
            return self.eval_bytecode_static_compound_assignment(
                op,
                object,
                property.name(),
                property.access(),
                right,
                strict,
            );
        };
        self.array_index_mutation_result(object, property, mutation, true, strict)
    }

    pub(super) fn eval_dynamic_array_index_compound_assignment(
        &mut self,
        op: BinaryOp,
        object: &Value,
        property: &Value,
        access: StaticPropertyAccessId,
        right: &Value,
        strict: bool,
    ) -> Result<Option<Value>> {
        if !self.optional_optimizations_enabled() {
            return Ok(None);
        }
        let Value::Object(id) = object else {
            return Ok(None);
        };
        if strict && self.objects.typed_array(*id)?.is_none() {
            return Ok(None);
        }
        let Some(index) = self.objects.dynamic_array_index_if_array(*id, property)? else {
            return Ok(None);
        };
        let Some(mutation) =
            self.try_array_index_read_modify_write(object, index, |context, old_value| {
                context
                    .eval_bytecode_compound_value(op, old_value, right)
                    .map(|new_value| (old_value.clone(), new_value))
            })?
        else {
            return Ok(None);
        };
        self.dynamic_array_index_mutation_result(object, property, access, mutation, true, strict)
            .map(Some)
    }

    fn try_array_index_read_modify_write(
        &mut self,
        object: &Value,
        index: usize,
        update: impl FnOnce(&mut Self, &Value) -> Result<(Value, Value)>,
    ) -> Result<Option<ArrayIndexMutation>> {
        let Value::Object(id) = object else {
            return Ok(None);
        };
        let Some(old_value) = self.objects.array_index_value_if_array(*id, index)? else {
            return Ok(None);
        };
        let old_value = self.runtime_value(old_value)?;
        let (old_value, new_value) = update(self, &old_value)?;
        let new_value = self.runtime_value(new_value)?;
        if self.set_array_or_typed_array_index(*id, index, new_value.clone())? {
            return Ok(Some(ArrayIndexMutation::Updated {
                old_value,
                new_value,
            }));
        }
        Ok(Some(ArrayIndexMutation::NeedsGenericSet {
            old_value,
            new_value,
        }))
    }

    fn bytecode_target_is_typed_array(&self, object: &Value) -> Result<bool> {
        let Value::Object(id) = object else {
            return Ok(false);
        };
        self.objects.typed_array(*id).map(|view| view.is_some())
    }

    fn array_index_mutation_result(
        &mut self,
        object: &Value,
        property: &BytecodeProperty,
        mutation: ArrayIndexMutation,
        prefix: bool,
        strict: bool,
    ) -> Result<Value> {
        match mutation {
            ArrayIndexMutation::Updated {
                old_value,
                new_value,
            } => Ok(if prefix { new_value } else { old_value }),
            ArrayIndexMutation::NeedsGenericSet {
                old_value,
                new_value,
            } => {
                self.set_bytecode_static_property_reference(
                    object,
                    property.name(),
                    property.access(),
                    new_value.clone(),
                    strict,
                )?;
                Ok(if prefix { new_value } else { old_value })
            }
        }
    }

    fn dynamic_array_index_mutation_result(
        &mut self,
        object: &Value,
        property: &Value,
        access: StaticPropertyAccessId,
        mutation: ArrayIndexMutation,
        prefix: bool,
        strict: bool,
    ) -> Result<Value> {
        match mutation {
            ArrayIndexMutation::Updated {
                old_value,
                new_value,
            } => Ok(if prefix { new_value } else { old_value }),
            ArrayIndexMutation::NeedsGenericSet {
                old_value,
                new_value,
            } => {
                let mut property = self.dynamic_property_key(property)?;
                self.set_bytecode_dynamic_property_reference(
                    object,
                    &mut property,
                    access,
                    new_value.clone(),
                    strict,
                )?;
                Ok(if prefix { new_value } else { old_value })
            }
        }
    }
}
