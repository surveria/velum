use std::rc::Rc;

use crate::{
    api::native_call::NativeCallTarget,
    ast::{
        BinaryOp, DeclKind, Expr, FunctionParam, ObjectProperty, Program, StaticBinding,
        StaticName, StaticPropertyAccessId, Stmt, UnaryOp, UpdateOp,
    },
    binding_layout::BindingLayout,
    error::{Error, Result},
};

mod call;
mod control;
mod function;
mod hoist;
mod metrics;
mod types;

pub use hoist::BytecodeHoistPlan;
pub use types::{
    BytecodeAddress, BytecodeArrayIndex, BytecodeAssignmentTarget, BytecodeBinding, BytecodeBlock,
    BytecodeCallSite, BytecodeCatch, BytecodeCompletion, BytecodeDynamicProperty,
    BytecodeForInTarget, BytecodeFunction, BytecodeFunctionDeclaration, BytecodeInstruction,
    BytecodeNumericBinaryOp, BytecodeNumericCompareOp, BytecodeProgram, BytecodeProperty,
    BytecodeSwitchCase,
};

const ARRAY_LENGTH_PROPERTY: &str = "length";

impl BytecodeProgram {
    pub fn compile(program: &Program, layout: &BindingLayout) -> Result<Self> {
        Ok(Self::new(
            BytecodeBlock::compile_statements(&program.statements, StatementValue::Store, layout)?,
            BytecodeHoistPlan::compile(&program.statements, layout)?,
        ))
    }
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
            Stmt::Switch {
                discriminant,
                cases,
            } => self.compile_switch(discriminant, cases),
            Stmt::Try {
                body,
                catch,
                finally_body,
            } => self.compile_try(body, catch.as_ref(), finally_body.as_deref()),
            Stmt::Break => {
                self.emit(BytecodeInstruction::Complete(BytecodeCompletion::Break));
                Ok(())
            }
            Stmt::Continue => {
                self.emit(BytecodeInstruction::Complete(BytecodeCompletion::Continue));
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
            Stmt::FunctionDecl { .. } => Ok(()),
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

    fn compile_expr(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::Literal(value) => {
                self.emit(BytecodeInstruction::PushLiteral(value.clone()));
            }
            Expr::StringLiteral(value) => {
                self.emit(BytecodeInstruction::PushString(value.clone()));
            }
            Expr::This => {
                self.emit(BytecodeInstruction::LoadThis);
            }
            Expr::Identifier(name) => {
                self.emit(BytecodeInstruction::LoadBinding(
                    self.compile_binding(name)?,
                ));
            }
            Expr::Parenthesized(expr) => return self.compile_expr(expr),
            Expr::Await(expr) => {
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::Await);
            }
            Expr::Unary { op, expr } => return self.compile_unary_expr(*op, expr),
            Expr::Binary {
                op,
                left,
                right,
                property_access,
            } => return self.compile_binary_expr(*op, left, right, *property_access),
            Expr::Conditional {
                condition,
                consequent,
                alternate,
            } => return self.compile_conditional_expr(condition, consequent, alternate),
            Expr::Assignment { name, expr } => {
                return self.compile_binding_assignment_expr(name, expr);
            }
            Expr::PropertyAssignment {
                object,
                property,
                access,
                expr,
            } => return self.compile_static_property_assignment(object, property, *access, expr),
            Expr::ComputedPropertyAssignment {
                object,
                property,
                access,
                expr,
            } => return self.compile_computed_property_assignment(object, property, *access, expr),
            Expr::Member {
                object,
                property,
                access,
            } => return self.compile_static_member_expr(object, property, *access),
            Expr::ComputedMember {
                object,
                property,
                access,
            } => return self.compile_computed_member_expr(object, property, *access),
            Expr::Object(properties) => return self.compile_object_literal(properties),
            Expr::Array(elements) => return self.compile_array_literal(elements),
            Expr::Update { op, prefix, expr } => {
                return self.compile_update_expr(*op, *prefix, expr);
            }
            Expr::CompoundAssignment { op, target, expr } => {
                return self.compile_compound_assignment(*op, target, expr);
            }
            Expr::Call { callee, site, args } => {
                return self.compile_call_expr(callee, *site, args);
            }
            Expr::Function {
                id,
                name,
                params,
                body,
                is_async,
            } => self.compile_function_expr(*id, name.clone(), params, body, true, *is_async)?,
            Expr::ArrowFunction {
                id,
                params,
                body,
                is_async,
            } => self.compile_function_expr(*id, None, params, body, false, *is_async)?,
            Expr::MethodFunction {
                id,
                name,
                params,
                body,
            } => self.compile_function_expr(*id, Some(name.clone()), params, body, false, false)?,
            Expr::New { constructor, args } => self.compile_new_expr(constructor, args)?,
        }
        Ok(())
    }

    fn compile_binding_assignment_expr(&mut self, name: &StaticBinding, expr: &Expr) -> Result<()> {
        self.compile_expr(expr)?;
        self.emit(BytecodeInstruction::StoreBinding(
            self.compile_binding(name)?,
        ));
        Ok(())
    }

    fn compile_static_property_assignment(
        &mut self,
        object: &Expr,
        property: &StaticName,
        access: StaticPropertyAccessId,
        expr: &Expr,
    ) -> Result<()> {
        self.compile_expr(object)?;
        self.compile_expr(expr)?;
        let property = Self::compile_property(property, access);
        if let Some(index) = Self::compile_array_index(&property) {
            self.emit(BytecodeInstruction::ArrayIndexAssign { property, index });
        } else {
            self.emit(BytecodeInstruction::StaticPropertyAssign { property });
        }
        Ok(())
    }

    fn compile_computed_property_assignment(
        &mut self,
        object: &Expr,
        property: &Expr,
        access: StaticPropertyAccessId,
        expr: &Expr,
    ) -> Result<()> {
        self.compile_expr(object)?;
        self.compile_expr(property)?;
        self.compile_expr(expr)?;
        self.emit(BytecodeInstruction::ComputedPropertyAssign {
            property: Self::compile_dynamic_property(access),
        });
        Ok(())
    }

    fn compile_static_member_expr(
        &mut self,
        object: &Expr,
        property: &StaticName,
        access: StaticPropertyAccessId,
    ) -> Result<()> {
        self.compile_expr(object)?;
        let property = Self::compile_property(property, access);
        if property.name().as_str() == ARRAY_LENGTH_PROPERTY {
            self.emit(BytecodeInstruction::ArrayLength { property });
        } else if let Some(index) = Self::compile_array_index(&property) {
            self.emit(BytecodeInstruction::ArrayIndexMember { property, index });
        } else {
            self.emit(BytecodeInstruction::StaticMember { property });
        }
        Ok(())
    }

    fn compile_computed_member_expr(
        &mut self,
        object: &Expr,
        property: &Expr,
        access: StaticPropertyAccessId,
    ) -> Result<()> {
        self.compile_expr(object)?;
        self.compile_expr(property)?;
        self.emit(BytecodeInstruction::ComputedMember {
            property: Self::compile_dynamic_property(access),
        });
        Ok(())
    }

    fn compile_function_expr(
        &mut self,
        id: crate::ast::StaticFunctionId,
        name: Option<crate::ast::StaticName>,
        params: &Rc<[FunctionParam]>,
        body: &[Stmt],
        constructable: bool,
        is_async: bool,
    ) -> Result<()> {
        self.emit(BytecodeInstruction::CreateFunction {
            id,
            name,
            params: Rc::clone(params),
            bytecode: BytecodeFunction::compile(params, body, self.layout)?,
            constructable,
            is_async,
        });
        Ok(())
    }

    fn compile_new_expr(&mut self, constructor: &StaticBinding, args: &[Expr]) -> Result<()> {
        self.compile_args(args)?;
        self.emit(BytecodeInstruction::Construct {
            constructor: self.compile_binding(constructor)?,
            native: NativeCallTarget::from_binding_name(constructor.as_str()),
            arg_count: args.len(),
        });
        Ok(())
    }

    fn compile_unary_expr(&mut self, op: UnaryOp, expr: &Expr) -> Result<()> {
        match op {
            UnaryOp::Not | UnaryOp::Negate | UnaryOp::Plus | UnaryOp::Void => {
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::Unary(op));
            }
            UnaryOp::Typeof => self.compile_typeof_expr(expr)?,
            UnaryOp::Delete => self.compile_delete_expr(expr)?,
        }
        Ok(())
    }

    fn compile_typeof_expr(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::Parenthesized(expr) => self.compile_typeof_expr(expr),
            Expr::Identifier(name) => {
                self.emit(BytecodeInstruction::TypeOfBinding(
                    self.compile_binding(name)?,
                ));
                Ok(())
            }
            expr => {
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::TypeOfValue);
                Ok(())
            }
        }
    }

    fn compile_delete_expr(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::Parenthesized(expr) => self.compile_delete_expr(expr),
            Expr::Identifier(name) => {
                self.emit(BytecodeInstruction::DeleteBinding(
                    self.compile_binding(name)?,
                ));
                Ok(())
            }
            Expr::Member {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.emit(BytecodeInstruction::DeleteStaticProperty {
                    property: Self::compile_property(property, *access),
                });
                Ok(())
            }
            Expr::ComputedMember {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.compile_expr(property)?;
                self.emit(BytecodeInstruction::DeleteComputedProperty {
                    property: Self::compile_dynamic_property(*access),
                });
                Ok(())
            }
            expr => {
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::DeleteValue);
                Ok(())
            }
        }
    }

    fn compile_binary_expr(
        &mut self,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
        property_access: Option<StaticPropertyAccessId>,
    ) -> Result<()> {
        match op {
            BinaryOp::LogicalAnd => self.compile_logical_and(left, right),
            BinaryOp::LogicalOr => self.compile_logical_or(left, right),
            _ => {
                self.compile_expr(left)?;
                self.compile_expr(right)?;
                if property_access.is_none()
                    && let Some(op) = BytecodeNumericBinaryOp::from_binary(op)
                {
                    self.emit(BytecodeInstruction::NumberBinary(op));
                } else if property_access.is_none()
                    && let Some(op) = BytecodeNumericCompareOp::from_binary(op)
                {
                    self.emit(BytecodeInstruction::NumberCompare(op));
                } else {
                    self.emit(BytecodeInstruction::Binary {
                        op,
                        property_access: property_access.map(Self::compile_dynamic_property),
                    });
                }
                Ok(())
            }
        }
    }

    fn compile_logical_and(&mut self, left: &Expr, right: &Expr) -> Result<()> {
        self.compile_expr(left)?;
        let end_jump = self.emit_jump_if_false_keep();
        self.emit(BytecodeInstruction::Pop);
        self.compile_expr(right)?;
        let end = self.current_address();
        self.patch_jump(end_jump, end)
    }

    fn compile_logical_or(&mut self, left: &Expr, right: &Expr) -> Result<()> {
        self.compile_expr(left)?;
        let end_jump = self.emit_jump_if_true_keep();
        self.emit(BytecodeInstruction::Pop);
        self.compile_expr(right)?;
        let end = self.current_address();
        self.patch_jump(end_jump, end)
    }

    fn compile_update_expr(&mut self, op: UpdateOp, prefix: bool, expr: &Expr) -> Result<()> {
        match expr {
            Expr::Identifier(name) => {
                self.emit(BytecodeInstruction::UpdateBinding {
                    name: self.compile_binding(name)?,
                    op,
                    prefix,
                });
                Ok(())
            }
            Expr::Member {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.emit(BytecodeInstruction::UpdateStaticProperty {
                    property: Self::compile_property(property, *access),
                    op,
                    prefix,
                });
                Ok(())
            }
            Expr::ComputedMember {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.compile_expr(property)?;
                self.emit(BytecodeInstruction::UpdateComputedProperty {
                    property: Self::compile_dynamic_property(*access),
                    op,
                    prefix,
                });
                Ok(())
            }
            Expr::Parenthesized(expr) => self.compile_update_expr(op, prefix, expr),
            _ => Err(Error::runtime("invalid bytecode update target")),
        }
    }

    fn compile_compound_assignment(
        &mut self,
        op: BinaryOp,
        target: &Expr,
        expr: &Expr,
    ) -> Result<()> {
        match target {
            Expr::Identifier(name) => {
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::CompoundStoreBinding {
                    name: self.compile_binding(name)?,
                    op,
                });
                Ok(())
            }
            Expr::Member {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::CompoundStaticProperty {
                    property: Self::compile_property(property, *access),
                    op,
                });
                Ok(())
            }
            Expr::ComputedMember {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.compile_expr(property)?;
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::CompoundComputedProperty {
                    property: Self::compile_dynamic_property(*access),
                    op,
                });
                Ok(())
            }
            Expr::Parenthesized(target) => self.compile_compound_assignment(op, target, expr),
            _ => Err(Error::runtime(
                "invalid bytecode compound assignment target",
            )),
        }
    }

    fn compile_conditional_expr(
        &mut self,
        condition: &Expr,
        consequent: &Expr,
        alternate: &Expr,
    ) -> Result<()> {
        self.compile_expr(condition)?;
        let false_jump = self.emit_jump_if_false();
        self.compile_expr(consequent)?;
        let end_jump = self.emit_jump();
        let alternate_address = self.current_address();
        self.patch_jump(false_jump, alternate_address)?;
        self.compile_expr(alternate)?;
        let end_address = self.current_address();
        self.patch_jump(end_jump, end_address)
    }

    fn compile_object_literal(&mut self, properties: &[ObjectProperty]) -> Result<()> {
        let mut names = Vec::with_capacity(properties.len());
        for property in properties {
            names.push(property.key.clone());
            self.compile_expr(&property.value)?;
        }
        self.emit(BytecodeInstruction::ObjectLiteral {
            properties: Rc::from(names.into_boxed_slice()),
        });
        Ok(())
    }

    fn compile_array_literal(&mut self, elements: &[Expr]) -> Result<()> {
        for element in elements {
            self.compile_expr(element)?;
        }
        self.emit(BytecodeInstruction::ArrayLiteral {
            len: elements.len(),
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
            | BytecodeInstruction::PushUndefined
            | BytecodeInstruction::LoadThis
            | BytecodeInstruction::LoadBinding(_)
            | BytecodeInstruction::StoreBinding(_)
            | BytecodeInstruction::DeclareBinding { .. }
            | BytecodeInstruction::StoreLast
            | BytecodeInstruction::Pop
            | BytecodeInstruction::Unary(_)
            | BytecodeInstruction::Await
            | BytecodeInstruction::TypeOfBinding(_)
            | BytecodeInstruction::TypeOfValue
            | BytecodeInstruction::DeleteBinding(_)
            | BytecodeInstruction::DeleteStaticProperty { .. }
            | BytecodeInstruction::DeleteComputedProperty { .. }
            | BytecodeInstruction::DeleteValue
            | BytecodeInstruction::UpdateBinding { .. }
            | BytecodeInstruction::UpdateStaticProperty { .. }
            | BytecodeInstruction::UpdateComputedProperty { .. }
            | BytecodeInstruction::Binary { .. }
            | BytecodeInstruction::NumberBinary(_)
            | BytecodeInstruction::NumberCompare(_)
            | BytecodeInstruction::CompoundStoreBinding { .. }
            | BytecodeInstruction::CompoundStaticProperty { .. }
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
            | BytecodeInstruction::CreateFunction { .. }
            | BytecodeInstruction::ArrayLiteral { .. }
            | BytecodeInstruction::ObjectLiteral { .. }
            | BytecodeInstruction::If { .. }
            | BytecodeInstruction::While { .. }
            | BytecodeInstruction::For { .. }
            | BytecodeInstruction::ForIn { .. }
            | BytecodeInstruction::Switch { .. }
            | BytecodeInstruction::Try { .. }
            | BytecodeInstruction::ScopedBlock(_)
            | BytecodeInstruction::Complete(_) => Err(Error::runtime(
                "bytecode jump patch target is not a jump instruction",
            )),
        }
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
