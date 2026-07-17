use std::{error::Error as StdError, fmt, future::Future, pin::Pin, task::Context as TaskContext};

use crate::{
    JsBigInt, OwnedValue, RetainedValue,
    api::{
        embedding::Vm,
        host::{HostCall, HostFunction, HostFunctionKind},
    },
    error::{Error, Result},
    runtime::{Context, HostFuturePoll},
};

/// A runtime-agnostic Rust future started by an async host function.
///
/// The future owns everything it needs after the synchronous host-call frame
/// ends. The embedding application chooses the executor and supplies its
/// task context through [`Vm::poll_host_futures`].
pub type HostFuture = Pin<Box<dyn Future<Output = HostTaskResult<OwnedValue>> + 'static>>;

/// Error returned by command-aware asynchronous Rust host work.
///
/// A JavaScript rejection retains its original VM-local value until the
/// surrounding host future settles its JavaScript Promise or the error is
/// dropped. This makes the result safe even if a command request is polled by
/// an embedder-owned executor outside [`Vm::poll_host_futures`].
#[derive(Debug)]
pub enum HostFutureError {
    /// Rust or engine failure converted to an ordinary JavaScript Error.
    Engine(Error),
    /// Original JavaScript rejection kept rooted in its owning VM.
    JavaScript(RetainedValue),
}

impl HostFutureError {
    fn with_context(self, context: impl AsRef<str>) -> Self {
        match self {
            Self::Engine(error) => Self::Engine(error.with_context(context)),
            Self::JavaScript(value) => Self::JavaScript(value),
        }
    }
}

impl fmt::Display for HostFutureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine(error) => error.fmt(formatter),
            Self::JavaScript(_) => formatter.write_str("JavaScript Promise rejected"),
        }
    }
}

impl StdError for HostFutureError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Engine(error) => Some(error),
            Self::JavaScript(_) => None,
        }
    }
}

impl From<Error> for HostFutureError {
    fn from(error: Error) -> Self {
        Self::Engine(error)
    }
}

/// Result type for an async host task that can await JavaScript commands.
pub type HostTaskResult<T> = std::result::Result<T, HostFutureError>;

/// Converts a typed async host-function result into a VM-independent
/// JavaScript primitive.
pub trait IntoOwnedJsValue {
    /// # Errors
    /// Fails when the result cannot be represented as an owned primitive.
    fn into_owned_js_value(self) -> Result<OwnedValue>;
}

impl IntoOwnedJsValue for OwnedValue {
    fn into_owned_js_value(self) -> Result<OwnedValue> {
        Ok(self)
    }
}

impl IntoOwnedJsValue for () {
    fn into_owned_js_value(self) -> Result<OwnedValue> {
        Ok(OwnedValue::Undefined)
    }
}

impl IntoOwnedJsValue for bool {
    fn into_owned_js_value(self) -> Result<OwnedValue> {
        Ok(OwnedValue::Bool(self))
    }
}

impl IntoOwnedJsValue for f64 {
    fn into_owned_js_value(self) -> Result<OwnedValue> {
        Ok(OwnedValue::Number(self))
    }
}

impl IntoOwnedJsValue for JsBigInt {
    fn into_owned_js_value(self) -> Result<OwnedValue> {
        Ok(OwnedValue::BigInt(self))
    }
}

impl IntoOwnedJsValue for String {
    fn into_owned_js_value(self) -> Result<OwnedValue> {
        Ok(OwnedValue::String(self))
    }
}

impl IntoOwnedJsValue for &str {
    fn into_owned_js_value(self) -> Result<OwnedValue> {
        Ok(OwnedValue::String(self.to_owned()))
    }
}

impl HostFunction {
    pub(super) fn new_async<F, Fut>(name: String, callback: F) -> Self
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = Result<OwnedValue>> + 'static,
    {
        Self::with_kind(
            name,
            HostFunctionKind::AsyncCallback {
                callback: std::rc::Rc::new(move |call| {
                    let future = callback(call)?;
                    let future: HostFuture =
                        Box::pin(async move { future.await.map_err(HostFutureError::from) });
                    Ok(future)
                }),
            },
        )
    }

    pub(super) fn new_async_typed<F, Fut, R>(name: String, callback: F) -> Self
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = Result<R>> + 'static,
        R: IntoOwnedJsValue + 'static,
    {
        Self::with_kind(
            name,
            HostFunctionKind::AsyncCallback {
                callback: std::rc::Rc::new(move |call| {
                    let future = callback(call)?;
                    let converted: HostFuture = Box::pin(async move {
                        future
                            .await?
                            .into_owned_js_value()
                            .map_err(HostFutureError::from)
                    });
                    Ok(converted)
                }),
            },
        )
    }

