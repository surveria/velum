use icu_casemap::CaseMapper;
use icu_locale::Locale;

use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs},
    value::Value,
};

impl Context {
    pub(in crate::runtime::native) fn eval_string_prototype_to_locale_lower_case(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_string_prototype_locale_case(args, this_value, false)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_to_locale_upper_case(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_string_prototype_locale_case(args, this_value, true)
    }

    fn eval_string_prototype_locale_case(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        uppercase: bool,
    ) -> Result<Value> {
        let text = self.string_receiver_value(this_value)?;
        let requested = args.as_slice().first().unwrap_or(&Value::Undefined);
        let locales = self.intl_locale_list(requested)?;
        let locale_tag = locales.first().map_or("und", String::as_str);
        let locale = locale_tag
            .parse::<Locale>()
            .map_err(|error| Error::runtime(format!("invalid canonical locale: {error}")))?;
        let mapper = CaseMapper::new();
        let result = if uppercase {
            mapper.uppercase_to_string(&text, &locale.id)
        } else {
            mapper.lowercase_to_string(&text, &locale.id)
        };
        self.check_string_len(&result)?;
        self.heap_string_value(&result)
    }
}
