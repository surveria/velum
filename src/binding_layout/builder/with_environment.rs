use crate::{
    ast::{Expression, Statement},
    binding_metadata::{BindingOperand, FunctionScopeId, ScopeId, types::ScopeKind},
    error::{Error, Result},
    syntax::{StaticBinding, StaticNameId},
};

use super::LayoutBuilder;

impl LayoutBuilder {
    pub(super) fn resolve_declaration_if_with_sensitive(
        &mut self,
        binding: &StaticBinding,
        scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        if self.with_scopes.is_empty() {
            return Ok(());
        }
        self.resolve(binding, scope, function)
    }

    pub(super) fn analyze_with_statement(
        &mut self,
        object: &Expression,
        body: &Statement,
        scope: ScopeId,
        var_scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        self.analyze_expr(object, scope, function)?;
        self.with_scopes.push(scope);
        let result = self.analyze_statement(body, scope, var_scope, function);
        let popped = self
            .with_scopes
            .pop()
            .ok_or_else(|| Error::runtime("with scope stack disappeared"))?;
        if popped != scope {
            return Err(Error::runtime("with scope stack mismatch"));
        }
        result
    }

    pub(super) fn unresolved_count(&self) -> usize {
        self.operands
            .iter()
            .filter(|operand| matches!(operand, BindingOperand::Unresolved))
            .count()
    }

    pub(super) fn set_operand(
        &mut self,
        binding: &StaticBinding,
        operand: BindingOperand,
    ) -> Result<()> {
        let index = binding.id().index()?;
        let Some(slot) = self.operands.get_mut(index) else {
            return Err(Error::runtime("static binding operand slot is not defined"));
        };
        *slot = operand;
        let count = self.with_environment_count_for(binding.name().id(), operand)?;
        let Some(with_slot) = self.with_environment_counts.get_mut(index) else {
            return Err(Error::runtime(
                "static binding with-environment slot is not defined",
            ));
        };
        *with_slot = count;
        Ok(())
    }

    fn with_environment_count_for(
        &self,
        name: StaticNameId,
        operand: BindingOperand,
    ) -> Result<u32> {
        let declaration_scope = self.declaration_scope_for_operand(name, operand)?;
        let mut count = 0_u32;
        for boundary in self.with_scopes.iter().rev() {
            if let Some(scope) = declaration_scope
                && self.scope_is_strict_descendant(scope, *boundary)?
            {
                break;
            }
            count = count
                .checked_add(1)
                .ok_or_else(|| Error::limit("with environment count overflowed"))?;
        }
        Ok(count)
    }

    fn declaration_scope_for_operand(
        &self,
        name: StaticNameId,
        operand: BindingOperand,
    ) -> Result<Option<ScopeId>> {
        match operand {
            BindingOperand::Local { scope, .. } => Ok(Some(scope)),
            BindingOperand::Global { .. } => Ok(self
                .scopes
                .iter()
                .find(|scope| scope.kind == ScopeKind::Global && scope.declaration(name).is_some())
                .and_then(|scope| scope.declaration(name))
                .map(|declaration| declaration.scope)),
            BindingOperand::EvalVariable { .. } => Ok(self
                .scopes
                .iter()
                .find(|scope| {
                    scope.kind == ScopeKind::EvalVariable && scope.declaration(name).is_some()
                })
                .and_then(|scope| scope.declaration(name))
                .map(|declaration| declaration.scope)),
            BindingOperand::Upvalue { function, slot } => Ok(self
                .function(function)?
                .upvalues
                .get(slot.index()?)
                .map(|reference| reference.scope)),
            BindingOperand::Unresolved => Ok(None),
        }
    }

    fn scope_is_strict_descendant(&self, scope: ScopeId, ancestor: ScopeId) -> Result<bool> {
        if scope == ancestor {
            return Ok(false);
        }
        let mut cursor = self.scope(scope)?.parent;
        while let Some(current) = cursor {
            if current == ancestor {
                return Ok(true);
            }
            cursor = self.scope(current)?.parent;
        }
        Ok(false)
    }
}
