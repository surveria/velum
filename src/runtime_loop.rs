use crate::{
    ast::{DeclKind, Expr, ForInTarget, Stmt},
    error::{Error, Result},
    runtime::Context,
    runtime_completion::Completion,
    runtime_scope::{BindingCell, BindingScope},
    value::Value,
};

impl Context {
    pub(crate) fn eval_while(&mut self, condition: &Expr, body: &Stmt) -> Result<Completion> {
        let mut last = Value::Undefined;
        while self.eval_expr(condition)?.is_truthy() {
            self.step()?;
            match self.eval_statement(body)? {
                Completion::Normal(value) => last = value,
                completion @ (Completion::Throw(_) | Completion::Return(_)) => {
                    return Ok(completion);
                }
                Completion::Break => return Ok(Completion::Normal(last)),
                Completion::Continue => {}
            }
        }
        Ok(Completion::Normal(last))
    }

    pub(crate) fn eval_for(
        &mut self,
        init: Option<&Stmt>,
        condition: Option<&Expr>,
        update: Option<&Expr>,
        body: &Stmt,
    ) -> Result<Completion> {
        if for_init_needs_lexical_scope(init) {
            return self.with_lexical_scope(|context| {
                context.eval_for_loop(init, condition, update, body)
            });
        }

        self.eval_for_loop(init, condition, update, body)
    }

    pub(crate) fn eval_for_in(
        &mut self,
        target: &ForInTarget,
        object: &Expr,
        body: &Stmt,
    ) -> Result<Completion> {
        let object = self.eval_expr(object)?;
        let keys = self.enumerable_keys(&object)?;
        match target {
            ForInTarget::Binding {
                name,
                kind: kind @ (DeclKind::Let | DeclKind::Const),
            } => self.eval_for_in_lexical_binding(name, *kind, keys, body),
            ForInTarget::Binding {
                name,
                kind: DeclKind::Var,
            } => self.eval_for_in_assignment_loop(keys, body, |context, key| {
                context.assign(name, Value::String(key))
            }),
            ForInTarget::Assignment(target) => {
                self.eval_for_in_assignment_loop(keys, body, |context, key| {
                    context.assign_for_in_target(target, Value::String(key))
                })
            }
        }
    }

    fn eval_for_in_lexical_binding(
        &mut self,
        name: &str,
        kind: DeclKind,
        keys: Vec<String>,
        body: &Stmt,
    ) -> Result<Completion> {
        let mut last = Value::Undefined;
        self.ensure_extra_binding_capacity(0)?;
        let mutable = kind != DeclKind::Const;
        let mut scope = BindingScope::new();
        let cleanup_scope = !matches!(body, Stmt::Block(_));
        for key in keys {
            self.step()?;
            let value = self.checked_value(Value::String(key))?;
            scope.insert_or_replace(name, BindingCell::new(value, mutable, kind));
            self.push_lexical_scope_with(scope);
            let completion = self.eval_statement(body);
            let Some(mut removed_scope) = self.pop_lexical_scope() else {
                return Err(Error::runtime("lexical scope disappeared"));
            };
            if cleanup_scope {
                removed_scope.retain_only(name);
            }
            scope = removed_scope;
            let completion = completion?;
            if let Some(completion) = loop_completion(&mut last, completion) {
                return Ok(completion);
            }
        }
        Ok(Completion::Normal(last))
    }

    fn eval_for_in_assignment_loop(
        &mut self,
        keys: Vec<String>,
        body: &Stmt,
        mut assign: impl FnMut(&mut Self, String) -> Result<()>,
    ) -> Result<Completion> {
        let mut last = Value::Undefined;
        for key in keys {
            self.step()?;
            assign(self, key)?;
            let completion = self.eval_statement(body)?;
            if let Some(completion) = loop_completion(&mut last, completion) {
                return Ok(completion);
            }
        }
        Ok(Completion::Normal(last))
    }

