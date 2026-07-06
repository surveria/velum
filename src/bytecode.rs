use std::rc::Rc;

use crate::{
    ast::{
        BinaryOp, DeclKind, Expr, ObjectProperty, Program, StaticBinding, StaticName,
        StaticPropertyAccessId, StaticString, Stmt, UnaryOp,
    },
    bytecode_analysis::{block_can_inline, for_can_compile},
    bytecode_hoist::BytecodeHoistPlan,
    error::{Error, Result},
    value::Value,
};

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeProgram {
    instructions: Rc<[BytecodeInstruction]>,
    hoist_plan: BytecodeHoistPlan,
}

impl BytecodeProgram {
    pub fn compile(program: &Program) -> Result<Self> {
        let mut compiler = BytecodeCompiler::new();
        compiler.compile_statements(&program.statements, StatementValue::Store)?;
        Ok(Self {
            instructions: Rc::from(compiler.instructions.into_boxed_slice()),
            hoist_plan: BytecodeHoistPlan::compile(&program.statements),
        })
    }

    pub fn instruction(&self, address: BytecodeAddress) -> Result<Option<&BytecodeInstruction>> {
        let index = address.index();
        if index == self.instructions.len() {
            return Ok(None);
        }
        if index > self.instructions.len() {
            return Err(Error::runtime(
                "bytecode instruction pointer escaped program",
            ));
        }
        Ok(self.instructions.get(index))
    }

    pub fn instruction_count(&self) -> usize {
        self.instructions.len()
    }

    pub fn ast_fallback_instruction_count(&self) -> usize {
        self.instructions
            .iter()
            .filter(|instruction| instruction.is_ast_fallback())
            .count()
    }

