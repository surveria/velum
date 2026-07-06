use crate::{
    ast::StaticString,
    error::{Error, Result},
    runtime::Context,
    value::Value,
};

impl Context {
    pub(crate) fn literal_value(&mut self, value: &Value) -> Result<Value> {
        self.runtime_value(value.clone())
    }

    pub(crate) fn static_string_value(&mut self, value: &StaticString) -> Result<Value> {
        self.heap_string_value(value.as_str())
    }

    pub(crate) fn runtime_value(&mut self, value: Value) -> Result<Value> {
        if let Value::String(text) = value {
            return self.heap_string_value(&text);
        }
        self.checked_value(value)
    }

    pub(crate) fn add(&mut self, left: &Value, right: &Value) -> Result<Value> {
        match (left, right) {
            (Value::Number(left), Value::Number(right)) => Ok(Value::Number(left + right)),
            (Value::String(_) | Value::HeapString(_), _)
            | (_, Value::String(_) | Value::HeapString(_)) => {
                let value = left.display_for_concat() + &right.display_for_concat();
                self.heap_string_value(&value)
            }
            _ => Err(Error::runtime("operator '+' expects numbers or strings")),
        }
    }

    pub(crate) fn checked_value(&self, value: Value) -> Result<Value> {
        match &value {
            Value::String(text) => self.check_string_len(text)?,
            Value::HeapString(text) => {
                self.check_string_len(text.as_str())?;
                self.strings.get(text.id())?;
            }
            Value::Error(error) => self.check_string_len(error.message())?,
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Object(_) => {}
        }
        Ok(value)
    }

    pub(crate) fn current_this(&self) -> Result<Value> {
        self.checked_value(self.this_values.last().cloned().unwrap_or(Value::Undefined))
    }

    pub(crate) fn check_string_len(&self, text: &str) -> Result<()> {
        if text.len() > self.limits.max_string_len {
            return Err(Error::limit(format!(
                "string length {} exceeded {}",
                text.len(),
                self.limits.max_string_len
            )));
        }
        Ok(())
    }

    pub(crate) fn step(&mut self) -> Result<()> {
        self.runtime_steps = self
            .runtime_steps
            .checked_add(1)
            .ok_or_else(|| Error::limit("runtime steps overflowed"))?;
        if self.runtime_steps > self.limits.max_runtime_steps {
            return Err(Error::limit(format!(
                "runtime steps exceeded {}",
                self.limits.max_runtime_steps
            )));
        }
        Ok(())
    }
}
