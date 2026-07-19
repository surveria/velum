#[cfg(not(feature = "std"))]
use crate::prelude::*;

use icu_casemap::CaseMapper;
use icu_locale::{LanguageIdentifier, Locale};

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
        let text = self.string_receiver_utf16(this_value)?;
        let requested = args.as_slice().first().unwrap_or(&Value::Undefined);
        let locales = self.intl_locale_list(requested)?;
        let locale_tag = locales.first().map_or("und", String::as_str);
        let locale = locale_tag
            .parse::<Locale>()
            .map_err(|error| Error::runtime(format!("invalid canonical locale: {error}")))?;
        self.string_case_map_utf16(&text, &locale.id, uppercase)
    }

    pub(in crate::runtime::native) fn string_case_map_utf16(
        &mut self,
        input: &[u16],
        locale: &LanguageIdentifier,
        uppercase: bool,
    ) -> Result<Value> {
        let mut output = Vec::with_capacity(input.len());
        let mut scalar_run = String::new();
        for decoded in char::decode_utf16(input.iter().copied()) {
            match decoded {
                Ok(character) => scalar_run.push(character),
                Err(error) => {
                    append_case_mapped_run(&mut output, &scalar_run, locale, uppercase);
                    scalar_run.clear();
                    output.push(error.unpaired_surrogate());
                }
            }
        }
        append_case_mapped_run(&mut output, &scalar_run, locale, uppercase);
        self.check_utf16_string_len(&output)?;
        self.heap_utf16_string_value(&output)
    }
}

fn append_case_mapped_run(
    output: &mut Vec<u16>,
    input: &str,
    locale: &LanguageIdentifier,
    uppercase: bool,
) {
    if input.is_empty() {
        return;
    }
    let case_mapper = CaseMapper::new();
    let transformed = if uppercase {
        case_mapper.uppercase_to_string(input, locale)
    } else {
        case_mapper.lowercase_to_string(input, locale)
    };
    output.extend(transformed.encode_utf16());
}
