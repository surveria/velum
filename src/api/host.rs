use std::{fmt, rc::Rc};

use crate::{
    api::owned_value::OwnedValue,
    error::{Error, Result},
    ownership::VmIdentity,
    runtime::RetainedValue,
    runtime::VmRootSnapshot,
    runtime::call::RuntimeCallArgs,
    runtime::retained_values::RetainedValueRegistry,
    runtime::{Context, RealmId},
    syntax::DeclKind,
    value::{HostFunctionId, Value},
};

const EMPTY_HOST_FUNCTION_NAME_ERROR: &str = "host function name must not be empty";
const HOST_FUNCTION_HANDLE_RETURN_ERROR: &str =
    "host functions cannot return VM-owned handles in the skeleton API";

type HostCallback = dyn for<'call> Fn(HostCall<'call>) -> Result<Value>;

/// Engine-owned operations that an embedder can expose under a chosen global
/// function name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostOperation {
    /// Detaches an ordinary `ArrayBuffer` supplied as the first argument.
    DetachArrayBuffer,
    /// Creates a VM-local realm and returns its global object.
    CreateRealm,
}

pub trait IntoJsValue {
    /// # Errors
    /// Fails when conversion cannot produce a JavaScript value.
    fn into_js_value(self) -> Result<Value>;
}

impl IntoJsValue for Value {
    fn into_js_value(self) -> Result<Value> {
        Ok(self)
    }
}

impl IntoJsValue for OwnedValue {
    fn into_js_value(self) -> Result<Value> {
        Ok(self.into())
    }
}

impl IntoJsValue for () {
    fn into_js_value(self) -> Result<Value> {
        Ok(Value::Undefined)
    }
}

impl IntoJsValue for bool {
    fn into_js_value(self) -> Result<Value> {
        Ok(Value::Bool(self))
    }
}

impl IntoJsValue for f64 {
    fn into_js_value(self) -> Result<Value> {
        Ok(Value::Number(self))
    }
}

impl IntoJsValue for String {
    fn into_js_value(self) -> Result<Value> {
        Ok(Value::from(self))
    }
}

impl IntoJsValue for &str {
    fn into_js_value(self) -> Result<Value> {
        Ok(Value::from(self))
    }
}

pub trait FromJsValue<'value>: Sized {
    const EXPECTED_TYPE: &'static str;

    fn from_js_value(value: &'value Value) -> Option<Self>;
}

impl FromJsValue<'_> for bool {
    const EXPECTED_TYPE: &'static str = "boolean";

    fn from_js_value(value: &Value) -> Option<Self> {
        match value {
            Value::Bool(value) => Some(*value),
            _ => None,
        }
    }
}

impl FromJsValue<'_> for f64 {
    const EXPECTED_TYPE: &'static str = "number";

    fn from_js_value(value: &Value) -> Option<Self> {
        match value {
            Value::Number(value) => Some(*value),
            _ => None,
        }
    }
}

impl<'value> FromJsValue<'value> for &'value str {
    const EXPECTED_TYPE: &'static str = "string";

    fn from_js_value(value: &'value Value) -> Option<Self> {
        match value {
            Value::String(value) => value.as_utf8(),
            _ => None,
        }
    }
}

