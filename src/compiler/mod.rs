use std::rc::Rc;

use crate::{
    api::native_call::NativeCallTarget,
    ast::{
        BinaryOp, DeclKind, Expr, Expression, ObjectPropertyKey, Program, Statement, StaticBinding,
        StaticPropertyAccessId, Stmt, TemplateElement, UnaryOp, UpdateOp,
    },
    binding_metadata::BindingLayout,
    bytecode::{
        BytecodeAddress, BytecodeArrayIndex, BytecodeBinding, BytecodeBlock, BytecodeCallSite,
        BytecodeClass, BytecodeClassDefinitionElement, BytecodeClassMember, BytecodeClassMemberKey,
        BytecodeClassMemberKind, BytecodeClassStaticElement, BytecodeCompletion,
        BytecodeDestructureMode, BytecodeDynamicProperty, BytecodeFunction, BytecodeFunctionParam,
        BytecodeFunctionParamTarget, BytecodeHoistPlan, BytecodeInstruction, BytecodeNewTargetMode,
        BytecodeNumericBinaryOp, BytecodeNumericCompareOp, BytecodeNumericEqualityOp,
        BytecodeNumericUnaryOp, BytecodeProgram, BytecodeProperty, BytecodeTemplateElement,
    },
    error::{Error, Result},
    source::{SourceId, SourceSpan},
    syntax::{StaticName, StaticString},
};

mod binding_effects;
mod call;
mod class_elements;
mod class_fields;
mod control;
mod expression;
mod function;
mod hoist;
mod inferred_name;
mod member;
mod object_literal;
mod optional_chain;
mod pattern;

const ARRAY_LENGTH_PROPERTY: &str = "length";

#[derive(Clone, Copy)]
struct FunctionCompileMode {
    kind: crate::syntax::FunctionKind,
    strict: bool,
}

impl FunctionCompileMode {
    const fn new(kind: crate::syntax::FunctionKind, strict: bool) -> Self {
        Self { kind, strict }
    }
}

pub fn compile_program(program: &Program, layout: &BindingLayout) -> Result<BytecodeProgram> {
    Ok(BytecodeProgram::new(
        BytecodeBlock::compile_statements(&program.statements, StatementValue::Store, layout)?,
        BytecodeHoistPlan::compile(&program.statements, layout)?,
    ))
}

impl BytecodeBlock {
    fn compile_statements(
        statements: &[Statement],
        value: StatementValue,
        layout: &BindingLayout,
    ) -> Result<Self> {
        let fallback_span = statements
            .first()
            .map_or_else(|| SourceSpan::point(SourceId::UNKNOWN, 0), Statement::span);
        let mut compiler = BytecodeCompiler::new(layout, fallback_span);
        compiler.compile_statements(statements, value)?;
        compiler.finish()
    }

    fn compile_function_statements(
        statements: &[Statement],
        kind: crate::syntax::FunctionKind,
        layout: &BindingLayout,
    ) -> Result<Self> {
        let fallback_span = statements
            .first()
            .map_or_else(|| SourceSpan::point(SourceId::UNKNOWN, 0), Statement::span);
        let mut compiler = BytecodeCompiler::new(layout, fallback_span);
        if kind.is_generator() {
            compiler.emit(BytecodeInstruction::GeneratorStart);
        }
        compiler.compile_statements(statements, StatementValue::Store)?;
        compiler.finish()
    }

    fn compile_expression(expr: &Expression, layout: &BindingLayout) -> Result<Self> {
        let mut compiler = BytecodeCompiler::new(layout, expr.span());
        compiler.compile_expr(expr)?;
        compiler.emit(BytecodeInstruction::StoreLast);
        compiler.finish()
    }

    fn compile_scoped_statements(statements: &[Statement], layout: &BindingLayout) -> Result<Self> {
        let fallback_span = statements
            .first()
            .map_or_else(|| SourceSpan::point(SourceId::UNKNOWN, 0), Statement::span);
        let inner = Self::compile_lexical_statements(statements, StatementValue::Store, layout)?;
        let var_hoist_plan = BytecodeHoistPlan::compile(statements, layout)?;
        let mut compiler = BytecodeCompiler::new(layout, fallback_span);
        compiler.emit(BytecodeInstruction::ScopedBlock {
            block: inner,
            var_hoist_plan: Some(Rc::new(var_hoist_plan)),
            preserve_last: !statements_have_value_completion(statements),
            push_result: false,
        });
        compiler.finish()
    }

