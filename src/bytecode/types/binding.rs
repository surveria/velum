use core::fmt;

use crate::{
    binding_metadata::{BindingLayout, BindingOperand},
    error::Result,
    syntax::StaticBinding,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BytecodeBinding {
    name: StaticBinding,
    operand: BindingOperand,
    strict_write: bool,
    with_environment_count: u32,
}

impl BytecodeBinding {
    pub(crate) fn compile(name: &StaticBinding, layout: &BindingLayout) -> Result<Self> {
        let operand = layout
            .operand_for_binding_id(name.id())?
            .unwrap_or(BindingOperand::Unresolved);
        Ok(Self {
            name: name.clone(),
            operand,
            strict_write: false,
            with_environment_count: layout.with_environment_count(name.id())?,
        })
    }

    pub(crate) fn compile_write(
        name: &StaticBinding,
        layout: &BindingLayout,
        strict_write: bool,
    ) -> Result<Self> {
        let mut binding = Self::compile(name, layout)?;
        binding.strict_write = strict_write;
        Ok(binding)
    }

    pub const fn name(&self) -> &StaticBinding {
        &self.name
    }

    pub const fn operand(&self) -> BindingOperand {
        self.operand
    }

    pub const fn strict_write(&self) -> bool {
        self.strict_write
    }

    pub const fn with_environment_count(&self) -> u32 {
        self.with_environment_count
    }
}

impl fmt::Display for BytecodeBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.fmt(formatter)
    }
}
