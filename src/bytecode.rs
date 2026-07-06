use std::rc::Rc;

use crate::{
    ast::{
        BinaryOp, CatchClause, DeclKind, Expr, ForInTarget, ObjectProperty, Program, StaticBinding,
        StaticPropertyAccessId, Stmt, SwitchCase, UnaryOp, UpdateOp,
    },
    bytecode_hoist::BytecodeHoistPlan,
    error::{Error, Result},
};

#[path = "bytecode_call.rs"]
mod bytecode_call;
#[path = "bytecode_function.rs"]
mod bytecode_function;

pub use crate::bytecode_types::{
    BytecodeAddress, BytecodeAssignmentTarget, BytecodeBlock, BytecodeCatch, BytecodeCompletion,
    BytecodeForInTarget, BytecodeFunction, BytecodeInstruction, BytecodeProgram,
    BytecodeSwitchCase,
};

impl BytecodeProgram {
    pub fn compile(program: &Program) -> Result<Self> {
        Ok(Self::new(
            BytecodeBlock::compile_statements(&program.statements, StatementValue::Store)?,
            BytecodeHoistPlan::compile(&program.statements),
        ))
    }
}

impl BytecodeBlock {
    fn compile_statements(statements: &[Stmt], value: StatementValue) -> Result<Self> {
        let mut compiler = BytecodeCompiler::new();
        compiler.compile_statements(statements, value)?;
        Ok(Self::from_instructions(compiler.instructions))
    }