    fn compile_lexical_statements(
        statements: &[Statement],
        value: StatementValue,
        layout: &BindingLayout,
    ) -> Result<Self> {
        let fallback_span = statements
            .first()
            .map_or_else(|| SourceSpan::point(SourceId::UNKNOWN, 0), Statement::span);
        let mut compiler = BytecodeCompiler::new(layout, fallback_span);
        compiler.compile_block_function_initializers(statements)?;
        compiler.compile_statements(statements, value)?;
        compiler.finish()
    }

    fn compile_block_function_init(
        statements: &[Statement],
        layout: &BindingLayout,
    ) -> Result<Self> {
        let fallback_span = statements
            .first()
            .map_or_else(|| SourceSpan::point(SourceId::UNKNOWN, 0), Statement::span);
        let mut compiler = BytecodeCompiler::new(layout, fallback_span);
        compiler.compile_block_function_initializers(statements)?;
        compiler.finish()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum StatementValue {
    Store,
    Discard,
}

#[derive(Debug)]
struct BytecodeCompiler<'a> {
    layout: &'a BindingLayout,
    instructions: Vec<BytecodeInstruction>,
    spans: Vec<SourceSpan>,
    current_span: SourceSpan,
}

impl<'a> BytecodeCompiler<'a> {
    const fn new(layout: &'a BindingLayout, current_span: SourceSpan) -> Self {
        Self {
            layout,
            instructions: Vec::new(),
            spans: Vec::new(),
            current_span,
        }
    }

    fn finish(self) -> Result<BytecodeBlock> {
        BytecodeBlock::from_parts(self.instructions, self.spans)
    }

    fn with_source_span<T>(
        &mut self,
        span: SourceSpan,
        compile: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let previous = std::mem::replace(&mut self.current_span, span);
        let result = compile(self);
        self.current_span = previous;
        result
    }

    fn compile_binding(&self, binding: &StaticBinding) -> Result<BytecodeBinding> {
        BytecodeBinding::compile(binding, self.layout)
    }

    fn source_text(&self, span: SourceSpan) -> Option<Rc<str>> {
        self.layout
            .source_text()?
            .get(span.start()..span.end())
            .map(Rc::from)
    }

    fn compile_property(property: &StaticName, access: StaticPropertyAccessId) -> BytecodeProperty {
        BytecodeProperty::new(property.clone(), access)
    }

    fn compile_array_index(property: &BytecodeProperty) -> Option<BytecodeArrayIndex> {
        BytecodeArrayIndex::parse(property)
    }

    const fn compile_dynamic_property(access: StaticPropertyAccessId) -> BytecodeDynamicProperty {
        BytecodeDynamicProperty::new(access)
    }

    fn compile_statements(
        &mut self,
        statements: &[Statement],
        value: StatementValue,
    ) -> Result<()> {
        for statement in statements {
            self.compile_statement(statement, value)?;
        }
        Ok(())
    }

    fn compile_statement(&mut self, statement: &Statement, value: StatementValue) -> Result<()> {
        self.with_source_span(statement.span(), |compiler| {
            compiler.compile_statement_kind(statement.kind(), value)
        })
    }

