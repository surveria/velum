use std::rc::Rc;

use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBlock, BytecodeDynamicProperty, BytecodeInstruction,
        BytecodeProperty, BytecodeSuperProperty,
    },
    error::{Error, Result},
    runtime::Context,
    runtime::abstract_operations::{SetFailureBehavior, to_boolean},
    runtime::control::Completion,
    runtime::function::FunctionSuperBinding,
    syntax::{BinaryOp, UpdateOp},
    value::Value,
};

use super::state::BytecodeState;

const SUPER_OUTSIDE_CLASS_ERROR: &str = "'super' is not available in this context";
const SUPER_NOT_CONSTRUCTOR_ERROR: &str = "super constructor is not callable";
const SUPER_BASE_NULL_ERROR: &str = "super base is null";

impl Context {
    pub(super) fn eval_bytecode_super_mutation_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let value = match instruction {
            BytecodeInstruction::SuperPropertyAssign {
                property,
                value,
                strict,
            } => self.eval_bytecode_super_property_assignment(property, value, *strict)?,
            BytecodeInstruction::UpdateSuperProperty {
                property,
                op,
                prefix,
                strict,
            } => self.eval_bytecode_update_super_property(property, *op, *prefix, *strict)?,
            BytecodeInstruction::CompoundSuperProperty {
                property,
                op,
                value,
                strict,
            } => self.eval_bytecode_compound_super_property(property, *op, value, *strict)?,
            _ => {
                return Err(Error::runtime(
                    "bytecode super mutation instruction mismatch",
                ));
            }
        };
        state.stack.push(value);
        state.pc = next;
        Ok(None)
    }

    fn super_frame(&self) -> Result<Rc<FunctionSuperBinding>> {
        self.current_super_frame()
            .ok_or_else(|| Error::type_error(SUPER_OUTSIDE_CLASS_ERROR))
    }

    fn super_reference_parts(&mut self) -> Result<(Value, Value)> {
        let frame = self.super_frame()?;
        let receiver = self.current_this()?;
        let base = self
            .semantic_get_prototype(&frame.home_object)?
            .ok_or_else(|| Error::type_error(SUPER_OUTSIDE_CLASS_ERROR))?;
        if matches!(base, Value::Null) {
            return Err(Error::type_error(SUPER_BASE_NULL_ERROR));
        }
        Ok((base, receiver))
    }

    fn prepare_super_reference(
        &mut self,
        property: &BytecodeSuperProperty,
    ) -> Result<PreparedSuperReference> {
        let receiver = self.current_this()?;
        let property = match property {
            BytecodeSuperProperty::Static(property) => {
                PreparedSuperProperty::Static(crate::runtime::property::DynamicPropertyKey::new(
                    property.name().as_str().to_owned(),
                    self.known_property_key(property.name().as_str()),
                ))
            }
            BytecodeSuperProperty::Computed { expression, .. } => {
                PreparedSuperProperty::Computed(self.eval_bytecode_expression(expression)?)
            }
        };
        let frame = self.super_frame()?;
        let base = self
            .semantic_get_prototype(&frame.home_object)?
            .ok_or_else(|| Error::type_error(SUPER_OUTSIDE_CLASS_ERROR))?;
        if matches!(base, Value::Null) {
            return Err(Error::type_error(SUPER_BASE_NULL_ERROR));
        }
        Ok(PreparedSuperReference {
            base,
            receiver,
            property,
        })
    }

    fn finish_super_property(
        &mut self,
        property: PreparedSuperProperty,
    ) -> Result<crate::runtime::property::DynamicPropertyKey> {
        match property {
            PreparedSuperProperty::Static(property) => Ok(property),
            PreparedSuperProperty::Computed(value) => self.dynamic_property_key(&value),
        }
    }

    fn set_super_property(
        &mut self,
        reference: &PreparedSuperReference,
        property: &crate::runtime::property::DynamicPropertyKey,
        value: Value,
        strict: bool,
    ) -> Result<()> {
        let failure = if strict {
            SetFailureBehavior::Throw
        } else {
            SetFailureBehavior::ReturnFalse
        };
        self.set(
            &reference.base,
            property.lookup(),
            value,
            &reference.receiver,
            failure,
        )?;
        Ok(())
    }

    pub(super) fn eval_bytecode_super_property_assignment(
        &mut self,
        property: &BytecodeSuperProperty,
        value: &BytecodeBlock,
        strict: bool,
    ) -> Result<Value> {
        let reference = self.prepare_super_reference(property)?;
        let value = self.eval_bytecode_expression(value)?;
        let property = self.finish_super_property(reference.property.clone())?;
        self.set_super_property(&reference, &property, value.clone(), strict)?;
        self.runtime_value(value)
    }

    pub(super) fn eval_bytecode_update_super_property(
        &mut self,
        property: &BytecodeSuperProperty,
        op: UpdateOp,
        prefix: bool,
        strict: bool,
    ) -> Result<Value> {
        let reference = self.prepare_super_reference(property)?;
        let property = self.finish_super_property(reference.property.clone())?;
        let current =
            self.get_super_property(&reference.base, &reference.receiver, property.lookup())?;
        let updated = Self::updated_bytecode_number(&current, op)?;
        self.checked_value(updated.clone())?;
        self.set_super_property(&reference, &property, updated.clone(), strict)?;
        self.runtime_value(if prefix { updated } else { current })
    }

    pub(super) fn eval_bytecode_compound_super_property(
        &mut self,
        property: &BytecodeSuperProperty,
        op: BinaryOp,
        value: &BytecodeBlock,
        strict: bool,
    ) -> Result<Value> {
        let reference = self.prepare_super_reference(property)?;
        let property = self.finish_super_property(reference.property.clone())?;
        let current =
            self.get_super_property(&reference.base, &reference.receiver, property.lookup())?;
        if let Some(store) = logical_super_assignment_store(op, &current) {
            if !store {
                return self.runtime_value(current);
            }
            let value = self.eval_bytecode_expression(value)?;
            self.set_super_property(&reference, &property, value.clone(), strict)?;
            return self.runtime_value(value);
        }
        let right = self.eval_bytecode_expression(value)?;
        let updated = self.eval_bytecode_compound_value(op, &current, &right)?;
        self.set_super_property(&reference, &property, updated.clone(), strict)?;
        self.runtime_value(updated)
    }

    fn get_super_property(
        &mut self,
        base: &Value,
        receiver: &Value,
        property: crate::runtime::object::PropertyLookup<'_>,
    ) -> Result<Value> {
        let read = self
            .semantic_property_read_with_receiver(base, receiver, property)?
            .ok_or_else(|| Error::type_error(SUPER_BASE_NULL_ERROR))?;
        self.finish_semantic_property_read(read, receiver, property)
    }

    pub(super) fn eval_bytecode_call_super(
        &mut self,
        state: &mut BytecodeState,
        arg_count: usize,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let args = state.stack.pop_many(arg_count)?;
        self.eval_super_call(state, &args, next)
    }

    pub(super) fn eval_bytecode_call_super_spread(
        &mut self,
        state: &mut BytecodeState,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let packed = state.stack.pop()?;
        let args = self.spread_call_arguments(&packed)?;
        self.eval_super_call(state, &args, next)
    }

    /// Constructs the parent with the active `new.target`, then initializes
    /// this derived constructor's fields on the returned object.
    fn eval_super_call(
        &mut self,
        state: &mut BytecodeState,
        args: &[Value],
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let frame = self.super_frame()?;
        if frame.constructor.is_none() {
            return Err(Error::type_error(SUPER_NOT_CONSTRUCTOR_ERROR));
        }
        let constructor = self
            .semantic_get_prototype(&frame.home_object)?
            .ok_or_else(|| Error::type_error(SUPER_NOT_CONSTRUCTOR_ERROR))?;
        let new_target = self.current_new_target()?;
        let this_value = self
            .semantic_construct(&constructor, args, new_target)
            .map_err(|error| error.with_context(SUPER_NOT_CONSTRUCTOR_ERROR))?;
        if frame.this_value.borrow().is_some() {
            return Err(Error::exception(
                crate::value::ErrorName::ReferenceError,
                "super constructor has already initialized this",
            ));
        }
        *frame.this_value.borrow_mut() = Some(this_value.clone());
        // Derived-class instance fields initialize after super() completes.
        if let Some(own) = frame.own_constructor {
            self.initialize_class_fields(own, &this_value)?;
        }
        state.stack.push(this_value);
        state.pc = next;
        Ok(None)
    }

    pub(super) fn eval_bytecode_super_member(
        &mut self,
        state: &mut BytecodeState,
        property: &BytecodeProperty,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let (base, receiver) = self.super_reference_parts()?;
        let lookup = self.property_lookup(property.name());
        let value = self.get_super_property(&base, &receiver, lookup)?;
        state.stack.push(value);
        state.pc = next;
        Ok(None)
    }

    pub(super) fn eval_bytecode_computed_super_member(
        &mut self,
        state: &mut BytecodeState,
        expression: &BytecodeBlock,
        operand: BytecodeDynamicProperty,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let reference = self.prepare_super_reference(&BytecodeSuperProperty::Computed {
            expression: expression.clone(),
            operand,
        })?;
        let property = self.finish_super_property(reference.property.clone())?;
        let value =
            self.get_super_property(&reference.base, &reference.receiver, property.lookup())?;
        state.stack.push(value);
        state.pc = next;
        Ok(None)
    }

    pub(super) fn eval_bytecode_call_super_member(
        &mut self,
        state: &mut BytecodeState,
        property: &BytecodeProperty,
        arg_count: usize,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let args = state.stack.pop_many(arg_count)?;
        self.eval_super_member_call(state, property, &args, next)
    }

    pub(super) fn eval_bytecode_call_super_member_spread(
        &mut self,
        state: &mut BytecodeState,
        property: &BytecodeProperty,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let packed = state.stack.pop()?;
        let args = self.spread_call_arguments(&packed)?;
        self.eval_super_member_call(state, property, &args, next)
    }

    /// Calls a `super.method(...)` with the method resolved on the home
    /// prototype and `this` bound to the current instance.
    fn eval_super_member_call(
        &mut self,
        state: &mut BytecodeState,
        property: &BytecodeProperty,
        args: &[Value],
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let (base, receiver) = self.super_reference_parts()?;
        let lookup = self.property_lookup(property.name());
        let callee = self.get_super_property(&base, &receiver, lookup)?;
        let completion = self.call(&callee, args, receiver)?;
        let Completion::Normal(value) = completion else {
            return Ok(Some(completion));
        };
        state.stack.push(value);
        state.pc = next;
        Ok(None)
    }

    pub(super) fn eval_bytecode_call_computed_super_member(
        &mut self,
        state: &mut BytecodeState,
        _property: BytecodeDynamicProperty,
        arg_count: usize,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let args = state.stack.pop_many(arg_count)?;
        let property_value = state.stack.pop()?;
        let property = self.dynamic_property_key(&property_value)?;
        self.eval_computed_super_member_call(state, property.lookup(), &args, next)
    }

    pub(super) fn eval_bytecode_call_computed_super_member_spread(
        &mut self,
        state: &mut BytecodeState,
        _property: BytecodeDynamicProperty,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let packed = state.stack.pop()?;
        let args = self.spread_call_arguments(&packed)?;
        let property_value = state.stack.pop()?;
        let property = self.dynamic_property_key(&property_value)?;
        self.eval_computed_super_member_call(state, property.lookup(), &args, next)
    }

    fn eval_computed_super_member_call(
        &mut self,
        state: &mut BytecodeState,
        property: crate::runtime::object::PropertyLookup<'_>,
        args: &[Value],
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let (base, receiver) = self.super_reference_parts()?;
        let callee = self.get_super_property(&base, &receiver, property)?;
        let completion = self.call(&callee, args, receiver)?;
        let Completion::Normal(value) = completion else {
            return Ok(Some(completion));
        };
        state.stack.push(value);
        state.pc = next;
        Ok(None)
    }
}

#[derive(Clone)]
enum PreparedSuperProperty {
    Static(crate::runtime::property::DynamicPropertyKey),
    Computed(Value),
}

struct PreparedSuperReference {
    base: Value,
    receiver: Value,
    property: PreparedSuperProperty,
}

fn logical_super_assignment_store(op: BinaryOp, value: &Value) -> Option<bool> {
    match op {
        BinaryOp::LogicalAnd => Some(to_boolean(value)),
        BinaryOp::LogicalOr => Some(!to_boolean(value)),
        BinaryOp::NullishCoalescing => Some(matches!(value, Value::Undefined | Value::Null)),
        _ => None,
    }
}
