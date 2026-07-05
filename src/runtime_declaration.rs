use crate::{
    ast::{DeclKind, Expr, ForInTarget, Stmt},
    error::{Error, Result},
    runtime::Context,
    runtime_assertions::reference_error_undefined,
    runtime_completion::Completion,
    runtime_scope::{BindingCell, BindingScope},
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

    fn hoist_var(&mut self, name: &str) -> Result<()> {
        if let Some(binding) = self.active_bindings().get(name) {
            if binding.kind() == DeclKind::Var {
                return Ok(());
            }
            return Err(Error::runtime(format!(
                "'{name}' has already been declared"
            )));
        }

        self.ensure_binding_capacity(name)?;
        self.active_bindings_mut().insert(
            name.to_owned(),
            BindingCell::new(Value::Undefined, true, DeclKind::Var),
        );
        Ok(())
    }

    pub(crate) fn eval_declaration(
        &mut self,
        name: &str,
        kind: DeclKind,
        init: Option<&Expr>,
    ) -> Result<Completion> {
        match kind {
            DeclKind::Var => {
                if let Some(init) = init {
                    let value = self.eval_expr(init)?;
                    self.assign(name, value)?;
                }
            }
            DeclKind::Let => {
                let value = self.eval_optional_init(init)?;
                self.define(name, value, DeclKind::Let)?;
            }
            DeclKind::Const => {
                let Some(init) = init else {
                    return Err(Error::runtime("const declaration requires an initializer"));
                };
                let value = self.eval_expr(init)?;
                self.define(name, value, DeclKind::Const)?;
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
        self.ensure_binding_capacity(name)?;
        if self.active_bindings().contains(name) {
            return Err(Error::runtime(format!(
                "'{name}' has already been declared"
            )));
        }

        self.checked_value(value.clone())?;
        let mutable = kind != DeclKind::Const;
        self.active_bindings_mut()
            .insert(name.to_owned(), BindingCell::new(value, mutable, kind));
        Ok(())
    }

    pub(crate) fn ensure_binding_capacity(&self, name: &str) -> Result<()> {
        if self.active_bindings().contains(name) {
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

    pub(crate) fn assign(&self, name: &str, value: Value) -> Result<()> {
        self.checked_value(value.clone())?;
        let Some(binding) = self.get_binding(name) else {
            return Err(reference_error_undefined(name));
        };
        binding.assign(name, value)
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
        self.locals
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .or_else(|| self.globals.get(name))
    }
}
