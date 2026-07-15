use crate::{
    bytecode::BytecodeHoistPlan,
    error::{Error, Result},
    runtime::{Context, activation::EvalBindingEnvironment, binding::scope::BindingCell},
    syntax::{DeclKind, StaticBinding},
    value::{ErrorName, Value},
};

struct EvalVarHoist {
    cell: BindingCell,
    deletable: bool,
    shadowed: bool,
}

impl Context {
    pub(in crate::runtime) fn hoist_bytecode_eval_local_var_declarations(
        &mut self,
        plan: &BytecodeHoistPlan,
        variable_scope_index: usize,
        environment: &EvalBindingEnvironment,
    ) -> Result<()> {
        for binding in plan.var_declarations() {
            let hoist = self.hoist_eval_local_var(binding, variable_scope_index)?;
            if hoist.shadowed {
                continue;
            }
            let atom = self.intern_static_name_atom(binding.name())?;
            environment
                .insert(atom, hoist.cell, hoist.deletable)
                .map(|_| ())?;
        }
        for declaration in plan.function_declarations() {
            let hoist =
                self.hoist_eval_local_var(declaration.name().name(), variable_scope_index)?;
            let atom = self.intern_static_name_atom(declaration.name().name().name())?;
            if !hoist.shadowed {
                environment
                    .insert(atom, hoist.cell.clone(), hoist.deletable)
                    .map(|_| ())?;
            }
            let function = self.instantiate_hoisted_function(declaration)?;
            hoist
                .cell
                .assign(declaration.name().name().as_str(), function)?;
        }
        Ok(())
    }

    fn hoist_eval_local_var(
        &mut self,
        name: &StaticBinding,
        variable_scope_index: usize,
    ) -> Result<EvalVarHoist> {
        let atom = self.intern_static_name_atom(name.name())?;
        let shadow_start = variable_scope_index
            .checked_add(1)
            .ok_or_else(|| Error::limit("eval variable scope index overflowed"))?;
        let mut shadow_scopes = self.locals.iter().skip(shadow_start);
        if shadow_scopes
            .clone()
            .any(|scope| scope.conflicts_with_eval_var(atom))
        {
            return Err(Error::exception(
                ErrorName::SyntaxError,
                format!("'{name}' has already been declared"),
            ));
        }
        let shadowed = shadow_scopes.any(|scope| scope.shadows_redeclared_eval_var(atom));
        if let Some(cell) = self
            .locals
            .get(variable_scope_index)
            .and_then(|scope| scope.get(atom))
        {
            if cell.kind() != DeclKind::Var {
                return Err(Error::exception(
                    ErrorName::SyntaxError,
                    format!("'{name}' has already been declared"),
                ));
            }
            self.remember_active_static_binding(name, atom)?;
            return Ok(EvalVarHoist {
                cell,
                deletable: false,
                shadowed,
            });
        }

        self.ensure_binding_capacity_for_atom(atom)?;
        let cell = BindingCell::new(Value::Undefined, true, DeclKind::Var);
        let scope = self
            .locals
            .get_mut(variable_scope_index)
            .ok_or_else(|| Error::runtime("eval variable environment scope disappeared"))?;
        scope.insert(atom, cell.clone())?;
        self.remember_active_static_binding(name, atom)?;
        Ok(EvalVarHoist {
            cell,
            deletable: true,
            shadowed,
        })
    }
}
