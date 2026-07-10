use crate::{
    error::Error,
    value::{ErrorName, ErrorObject, Value},
};

const ERROR_NAME_PROPERTY: &str = "name";
const ERROR_MESSAGE_PROPERTY: &str = "message";

pub fn thrown_value_matches(value: &Value, expected_name: &str) -> bool {
    let Some(expected) = ErrorName::from_constructor_name(expected_name) else {
        return false;
    };
    let Value::Error(error) = value else {
        return false;
    };
    if expected == ErrorName::Base {
        return error.name().is_standard();
    }
    error.name() == expected
}

pub fn error_property_text<'a>(error: &'a ErrorObject, property: &str) -> Option<&'a str> {
    match property {
        ERROR_NAME_PROPERTY => Some(error.name().as_str()),
        ERROR_MESSAGE_PROPERTY => Some(error.message()),
        _ => None,
    }
}

pub fn runtime_exception_value(error: &Error) -> Option<Value> {
    match error {
        Error::JavaScript { value } => Some(value.clone()),
        Error::Lex { .. }
        | Error::Parse { .. }
        | Error::Runtime { .. }
        | Error::ResourceLimit { .. } => None,
    }
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