    fn compile_expression(expr: &Expr) -> Result<Self> {
        let mut compiler = BytecodeCompiler::new();
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
struct BytecodeCompiler {
    instructions: Vec<BytecodeInstruction>,
}

impl BytecodeCompiler {
    const fn new() -> Self {
        Self {
            instructions: Vec::new(),
        }
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
                let block = BytecodeBlock::compile_statements(statements, value)?;
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

    fn compile_if(
        &mut self,
        condition: &Expr,
        consequent: &Stmt,
        alternate: Option<&Stmt>,
        value: StatementValue,
    ) -> Result<()> {
        let condition = BytecodeBlock::compile_expression(condition)?;
        let consequent = Self::compile_statement_block(consequent, value)?;
        let alternate = alternate
            .map(|alternate| Self::compile_statement_block(alternate, value))
            .transpose()?;
        self.emit(BytecodeInstruction::If {
            condition,
            consequent,
            alternate,
        });
        Ok(())
    }

    fn compile_while(&mut self, condition: &Expr, body: &Stmt) -> Result<()> {
        self.emit(BytecodeInstruction::While {
            condition: BytecodeBlock::compile_expression(condition)?,
            body: Self::compile_statement_block(body, StatementValue::Store)?,
        });
        Ok(())
    }

    fn compile_for(
        &mut self,
        init: Option<&Stmt>,
        condition: Option<&Expr>,
        update: Option<&Expr>,
        body: &Stmt,
    ) -> Result<()> {
        self.emit(BytecodeInstruction::For {
            init: init
                .map(|init| Self::compile_statement_block(init, StatementValue::Discard))
                .transpose()?,
            condition: condition
                .map(BytecodeBlock::compile_expression)
                .transpose()?,
            update: update.map(BytecodeBlock::compile_expression).transpose()?,
            body: Self::compile_statement_block(body, StatementValue::Store)?,
            scoped: for_init_needs_lexical_scope(init),
        });
        Ok(())
    }

    fn compile_for_in(&mut self, target: &ForInTarget, object: &Expr, body: &Stmt) -> Result<()> {
        self.emit(BytecodeInstruction::ForIn {
            target: Self::compile_for_in_target(target)?,
            object: BytecodeBlock::compile_expression(object)?,
            body: Self::compile_statement_block(body, StatementValue::Store)?,
        });
        Ok(())
    }

    fn compile_switch(&mut self, discriminant: &Expr, cases: &[SwitchCase]) -> Result<()> {
        let mut bytecode_cases = Vec::with_capacity(cases.len());
        for case in cases {
            bytecode_cases.push(BytecodeSwitchCase {
                test: case
                    .test
                    .as_ref()
                    .map(BytecodeBlock::compile_expression)
                    .transpose()?,
                body: BytecodeBlock::compile_statements(&case.statements, StatementValue::Store)?,
            });
        }
        self.emit(BytecodeInstruction::Switch {
            discriminant: BytecodeBlock::compile_expression(discriminant)?,
            cases: Rc::from(bytecode_cases.into_boxed_slice()),
        });
        Ok(())
    }

    fn compile_try(
        &mut self,
        body: &[Stmt],
        catch: Option<&CatchClause>,
        finally_body: Option<&[Stmt]>,
    ) -> Result<()> {
        self.emit(BytecodeInstruction::Try {
            body: BytecodeBlock::compile_statements(body, StatementValue::Store)?,
            catch: catch
                .map(|catch| {
                    Ok(BytecodeCatch {
                        param: catch.param.clone(),
                        body: BytecodeBlock::compile_statements(
                            &catch.body,
                            StatementValue::Store,
                        )?,
                    })
                })
                .transpose()?,
            finally_body: finally_body
                .map(|body| BytecodeBlock::compile_statements(body, StatementValue::Store))
                .transpose()?,
        });
        Ok(())
    }

    fn compile_statement_block(statement: &Stmt, value: StatementValue) -> Result<BytecodeBlock> {
        let mut compiler = Self::new();
        compiler.compile_statement(statement, value)?;
        Ok(BytecodeBlock::from_instructions(compiler.instructions))
    }

    fn compile_for_in_target(target: &ForInTarget) -> Result<BytecodeForInTarget> {
        match target {
            ForInTarget::Binding { name, kind } => Ok(BytecodeForInTarget::Binding {
                name: name.clone(),
                kind: *kind,
            }),
            ForInTarget::Assignment(expr) => {
                Self::compile_assignment_target(expr).map(BytecodeForInTarget::Assignment)
            }
        }
    }

    fn compile_assignment_target(expr: &Expr) -> Result<BytecodeAssignmentTarget> {
        match expr {
            Expr::Identifier(name) => Ok(BytecodeAssignmentTarget::Binding(name.clone())),
            Expr::Member {
                object,
                property,
                access,
            } => Ok(BytecodeAssignmentTarget::StaticProperty {
                object: BytecodeBlock::compile_expression(object)?,
                property: property.clone(),
                access: *access,
            }),
            Expr::ComputedMember {
                object,
                property,
                access,
            } => Ok(BytecodeAssignmentTarget::ComputedProperty {
                object: BytecodeBlock::compile_expression(object)?,
                property: BytecodeBlock::compile_expression(property)?,
                access: *access,
            }),
            Expr::Parenthesized(expr) => Self::compile_assignment_target(expr),
            _ => Err(Error::runtime("invalid bytecode assignment target")),
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
            name: name.clone(),
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
                self.emit(BytecodeInstruction::LoadBinding(name.clone()));
            }
            Expr::Parenthesized(expr) => return self.compile_expr(expr),
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
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::StoreBinding(name.clone()));
            }
            Expr::PropertyAssignment {
                object,
                property,
                access,
                expr,
            } => {
                self.compile_expr(object)?;
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::StaticPropertyAssign {
                    property: property.clone(),
                    access: *access,
                });
            }
            Expr::ComputedPropertyAssignment {
                object,
                property,
                access,
                expr,
            } => {
                self.compile_expr(object)?;
                self.compile_expr(property)?;
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::ComputedPropertyAssign { access: *access });
            }
            Expr::Member {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.emit(BytecodeInstruction::StaticMember {
                    property: property.clone(),
                    access: *access,
                });
            }
            Expr::ComputedMember {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.compile_expr(property)?;
                self.emit(BytecodeInstruction::ComputedMember { access: *access });
            }
            Expr::Object(properties) => return self.compile_object_literal(properties),
            Expr::Array(elements) => return self.compile_array_literal(elements),
            Expr::Update { op, prefix, expr } => {
                return self.compile_update_expr(*op, *prefix, expr);
            }
            Expr::CompoundAssignment { op, target, expr } => {
                return self.compile_compound_assignment(*op, target, expr);
            }
            Expr::Call { callee, args } => return self.compile_call_expr(callee, args),
            Expr::Function {
                id,
                name,
                params,
                body,
            } => self.compile_function_expr(*id, name.clone(), params, body, true)?,
            Expr::MethodFunction {
                id,
                name,
                params,
                body,
            } => self.compile_function_expr(*id, Some(name.clone()), params, body, false)?,
            Expr::New { constructor, args } => self.compile_new_expr(constructor, args)?,
        }
        Ok(())
    }

    fn compile_function_expr(
        &mut self,
        id: crate::ast::StaticFunctionId,
        name: Option<crate::ast::StaticName>,
        params: &Rc<[StaticBinding]>,
        body: &[Stmt],
        constructable: bool,
    ) -> Result<()> {
        self.emit(BytecodeInstruction::CreateFunction {
            id,
            name,
            params: Rc::clone(params),
            bytecode: BytecodeFunction::compile(body)?,
            constructable,
        });
        Ok(())
    }

    fn compile_new_expr(&mut self, constructor: &StaticBinding, args: &[Expr]) -> Result<()> {
        self.compile_args(args)?;
        self.emit(BytecodeInstruction::Construct {
            constructor: constructor.clone(),
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
                self.emit(BytecodeInstruction::TypeOfBinding(name.clone()));
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
                self.emit(BytecodeInstruction::DeleteBinding(name.clone()));
                Ok(())
            }
            Expr::Member {
                object, property, ..
            } => {
                self.compile_expr(object)?;
                self.emit(BytecodeInstruction::DeleteStaticProperty {
                    property: property.clone(),
                });
                Ok(())
            }
            Expr::ComputedMember {
                object, property, ..
            } => {
                self.compile_expr(object)?;
                self.compile_expr(property)?;
                self.emit(BytecodeInstruction::DeleteComputedProperty);
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
                self.emit(BytecodeInstruction::Binary {
                    op,
                    property_access,
                });
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
                    name: name.clone(),
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
                    property: property.clone(),
                    access: *access,
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
                    access: *access,
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
                    name: name.clone(),
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
                    property: property.clone(),
                    access: *access,
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
                    access: *access,
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
            | BytecodeInstruction::TypeOfBinding(_)
            | BytecodeInstruction::TypeOfValue
            | BytecodeInstruction::DeleteBinding(_)
            | BytecodeInstruction::DeleteStaticProperty { .. }
            | BytecodeInstruction::DeleteComputedProperty
            | BytecodeInstruction::DeleteValue
            | BytecodeInstruction::UpdateBinding { .. }
            | BytecodeInstruction::UpdateStaticProperty { .. }
            | BytecodeInstruction::UpdateComputedProperty { .. }
            | BytecodeInstruction::Binary { .. }
            | BytecodeInstruction::CompoundStoreBinding { .. }
            | BytecodeInstruction::CompoundStaticProperty { .. }
            | BytecodeInstruction::CompoundComputedProperty { .. }
            | BytecodeInstruction::StaticMember { .. }
            | BytecodeInstruction::ComputedMember { .. }
            | BytecodeInstruction::StaticPropertyAssign { .. }
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

fn for_init_needs_lexical_scope(init: Option<&Stmt>) -> bool {
    match init {
        Some(Stmt::VarDecl {
            kind: DeclKind::Let | DeclKind::Const,
            ..
        }) => true,
        Some(Stmt::DeclList(statements)) => statements.iter().any(|statement| {
            matches!(
                statement,
                Stmt::VarDecl {
                    kind: DeclKind::Let | DeclKind::Const,
                    ..
                }
            )
        }),
        Some(_) | None => false,
    }
}
