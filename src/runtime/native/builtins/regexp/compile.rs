use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, object::RegExpValue},
    value::Value,
};

use super::{
    REGEXP_RECEIVER_ERROR, compile_regexp_pattern_utf16, parse_regexp_flags, value_is_undefined,
};

impl Context {
    pub(in crate::runtime::native) fn eval_regexp_prototype_compile(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let Value::Object(receiver) = this_value else {
            return Err(Error::type_error(REGEXP_RECEIVER_ERROR));
        };
        if self.objects.regexp_value(*receiver)?.is_none()
            || self.objects.prototype_value(*receiver)?
                != Value::Object(self.regexp_constructor_prototype()?)
        {
            return Err(Error::type_error(REGEXP_RECEIVER_ERROR));
        }
        let pattern_value = args.as_slice().first();
        let flags_value = args.as_slice().get(1);
        let replacement = if let Some(Value::Object(pattern_id)) = pattern_value
            && let Some(regexp) = self.objects.regexp_value(*pattern_id)?.cloned()
        {
            if flags_value.is_some_and(|value| !value_is_undefined(value)) {
                return Err(Error::type_error(
                    "RegExp.prototype.compile flags must be undefined for a RegExp pattern",
                ));
            }
            regexp
        } else {
            let pattern = match pattern_value {
                None | Some(Value::Undefined) => Vec::new(),
                Some(value) => self.to_utf16_string(value)?,
            };
            let flags = match flags_value {
                None | Some(Value::Undefined) => String::new(),
                Some(value) => self.to_string(value)?,
            };
            self.charge_regexp_utf16_work(&pattern, &[])?;
            self.check_utf16_string_len(&pattern)?;
            self.check_string_len(&flags)?;
            let parsed_flags = parse_regexp_flags(&flags)?;
            let compiled = compile_regexp_pattern_utf16(&pattern, parsed_flags)?;
            RegExpValue::new_utf16(pattern, parsed_flags, compiled)?
        };
        self.objects.replace_regexp_value(*receiver, replacement)?;
        self.set_regexp_last_index(this_value, 0)?;
        Ok(this_value.clone())
    }
}