    fn compile_statement_kind(&mut self, statement: &Stmt, value: StatementValue) -> Result<()> {
        match statement {
            Stmt::Block(statements) => self.compile_block_statement(statements, value),
            Stmt::DeclList(declarations) => self.compile_statements(declarations, value),
            Stmt::If {
                condition,
                consequent,
                alternate,
            } => self.compile_if(condition, consequent, alternate.as_deref(), value),
            Stmt::While { condition, body } => self.compile_while(condition, body),
            Stmt::DoWhile { body, condition } => self.compile_do_while(body, condition),
            Stmt::With { object, body } => self.compile_with(object, body, value),
            Stmt::Label { label, body } => self.compile_label(label, body, value),
            Stmt::For {
                init,
                condition,
                update,
                body,
            } => self.compile_for(init.as_deref(), condition.as_ref(), update.as_ref(), body),
            Stmt::ForIn {
                target,
                object,
                body,
            } => self.compile_for_in(target, object, body),
            Stmt::ForOf {
                target,
                object,
                body,
                asynchronous,
            } => self.compile_for_of(target, object, body, *asynchronous),
            Stmt::PatternDecl {
                pattern,
                kind,
                init,
            } => {
                self.compile_expr(init)?;
                let pattern = self.compile_pattern(pattern)?;
                self.emit(BytecodeInstruction::DestructurePattern {
                    pattern: Rc::new(pattern),
                    mode: BytecodeDestructureMode::Declaration(*kind),
                });
                Ok(())
            }
            Stmt::ClassDecl { name, class } => self.compile_class_declaration(name, class),
            Stmt::Switch {
                discriminant,
                cases,
            } => self.compile_switch(discriminant, cases),
            Stmt::Try {
                body,
                catch,
                finally_body,
            } => self.compile_try(body, catch.as_ref(), finally_body.as_deref()),
            Stmt::Break(label) => {
                self.emit(BytecodeInstruction::Complete(BytecodeCompletion::Break(
                    label.clone(),
                )));
                Ok(())
            }
            Stmt::Continue(label) => {
                self.emit(BytecodeInstruction::Complete(BytecodeCompletion::Continue(
                    label.clone(),
                )));
                Ok(())
            }
            Stmt::Throw(expr) => {
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::Complete(BytecodeCompletion::Throw));
                Ok(())
            }
            Stmt::Return(expr) => self.compile_return_statement(expr.as_ref()),
            Stmt::FunctionDecl {
                name,
                annex_b_var_binding: Some(variable),
                block_scoped: true,
                ..
            } => self.compile_annex_b_function_update(name, variable),
            Stmt::Empty
            | Stmt::Debugger
            | Stmt::FunctionDecl { .. }
            | Stmt::ImportBinding { .. } => Ok(()),
            Stmt::VarDecl { name, kind, init } => {
                self.compile_declaration(name, *kind, init.as_ref())
            }
            Stmt::Expr(expr) => {
                self.compile_expr(expr)?;
                self.emit(match value {
                    StatementValue::Store => BytecodeInstruction::StoreLast,
                    StatementValue::Discard => BytecodeInstruction::Pop,
                });
                Ok(())
            }
        }
    }

    fn compile_return_statement(&mut self, expr: Option<&Expression>) -> Result<()> {
        let Some(expr) = expr else {
            self.emit(BytecodeInstruction::PushUndefined);
            self.emit(BytecodeInstruction::Complete(
                BytecodeCompletion::ReturnDirect,
            ));
            return Ok(());
        };
        if self.compile_tail_call_expr(expr)? {
            return Ok(());
        }
        self.compile_expr(expr)?;
        self.emit(BytecodeInstruction::Complete(BytecodeCompletion::Return));
        Ok(())
    }

    fn compile_declaration(
        &mut self,
        name: &StaticBinding,
        kind: DeclKind,
        init: Option<&Expression>,
    ) -> Result<()> {
        if let Some(init) = init {
            self.compile_expr_with_inferred_name(init, name.name())?;
        }
        self.emit(BytecodeInstruction::DeclareBinding {
            name: self.compile_binding(name)?,
            kind,
            has_init: init.is_some(),
        });
        Ok(())
    }

    fn emit_jump(&mut self) -> InstructionIndex {
        self.emit(BytecodeInstruction::Jump(BytecodeAddress::new(0)))
    }

    fn emit_jump_if_false(&mut self) -> InstructionIndex {
        self.emit(BytecodeInstruction::JumpIfFalse(BytecodeAddress::new(0)))
    }

    fn emit_jump_if_false_keep(&mut self) -> InstructionIndex {
        self.emit(BytecodeInstruction::JumpIfFalseKeep(BytecodeAddress::new(
            0,
        )))
    }

    fn emit_jump_if_true_keep(&mut self) -> InstructionIndex {
        self.emit(BytecodeInstruction::JumpIfTrueKeep(BytecodeAddress::new(0)))
    }

    fn emit_jump_if_nullish_keep(&mut self) -> InstructionIndex {
        self.emit(BytecodeInstruction::JumpIfNullishKeep(
            BytecodeAddress::new(0),
        ))
    }

