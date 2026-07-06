use crate::{
    ast::StaticFunctionId,
    binding_layout_types::{
        BindingOperand, DeclarationRef, FunctionScope, FunctionScopeId, Scope, ScopeId, UpvalueSlot,
    },
    error::{Error, Result},
};

use super::BindingLayout;

impl BindingLayout {
    pub(crate) fn function_for_static_id(
        &self,
        id: StaticFunctionId,
    ) -> Result<Option<FunctionScopeId>> {
        self.static_functions
            .get(id.index()?)
            .copied()
            .ok_or_else(|| Error::runtime("static function layout slot is not defined"))
    }

    pub(crate) fn parent_function(
        &self,
        function: FunctionScopeId,
    ) -> Result<Option<FunctionScopeId>> {
        self.function(function).map(|scope| scope.parent)
    }

    pub(crate) fn declaration_operand(
        &self,
        declaration: DeclarationRef,
    ) -> Result<Option<BindingOperand>> {
        Ok(self
            .scope(declaration.scope)?
            .declaration(declaration.name)
            .map(|declaration| declaration.operand))
    }

    pub(crate) fn upvalue_count_for_function(&self, function: FunctionScopeId) -> Result<usize> {
        self.function(function).map(|scope| scope.upvalues.len())
    }

    pub(crate) fn upvalue_declaration(
        &self,
        function: FunctionScopeId,
        slot: UpvalueSlot,
    ) -> Result<Option<DeclarationRef>> {
        Ok(self
            .function(function)?
            .upvalues
            .get(slot.index()?)
            .copied())
    }

    pub(crate) fn upvalue_slot_for_declaration(
        &self,
        function: FunctionScopeId,
        declaration: DeclarationRef,
    ) -> Result<Option<UpvalueSlot>> {
        let Ok(position) = self.function(function)?.upvalue_position(declaration) else {
            return Ok(None);
        };
        UpvalueSlot::from_index(position).map(Some)
    }

    fn scope(&self, id: ScopeId) -> Result<&Scope> {
        self.scopes
            .get(id.index())
            .ok_or_else(|| Error::runtime("binding layout scope is not defined"))
    }

    fn function(&self, id: FunctionScopeId) -> Result<&FunctionScope> {
        self.functions
            .get(id.index())
            .ok_or_else(|| Error::runtime("binding layout function is not defined"))
    }
}
