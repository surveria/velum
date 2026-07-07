use crate::{
    bytecode::{BytecodeArrayIndex, BytecodeProperty},
    error::Result,
    runtime::Context,
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
        if let Some(value) = self.get_array_length_property_value(object)? {
            return Ok(value);
        }
        self.get_static_property_value(object, property.name(), property.access())
    }

    pub(super) fn eval_bytecode_array_index_member(
        &mut self,
        object: &Value,
        property: &BytecodeProperty,
        index: BytecodeArrayIndex,
    ) -> Result<Value> {
        if let Value::Object(id) = object
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
    ) -> Result<()> {
        let value = self.runtime_value(value)?;
        if let Value::Object(id) = object
            && self.objects.set_array_index_if_array(
                *id,
                index.index()?,
                value.clone(),
                self.limits.max_object_properties,
            )?
        {
            return Ok(());
        }
        self.set_static_property_value(object, property.name(), property.access(), value)
    }

    pub(super) fn set_dynamic_array_index_property(
        &mut self,
        object: &Value,
        property: &Value,
        value: Value,
    ) -> Result<bool> {
        let Value::Object(id) = object else {
            return Ok(false);
        };
        let Some(index) = self.objects.dynamic_array_index_if_array(*id, property)? else {
            return Ok(false);
        };
        let value = self.runtime_value(value)?;
        self.objects
            .set_array_index_if_array(*id, index, value, self.limits.max_object_properties)
    }

    pub(super) fn eval_bytecode_array_index_update(
        &mut self,
        object: &Value,
        property: &BytecodeProperty,
        index: BytecodeArrayIndex,
        op: UpdateOp,
        prefix: bool,
    ) -> Result<Value> {
        let Some(mutation) =
            self.try_array_index_read_modify_write(object, index.index()?, |_, old_value| {
                Self::updated_bytecode_number(old_value, op)
            })?
        else {
            return self.eval_bytecode_update_static_property(
                object,
                property.name(),
                property.access(),
                op,
                prefix,
            );
        };
        self.array_index_mutation_result(object, property, mutation, prefix)
    }

    pub(super) fn eval_dynamic_array_index_update(
        &mut self,
        object: &Value,
        property: &Value,
        access: StaticPropertyAccessId,
        op: UpdateOp,
        prefix: bool,
    ) -> Result<Option<Value>> {
        let Value::Object(id) = object else {
            return Ok(None);
        };
        let Some(index) = self.objects.dynamic_array_index_if_array(*id, property)? else {
            return Ok(None);
        };
        let Some(mutation) =
            self.try_array_index_read_modify_write(object, index, |_, old_value| {
                Self::updated_bytecode_number(old_value, op)
            })?
        else {
            return Ok(None);
        };
        self.dynamic_array_index_mutation_result(object, property, access, mutation, prefix)
            .map(Some)
    }

    pub(super) fn eval_bytecode_array_index_compound_assignment(
        &mut self,
        op: BinaryOp,
        object: &Value,
        property: &BytecodeProperty,
        index: BytecodeArrayIndex,
        right: &Value,
    ) -> Result<Value> {
        let Some(mutation) = self.try_array_index_read_modify_write(
            object,
            index.index()?,
            |context, old_value| context.eval_bytecode_compound_value(op, old_value, right),
        )?
        else {
            return self.eval_bytecode_static_compound_assignment(
                op,
                object,
                property.name(),
                property.access(),
                right,
            );
        };
        self.array_index_mutation_result(object, property, mutation, true)
    }

    pub(super) fn eval_dynamic_array_index_compound_assignment(
        &mut self,
        op: BinaryOp,
        object: &Value,
        property: &Value,
        access: StaticPropertyAccessId,
        right: &Value,
    ) -> Result<Option<Value>> {
        let Value::Object(id) = object else {
            return Ok(None);
        };
        let Some(index) = self.objects.dynamic_array_index_if_array(*id, property)? else {
            return Ok(None);
        };
        let Some(mutation) =
            self.try_array_index_read_modify_write(object, index, |context, old_value| {
                context.eval_bytecode_compound_value(op, old_value, right)
            })?
        else {
            return Ok(None);
        };
        self.dynamic_array_index_mutation_result(object, property, access, mutation, true)
            .map(Some)
    }

    fn try_array_index_read_modify_write(
        &mut self,
        object: &Value,
        index: usize,
        update: impl FnOnce(&mut Self, &Value) -> Result<Value>,
    ) -> Result<Option<ArrayIndexMutation>> {
        let Value::Object(id) = object else {
            return Ok(None);
        };
        let Some(old_value) = self.objects.array_index_value_if_array(*id, index)? else {
            return Ok(None);
        };
        let old_value = self.runtime_value(old_value)?;
        let new_value = update(self, &old_value)?;
        let new_value = self.runtime_value(new_value)?;
        if self.objects.set_array_index_if_array(
            *id,
            index,
            new_value.clone(),
            self.limits.max_object_properties,
        )? {
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

    fn array_index_mutation_result(
        &mut self,
        object: &Value,
        property: &BytecodeProperty,
        mutation: ArrayIndexMutation,
        prefix: bool,
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
                self.set_static_property_value(
                    object,
                    property.name(),
                    property.access(),
                    new_value.clone(),
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
                self.set_cached_dynamic_property_value(
                    object,
                    &mut property,
                    access,
                    new_value.clone(),
                )?;
                Ok(if prefix { new_value } else { old_value })
            }
        }
    }
}