    fn patch_jump(&mut self, index: InstructionIndex, target: BytecodeAddress) -> Result<()> {
        let instruction = self
            .instructions
            .get_mut(index.index())
            .ok_or_else(|| Error::runtime("bytecode jump patch target disappeared"))?;
        if let BytecodeInstruction::Jump(address)
        | BytecodeInstruction::JumpIfFalse(address)
        | BytecodeInstruction::JumpIfFalseKeep(address)
        | BytecodeInstruction::JumpIfTrueKeep(address)
        | BytecodeInstruction::JumpIfNullishKeep(address) = instruction
        {
            *address = target;
            return Ok(());
        }
        Err(Error::runtime(
            "bytecode jump patch target is not a jump instruction",
        ))
    }

    fn compile_class_declaration(
        &mut self,
        name: &StaticBinding,
        class: &crate::ast::ClassLiteral,
    ) -> Result<()> {
        self.compile_class_literal(class)?;
        self.emit(BytecodeInstruction::DeclareBinding {
            name: self.compile_binding(name)?,
            kind: DeclKind::Let,
            has_init: true,
        });
        Ok(())
    }

    pub(super) fn compile_class_literal(&mut self, class: &crate::ast::ClassLiteral) -> Result<()> {
        self.compile_class_literal_with_inferred_name(class, None)
    }

    pub(super) fn compile_class_literal_with_inferred_name(
        &mut self,
        class: &crate::ast::ClassLiteral,
        inferred_name: Option<&StaticName>,
    ) -> Result<()> {
        if class.inner_name_binding.is_some() {
            let mut compiler = Self::new(self.layout, self.current_span);
            compiler.compile_class_literal_body(class, inferred_name)?;
            compiler.emit(BytecodeInstruction::StoreLast);
            self.emit(BytecodeInstruction::ScopedBlock {
                block: compiler.finish()?,
                var_hoist_plan: None,
                preserve_last: false,
                push_result: true,
            });
            return Ok(());
        }
        self.compile_class_literal_body(class, inferred_name)
    }

    fn compile_class_literal_body(
        &mut self,
        class: &crate::ast::ClassLiteral,
        inferred_name: Option<&StaticName>,
    ) -> Result<()> {
        let private_names = Self::class_private_names(class);
        let inner_name_binding = class
            .inner_name_binding
            .as_ref()
            .map(|binding| self.compile_binding(binding))
            .transpose()?;
        if let Some(binding) = &inner_name_binding {
            self.emit(BytecodeInstruction::HoistLexicalBinding {
                name: binding.clone(),
                kind: DeclKind::Const,
            });
        }
        for decorator in &class.decorators {
            self.compile_expr(decorator)?;
        }
        if let Some(heritage) = &class.heritage {
            self.compile_expr(heritage)?;
        }
        self.emit(BytecodeInstruction::BeginPrivateEnvironment {
            names: private_names.clone(),
        });
        self.compile_class_element_inputs(class)?;
        let members = self.compile_class_members(class, &private_names)?;
        let fields = self.compile_class_fields(class, &private_names)?;
        let static_blocks = class
            .static_blocks
            .iter()
            .map(|block| BytecodeBlock::compile_scoped_statements(&block.body, self.layout))
            .collect::<Result<Vec<_>>>()?;
        let static_element_order = Self::compile_class_static_element_order(class);
        self.emit(BytecodeInstruction::CreateClass {
            class: Rc::new(BytecodeClass {
                name: class.name.clone().or_else(|| inferred_name.cloned()),
                decorator_count: class.decorators.len(),
                inner_name_binding,
                heritage: class.heritage.is_some(),
                constructor_id: class.constructor.id,
                default_derived_constructor: class.constructor.default_derived,
                constructor: BytecodeFunction::compile_class_constructor(
                    class.constructor.arguments_binding.clone(),
                    &class.constructor.params,
                    &class.constructor.body,
                    &class.fields,
                    FunctionCompileMode::new(crate::syntax::FunctionKind::Ordinary, true),
                    self.layout,
                    self.source_text(self.current_span),
                )?,
                members: members.into(),
                fields: fields.into(),
                definition_order: Self::compile_class_definition_order(class).into(),
                static_blocks: static_blocks.into(),
                static_element_order: static_element_order.into(),
                private_names,
            }),
        });
        Ok(())
    }

