use std::{collections::BTreeSet, rc::Rc};

use crate::{
    ast::{
        AssignmentPattern, BindingPattern, CatchClause, Expr, Expression, ForInTarget,
        FunctionParam, ObjectProperty, ObjectPropertyKey, PatternPropertyKey, Statement,
        StaticBinding, Stmt, SwitchCase,
    },
    binding_metadata::BindingLayout,
    bytecode::BytecodeFunctionInit,
    error::{Error, Result},
    syntax::{FunctionKind, StaticFunctionId, StaticName},
};

use super::{
    BytecodeBlock, BytecodeCompiler, BytecodeFunction, BytecodeFunctionParam, BytecodeHoistPlan,
    BytecodeInstruction, BytecodeNewTargetMode, FunctionCompileMode,
};

struct FunctionCompileSpec<'a> {
    id: StaticFunctionId,
    name: Option<StaticName>,
    self_binding: Option<StaticBinding>,
    arguments_binding: Option<StaticBinding>,
    params: &'a Rc<[FunctionParam]>,
    body: &'a [Statement],
    parameter_prologue_count: usize,
    constructable: bool,
    kind: FunctionKind,
    strict: bool,
    new_target_mode: BytecodeNewTargetMode,
}

impl BytecodeCompiler<'_> {
    pub(super) fn compile_function_literal(&mut self, expr: &Expr) -> Result<()> {
        let spec = function_compile_spec(expr, None)?;
        self.compile_function_expr(spec)
    }

    pub(super) fn compile_function_literal_with_inferred_name(
        &mut self,
        expr: &Expr,
        inferred_name: &StaticName,
    ) -> Result<()> {
        let spec = function_compile_spec(expr, Some(inferred_name))?;
        self.compile_function_expr(spec)
    }

    fn compile_function_expr(&mut self, spec: FunctionCompileSpec<'_>) -> Result<()> {
        self.emit(BytecodeInstruction::CreateFunction {
            id: spec.id,
            name: spec.name,
            bytecode: BytecodeFunction::compile(
                spec.self_binding,
                spec.arguments_binding,
                spec.params,
                spec.body,
                spec.parameter_prologue_count,
                FunctionCompileMode::new(spec.kind, spec.strict),
                self.layout,
            )?,
            constructable: spec.constructable,
            kind: spec.kind,
            new_target_mode: spec.new_target_mode,
        });
        Ok(())
    }

    pub(super) fn compile_block_function_initializers(
        &mut self,
        statements: &[Statement],
    ) -> Result<()> {
        self.compile_block_lexical_hoists(statements)?;
        for (index, statement) in statements.iter().enumerate() {
            let Stmt::FunctionDecl {
                name,
                arguments_binding,
                id,
                params,
                body,
                parameter_prologue_count,
                kind,
                strict,
                block_scoped: true,
                ..
            } = statement.kind()
            else {
                continue;
            };
            if statement.kind().is_annex_b_function()
                && statements
                    .iter()
                    .skip(index.saturating_add(1))
                    .any(|later| {
                        matches!(
                            later.kind(),
                            Stmt::FunctionDecl {
                                name: later_name,
                                annex_b_var_binding: Some(_),
                                block_scoped: true,
                                ..
                            } if later_name.name() == name.name()
                        )
                    })
            {
                continue;
            }
            let binding = self.compile_binding(name)?;
            self.emit(BytecodeInstruction::CreateFunction {
                id: *id,
                name: Some(name.name().clone()),
                bytecode: BytecodeFunction::compile(
                    None,
                    arguments_binding.clone(),
                    params,
                    body,
                    *parameter_prologue_count,
                    FunctionCompileMode::new(*kind, *strict),
                    self.layout,
                )?,
                constructable: kind.is_constructable(),
                kind: *kind,
                new_target_mode: BytecodeNewTargetMode::Own,
            });
            self.emit(BytecodeInstruction::DeclareBinding {
                name: binding,
                kind: crate::syntax::DeclKind::Let,
                has_init: true,
            });
        }
        Ok(())
    }

    fn compile_block_lexical_hoists(&mut self, statements: &[Statement]) -> Result<()> {
        let mut annex_b_functions = BTreeSet::new();
        self.compile_block_lexical_hoists_with_names(statements, &mut annex_b_functions)
    }

    fn compile_block_lexical_hoists_with_names(
        &mut self,
        statements: &[Statement],
        annex_b_functions: &mut BTreeSet<crate::syntax::StaticNameId>,
    ) -> Result<()> {
        for statement in statements {
            match statement.kind() {
                Stmt::DeclList(declarations) => {
                    self.compile_block_lexical_hoists_with_names(declarations, annex_b_functions)?;
                }
                Stmt::VarDecl {
                    name,
                    kind:
                        kind @ (crate::syntax::DeclKind::Let
                        | crate::syntax::DeclKind::Const
                        | crate::syntax::DeclKind::Using
                        | crate::syntax::DeclKind::AwaitUsing),
                    ..
                } => self.emit_lexical_hoist(name, *kind)?,
                Stmt::PatternDecl {
                    pattern,
                    kind:
                        kind @ (crate::syntax::DeclKind::Let
                        | crate::syntax::DeclKind::Const
                        | crate::syntax::DeclKind::Using
                        | crate::syntax::DeclKind::AwaitUsing),
                    ..
                } => pattern
                    .for_each_binding(&mut |binding| self.emit_lexical_hoist(binding, *kind))?,
                Stmt::ClassDecl { name, .. } => {
                    self.emit_lexical_hoist(name, crate::syntax::DeclKind::Let)?;
                }
                Stmt::FunctionDecl {
                    name,
                    block_scoped: true,
                    annex_b_var_binding,
                    ..
                } if annex_b_var_binding.is_none()
                    || annex_b_functions.insert(name.name().id()) =>
                {
                    self.emit_lexical_hoist(name, crate::syntax::DeclKind::Let)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn emit_lexical_hoist(
        &mut self,
        name: &StaticBinding,
        kind: crate::syntax::DeclKind,
    ) -> Result<()> {
        self.emit(BytecodeInstruction::HoistLexicalBinding {
            name: self.compile_binding(name)?,
            kind,
        });
        Ok(())
    }

    pub(super) fn compile_annex_b_function_update(
        &mut self,
        lexical: &StaticBinding,
        variable: &StaticBinding,
    ) -> Result<()> {
        let variable = self.compile_binding(variable)?;
        if variable.operand() == crate::binding_metadata::BindingOperand::Unresolved {
            return Ok(());
        }
        self.emit(BytecodeInstruction::LoadBinding(
            self.compile_binding(lexical)?,
        ));
        self.emit(BytecodeInstruction::StoreAnnexBVar(
            variable.name().name().clone(),
        ));
        self.emit(BytecodeInstruction::Pop);
        Ok(())
    }
}

trait AnnexBFunctionStatement {
    fn is_annex_b_function(&self) -> bool;
}

impl AnnexBFunctionStatement for Stmt {
    fn is_annex_b_function(&self) -> bool {
        matches!(
            self,
            Self::FunctionDecl {
                annex_b_var_binding: Some(_),
                block_scoped: true,
                ..
            }
        )
    }
}

impl BytecodeFunction {
    pub(super) fn compile(
        self_binding: Option<StaticBinding>,
        arguments_binding: Option<StaticBinding>,
        params: &[FunctionParam],
        statements: &[Statement],
        parameter_prologue_count: usize,
        mode: FunctionCompileMode,
        layout: &BindingLayout,
    ) -> Result<Self> {
        let collected = CaptureBindingCollector::collect_function(params, statements);
        Ok(Self::new(BytecodeFunctionInit {
            self_binding,
            arguments_binding,
            params: compile_params(params),
            param_defaults: compile_param_defaults(params, layout)?,
            body: BytecodeBlock::compile_function_statements(
                statements,
                mode.kind,
                parameter_prologue_count,
                layout,
            )?,
            hoist_plan: BytecodeHoistPlan::compile(statements, layout)?,
            capture_bindings: collected.bindings,
            uses_arguments: collected.uses_arguments,
            strict: mode.strict,
            simple_parameters: parameter_prologue_count == 0
                && params
                    .iter()
                    .all(|param| param.default.is_none() && !param.rest),
        }))
    }
}

fn compile_params(params: &[FunctionParam]) -> Rc<[BytecodeFunctionParam]> {
    params
        .iter()
        .map(|param| {
            BytecodeFunctionParam::new(param.name.clone(), param.default.is_some(), param.rest)
        })
        .collect::<Vec<_>>()
        .into()
}

fn compile_param_defaults(
    params: &[FunctionParam],
    layout: &BindingLayout,
) -> Result<std::rc::Rc<[Option<BytecodeBlock>]>> {
    params
        .iter()
        .map(|param| {
            param.default.as_ref().map_or(Ok(None), |expr| {
                BytecodeBlock::compile_expression_with_inferred_name(
                    expr,
                    param.name.name(),
                    layout,
                )
                .map(Some)
            })
        })
        .collect::<Result<Vec<_>>>()
        .map(Into::into)
}

fn function_compile_spec<'a>(
    expr: &'a Expr,
    inferred_name: Option<&StaticName>,
) -> Result<FunctionCompileSpec<'a>> {
    match expr {
        Expr::Function {
            id,
            name,
            arguments_binding,
            params,
            body,
            parameter_prologue_count,
            kind,
            strict,
            ..
        } => Ok(FunctionCompileSpec {
            id: *id,
            name: name
                .as_ref()
                .map(|binding| binding.name().clone())
                .or_else(|| inferred_name.cloned()),
            self_binding: name.clone(),
            arguments_binding: arguments_binding.clone(),
            params,
            body,
            parameter_prologue_count: *parameter_prologue_count,
            constructable: kind.is_constructable(),
            kind: *kind,
            strict: *strict,
            new_target_mode: BytecodeNewTargetMode::Own,
        }),
        Expr::ArrowFunction {
            id,
            params,
            body,
            parameter_prologue_count,
            kind,
            strict,
            ..
        } => Ok(FunctionCompileSpec {
            id: *id,
            name: inferred_name.cloned(),
            self_binding: None,
            arguments_binding: None,
            params,
            body,
            parameter_prologue_count: *parameter_prologue_count,
            constructable: false,
            kind: *kind,
            strict: *strict,
            new_target_mode: BytecodeNewTargetMode::Lexical,
        }),
        Expr::MethodFunction {
            id,
            name,
            arguments_binding,
            params,
            body,
            parameter_prologue_count,
            kind,
            strict,
            ..
        } => Ok(FunctionCompileSpec {
            id: *id,
            name: name.clone(),
            self_binding: None,
            arguments_binding: arguments_binding.clone(),
            params,
            body,
            parameter_prologue_count: *parameter_prologue_count,
            constructable: false,
            kind: *kind,
            strict: *strict,
            new_target_mode: BytecodeNewTargetMode::Own,
        }),
        _ => Err(Error::runtime("expected function expression")),
    }
}

#[derive(Debug, Default)]
struct CaptureBindingCollector {
    bindings: Vec<StaticBinding>,
    uses_arguments: bool,
}

/// Free bindings referenced by a function body plus whether the body reads
/// the implicit `arguments` binding.
struct CollectedFunctionBindings {
    bindings: Rc<[StaticBinding]>,
    uses_arguments: bool,
}

const ARGUMENTS_BINDING_NAME: &str = "arguments";

impl CaptureBindingCollector {
    fn collect_function(
        params: &[FunctionParam],
        statements: &[Statement],
    ) -> CollectedFunctionBindings {
        let mut collector = Self::default();
        collector.collect_param_defaults(params);
        collector.collect_statements(statements);
        CollectedFunctionBindings {
            bindings: Rc::from(collector.bindings.into_boxed_slice()),
            uses_arguments: collector.uses_arguments,
        }
    }

    fn collect_param_defaults(&mut self, params: &[FunctionParam]) {
        for param in params {
            if let Some(default) = &param.default {
                self.collect_expr(default);
            }
        }
    }

    fn collect_statements(&mut self, statements: &[Statement]) {
        for statement in statements {
            self.collect_statement(statement);
        }
    }

    fn collect_statement(&mut self, statement: &Statement) {
        match statement.kind() {
            Stmt::Block(statements) | Stmt::DeclList(statements) => {
                self.collect_statements(statements);
            }
            Stmt::If {
                condition,
                consequent,
                alternate,
            } => {
                self.collect_expr(condition);
                self.collect_statement(consequent);
                if let Some(alternate) = alternate {
                    self.collect_statement(alternate);
                }
            }
            Stmt::While { condition, body } => {
                self.collect_expr(condition);
                self.collect_statement(body);
            }
            Stmt::DoWhile { body, condition } => {
                self.collect_statement(body);
                self.collect_expr(condition);
            }
            Stmt::With { object, body } => {
                self.collect_expr(object);
                self.collect_statement(body);
            }
            Stmt::Label { body, .. } => self.collect_statement(body),
            Stmt::For {
                init,
                condition,
                update,
                body,
            } => {
                if let Some(init) = init {
                    self.collect_statement(init);
                }
                if let Some(condition) = condition {
                    self.collect_expr(condition);
                }
                if let Some(update) = update {
                    self.collect_expr(update);
                }
                self.collect_statement(body);
            }
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
            } => {
                self.collect_for_in_target(target);
                self.collect_expr(object);
                self.collect_statement(body);
            }
            Stmt::Switch {
                discriminant,
                cases,
            } => {
                self.collect_expr(discriminant);
                self.collect_switch_cases(cases);
            }
            Stmt::Try {
                body,
                catch,
                finally_body,
            } => {
                self.collect_statements(body);
                if let Some(catch) = catch {
                    self.collect_catch(catch);
                }
                if let Some(finally_body) = finally_body {
                    self.collect_statements(finally_body);
                }
            }
            Stmt::Throw(expr) | Stmt::Expr(expr) => self.collect_expr(expr),
            Stmt::Return(expr) => {
                if let Some(expr) = expr {
                    self.collect_expr(expr);
                }
            }
            Stmt::FunctionDecl { params, body, .. } => self.collect_function_body(params, body),
            Stmt::VarDecl { init, .. } => {
                if let Some(init) = init {
                    self.collect_expr(init);
                }
            }
            Stmt::PatternDecl { pattern, init, .. } => {
                self.collect_expr(init);
                self.collect_pattern(pattern);
            }
            Stmt::ClassDecl { class, .. } => self.collect_class(class),
            Stmt::Empty | Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn collect_for_in_target(&mut self, target: &ForInTarget) {
        match target {
            ForInTarget::Binding { .. } => {}
            ForInTarget::PatternBinding { pattern, .. } => self.collect_pattern(pattern),
            ForInTarget::PatternAssignment { pattern, .. } => {
                self.collect_assignment_pattern(pattern);
            }
            ForInTarget::Assignment(expr) => self.collect_expr(expr),
        }
    }

    fn collect_class(&mut self, class: &crate::ast::ClassLiteral) {
        if let Some(heritage) = &class.heritage {
            self.collect_expr(heritage);
        }
        self.collect_function_body(&class.constructor.params, &class.constructor.body);
        for member in &class.members {
            if let crate::ast::ClassElementName::Property(
                crate::ast::ObjectPropertyKey::Computed(key),
            ) = &member.key
            {
                self.collect_expr(key);
            }
            self.collect_function_body(&member.params, &member.body);
        }
        for field in &class.fields {
            if let crate::ast::ClassElementName::Property(
                crate::ast::ObjectPropertyKey::Computed(key),
            ) = &field.key
            {
                self.collect_expr(key);
            }
            if let Some(initializer) = &field.initializer {
                self.collect_expr(initializer);
            }
        }
        for block in &class.static_blocks {
            self.collect_statements(&block.body);
        }
    }

    fn collect_pattern(&mut self, pattern: &BindingPattern) {
        let mut visit = |expr: &Expression| -> std::result::Result<(), std::convert::Infallible> {
            self.collect_expr(expr);
            Ok(())
        };
        match pattern.for_each_expr(&mut visit) {
            Ok(()) => {}
        }
    }

    fn collect_switch_cases(&mut self, cases: &[SwitchCase]) {
        for case in cases {
            if let Some(test) = &case.test {
                self.collect_expr(test);
            }
            self.collect_statements(&case.statements);
        }
    }

    fn collect_catch(&mut self, catch: &CatchClause) {
        if let Some(param) = &catch.param {
            self.collect_pattern(param);
        }
        self.collect_statements(&catch.body);
    }

    fn collect_expr(&mut self, expr: &Expression) {
        match expr.kind() {
            Expr::Literal(_)
            | Expr::StringLiteral(_)
            | Expr::RegExpLiteral { .. }
            | Expr::This
            | Expr::SuperMember { .. }
            | Expr::NewTarget
            | Expr::ArrayHole => {}
            Expr::SuperComputedMember { property, .. } => self.collect_expr(property),
            Expr::TemplateLiteral { expressions, .. } | Expr::Sequence(expressions) => {
                self.collect_exprs(expressions);
            }
            Expr::Function { params, body, .. }
            | Expr::ArrowFunction { params, body, .. }
            | Expr::MethodFunction { params, body, .. } => self.collect_function_body(params, body),
            Expr::Class(class) => self.collect_class(class),
            Expr::SuperCall { args } => self.collect_exprs(args),
            Expr::Identifier(binding) => self.collect_binding(binding),
            Expr::New { constructor, args } => {
                self.collect_expr(constructor);
                self.collect_exprs(args);
            }
            Expr::Parenthesized(expr)
            | Expr::Spread(expr)
            | Expr::Unary { expr, .. }
            | Expr::Update { expr, .. }
            | Expr::Await(expr) => {
                self.collect_expr(expr);
            }
            Expr::Yield { expr, .. } => {
                if let Some(expr) = expr {
                    self.collect_expr(expr);
                }
            }
            Expr::Binary { left, right, .. } => {
                self.collect_expr(left);
                self.collect_expr(right);
            }
            Expr::Conditional {
                condition,
                consequent,
                alternate,
            } => {
                self.collect_expr(condition);
                self.collect_expr(consequent);
                self.collect_expr(alternate);
            }
            Expr::Assignment { name, expr, .. } => {
                self.collect_binding(name);
                self.collect_expr(expr);
            }
            Expr::DestructuringAssignment { pattern, expr, .. } => {
                self.collect_assignment_pattern(pattern);
                self.collect_expr(expr);
            }
            Expr::CompoundAssignment { target, expr, .. } => {
                self.collect_expr(target);
                self.collect_expr(expr);
            }
            Expr::PropertyAssignment { object, expr, .. }
            | Expr::PrivateAssignment { object, expr, .. } => {
                self.collect_expr(object);
                self.collect_expr(expr);
            }
            Expr::ComputedPropertyAssignment {
                object,
                property,
                expr,
                ..
            } => {
                self.collect_expr(object);
                self.collect_expr(property);
                self.collect_expr(expr);
            }
            Expr::SuperPropertyAssignment { expr, .. } => self.collect_expr(expr),
            Expr::SuperComputedPropertyAssignment { property, expr, .. } => {
                self.collect_expr(property);
                self.collect_expr(expr);
            }
            Expr::Member { object, .. }
            | Expr::PrivateMember { object, .. }
            | Expr::PrivateIn { object, .. } => self.collect_expr(object),
            Expr::ComputedMember {
                object, property, ..
            } => {
                self.collect_expr(object);
                self.collect_expr(property);
            }
            Expr::Call { callee, args, .. } => {
                self.collect_expr(callee);
                self.collect_exprs(args);
            }
            Expr::Object(properties) => self.collect_object_properties(properties),
            Expr::Array(elements) => self.collect_exprs(elements),
        }
    }

    fn collect_object_properties(&mut self, properties: &[ObjectProperty]) {
        for property in properties {
            if let ObjectPropertyKey::Computed(expr) = &property.key {
                self.collect_expr(expr);
            }
            self.collect_expr(&property.value);
        }
    }

    fn collect_exprs(&mut self, exprs: &[Expression]) {
        for expr in exprs {
            self.collect_expr(expr);
        }
    }

    fn collect_assignment_pattern(&mut self, pattern: &AssignmentPattern) {
        match pattern {
            AssignmentPattern::Target(target) => self.collect_expr(target),
            AssignmentPattern::Object { properties, rest } => {
                for property in properties {
                    if let PatternPropertyKey::Computed(key) = &property.key {
                        self.collect_expr(key);
                    }
                    if let Some(default) = &property.default {
                        self.collect_expr(default);
                    }
                    self.collect_assignment_pattern(&property.target);
                }
                if let Some(rest) = rest {
                    self.collect_expr(rest);
                }
            }
            AssignmentPattern::Array { elements, rest } => {
                for element in elements.iter().flatten() {
                    if let Some(default) = &element.default {
                        self.collect_expr(default);
                    }
                    self.collect_assignment_pattern(&element.target);
                }
                if let Some(rest) = rest {
                    self.collect_assignment_pattern(rest);
                }
            }
        }
    }

    fn collect_function_body(&mut self, params: &[FunctionParam], body: &[Statement]) {
        self.collect_param_defaults(params);
        self.collect_statements(body);
    }

    fn collect_binding(&mut self, binding: &StaticBinding) {
        if binding.as_str() == ARGUMENTS_BINDING_NAME {
            self.uses_arguments = true;
        }
        if self
            .bindings
            .iter()
            .any(|existing| existing.id() == binding.id())
        {
            return;
        }
        self.bindings.push(binding.clone());
    }
}
