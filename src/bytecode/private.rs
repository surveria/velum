use crate::syntax::StaticName;

/// One lexically resolved `#name` reference in executable bytecode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodePrivateName {
    name: StaticName,
}

impl BytecodePrivateName {
    pub(crate) const fn new(name: StaticName) -> Self {
        Self { name }
    }

    pub const fn name(&self) -> &StaticName {
        &self.name
    }
}