    pub const fn hoist_plan(&self) -> &BytecodeHoistPlan {
        &self.hoist_plan
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeInstruction {
    PushLiteral(Value),
    PushString(StaticString),
    PushUndefined,
    LoadThis,
    LoadBinding(StaticBinding),
    StoreBinding(StaticBinding),
    DeclareBinding {
        name: StaticBinding,
        kind: DeclKind,
        has_init: bool,
    },
    SetLastUndefined,
    StoreLast,
    Pop,
    Unary(UnaryOp),
    Binary {
        op: BinaryOp,
        property_access: Option<StaticPropertyAccessId>,
    },
    StaticMember {
        property: StaticName,
        access: StaticPropertyAccessId,
    },
    ComputedMember {
        access: StaticPropertyAccessId,
    },
    StaticPropertyAssign {
        property: StaticName,
        access: StaticPropertyAccessId,
    },
    ComputedPropertyAssign {
        access: StaticPropertyAccessId,
    },
    ArrayLiteral {
        len: usize,
    },
    ObjectLiteral {
        properties: Rc<[StaticName]>,
    },
    Jump(BytecodeAddress),
    JumpIfFalse(BytecodeAddress),
    Complete(BytecodeCompletion),
    EvalAstExpr(Box<Expr>),
    EvalAstStatement(Box<Stmt>),
    EvalAstLoopStatement {
        statement: Box<Stmt>,
        break_target: BytecodeAddress,
        continue_target: BytecodeAddress,
    },
}

impl BytecodeInstruction {
    const fn is_ast_fallback(&self) -> bool {
        matches!(
            self,
            Self::EvalAstExpr(_) | Self::EvalAstStatement(_) | Self::EvalAstLoopStatement { .. }
        )
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeCompletion {
    Break,
    Continue,
    Return,
    Throw,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BytecodeAddress(usize);

impl BytecodeAddress {
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0
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
    loops: Vec<LoopPatch>,
}

impl BytecodeCompiler {
    const fn new() -> Self {
        Self {
            instructions: Vec::new(),
            loops: Vec::new(),
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
            Stmt::Block(statements) if block_can_inline(statements) => {
                self.compile_statements(statements, value)
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
            } if for_can_compile(init.as_deref(), body) => {
                self.compile_for(init.as_deref(), condition.as_ref(), update.as_ref(), body)
            }
            Stmt::Break => self.compile_loop_completion(BytecodeCompletion::Break),
            Stmt::Continue => self.compile_loop_completion(BytecodeCompletion::Continue),
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
            Stmt::Block(_)
            | Stmt::For { .. }
            | Stmt::ForIn { .. }
            | Stmt::Switch { .. }
            | Stmt::Try { .. } => self.compile_ast_statement(statement),
        }
    }

    fn compile_if(
        &mut self,
        condition: &Expr,
        consequent: &Stmt,
        alternate: Option<&Stmt>,
        value: StatementValue,
    ) -> Result<()> {
        self.compile_expr(condition)?;
        let false_jump = self.emit_jump_if_false();
        self.compile_statement(consequent, value)?;
        let end_jump = self.emit_jump();
        let alternate_address = self.current_address();
        self.patch_jump(false_jump, alternate_address)?;
        if let Some(alternate) = alternate {
            self.compile_statement(alternate, value)?;
        } else if value == StatementValue::Store {
            self.emit(BytecodeInstruction::SetLastUndefined);
        }
        let end_address = self.current_address();
        self.patch_jump(end_jump, end_address)
    }

    fn compile_while(&mut self, condition: &Expr, body: &Stmt) -> Result<()> {
        self.emit(BytecodeInstruction::SetLastUndefined);
        let start = self.current_address();
        self.compile_expr(condition)?;
        let false_jump = self.emit_jump_if_false();
        self.push_loop();
        self.compile_statement(body, StatementValue::Store)?;
        let loop_patch = self.pop_loop()?;
        self.emit(BytecodeInstruction::Jump(start));
        let end = self.current_address();
        self.patch_jump(false_jump, end)?;
        self.patch_loop(loop_patch, end, start)
    }

    fn compile_for(
        &mut self,
        init: Option<&Stmt>,
        condition: Option<&Expr>,
        update: Option<&Expr>,
        body: &Stmt,
    ) -> Result<()> {
        if let Some(init) = init {
            self.compile_statement(init, StatementValue::Discard)?;
        }
        self.emit(BytecodeInstruction::SetLastUndefined);
        let condition_address = self.current_address();
        let false_jump = if let Some(condition) = condition {
            self.compile_expr(condition)?;
            Some(self.emit_jump_if_false())
        } else {
            None
        };
        self.push_loop();
        self.compile_statement(body, StatementValue::Store)?;
        let loop_patch = self.pop_loop()?;
        let update_address = self.current_address();
        self.patch_loop_continues(&loop_patch, update_address)?;
        if let Some(update) = update {
            self.compile_expr(update)?;
            self.emit(BytecodeInstruction::Pop);
        }
        self.emit(BytecodeInstruction::Jump(condition_address));
        let end = self.current_address();
        if let Some(false_jump) = false_jump {
            self.patch_jump(false_jump, end)?;
        }
        self.patch_loop_breaks(loop_patch, end)
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
            Expr::Unary { op, expr } if unary_can_compile(*op) => {
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::Unary(*op));
            }
            Expr::Binary {
                op,
                left,
                right,
                property_access,
            } if binary_can_compile(*op) => {
                self.compile_expr(left)?;
                self.compile_expr(right)?;
                self.emit(BytecodeInstruction::Binary {
                    op: *op,
                    property_access: *property_access,
                });
            }
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
            Expr::Unary { .. }
            | Expr::Update { .. }
            | Expr::Binary { .. }
            | Expr::CompoundAssignment { .. }
            | Expr::Call { .. }
            | Expr::Function { .. }
            | Expr::MethodFunction { .. }
            | Expr::New { .. } => {
                self.emit(BytecodeInstruction::EvalAstExpr(Box::new(expr.clone())));
            }
        }
        Ok(())
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

    fn compile_loop_completion(&mut self, completion: BytecodeCompletion) -> Result<()> {
        if !matches!(
            completion,
            BytecodeCompletion::Break | BytecodeCompletion::Continue
        ) {
            self.emit(BytecodeInstruction::Complete(completion));
            return Ok(());
        }

        if self.loops.last().is_none() {
            self.emit(BytecodeInstruction::Complete(completion));
            return Ok(());
        }
        let jump = self.emit_jump();
        let Some(loop_patch) = self.loops.last_mut() else {
            return Err(Error::runtime("bytecode loop patch disappeared"));
        };
        match completion {
            BytecodeCompletion::Break => loop_patch.break_jumps.push(jump),
            BytecodeCompletion::Continue => loop_patch.continue_jumps.push(jump),
            BytecodeCompletion::Return | BytecodeCompletion::Throw => {}
        }
        Ok(())
    }

    fn compile_ast_statement(&mut self, statement: &Stmt) -> Result<()> {
        if self.loops.last().is_none() {
            self.emit(BytecodeInstruction::EvalAstStatement(Box::new(
                statement.clone(),
            )));
            return Ok(());
        }
        let index = self.emit(BytecodeInstruction::EvalAstLoopStatement {
            statement: Box::new(statement.clone()),
            break_target: BytecodeAddress::new(0),
            continue_target: BytecodeAddress::new(0),
        });
        let Some(loop_patch) = self.loops.last_mut() else {
            return Err(Error::runtime("bytecode loop patch disappeared"));
        };
        loop_patch.ast_loop_statements.push(index);
        Ok(())
    }

    fn push_loop(&mut self) {
        self.loops.push(LoopPatch::new());
    }

    fn pop_loop(&mut self) -> Result<LoopPatch> {
        self.loops
            .pop()
            .ok_or_else(|| Error::runtime("bytecode loop stack underflowed"))
    }

    fn patch_loop(
        &mut self,
        patch: LoopPatch,
        break_target: BytecodeAddress,
        continue_target: BytecodeAddress,
    ) -> Result<()> {
        self.patch_loop_continues(&patch, continue_target)?;
        self.patch_loop_breaks(patch, break_target)
    }

    fn patch_loop_continues(
        &mut self,
        patch: &LoopPatch,
        continue_target: BytecodeAddress,
    ) -> Result<()> {
        for jump in &patch.continue_jumps {
            self.patch_jump(*jump, continue_target)?;
        }
        for jump in &patch.ast_loop_statements {
            self.patch_jump(*jump, continue_target)?;
        }
        Ok(())
    }

    fn patch_loop_breaks(&mut self, patch: LoopPatch, break_target: BytecodeAddress) -> Result<()> {
        for jump in patch.break_jumps {
            self.patch_jump(jump, break_target)?;
        }
        for index in patch.ast_loop_statements {
            self.patch_ast_loop(index, break_target)?;
        }
        Ok(())
    }

    fn patch_ast_loop(
        &mut self,
        index: InstructionIndex,
        break_target: BytecodeAddress,
    ) -> Result<()> {
        let instruction = self
            .instructions
            .get_mut(index.index())
            .ok_or_else(|| Error::runtime("bytecode AST loop patch target disappeared"))?;
        let BytecodeInstruction::EvalAstLoopStatement {
            break_target: patched_break,
            continue_target,
            ..
        } = instruction
        else {
            return Err(Error::runtime(
                "bytecode AST loop patch target is not a loop statement",
            ));
        };
        *patched_break = break_target;
        if *continue_target == BytecodeAddress::new(0) {
            return Err(Error::runtime(
                "bytecode AST loop continue target was not patched",
            ));
        }
        Ok(())
    }

    fn emit_jump(&mut self) -> InstructionIndex {
        self.emit(BytecodeInstruction::Jump(BytecodeAddress::new(0)))
    }

    fn emit_jump_if_false(&mut self) -> InstructionIndex {
        self.emit(BytecodeInstruction::JumpIfFalse(BytecodeAddress::new(0)))
    }

    fn patch_jump(&mut self, index: InstructionIndex, target: BytecodeAddress) -> Result<()> {
        let instruction = self
            .instructions
            .get_mut(index.index())
            .ok_or_else(|| Error::runtime("bytecode jump patch target disappeared"))?;
        match instruction {
            BytecodeInstruction::Jump(address) | BytecodeInstruction::JumpIfFalse(address) => {
                *address = target;
                Ok(())
            }
            BytecodeInstruction::EvalAstLoopStatement {
                continue_target, ..
            } => {
                *continue_target = target;
                Ok(())
            }
            BytecodeInstruction::PushLiteral(_)
            | BytecodeInstruction::PushString(_)
            | BytecodeInstruction::PushUndefined
            | BytecodeInstruction::LoadThis
            | BytecodeInstruction::LoadBinding(_)
            | BytecodeInstruction::StoreBinding(_)
            | BytecodeInstruction::DeclareBinding { .. }
            | BytecodeInstruction::SetLastUndefined
            | BytecodeInstruction::StoreLast
            | BytecodeInstruction::Pop
            | BytecodeInstruction::Unary(_)
            | BytecodeInstruction::Binary { .. }
            | BytecodeInstruction::StaticMember { .. }
            | BytecodeInstruction::ComputedMember { .. }
            | BytecodeInstruction::StaticPropertyAssign { .. }
            | BytecodeInstruction::ComputedPropertyAssign { .. }
            | BytecodeInstruction::ArrayLiteral { .. }
            | BytecodeInstruction::ObjectLiteral { .. }
            | BytecodeInstruction::Complete(_)
            | BytecodeInstruction::EvalAstExpr(_)
            | BytecodeInstruction::EvalAstStatement(_) => Err(Error::runtime(
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

#[derive(Debug)]
struct LoopPatch {
    break_jumps: Vec<InstructionIndex>,
    continue_jumps: Vec<InstructionIndex>,
    ast_loop_statements: Vec<InstructionIndex>,
}

impl LoopPatch {
    const fn new() -> Self {
        Self {
            break_jumps: Vec::new(),
            continue_jumps: Vec::new(),
            ast_loop_statements: Vec::new(),
        }
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

const fn unary_can_compile(op: UnaryOp) -> bool {
    matches!(
        op,
        UnaryOp::Negate | UnaryOp::Plus | UnaryOp::Not | UnaryOp::Void
    )
}

const fn binary_can_compile(op: BinaryOp) -> bool {
    !matches!(op, BinaryOp::LogicalAnd | BinaryOp::LogicalOr)
}
