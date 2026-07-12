use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::same_value,
        call::RuntimeCallArgs,
        native::NativeFunctionKind,
        trace::{StrongEdgeReference, StrongEdgeVisitor, VmCallableEdgeKind},
    },
    value::{BoundFunctionId, Value},
};

const CALL_TARGET_NOT_CALLABLE_ERROR: &str = "Function.prototype.call target is not callable";
const BIND_TARGET_NOT_CALLABLE_ERROR: &str = "Function.prototype.bind target is not callable";
const APPLY_TARGET_NOT_CALLABLE_ERROR: &str = "Function.prototype.apply target is not callable";
const APPLY_ARGUMENTS_NOT_ARRAY_LIKE_ERROR: &str =
    "Function.prototype.apply arguments must be an array-like object";
const ARRAY_LIKE_LENGTH_PROPERTY: &str = "length";

#[derive(Debug, Clone)]
pub(in crate::runtime) struct BoundFunction {
    target: Value,
    behavior: BoundFunctionBehavior,
}

#[derive(Debug, Clone)]
enum BoundFunctionBehavior {
    Ordinary { this_value: Value, args: Vec<Value> },
    ShadowRealm,
}

impl BoundFunction {
    const fn ordinary(target: Value, this_value: Value, args: Vec<Value>) -> Self {
        Self {
            target,
            behavior: BoundFunctionBehavior::Ordinary { this_value, args },
        }
    }

    const fn shadow_realm(target: Value) -> Self {
        Self {
            target,
            behavior: BoundFunctionBehavior::ShadowRealm,
        }
    }

    const fn is_shadow_realm(&self) -> bool {
        matches!(self.behavior, BoundFunctionBehavior::ShadowRealm)
    }
}

impl BoundFunction {
    pub(in crate::runtime) fn visit_strong_edges<V: StrongEdgeVisitor<VmCallableEdgeKind>>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        visitor.visit(
            VmCallableEdgeKind::BoundFunctionInternal,
            StrongEdgeReference::Value(&self.target),
        )?;
        if let BoundFunctionBehavior::Ordinary { this_value, args } = &self.behavior {
            visitor.visit(
                VmCallableEdgeKind::BoundFunctionInternal,
                StrongEdgeReference::Value(this_value),
            )?;
            for arg in args {
                visitor.visit(
                    VmCallableEdgeKind::BoundFunctionInternal,
                    StrongEdgeReference::Value(arg),
                )?;
            }
        }
        Ok(())
    }
}

impl Context {
    pub(in crate::runtime) fn eval_function_prototype_call(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if !self.semantic_is_callable(this_value)? {
            return Err(Error::type_error(CALL_TARGET_NOT_CALLABLE_ERROR));
        }
        let (call_this, call_args): (Value, &[Value]) =
            if let Some((this_arg, call_args)) = args.as_slice().split_first() {
                (this_arg.clone(), call_args)
            } else {
                (Value::Undefined, &[])
            };
        self.call_value(this_value, call_args, call_this)
    }

    pub(in crate::runtime) fn eval_function_prototype_apply(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if !self.semantic_is_callable(this_value)? {
            return Err(Error::type_error(APPLY_TARGET_NOT_CALLABLE_ERROR));
        }
        let slice = args.as_slice();
        let this_arg = slice.first().cloned().unwrap_or(Value::Undefined);
        let args_array = slice.get(1).cloned().unwrap_or(Value::Undefined);
        let call_args = self.create_list_from_array_like(&args_array)?;
        self.call_value(this_value, &call_args, this_arg)
    }

    pub(in crate::runtime) fn eval_function_prototype_has_instance(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if !self.semantic_is_callable(this_value)? {
            return Ok(Value::Bool(false));
        }
        let value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        // OrdinaryHasInstance(this, value) is `value instanceof this`.
        self.eval_bytecode_instanceof(&value, this_value)
    }

    /// Spec `CreateListFromArrayLike` restricted to the default element types.
    fn create_list_from_array_like(&mut self, value: &Value) -> Result<Vec<Value>> {
        if matches!(value, Value::Undefined | Value::Null) {
            return Ok(Vec::new());
        }
        let Value::Object(id) = value else {
            return Err(Error::type_error(APPLY_ARGUMENTS_NOT_ARRAY_LIKE_ERROR));
        };
        if let Some(values) = self.objects.packed_default_array_values_if_array(*id)? {
            return Ok(values);
        }
        let length_value = self.get_named(value, ARRAY_LIKE_LENGTH_PROPERTY)?;
        let length = self.array_like_length_from_value(&length_value)?;
        let mut list = Vec::new();
        for index in 0..length {
            self.step()?;
            let element = self.get_named(value, &index.to_string())?;
            list.push(element);
        }
        Ok(list)
    }

    fn array_like_length_from_value(&mut self, value: &Value) -> Result<usize> {
        let length = self.to_length(value)?;
        Self::length_to_usize(
            length,
            "apply argument list length exceeded supported range",
        )
    }

