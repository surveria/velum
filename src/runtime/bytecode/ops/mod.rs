mod assignment;
mod object_literal;

use crate::{
    bytecode::{
        BytecodeBinding, BytecodeDynamicProperty, BytecodeNumericBinaryOp,
        BytecodeNumericCompareOp, BytecodeNumericEqualityOp, BytecodeNumericUnaryOp,
    },
    error::{Error, Result},
    runtime::bytecode::coercion::relational_compare,
    runtime::call::RuntimeCallArgs,
    runtime::control::Completion,
    runtime::control::thrown_value_matches,
    runtime::native::NativeFunctionKind,
    runtime::numeric::{
        bitwise_and, bitwise_or, bitwise_xor, number_shift_count, number_to_i32, number_to_uint32,
        numeric_binary, shift_left, shift_right, shift_right_unsigned,
    },
    runtime::object::PropertyKey,
    runtime::property::DynamicPropertyKey,
    runtime::{
        Context,
        abstract_operations::{abstract_equality, number_strict_equality, strict_equality},
    },
    syntax::{BinaryOp, DeclKind, UnaryOp},
    value::{ErrorName, Value},
};

const INSTANCEOF_PROTOTYPE_PROPERTY: &str = "prototype";
const HAS_INSTANCE_SYMBOL_PROPERTY: &str = "hasInstance";
const HAS_INSTANCE_SYMBOL_DISPLAY: &str = "Symbol(Symbol.hasInstance)";
const HAS_INSTANCE_NOT_CALLABLE_ERROR: &str = "Symbol.hasInstance method is not callable";
const INSTANCEOF_NOT_CALLABLE_ERROR: &str = "right-hand side of 'instanceof' is not callable";
const INSTANCEOF_NON_OBJECT_PROTOTYPE_ERROR: &str =
    "right-hand side of 'instanceof' has non-object prototype";

impl Context {
    pub(super) fn eval_bytecode_declaration(
        &mut self,
        name: &BytecodeBinding,
        kind: DeclKind,
        value: Option<Value>,
    ) -> Result<()> {
        match kind {
            DeclKind::Var => {
                if let Some(value) = value {
                    self.assign_bytecode(name, value)?;
                }
            }
            DeclKind::Let => {
                self.define_static(
                    name.name(),
                    value.unwrap_or(Value::Undefined),
                    DeclKind::Let,
                )?;
            }
            DeclKind::Const => {
                let Some(value) = value else {
                    return Err(Error::runtime("const declaration requires an initializer"));
                };
                self.define_static(name.name(), value, DeclKind::Const)?;
            }
        }
        Ok(())
    }

    pub(super) fn eval_bytecode_identifier(&mut self, name: &BytecodeBinding) -> Result<Value> {
        if let Some(value) = self.unresolved_builtin_numeric_constant(name) {
            return Ok(value);
        }
        if let Some(binding) = self.get_binding_bytecode(name)? {
            return self.checked_value(binding.value(name.name())?);
        }
        self.builtin_value(name.name().name())?
            .ok_or_else(|| crate::runtime::control::reference_error_undefined(name.name()))
    }

    pub(super) fn eval_bytecode_typeof_binding(&mut self, name: &BytecodeBinding) -> Result<Value> {
        if self.unresolved_builtin_numeric_constant(name).is_some() {
            return self.heap_string_value(Value::Number(0.0).type_name());
        }
        if let Some(binding) = self.get_binding_bytecode(name)? {
            let value = binding.value(name.name())?;
            let type_name = self.semantic_type_name(&value)?;
            return self.heap_string_value(type_name);
        }
        if let Some(value) = self.builtin_value(name.name().name())? {
            let type_name = self.semantic_type_name(&value)?;
            return self.heap_string_value(type_name);
        }
        self.heap_string_value(Value::Undefined.type_name())
    }

