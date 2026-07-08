use std::rc::Rc;

use crate::{
    ast::{Program, StaticBindingId},
    binding_layout::{
        LayoutBuilder,
        types::{BindingOperand, FunctionScope, FunctionScopeId, Scope},
    },
    error::{Error, Result},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BindingLayout {
    pub(super) operands: Rc<[BindingOperand]>,
    pub(super) static_functions: Rc<[Option<FunctionScopeId>]>,
    pub(super) scopes: Rc<[Scope]>,
    pub(super) functions: Rc<[FunctionScope]>,
    global_slot_count: usize,
    local_slot_count: usize,
    upvalue_slot_count: usize,
    unresolved_count: usize,
}

impl BindingLayout {
    pub fn build(
        program: &Program,
        static_binding_count: usize,
        static_function_count: usize,
    ) -> Result<Self> {
        let mut builder = LayoutBuilder::new(static_binding_count, static_function_count);
        builder.build(program)
    }

    pub(super) fn from_parts(parts: BindingLayoutParts) -> Self {
        Self {
            operands: Rc::from(parts.operands.into_boxed_slice()),
            static_functions: Rc::from(parts.static_functions.into_boxed_slice()),
            scopes: Rc::from(parts.scopes.into_boxed_slice()),
            functions: Rc::from(parts.functions.into_boxed_slice()),
            global_slot_count: parts.global_slot_count,
            local_slot_count: parts.local_slot_count,
            upvalue_slot_count: parts.upvalue_slot_count,
            unresolved_count: parts.unresolved_count,
        }
    }

    pub const fn global_slot_count(&self) -> usize {
        self.global_slot_count
    }

    pub const fn local_slot_count(&self) -> usize {
        self.local_slot_count
    }

    pub const fn upvalue_slot_count(&self) -> usize {
        self.upvalue_slot_count
    }

    pub const fn unresolved_count(&self) -> usize {
        self.unresolved_count
    }

    pub fn operand_count(&self) -> usize {
        self.operands.len()
    }

    pub fn resolved_count(&self) -> usize {
        self.operands.len().saturating_sub(self.unresolved_count)
    }

    pub fn for_each_matching_operand_id(
        &self,
        binding: StaticBindingId,
        mut visit: impl FnMut(StaticBindingId) -> Result<()>,
    ) -> Result<()> {
        let Some(target) = self.operand_for_binding_id(binding)? else {
            return Ok(());
        };
        for (index, operand) in self.operands.iter().enumerate() {
            if *operand != target {
                continue;
            }
            visit(StaticBindingId::from_index(index)?)?;
        }
        Ok(())
    }

    pub fn operand_for_binding_id(
        &self,
        binding: StaticBindingId,
    ) -> Result<Option<BindingOperand>> {
        let operand = self
            .operands
            .get(binding.index()?)
            .copied()
            .ok_or_else(|| Error::runtime("binding layout operand slot is not defined"))?;
        if operand == BindingOperand::Unresolved {
            return Ok(None);
        }
        Ok(Some(operand))
    }
}

pub(super) struct BindingLayoutParts {
    pub(super) operands: Vec<BindingOperand>,
    pub(super) static_functions: Vec<Option<FunctionScopeId>>,
    pub(super) scopes: Vec<Scope>,
    pub(super) functions: Vec<FunctionScope>,
    pub(super) global_slot_count: usize,
    pub(super) local_slot_count: usize,
    pub(super) upvalue_slot_count: usize,
    pub(super) unresolved_count: usize,
}
