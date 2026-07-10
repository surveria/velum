use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{PreferredType, to_string_primitive},
    },
    syntax::StaticString,
    value::Value,
};

const STRING_CONCAT_INTERMEDIATE_EXTRA_CAPACITY: usize = 24;

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
        if let (Value::Number(left), Value::Number(right)) = (left, right) {
            return Ok(Value::Number(left + right));
        }
        let left = self.to_primitive(
            left,
            crate::runtime::abstract_operations::PreferredType::Default,
        )?;
        let right = self.to_primitive(
            right,
            crate::runtime::abstract_operations::PreferredType::Default,
        )?;
        if matches!(left, Value::String(_) | Value::HeapString(_))
            || matches!(right, Value::String(_) | Value::HeapString(_))
        {
            let value = self.concat_values(&left, &right)?;
            return self.heap_string_owned_value(value);
        }
        let left = self.to_number(&left)?;
        let right = self.to_number(&right)?;
        Ok(Value::Number(left + right))
    }

    pub(crate) fn string_concat_step(
        &mut self,
        left: Value,
        right: &Value,
        final_result: bool,
    ) -> Result<Value> {
        if requires_generic_add(&left)
            || requires_generic_add(right)
            || (!matches!(left, Value::String(_) | Value::HeapString(_))
                && !matches!(right, Value::String(_) | Value::HeapString(_)))
        {
            return self.add(&left, right);
        }

        let mut text = match left {
            Value::String(mut text) => {
                self.push_primitive_string(&mut text, right)?;
                text
            }
            left => self.concat_values(&left, right)?,
        };

        if !final_result {
            self.reserve_string_concat_tail(&mut text)?;
        }
        self.checked_value(Value::String(text))
    }

    pub(crate) fn string_concat_static_step(
        &mut self,
        left: Value,
        right: &str,
        final_result: bool,
    ) -> Result<Value> {
        let left = if requires_generic_add(&left) {
            self.to_primitive(&left, PreferredType::Default)?
        } else {
            left
        };
        let mut text = match left {
            Value::String(mut text) => {
                self.push_concat_text(&mut text, right)?;
                text
            }
            left => self.concat_value_with_static(&left, right)?,
        };

        if !final_result {
            self.reserve_string_concat_tail(&mut text)?;
        }
        self.checked_value(Value::String(text))
    }

    fn reserve_string_concat_tail(&self, text: &mut String) -> Result<()> {
        let target = text
            .len()
            .checked_add(STRING_CONCAT_INTERMEDIATE_EXTRA_CAPACITY)
            .ok_or_else(|| Error::limit("string length exceeded supported range"))?
            .min(self.limits.max_string_len);
        let additional = target.saturating_sub(text.capacity());
        if additional > 0 {
            text.try_reserve(additional)
                .map_err(|_| Error::limit("string length exceeded supported range"))?;
        }
        Ok(())
    }

    fn concat_values(&self, left: &Value, right: &Value) -> Result<String> {
        let capacity = self.concat_capacity(left, right)?;
        let mut text = String::with_capacity(capacity);
        self.push_primitive_string(&mut text, left)?;
        self.push_primitive_string(&mut text, right)?;
        Ok(text)
    }

    fn concat_value_with_static(&self, left: &Value, right: &str) -> Result<String> {
        let capacity = Self::concat_capacity_hint(left)
            .checked_add(right.len())
            .ok_or_else(|| Error::limit("string length exceeded supported range"))?
            .min(self.limits.max_string_len);
        let mut text = String::with_capacity(capacity);
        self.push_primitive_string(&mut text, left)?;
        self.push_concat_text(&mut text, right)?;
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
            Value::Function(_) | Value::Symbol(_) => 0,
        }
    }

    fn push_primitive_string(&self, text: &mut String, value: &Value) -> Result<()> {
        let value = to_string_primitive(value)?;
        self.push_concat_text(text, &value)
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

    pub(crate) fn template_concat_text(&mut self, parts: &[Value]) -> Result<String> {
        let mut text = String::new();
        for value in parts {
            let value = self.to_string(value)?;
            self.push_concat_text(&mut text, &value)?;
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

    pub(crate) fn current_this(&mut self) -> Result<Value> {
        if let Some(value) = self.this_values.last() {
            return self.checked_value(value.clone());
        }
        self.global_this_value()
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

    pub(crate) fn charge_runtime_steps(&mut self, steps: usize) -> Result<()> {
        self.runtime_steps = self
            .runtime_steps
            .checked_add(steps)
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

    pub(crate) fn record_bytecode_linear_direct_runs(&mut self, runs: usize) -> Result<()> {
        self.bytecode_linear_direct_runs = self
            .bytecode_linear_direct_runs
            .checked_add(runs)
            .ok_or_else(|| Error::limit("bytecode linear direct runs overflowed"))?;
        Ok(())
    }
}

const fn requires_generic_add(value: &Value) -> bool {
    !crate::runtime::abstract_operations::is_primitive(value) || matches!(value, Value::Symbol(_))
}
