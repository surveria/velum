use crate::syntax::StaticName;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BytecodeCompletion {
    Break(Option<StaticName>),
    Continue(Option<StaticName>),
    Return,
    ReturnDirect,
    Throw,
}
