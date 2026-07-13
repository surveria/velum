use crate::{
    ast::{
        BindingPattern, CatchClause, DeclKind, Expression, ForInTarget, Program, Statement,
        StaticBinding, StaticFunctionId, StaticNameId, Stmt, SwitchCase,
    },
    binding_metadata::{
        BindingLayout, BindingLayoutParts, BindingOperand, FunctionScopeId, LocalSlot, ScopeId,
        types::{Declaration, FunctionScope, GlobalSlot, Scope, ScopeContext, ScopeKind},
    },
    error::{Error, Result},
};

use super::{RootLayoutMode, for_init_needs_lexical_scope};

mod annex_b;
mod expression;
mod function;
mod with_environment;
use function::FunctionBindings;
pub(super) struct LayoutBuilder {
    operands: Vec<BindingOperand>,
    with_environment_counts: Vec<u32>,
    static_functions: Vec<Option<FunctionScopeId>>,
    scopes: Vec<Scope>,
    functions: Vec<FunctionScope>,
    global_slot_count: usize,
    local_slot_count: usize,
    pub(super) upvalue_slot_count: usize,
    with_scopes: Vec<ScopeId>,
}

impl LayoutBuilder {
    pub(super) fn new(static_binding_count: usize, static_function_count: usize) -> Self {
        Self {
            operands: vec![BindingOperand::Unresolved; static_binding_count],
            with_environment_counts: vec![0; static_binding_count],
            static_functions: vec![None; static_function_count],
            scopes: Vec::new(),
            functions: Vec::new(),
            global_slot_count: 0,
            local_slot_count: 0,
            upvalue_slot_count: 0,
            with_scopes: Vec::new(),
        }
    }

    pub(super) fn build(
        &mut self,
        program: &Program,
        mode: &RootLayoutMode,
    ) -> Result<BindingLayout> {
        let root_function = self.add_function(None);
        let (root_scope, var_scope) = match mode {
            RootLayoutMode::Script => {
                let root = self.add_scope(None, root_function, ScopeKind::Global);
                (root, root)
            }
            RootLayoutMode::SloppyEval => {
                let var_scope = self.add_scope(None, root_function, ScopeKind::Global);
                let lexical_scope =
                    self.add_scope(Some(var_scope), root_function, ScopeKind::Local);
                (lexical_scope, var_scope)
            }
            RootLayoutMode::StrictEval => {
                let root = self.add_scope(None, root_function, ScopeKind::Local);
                (root, root)
            }
        };
        self.collect_annex_b_var_bindings(&program.statements, var_scope)?;
        self.analyze_statements(&program.statements, root_scope, var_scope, root_function)?;
        let unresolved_count = self.unresolved_count();
        Ok(BindingLayout::from_parts(BindingLayoutParts {
            operands: self.operands.clone(),
            with_environment_counts: self.with_environment_counts.clone(),
            static_functions: self.static_functions.clone(),
            scopes: self.scopes.clone(),
            functions: self.functions.clone(),
            global_slot_count: self.global_slot_count,
            local_slot_count: self.local_slot_count,
            upvalue_slot_count: self.upvalue_slot_count,
            unresolved_count,
        }))
    }

    fn analyze_statements(
        &mut self,
        statements: &[Statement],
        scope: ScopeId,
        var_scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        self.collect_scope_declarations(statements, scope, var_scope)?;
        for statement in statements {
            self.analyze_statement(statement, scope, var_scope, function)?;
        }
        Ok(())
    }

    fn collect_scope_declarations(
        &mut self,
        statements: &[Statement],
        scope: ScopeId,
        var_scope: ScopeId,
    ) -> Result<()> {
        for statement in statements {
            self.collect_hoisted_vars(statement, var_scope)?;
        }
        for statement in statements {
            self.collect_direct_declaration(statement, scope, var_scope)?;
        }
        Ok(())
    }

    fn collect_hoisted_statement_list(
        &mut self,
        statements: &[Statement],
        var_scope: ScopeId,
    ) -> Result<()> {
        for statement in statements {
            self.collect_hoisted_vars(statement, var_scope)?;
        }
        Ok(())
    }

