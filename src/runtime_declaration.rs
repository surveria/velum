use crate::{
    ast::{DeclKind, Expr, ForInTarget, StaticBinding, Stmt},
    atom::AtomId,
    error::{Error, Result},
    runtime::Context,
    runtime_completion::Completion,
    runtime_scope::{BindingCell, BindingScope, BindingSlot},
    value::Value,
};

impl Context {
    pub(crate) fn hoist_var_declarations(&mut self, statements: &[Stmt]) -> Result<()> {
        for statement in statements {
            self.hoist_statement_vars(statement)?;
        }
        Ok(())
    }

    fn hoist_statement_vars(&mut self, statement: &Stmt) -> Result<()> {
        match statement {
            Stmt::Block(statements) | Stmt::DeclList(statements) => {
                self.hoist_var_declarations(statements)
            }
            Stmt::If {
                consequent,
                alternate,
                ..
            } => {
                self.hoist_statement_vars(consequent)?;
                if let Some(alternate) = alternate {
                    self.hoist_statement_vars(alternate)?;
                }
                Ok(())
            }
            Stmt::While { body, .. } => self.hoist_statement_vars(body),
            Stmt::For { init, body, .. } => {
                if let Some(init) = init {
                    self.hoist_statement_vars(init)?;
                }
                self.hoist_statement_vars(body)
            }
            Stmt::ForIn { target, body, .. } => {
                if let ForInTarget::Binding {
                    name,
                    kind: DeclKind::Var,
                } = target
                {
                    self.hoist_var(name)?;
                }
                self.hoist_statement_vars(body)
            }
            Stmt::Switch { cases, .. } => self.hoist_switch_vars(cases),
            Stmt::Try {
                body,
                catch,
                finally_body,
            } => {
                self.hoist_var_declarations(body)?;
                if let Some(catch) = catch {
                    self.hoist_var_declarations(&catch.body)?;
                }
                if let Some(finally_body) = finally_body {
                    self.hoist_var_declarations(finally_body)?;
                }
                Ok(())
            }
            Stmt::VarDecl {
                name,
                kind: DeclKind::Var,
                ..
            } => self.hoist_var(name),
            Stmt::Break
            | Stmt::Continue
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::VarDecl { .. }
            | Stmt::Expr(_) => Ok(()),
        }
    }

    fn hoist_var(&mut self, name: &StaticBinding) -> Result<()> {
        let atom = self.intern_static_name_atom(name.name())?;
        if let Some(binding) = self.active_bindings().get(atom) {
            if binding.kind() == DeclKind::Var {
                self.remember_active_static_binding(name, atom)?;
                return Ok(());
            }
            return Err(Error::runtime(format!(
                "'{name}' has already been declared"
            )));
        }

        self.ensure_binding_capacity_for_atom(atom)?;
        self.active_bindings_mut().insert(
            atom,
            BindingCell::new(Value::Undefined, true, DeclKind::Var),
        );
        self.remember_active_static_binding(name, atom)?;
        Ok(())
    }

    pub(crate) fn eval_declaration(
        &mut self,
        name: &StaticBinding,
        kind: DeclKind,
        init: Option<&Expr>,
    ) -> Result<Completion> {
        match kind {
            DeclKind::Var => {
                if let Some(init) = init {
                    let value = self.eval_expr(init)?;
                    self.assign_static(name, value)?;
                }
            }
            DeclKind::Let => {
                let value = self.eval_optional_init(init)?;
                self.define_static(name, value, DeclKind::Let)?;
            }
            DeclKind::Const => {
                let Some(init) = init else {
                    return Err(Error::runtime("const declaration requires an initializer"));
                };
                let value = self.eval_expr(init)?;
                self.define_static(name, value, DeclKind::Const)?;
            }
        }
        Ok(Completion::Normal(Value::Undefined))
    }

    pub(crate) fn eval_optional_init(&mut self, init: Option<&Expr>) -> Result<Value> {
        if let Some(init) = init {
            return self.eval_expr(init);
        }
        Ok(Value::Undefined)
    }

    pub(crate) fn define(&mut self, name: &str, value: Value, kind: DeclKind) -> Result<()> {
        let atom = self.intern_atom(name)?;
        self.define_atom(atom, name, value, kind, None)
    }

    pub(crate) fn define_static(
        &mut self,
        name: &StaticBinding,
        value: Value,
        kind: DeclKind,
    ) -> Result<()> {
        let atom = self.intern_static_name_atom(name.name())?;
        let slot = if self.locals.last().is_some() {
            self.compiled_local_binding_slot(name)?
        } else {
            None
        };
        self.define_atom(atom, name, value, kind, slot)?;
        self.remember_active_static_binding(name, atom)
    }

    fn define_atom(
        &mut self,
        atom: AtomId,
        name: &str,
        value: Value,
        kind: DeclKind,
        slot: Option<BindingSlot>,
    ) -> Result<()> {
        if self.active_bindings().contains(atom) {
            return Err(Error::runtime(format!(
                "'{name}' has already been declared"
            )));
        }
        self.ensure_binding_capacity_for_atom(atom)?;

        self.checked_value(value.clone())?;
        let mutable = kind != DeclKind::Const;
        self.active_bindings_mut()
            .insert_or_replace_at_optional_slot(
                atom,
                BindingCell::new(value, mutable, kind),
                slot,
            )?;
        Ok(())
    }

    pub(crate) fn ensure_binding_capacity_static(
        &mut self,
        name: &StaticBinding,
    ) -> Result<AtomId> {
        let atom = self.intern_static_name_atom(name.name())?;
        self.ensure_binding_capacity_for_atom(atom)?;
        Ok(atom)
    }

    fn ensure_binding_capacity_for_atom(&self, atom: AtomId) -> Result<()> {
        if self.active_bindings().contains(atom) {
            return Ok(());
        }
        if self.binding_count()? >= self.limits.max_bindings {
            return Err(Error::limit(format!(
                "binding count exceeded {}",
                self.limits.max_bindings
            )));
        }
        Ok(())
    }

    pub(crate) fn ensure_extra_binding_capacity(&self, extra_bindings: usize) -> Result<()> {
        let projected = self
            .binding_count()?
            .checked_add(extra_bindings)
            .ok_or_else(|| Error::limit("binding count overflowed"))?;
        if projected >= self.limits.max_bindings {
            return Err(Error::limit(format!(
                "binding count exceeded {}",
                self.limits.max_bindings
            )));
        }
        Ok(())
    }

    fn binding_count(&self) -> Result<usize> {
        self.locals
            .iter()
            .try_fold(self.globals.len(), |count, scope| {
                count
                    .checked_add(scope.len())
                    .ok_or_else(|| Error::limit("binding count overflowed"))
            })
    }

    fn active_bindings(&self) -> &BindingScope {
        if let Some(scope) = self.locals.last() {
            return scope;
        }
        &self.globals
    }

    pub(crate) fn active_bindings_mut(&mut self) -> &mut BindingScope {
        if let Some(scope) = self.locals.last_mut() {
            return scope;
        }
        &mut self.globals
    }

    pub(crate) fn get_binding(&self, name: &str) -> Option<BindingCell> {
        let atom = self.atom(name)?;
        self.get_binding_by_atom(atom)
    }

    pub(crate) fn get_binding_by_atom(&self, atom: AtomId) -> Option<BindingCell> {
        self.locals
            .iter()
            .rev()
            .find_map(|scope| scope.get(atom))
            .or_else(|| self.globals.get(atom))
    }
}
