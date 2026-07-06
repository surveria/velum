use crate::{
    ast::{BinaryOp, Expr, StaticBinding, StaticName},
    error::{Error, Result},
    runtime::Context,
    runtime_numeric::{
        bitwise_and, bitwise_or, bitwise_xor, numeric_binary, shift_left, shift_right,
        shift_right_unsigned,
    },
    value::Value,
};

impl Context {
    pub(crate) fn eval_property_assignment(
        &mut self,
        object: &Expr,
        property: &StaticName,
        expr: &Expr,
    ) -> Result<Value> {
        let object = self.eval_expr(object)?;
        let value = self.eval_expr(expr)?;
        self.set_static_property_value(&object, property, value.clone())?;
        Ok(value)
    }

    pub(crate) fn eval_computed_property_assignment(
        &mut self,
        object: &Expr,
        property: &Expr,
        expr: &Expr,
    ) -> Result<Value> {
        let object = self.eval_expr(object)?;
        let property = self.eval_property_key(property)?;
        let value = self.eval_expr(expr)?;
        self.set_property_value(&object, &property, value.clone())?;
        Ok(value)
    }

    pub(crate) fn eval_compound_assignment(
        &mut self,
        op: BinaryOp,
        target: &Expr,
        expr: &Expr,
    ) -> Result<Value> {
        match target {
            Expr::Identifier(name) => self.eval_binding_compound_assignment(op, name, expr),
            Expr::Member { object, property } => {
                let object = self.eval_expr(object)?;
                self.eval_property_compound_assignment(op, &object, property, expr)
            }
            Expr::ComputedMember { object, property } => {
                let object = self.eval_expr(object)?;
                let property = self.eval_property_key(property)?;
                self.eval_dynamic_property_compound_assignment(op, &object, &property, expr)
            }
            _ => Err(Error::runtime("invalid compound assignment target")),
        }
    }

    fn eval_binding_compound_assignment(
        &mut self,
        op: BinaryOp,
        name: &StaticBinding,
        expr: &Expr,
    ) -> Result<Value> {
        self.materialize_builtin_binding(name)?;
        let old_value = self
            .get_binding_static(name)?
            .map(|binding| binding.value())
            .ok_or_else(|| Error::runtime(format!("ReferenceError: '{name}' is not defined")))?;
        let right = self.eval_expr(expr)?;
        let value = self.eval_compound_value(op, &old_value, &right)?;
        self.assign_static(name, value.clone())?;
        Ok(value)
    }

    fn eval_property_compound_assignment(
        &mut self,
        op: BinaryOp,
        object: &Value,
        property: &StaticName,
        expr: &Expr,
    ) -> Result<Value> {
        let old_value = self.get_static_property_value(object, property)?;
        let right = self.eval_expr(expr)?;
        let value = self.eval_compound_value(op, &old_value, &right)?;
        self.set_static_property_value(object, property, value.clone())?;
        Ok(value)
    }

    fn eval_dynamic_property_compound_assignment(
        &mut self,
        op: BinaryOp,
        object: &Value,
        property: &str,
        expr: &Expr,
    ) -> Result<Value> {
        let old_value = self.get_property_value(object, property)?;
        let right = self.eval_expr(expr)?;
        let value = self.eval_compound_value(op, &old_value, &right)?;
        self.set_property_value(object, property, value.clone())?;
        Ok(value)
    }

    fn eval_compound_value(&self, op: BinaryOp, left: &Value, right: &Value) -> Result<Value> {
        let value = match op {
            BinaryOp::Add => self.add(left, right)?,
            BinaryOp::Sub => numeric_binary(left, right, "-=", |left, right| left - right)?,
            BinaryOp::Mul => numeric_binary(left, right, "*=", |left, right| left * right)?,
            BinaryOp::Div => numeric_binary(left, right, "/=", |left, right| left / right)?,
            BinaryOp::Rem => numeric_binary(left, right, "%=", |left, right| left % right)?,
            BinaryOp::Pow => numeric_binary(left, right, "**=", f64::powf)?,
            BinaryOp::BitAnd => bitwise_and(left, right)?,
            BinaryOp::BitOr => bitwise_or(left, right)?,
            BinaryOp::BitXor => bitwise_xor(left, right)?,
            BinaryOp::ShiftLeft => shift_left(left, right)?,
            BinaryOp::ShiftRight => shift_right(left, right)?,
            BinaryOp::ShiftRightUnsigned => shift_right_unsigned(left, right)?,
            BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::StrictEqual
            | BinaryOp::StrictNotEqual
            | BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual
            | BinaryOp::In
            | BinaryOp::LogicalAnd
            | BinaryOp::LogicalOr => {
                return Err(Error::runtime("invalid compound assignment operator"));
            }
        };
        self.checked_value(value)
    }
}