    fn collect_hoisted_vars(&mut self, statement: &Statement, var_scope: ScopeId) -> Result<()> {
        match statement.kind() {
            Stmt::Block(statements) | Stmt::DeclList(statements) => {
                self.collect_hoisted_statement_list(statements, var_scope)
            }
            Stmt::If {
                consequent,
                alternate,
                ..
            } => {
                self.collect_hoisted_vars(consequent, var_scope)?;
                if let Some(alternate) = alternate {
                    self.collect_hoisted_vars(alternate, var_scope)?;
                }
                Ok(())
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::With { body, .. }
            | Stmt::Label { body, .. } => self.collect_hoisted_vars(body, var_scope),
            Stmt::For { init, body, .. } => {
                if let Some(init) = init {
                    self.collect_hoisted_vars(init, var_scope)?;
                }
                self.collect_hoisted_vars(body, var_scope)
            }
            Stmt::ForIn { target, body, .. } | Stmt::ForOf { target, body, .. } => {
                match target {
                    ForInTarget::Binding {
                        name,
                        kind: DeclKind::Var,
                    } => self.declare(var_scope, name)?,
                    ForInTarget::PatternBinding {
                        pattern,
                        kind: DeclKind::Var,
                    } => self.declare_pattern(pattern, var_scope)?,
                    ForInTarget::Binding { .. }
                    | ForInTarget::PatternBinding { .. }
                    | ForInTarget::PatternAssignment { .. }
                    | ForInTarget::Assignment { .. } => {}
                }
                self.collect_hoisted_vars(body, var_scope)
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    for statement in &case.statements {
                        self.collect_hoisted_vars(statement, var_scope)?;
                    }
                }
                Ok(())
            }
            Stmt::Try {
                body,
                catch,
                finally_body,
            } => {
                self.collect_hoisted_statement_list(body, var_scope)?;
                if let Some(catch) = catch {
                    self.collect_hoisted_statement_list(&catch.body, var_scope)?;
                }
                if let Some(finally_body) = finally_body {
                    self.collect_hoisted_statement_list(finally_body, var_scope)?;
                }
                Ok(())
            }
            Stmt::VarDecl {
                name,
                kind: DeclKind::Var,
                ..
            }
            | Stmt::FunctionDecl {
                name,
                block_scoped: false,
                ..
            } => self.declare(var_scope, name),
            Stmt::PatternDecl {
                pattern,
                kind: DeclKind::Var,
                ..
            } => self.declare_pattern(pattern, var_scope),
            Stmt::FunctionDecl { .. }
            | Stmt::ImportBinding { .. }
            | Stmt::VarDecl { .. }
            | Stmt::PatternDecl { .. }
            | Stmt::ClassDecl { .. }
            | Stmt::Empty
            | Stmt::Debugger
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::Expr(_) => Ok(()),
        }
    }

    fn analyze_statement(
        &mut self,
        statement: &Statement,
        scope: ScopeId,
        var_scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        match statement.kind() {
            Stmt::Block(statements) => {
                let block_scope = self.add_scope(Some(scope), function, ScopeKind::Local);
                self.analyze_statements(statements, block_scope, var_scope, function)
            }
            Stmt::DeclList(statements) => {
                for statement in statements {
                    self.analyze_statement(statement, scope, var_scope, function)?;
                }
                Ok(())
            }
            Stmt::If {
                condition,
                consequent,
                alternate,
            } => {
                self.analyze_expr(condition, scope, function)?;
                self.analyze_statement(consequent, scope, var_scope, function)?;
                if let Some(alternate) = alternate {
                    self.analyze_statement(alternate, scope, var_scope, function)?;
                }
                Ok(())
            }
            Stmt::While { condition, body } | Stmt::DoWhile { condition, body } => {
                self.analyze_expr(condition, scope, function)?;
                self.analyze_statement(body, scope, var_scope, function)
            }
            Stmt::With { object, body } => {
                self.analyze_with_statement(object, body, scope, var_scope, function)
            }
            Stmt::Label { body, .. } => self.analyze_statement(body, scope, var_scope, function),
            Stmt::For {
                init,
                condition,
                update,
                body,
            } => self.analyze_for(
                init.as_deref(),
                condition.as_ref(),
                update.as_ref(),
                body,
                ScopeContext::new(scope, var_scope, function),
            ),
            Stmt::ForIn {
                target,
                object,
                body,
            }
            | Stmt::ForOf {
                target,
                object,
                body,
                ..
            } => self.analyze_for_in(target, object, body, scope, var_scope, function),
            Stmt::Switch {
                discriminant,
                cases,
            } => self.analyze_switch(discriminant, cases, scope, var_scope, function),
            Stmt::Try {
                body,
                catch,
                finally_body,
            } => self.analyze_try(
                body,
                catch.as_ref(),
                finally_body.as_deref(),
                ScopeContext::new(scope, var_scope, function),
            ),
            Stmt::Throw(expr) | Stmt::Return(Some(expr)) | Stmt::Expr(expr) => {
                self.analyze_expr(expr, scope, function)
            }
            declaration @ Stmt::FunctionDecl { .. } => {
                self.analyze_function_declaration(declaration, scope, function)
            }
            Stmt::Empty
            | Stmt::Debugger
            | Stmt::Return(None)
            | Stmt::Break(_)
            | Stmt::Continue(_) => Ok(()),
            Stmt::VarDecl { name, init, .. } => {
                if let Some(init) = init {
                    self.analyze_expr(init, scope, function)?;
                }
                self.resolve_declaration_if_with_sensitive(name, scope, function)
            }
            Stmt::ImportBinding { name } => {
                self.resolve_declaration_if_with_sensitive(name, scope, function)
            }
            Stmt::PatternDecl { pattern, init, .. } => {
                self.analyze_expr(init, scope, function)?;
                self.analyze_pattern_exprs(pattern, scope, function)?;
                pattern.for_each_binding(&mut |binding| {
                    self.resolve_declaration_if_with_sensitive(binding, scope, function)
                })
            }
            Stmt::ClassDecl { name, class } => {
                self.resolve_declaration_if_with_sensitive(name, scope, function)?;
                self.analyze_class(class, scope, function)
            }
        }
    }

