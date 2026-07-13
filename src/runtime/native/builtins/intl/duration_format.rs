use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, native::IntlFunctionKind, object::IntlValue},
    value::Value,
};

impl Context {
    pub(super) fn construct_intl_duration_format(&mut self) -> Result<Value> {
        let prototype =
            self.intl_constructor_prototype(IntlFunctionKind::DurationFormatConstructor)?;
        self.objects
            .create_intl_object(IntlValue::Duration, prototype, self.limits.max_objects)
    }

    pub(super) fn eval_intl_duration_format(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let Value::Object(formatter_id) = this_value else {
            return Err(Error::type_error("Intl.DurationFormat receiver is invalid"));
        };
        if !matches!(
            self.objects.intl_value(*formatter_id)?,
            Some(IntlValue::Duration)
        ) {
            return Err(Error::type_error("Intl.DurationFormat receiver is invalid"));
        }
        let duration = self.duration_from_value(args.as_slice().first())?;
        self.heap_string_value(&duration.to_string())
    }
}
