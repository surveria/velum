use crate::{
    ast::Expr,
    error::{Error, Result},
    value::{ErrorName, ErrorObject, Value},
};

const ASSERT_NAME: &str = "assert";
const ASSERT_THROWS_NAME: &str = "throws";
const REFERENCE_ERROR_NAME: &str = "ReferenceError";
const REFERENCE_ERROR_PREFIX: &str = "ReferenceError:";
const ERROR_NAME_PROPERTY: &str = "name";
const ERROR_MESSAGE_PROPERTY: &str = "message";

pub fn is_assert_throws_call(callee: &Expr) -> bool {
    matches!(
        callee,
        Expr::Member {
            object, property, ..
        }
            if is_identifier(object, ASSERT_NAME) && property.as_str() == ASSERT_THROWS_NAME
    )
}

pub fn expected_error_name(expr: &Expr) -> Result<&'static str> {
    match expr {
        Expr::Identifier(name) => ErrorName::from_constructor_name(name)
            .map(ErrorName::as_str)
            .ok_or_else(|| {
                Error::runtime(format!(
                    "assert.throws error constructor '{name}' is not supported"
                ))
            }),
        _ => Err(Error::runtime(
            "assert.throws first argument must be an error constructor",
        )),
    }
}

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

pub fn error_property(error: &ErrorObject, property: &str) -> Value {
    error_property_text(error, property)
        .map_or(Value::Undefined, |value| Value::String(value.to_owned()))
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

fn is_identifier(expr: &Expr, expected: &str) -> bool {
    matches!(expr, Expr::Identifier(name) if name.as_str() == expected)
}
