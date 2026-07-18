use unicode_normalization::UnicodeNormalization;

use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, roots::VmRootKind},
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
        let collator_args = args.as_slice().get(1..).unwrap_or_default();
        let collator = self.construct_intl_collator(RuntimeCallArgs::values(collator_args))?;
        let Value::Object(collator_id) = &collator else {
            return Err(Error::runtime(
                "Intl.Collator construction returned a non-object",
            ));
        };
        let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        roots.add_values(core::iter::once(&collator))?;
        let left = self.heap_string_value(&left)?;
        let right = self.heap_string_value(&right)?;
        roots.add_values([&left, &right])?;
        self.eval_intl_collator_compare(RuntimeCallArgs::values(&[left, right]), *collator_id)
    }
}
