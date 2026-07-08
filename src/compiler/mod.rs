use std::rc::Rc;

use crate::{
    api::native_call::NativeCallTarget,
    ast::{
        BinaryOp, DeclKind, Expr, ObjectProperty, ObjectPropertyKey, ObjectPropertyKind, Program,
        StaticBinding, StaticPropertyAccessId, Stmt, UnaryOp, UpdateOp,
    },
    binding_layout::BindingLayout,
    bytecode::{
        BytecodeAddress, BytecodeArrayIndex, BytecodeBinding, BytecodeBlock, BytecodeCallSite,
        BytecodeClass, BytecodeClassMember, BytecodeClassMemberKey, BytecodeClassMemberKind,
        BytecodeCompletion, BytecodeDynamicProperty, BytecodeFunction, BytecodeFunctionParam,
        BytecodeHoistPlan, BytecodeInstruction, BytecodeNewTargetMode, BytecodeNumericBinaryOp,
        BytecodeNumericCompareOp, BytecodeNumericEqualityOp, BytecodeNumericUnaryOp,
        BytecodeObjectProperty, BytecodeProgram, BytecodeProperty,
    },
    error::{Error, Result},
    syntax::{AccessorKind, StaticName, StaticString},
};

mod call;
mod control;
mod expression;
mod function;
mod hoist;
mod pattern;

const ARRAY_LENGTH_PROPERTY: &str = "length";

pub fn compile_program(program: &Program, layout: &BindingLayout) -> Result<BytecodeProgram> {
    Ok(BytecodeProgram::new(
        BytecodeBlock::compile_statements(&program.statements, StatementValue::Store, layout)?,
        BytecodeHoistPlan::compile(&program.statements, layout)?,
    ))
}

impl BytecodeBlock {
    fn compile_statements(
        statements: &[Stmt],
        value: StatementValue,
        layout: &BindingLayout,
    ) -> Result<Self> {
        let mut compiler = BytecodeCompiler::new(layout);
        compiler.compile_statements(statements, value)?;
        Ok(Self::from_instructions(compiler.instructions))
    }

    fn compile_expression(expr: &Expr, layout: &BindingLayout) -> Result<Self> {
        let mut compiler = BytecodeCompiler::new(layout);
        compiler.compile_expr(expr)?;
        compiler.emit(BytecodeInstruction::StoreLast);
        Ok(Self::from_instructions(compiler.instructions))
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
}

impl<'a> BytecodeCompiler<'a> {
    const fn new(layout: &'a BindingLayout) -> Self {
        Self {
            layout,
            instructions: Vec::new(),
        }
    }

