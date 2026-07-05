use crate::{
    ast::Expr,
    error::{Error, Result},
    value::{ErrorName, ErrorObject, Value},
};

const ASSERT_NAME: &str = "assert";
const ASSERT_THROWS_NAME: &str = "throws";
const REFERENCE_ERROR_NAME: &str = "ReferenceError";
const REFERENCE_ERROR_PREFIX: &str = "ReferenceError:";

pub fn is_assert_throws_call(callee: &Expr) -> bool {
    matches!(
        callee,
        Expr::Member { object, property }
            if is_identifier(object, ASSERT_NAME) && property == ASSERT_THROWS_NAME
    )
}

pub fn expected_error_name(expr: &Expr) -> Result<&'static str> {
    match expr {
        Expr::Identifier(name) if name == REFERENCE_ERROR_NAME => Ok(REFERENCE_ERROR_NAME),
        Expr::Identifier(name) => Err(Error::runtime(format!(
            "assert.throws error constructor '{name}' is not supported"
        ))),
        _ => Err(Error::runtime(
            "assert.throws first argument must be an error constructor",
        )),
    }
}

pub fn thrown_value_matches(value: &Value, expected_name: &str) -> bool {
    matches!(
        (value, expected_name),
        (Value::Error(error), REFERENCE_ERROR_NAME)
            if error.name() == ErrorName::ReferenceError
    )
}

pub fn error_property(error: &ErrorObject, property: &str) -> Value {
    match property {
        "name" => Value::String(error.name().as_str().to_owned()),
        "message" => Value::String(error.message().to_owned()),
        _ => Value::Undefined,
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
    matches!(expr, Expr::Identifier(name) if name == expected)
}