    pub(super) const fn is_async(&self) -> bool {
        matches!(self.kind, HostFunctionKind::AsyncCallback { .. })
    }

    pub(super) fn new_async_task<F, Fut>(name: String, callback: F) -> Self
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = HostTaskResult<OwnedValue>> + 'static,
    {
        Self::with_kind(
            name,
            HostFunctionKind::AsyncCallback {
                callback: std::rc::Rc::new(move |call| {
                    let future: HostFuture = Box::pin(callback(call)?);
                    Ok(future)
                }),
            },
        )
    }

    pub(super) fn new_async_task_typed<F, Fut, R>(name: String, callback: F) -> Self
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = HostTaskResult<R>> + 'static,
        R: IntoOwnedJsValue + 'static,
    {
        Self::with_kind(
            name,
            HostFunctionKind::AsyncCallback {
                callback: std::rc::Rc::new(move |call| {
                    let future = callback(call)?;
                    let converted: HostFuture = Box::pin(async move {
                        future
                            .await?
                            .into_owned_js_value()
                            .map_err(HostFutureError::from)
                    });
                    Ok(converted)
                }),
            },
        )
    }

    pub(super) fn start_async(&self, call: HostCall<'_>) -> Result<HostFuture> {
        let HostFunctionKind::AsyncCallback { callback } = &self.kind else {
            return Err(crate::Error::runtime(
                "synchronous host function was routed as async",
            ));
        };
        let future = callback(call).map_err(|error| error.with_context(self.context_message()))?;
        let context = self.context_message();
        Ok(Box::pin(async move {
            future.await.map_err(|error| error.with_context(context))
        }))
    }
}

impl Context {
    fn create_retained_async_host_function<F, Fut>(
        &mut self,
        name: String,
        callback: F,
    ) -> Result<RetainedValue>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = Result<OwnedValue>> + 'static,
    {
        self.create_retained_host_function_value(HostFunction::new_async(name, callback))
    }

    fn create_retained_async_host_function_typed<F, Fut, R>(
        &mut self,
        name: String,
        callback: F,
    ) -> Result<RetainedValue>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = Result<R>> + 'static,
        R: IntoOwnedJsValue + 'static,
    {
        self.create_retained_host_function_value(HostFunction::new_async_typed(name, callback))
    }

    fn create_retained_async_host_task<F, Fut>(
        &mut self,
        name: String,
        callback: F,
    ) -> Result<RetainedValue>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = HostTaskResult<OwnedValue>> + 'static,
    {
        self.create_retained_host_function_value(HostFunction::new_async_task(name, callback))
    }

    fn create_retained_async_host_task_typed<F, Fut, R>(
        &mut self,
        name: String,
        callback: F,
    ) -> Result<RetainedValue>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = HostTaskResult<R>> + 'static,
        R: IntoOwnedJsValue + 'static,
    {
        self.create_retained_host_function_value(HostFunction::new_async_task_typed(name, callback))
    }

    /// Registers a runtime-agnostic async Rust function as a global binding.
    ///
    /// # Errors
    /// Fails when callable registration fails.
    pub fn register_async_host_function<F, Fut>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<()>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = Result<OwnedValue>> + 'static,
    {
        self.register_host_function_value(HostFunction::new_async(name.into(), callback))
    }

    /// Registers a runtime-agnostic async Rust function with typed result
    /// conversion as a global binding.
    ///
    /// # Errors
    /// Fails when callable registration or result conversion fails.
    pub fn register_async_host_function_typed<F, Fut, R>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<()>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = Result<R>> + 'static,
        R: IntoOwnedJsValue + 'static,
    {
        self.register_host_function_value(HostFunction::new_async_typed(name.into(), callback))
    }

    /// Registers a command-aware async Rust task as a global binding.
    ///
    /// # Errors
    /// Fails when callable registration fails.
    pub fn register_async_host_task<F, Fut>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<()>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = HostTaskResult<OwnedValue>> + 'static,
    {
        self.register_host_function_value(HostFunction::new_async_task(name.into(), callback))
    }

