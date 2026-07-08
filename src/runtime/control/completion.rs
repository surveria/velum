use crate::{
    error::{Error, Result},
    syntax::StaticName,
    value::Value,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Completion {
    Normal(Value),
    Throw(Value),
    Return(Value),
    Break {
        label: Option<StaticName>,
        value: Value,
    },
    Continue(Option<StaticName>),
}

impl Completion {
    pub fn into_result(self) -> Result<Value> {
        match self {
            Self::Normal(value) => Ok(value),
            Self::Throw(value) => Err(Error::runtime(format!("uncaught throw: {value}"))),
            Self::Return(value) => Err(Error::runtime(format!(
                "return statement outside function returned {value}"
            ))),
            Self::Break { .. } => Err(Error::runtime("break statement outside loop")),
            Self::Continue(_) => Err(Error::runtime("continue statement outside loop")),
        }
    }

    pub fn into_function_result(self) -> Result<Value> {
        match self {
            Self::Normal(_) => Ok(Value::Undefined),
            Self::Throw(value) => Err(Error::runtime(format!("uncaught throw: {value}"))),
            Self::Return(value) => Ok(value),
            Self::Break { .. } => Err(Error::runtime("break statement outside loop")),
            Self::Continue(_) => Err(Error::runtime("continue statement outside loop")),
        }
    }

    pub fn into_call_completion(self) -> Result<Self> {
        match self {
            Self::Normal(_) => Ok(Self::Normal(Value::Undefined)),
            Self::Throw(value) => Ok(Self::Throw(value)),
            Self::Return(value) => Ok(Self::Normal(value)),
            Self::Break { .. } => Err(Error::runtime("break statement outside loop")),
            Self::Continue(_) => Err(Error::runtime("continue statement outside loop")),
        }
    }
}