impl FromJsValue<'_> for String {
    const EXPECTED_TYPE: &'static str = "string";

    fn from_js_value(value: &Value) -> Option<Self> {
        match value {
            Value::String(value) => value.as_utf8().map(str::to_owned),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct HostFunction {
    name: String,
    kind: HostFunctionKind,
}

#[derive(Clone)]
enum HostFunctionKind {
    Callback {
        callback: Rc<HostCallback>,
        allow_vm_handles: bool,
    },
    Operation(HostOperation),
    RealmEval(RealmId),
}

impl HostFunction {
    fn new<F>(name: String, callback: F) -> Self
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Value> + 'static,
    {
        Self {
            name,
            kind: HostFunctionKind::Callback {
                callback: Rc::new(callback),
                allow_vm_handles: false,
            },
        }
    }

    const fn operation(name: String, operation: HostOperation) -> Self {
        Self {
            name,
            kind: HostFunctionKind::Operation(operation),
        }
    }

    const fn realm_eval(name: String, realm: RealmId) -> Self {
        Self {
            name,
            kind: HostFunctionKind::RealmEval(realm),
        }
    }

    fn new_internal<F>(name: String, callback: F) -> Self
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Value> + 'static,
    {
        Self {
            name,
            kind: HostFunctionKind::Callback {
                callback: Rc::new(callback),
                allow_vm_handles: true,
            },
        }
    }

    fn new_typed<F, R>(name: String, callback: F) -> Self
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<R> + 'static,
        R: IntoJsValue + 'static,
    {
        Self::new(name, move |call| callback(call)?.into_js_value())
    }

    fn call(
        &self,
        identity: &VmIdentity,
        retained_values: &RetainedValueRegistry,
        roots: VmRootSnapshot,
        args: &[Value],
    ) -> Result<Value> {
        let call = HostCall {
            function_name: self.name.as_str(),
            identity,
            retained_values,
            roots,
            args,
        };
        let HostFunctionKind::Callback { callback, .. } = &self.kind else {
            return Err(Error::runtime("host operation was routed as a callback"));
        };
        callback(call).map_err(|error| error.with_context(self.context_message()))
    }

    fn context_message(&self) -> String {
        format!("host function '{}'", self.name)
    }

    pub(crate) const fn storage_name_bytes(&self) -> usize {
        self.name.len()
    }

    const fn operation_kind(&self) -> Option<HostOperation> {
        match self.kind {
            HostFunctionKind::Operation(operation) => Some(operation),
            HostFunctionKind::Callback { .. } | HostFunctionKind::RealmEval(_) => None,
        }
    }

    fn realm_eval_target(&self) -> Option<RealmId> {
        match &self.kind {
            HostFunctionKind::RealmEval(realm) => Some(realm.clone()),
            HostFunctionKind::Callback { .. } | HostFunctionKind::Operation(_) => None,
        }
    }

    const fn allows_vm_handles(&self) -> bool {
        match self.kind {
            HostFunctionKind::Callback {
                allow_vm_handles, ..
            } => allow_vm_handles,
            HostFunctionKind::Operation(_) | HostFunctionKind::RealmEval(_) => false,
        }
    }
}

impl fmt::Debug for HostFunction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostFunction")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LocalValue<'value> {
    identity: &'value VmIdentity,
    retained_values: &'value RetainedValueRegistry,
    value: &'value Value,
}

impl<'value> LocalValue<'value> {
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

    /// Retains this callback-local value beyond the active host call.
    ///
    /// # Errors
    /// Fails when retained-slot allocation fails.
    pub fn retain(self) -> Result<RetainedValue> {
        self.retained_values
            .retain(self.identity, self.value.clone())
    }

    /// Creates a JavaScript throw that remains bound to the argument's VM.
    #[must_use]
    pub fn javascript_error(self) -> Error {
        Error::javascript_local(self.identity.clone(), self.value.clone())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HostCall<'call> {
    function_name: &'call str,
    identity: &'call VmIdentity,
    retained_values: &'call RetainedValueRegistry,
    roots: VmRootSnapshot,
    args: &'call [Value],
}

impl<'call> HostCall<'call> {
    #[must_use]
    pub const fn function_name(self) -> &'call str {
        self.function_name
    }

    /// Returns the direct-root snapshot captured immediately before this host
    /// callback began.
    #[must_use]
    pub const fn root_snapshot(self) -> VmRootSnapshot {
        self.roots
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
        self.args.get(index).map(|value| LocalValue {
            identity: self.identity,
            retained_values: self.retained_values,
            value,
        })
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

impl Context {
    pub(crate) fn create_internal_host_function<F>(
        &mut self,
        name: String,
        callback: F,
    ) -> Result<Value>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Value> + 'static,
    {
        self.create_internal_host_function_value(HostFunction::new_internal(name, callback))
    }

    fn create_internal_realm_eval_function(
        &mut self,
        name: String,
        realm: RealmId,
    ) -> Result<Value> {
        self.create_internal_host_function_value(HostFunction::realm_eval(name, realm))
    }

    fn create_internal_host_function_value(&mut self, function: HostFunction) -> Result<Value> {
        if function.name.is_empty() {
            return Err(Error::runtime(EMPTY_HOST_FUNCTION_NAME_ERROR));
        }
        self.check_string_len(&function.name)?;
        let projected_count = self
            .host_functions
            .len()
            .checked_add(1)
            .ok_or_else(|| Error::limit("host callback count overflowed"))?;
        let projected_payload_bytes = self
            .host_callback_name_bytes()?
            .checked_add(function.name.len())
            .ok_or_else(|| Error::limit("host callback name bytes overflowed"))?;
        self.ensure_storage_totals(
            crate::runtime::VmStorageKind::HostCallback,
            projected_count,
            projected_payload_bytes,
        )?;
        self.host_functions.reserve_insert()?;
        let id = HostFunctionId::new(self.host_functions.next_index());
        self.host_functions.insert_at_next(id.index(), function)?;
        Ok(Value::HostFunction(id))
    }

    /// # Errors
    /// Fails when the name is empty, exceeds string limits, duplicates an
    /// existing binding, or would exceed the binding limit.
    pub fn register_host_function<F>(&mut self, name: impl Into<String>, callback: F) -> Result<()>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Value> + 'static,
    {
        self.register_host_callback(name.into(), HostFunction::new, callback)
    }

    /// # Errors
    /// Fails when the name is empty, exceeds string limits, duplicates an
    /// existing binding, or would exceed the binding limit.
    pub fn register_host_function_typed<F, R>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<()>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<R> + 'static,
        R: IntoJsValue + 'static,
    {
        self.register_host_callback(name.into(), HostFunction::new_typed, callback)
    }

    /// Registers an engine-owned host operation under an embedder-selected
    /// global function name.
    ///
    /// # Errors
    /// Fails when the name is empty, exceeds string limits, duplicates an
    /// existing binding, or would exceed the binding limit.
    pub fn register_host_operation(
        &mut self,
        name: impl Into<String>,
        operation: HostOperation,
    ) -> Result<()> {
        self.register_host_function_value(HostFunction::operation(name.into(), operation))
    }

    fn register_host_callback<F, C>(
        &mut self,
        name: String,
        create_host_function: C,
        callback: F,
    ) -> Result<()>
    where
        C: FnOnce(String, F) -> HostFunction,
    {
        self.register_host_function_value(create_host_function(name, callback))
    }

    fn register_host_function_value(&mut self, function: HostFunction) -> Result<()> {
        if function.name.is_empty() {
            return Err(Error::runtime(EMPTY_HOST_FUNCTION_NAME_ERROR));
        }
        self.check_string_len(&function.name)?;

        let projected_count = self
            .host_functions
            .len()
            .checked_add(1)
            .ok_or_else(|| Error::limit("host callback count overflowed"))?;
        let projected_payload_bytes = self
            .host_callback_name_bytes()?
            .checked_add(function.name.len())
            .ok_or_else(|| Error::limit("host callback name bytes overflowed"))?;
        self.ensure_storage_totals(
            crate::runtime::VmStorageKind::HostCallback,
            projected_count,
            projected_payload_bytes,
        )?;

        self.host_functions.reserve_insert()?;
        self.host_functions.reserve_removals(1)?;
        let id = HostFunctionId::new(self.host_functions.next_index());
        let binding_name = function.name.clone();
        self.host_functions.insert_at_next(id.index(), function)?;
        let result = self.define(&binding_name, Value::HostFunction(id), DeclKind::Const);
        if let Err(error) = result {
            let removed = self.host_functions.remove_reserved(id.index())?;
            if removed.is_none() {
                return Err(Error::runtime("host function rollback failed"));
            }
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn eval_host_function(
        &mut self,
        id: HostFunctionId,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.to_owned_values();
        let _root_scope =
            self.transient_root_scope(crate::runtime::VmRootKind::TransientCall, values.iter())?;
        let function = self.host_function(id)?.clone();
        if let Some(operation) = function.operation_kind() {
            return self
                .eval_host_operation(operation, &values)
                .map_err(|error| error.with_context(function.context_message()));
        }
        if let Some(realm) = function.realm_eval_target() {
            return self
                .eval_realm_source_value(&realm, values.first().unwrap_or(&Value::Undefined))
                .map_err(|error| error.with_context(function.context_message()));
        }
        let allow_vm_handles = function.allows_vm_handles();
        let roots = self.root_snapshot()?;
        let value = function.call(
            self.identity(),
            self.retained_value_registry(),
            roots,
            &values,
        )?;
        if allow_vm_handles {
            return self
                .checked_value(value)
                .map_err(|error| error.with_context(function.context_message()));
        }
        self.checked_host_return_value(value)
            .map_err(|error| error.with_context(function.context_message()))
    }

    fn eval_host_operation(&mut self, operation: HostOperation, args: &[Value]) -> Result<Value> {
        match operation {
            HostOperation::DetachArrayBuffer => {
                let Some(Value::Object(id)) = args.first() else {
                    return Err(Error::type_error(
                        "ArrayBuffer detachment requires an ArrayBuffer argument",
                    ));
                };
                self.detach_host_array_buffer(*id)?;
                Ok(Value::Undefined)
            }
            HostOperation::CreateRealm => {
                let realm = self.create_realm()?;
                let eval =
                    self.create_internal_realm_eval_function("eval".to_owned(), realm.clone())?;
                self.install_realm_global_eval(&realm, eval)
            }
        }
    }

    fn host_function(&self, id: HostFunctionId) -> Result<&HostFunction> {
        self.host_functions
            .get(id.index())
            .ok_or_else(|| Error::runtime("host function id is not defined"))
    }

    pub(crate) fn validate_host_function_id(&self, id: HostFunctionId) -> Result<()> {
        self.host_function(id).map(|_| ())
    }

    fn checked_host_return_value(&mut self, value: Value) -> Result<Value> {
        match value {
            Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Object(_) => Err(Error::runtime(HOST_FUNCTION_HANDLE_RETURN_ERROR)),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_)
            | Value::Symbol(_) => self.runtime_value(value),
        }
    }
}