    /// Registers a command-aware async Rust task with typed result conversion.
    ///
    /// # Errors
    /// Fails when callable registration or result conversion fails.
    pub fn register_async_host_task_typed<F, Fut, R>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<()>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = HostTaskResult<R>> + 'static,
        R: IntoOwnedJsValue + 'static,
    {
        self.register_host_function_value(HostFunction::new_async_task_typed(name.into(), callback))
    }
}

impl Vm {
    /// Creates a VM-local async Rust callable without installing a global.
    ///
    /// # Errors
    /// Fails when callable creation or retained-root admission fails.
    pub fn create_async_host_function<F, Fut>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<RetainedValue>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = Result<OwnedValue>> + 'static,
    {
        self.embedding_context_mut()
            .create_retained_async_host_function(name.into(), callback)
    }

    /// Creates a VM-local async Rust callable with typed result conversion.
    ///
    /// # Errors
    /// Fails when callable creation, retained-root admission, or result
    /// conversion fails.
    pub fn create_async_host_function_typed<F, Fut, R>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<RetainedValue>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = Result<R>> + 'static,
        R: IntoOwnedJsValue + 'static,
    {
        self.embedding_context_mut()
            .create_retained_async_host_function_typed(name.into(), callback)
    }

    /// Creates a first-class command-aware async Rust task.
    ///
    /// # Errors
    /// Fails when callable creation or retained-root admission fails.
    pub fn create_async_host_task<F, Fut>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<RetainedValue>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = HostTaskResult<OwnedValue>> + 'static,
    {
        self.embedding_context_mut()
            .create_retained_async_host_task(name.into(), callback)
    }

    /// Creates a first-class command-aware async Rust task with typed result
    /// conversion.
    ///
    /// # Errors
    /// Fails when callable creation, retained-root admission, or result
    /// conversion fails.
    pub fn create_async_host_task_typed<F, Fut, R>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<RetainedValue>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = HostTaskResult<R>> + 'static,
        R: IntoOwnedJsValue + 'static,
    {
        self.embedding_context_mut()
            .create_retained_async_host_task_typed(name.into(), callback)
    }

    /// Registers a runtime-agnostic async Rust function as a global binding.
    ///
    /// # Errors
    /// Fails when callable registration fails.
    pub fn register_async_host_function<F, Fut>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<()>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = Result<OwnedValue>> + 'static,
    {
        self.embedding_context_mut()
            .register_async_host_function(name, callback)
    }

    /// Registers a runtime-agnostic async Rust function with typed result
    /// conversion as a global binding.
    ///
    /// # Errors
    /// Fails when callable registration or result conversion fails.
    pub fn register_async_host_function_typed<F, Fut, R>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<()>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = Result<R>> + 'static,
        R: IntoOwnedJsValue + 'static,
    {
        self.embedding_context_mut()
            .register_async_host_function_typed(name, callback)
    }

    /// Registers a command-aware async Rust task as a global binding.
    ///
    /// # Errors
    /// Fails when callable registration fails.
    pub fn register_async_host_task<F, Fut>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<()>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = HostTaskResult<OwnedValue>> + 'static,
    {
        self.embedding_context_mut()
            .register_async_host_task(name, callback)
    }

    /// Registers a command-aware async Rust task with typed result conversion.
    ///
    /// # Errors
    /// Fails when callable registration or result conversion fails.
    pub fn register_async_host_task_typed<F, Fut, R>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<()>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = HostTaskResult<R>> + 'static,
        R: IntoOwnedJsValue + 'static,
    {
        self.embedding_context_mut()
            .register_async_host_task_typed(name, callback)
    }

    /// Polls every pending async host future once in FIFO creation order.
    ///
    /// Promise reactions produced by completed futures remain in the VM job
    /// queue until [`Self::run_jobs`] is called.
    ///
    /// # Errors
    /// Fails when polling, result admission, Promise settlement, or storage
    /// reconciliation fails.
    pub fn poll_host_futures(
        &mut self,
        task_context: &mut TaskContext<'_>,
    ) -> Result<HostFuturePoll> {
        self.embedding_context_mut().poll_host_futures(task_context)
    }

    /// Returns the number of Rust host futures awaiting completion.
    #[must_use]
    pub const fn pending_host_future_count(&self) -> usize {
        self.embedding_context_ref().pending_host_future_count()
    }

    /// Drops every pending host future and rejects its JavaScript Promise.
    ///
    /// # Errors
    /// Fails when cancellation rejection or storage reconciliation fails.
    pub fn cancel_host_futures(&mut self) -> Result<usize> {
        self.embedding_context_mut().cancel_host_futures()
    }
}