    pub(in crate::runtime) fn eval_function_prototype_bind(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if !self.semantic_is_callable(this_value)? {
            return Err(Error::type_error(BIND_TARGET_NOT_CALLABLE_ERROR));
        }
        let (bound_this, bound_args) =
            if let Some((this_arg, bound_args)) = args.as_slice().split_first() {
                (this_arg.clone(), bound_args.to_vec())
            } else {
                (Value::Undefined, Vec::new())
            };
        self.create_bound_function(this_value.clone(), bound_this, bound_args)
    }

    pub(in crate::runtime) fn eval_bound_function(
        &mut self,
        id: BoundFunctionId,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let function = self.bound_function(id)?.clone();
        if function.is_shadow_realm() {
            return self.eval_shadow_realm_wrapped_call(&function.target, args.as_slice());
        }
        let BoundFunctionBehavior::Ordinary {
            this_value,
            args: bound_args,
        } = function.behavior
        else {
            return Err(Error::runtime("bound function behavior is invalid"));
        };
        let call_args = args.as_slice();
        let capacity = bound_args
            .len()
            .checked_add(call_args.len())
            .ok_or_else(|| Error::limit("bound function argument count overflowed"))?;
        let mut values = Vec::with_capacity(capacity);
        values.extend_from_slice(&bound_args);
        values.extend_from_slice(call_args);
        self.call_value(&function.target, &values, this_value)
    }

    pub(in crate::runtime) fn eval_bound_function_construct(
        &mut self,
        id: BoundFunctionId,
        args: &[Value],
        bound_value: &Value,
        new_target: Value,
    ) -> Result<Value> {
        let function = self.bound_function(id)?.clone();
        let BoundFunctionBehavior::Ordinary {
            this_value: _,
            args: bound_args,
        } = function.behavior
        else {
            return Err(Error::type_error(
                "ShadowRealm wrapped function is not a constructor",
            ));
        };
        let capacity = bound_args
            .len()
            .checked_add(args.len())
            .ok_or_else(|| Error::limit("bound constructor argument count overflowed"))?;
        let mut values = Vec::with_capacity(capacity);
        values.extend_from_slice(&bound_args);
        values.extend_from_slice(args);
        let new_target = if same_value(&new_target, bound_value) {
            function.target.clone()
        } else {
            new_target
        };
        self.semantic_construct(&function.target, &values, new_target)
    }

    pub(in crate::runtime) fn bound_function_target(&self, id: BoundFunctionId) -> Result<Value> {
        self.bound_function(id)
            .map(|function| function.target.clone())
    }

    pub(in crate::runtime) fn bound_function_is_shadow_realm(
        &self,
        id: BoundFunctionId,
    ) -> Result<bool> {
        self.bound_function(id).map(BoundFunction::is_shadow_realm)
    }

    fn create_bound_function(
        &mut self,
        target: Value,
        this_value: Value,
        args: Vec<Value>,
    ) -> Result<Value> {
        let prototype = self.function_constructor_prototype_value()?;
        let reservation = self
            .storage_ledger
            .reserve_count(crate::runtime::VmStorageKind::BoundFunction, 1)?;
        self.bound_functions.reserve_insert()?;
        self.bound_functions.reserve_removals(1)?;
        let id = BoundFunctionId::new(self.bound_functions.next_index());
        reservation.commit()?;
        self.bound_functions.insert_at_next(
            id.index(),
            BoundFunction::ordinary(target, this_value, args),
        )?;
        let result =
            self.create_ephemeral_native_function(NativeFunctionKind::BoundFunction(id), prototype);
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                let removed = self.bound_functions.remove_reserved(id.index())?;
                if removed.is_none() {
                    return Err(Error::runtime("bound function rollback failed"));
                }
                self.storage_ledger
                    .release_count(crate::runtime::VmStorageKind::BoundFunction, 1)?;
                Err(error)
            }
        }
    }

    pub(in crate::runtime) fn create_shadow_realm_wrapper_record(
        &mut self,
        target: Value,
    ) -> Result<Value> {
        let prototype = self.function_constructor_prototype_value()?;
        let reservation = self
            .storage_ledger
            .reserve_count(crate::runtime::VmStorageKind::BoundFunction, 1)?;
        self.bound_functions.reserve_insert()?;
        self.bound_functions.reserve_removals(1)?;
        let id = BoundFunctionId::new(self.bound_functions.next_index());
        reservation.commit()?;
        self.bound_functions
            .insert_at_next(id.index(), BoundFunction::shadow_realm(target))?;
        match self
            .create_ephemeral_native_function(NativeFunctionKind::BoundFunction(id), prototype)
        {
            Ok(value) => Ok(value),
            Err(error) => {
                let removed = self.bound_functions.remove_reserved(id.index())?;
                if removed.is_none() {
                    return Err(Error::runtime("ShadowRealm wrapper rollback failed"));
                }
                self.storage_ledger
                    .release_count(crate::runtime::VmStorageKind::BoundFunction, 1)?;
                Err(error)
            }
        }
    }

    fn bound_function(&self, id: BoundFunctionId) -> Result<&BoundFunction> {
        self.bound_functions
            .get(id.index())
            .ok_or_else(|| Error::runtime("bound function id is not defined"))
    }
}
