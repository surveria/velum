use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs},
    value::Value,
};

use super::ARRAY_JOIN_DEFAULT_SEPARATOR;

const ARRAY_TO_LOCALE_STRING_PROPERTY: &str = "toLocaleString";

impl Context {
    pub(in crate::runtime::native) fn eval_array_to_locale_string(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let length = self.array_like_length(this_value)?;
        self.eval_array_to_locale_string_with_length(args.as_slice(), this_value, length)
    }

    pub(in crate::runtime::native) fn eval_array_to_locale_string_with_length(
        &mut self,
        args: &[Value],
        this_value: &Value,
        length: usize,
    ) -> Result<Value> {
        let locale_args = [
            args.first().cloned().unwrap_or(Value::Undefined),
            args.get(1).cloned().unwrap_or(Value::Undefined),
        ];
        let mut joined =
            self.join_string_with_separator_capacity(length, ARRAY_JOIN_DEFAULT_SEPARATOR.len())?;
        for index in 0..length {
            self.step()?;
            if index > 0 {
                self.push_join_text(&mut joined, ARRAY_JOIN_DEFAULT_SEPARATOR)?;
            }
            let value = self.get_array_like_index(this_value, index)?;
            if matches!(value, Value::Undefined | Value::Null) {
                continue;
            }
            let method = self
                .get_named_method(&value, ARRAY_TO_LOCALE_STRING_PROPERTY)?
                .ok_or_else(|| Error::type_error("element toLocaleString method is missing"))?;
            let localized = self.call_value(&method, &locale_args, value)?;
            let text = self.to_string(&localized)?;
            self.push_join_text(&mut joined, &text)?;
        }
        self.heap_string_value(&joined)
    }
}
