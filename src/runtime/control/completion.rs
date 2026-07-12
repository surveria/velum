use crate::{
    error::{Error, Result},
    runtime::promise::PromiseId,
    syntax::StaticName,
    value::Value,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Completion {
    Normal(Value),
    Throw(Value),
    Return(Value),
    ReturnDirect(Value),
    Break {
        label: Option<StaticName>,
        value: Value,
    },
    Continue {
        label: Option<StaticName>,
        value: Value,
    },
    Suspended(PromiseId),
    GeneratorStart,
    Yielded(Value),
    YieldedIteratorResult(Value),
}

impl Completion {
    pub const fn suspends_execution(&self) -> bool {
        matches!(
            self,
            Self::Suspended(_)
                | Self::GeneratorStart
                | Self::Yielded(_)
                | Self::YieldedIteratorResult(_)
        )
    }

    pub fn into_result(self) -> Result<Value> {
        match self {
            Self::Normal(value) => Ok(value),
            Self::Throw(value) => Err(Error::javascript(value)),
            Self::Return(value) | Self::ReturnDirect(value) => Err(Error::runtime(format!(
                "return statement outside function returned {value}"
            ))),
            Self::Break { .. } => Err(Error::runtime("break statement outside loop")),
            Self::Continue { .. } => Err(Error::runtime("continue statement outside loop")),
            Self::Suspended(_) => Err(Error::runtime(
                "suspended bytecode escaped its execution owner",
            )),
            Self::Yielded(_) | Self::YieldedIteratorResult(_) => Err(Error::runtime(
                "yielded bytecode escaped its generator owner",
            )),
            Self::GeneratorStart => Err(Error::runtime(
                "generator start escaped its generator owner",
            )),
        }
    }

    pub fn into_function_result(self) -> Result<Value> {
        match self {
            Self::Normal(_) => Ok(Value::Undefined),
            Self::Throw(value) => Err(Error::javascript(value)),
            Self::Return(value) | Self::ReturnDirect(value) => Ok(value),
            Self::Break { .. } => Err(Error::runtime("break statement outside loop")),
            Self::Continue { .. } => Err(Error::runtime("continue statement outside loop")),
            Self::Suspended(_) => Err(Error::runtime(
                "suspended bytecode escaped its function owner",
            )),
            Self::Yielded(_) | Self::YieldedIteratorResult(_) => Err(Error::runtime(
                "yielded bytecode escaped its generator function owner",
            )),
            Self::GeneratorStart => Err(Error::runtime(
                "generator start escaped its generator function owner",
            )),
        }
    }

    pub fn into_call_completion(self) -> Result<Self> {
        match self {
            Self::Normal(_) => Ok(Self::Normal(Value::Undefined)),
            Self::Throw(value) => Ok(Self::Throw(value)),
            Self::Return(value) | Self::ReturnDirect(value) => Ok(Self::Normal(value)),
            Self::Break { .. } => Err(Error::runtime("break statement outside loop")),
            Self::Continue { .. } => Err(Error::runtime("continue statement outside loop")),
            Self::Suspended(_) => Err(Error::runtime("suspended bytecode escaped its call owner")),
            Self::Yielded(_) | Self::YieldedIteratorResult(_) => {
                Err(Error::runtime("yielded bytecode escaped its call owner"))
            }
            Self::GeneratorStart => Err(Error::runtime("generator start escaped its call owner")),
        }
    }

    pub fn into_native_value_result(self) -> Result<Value> {
        match self {
            Self::Normal(value) | Self::Return(value) | Self::ReturnDirect(value) => Ok(value),
            Self::Throw(value) => Err(Error::javascript(value)),
            Self::Break { .. } => Err(Error::runtime("break statement outside loop")),
            Self::Continue { .. } => Err(Error::runtime("continue statement outside loop")),
            Self::Suspended(_) => Err(Error::runtime(
                "suspended bytecode escaped its native-call owner",
            )),
            Self::Yielded(_) | Self::YieldedIteratorResult(_) => Err(Error::runtime(
                "yielded bytecode escaped its native-call owner",
            )),
            Self::GeneratorStart => Err(Error::runtime(
                "generator start escaped its native-call owner",
            )),
        }
    }
}
