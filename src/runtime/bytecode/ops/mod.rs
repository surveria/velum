mod assignment;
mod object_literal;

pub(super) use assignment::{BytecodeAssignmentReference, web_compat_call_assignment_error};

use crate::{
    bytecode::{
        BytecodeBinding, BytecodeDestructureMode, BytecodeDynamicProperty, BytecodeNumericBinaryOp,
        BytecodeNumericCompareOp, BytecodeNumericEqualityOp, BytecodeNumericUnaryOp,
    },
    error::{Error, Result},
    runtime::bytecode::coercion::relational_compare,
    runtime::control::Completion,
    runtime::native::NativeFunctionKind,
    runtime::numeric::{
        bitwise_and, bitwise_not, bitwise_or, bitwise_xor, number_exponentiate, number_shift_count,
        number_to_i32, number_to_uint32, numeric_binary, shift_left, shift_right,
        shift_right_unsigned,
    },
    runtime::object::PropertyKey,
    runtime::property::DynamicPropertyKey,
    runtime::{
        Context,
        abstract_operations::{
            abstract_equality, number_strict_equality, strict_equality, to_boolean,
        },
    },
    syntax::{BinaryOp, DeclKind, UnaryOp},
    value::Value,
};

const INSTANCEOF_PROTOTYPE_PROPERTY: &str = "prototype";
const HAS_INSTANCE_SYMBOL_PROPERTY: &str = "hasInstance";
const HAS_INSTANCE_SYMBOL_DISPLAY: &str = "Symbol(Symbol.hasInstance)";
const INSTANCEOF_NOT_CALLABLE_ERROR: &str = "right-hand side of 'instanceof' is not callable";
const INSTANCEOF_NON_OBJECT_PROTOTYPE_ERROR: &str =
    "right-hand side of 'instanceof' has non-object prototype";

impl Context {
    pub(super) fn initialize_bytecode_pattern_binding(
        &mut self,
        name: &BytecodeBinding,
        mode: BytecodeDestructureMode,
        value: Value,
    ) -> Result<()> {
        match mode {
            BytecodeDestructureMode::Declaration(kind) => {
                self.eval_bytecode_declaration(name, kind, Some(value))
            }
            BytecodeDestructureMode::Parameter => self.initialize_bytecode_parameter(name, value),
            BytecodeDestructureMode::Assignment => Err(Error::runtime(
                "binding pattern leaf used by assignment destructuring",
            )),
        }
    }

    pub(super) fn eval_bytecode_declaration(
        &mut self,
        name: &BytecodeBinding,
        kind: DeclKind,
        value: Option<Value>,
    ) -> Result<()> {
        match kind {
            DeclKind::Var => {
                if let Some(value) = value {
                    self.assign_bytecode_or_create_sloppy_global(name, value)?;
                }
            }
            DeclKind::Let => {
                self.initialize_bytecode_lexical(
                    name,
                    value.unwrap_or(Value::Undefined),
                    DeclKind::Let,
                )?;
            }
            DeclKind::Const => {
                let Some(value) = value else {
                    return Err(Error::runtime("const declaration requires an initializer"));
                };
                self.initialize_bytecode_lexical(name, value, DeclKind::Const)?;
            }
            kind @ (DeclKind::Using | DeclKind::AwaitUsing) => {
                let Some(value) = value else {
                    return Err(Error::runtime(
                        "resource declaration requires an initializer",
                    ));
                };
                if kind.is_async_resource() {
                    self.register_await_using_resource(&value)?;
                } else {
                    self.register_using_resource(&value)?;
                }
                self.initialize_bytecode_lexical(name, value, kind)?;
            }
        }
        Ok(())
    }

    pub(super) fn eval_bytecode_identifier(&mut self, name: &BytecodeBinding) -> Result<Value> {
        if let Some(reference) = self.resolve_with_binding(name)? {
            let value = reference.get(self, name)?;
            return self.checked_value(value);
        }
        if let Some(value) = self.unresolved_builtin_numeric_constant(name) {
            return Ok(value);
        }
        if let Some(binding) = self.get_binding_bytecode(name)? {
            return self.checked_value(binding.value(name.name())?);
        }
        self.unresolved_global_property_value(name.name().name())?
            .ok_or_else(|| crate::runtime::control::reference_error_undefined(name.name()))
    }

