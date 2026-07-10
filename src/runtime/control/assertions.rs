use crate::{
    error::{Error, Result},
    runtime::Context,
    value::{ErrorName, Value},
};

const FOREIGN_JAVASCRIPT_VALUE_ERROR: &str = "JavaScript thrown value belongs to another VM";

pub fn thrown_value_matches(context: &Context, value: &Value, expected_name: &str) -> Result<bool> {
    let Some(expected) = ErrorName::from_constructor_name(expected_name) else {
        return Ok(false);
    };
    let Value::Object(id) = value else {
        return Ok(false);
    };
    let Some(metadata) = context.objects.error_metadata(*id)? else {
        return Ok(false);
    };
    if expected == ErrorName::Base {
        return Ok(metadata.error_name().is_standard());
    }
    Ok(metadata.error_name() == expected)
}

pub fn runtime_exception_value(context: &mut Context, error: &Error) -> Result<Option<Value>> {
    if let Some(value) = error.javascript_value() {
        if let Some(identity) = error.javascript_identity()
            && identity != context.identity()
        {
            return Err(Error::runtime(FOREIGN_JAVASCRIPT_VALUE_ERROR));
        }
        return Ok(Some(value.clone()));
    }
    let Some(metadata) = error.javascript_error_request() else {
        return Ok(None);
    };
    context
        .create_error_object(metadata.clone(), true)
        .map(Some)
}

pub fn reference_error_undefined(name: &str) -> Error {
    Error::exception(
        ErrorName::ReferenceError,
        format!("'{name}' is not defined"),
    )
}

pub fn reference_error_uninitialized(name: &str) -> Error {
    Error::exception(
        ErrorName::ReferenceError,
        format!("'{name}' is not initialized"),
    )
}
