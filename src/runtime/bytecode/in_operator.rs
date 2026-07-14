use crate::{
    bytecode::BytecodeDynamicProperty,
    error::{Error, Result},
    runtime::Context,
    syntax::StaticString,
    value::Value,
};

const IN_OPERATOR_RECEIVER_ERROR: &str = "right-hand side of operator 'in' is not an object";

impl Context {
    pub(super) fn eval_bytecode_in(
        &mut self,
        left: &Value,
        right: &Value,
        property_access: Option<BytecodeDynamicProperty>,
    ) -> Result<Value> {
        Self::require_in_operator_object(right)?;
        let property = self.dynamic_property_key(left)?;
        if let Some(access) = property_access {
            return self
                .has_cached_dynamic_property_value(right, &property, access.access())
                .map(Value::Bool);
        }
        self.has_dynamic_property_value(right, &property)
            .map(Value::Bool)
    }

    pub(super) fn eval_bytecode_in_static_property(
        &mut self,
        object: &Value,
        property: &StaticString,
        access: BytecodeDynamicProperty,
    ) -> Result<Value> {
        Self::require_in_operator_object(object)?;
        self.has_cached_property_name_value(object, property.as_str(), access.access())
            .map(Value::Bool)
    }

    fn require_in_operator_object(value: &Value) -> Result<()> {
        if matches!(
            value,
            Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
        ) {
            return Ok(());
        }
        Err(Error::type_error(IN_OPERATOR_RECEIVER_ERROR))
    }

    pub(super) fn has_own_array_index_for_in(
        &self,
        object: &Value,
        index: i32,
    ) -> Result<Option<bool>> {
        let Ok(index) = usize::try_from(index) else {
            return Ok(None);
        };
        let Value::Object(id) = object else {
            return Ok(None);
        };
        self.objects.has_own_array_index_if_array(*id, index)
    }
}