    fn analyze_for(
        &mut self,
        init: Option<&Statement>,
        condition: Option<&Expression>,
        update: Option<&Expression>,
        body: &Statement,
        context: ScopeContext,
    ) -> Result<()> {
        let loop_scope = if for_init_needs_lexical_scope(init) {
            let loop_scope =
                self.add_scope(Some(context.scope), context.function, ScopeKind::Local);
            if let Some(init) = init {
                self.collect_direct_declaration(init, loop_scope, context.var_scope)?;
            }
            loop_scope
        } else {
            context.scope
        };
        if let Some(init) = init {
            self.analyze_statement(init, loop_scope, context.var_scope, context.function)?;
        }
        if let Some(condition) = condition {
            self.analyze_expr(condition, loop_scope, context.function)?;
        }
        if let Some(update) = update {
            self.analyze_expr(update, loop_scope, context.function)?;
        }
        self.analyze_statement(body, loop_scope, context.var_scope, context.function)
    }

    fn analyze_for_in(
        &mut self,
        target: &ForInTarget,
        object: &Expression,
        body: &Statement,
        scope: ScopeId,
        var_scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        self.analyze_expr(object, scope, function)?;
        match target {
            ForInTarget::Binding {
                name,
                kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
            } => {
                let loop_scope = self.add_scope(Some(scope), function, ScopeKind::Local);
                self.declare(loop_scope, name)?;
                self.resolve_declaration_if_with_sensitive(name, loop_scope, function)?;
                self.analyze_statement(body, loop_scope, var_scope, function)
            }
            ForInTarget::Binding {
                name,
                kind: DeclKind::Var,
            } => {
                self.declare(var_scope, name)?;
                self.resolve_declaration_if_with_sensitive(name, scope, function)?;
                self.analyze_statement(body, scope, var_scope, function)
            }
            ForInTarget::PatternBinding {
                pattern,
                kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
            } => {
                let loop_scope = self.add_scope(Some(scope), function, ScopeKind::Local);
                self.declare_pattern(pattern, loop_scope)?;
                pattern.for_each_binding(&mut |binding| {
                    self.resolve_declaration_if_with_sensitive(binding, loop_scope, function)
                })?;
                self.analyze_pattern_exprs(pattern, loop_scope, function)?;
                self.analyze_statement(body, loop_scope, var_scope, function)
            }
            ForInTarget::PatternBinding {
                pattern,
                kind: DeclKind::Var,
            } => {
                self.declare_pattern(pattern, var_scope)?;
                pattern.for_each_binding(&mut |binding| {
                    self.resolve_declaration_if_with_sensitive(binding, scope, function)
                })?;
                self.analyze_pattern_exprs(pattern, scope, function)?;
                self.analyze_statement(body, scope, var_scope, function)
            }
            ForInTarget::Assignment { target, .. } => {
                self.analyze_expr(target, scope, function)?;
                self.analyze_statement(body, scope, var_scope, function)
            }
            ForInTarget::PatternAssignment { pattern, .. } => {
                pattern.for_each_expr(&mut |expr| self.analyze_expr(expr, scope, function))?;
                self.analyze_statement(body, scope, var_scope, function)
            }
        }
    }