    fn compile_binding(&self, binding: &StaticBinding) -> Result<BytecodeBinding> {
        BytecodeBinding::compile(binding, self.layout)
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

    fn compile_statements(&mut self, statements: &[Stmt], value: StatementValue) -> Result<()> {
        for statement in statements {
            self.compile_statement(statement, value)?;
        }
        Ok(())
    }

    fn compile_statement(&mut self, statement: &Stmt, value: StatementValue) -> Result<()> {
        match statement {
            Stmt::Block(statements) => {
                let block = BytecodeBlock::compile_statements(statements, value, self.layout)?;
                self.emit(BytecodeInstruction::ScopedBlock(block));
                Ok(())
            }
            Stmt::DeclList(declarations) => self.compile_statements(declarations, value),
            Stmt::If {
                condition,
                consequent,
                alternate,
            } => self.compile_if(condition, consequent, alternate.as_deref(), value),
            Stmt::While { condition, body } => self.compile_while(condition, body),
            Stmt::DoWhile { body, condition } => self.compile_do_while(body, condition),
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
            } => self.compile_for_of(target, object, body),
            Stmt::PatternDecl {
                pattern,
                kind,
                init,
            } => {
                self.compile_expr(init)?;
                let pattern = self.compile_pattern(pattern)?;
                self.emit(BytecodeInstruction::DestructurePattern {
                    pattern: Rc::new(pattern),
                    kind: *kind,
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
            Stmt::Return(expr) => {
                if let Some(expr) = expr {
                    self.compile_expr(expr)?;
                } else {
                    self.emit(BytecodeInstruction::PushUndefined);
                }
                self.emit(BytecodeInstruction::Complete(BytecodeCompletion::Return));
                Ok(())
            }
            Stmt::Empty | Stmt::FunctionDecl { .. } => Ok(()),
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

    fn compile_declaration(
        &mut self,
        name: &StaticBinding,
        kind: DeclKind,
        init: Option<&Expr>,
    ) -> Result<()> {
        if let Some(init) = init {
            self.compile_expr(init)?;
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

    fn patch_jump(&mut self, index: InstructionIndex, target: BytecodeAddress) -> Result<()> {
        let instruction = self
            .instructions
            .get_mut(index.index())
            .ok_or_else(|| Error::runtime("bytecode jump patch target disappeared"))?;
        match instruction {
            BytecodeInstruction::Jump(address)
            | BytecodeInstruction::JumpIfFalse(address)
            | BytecodeInstruction::JumpIfFalseKeep(address)
            | BytecodeInstruction::JumpIfTrueKeep(address) => {
                *address = target;
                Ok(())
            }
            BytecodeInstruction::PushLiteral(_)
            | BytecodeInstruction::PushString(_)
            | BytecodeInstruction::TemplateConcat { .. }
            | BytecodeInstruction::CollectSpreadArgs { .. }
            | BytecodeInstruction::CallBindingSpread { .. }
            | BytecodeInstruction::CallValueSpread
            | BytecodeInstruction::CallStaticMemberSpread { .. }
            | BytecodeInstruction::CallComputedMemberSpread { .. }
            | BytecodeInstruction::ConstructValueSpread
            | BytecodeInstruction::ArrayLiteralSpread { .. }
            | BytecodeInstruction::CreateRegExp { .. }
            | BytecodeInstruction::PushUndefined
            | BytecodeInstruction::LoadThis
            | BytecodeInstruction::LoadNewTarget
            | BytecodeInstruction::LoadBinding(_)
            | BytecodeInstruction::StoreBinding(_)
            | BytecodeInstruction::DeclareBinding { .. }
            | BytecodeInstruction::StoreLast
            | BytecodeInstruction::Pop
            | BytecodeInstruction::Unary(_)
            | BytecodeInstruction::NumberUnary(_)
            | BytecodeInstruction::Await
            | BytecodeInstruction::TypeOfBinding(_)
            | BytecodeInstruction::TypeOfValue
            | BytecodeInstruction::DeleteBinding(_)
            | BytecodeInstruction::DeleteStaticProperty { .. }
            | BytecodeInstruction::DeleteComputedProperty { .. }
            | BytecodeInstruction::DeleteValue
            | BytecodeInstruction::UpdateBinding { .. }
            | BytecodeInstruction::UpdateStaticProperty { .. }
            | BytecodeInstruction::UpdateArrayIndexProperty { .. }
            | BytecodeInstruction::UpdateComputedProperty { .. }
            | BytecodeInstruction::Binary { .. }
            | BytecodeInstruction::NumberBinary(_)
            | BytecodeInstruction::NumberCompare(_)
            | BytecodeInstruction::NumberEquality(_)
            | BytecodeInstruction::CompoundStoreBinding { .. }
            | BytecodeInstruction::CompoundStaticProperty { .. }
            | BytecodeInstruction::CompoundArrayIndexProperty { .. }
            | BytecodeInstruction::CompoundComputedProperty { .. }
            | BytecodeInstruction::StaticMember { .. }
            | BytecodeInstruction::ArrayLength { .. }
            | BytecodeInstruction::ArrayIndexMember { .. }
            | BytecodeInstruction::ComputedMember { .. }
            | BytecodeInstruction::StaticPropertyAssign { .. }
            | BytecodeInstruction::ArrayIndexAssign { .. }
            | BytecodeInstruction::ComputedPropertyAssign { .. }
            | BytecodeInstruction::CallBinding { .. }
            | BytecodeInstruction::CallValue { .. }
            | BytecodeInstruction::CallStaticMember { .. }
            | BytecodeInstruction::CallComputedMember { .. }
            | BytecodeInstruction::Print { .. }
            | BytecodeInstruction::AssertThrows { .. }
            | BytecodeInstruction::Construct { .. }
            | BytecodeInstruction::ConstructValue { .. }
            | BytecodeInstruction::CreateFunction { .. }
            | BytecodeInstruction::ArrayLiteral { .. }
            | BytecodeInstruction::ObjectLiteral { .. }
            | BytecodeInstruction::While { .. }
            | BytecodeInstruction::DoWhile { .. }
            | BytecodeInstruction::For { .. }
            | BytecodeInstruction::ForIn { .. }
            | BytecodeInstruction::ForOf { .. }
            | BytecodeInstruction::DestructurePattern { .. }
            | BytecodeInstruction::CreateClass { .. }
            | BytecodeInstruction::CreateClass { .. }
            | BytecodeInstruction::Switch { .. }
            | BytecodeInstruction::Try { .. }
            | BytecodeInstruction::Label { .. }
            | BytecodeInstruction::ScopedBlock(_)
            | BytecodeInstruction::Complete(_) => Err(Error::runtime(
                "bytecode jump patch target is not a jump instruction",
            )),
        }
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
        let mut members = Vec::with_capacity(class.members.len());
        for member in &class.members {
            let key = match &member.key {
                ObjectPropertyKey::Static(name) => BytecodeClassMemberKey::Static(name.clone()),
                ObjectPropertyKey::Computed(expr) => {
                    self.compile_expr(expr)?;
                    BytecodeClassMemberKey::Computed
                }
            };
            let kind = match member.kind {
                crate::ast::ClassMemberKind::Method => BytecodeClassMemberKind::Method,
                crate::ast::ClassMemberKind::Getter => BytecodeClassMemberKind::Getter,
                crate::ast::ClassMemberKind::Setter => BytecodeClassMemberKind::Setter,
            };
            members.push(BytecodeClassMember {
                key,
                kind,
                is_static: member.is_static,
                id: member.id,
                name: member.name.clone(),
                bytecode: BytecodeFunction::compile(&member.params, &member.body, self.layout)?,
            });
        }
        self.emit(BytecodeInstruction::CreateClass {
            class: Rc::new(BytecodeClass {
                name: class.name.clone(),
                constructor_id: class.constructor.id,
                constructor: BytecodeFunction::compile(
                    &class.constructor.params,
                    &class.constructor.body,
                    self.layout,
                )?,
                members: members.into(),
            }),
        });
        Ok(())
    }

    fn emit(&mut self, instruction: BytecodeInstruction) -> InstructionIndex {
        let index = InstructionIndex::new(self.instructions.len());
        self.instructions.push(instruction);
        index
    }

    const fn current_address(&self) -> BytecodeAddress {
        BytecodeAddress::new(self.instructions.len())
    }
}

fn has_spread_arg(args: &[Expr]) -> bool {
    args.iter().any(|arg| matches!(arg, Expr::Spread(_)))
}

fn checked_template_part_count(part_count: usize) -> Result<usize> {
    part_count
        .checked_add(1)
        .ok_or_else(|| Error::runtime("template literal part count overflowed"))
}

fn constructor_binding_expr(expr: &Expr) -> Option<&StaticBinding> {
    match expr {
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