    pub(super) fn eval_bytecode_unary(&mut self, op: UnaryOp, value: &Value) -> Result<Value> {
        match op {
            UnaryOp::Not => Ok(Value::Bool(!value.is_truthy())),
            UnaryOp::Negate => self.to_number(value).map(|value| Value::Number(-value)),
            UnaryOp::Plus => self.to_number(value).map(Value::Number),
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
        if let Value::Number(value) = value {
            let value = match op {
                BytecodeNumericUnaryOp::Negate => -*value,
                BytecodeNumericUnaryOp::Plus => *value,
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
            BinaryOp::Sub => numeric_binary(self, left, right, "-", |left, right| left - right)?,
            BinaryOp::Mul => numeric_binary(self, left, right, "*", |left, right| left * right)?,
            BinaryOp::Div => numeric_binary(self, left, right, "/", |left, right| left / right)?,
            BinaryOp::Rem => numeric_binary(self, left, right, "%", |left, right| left % right)?,
            BinaryOp::Pow => numeric_binary(self, left, right, "**", f64::powf)?,
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
        if let (Value::Number(left), Value::Number(right)) = (left, right) {
            let value = match op {
                BytecodeNumericBinaryOp::Add => left + right,
                BytecodeNumericBinaryOp::Sub => left - right,
                BytecodeNumericBinaryOp::Mul => left * right,
                BytecodeNumericBinaryOp::Div => left / right,
                BytecodeNumericBinaryOp::Rem => left % right,
                BytecodeNumericBinaryOp::Pow => left.powf(*right),
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
        if let (Value::Number(left), Value::Number(right)) = (left, right) {
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
        if let (Value::Number(left), Value::Number(right)) = (left, right) {
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
            let result = match self.eval_call_completion(&handler, &args, right.clone())? {
                Completion::Normal(value) => value,
                completion => return completion.into_result(),
            };
            return Ok(Value::Bool(result.is_truthy()));
        }
        let target = self.instanceof_target_prototype(right)?;
        let matches = if let Value::Error(error) = left {
            self.error_matches_instanceof(error.name(), right)?
        } else {
            self.value_prototype_chain_has_object(left, target)?
        };
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
        let handler = self.get_has_instance_property(right, symbol)?;
        match &handler {
            Value::Undefined | Value::Null => return Ok(None),
            Value::NativeFunction(id)
                if self.native_function(*id)?.kind()
                    == NativeFunctionKind::FunctionPrototypeHasInstance =>
            {
                return Ok(None);
            }
            _ => {}
        }
        if self.semantic_is_callable(&handler)? {
            return Ok(Some(handler));
        }
        Err(Error::type_error(HAS_INSTANCE_NOT_CALLABLE_ERROR))
    }

    fn has_instance_symbol(&mut self) -> Result<Option<crate::storage::symbol::SymbolId>> {
        let constructor = self.symbol_constructor_value()?;
        let value = self.get_property_value(&constructor, HAS_INSTANCE_SYMBOL_PROPERTY)?;
        Ok(match value {
            Value::Symbol(symbol) => Some(symbol.id()),
            _ => None,
        })
    }

    fn get_has_instance_property(
        &mut self,
        value: &Value,
        symbol: crate::storage::symbol::SymbolId,
    ) -> Result<Value> {
        let key = DynamicPropertyKey::new(
            HAS_INSTANCE_SYMBOL_DISPLAY.to_owned(),
            Some(PropertyKey::symbol(symbol)),
        );
        match value {
            Value::Function(_) | Value::NativeFunction(_) | Value::Object(_) => {
                self.get_property_value_with_lookup(value, key.lookup())
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_)
            | Value::HostFunction(_)
            | Value::Error(_) => Ok(Value::Undefined),
        }
    }

    fn instanceof_target_prototype(&mut self, right: &Value) -> Result<crate::value::ObjectId> {
        if !self.semantic_is_callable(right)? {
            return Err(Error::type_error(INSTANCEOF_NOT_CALLABLE_ERROR));
        }
        let prototype = self.get_property_value(right, INSTANCEOF_PROTOTYPE_PROPERTY)?;
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
        match value {
            Value::Object(id) => self.objects.prototype_chain_has_object(*id, target),
            Value::Function(id) => {
                let prototype = self.function_object_prototype_value(*id)?;
                self.prototype_value_chain_has_object(&prototype, target)
            }
            Value::NativeFunction(id) => {
                let prototype = self.native_function_object_prototype_value(*id)?;
                self.prototype_value_chain_has_object(&prototype, target)
            }
            Value::HostFunction(_) => {
                let prototype = self.function_constructor_prototype_value()?;
                self.prototype_value_chain_has_object(&prototype, target)
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_)
            | Value::Error(_) => Ok(false),
        }
    }

    fn prototype_value_chain_has_object(
        &self,
        prototype: &Value,
        target: crate::value::ObjectId,
    ) -> Result<bool> {
        let Value::Object(id) = prototype else {
            return Ok(false);
        };
        if *id == target {
            return Ok(true);
        }
        self.objects.prototype_chain_has_object(*id, target)
    }

    fn error_matches_instanceof(&self, name: ErrorName, right: &Value) -> Result<bool> {
        let Value::NativeFunction(id) = right else {
            return Ok(false);
        };
        let NativeFunctionKind::ErrorConstructor(expected) = self.native_function(*id)?.kind()
        else {
            return Ok(false);
        };
        if expected == ErrorName::Base {
            return Ok(name.is_standard());
        }
        Ok(name == expected)
    }

    pub(super) fn eval_bytecode_assert_throws(
        &mut self,
        expected: ErrorName,
        callback: &Value,
        message: Option<Value>,
    ) -> Result<Value> {
        if let Some(message) = message {
            self.runtime_value(message)?;
        }
        let Value::Function(id) = callback else {
            return Err(Error::runtime("assert.throws callback must be a function"));
        };
        let expected_name = expected.as_str();
        match self.eval_function_completion(*id, RuntimeCallArgs::values(&[]))? {
            Completion::Throw(value) if thrown_value_matches(&value, expected_name) => {
                Ok(Value::Undefined)
            }
            Completion::Throw(value) => Err(Error::runtime(format!(
                "assert.throws expected {expected_name}, got {value}"
            ))),
            Completion::Normal(_) | Completion::Return(_) => Err(Error::runtime(format!(
                "assert.throws expected {expected_name}, but no exception was thrown"
            ))),
            completion @ (Completion::Break { .. } | Completion::Continue(_)) => {
                completion.into_function_result()
            }
        }
    }
}
