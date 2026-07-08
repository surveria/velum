use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, native::NativeFunctionKind},
    value::{BoundFunctionId, Value},
};

const CALL_TARGET_NOT_CALLABLE_ERROR: &str = "Function.prototype.call target is not callable";
const BIND_TARGET_NOT_CALLABLE_ERROR: &str = "Function.prototype.bind target is not callable";

#[derive(Debug, Clone)]
pub(in crate::runtime) struct BoundFunction {
    target: Value,
    this_value: Value,
    args: Vec<Value>,
}

impl BoundFunction {
    const fn new(target: Value, this_value: Value, args: Vec<Value>) -> Self {
        Self {
            target,
            this_value,
            args,
        }
    }
}

impl Context {
    pub(in crate::runtime) fn eval_function_prototype_call(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if !Self::is_callable(this_value) {
            return Err(Error::type_error(CALL_TARGET_NOT_CALLABLE_ERROR));
        }
        let (call_this, call_args): (Value, &[Value]) =
            if let Some((this_arg, call_args)) = args.as_slice().split_first() {
                (this_arg.clone(), call_args)
            } else {
                (Value::Undefined, &[])
            };
        self.eval_call_value(this_value.clone(), call_args, call_this)
    }

    pub(in crate::runtime) fn eval_function_prototype_bind(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if !Self::is_callable(this_value) {
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
        let call_args = args.as_slice();
        let capacity = function
            .args
            .len()
            .checked_add(call_args.len())
            .ok_or_else(|| Error::limit("bound function argument count overflowed"))?;
        let mut values = Vec::with_capacity(capacity);
        values.extend_from_slice(&function.args);
        values.extend_from_slice(call_args);
        self.eval_call_value(function.target, &values, function.this_value)
    }

    fn create_bound_function(
        &mut self,
        target: Value,
        this_value: Value,
        args: Vec<Value>,
    ) -> Result<Value> {
        let id = BoundFunctionId::new(self.bound_functions.len());
        self.bound_functions
            .push(BoundFunction::new(target, this_value, args));
        let prototype = self.function_constructor_prototype_value()?;
        let result =
            self.create_ephemeral_native_function(NativeFunctionKind::BoundFunction(id), prototype);
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                let removed = self.bound_functions.pop();
                if removed.is_none() {
                    return Err(Error::runtime("bound function rollback failed"));
                }
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
