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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BytecodeClassMemberKey {
    Static(StaticName),
    Computed,
    /// Index into the owning class's `private_names` declaration list.
    Private {
        index: u32,
    },
}
