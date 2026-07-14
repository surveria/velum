use crate::{
    error::{Error, Result},
    runtime::promise::PromiseId,
    syntax::StaticName,
    value::Value,
};

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq)]
pub struct TailCall {
    callee: Value,
    arguments: Vec<Value>,
    this_value: Value,
    return_mode: TailCallReturnMode,
}

impl TailCall {
    pub(in crate::runtime) const fn new(
        callee: Value,
        arguments: Vec<Value>,
        this_value: Value,
    ) -> Self {
        Self {
            callee,
            arguments,
            this_value,
            return_mode: TailCallReturnMode::Ordinary,
        }
    }

    pub(in crate::runtime) fn into_parts(self) -> (Value, Vec<Value>, Value, TailCallReturnMode) {
        (
            self.callee,
            self.arguments,
            self.this_value,
            self.return_mode,
        )
    }

    pub(in crate::runtime) const fn callee(&self) -> &Value {
        &self.callee
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::runtime) enum TailCallReturnMode {
    Ordinary,
    DerivedConstructor { this_value: Option<Value> },
}

impl TailCallReturnMode {
    pub(in crate::runtime) fn merge(self, next: Self) -> Result<Self> {
        match (self, next) {
            (current, Self::Ordinary) => Ok(current),
            (Self::Ordinary, derived @ Self::DerivedConstructor { .. }) => Ok(derived),
            (Self::DerivedConstructor { .. }, Self::DerivedConstructor { .. }) => Err(
                Error::runtime("tail call acquired two derived constructor return owners"),
            ),
        }
    }

    pub(in crate::runtime) const fn root_value(&self) -> Option<&Value> {
        match self {
            Self::Ordinary | Self::DerivedConstructor { this_value: None } => None,
            Self::DerivedConstructor {
                this_value: Some(value),
            } => Some(value),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq)]
pub struct DelegatedYield {
    value: Value,
    mode: DelegatedYieldMode,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum DelegatedYieldMode {
    IteratorResult,
    AsyncValue { await_before_yield: bool },
}

impl DelegatedYield {
    pub(in crate::runtime) const fn iterator_result(value: Value) -> Self {
        Self {
            value,
            mode: DelegatedYieldMode::IteratorResult,
        }
    }

    pub(in crate::runtime) const fn async_value(value: Value, await_before_yield: bool) -> Self {
        Self {
            value,
            mode: DelegatedYieldMode::AsyncValue { await_before_yield },
        }
    }

    pub(in crate::runtime) const fn root_value(&self) -> &Value {
        &self.value
    }

    pub(in crate::runtime) fn into_iterator_result(self) -> Result<Value> {
        match self.mode {
            DelegatedYieldMode::IteratorResult => Ok(self.value),
            DelegatedYieldMode::AsyncValue { .. } => Err(Error::runtime(
                "async delegated yield reached a synchronous generator",
            )),
        }
    }

    pub(in crate::runtime) fn into_async_value(self) -> Result<(Value, bool)> {
        match self.mode {
            DelegatedYieldMode::AsyncValue { await_before_yield } => {
                Ok((self.value, await_before_yield))
            }
            DelegatedYieldMode::IteratorResult => Err(Error::runtime(
                "synchronous delegated yield reached an async generator",
            )),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq)]
pub enum Suspension {
    Await(PromiseId),
    GeneratorStart,
    Yield(Value),
    DelegatedYield(DelegatedYield),
}

impl Suspension {
    pub(in crate::runtime) const fn root_value(&self) -> Option<&Value> {
        match self {
            Self::Yield(value) => Some(value),
            Self::DelegatedYield(delegated) => Some(delegated.root_value()),
            Self::Await(_) | Self::GeneratorStart => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Completion {
    Normal(Value),
    Throw(Value),
    Return(Value),
    ReturnDirect(Value),
    #[doc(hidden)]
    TailCall(TailCall),
    Break {
        label: Option<StaticName>,
        value: Value,
    },
    Continue {
        label: Option<StaticName>,
        value: Value,
    },
    Suspend(Suspension),
}

impl Completion {
    pub const fn suspends_execution(&self) -> bool {
        matches!(self, Self::Suspend(_))
    }

    pub fn into_result(self) -> Result<Value> {
        match self {
            Self::Normal(value) => Ok(value),
            Self::Throw(value) => Err(Error::javascript(value)),
            Self::Return(value) | Self::ReturnDirect(value) => Err(Error::runtime(format!(
                "return statement outside function returned {value}"
            ))),
            Self::TailCall(_) => Err(Error::runtime("tail call escaped its function owner")),
            Self::Break { .. } => Err(Error::runtime("break statement outside loop")),
            Self::Continue { .. } => Err(Error::runtime("continue statement outside loop")),
            Self::Suspend(_) => Err(Error::runtime("suspension escaped its execution owner")),
        }
    }

    pub fn into_function_result(self) -> Result<Value> {
        match self {
            Self::Normal(_) => Ok(Value::Undefined),
            Self::Throw(value) => Err(Error::javascript(value)),
            Self::Return(value) | Self::ReturnDirect(value) => Ok(value),
            Self::TailCall(_) => Err(Error::runtime("tail call escaped its function owner")),
            Self::Break { .. } => Err(Error::runtime("break statement outside loop")),
            Self::Continue { .. } => Err(Error::runtime("continue statement outside loop")),
            Self::Suspend(_) => Err(Error::runtime("suspension escaped its function owner")),
        }
    }

    pub fn into_call_completion(self) -> Result<Self> {
        match self {
            Self::Normal(_) => Ok(Self::Normal(Value::Undefined)),
            Self::Throw(value) => Ok(Self::Throw(value)),
            Self::Return(value) | Self::ReturnDirect(value) => Ok(Self::Normal(value)),
            Self::TailCall(request) => Ok(Self::TailCall(request)),
            Self::Break { .. } => Err(Error::runtime("break statement outside loop")),
            Self::Continue { .. } => Err(Error::runtime("continue statement outside loop")),
            Self::Suspend(_) => Err(Error::runtime("suspension escaped its call owner")),
        }
    }

    pub fn into_native_value_result(self) -> Result<Value> {
        match self {
            Self::Normal(value) | Self::Return(value) | Self::ReturnDirect(value) => Ok(value),
            Self::TailCall(_) => Err(Error::runtime("tail call escaped its function owner")),
            Self::Throw(value) => Err(Error::javascript(value)),
            Self::Break { .. } => Err(Error::runtime("break statement outside loop")),
            Self::Continue { .. } => Err(Error::runtime("continue statement outside loop")),
            Self::Suspend(_) => Err(Error::runtime("suspension escaped its native-call owner")),
        }
    }
}
