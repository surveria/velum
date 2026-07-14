use std::{collections::BTreeSet, rc::Rc};

use crate::{
    ast::{
        AssignmentPattern, BindingPattern, CatchClause, Expr, Expression, ForInTarget,
        FunctionParam, FunctionParamTarget, ObjectProperty, ObjectPropertyKey, PatternPropertyKey,
        Statement, StaticBinding, Stmt, SwitchCase,
    },
    binding_metadata::BindingLayout,
    error::{Error, Result},
    syntax::{FunctionKind, StaticFunctionId, StaticName},
};

use super::{
    BytecodeBlock, BytecodeCompiler, BytecodeFunction, BytecodeFunctionParam,
    BytecodeFunctionParamTarget, BytecodeHoistPlan, BytecodeInstruction, BytecodeNewTargetMode,
    FunctionCompileMode,
};

mod bytecode_compile;
mod expression_collector;

struct FunctionCompileSpec<'a> {
    id: StaticFunctionId,
    name: Option<StaticName>,
    self_binding: Option<StaticBinding>,
    arguments_binding: Option<StaticBinding>,
    params: &'a Rc<[FunctionParam]>,
    body: &'a [Statement],
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
                FunctionCompileMode::new(spec.kind, spec.strict),
                self.layout,
                self.source_text(self.current_span),
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
                    FunctionCompileMode::new(*kind, *strict),
                    self.layout,
                    self.source_text(statement.span()),
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
                Stmt::ImportBinding { name } => {
                    self.emit_lexical_hoist(name, crate::syntax::DeclKind::Const)?;
                }
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

fn compile_params(
    params: &[FunctionParam],
    layout: &BindingLayout,
) -> Result<Rc<[BytecodeFunctionParam]>> {
    params
        .iter()
        .map(|param| {
            let target = match &param.target {
                FunctionParamTarget::Binding(binding) => BytecodeFunctionParamTarget::Binding(
                    super::BytecodeBinding::compile(binding, layout)?,
                ),
                FunctionParamTarget::Pattern(pattern) => {
                    let compiler = BytecodeCompiler::new(
                        layout,
                        crate::SourceSpan::point(crate::SourceId::UNKNOWN, 0),
                    );
                    BytecodeFunctionParamTarget::Pattern(Rc::new(
                        compiler.compile_pattern(pattern)?,
                    ))
                }
            };
            let default = param.default.as_ref().map_or(Ok(None), |expr| {
                param.target.binding().map_or_else(
                    || BytecodeBlock::compile_expression(expr, layout).map(Some),
                    |binding| {
                        BytecodeBlock::compile_expression_with_inferred_name(
                            expr,
                            binding.name(),
                            layout,
                        )
                        .map(Some)
                    },
                )
            })?;
            Ok(BytecodeFunctionParam::new(target, default, param.rest))
        })
        .collect::<Result<Vec<_>>>()
        .map(Rc::from)
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
            constructable: kind.is_constructable(),
            kind: *kind,
            strict: *strict,
            new_target_mode: BytecodeNewTargetMode::Own,
        }),
        Expr::ArrowFunction {
            id,
            params,
            body,
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
    contains_direct_eval: bool,
    requires_dynamic_lexical_capture: bool,
    inside_nested_function: bool,
}

/// Free bindings referenced by a function body plus whether the body reads
/// the implicit `arguments` binding.
struct CollectedFunctionBindings {
    bindings: Rc<[StaticBinding]>,
    uses_arguments: bool,
    contains_direct_eval: bool,
    requires_dynamic_lexical_capture: bool,
}

const ARGUMENTS_BINDING_NAME: &str = "arguments";

