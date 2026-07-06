use std::rc::Rc;

use crate::{
    ast::{BinaryOp, DeclKind, StaticName, UnaryOp},
    bytecode::{BytecodeAddress, BytecodeCompletion, BytecodeInstruction, BytecodeProgram},
    error::{Error, Result},
    runtime::Context,
    runtime_completion::Completion,
    runtime_numeric::{
        bitwise_and, bitwise_or, bitwise_xor, compare_binary, numeric_binary, shift_left,
        shift_right, shift_right_unsigned,
    },
    runtime_object::{OBJECT_CONSTRUCTOR_PROPERTY, ObjectPropertyInit, PropertyEnumerable},
    value::Value,
};

impl Context {
    pub(crate) fn eval_bytecode_program(
        &mut self,
        bytecode: &BytecodeProgram,
    ) -> Result<Completion> {
        let mut state = BytecodeState::new();
        while let Some(instruction) = bytecode.instruction(state.pc)? {
            self.step()?;
            if let Some(completion) = self.eval_bytecode_instruction(&mut state, instruction)? {
                return Ok(completion);
            }
        }
        Ok(Completion::Normal(state.last))
    }

    fn eval_bytecode_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
    ) -> Result<Option<Completion>> {
        let next = state.next_pc()?;
        match instruction {
            BytecodeInstruction::PushLiteral(_)
            | BytecodeInstruction::PushString(_)
            | BytecodeInstruction::PushUndefined
            | BytecodeInstruction::LoadThis
            | BytecodeInstruction::LoadBinding(_)
            | BytecodeInstruction::StoreBinding(_)
            | BytecodeInstruction::DeclareBinding { .. }
            | BytecodeInstruction::SetLastUndefined
            | BytecodeInstruction::StoreLast
            | BytecodeInstruction::Pop => {
                self.eval_bytecode_stack_instruction(state, instruction, next)?;
                Ok(None)
            }
            BytecodeInstruction::Unary(_)
            | BytecodeInstruction::Binary { .. }
            | BytecodeInstruction::StaticMember { .. }
            | BytecodeInstruction::ComputedMember { .. }
            | BytecodeInstruction::StaticPropertyAssign { .. }
            | BytecodeInstruction::ComputedPropertyAssign { .. }
            | BytecodeInstruction::ArrayLiteral { .. }
            | BytecodeInstruction::ObjectLiteral { .. } => {
                self.eval_bytecode_value_instruction(state, instruction, next)?;
                Ok(None)
            }
            BytecodeInstruction::Jump(target) => {
                state.pc = *target;
                Ok(None)
            }
            BytecodeInstruction::JumpIfFalse(target) => {
                let value = state.stack.pop()?;
                state.pc = if value.is_truthy() { next } else { *target };
                Ok(None)
            }
            BytecodeInstruction::Complete(completion) => state.complete(*completion).map(Some),
            BytecodeInstruction::EvalAstExpr(_)
            | BytecodeInstruction::EvalAstStatement(_)
            | BytecodeInstruction::EvalAstLoopStatement { .. } => {
                self.eval_bytecode_ast_instruction(state, instruction, next)
            }
        }
    }

    fn eval_bytecode_stack_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<()> {
        match instruction {
            BytecodeInstruction::PushLiteral(value) => {
                state.stack.push(self.runtime_value(value.clone())?);
            }
            BytecodeInstruction::PushString(value) => {
                state.stack.push(self.static_string_value(value)?);
            }
            BytecodeInstruction::PushUndefined => {
                state.stack.push(Value::Undefined);
            }
            BytecodeInstruction::LoadThis => {
                state.stack.push(self.current_this()?);
            }
            BytecodeInstruction::LoadBinding(binding) => {
                state.stack.push(self.eval_identifier(binding)?);
            }
            BytecodeInstruction::StoreBinding(binding) => {
                let value = state.stack.pop()?;
                self.assign_static_or_builtin(binding, value.clone())?;
                state.stack.push(value);
            }
            BytecodeInstruction::DeclareBinding {
                name,
                kind,
                has_init,
            } => {
                let value = if *has_init {
                    Some(state.stack.pop()?)
                } else {
                    None
                };
                self.eval_bytecode_declaration(name, *kind, value)?;
                state.last = Value::Undefined;
            }
            BytecodeInstruction::SetLastUndefined => {
                state.last = Value::Undefined;
            }
            BytecodeInstruction::StoreLast => {
                state.last = state.stack.pop()?;
            }
            BytecodeInstruction::Pop => {
                state.stack.pop()?;
            }
            BytecodeInstruction::Unary(_)
            | BytecodeInstruction::Binary { .. }
            | BytecodeInstruction::StaticMember { .. }
            | BytecodeInstruction::ComputedMember { .. }
            | BytecodeInstruction::StaticPropertyAssign { .. }
            | BytecodeInstruction::ComputedPropertyAssign { .. }
            | BytecodeInstruction::ArrayLiteral { .. }
            | BytecodeInstruction::ObjectLiteral { .. }
            | BytecodeInstruction::Jump(_)
            | BytecodeInstruction::JumpIfFalse(_)
            | BytecodeInstruction::Complete(_)
            | BytecodeInstruction::EvalAstExpr(_)
            | BytecodeInstruction::EvalAstStatement(_)
            | BytecodeInstruction::EvalAstLoopStatement { .. } => {
                return Err(Error::runtime("bytecode stack instruction mismatch"));
            }
        }
        state.pc = next;
        Ok(())
    }

    fn eval_bytecode_value_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<()> {
        match instruction {
            BytecodeInstruction::Unary(op) => {
                let value = state.stack.pop()?;
                state.stack.push(Self::eval_bytecode_unary(*op, &value)?);
            }
            BytecodeInstruction::Binary {
                op,
                property_access,
            } => {
                let right = state.stack.pop()?;
                let left = state.stack.pop()?;
                state.stack.push(self.eval_bytecode_binary(
                    *op,
                    &left,
                    &right,
                    *property_access,
                )?);
            }
            BytecodeInstruction::StaticMember { property, access } => {
                let object = state.stack.pop()?;
                state
                    .stack
                    .push(self.get_static_property_value(&object, property, *access)?);
            }
            BytecodeInstruction::ComputedMember { access } => {
                let property = state.stack.pop()?;
                let object = state.stack.pop()?;
                let property = self.dynamic_property_key(&property)?;
                state
                    .stack
                    .push(self.get_cached_dynamic_property_value(&object, &property, *access)?);
            }
            BytecodeInstruction::StaticPropertyAssign { property, access } => {
                let value = state.stack.pop()?;
                let object = state.stack.pop()?;
                self.set_static_property_value(&object, property, *access, value.clone())?;
                state.stack.push(value);
            }
            BytecodeInstruction::ComputedPropertyAssign { access } => {
                let value = state.stack.pop()?;
                let property = state.stack.pop()?;
                let object = state.stack.pop()?;
                let mut property = self.dynamic_property_key(&property)?;
                self.set_cached_dynamic_property_value(
                    &object,
                    &mut property,
                    *access,
                    value.clone(),
                )?;
                state.stack.push(value);
            }
            BytecodeInstruction::ArrayLiteral { len } => {
                let values = state.stack.pop_many(*len)?;
                state.stack.push(self.create_array_from_elements(values)?);
            }
            BytecodeInstruction::ObjectLiteral { properties } => {
                let values = state.stack.pop_many(properties.len())?;
                state
                    .stack
                    .push(self.create_bytecode_object_literal(properties, values)?);
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
            | BytecodeInstruction::Jump(_)
            | BytecodeInstruction::JumpIfFalse(_)
            | BytecodeInstruction::Complete(_)
            | BytecodeInstruction::EvalAstExpr(_)
            | BytecodeInstruction::EvalAstStatement(_)
            | BytecodeInstruction::EvalAstLoopStatement { .. } => {
                return Err(Error::runtime("bytecode value instruction mismatch"));
            }
        }
        state.pc = next;
        Ok(())
    }

    fn eval_bytecode_ast_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::EvalAstExpr(expr) => {
                state.stack.push(self.eval_expr(expr)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::EvalAstStatement(statement) => {
                match self.eval_statement(statement)? {
                    Completion::Normal(value) => state.last = value,
                    completion => return Ok(Some(completion)),
                }
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::EvalAstLoopStatement {
                statement,
                break_target,
                continue_target,
            } => {
                match self.eval_statement(statement)? {
                    Completion::Normal(value) => {
                        state.last = value;
                        state.pc = next;
                    }
                    Completion::Break => state.pc = *break_target,
                    Completion::Continue => state.pc = *continue_target,
                    completion @ (Completion::Throw(_) | Completion::Return(_)) => {
                        return Ok(Some(completion));
                    }
                }
                Ok(None)
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
            | BytecodeInstruction::Jump(_)
            | BytecodeInstruction::JumpIfFalse(_)
            | BytecodeInstruction::Complete(_) => {
                Err(Error::runtime("bytecode AST instruction mismatch"))
            }
        }
    }

    fn eval_bytecode_declaration(
        &mut self,
        name: &crate::ast::StaticBinding,
        kind: DeclKind,
        value: Option<Value>,
    ) -> Result<()> {
        match kind {
            DeclKind::Var => {
                if let Some(value) = value {
                    self.assign_static(name, value)?;
                }
            }
            DeclKind::Let => {
                self.define_static(name, value.unwrap_or(Value::Undefined), DeclKind::Let)?;
            }
            DeclKind::Const => {
                let Some(value) = value else {
                    return Err(Error::runtime("const declaration requires an initializer"));
                };
                self.define_static(name, value, DeclKind::Const)?;
            }
        }
        Ok(())
    }

    fn eval_bytecode_unary(op: UnaryOp, value: &Value) -> Result<Value> {
        match op {
            UnaryOp::Not => Ok(Value::Bool(!value.is_truthy())),
            UnaryOp::Negate => value
                .as_number()
                .map(|value| Value::Number(-value))
                .ok_or_else(|| Error::runtime("unary '-' expects a number")),
            UnaryOp::Plus => value
                .as_number()
                .map(Value::Number)
                .ok_or_else(|| Error::runtime("unary '+' expects a number")),
            UnaryOp::Void => Ok(Value::Undefined),
            UnaryOp::Typeof | UnaryOp::Delete => Err(Error::runtime(
                "non-bytecode unary operator reached bytecode unary path",
            )),
        }
    }

    fn eval_bytecode_binary(
        &mut self,
        op: BinaryOp,
        left: &Value,
        right: &Value,
        property_access: Option<crate::ast::StaticPropertyAccessId>,
    ) -> Result<Value> {
        let value = match op {
            BinaryOp::Add => self.add(left, right)?,
            BinaryOp::Sub => numeric_binary(left, right, "-", |left, right| left - right)?,
            BinaryOp::Mul => numeric_binary(left, right, "*", |left, right| left * right)?,
            BinaryOp::Div => numeric_binary(left, right, "/", |left, right| left / right)?,
            BinaryOp::Rem => numeric_binary(left, right, "%", |left, right| left % right)?,
            BinaryOp::Pow => numeric_binary(left, right, "**", f64::powf)?,
            BinaryOp::Equal | BinaryOp::StrictEqual => Value::Bool(left == right),
            BinaryOp::NotEqual | BinaryOp::StrictNotEqual => Value::Bool(left != right),
            BinaryOp::Less => compare_binary(left, right, "<", |left, right| left < right)?,
            BinaryOp::LessEqual => compare_binary(left, right, "<=", |left, right| left <= right)?,
            BinaryOp::Greater => compare_binary(left, right, ">", |left, right| left > right)?,
            BinaryOp::GreaterEqual => {
                compare_binary(left, right, ">=", |left, right| left >= right)?
            }
            BinaryOp::In => self.eval_bytecode_in(left, right, property_access)?,
            BinaryOp::BitAnd => bitwise_and(left, right)?,
            BinaryOp::BitOr => bitwise_or(left, right)?,
            BinaryOp::BitXor => bitwise_xor(left, right)?,
            BinaryOp::ShiftLeft => shift_left(left, right)?,
            BinaryOp::ShiftRight => shift_right(left, right)?,
            BinaryOp::ShiftRightUnsigned => shift_right_unsigned(left, right)?,
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
                return Err(Error::runtime(
                    "logical operator reached bytecode eager evaluation",
                ));
            }
        };
        self.checked_value(value)
    }

    fn eval_bytecode_in(
        &self,
        left: &Value,
        right: &Value,
        property_access: Option<crate::ast::StaticPropertyAccessId>,
    ) -> Result<Value> {
        let property = self.dynamic_property_key(left)?;
        if let Some(access) = property_access {
            return self
                .has_cached_dynamic_property_value(right, &property, access)
                .map(Value::Bool);
        }
        self.has_dynamic_property_value(right, &property)
            .map(Value::Bool)
    }

    fn create_bytecode_object_literal(
        &mut self,
        properties: &Rc<[StaticName]>,
        values: Vec<Value>,
    ) -> Result<Value> {
        if properties.len() != values.len() {
            return Err(Error::runtime(
                "bytecode object literal stack arity mismatch",
            ));
        }
        let mut inits = Vec::with_capacity(properties.len());
        for (property, value) in properties.iter().zip(values) {
            let key = self.intern_static_property_key(property)?;
            inits.push(ObjectPropertyInit::new(
                key,
                property.as_str(),
                value,
                PropertyEnumerable::Yes,
            ));
        }
        let constructor_key = self.intern_property_key(OBJECT_CONSTRUCTOR_PROPERTY)?;
        self.objects.create(
            inits,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }
}

#[derive(Debug)]
struct BytecodeState {
    pc: BytecodeAddress,
    stack: BytecodeStack,
    last: Value,
}

impl BytecodeState {
    const fn new() -> Self {
        Self {
            pc: BytecodeAddress::new(0),
            stack: BytecodeStack::new(),
            last: Value::Undefined,
        }
    }

    fn next_pc(&self) -> Result<BytecodeAddress> {
        let next = self
            .pc
            .index()
            .checked_add(1)
            .ok_or_else(|| Error::runtime("bytecode instruction pointer overflowed"))?;
        Ok(BytecodeAddress::new(next))
    }

    fn complete(&mut self, completion: BytecodeCompletion) -> Result<Completion> {
        match completion {
            BytecodeCompletion::Break => Ok(Completion::Break),
            BytecodeCompletion::Continue => Ok(Completion::Continue),
            BytecodeCompletion::Return => Ok(Completion::Return(self.stack.pop_single()?)),
            BytecodeCompletion::Throw => Ok(Completion::Throw(self.stack.pop_single()?)),
        }
    }
}

#[derive(Debug)]
struct BytecodeStack {
    values: Vec<Value>,
}

impl BytecodeStack {
    const fn new() -> Self {
        Self { values: Vec::new() }
    }

    fn push(&mut self, value: Value) {
        self.values.push(value);
    }

    fn pop(&mut self) -> Result<Value> {
        self.values
            .pop()
            .ok_or_else(|| Error::runtime("bytecode stack underflowed"))
    }

    fn pop_many(&mut self, count: usize) -> Result<Vec<Value>> {
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(self.pop()?);
        }
        values.reverse();
        Ok(values)
    }

    fn pop_single(&mut self) -> Result<Value> {
        let value = self.pop()?;
        if !self.values.is_empty() {
            return Err(Error::runtime(
                "bytecode completion left extra stack values",
            ));
        }
        Ok(value)
    }
}
