use crate::{
    RetainedValue, Value,
    api::{
        embedding::Vm,
        host::{HostCall, HostFunction, IntoJsValue},
    },
    error::{Error, Result},
    runtime::Context,
};

const RETAINED_HOST_FUNCTION_ROLLBACK_ERROR: &str =
    "retained host function rollback did not find the allocated callback";

impl Vm {
    /// Creates a VM-local JavaScript callable backed by a Rust callback.
    ///
    /// The callable is not installed as a global binding. Its returned
    /// [`RetainedValue`] is an explicit root that can be passed to JavaScript,
    /// stored in ordinary properties, and released or dropped independently.
    ///
    /// # Errors
    /// Fails when the name is empty or exceeds string limits, host callback or
    /// retained-root storage limits are exceeded, or callable initialization
    /// cannot be completed transactionally.
    pub fn create_host_function<F>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<RetainedValue>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Value> + 'static,
    {
        self.embedding_context_mut()
            .create_retained_host_function(name.into(), callback)
    }

    /// Creates a VM-local JavaScript callable with typed Rust return
    /// conversion.
    ///
    /// The callable is not installed as a global binding. Its returned
    /// [`RetainedValue`] follows the same explicit rooting and release model as
    /// [`Self::create_host_function`].
    ///
    /// # Errors
    /// Fails when callable creation fails or the callback result cannot be
    /// converted and admitted to this VM.
    pub fn create_host_function_typed<F, R>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<RetainedValue>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<R> + 'static,
        R: IntoJsValue + 'static,
    {
        self.embedding_context_mut()
            .create_retained_host_function_typed(name.into(), callback)
    }
}

impl Context {
    fn create_retained_host_function<F>(
        &mut self,
        name: String,
        callback: F,
    ) -> Result<RetainedValue>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Value> + 'static,
    {
        let function = HostFunction::new(name, callback);
        self.create_retained_host_function_value(function)
    }

    fn create_retained_host_function_typed<F, R>(
        &mut self,
        name: String,
        callback: F,
    ) -> Result<RetainedValue>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<R> + 'static,
        R: IntoJsValue + 'static,
    {
        let function = HostFunction::new_typed(name, callback);
        self.create_retained_host_function_value(function)
    }

    pub(crate) fn create_retained_host_function_value(
        &mut self,
        function: HostFunction,
    ) -> Result<RetainedValue> {
        let value = self.create_internal_host_function_value(function)?;
        let Value::HostFunction(id) = value else {
            return Err(Error::runtime(
                "host function allocation returned a non-callable value",
            ));
        };
        match self.retain_embedder_value(Value::HostFunction(id)) {
            Ok(handle) => Ok(handle),
            Err(error) => {
                let Some(mut function) = self.host_functions.remove_reserved(id.index())? else {
                    return Err(Error::runtime(RETAINED_HOST_FUNCTION_ROLLBACK_ERROR));
                };
                function.release_property_storage()?;
                Err(error)
            }
        }
    }
}
