use std::borrow::Cow;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{NumericValue, PreferredType, to_string_primitive},
    },
    syntax::StaticString,
    value::Value,
};

const STRING_CONCAT_INTERMEDIATE_EXTRA_CAPACITY: usize = 24;
const FOREIGN_VM_VALUE_ERROR: &str = "value belongs to another VM";
const BIGINT_BIT_LIMIT_ERROR: &str = "BigInt bit length exceeded the configured limit";

impl Context {
    pub(crate) fn static_string_value(&mut self, value: &StaticString) -> Result<Value> {
        self.heap_utf16_string_value(value.as_utf16())
    }

    pub(crate) fn runtime_value(&mut self, value: Value) -> Result<Value> {
        if let Value::String(text) = value {
            return self.heap_string_value(&text);
        }
        if let Value::HeapString(text) = &value
            && !text.is_heap_owned()
        {
            return self.heap_js_string_value(text);
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
            let value = self.concat_utf16_values(&left, &right)?;
            return self.heap_utf16_string_value(&value);
        }
        let left = self.to_numeric(&left)?;
        let right = self.to_numeric(&right)?;
        match (left, right) {
            (NumericValue::Number(left), NumericValue::Number(right)) => {
                Ok(Value::Number(left + right))
            }
            (NumericValue::BigInt(left), NumericValue::BigInt(right)) => {
                self.bigint_value(left.add(&right))
            }
            (NumericValue::Number(_), NumericValue::BigInt(_))
            | (NumericValue::BigInt(_), NumericValue::Number(_)) => {
                Err(Error::type_error("Cannot mix BigInt and other types"))
            }
        }
    }

    pub(crate) fn string_concat_step(
        &mut self,
        left: Value,
        right: &Value,
        final_result: bool,
    ) -> Result<Value> {
        if requires_generic_add(&left)
            || requires_generic_add(right)
            || !has_exact_utf8(&left)
            || !has_exact_utf8(right)
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
            Value::HeapString(text) if text.is_well_formed() => {
                let mut text = text
                    .into_utf8_accumulator()
                    .ok_or_else(|| Error::runtime("well-formed string lost its UTF-8 value"))?;
                self.push_primitive_string(&mut text, right)?;
                text
            }
            left => self.concat_values(&left, right)?,
        };

        if !final_result {
            self.reserve_string_concat_tail(&mut text)?;
        }
        Ok(Value::HeapString(text.into()))
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
        if !has_exact_utf8(&left) {
            return self.add(&left, &Value::HeapString(right.into()));
        }
        let mut text = match left {
            Value::String(mut text) => {
                self.push_concat_text(&mut text, right)?;
                text
            }
            Value::HeapString(text) if text.is_well_formed() => {
                let mut text = text
                    .into_utf8_accumulator()
                    .ok_or_else(|| Error::runtime("well-formed string lost its UTF-8 value"))?;
                self.push_concat_text(&mut text, right)?;
                text
            }
            left => self.concat_value_with_static(&left, right)?,
        };

        if !final_result {
            self.reserve_string_concat_tail(&mut text)?;
        }
        Ok(Value::HeapString(text.into()))
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

    fn concat_utf16_values(&self, left: &Value, right: &Value) -> Result<Vec<u16>> {
        let left = primitive_utf16_units(left)?;
        let right = primitive_utf16_units(right)?;
        let length = left
            .len()
            .checked_add(right.len())
            .ok_or_else(|| Error::limit("string length exceeded supported range"))?;
        if length > self.limits.max_string_len {
            return Err(Error::limit(format!(
                "string length {length} exceeded {}",
                self.limits.max_string_len
            )));
        }
        let mut output = Vec::new();
        output.try_reserve(length).map_err(|_| {
            Error::limit("string concatenation allocation exceeded supported range")
        })?;
        output.extend_from_slice(&left);
        output.extend_from_slice(&right);
        Ok(output)
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
            Value::BigInt(value) => value.to_string().len(),
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
                self.check_utf16_string_len(text.as_utf16())?;
                if text.identity() != Some(self.identity()) {
                    return Err(Error::runtime(FOREIGN_VM_VALUE_ERROR));
                }
                let Some(id) = text.id() else {
                    return Err(Error::runtime("string is not owned by a VM"));
                };
                self.strings.get(id)?;
            }
            Value::Symbol(symbol) => {
                if let Some(description) = symbol.description() {
                    self.check_string_len(description)?;
                }
                if symbol.identity() != self.identity() {
                    return Err(Error::runtime(FOREIGN_VM_VALUE_ERROR));
                }
                self.symbols.get(symbol.id())?;
            }
            Value::BigInt(value) => self.check_bigint_len(value)?,
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

    pub(in crate::runtime) fn bigint_value(&self, value: crate::value::JsBigInt) -> Result<Value> {
        self.check_bigint_len(&value)?;
        Ok(Value::BigInt(value))
    }

    fn check_bigint_len(&self, value: &crate::value::JsBigInt) -> Result<()> {
        let bits =
            usize::try_from(value.bit_len()).map_err(|_| Error::limit(BIGINT_BIT_LIMIT_ERROR))?;
        if bits > self.limits.max_bigint_bits {
            return Err(Error::limit(format!(
                "BigInt bit length {bits} exceeded {}",
                self.limits.max_bigint_bits
            )));
        }
        Ok(())
    }

    pub(crate) fn current_this(&mut self) -> Result<Value> {
        if let Some(binding) = self.current_activation_super()
            && binding.constructor.is_some()
        {
            let this_value = binding.this_value.borrow().clone();
            let Some(value) = this_value else {
                return Err(Error::exception(
                    crate::value::ErrorName::ReferenceError,
                    "this is not initialized before super()",
                ));
            };
            return self.checked_value(value);
        }
        if let Some(value) = self.current_activation_this() {
            return self.checked_value(value.clone());
        }
        if self.module_evaluation_depth > 0 {
            return Ok(Value::Undefined);
        }
        self.global_this_value()
    }

    pub(crate) fn current_new_target(&self) -> Result<Value> {
        self.checked_value(
            self.current_activation_new_target()
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

    pub(crate) fn check_utf16_string_len(&self, units: &[u16]) -> Result<()> {
        if units.len() > self.limits.max_string_len {
            return Err(Error::limit(format!(
                "string length {} exceeded {}",
                units.len(),
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
}

const fn requires_generic_add(value: &Value) -> bool {
    !crate::runtime::abstract_operations::is_primitive(value) || matches!(value, Value::Symbol(_))
}

fn has_exact_utf8(value: &Value) -> bool {
    !matches!(value, Value::HeapString(text) if !text.is_well_formed())
}

fn primitive_utf16_units(value: &Value) -> Result<Cow<'_, [u16]>> {
    match value {
        Value::String(text) => Ok(Cow::Owned(text.encode_utf16().collect())),
        Value::HeapString(text) => Ok(Cow::Borrowed(text.as_utf16())),
        value => to_string_primitive(value).map(|text| Cow::Owned(text.encode_utf16().collect())),
    }
}
