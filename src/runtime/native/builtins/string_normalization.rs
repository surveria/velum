use std::cmp::Ordering;

use unicode_normalization::UnicodeNormalization;

use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs},
    value::{ErrorName, Value},
};

const DEFAULT_NORMALIZATION_FORM: &str = "NFC";
const INVALID_NORMALIZATION_FORM_ERROR: &str = "normalization form is invalid";

impl Context {
    pub(in crate::runtime::native) fn eval_string_prototype_normalize(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_value(this_value)?;
        let form = match args.as_slice().first() {
            None | Some(Value::Undefined) => DEFAULT_NORMALIZATION_FORM.to_owned(),
            Some(value) => self.to_string(value)?,
        };
        let normalized = match form.as_str() {
            "NFC" => text.nfc().collect::<String>(),
            "NFD" => text.nfd().collect::<String>(),
            "NFKC" => text.nfkc().collect::<String>(),
            "NFKD" => text.nfkd().collect::<String>(),
            _ => {
                return Err(Error::exception(
                    ErrorName::RangeError,
                    INVALID_NORMALIZATION_FORM_ERROR,
                ));
            }
        };
        self.check_string_len(&normalized)?;
        self.heap_string_value(&normalized)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_locale_compare(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let left = self.string_receiver_value(this_value)?;
        let right = match args.as_slice().first() {
            Some(value) => self.to_string(value)?,
            None => self.to_string(&Value::Undefined)?,
        };
        let left = left.nfc().collect::<String>();
        let right = right.nfc().collect::<String>();
        let result = match left.cmp(&right) {
            Ordering::Less => -1.0,
            Ordering::Equal => 0.0,
            Ordering::Greater => 1.0,
        };
        Ok(Value::Number(result))
    }
}
