use crate::{
    bytecode::{BytecodeBinding, BytecodeNumericBinaryOp},
    error::{Error, Result},
    runtime::{Context, binding::scope::BindingCell, numeric::number_to_uint32},
    syntax::{BinaryOp, StaticName, StaticPropertyAccessId},
    value::Value,
};

use super::numeric_chain::apply_number_binary;

impl Context {
    pub(super) fn eval_static_numeric_property_compound(
        &mut self,
        op: BinaryOp,
        object: &Value,
        property: &StaticName,
        access: StaticPropertyAccessId,
        rhs: &Value,
    ) -> Result<Value> {
        if let Some((_, value)) = self.try_cached_static_property_read_modify_write(
            object,
            property,
            access,
            |context, old_value| {
                context
                    .eval_numeric_compound_value(op, old_value, rhs)
                    .map(|new_value| (old_value.clone(), new_value))
            },
        )? {
            return Ok(value);
        }
        let old_value = self.get_static_property_value(object, property, access)?;
        let value = self.eval_numeric_compound_value(op, &old_value, rhs)?;
        self.set_static_property_value(object, property, access, value.clone())?;
        Ok(value)
    }

    pub(super) fn eval_dynamic_array_numeric_compound(
        &mut self,
        op: BinaryOp,
        object: &Value,
        index: usize,
        rhs: &Value,
    ) -> Result<Option<Value>> {
        let Some(op) = BytecodeNumericBinaryOp::from_binary(op) else {
            return Ok(None);
        };
        let (Value::Object(id), Value::Number(right)) = (object, rhs) else {
            return Ok(None);
        };
        let Some(old_value) = self.objects.array_index_value_if_array(*id, index)? else {
            return Ok(None);
        };
        let old_value = self.runtime_value(old_value)?;
        let Value::Number(left) = old_value else {
            return Ok(None);
        };
        let value = self.checked_value(Value::Number(apply_number_binary(op, left, *right)?))?;
        if !self.objects.set_array_index_if_array(
            *id,
            index,
            value.clone(),
            self.limits.max_object_properties,
        )? {
            return Ok(None);
        }
        Ok(Some(value))
    }

    pub(super) fn eval_property_index_usize(
        &mut self,
        index: &BytecodeBinding,
        index_cell: &BindingCell,
        index_mask: Option<f64>,
    ) -> Result<Option<usize>> {
        let value = self.runtime_value(index_cell.value(index.name())?)?;
        let Value::Number(mut value) = value else {
            return Ok(None);
        };
        if let Some(mask) = index_mask {
            value = apply_number_binary(BytecodeNumericBinaryOp::BitAnd, value, mask)?;
        }
        array_index_from_number(value)
    }

    fn eval_numeric_compound_value(
        &mut self,
        op: BinaryOp,
        left: &Value,
        right: &Value,
    ) -> Result<Value> {
        if let (Value::Number(left), Value::Number(right)) = (left, right)
            && let Some(op) = BytecodeNumericBinaryOp::from_binary(op)
        {
            return apply_number_binary(op, *left, *right).map(Value::Number);
        }
        self.eval_bytecode_compound_value(op, left, right)
    }
}

fn array_index_from_number(value: f64) -> Result<Option<usize>> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value >= f64::from(u32::MAX) {
        return Ok(None);
    }
    usize::try_from(number_to_uint32(value, "array index")?)
        .map(Some)
        .map_err(|_| Error::limit("array index exceeded supported range"))
}
