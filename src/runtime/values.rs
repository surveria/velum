use std::fmt::Write as _;

use crate::{
    error::{Error, Result},
    runtime::Context,
    syntax::StaticString,
    value::Value,
};

impl Context {
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
                let value = self.concat_values(left, right)?;
                self.heap_string_owned_value(value)
            }
            _ => Err(Error::runtime("operator '+' expects numbers or strings")),
        }
    }

    fn concat_values(&self, left: &Value, right: &Value) -> Result<String> {
        let capacity = self.concat_capacity(left, right)?;
        let mut text = String::with_capacity(capacity);
        self.push_display_for_concat(&mut text, left)?;
        self.push_display_for_concat(&mut text, right)?;
        Ok(text)
    }

    fn concat_capacity(&self, left: &Value, right: &Value) -> Result<usize> {
        let left = Self::concat_capacity_hint(left);
        let right = Self::concat_capacity_hint(right);
        let capacity = left
            .checked_add(right)
            .ok_or_else(|| Error::limit("string length exceeded supported range"))?;
        Ok(capacity.min(self.limits.max_string_len))
    }

    fn concat_capacity_hint(value: &Value) -> usize {
        match value {
            Value::String(value) => value.len(),
            Value::HeapString(value) => value.as_str().len(),
            Value::Undefined => "undefined".len(),
            Value::Null => "null".len(),
            Value::Bool(value) => {
                if *value {
                    "true".len()
                } else {
                    "false".len()
                }
            }
            Value::Number(_) => 24,
            Value::NativeFunction(_) | Value::HostFunction(_) => "function()".len(),
            Value::Object(_) => "[object Object]".len(),
            Value::Function(_) | Value::Symbol(_) | Value::Error(_) => 0,
        }
    }

    fn push_display_for_concat(&self, text: &mut String, value: &Value) -> Result<()> {
        match value {
            Value::Function(id) => {
                let source = self.function_source_text(*id)?;
                self.push_concat_text(text, &source)
            }
            Value::String(value) => self.push_concat_text(text, value),
            Value::HeapString(value) => self.push_concat_text(text, value.as_str()),
            Value::Symbol(value) => self.push_concat_text(text, &value.display_name()),
            Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::Object(_)
            | Value::Error(_) => self.write_concat_display(text, value),
        }
    }

    fn push_concat_text(&self, text: &mut String, value: &str) -> Result<()> {
        let length = text
            .len()
            .checked_add(value.len())
            .ok_or_else(|| Error::limit("string length exceeded supported range"))?;
        if length > self.limits.max_string_len {
            return Err(Error::limit(format!(
                "string length {length} exceeded {}",
                self.limits.max_string_len
            )));
        }
        text.push_str(value);
        Ok(())
    }

    fn write_concat_display(&self, text: &mut String, value: &Value) -> Result<()> {
        text.write_fmt(format_args!("{value}")).map_err(|error| {
            Error::runtime(format!("failed to format concatenated value: {error}"))
        })?;
        if text.len() > self.limits.max_string_len {
            return Err(Error::limit(format!(
                "string length {} exceeded {}",
                text.len(),
                self.limits.max_string_len
            )));
        }
        Ok(())
    }

    pub(crate) fn template_concat_text(&self, parts: &[Value]) -> Result<String> {
        let mut text = String::new();
        for value in parts {
            // Template substitutions use ToString semantics, which reject
            // symbol values instead of stringifying their description.
            if matches!(value, Value::Symbol(_)) {
                return Err(Error::type_error(
                    "cannot convert a Symbol value to a string",
                ));
            }
            self.push_display_for_concat(&mut text, value)?;
        }
        Ok(text)
    }

    pub(crate) fn checked_value(&self, value: Value) -> Result<Value> {
        match &value {
            Value::String(text) => self.check_string_len(text)?,
            Value::HeapString(text) => {
                self.check_string_len(text.as_str())?;
                self.strings.get(text.id())?;
            }
            Value::Symbol(symbol) => {
                if let Some(description) = symbol.description() {
                    self.check_string_len(description)?;
                }
                self.symbols.get(symbol.id())?;
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

    pub(crate) fn current_new_target(&self) -> Result<Value> {
        self.checked_value(
            self.new_target_values
                .last()
                .cloned()
                .unwrap_or(Value::Undefined),
        )
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

    pub(crate) fn record_bytecode_linear_segment_run(&mut self) -> Result<()> {
        self.bytecode_linear_segment_runs = self
            .bytecode_linear_segment_runs
            .checked_add(1)
            .ok_or_else(|| Error::limit("bytecode linear segment runs overflowed"))?;
        Ok(())
    }

    pub(crate) fn record_bytecode_linear_direct_run(&mut self) -> Result<()> {
        self.bytecode_linear_direct_runs = self
            .bytecode_linear_direct_runs
            .checked_add(1)
            .ok_or_else(|| Error::limit("bytecode linear direct runs overflowed"))?;
        Ok(())
    }
}