    fn compile_class_static_element_order(
        class: &crate::ast::ClassLiteral,
    ) -> Vec<BytecodeClassStaticElement> {
        let mut ordered = Vec::new();
        let mut static_field_index = 0usize;
        for field in &class.fields {
            if !field.is_static {
                continue;
            }
            ordered.push((
                field.source_order,
                BytecodeClassStaticElement::Field(static_field_index),
            ));
            static_field_index = static_field_index.saturating_add(1);
        }
        for (index, block) in class.static_blocks.iter().enumerate() {
            ordered.push((block.source_order, BytecodeClassStaticElement::Block(index)));
        }
        ordered.sort_by_key(|(source_order, _)| *source_order);
        ordered.into_iter().map(|(_, element)| element).collect()
    }

    fn compile_class_definition_order(
        class: &crate::ast::ClassLiteral,
    ) -> Vec<BytecodeClassDefinitionElement> {
        let mut ordered = class
            .members
            .iter()
            .enumerate()
            .map(|(index, member)| {
                (
                    member.source_order,
                    BytecodeClassDefinitionElement::Member(index),
                )
            })
            .chain(class.fields.iter().enumerate().map(|(index, field)| {
                (
                    field.source_order,
                    BytecodeClassDefinitionElement::Field(index),
                )
            }))
            .collect::<Vec<_>>();
        ordered.sort_by_key(|(source_order, _)| *source_order);
        ordered.into_iter().map(|(_, element)| element).collect()
    }

    /// Lowers class methods and accessors, pushing computed keys onto the
    /// stack in member order.
    fn compile_class_members(
        &self,
        class: &crate::ast::ClassLiteral,
        private_names: &[StaticName],
    ) -> Result<Vec<BytecodeClassMember>> {
        let mut members = Vec::with_capacity(class.members.len());
        for member in &class.members {
            let key = Self::lower_class_element_key(&member.key, private_names)?;
            let kind = match member.kind {
                crate::ast::ClassMemberKind::Method => BytecodeClassMemberKind::Method,
                crate::ast::ClassMemberKind::Getter => BytecodeClassMemberKind::Getter,
                crate::ast::ClassMemberKind::Setter => BytecodeClassMemberKind::Setter,
            };
            members.push(BytecodeClassMember {
                key,
                decorator_count: member.decorators.len(),
                kind,
                function_kind: member.function_kind,
                is_static: member.is_static,
                id: member.id,
                bytecode: BytecodeFunction::compile(
                    None,
                    member.arguments_binding.clone(),
                    &member.params,
                    &member.body,
                    FunctionCompileMode::new(member.function_kind, true),
                    self.layout,
                    None,
                )?,
            });
        }
        Ok(members)
    }

    pub(super) fn lower_class_element_key(
        key: &crate::ast::ClassElementName,
        private_names: &[StaticName],
    ) -> Result<BytecodeClassMemberKey> {
        match key {
            crate::ast::ClassElementName::Property(ObjectPropertyKey::Static(name)) => {
                Ok(BytecodeClassMemberKey::Static(name.clone()))
            }
            crate::ast::ClassElementName::Property(ObjectPropertyKey::Computed(_)) => {
                Ok(BytecodeClassMemberKey::Computed)
            }
            crate::ast::ClassElementName::Private(name) => {
                let index = private_names
                    .iter()
                    .position(|candidate| candidate.as_str() == name.as_str())
                    .ok_or_else(|| Error::runtime("private class name disappeared"))?;
                let index = u32::try_from(index)
                    .map_err(|_| Error::limit("private class name index overflowed"))?;
                Ok(BytecodeClassMemberKey::Private { index })
            }
        }
    }

    fn class_private_names(class: &crate::ast::ClassLiteral) -> Rc<[StaticName]> {
        let mut names = Vec::new();
        for key in class
            .members
            .iter()
            .map(|member| &member.key)
            .chain(class.fields.iter().map(|field| &field.key))
        {
            let crate::ast::ClassElementName::Private(name) = key else {
                continue;
            };
            if names
                .iter()
                .any(|candidate: &StaticName| candidate.as_str() == name.as_str())
            {
                continue;
            }
            names.push(name.clone());
        }
        for name in class
            .fields
            .iter()
            .filter_map(|field| field.auto_accessor.as_ref())
            .map(|auto_accessor| &auto_accessor.backing_name)
        {
            if names
                .iter()
                .all(|candidate| candidate.as_str() != name.as_str())
            {
                names.push(name.clone());
            }
        }
        names.into()
    }