    fn assign_for_in_target(&mut self, target: &Expr, value: Value) -> Result<()> {
        match target {
            Expr::Identifier(name) => self.assign(name, value),
            Expr::Member { object, property } => {
                let object = self.eval_expr(object)?;
                self.set_property_value(&object, property.to_owned(), value)
            }
            Expr::ComputedMember { object, property } => {
                let object = self.eval_expr(object)?;
                let property = self.eval_property_key(property)?;
                self.set_property_value(&object, property, value)
            }
            Expr::Parenthesized(expr) => self.assign_for_in_target(expr, value),
            _ => Err(Error::runtime("invalid for-in assignment target")),
        }
    }

    fn eval_for_loop(
        &mut self,
        init: Option<&Stmt>,
        condition: Option<&Expr>,
        update: Option<&Expr>,
        body: &Stmt,
    ) -> Result<Completion> {
        if let Some(init) = init {
            self.evaluate_for_init(init)?;
        }

        let mut last = Value::Undefined;
        loop {
            if !self.for_condition_is_truthy(condition)? {
                break;
            }
            self.step()?;
            match self.eval_statement(body)? {
                Completion::Normal(value) => last = value,
                Completion::Continue => {}
                Completion::Break => return Ok(Completion::Normal(last)),
                completion @ (Completion::Throw(_) | Completion::Return(_)) => {
                    return Ok(completion);
                }
            }
            self.eval_for_update(update)?;
        }
        Ok(Completion::Normal(last))
    }

    fn evaluate_for_init(&mut self, init: &Stmt) -> Result<()> {
        if let Stmt::DeclList(statements) = init {
            return self.evaluate_for_init_block(statements);
        }
        self.evaluate_for_init_statement(init)
    }

    fn evaluate_for_init_block(&mut self, statements: &[Stmt]) -> Result<()> {
        for statement in statements {
            self.evaluate_for_init_statement(statement)?;
        }
        Ok(())
    }

    fn evaluate_for_init_statement(&mut self, init: &Stmt) -> Result<()> {
        match self.eval_statement(init)? {
            Completion::Normal(_) => Ok(()),
            completion => completion.into_result().map(|_| ()),
        }
    }

    fn for_condition_is_truthy(&mut self, condition: Option<&Expr>) -> Result<bool> {
        let Some(condition) = condition else {
            return Ok(true);
        };
        Ok(self.eval_expr(condition)?.is_truthy())
    }

    fn eval_for_update(&mut self, update: Option<&Expr>) -> Result<()> {
        if let Some(update) = update {
            self.eval_expr(update)?;
        }
        Ok(())
    }
}

fn loop_completion(last: &mut Value, completion: Completion) -> Option<Completion> {
    match completion {
        Completion::Normal(value) => {
            *last = value;
            None
        }
        Completion::Continue => None,
        Completion::Break => Some(Completion::Normal(last.clone())),
        completion @ (Completion::Throw(_) | Completion::Return(_)) => Some(completion),
    }
}

fn for_init_needs_lexical_scope(init: Option<&Stmt>) -> bool {
    match init {
        Some(Stmt::VarDecl {
            kind: DeclKind::Let | DeclKind::Const,
            ..
        }) => true,
        Some(Stmt::DeclList(statements)) => statements.iter().any(is_lexical_declaration),
        Some(
            Stmt::VarDecl {
                kind: DeclKind::Var,
                ..
            }
            | Stmt::Block(_)
            | Stmt::If { .. }
            | Stmt::While { .. }
            | Stmt::For { .. }
            | Stmt::ForIn { .. }
            | Stmt::Switch { .. }
            | Stmt::Try { .. }
            | Stmt::Break
            | Stmt::Continue
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::Expr(_),
        )
        | None => false,
    }
}

const fn is_lexical_declaration(statement: &Stmt) -> bool {
    matches!(
        statement,
        Stmt::VarDecl {
            kind: DeclKind::Let | DeclKind::Const,
            ..
        }
    )
}
