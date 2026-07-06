use crate::{
    ast::{Expr, StaticBinding, StaticName, StaticPropertyAccessId, UpdateOp},
    error::{Error, Result},
    runtime::Context,
    runtime_property::DynamicPropertyKey,
    value::Value,
};

impl Context {
    pub(crate) fn eval_update_expr(
        &mut self,
        op: UpdateOp,
        prefix: bool,
        expr: &Expr,
    ) -> Result<Value> {
        match expr {
            Expr::Identifier(name) => self.update_binding(name, op, prefix),
            Expr::Member {
                object,
                property,
                access,
            } => {
                let object = self.eval_expr(object)?;
                self.update_property(&object, property, *access, op, prefix)
            }
            Expr::ComputedMember {
                object,
                property,
                access,
            } => {
                let object = self.eval_expr(object)?;
                let property = self.eval_property_key(property)?;
                self.update_dynamic_property(&object, property, *access, op, prefix)
            }
            _ => Err(Error::runtime("invalid update target")),
        }
    }

    fn update_binding(&self, name: &StaticBinding, op: UpdateOp, prefix: bool) -> Result<Value> {
        let binding = self
            .get_binding_static(name)?
            .ok_or_else(|| Error::runtime(format!("ReferenceError: '{name}' is not defined")))?;
        let old_value = binding.value();
        let new_value = Self::updated_number(&old_value, op)?;
        self.checked_value(new_value.clone())?;
        binding.assign(name, new_value.clone())?;
        Ok(if prefix { new_value } else { old_value })
    }

    fn update_property(
        &mut self,
        object: &Value,
        property: &StaticName,
        access: StaticPropertyAccessId,
        op: UpdateOp,
        prefix: bool,
    ) -> Result<Value> {
        let old_value = self.get_static_property_value(object, property, access)?;
        let new_value = Self::updated_number(&old_value, op)?;
        self.set_static_property_value(object, property, access, new_value.clone())?;
        Ok(if prefix { new_value } else { old_value })
    }

    fn update_dynamic_property(
        &mut self,
        object: &Value,
        mut property: DynamicPropertyKey,
        access: StaticPropertyAccessId,
        op: UpdateOp,
        prefix: bool,
    ) -> Result<Value> {
        let old_value = self.get_cached_dynamic_property_value(object, &property, access)?;
        let new_value = Self::updated_number(&old_value, op)?;
        self.set_cached_dynamic_property_value(object, &mut property, access, new_value.clone())?;
        Ok(if prefix { new_value } else { old_value })
    }

    fn updated_number(value: &Value, op: UpdateOp) -> Result<Value> {
        let Some(number) = value.as_number() else {
            return Err(Error::runtime("update operator expects a number"));
        };
        let updated = match op {
            UpdateOp::Increment => number + 1.0,
            UpdateOp::Decrement => number - 1.0,
        };
        Ok(Value::Number(updated))
    }
}
