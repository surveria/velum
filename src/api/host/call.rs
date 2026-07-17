use crate::{
    SharedArrayBufferHandle,
    api::owned_value::OwnedValue,
    error::{Error, Result},
    ownership::VmIdentity,
    runtime::{
        HostAsyncContext, RetainedValue, VmRootSnapshot, object::ObjectHeap,
        retained_values::RetainedValueRegistry,
    },
    value::Value,
};

use super::FromJsValue;

/// A JavaScript value borrowed for the duration of one Rust host callback.
#[derive(Clone, Copy, Debug)]
pub struct LocalValue<'value> {
    identity: &'value VmIdentity,
    objects: &'value ObjectHeap,
    retained_values: &'value RetainedValueRegistry,
    value: &'value Value,
}

impl<'value> LocalValue<'value> {
    const fn new(
        identity: &'value VmIdentity,
        objects: &'value ObjectHeap,
        retained_values: &'value RetainedValueRegistry,
        value: &'value Value,
    ) -> Self {
        Self {
            identity,
            objects,
            retained_values,
            value,
        }
    }

    /// Returns the VM owner of this callback-local value.
    #[must_use]
    pub const fn identity(self) -> &'value VmIdentity {
        self.identity
    }

    /// Borrows the underlying JavaScript value for synchronous inspection.
    #[must_use]
    pub const fn as_value(self) -> &'value Value {
        self.value
    }

    /// Copies this callback-local value into a VM-independent primitive.
    ///
    /// # Errors
    /// Fails for Symbols, objects, and functions, which require a retained
    /// VM-local handle instead of an owned primitive.
    pub fn to_owned_value(self) -> Result<OwnedValue> {
        OwnedValue::try_from(self.value)
    }

    /// Clones an opaque handle to this value's shared backing store.
    ///
    /// # Errors
    /// Fails when the value is not a `SharedArrayBuffer`.
    pub fn to_shared_array_buffer(self) -> Result<SharedArrayBufferHandle> {
        let Value::Object(id) = self.value else {
            return Err(Error::type_error("value is not a SharedArrayBuffer"));
        };
        let Some(buffer) = self.objects.array_buffer(*id)? else {
            return Err(Error::type_error("value is not a SharedArrayBuffer"));
        };
        SharedArrayBufferHandle::from_buffer(&buffer)
    }

    /// Retains this callback-local value beyond the active host call.
    ///
    /// # Errors
    /// Fails when retained-slot allocation fails.
    pub fn retain(self) -> Result<RetainedValue> {
        self.retained_values
            .retain(self.identity, self.value.clone())
    }

    /// Creates a JavaScript throw that remains bound to the value's VM.
    #[must_use]
    pub fn javascript_error(self) -> Error {
        Error::javascript_local(self.identity.clone(), self.value.clone())
    }
}

/// Callback-local metadata and values for one Rust host-function invocation.
#[derive(Clone, Copy, Debug)]
pub struct HostCall<'call> {
    pub(super) function_name: &'call str,
    pub(super) identity: &'call VmIdentity,
    pub(super) objects: &'call ObjectHeap,
    pub(super) retained_values: &'call RetainedValueRegistry,
    pub(super) async_context: Option<&'call HostAsyncContext>,
    pub(super) roots: VmRootSnapshot,
    pub(super) receiver: &'call Value,
    pub(super) args: &'call [Value],
}

impl<'call> HostCall<'call> {
    #[must_use]
    pub const fn function_name(self) -> &'call str {
        self.function_name
    }

    /// Returns the exact JavaScript `this` value supplied to this call.
    ///
    /// The value is callback-local. Copy portable primitives with
    /// [`LocalValue::to_owned_value`] or call [`LocalValue::retain`] before the
    /// callback returns when the receiver must outlive this frame.
    #[must_use]
    pub const fn receiver(self) -> LocalValue<'call> {
        LocalValue::new(
            self.identity,
            self.objects,
            self.retained_values,
            self.receiver,
        )
    }

    /// Returns the direct-root snapshot captured immediately before this host
    /// callback began.
    #[must_use]
    pub const fn root_snapshot(self) -> VmRootSnapshot {
        self.roots
    }

    /// Returns a VM-bound sender for queued JavaScript calls.
    ///
    /// The sender is available only while starting an asynchronous host
    /// function. It can be moved into that function's Rust future and never
    /// borrows or reenters the VM.
    ///
    /// # Errors
    /// Fails when a synchronous host callback requests an async context.
    pub fn async_context(self) -> Result<HostAsyncContext> {
        self.async_context.cloned().ok_or_else(|| {
            Error::runtime("async JavaScript context requires an async host function")
        })
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.args.len()
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.args.is_empty()
    }

    #[must_use]
    pub fn value(self, index: usize) -> Option<LocalValue<'call>> {
        self.args
            .get(index)
            .map(|value| LocalValue::new(self.identity, self.objects, self.retained_values, value))
    }

    /// # Errors
    /// Fails when the argument is missing.
    pub fn required_value(self, index: usize, label: &str) -> Result<LocalValue<'call>> {
        let Some(value) = self.value(index) else {
            return Err(Self::missing_argument(index, label));
        };
        Ok(value)
    }

    /// # Errors
    /// Fails when the argument is missing or is not a JavaScript number.
    pub fn number(self, index: usize, label: &str) -> Result<f64> {
        self.argument(index, label)
    }

    /// # Errors
    /// Fails when the argument is missing or is not a JavaScript string.
    pub fn string(self, index: usize, label: &str) -> Result<&'call str> {
        self.argument(index, label)
    }

    /// # Errors
    /// Fails when the argument is missing or is not a JavaScript boolean.
    pub fn boolean(self, index: usize, label: &str) -> Result<bool> {
        self.argument(index, label)
    }

    /// # Errors
    /// Fails when the argument is missing or cannot be converted into `T`.
    pub fn argument<T>(self, index: usize, label: &str) -> Result<T>
    where
        T: FromJsValue<'call>,
    {
        let value = self.required_value(index, label)?;
        let Some(converted) = T::from_js_value(value.as_value()) else {
            return Err(Self::type_error(
                index,
                label,
                T::EXPECTED_TYPE,
                value.as_value(),
            ));
        };
        Ok(converted)
    }

    fn missing_argument(index: usize, label: &str) -> Error {
        Error::runtime(format!("missing argument '{label}' at index {index}"))
    }

    fn type_error(index: usize, label: &str, expected: &str, actual: &Value) -> Error {
        Error::runtime(format!(
            "argument '{label}' at index {index} expected {expected}, got {}",
            actual.type_name()
        ))
    }
}
