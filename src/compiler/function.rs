use std::rc::Rc;

use crate::{
    ast::{
        AssignmentPattern, BindingPattern, CatchClause, Expr, Expression, ForInTarget,
        FunctionParam, ObjectProperty, ObjectPropertyKey, PatternPropertyKey, Statement,
        StaticBinding, Stmt, SwitchCase,
    },
    binding_metadata::BindingLayout,
    error::{Error, Result},
    syntax::{FunctionKind, StaticFunctionId, StaticName},
};

use super::{
    BytecodeBlock, BytecodeCompiler, BytecodeFunction, BytecodeFunctionParam, BytecodeHoistPlan,
    BytecodeInstruction, BytecodeNewTargetMode,
};

struct FunctionCompileSpec<'a> {
    id: StaticFunctionId,
    name: Option<StaticName>,
    self_binding: Option<StaticBinding>,
    params: &'a Rc<[FunctionParam]>,
    body: &'a [Statement],
    parameter_prologue_count: usize,
    constructable: bool,
    kind: FunctionKind,
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
                spec.params,
                spec.body,
                spec.kind,
                spec.parameter_prologue_count,
                self.layout,
            )?,
            constructable: spec.constructable,
            kind: spec.kind,
            new_target_mode: spec.new_target_mode,
        });
        Ok(())
    }
}

impl BytecodeFunction {
    pub fn compile(
        self_binding: Option<StaticBinding>,
        params: &[FunctionParam],
        statements: &[Statement],
        kind: FunctionKind,
        parameter_prologue_count: usize,
        layout: &BindingLayout,
    ) -> Result<Self> {
        let collected = CaptureBindingCollector::collect_function(params, statements);
        Ok(Self::new(
            self_binding,
            compile_params(params),
            compile_param_defaults(params, layout)?,
            BytecodeBlock::compile_function_statements(
                statements,
                kind,
                parameter_prologue_count,
                layout,
            )?,
            BytecodeHoistPlan::compile(statements, layout)?,
            collected.bindings,
            collected.uses_arguments,
        ))
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
            params,
            body,
            parameter_prologue_count,
            kind,
        } => Ok(FunctionCompileSpec {
            id: *id,
            name: name
                .as_ref()
                .map(|binding| binding.name().clone())
                .or_else(|| inferred_name.cloned()),
            self_binding: name.clone(),
            params,
            body,
            parameter_prologue_count: *parameter_prologue_count,
            constructable: kind.is_constructable(),
            kind: *kind,
            new_target_mode: BytecodeNewTargetMode::Own,
        }),
        Expr::ArrowFunction {
            id,
            params,
            body,
            parameter_prologue_count,
            kind,
        } => Ok(FunctionCompileSpec {
            id: *id,
            name: inferred_name.cloned(),
            self_binding: None,
            params,
            body,
            parameter_prologue_count: *parameter_prologue_count,
            constructable: false,
            kind: *kind,
            new_target_mode: BytecodeNewTargetMode::Lexical,
        }),
        Expr::MethodFunction {
            id,
            name,
            params,
            body,
            parameter_prologue_count,
            kind,
        } => Ok(FunctionCompileSpec {
            id: *id,
            name: name.clone(),
            self_binding: None,
            params,
            body,
            parameter_prologue_count: *parameter_prologue_count,
            constructable: false,
            kind: *kind,
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
            if let crate::ast::ObjectPropertyKey::Computed(key) = &member.key {
                self.collect_expr(key);
            }
            self.collect_function_body(&member.params, &member.body);
        }
        for field in &class.fields {
            if let crate::ast::ObjectPropertyKey::Computed(key) = &field.key {
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
            Expr::PropertyAssignment { object, expr, .. } => {
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
            Expr::Member { object, .. } => self.collect_expr(object),
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