    fn declare_pattern(&mut self, pattern: &BindingPattern, scope: ScopeId) -> Result<()> {
        pattern.for_each_binding(&mut |binding| self.declare(scope, binding))
    }

    fn analyze_pattern_exprs(
        &mut self,
        pattern: &BindingPattern,
        scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        pattern.for_each_expr(&mut |expr| self.analyze_expr(expr, scope, function))
    }

    fn analyze_class(
        &mut self,
        class: &crate::ast::ClassLiteral,
        scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        let class_scope = if let Some(binding) = &class.inner_name_binding {
            let class_scope = self.add_scope(Some(scope), function, ScopeKind::Local);
            self.declare(class_scope, binding)?;
            class_scope
        } else {
            scope
        };
        if let Some(heritage) = &class.heritage {
            self.analyze_expr(heritage, class_scope, function)?;
        }
        self.analyze_function(
            class.constructor.id,
            FunctionBindings::new(None, class.constructor.arguments_binding.as_ref()),
            &class.constructor.params,
            &class.constructor.body,
            class_scope,
            function,
        )?;
        for member in &class.members {
            if let crate::ast::ClassElementName::Property(
                crate::ast::ObjectPropertyKey::Computed(key),
            ) = &member.key
            {
                self.analyze_expr(key, class_scope, function)?;
            }
            self.analyze_function(
                member.id,
                FunctionBindings::new(None, member.arguments_binding.as_ref()),
                &member.params,
                &member.body,
                class_scope,
                function,
            )?;
        }
        for field in &class.fields {
            if let crate::ast::ClassElementName::Property(
                crate::ast::ObjectPropertyKey::Computed(key),
            ) = &field.key
            {
                self.analyze_expr(key, class_scope, function)?;
            }
            if let Some(initializer) = &field.initializer {
                self.analyze_expr(initializer, class_scope, function)?;
            }
        }
        for block in &class.static_blocks {
            let block_scope = self.add_scope(Some(class_scope), function, ScopeKind::Local);
            self.analyze_statements(&block.body, block_scope, block_scope, function)?;
        }
        Ok(())
    }

    fn analyze_switch(
        &mut self,
        discriminant: &Expression,
        cases: &[SwitchCase],
        scope: ScopeId,
        var_scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        self.analyze_expr(discriminant, scope, function)?;
        let switch_scope = self.add_scope(Some(scope), function, ScopeKind::Local);
        for case in cases {
            if let Some(test) = &case.test {
                self.analyze_expr(test, switch_scope, function)?;
            }
            self.collect_scope_declarations(&case.statements, switch_scope, var_scope)?;
        }
        for case in cases {
            self.analyze_statements(&case.statements, switch_scope, var_scope, function)?;
        }
        Ok(())
    }

    fn analyze_try(
        &mut self,
        body: &[Statement],
        catch: Option<&CatchClause>,
        finally_body: Option<&[Statement]>,
        context: ScopeContext,
    ) -> Result<()> {
        let body_scope = self.add_scope(Some(context.scope), context.function, ScopeKind::Local);
        self.analyze_statements(body, body_scope, context.var_scope, context.function)?;
        if let Some(catch) = catch {
            self.analyze_catch(catch, context.scope, context.var_scope, context.function)?;
        }
        if let Some(finally_body) = finally_body {
            let finally_scope =
                self.add_scope(Some(context.scope), context.function, ScopeKind::Local);
            self.analyze_statements(
                finally_body,
                finally_scope,
                context.var_scope,
                context.function,
            )?;
        }
        Ok(())
    }

