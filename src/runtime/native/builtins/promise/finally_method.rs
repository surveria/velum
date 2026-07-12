use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::NativeFunctionKind,
        object::{PropertyKey, PropertyLookup},
        promise::PromiseFinallyFunctionKind,
        roots::VmRootKind,
    },
    value::{ObjectId, Value},
};

use super::PROMISE_THEN_NAME;

const PROMISE_FINALLY_CONSTRUCTOR_PROPERTY: &str = "[[PromiseFinallyConstructor]]";
const PROMISE_FINALLY_HANDLER_PROPERTY: &str = "[[PromiseFinallyHandler]]";
const PROMISE_FINALLY_VALUE_PROPERTY: &str = "[[PromiseFinallyValue]]";
const SPECIES_PROPERTY: &str = "species";
const SPECIES_SYMBOL_DISPLAY: &str = "[Symbol.species]";

impl Context {
    pub(in crate::runtime::native) fn eval_promise_finally(
        &mut self,
        args: RuntimeCallArgs<'_>,
        promise: &Value,
    ) -> Result<Value> {
        if self.semantic_object_ref(promise)?.is_none() {
            return Err(Error::type_error(
                "Promise.prototype.finally receiver must be an object",
            ));
        }
        let constructor = self.promise_species_constructor(promise)?;
        let on_finally = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let (then_finally, catch_finally) = if self.semantic_is_callable(&on_finally)? {
            self.create_promise_finally_handlers(constructor, on_finally)?
        } else {
            (on_finally.clone(), on_finally)
        };
        let then = self.get_named(promise, PROMISE_THEN_NAME)?;
        let handlers: [Value; 2] = (then_finally, catch_finally).into();
        self.call_value(&then, &handlers, promise.clone())
    }

    pub(super) fn eval_promise_finally_function(
        &mut self,
        state: ObjectId,
        kind: PromiseFinallyFunctionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        match kind {
            PromiseFinallyFunctionKind::Then | PromiseFinallyFunctionKind::Catch => {
                self.eval_promise_finally_handler(state, kind, args)
            }
            PromiseFinallyFunctionKind::ValueThunk => self.promise_finally_forward_value(state),
            PromiseFinallyFunctionKind::Thrower => self.promise_finally_throw_value(state),
        }
    }

    fn eval_promise_finally_handler(
        &mut self,
        state: ObjectId,
        kind: PromiseFinallyFunctionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let state_value = Value::Object(state);
        let constructor = self.get_named(&state_value, PROMISE_FINALLY_CONSTRUCTOR_PROPERTY)?;
        let on_finally = self.get_named(&state_value, PROMISE_FINALLY_HANDLER_PROPERTY)?;
        let settled_value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let _root_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            [&constructor, &on_finally, &settled_value],
        )?;
        let result = self.call_value(&on_finally, &[], Value::Undefined)?;
        let promise = self.promise_resolve_with_constructor(&constructor, result)?;
        let forward_kind = match kind {
            PromiseFinallyFunctionKind::Then => PromiseFinallyFunctionKind::ValueThunk,
            PromiseFinallyFunctionKind::Catch => PromiseFinallyFunctionKind::Thrower,
            PromiseFinallyFunctionKind::ValueThunk | PromiseFinallyFunctionKind::Thrower => {
                return Err(Error::runtime("Promise finally handler kind is invalid"));
            }
        };
        let forwarder = self.create_promise_finally_forwarder(settled_value, forward_kind)?;
        let then = self.get_named(&promise, PROMISE_THEN_NAME)?;
        self.call_value(&then, std::slice::from_ref(&forwarder), promise)
    }

    fn promise_finally_forward_value(&mut self, state: ObjectId) -> Result<Value> {
        self.get_named(&Value::Object(state), PROMISE_FINALLY_VALUE_PROPERTY)
    }

    fn promise_finally_throw_value(&mut self, state: ObjectId) -> Result<Value> {
        let value = self.get_named(&Value::Object(state), PROMISE_FINALLY_VALUE_PROPERTY)?;
        Err(Error::javascript(value))
    }

    pub(super) fn promise_species_constructor(&mut self, promise: &Value) -> Result<Value> {
        let default = self.promise_constructor_value()?;
        let constructor = self.get_named(promise, "constructor")?;
        if matches!(constructor, Value::Undefined) {
            return Ok(default);
        }
        if self.semantic_object_ref(&constructor)?.is_none() {
            return Err(Error::type_error(
                "Promise species constructor property must be an object",
            ));
        }
        let symbol_constructor = self.symbol_constructor_value()?;
        let species_symbol = self.get_named(&symbol_constructor, SPECIES_PROPERTY)?;
        let Value::Symbol(species_symbol) = species_symbol else {
            return Err(Error::runtime("Symbol.species is not initialized"));
        };
        let lookup = PropertyLookup::from_key(
            SPECIES_SYMBOL_DISPLAY,
            PropertyKey::symbol(species_symbol.id()),
        );
        let species = self.get(&constructor, lookup)?;
        if matches!(species, Value::Undefined | Value::Null) {
            return Ok(default);
        }
        if !self.semantic_is_constructor(&species)? {
            return Err(Error::type_error(
                "Promise species value must be a constructor",
            ));
        }
        Ok(species)
    }

    fn create_promise_finally_handlers(
        &mut self,
        constructor: Value,
        on_finally: Value,
    ) -> Result<(Value, Value)> {
        let state = self.create_promise_internal_state()?;
        self.define_non_enumerable_object_property(
            state,
            PROMISE_FINALLY_CONSTRUCTOR_PROPERTY,
            constructor,
        )?;
        self.define_non_enumerable_object_property(
            state,
            PROMISE_FINALLY_HANDLER_PROPERTY,
            on_finally,
        )?;
        let then_finally = self.create_ephemeral_native_function(
            NativeFunctionKind::PromiseFinallyFunction {
                state,
                kind: PromiseFinallyFunctionKind::Then,
            },
            Value::Undefined,
        )?;
        let catch_finally = self.create_ephemeral_native_function(
            NativeFunctionKind::PromiseFinallyFunction {
                state,
                kind: PromiseFinallyFunctionKind::Catch,
            },
            Value::Undefined,
        )?;
        Ok((then_finally, catch_finally))
    }

    fn create_promise_finally_forwarder(
        &mut self,
        value: Value,
        kind: PromiseFinallyFunctionKind,
    ) -> Result<Value> {
        let state = self.create_promise_internal_state()?;
        self.define_non_enumerable_object_property(state, PROMISE_FINALLY_VALUE_PROPERTY, value)?;
        self.create_ephemeral_native_function(
            NativeFunctionKind::PromiseFinallyFunction { state, kind },
            Value::Undefined,
        )
    }
}
