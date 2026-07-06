use crate::{
    binding_layout::{BindingOperand, Declaration, DeclarationRef, FunctionScopeId, UpvalueSlot},
    error::{Error, Result},
};

use super::LayoutBuilder;

impl LayoutBuilder {
    pub(super) fn upvalue_operand(
        &mut self,
        function: FunctionScopeId,
        declaration: Declaration,
    ) -> Result<BindingOperand> {
        let reference = DeclarationRef::new(declaration.scope, declaration.name);
        let slot = self.ensure_upvalue_slot(function, reference)?;
        self.lift_upvalue_to_intermediate_functions(function, declaration, reference)?;
        Ok(BindingOperand::Upvalue { function, slot })
    }

    fn lift_upvalue_to_intermediate_functions(
        &mut self,
        function: FunctionScopeId,
        declaration: Declaration,
        reference: DeclarationRef,
    ) -> Result<()> {
        let declaration_function = self.scope(declaration.scope)?.function;
        let mut cursor = self.function(function)?.parent;
        while let Some(parent) = cursor {
            if parent == declaration_function {
                return Ok(());
            }
            self.ensure_upvalue_slot(parent, reference)?;
            cursor = self.function(parent)?.parent;
        }
        Ok(())
    }

    fn ensure_upvalue_slot(
        &mut self,
        function: FunctionScopeId,
        reference: DeclarationRef,
    ) -> Result<UpvalueSlot> {
        let position = match self.function(function)?.upvalue_position(reference) {
            Ok(position) => return UpvalueSlot::from_index(position),
            Err(position) => position,
        };
        self.function_mut(function)?
            .upvalues
            .insert(position, reference);
        self.upvalue_slot_count = self
            .upvalue_slot_count
            .checked_add(1)
            .ok_or_else(|| Error::limit("upvalue binding slot count overflowed"))?;
        UpvalueSlot::from_index(position)
    }
}