impl CaptureBindingCollector {
    fn collect_param_defaults(&mut self, params: &[FunctionParam]) {
        for param in params {
            if let Some(default) = &param.default {
                self.collect_expr(default);
            }
            if let Some(pattern) = param.target.pattern() {
                self.collect_pattern(pattern);
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
            Stmt::Return(expr) | Stmt::VarDecl { init: expr, .. } => {
                if let Some(expr) = expr {
                    self.collect_expr(expr);
                }
            }
            Stmt::FunctionDecl { params, body, .. } => self.collect_function_body(params, body),
            Stmt::PatternDecl { pattern, init, .. } => {
                self.collect_expr(init);
                self.collect_pattern(pattern);
            }
            Stmt::ClassDecl { class, .. } => self.collect_class(class),
            Stmt::Empty
            | Stmt::Debugger
            | Stmt::ImportBinding { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_) => {}
        }
    }

    fn collect_for_in_target(&mut self, target: &ForInTarget) {
        match target {
            ForInTarget::Binding { initializer, .. } => {
                if let Some(initializer) = initializer {
                    self.collect_expr(initializer);
                }
            }
            ForInTarget::PatternBinding { pattern, .. } => self.collect_pattern(pattern),
            ForInTarget::PatternAssignment { pattern, .. } => {
                self.collect_assignment_pattern(pattern);
            }
            ForInTarget::Assignment { target, .. } => self.collect_expr(target),
        }
    }

    fn collect_class(&mut self, class: &crate::ast::ClassLiteral) {
        self.collect_exprs(&class.decorators);
        if let Some(heritage) = &class.heritage {
            self.collect_expr(heritage);
        }
        self.collect_function_body(&class.constructor.params, &class.constructor.body);
        for member in &class.members {
            self.collect_exprs(&member.decorators);
            if let crate::ast::ClassElementName::Property(
                crate::ast::ObjectPropertyKey::Computed(key),
            ) = &member.key
            {
                self.collect_expr(key);
            }
            self.collect_function_body(&member.params, &member.body);
        }
        for field in &class.fields {
            self.collect_exprs(&field.decorators);
            if let crate::ast::ClassElementName::Property(
                crate::ast::ObjectPropertyKey::Computed(key),
            ) = &field.key
            {
                self.collect_expr(key);
            }
            if let Some(initializer) = &field.initializer {
                self.collect_nested_expr(initializer);
            }
            if let Some(auto_accessor) = &field.auto_accessor {
                self.collect_function_body(
                    &auto_accessor.getter.params,
                    &auto_accessor.getter.body,
                );
                self.collect_function_body(
                    &auto_accessor.setter.params,
                    &auto_accessor.setter.body,
                );
            }
        }
        for block in &class.static_blocks {
            self.collect_nested_statements(&block.body);
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
            | Expr::StringLiteral { .. }
            | Expr::TemplateObject { .. }
            | Expr::RegExpLiteral { .. }
            | Expr::This
            | Expr::ImportMeta
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
            Expr::Parenthesized(expr)
            | Expr::OptionalChain(expr)
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
            } => self.collect_conditional_expr(condition, consequent, alternate),
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
            Expr::WebCompatCallAssignment { target, discarded } => {
                self.collect_web_compat_call_assignment(target, discarded.as_deref());
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
            } => self.collect_computed_property_assignment(object, property, expr),
            Expr::SuperPropertyAssignment { expr, .. } => self.collect_expr(expr),
            Expr::SuperComputedPropertyAssignment { property, expr, .. } => {
                self.collect_expr(property);
                self.collect_expr(expr);
            }
            Expr::Member { object, .. }
            | Expr::OptionalMember { object, .. }
            | Expr::PrivateMember { object, .. }
            | Expr::OptionalPrivateMember { object, .. }
            | Expr::PrivateIn { object, .. } => self.collect_expr(object),
            Expr::ComputedMember {
                object, property, ..
            }
            | Expr::OptionalComputedMember {
                object, property, ..
            } => {
                self.collect_expr(object);
                self.collect_expr(property);
            }
            Expr::Call { .. } | Expr::OptionalCall { .. } | Expr::New { .. } => {
                self.collect_call_like_expr(expr.kind());
            }
            Expr::DynamicImport {
                specifier, options, ..
            } => self.collect_dynamic_import(specifier, options.as_deref()),
            Expr::Object(properties) => self.collect_object_properties(properties),
            Expr::Array(elements) => self.collect_exprs(elements),
        }
    }

    fn collect_dynamic_import(&mut self, specifier: &Expression, options: Option<&Expression>) {
        self.collect_expr(specifier);
        if let Some(options) = options {
            self.collect_expr(options);
        }
    }

    fn collect_computed_property_assignment(
        &mut self,
        object: &Expression,
        property: &Expression,
        expr: &Expression,
    ) {
        self.collect_expr(object);
        self.collect_expr(property);
        self.collect_expr(expr);
    }

    fn collect_web_compat_call_assignment(
        &mut self,
        target: &Expression,
        discarded: Option<&Expression>,
    ) {
        self.collect_expr(target);
        if let Some(discarded) = discarded {
            self.collect_expr(discarded);
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
        let was_inside_nested_function = self.inside_nested_function;
        self.inside_nested_function = true;
        self.collect_param_defaults(params);
        self.collect_statements(body);
        self.inside_nested_function = was_inside_nested_function;
    }

    fn collect_nested_expr(&mut self, expr: &Expression) {
        let was_inside_nested_function = self.inside_nested_function;
        self.inside_nested_function = true;
        self.collect_expr(expr);
        self.inside_nested_function = was_inside_nested_function;
    }

    fn collect_nested_statements(&mut self, statements: &[Statement]) {
        let was_inside_nested_function = self.inside_nested_function;
        self.inside_nested_function = true;
        self.collect_statements(statements);
        self.inside_nested_function = was_inside_nested_function;
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