    fn compile_block_statement(
        &mut self,
        statements: &[Statement],
        value: StatementValue,
    ) -> Result<()> {
        if statements_need_lexical_scope(statements) {
            let block = BytecodeBlock::compile_lexical_statements(statements, value, self.layout)?;
            self.emit(BytecodeInstruction::ScopedBlock {
                block,
                var_hoist_plan: None,
                preserve_last: !statements_have_value_completion(statements),
                push_result: false,
            });
            return Ok(());
        }

        let before = self.instructions.len();
        self.compile_statements(statements, value)?;
        if value == StatementValue::Store && before == self.instructions.len() {
            self.emit(BytecodeInstruction::PushUndefined);
            self.emit(BytecodeInstruction::StoreLast);
        }
        Ok(())
    }

    fn emit(&mut self, instruction: BytecodeInstruction) -> InstructionIndex {
        let index = InstructionIndex::new(self.instructions.len());
        self.instructions.push(instruction);
        self.spans.push(self.current_span);
        index
    }

    const fn current_address(&self) -> BytecodeAddress {
        BytecodeAddress::new(self.instructions.len())
    }
}

fn statements_need_lexical_scope(statements: &[Statement]) -> bool {
    statements.iter().any(statement_needs_lexical_scope)
}

fn statements_have_value_completion(statements: &[Statement]) -> bool {
    statements.iter().any(|statement| match statement.kind() {
        Stmt::Empty
        | Stmt::Debugger
        | Stmt::ImportBinding { .. }
        | Stmt::VarDecl { .. }
        | Stmt::PatternDecl { .. }
        | Stmt::ClassDecl { .. }
        | Stmt::FunctionDecl { .. } => false,
        Stmt::DeclList(declarations) | Stmt::Block(declarations) => {
            statements_have_value_completion(declarations)
        }
        Stmt::Expr(_)
        | Stmt::If { .. }
        | Stmt::While { .. }
        | Stmt::DoWhile { .. }
        | Stmt::With { .. }
        | Stmt::Label { .. }
        | Stmt::For { .. }
        | Stmt::ForIn { .. }
        | Stmt::ForOf { .. }
        | Stmt::Switch { .. }
        | Stmt::Try { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Throw(_)
        | Stmt::Return(_) => true,
    })
}

fn statement_needs_lexical_scope(statement: &Statement) -> bool {
    match statement.kind() {
        Stmt::DeclList(statements) => statements_need_lexical_scope(statements),
        Stmt::VarDecl {
            kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
            ..
        }
        | Stmt::PatternDecl {
            kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
            ..
        }
        | Stmt::ImportBinding { .. }
        | Stmt::ClassDecl { .. }
        | Stmt::FunctionDecl { .. } => true,
        Stmt::Block(_)
        | Stmt::Empty
        | Stmt::Debugger
        | Stmt::If { .. }
        | Stmt::While { .. }
        | Stmt::DoWhile { .. }
        | Stmt::With { .. }
        | Stmt::Label { .. }
        | Stmt::For { .. }
        | Stmt::ForIn { .. }
        | Stmt::ForOf { .. }
        | Stmt::Switch { .. }
        | Stmt::Try { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Throw(_)
        | Stmt::Return(_)
        | Stmt::VarDecl { .. }
        | Stmt::PatternDecl { .. }
        | Stmt::Expr(_) => false,
    }
}

fn has_spread_arg(args: &[Expression]) -> bool {
    args.iter().any(|arg| matches!(arg.kind(), Expr::Spread(_)))
}

fn checked_template_part_count(part_count: usize) -> Result<usize> {
    part_count
        .checked_add(1)
        .ok_or_else(|| Error::runtime("template literal part count overflowed"))
}

fn constructor_binding_expr(expr: &Expression) -> Option<&StaticBinding> {
    match expr.kind() {
        Expr::Identifier(binding) => Some(binding),
        Expr::Parenthesized(expr) => constructor_binding_expr(expr),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
struct InstructionIndex(usize);

impl InstructionIndex {
    const fn new(index: usize) -> Self {
        Self(index)
    }

    const fn index(self) -> usize {
        self.0
    }
}