    pub(super) fn eval_bytecode_typeof_binding(&mut self, name: &BytecodeBinding) -> Result<Value> {
        if let Some(reference) = self.resolve_with_binding(name)? {
            let value = reference.get(self, name)?;
            let type_name = self.semantic_type_name(&value)?;
            return self.heap_string_value(type_name);
        }
        if self.unresolved_builtin_numeric_constant(name).is_some() {
            return self.heap_string_value(Value::Number(0.0).type_name());
        }
        if let Some(binding) = self.get_binding_bytecode(name)? {
            let value = binding.value(name.name())?;
            let type_name = self.semantic_type_name(&value)?;
            return self.heap_string_value(type_name);
        }
        if let Some(value) = self.unresolved_global_property_value(name.name().name())? {
            let type_name = self.semantic_type_name(&value)?;
            return self.heap_string_value(type_name);
        }
        self.heap_string_value(Value::Undefined.type_name())
    }

    pub(super) fn eval_bytecode_unary(&mut self, op: UnaryOp, value: &Value) -> Result<Value> {
        match op {
            UnaryOp::Not => Ok(Value::Bool(!to_boolean(self, value)?)),
            UnaryOp::Negate => match self.to_numeric(value)? {
                crate::runtime::abstract_operations::NumericValue::Number(value) => {
                    Ok(Value::Number(-value))
                }
                crate::runtime::abstract_operations::NumericValue::BigInt(value) => {
                    self.bigint_value(value.negated())
                }
            },
            UnaryOp::Plus => self.to_number(value).map(Value::Number),
            UnaryOp::BitNot => bitwise_not(self, value),
            UnaryOp::Void => Ok(Value::Undefined),
            UnaryOp::Typeof | UnaryOp::Delete => Err(Error::runtime(
                "non-bytecode unary operator reached bytecode unary path",
            )),
        }
    }

    pub(super) fn eval_bytecode_number_unary(
        &mut self,
        op: BytecodeNumericUnaryOp,
        value: &Value,
    ) -> Result<Value> {
        if self.optional_optimizations_enabled()
            && let Value::Number(value) = value
        {
            let value = match op {
                BytecodeNumericUnaryOp::Negate => -*value,
                BytecodeNumericUnaryOp::Plus => *value,
                BytecodeNumericUnaryOp::BitNot => f64::from(!number_to_i32(*value, "~")?),
            };
            return self.checked_value(Value::Number(value));
        }
        self.eval_bytecode_unary(op.generic_unary(), value)
    }

