#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::rc::Rc;

use crate::{
    error::{Error, Result},
    syntax::StaticBindingId,
};

use super::types::{BindingOperand, FunctionScope, FunctionScopeId, Scope};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BindingLayout {
    pub(super) operands: Rc<[BindingOperand]>,
    pub(super) with_environment_counts: Rc<[u32]>,
    pub(super) static_functions: Rc<[Option<FunctionScopeId>]>,
    pub(super) scopes: Rc<[Scope]>,
    pub(super) functions: Rc<[FunctionScope]>,
    global_slot_count: usize,
    local_slot_count: usize,
    upvalue_slot_count: usize,
    unresolved_count: usize,
    source_text: Option<Rc<str>>,
}

impl BindingLayout {
    pub fn from_parts(parts: BindingLayoutParts) -> Self {
        Self {
            operands: Rc::from(parts.operands.into_boxed_slice()),
            with_environment_counts: Rc::from(parts.with_environment_counts.into_boxed_slice()),
            static_functions: Rc::from(parts.static_functions.into_boxed_slice()),
            scopes: Rc::from(parts.scopes.into_boxed_slice()),
            functions: Rc::from(parts.functions.into_boxed_slice()),
            global_slot_count: parts.global_slot_count,
            local_slot_count: parts.local_slot_count,
            upvalue_slot_count: parts.upvalue_slot_count,
            unresolved_count: parts.unresolved_count,
            source_text: None,
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

    pub(crate) fn set_source_text(&mut self, source: Rc<str>) {
        self.source_text = Some(source);
    }

    pub(crate) const fn source_text(&self) -> Option<&Rc<str>> {
        self.source_text.as_ref()
    }

    pub(crate) fn clear_source_text(&mut self) {
        self.source_text = None;
    }

    pub fn resolved_count(&self) -> usize {
        self.operands.len().saturating_sub(self.unresolved_count)
    }

    pub(crate) fn storage_entry_count(&self) -> Result<usize> {
        self.operands
            .len()
            .checked_add(self.with_environment_counts.len())
            .and_then(|count| count.checked_add(self.static_functions.len()))
            .and_then(|count| count.checked_add(self.scopes.len()))
            .and_then(|count| count.checked_add(self.functions.len()))
            .ok_or_else(|| Error::limit("binding layout entry count overflowed"))
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

    pub fn with_environment_count(&self, binding: StaticBindingId) -> Result<u32> {
        self.with_environment_counts
            .get(binding.index()?)
            .copied()
            .ok_or_else(|| Error::runtime("binding layout with-environment slot is not defined"))
    }
}

pub struct BindingLayoutParts {
    pub operands: Vec<BindingOperand>,
    pub with_environment_counts: Vec<u32>,
    pub static_functions: Vec<Option<FunctionScopeId>>,
    pub scopes: Vec<Scope>,
    pub functions: Vec<FunctionScope>,
    pub global_slot_count: usize,
    pub local_slot_count: usize,
    pub upvalue_slot_count: usize,
    pub unresolved_count: usize,
}
