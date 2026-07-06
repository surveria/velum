use std::rc::Rc;

use crate::{
    ast::{
        CatchClause, DeclKind, Expr, ForInTarget, Program, StaticBinding, StaticBindingId,
        StaticFunctionId, StaticNameId, Stmt, SwitchCase,
    },
    binding_layout::types::{
        Declaration, FunctionScope, GlobalSlot, Scope, ScopeContext, ScopeKind,
    },
    error::{Error, Result},
};

mod metadata;
pub mod types;
mod upvalues;

pub use types::{BindingOperand, DeclarationRef, FunctionScopeId, LocalSlot, ScopeId, UpvalueSlot};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BindingLayout {
    operands: Rc<[BindingOperand]>,
    static_functions: Rc<[Option<FunctionScopeId>]>,
    scopes: Rc<[Scope]>,
    functions: Rc<[FunctionScope]>,
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

struct LayoutBuilder {
    operands: Vec<BindingOperand>,
    static_functions: Vec<Option<FunctionScopeId>>,
    scopes: Vec<Scope>,
    functions: Vec<FunctionScope>,
    global_slot_count: usize,
    local_slot_count: usize,
    upvalue_slot_count: usize,
}

impl LayoutBuilder {
    fn new(static_binding_count: usize, static_function_count: usize) -> Self {
        Self {
            operands: vec![BindingOperand::Unresolved; static_binding_count],
            static_functions: vec![None; static_function_count],
            scopes: Vec::new(),
            functions: Vec::new(),
            global_slot_count: 0,
            local_slot_count: 0,
            upvalue_slot_count: 0,
        }
    }

    fn build(&mut self, program: &Program) -> Result<BindingLayout> {
        let root_function = self.add_function(None);
        let root_scope = self.add_scope(None, root_function, ScopeKind::Global);
        self.analyze_statements(&program.statements, root_scope, root_scope, root_function)?;
        let unresolved_count = self.unresolved_count();
        Ok(BindingLayout {
            operands: Rc::from(self.operands.clone().into_boxed_slice()),
            static_functions: Rc::from(self.static_functions.clone().into_boxed_slice()),
            scopes: Rc::from(self.scopes.clone().into_boxed_slice()),
            functions: Rc::from(self.functions.clone().into_boxed_slice()),
            global_slot_count: self.global_slot_count,
            local_slot_count: self.local_slot_count,
            upvalue_slot_count: self.upvalue_slot_count,
            unresolved_count,
        })
    }

    fn analyze_statements(
        &mut self,
        statements: &[Stmt],
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
        statements: &[Stmt],
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

    fn collect_direct_declaration(
        &mut self,
        statement: &Stmt,
        scope: ScopeId,
        var_scope: ScopeId,
    ) -> Result<()> {
        match statement {
            Stmt::DeclList(statements) => {
                for declaration in statements {
                    self.collect_direct_declaration(declaration, scope, var_scope)?;
                }
                Ok(())
            }
            Stmt::VarDecl { name, kind, .. } => match kind {
                DeclKind::Var => self.declare(var_scope, name),
                DeclKind::Let | DeclKind::Const => self.declare(scope, name),
            },
            Stmt::FunctionDecl { .. }
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
            | Stmt::Expr(_) => Ok(()),
        }
    }

    fn collect_hoisted_vars(&mut self, statement: &Stmt, var_scope: ScopeId) -> Result<()> {
        match statement {
            Stmt::Block(statements) | Stmt::DeclList(statements) => {
                for statement in statements {
                    self.collect_hoisted_vars(statement, var_scope)?;
                }
                Ok(())
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
            Stmt::While { body, .. } => self.collect_hoisted_vars(body, var_scope),
            Stmt::For { init, body, .. } => {
                if let Some(init) = init {
                    self.collect_hoisted_vars(init, var_scope)?;
                }
                self.collect_hoisted_vars(body, var_scope)
            }
            Stmt::ForIn { target, body, .. } => {
                if let ForInTarget::Binding {
                    name,
                    kind: DeclKind::Var,
                } = target
                {
                    self.declare(var_scope, name)?;
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
                for statement in body {
                    self.collect_hoisted_vars(statement, var_scope)?;
                }
                if let Some(catch) = catch {
                    for statement in &catch.body {
                        self.collect_hoisted_vars(statement, var_scope)?;
                    }
                }
                if let Some(finally_body) = finally_body {
                    for statement in finally_body {
                        self.collect_hoisted_vars(statement, var_scope)?;
                    }
                }
                Ok(())
            }
            Stmt::VarDecl {
                name,
                kind: DeclKind::Var,
                ..
            }
            | Stmt::FunctionDecl { name, .. } => self.declare(var_scope, name),
            Stmt::VarDecl { .. }
            | Stmt::Break
            | Stmt::Continue
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::Expr(_) => Ok(()),
        }
    }

    fn analyze_statement(
        &mut self,
        statement: &Stmt,
        scope: ScopeId,
        var_scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        match statement {
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
            Stmt::While { condition, body } => {
                self.analyze_expr(condition, scope, function)?;
                self.analyze_statement(body, scope, var_scope, function)
            }
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
            Stmt::Return(None) | Stmt::Break | Stmt::Continue => Ok(()),
            Stmt::FunctionDecl {
                id, params, body, ..
            } => self.analyze_function(*id, params, body, scope, function),
            Stmt::VarDecl { init, .. } => {
                if let Some(init) = init {
                    self.analyze_expr(init, scope, function)?;
                }
                Ok(())
            }
        }
    }

    fn analyze_for(
        &mut self,
        init: Option<&Stmt>,
        condition: Option<&Expr>,
        update: Option<&Expr>,
        body: &Stmt,
        context: ScopeContext,
    ) -> Result<()> {
        let loop_scope = if for_init_needs_layout_scope(init) {
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
        object: &Expr,
        body: &Stmt,
        scope: ScopeId,
        var_scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        self.analyze_expr(object, scope, function)?;
        match target {
            ForInTarget::Binding {
                name,
                kind: DeclKind::Let | DeclKind::Const,
            } => {
                let loop_scope = self.add_scope(Some(scope), function, ScopeKind::Local);
                self.declare(loop_scope, name)?;
                self.analyze_statement(body, loop_scope, var_scope, function)
            }
            ForInTarget::Binding {
                name,
                kind: DeclKind::Var,
            } => {
                self.declare(var_scope, name)?;
                self.analyze_statement(body, scope, var_scope, function)
            }
            ForInTarget::Assignment(target) => {
                self.analyze_expr(target, scope, function)?;
                self.analyze_statement(body, scope, var_scope, function)
            }
        }
    }

    fn analyze_switch(
        &mut self,
        discriminant: &Expr,
        cases: &[SwitchCase],
        scope: ScopeId,
        var_scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        self.analyze_expr(discriminant, scope, function)?;
        let switch_scope = self.add_scope(Some(scope), function, ScopeKind::Local);
        for case in cases {
            if let Some(test) = &case.test {
                self.analyze_expr(test, scope, function)?;
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
        body: &[Stmt],
        catch: Option<&CatchClause>,
        finally_body: Option<&[Stmt]>,
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
            self.declare(catch_scope, param)?;
        }
        let body_scope = self.add_scope(Some(catch_scope), function, ScopeKind::Local);
        self.analyze_statements(&catch.body, body_scope, var_scope, function)
    }

    fn analyze_expr(
        &mut self,
        expr: &Expr,
        scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        match expr {
            Expr::Literal(_) | Expr::StringLiteral(_) | Expr::This => Ok(()),
            Expr::Identifier(binding) => self.resolve(binding, scope, function),
            Expr::Parenthesized(expr) | Expr::Unary { expr, .. } | Expr::Update { expr, .. } => {
                self.analyze_expr(expr, scope, function)
            }
            Expr::Binary { left, right, .. } => {
                self.analyze_expr(left, scope, function)?;
                self.analyze_expr(right, scope, function)
            }
            Expr::Conditional {
                condition,
                consequent,
                alternate,
            } => {
                self.analyze_expr(condition, scope, function)?;
                self.analyze_expr(consequent, scope, function)?;
                self.analyze_expr(alternate, scope, function)
            }
            Expr::Assignment { name, expr } => {
                self.resolve(name, scope, function)?;
                self.analyze_expr(expr, scope, function)
            }
            Expr::CompoundAssignment { target, expr, .. } => {
                self.analyze_expr(target, scope, function)?;
                self.analyze_expr(expr, scope, function)
            }
            Expr::PropertyAssignment { object, expr, .. } => {
                self.analyze_expr(object, scope, function)?;
                self.analyze_expr(expr, scope, function)
            }
            Expr::ComputedPropertyAssignment {
                object,
                property,
                expr,
                ..
            } => {
                self.analyze_expr(object, scope, function)?;
                self.analyze_expr(property, scope, function)?;
                self.analyze_expr(expr, scope, function)
            }
            Expr::Member { object, .. } => self.analyze_expr(object, scope, function),
            Expr::ComputedMember {
                object, property, ..
            } => {
                self.analyze_expr(object, scope, function)?;
                self.analyze_expr(property, scope, function)
            }
            Expr::Call { callee, args } => {
                self.analyze_expr(callee, scope, function)?;
                self.analyze_exprs(args, scope, function)
            }
            Expr::Function {
                id, params, body, ..
            }
            | Expr::MethodFunction {
                id, params, body, ..
            } => self.analyze_function(*id, params, body, scope, function),
            Expr::Object(properties) => {
                for property in properties {
                    self.analyze_expr(&property.value, scope, function)?;
                }
                Ok(())
            }
            Expr::Array(elements) => self.analyze_exprs(elements, scope, function),
            Expr::New { constructor, args } => {
                self.resolve(constructor, scope, function)?;
                self.analyze_exprs(args, scope, function)
            }
        }
    }

    fn analyze_exprs(
        &mut self,
        exprs: &[Expr],
        scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        for expr in exprs {
            self.analyze_expr(expr, scope, function)?;
        }
        Ok(())
    }

    fn analyze_function(
        &mut self,
        id: StaticFunctionId,
        params: &[StaticBinding],
        body: &[Stmt],
        parent_scope: ScopeId,
        parent_function: FunctionScopeId,
    ) -> Result<()> {
        let function = self.add_function(Some(parent_function));
        self.record_static_function(id, function)?;
        let function_scope = self.add_scope(Some(parent_scope), function, ScopeKind::Local);
        for param in params {
            self.declare(function_scope, param)?;
        }
        self.analyze_statements(body, function_scope, function_scope, function)
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

    fn set_operand(&mut self, binding: &StaticBinding, operand: BindingOperand) -> Result<()> {
        let index = binding.id().index()?;
        let Some(slot) = self.operands.get_mut(index) else {
            return Err(Error::runtime("static binding operand slot is not defined"));
        };
        *slot = operand;
        Ok(())
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

    fn unresolved_count(&self) -> usize {
        self.operands
            .iter()
            .filter(|operand| matches!(operand, BindingOperand::Unresolved))
            .count()
    }

    fn scope(&self, id: ScopeId) -> Result<&Scope> {
        self.scopes
            .get(id.index())
            .ok_or_else(|| Error::runtime("binding layout scope is not defined"))
    }

    fn scope_mut(&mut self, id: ScopeId) -> Result<&mut Scope> {
        self.scopes
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("binding layout scope is not defined"))
    }

    fn function(&self, id: FunctionScopeId) -> Result<&FunctionScope> {
        self.functions
            .get(id.index())
            .ok_or_else(|| Error::runtime("binding layout function is not defined"))
    }

    fn function_mut(&mut self, id: FunctionScopeId) -> Result<&mut FunctionScope> {
        self.functions
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("binding layout function is not defined"))
    }
}

fn for_init_needs_layout_scope(init: Option<&Stmt>) -> bool {
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
            | Stmt::FunctionDecl { .. }
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
