use std::rc::Rc;

use crate::{
    bytecode::{BytecodeAddress, BytecodeProperty},
    error::{Error, Result},
    runtime::Context,
    runtime::control::Completion,
    runtime::function::FunctionSuperBinding,
    value::Value,
};

use super::state::BytecodeState;

const SUPER_OUTSIDE_CLASS_ERROR: &str = "'super' is not available in this context";
const SUPER_NOT_CONSTRUCTOR_ERROR: &str = "super constructor is not callable";

impl Context {
    fn super_frame(&self) -> Result<Rc<FunctionSuperBinding>> {
        self.current_super_frame()
            .ok_or_else(|| Error::type_error(SUPER_OUTSIDE_CLASS_ERROR))
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

    /// Calls the parent constructor with the already-created `this` so the
    /// parent chain initializes instance state, preserving `new.target`.
    fn eval_super_call(
        &mut self,
        state: &mut BytecodeState,
        args: &[Value],
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let frame = self.super_frame()?;
        let Some(constructor) = frame.constructor.clone() else {
            return Err(Error::type_error(SUPER_NOT_CONSTRUCTOR_ERROR));
        };
        let this_value = self.current_this()?;
        let new_target = self.current_new_target()?;
        let completion = match constructor {
            Value::Function(id) => {
                self.eval_class_super_constructor_completion(id, args, &this_value, new_target)?
            }
            // Native superclasses cannot initialize an existing instance;
            // run them for effect compatibility and keep the current this.
            Value::NativeFunction(_) => Completion::Normal(Value::Undefined),
            _ => return Err(Error::type_error(SUPER_NOT_CONSTRUCTOR_ERROR)),
        };
        let Completion::Normal(_) = completion else {
            return Ok(Some(completion));
        };
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
        let frame = self.super_frame()?;
        let value = self.get_named(&frame.home_prototype, property.name())?;
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
        let frame = self.super_frame()?;
        let callee = self.get_named(&frame.home_prototype, property.name())?;
        let this_value = self.current_this()?;
        let completion = self.call(&callee, args, this_value)?;
        let Completion::Normal(value) = completion else {
            return Ok(Some(completion));
        };
        state.stack.push(value);
        state.pc = next;
        Ok(None)
    }
}
