use crate::{
    bytecode::{BytecodeArrayIndex, BytecodeProperty},
    error::Result,
    runtime::Context,
    value::Value,
};

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
}
