use crate::{
    ast::{Expr, UnaryOp},
    error::{Error, Result},
    runtime::Context,
    value::Value,
};

impl Context {
    pub(crate) fn eval_unary_expr(&mut self, op: UnaryOp, expr: &Expr) -> Result<Value> {
        match op {
            UnaryOp::Not | UnaryOp::Negate | UnaryOp::Plus => {
                let value = self.eval_expr(expr)?;
                Self::eval_numeric_unary(op, &value)
            }
            UnaryOp::Typeof => self.eval_typeof(expr),
            UnaryOp::Void => {
                self.eval_expr(expr)?;
                Ok(Value::Undefined)
            }
            UnaryOp::Delete => self.eval_delete(expr),
        }
    }

    fn eval_numeric_unary(op: UnaryOp, value: &Value) -> Result<Value> {
        match op {
            UnaryOp::Not => Ok(Value::Bool(!value.is_truthy())),
            UnaryOp::Negate => value
                .as_number()
                .map(|value| Value::Number(-value))
                .ok_or_else(|| Error::runtime("unary '-' expects a number")),
            UnaryOp::Plus => value
                .as_number()
                .map(Value::Number)
                .ok_or_else(|| Error::runtime("unary '+' expects a number")),
            UnaryOp::Typeof | UnaryOp::Void | UnaryOp::Delete => Err(Error::runtime(
                "non-numeric unary operator reached numeric path",
            )),
        }
    }

    fn eval_typeof(&mut self, expr: &Expr) -> Result<Value> {
        if let Expr::Identifier(name) = expr {
            let Some(binding) = self.get_binding(name) else {
                return Ok(Value::String(Value::Undefined.type_name().to_owned()));
            };
            return Ok(Value::String(binding.value().type_name().to_owned()));
        }

        let value = self.eval_expr(expr)?;
        Ok(Value::String(value.type_name().to_owned()))
    }

    fn eval_delete(&mut self, expr: &Expr) -> Result<Value> {
        match expr {
            Expr::Identifier(name) => Ok(Value::Bool(self.get_binding(name).is_none())),
            Expr::Member { object, property } => {
                let object = self.eval_expr(object)?;
                self.delete_property_value(&object, property)
            }
            Expr::ComputedMember { object, property } => {
                let object = self.eval_expr(object)?;
                let property = self.eval_property_key(property)?;
                self.delete_property_value(&object, &property)
            }
            expr => {
                self.eval_expr(expr)?;
                Ok(Value::Bool(true))
            }
        }
    }
}