    fn analyze_catch(
        &mut self,
        catch: &CatchClause,
        scope: ScopeId,
        var_scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        let catch_scope = self.add_scope(Some(scope), function, ScopeKind::Local);
        if let Some(param) = &catch.param {
            self.declare_pattern(param, catch_scope)?;
            self.analyze_pattern_exprs(param, catch_scope, function)?;
        }
        let body_scope = self.add_scope(Some(catch_scope), function, ScopeKind::Local);
        self.analyze_statements(&catch.body, body_scope, var_scope, function)
    }

    fn declare(&mut self, scope: ScopeId, binding: &StaticBinding) -> Result<()> {
        let name = binding.name().id();
        let operand = if let Some(declaration) = self.scope(scope)?.declaration(name) {
            declaration.operand
        } else {
            self.insert_declaration(scope, name)?
        };
        self.set_operand(binding, operand)
    }

    fn insert_declaration(&mut self, scope: ScopeId, name: StaticNameId) -> Result<BindingOperand> {
        let operand = self.next_declaration_operand(scope)?;
        let position = match self.scope(scope)?.declaration_position(name) {
            Ok(position) | Err(position) => position,
        };
        let declaration = Declaration::new(name, scope, operand);
        self.scope_mut(scope)?
            .declarations
            .insert(position, declaration);
        Ok(operand)
    }

    fn next_declaration_operand(&mut self, scope: ScopeId) -> Result<BindingOperand> {
        match self.scope(scope)?.kind {
            ScopeKind::Global => {
                let slot = GlobalSlot::from_index(self.global_slot_count)?;
                self.global_slot_count = self
                    .global_slot_count
                    .checked_add(1)
                    .ok_or_else(|| Error::limit("global binding slot count overflowed"))?;
                Ok(BindingOperand::Global { slot })
            }
            ScopeKind::Local => {
                let local_count = self.scope(scope)?.declarations.len();
                let slot = LocalSlot::from_index(local_count)?;
                self.local_slot_count = self
                    .local_slot_count
                    .checked_add(1)
                    .ok_or_else(|| Error::limit("local binding slot count overflowed"))?;
                Ok(BindingOperand::Local { scope, slot })
            }
        }
    }

    fn resolve(
        &mut self,
        binding: &StaticBinding,
        scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        let operand = self
            .resolve_operand(binding.name().id(), scope, function)?
            .unwrap_or(BindingOperand::Unresolved);
        self.set_operand(binding, operand)
    }

    fn resolve_operand(
        &mut self,
        name: StaticNameId,
        scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<Option<BindingOperand>> {
        let mut cursor = Some(scope);
        while let Some(scope_id) = cursor {
            let scope_ref = self.scope(scope_id)?;
            if let Some(declaration) = scope_ref.declaration(name) {
                if scope_ref.kind == ScopeKind::Global {
                    return Ok(Some(declaration.operand));
                }
                if scope_ref.function == function {
                    return Ok(Some(declaration.operand));
                }
                return self.upvalue_operand(function, declaration).map(Some);
            }
            cursor = scope_ref.parent;
        }
        Ok(None)
    }

    fn add_function(&mut self, parent: Option<FunctionScopeId>) -> FunctionScopeId {
        let id = FunctionScopeId::from_index(self.functions.len());
        self.functions.push(FunctionScope::new(parent));
        id
    }

    fn record_static_function(
        &mut self,
        id: StaticFunctionId,
        function: FunctionScopeId,
    ) -> Result<()> {
        let index = id.index()?;
        let Some(slot) = self.static_functions.get_mut(index) else {
            return Err(Error::runtime("static function layout slot is not defined"));
        };
        *slot = Some(function);
        Ok(())
    }

    fn add_scope(
        &mut self,
        parent: Option<ScopeId>,
        function: FunctionScopeId,
        kind: ScopeKind,
    ) -> ScopeId {
        let id = ScopeId::from_index(self.scopes.len());
        self.scopes.push(Scope::new(parent, function, kind));
        id
    }

    pub(super) fn scope(&self, id: ScopeId) -> Result<&Scope> {
        self.scopes
            .get(id.index())
            .ok_or_else(|| Error::runtime("binding layout scope is not defined"))
    }

    fn scope_mut(&mut self, id: ScopeId) -> Result<&mut Scope> {
        self.scopes
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("binding layout scope is not defined"))
    }

    pub(super) fn function(&self, id: FunctionScopeId) -> Result<&FunctionScope> {
        self.functions
            .get(id.index())
            .ok_or_else(|| Error::runtime("binding layout function is not defined"))
    }
}
