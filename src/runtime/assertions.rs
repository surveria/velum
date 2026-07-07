use crate::{
    error::Error,
    value::{ErrorName, ErrorObject, Value},
};

const REFERENCE_ERROR_NAME: &str = "ReferenceError";
const REFERENCE_ERROR_PREFIX: &str = "ReferenceError:";
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
        Error::Exception { name, message } => {
            Some(Value::Error(ErrorObject::new(*name, message.clone())))
        }
        Error::Runtime { message } => reference_error_message(message)
            .map(|message| Value::Error(ErrorObject::new(ErrorName::ReferenceError, message))),
        Error::Lex { .. } | Error::Parse { .. } | Error::ResourceLimit { .. } => None,
    }
}

pub fn reference_error_undefined(name: &str) -> Error {
    Error::runtime(format!("{REFERENCE_ERROR_NAME}: '{name}' is not defined"))
}

fn reference_error_message(message: &str) -> Option<&str> {
    message
        .strip_prefix(REFERENCE_ERROR_PREFIX)?
        .strip_prefix(' ')
}