    pub(super) fn eval_bytecode_binary(
        &mut self,
        op: BinaryOp,
        left: &Value,
        right: &Value,
        property_access: Option<BytecodeDynamicProperty>,
    ) -> Result<Value> {
        let value = match op {
            BinaryOp::Add => self.add(left, right)?,
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem | BinaryOp::Pow => {
                numeric_binary(self, left, right, op)?
            }
            BinaryOp::Equal => Value::Bool(abstract_equality(self, left, right)?),
            BinaryOp::NotEqual => Value::Bool(!abstract_equality(self, left, right)?),
            BinaryOp::StrictEqual => Value::Bool(strict_equality(left, right)),
            BinaryOp::StrictNotEqual => Value::Bool(!strict_equality(left, right)),
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                relational_compare(self, op, left, right)?
            }
            BinaryOp::In => self.eval_bytecode_in(left, right, property_access)?,
            BinaryOp::InstanceOf => self.eval_bytecode_instanceof(left, right)?,
            BinaryOp::BitAnd => bitwise_and(self, left, right)?,
            BinaryOp::BitOr => bitwise_or(self, left, right)?,
            BinaryOp::BitXor => bitwise_xor(self, left, right)?,
            BinaryOp::ShiftLeft => shift_left(self, left, right)?,
            BinaryOp::ShiftRight => shift_right(self, left, right)?,
            BinaryOp::ShiftRightUnsigned => shift_right_unsigned(self, left, right)?,
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr | BinaryOp::NullishCoalescing => {
                return Err(Error::runtime(
                    "logical operator reached bytecode eager evaluation",
                ));
            }
        };
        self.checked_value(value)
    }

    pub(super) fn eval_bytecode_number_binary(
        &mut self,
        op: BytecodeNumericBinaryOp,
        left: &Value,
        right: &Value,
    ) -> Result<Value> {
        if self.optional_optimizations_enabled()
            && let (Value::Number(left), Value::Number(right)) = (left, right)
        {
            let value = match op {
                BytecodeNumericBinaryOp::Add => left + right,
                BytecodeNumericBinaryOp::Sub => left - right,
                BytecodeNumericBinaryOp::Mul => left * right,
                BytecodeNumericBinaryOp::Div => left / right,
                BytecodeNumericBinaryOp::Rem => left % right,
                BytecodeNumericBinaryOp::Pow => number_exponentiate(*left, *right),
                BytecodeNumericBinaryOp::BitAnd => {
                    f64::from(number_to_i32(*left, "&")? & number_to_i32(*right, "&")?)
                }
                BytecodeNumericBinaryOp::BitOr => {
                    f64::from(number_to_i32(*left, "|")? | number_to_i32(*right, "|")?)
                }
                BytecodeNumericBinaryOp::BitXor => {
                    f64::from(number_to_i32(*left, "^")? ^ number_to_i32(*right, "^")?)
                }
                BytecodeNumericBinaryOp::ShiftLeft => {
                    let left = number_to_i32(*left, "<<")?;
                    let right = number_shift_count(*right, "<<")?;
                    f64::from(left.wrapping_shl(right))
                }
                BytecodeNumericBinaryOp::ShiftRight => {
                    let left = number_to_i32(*left, ">>")?;
                    let right = number_shift_count(*right, ">>")?;
                    f64::from(left.wrapping_shr(right))
                }
                BytecodeNumericBinaryOp::ShiftRightUnsigned => {
                    let left = number_to_uint32(*left, ">>>")?;
                    let right = number_shift_count(*right, ">>>")?;
                    f64::from(left.wrapping_shr(right))
                }
            };
            return self.checked_value(Value::Number(value));
        }
        self.eval_bytecode_binary(op.generic_binary(), left, right, None)
    }

    pub(super) fn eval_bytecode_number_compare(
        &mut self,
        op: BytecodeNumericCompareOp,
        left: &Value,
        right: &Value,
    ) -> Result<Value> {
        if self.optional_optimizations_enabled()
            && let (Value::Number(left), Value::Number(right)) = (left, right)
        {
            let value = match op {
                BytecodeNumericCompareOp::Less => left < right,
                BytecodeNumericCompareOp::LessEqual => left <= right,
                BytecodeNumericCompareOp::Greater => left > right,
                BytecodeNumericCompareOp::GreaterEqual => left >= right,
            };
            return self.checked_value(Value::Bool(value));
        }
        self.eval_bytecode_binary(op.generic_binary(), left, right, None)
    }

    pub(super) fn eval_bytecode_number_equality(
        &mut self,
        op: BytecodeNumericEqualityOp,
        left: &Value,
        right: &Value,
    ) -> Result<Value> {
        if self.optional_optimizations_enabled()
            && let (Value::Number(left), Value::Number(right)) = (left, right)
        {
            let equal = number_strict_equality(*left, *right);
            let value = match op {
                BytecodeNumericEqualityOp::Equal | BytecodeNumericEqualityOp::StrictEqual => equal,
                BytecodeNumericEqualityOp::NotEqual | BytecodeNumericEqualityOp::StrictNotEqual => {
                    !equal
                }
            };
            return self.checked_value(Value::Bool(value));
        }
        self.eval_bytecode_binary(op.generic_binary(), left, right, None)
    }

    pub(in crate::runtime) fn eval_bytecode_instanceof(
        &mut self,
        left: &Value,
        right: &Value,
    ) -> Result<Value> {
        if let Some(handler) = self.custom_has_instance_handler(right)? {
            let args = [left.clone()];
            let result = match self.call(&handler, &args, right.clone())? {
                Completion::Normal(value) => value,
                completion => return completion.into_result(),
            };
            return Ok(Value::Bool(to_boolean(self, &result)?));
        }
        if !self.semantic_is_callable(right)? {
            return Err(Error::type_error(INSTANCEOF_NOT_CALLABLE_ERROR));
        }
        if !matches!(
            left,
            Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
        ) {
            return Ok(Value::Bool(false));
        }
        let target = self.instanceof_target_prototype(right)?;
        let matches = self.value_prototype_chain_has_object(left, target)?;
        Ok(Value::Bool(matches))
    }

    /// Resolve a callable, non-builtin `@@hasInstance` method on the right
    /// operand of `instanceof`. Returns `None` when the method is absent or is
    /// the builtin `Function.prototype[@@hasInstance]` (so the ordinary
    /// prototype-chain check runs and no recursion occurs).
    fn custom_has_instance_handler(&mut self, right: &Value) -> Result<Option<Value>> {
        if !matches!(
            right,
            Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
        ) {
            return Ok(None);
        }
        let Some(symbol) = self.has_instance_symbol()? else {
            return Ok(None);
        };
        let Some(handler) = self.get_has_instance_method(right, symbol)? else {
            return Ok(None);
        };
        match &handler {
            Value::NativeFunction(id)
                if self.native_function(*id)?.kind()
                    == NativeFunctionKind::FunctionPrototypeHasInstance =>
            {
                return Ok(None);
            }
            _ => {}
        }
        Ok(Some(handler))
    }

    fn has_instance_symbol(&mut self) -> Result<Option<crate::storage::symbol::SymbolId>> {
        let constructor = self.symbol_constructor_value()?;
        let value = self.get_named(&constructor, HAS_INSTANCE_SYMBOL_PROPERTY)?;
        Ok(match value {
            Value::Symbol(symbol) => Some(symbol.id()),
            _ => None,
        })
    }

    fn get_has_instance_method(
        &mut self,
        value: &Value,
        symbol: crate::storage::symbol::SymbolId,
    ) -> Result<Option<Value>> {
        let key = DynamicPropertyKey::new(
            HAS_INSTANCE_SYMBOL_DISPLAY.to_owned(),
            Some(PropertyKey::symbol(symbol)),
        );
        match value {
            Value::Function(_) | Value::NativeFunction(_) | Value::Object(_) => {
                self.get_method(value, key.lookup())
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_)
            | Value::Symbol(_)
            | Value::HostFunction(_) => Ok(None),
        }
    }

    fn instanceof_target_prototype(&mut self, right: &Value) -> Result<crate::value::ObjectId> {
        if !self.semantic_is_callable(right)? {
            return Err(Error::type_error(INSTANCEOF_NOT_CALLABLE_ERROR));
        }
        let prototype = self.get_named(right, INSTANCEOF_PROTOTYPE_PROPERTY)?;
        let Value::Object(id) = prototype else {
            return Err(Error::type_error(INSTANCEOF_NON_OBJECT_PROTOTYPE_ERROR));
        };
        Ok(id)
    }

    fn value_prototype_chain_has_object(
        &mut self,
        value: &Value,
        target: crate::value::ObjectId,
    ) -> Result<bool> {
        let mut current = value.clone();
        loop {
            let Some(prototype) = self.semantic_get_prototype(&current)? else {
                return Ok(false);
            };
            if matches!(prototype, Value::Object(id) if id == target) {
                return Ok(true);
            }
            if matches!(prototype, Value::Null) {
                return Ok(false);
            }
            self.step()?;
            current = prototype;
        }
    }
}
