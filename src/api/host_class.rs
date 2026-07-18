use alloc::rc::Rc;
use core::{any::Any, fmt, future::Future};

use crate::{
    RetainedValue,
    api::{
        embedding::Vm,
        host::{
            HostCall, HostFunction, HostFuture, HostFutureError, IntoJsValue, IntoOwnedJsValue,
        },
    },
    error::{Error, Result},
    runtime::{
        Context, EmbeddingObjectPrototype,
        object::{
            AccessorPropertyUpdate, DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable,
            PropertyUpdate, PropertyWritable,
        },
    },
    syntax::DeclKind,
    value::Value,
};

mod staging;

use staging::StagedHostClass;

type ConstructorCallback<T> = dyn for<'call> Fn(HostCall<'call>) -> Result<HostInstance<T>>;
type MethodCallback<T> = dyn for<'call> Fn(&'call T, HostCall<'call>) -> Result<HostMethodResult>;
type AsyncMethodCallback<T> = dyn for<'call> Fn(&'call T, HostCall<'call>) -> Result<HostFuture>;
type StaticMethodCallback = dyn for<'call> Fn(HostCall<'call>) -> Result<HostMethodResult>;
type StaticAsyncMethodCallback = dyn for<'call> Fn(HostCall<'call>) -> Result<HostFuture>;

/// Payload and accounting policy produced by one Rust-backed class constructor.
pub struct HostInstance<T> {
    payload: T,
    logical_payload_bytes: usize,
    traced_values: Vec<RetainedValue>,
}

impl<T> HostInstance<T> {
    /// Creates one typed instance payload with its logical VM memory charge.
    #[must_use]
    pub const fn new(payload: T, logical_payload_bytes: usize) -> Self {
        Self {
            payload,
            logical_payload_bytes,
            traced_values: Vec::new(),
        }
    }

    /// Adds VM-local values that the payload retains as traced internal edges.
    ///
    /// The supplied retained roots are transferred into the new wrapper's GC
    /// graph during construction and released after the edges are installed.
    #[must_use]
    pub fn with_traced_values(mut self, values: Vec<RetainedValue>) -> Self {
        self.traced_values = values;
        self
    }

    fn erase(self) -> ErasedHostInstance
    where
        T: 'static,
    {
        ErasedHostInstance {
            payload: Box::new(self.payload),
            logical_payload_bytes: self.logical_payload_bytes,
            traced_values: self.traced_values,
        }
    }
}

impl<T> fmt::Debug for HostInstance<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostInstance")
            .field("payload_type", &core::any::type_name::<T>())
            .field("logical_payload_bytes", &self.logical_payload_bytes)
            .field("traced_value_count", &self.traced_values.len())
            .finish_non_exhaustive()
    }
}

/// Result policy for a synchronous Rust-backed class method.
#[derive(Debug)]
pub enum HostMethodResult {
    /// Returns a portable JavaScript value after the ordinary host conversion
    /// and admission checks.
    Value(Value),
    /// Returns a VM-local value through an identity-checked retained handle.
    Retained(RetainedValue),
    /// Creates a distinct ordinary wrapper sharing the receiver's typed
    /// payload and current prototype.
    SharedReceiver,
}

impl HostMethodResult {
    /// Converts a portable Rust result into a method result.
    ///
    /// # Errors
    /// Fails when conversion cannot produce a JavaScript value.
    pub fn value(value: impl IntoJsValue) -> Result<Self> {
        value.into_js_value().map(Self::Value)
    }

    /// Returns a VM-local retained value after owner validation.
    #[must_use]
    pub const fn retained(value: RetainedValue) -> Self {
        Self::Retained(value)
    }

    /// Requests a new wrapper that explicitly shares the receiver payload.
    #[must_use]
    pub const fn shared_receiver() -> Self {
        Self::SharedReceiver
    }
}

pub struct ErasedHostInstance {
    pub(crate) payload: Box<dyn Any>,
    pub(crate) logical_payload_bytes: usize,
    pub(crate) traced_values: Vec<RetainedValue>,
}

enum PrototypePropertyKind<T> {
    Method {
        length: u16,
        callback: Rc<MethodCallback<T>>,
    },
    AsyncMethod {
        length: u16,
        callback: Rc<AsyncMethodCallback<T>>,
    },
    Accessor {
        getter: Option<Rc<MethodCallback<T>>>,
        setter: Option<Rc<MethodCallback<T>>>,
    },
}

struct PrototypeProperty<T> {
    name: String,
    kind: PrototypePropertyKind<T>,
}

enum StaticPropertyKind {
    Method {
        length: u16,
        callback: Rc<StaticMethodCallback>,
    },
    AsyncMethod {
        length: u16,
        callback: Rc<StaticAsyncMethodCallback>,
    },
}

struct StaticProperty {
    name: String,
    kind: StaticPropertyKind,
}

/// Builder for one JavaScript class backed by a typed Rust payload.
///
/// Instances remain ordinary JavaScript objects. This builder only defines
/// their constructor, prototype descriptors, and opaque payload policy.
pub struct HostClass<T> {
    name: String,
    constructor_length: u16,
    constructor: Rc<ConstructorCallback<T>>,
    prototype_properties: Vec<PrototypeProperty<T>>,
    static_properties: Vec<StaticProperty>,
    configuration_error: Option<String>,
}

impl<T: 'static> HostClass<T> {
    /// Creates a class builder with a synchronous typed payload factory.
    pub fn new<F>(name: impl Into<String>, constructor: F) -> Self
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<HostInstance<T>> + 'static,
    {
        Self {
            name: name.into(),
            constructor_length: 0,
            constructor: Rc::new(constructor),
            prototype_properties: Vec::new(),
            static_properties: Vec::new(),
            configuration_error: None,
        }
    }

    /// Sets the observable JavaScript constructor `length`.
    #[must_use]
    pub const fn with_constructor_length(mut self, length: u16) -> Self {
        self.constructor_length = length;
        self
    }

    /// Adds a synchronous prototype method with `length === 0`.
    #[must_use]
    pub fn method<F, R>(self, name: impl Into<String>, callback: F) -> Self
    where
        F: for<'call> Fn(&'call T, HostCall<'call>) -> Result<R> + 'static,
        R: IntoJsValue + 'static,
    {
        self.method_with_length(name, 0, callback)
    }

    /// Adds a synchronous prototype method with an explicit JavaScript length.
    #[must_use]
    pub fn method_with_length<F, R>(
        mut self,
        name: impl Into<String>,
        length: u16,
        callback: F,
    ) -> Self
    where
        F: for<'call> Fn(&'call T, HostCall<'call>) -> Result<R> + 'static,
        R: IntoJsValue + 'static,
    {
        let callback = Rc::new(move |payload: &T, call: HostCall<'_>| {
            callback(payload, call)?
                .into_js_value()
                .map(HostMethodResult::Value)
        });
        self.push_prototype_property(
            name.into(),
            PrototypePropertyKind::Method { length, callback },
        );
        self
    }

    /// Adds a method that may return a retained value or shared wrapper.
    #[must_use]
    pub fn method_with_result<F>(
        mut self,
        name: impl Into<String>,
        length: u16,
        callback: F,
    ) -> Self
    where
        F: for<'call> Fn(&'call T, HostCall<'call>) -> Result<HostMethodResult> + 'static,
    {
        self.push_prototype_property(
            name.into(),
            PrototypePropertyKind::Method {
                length,
                callback: Rc::new(callback),
            },
        );
        self
    }

    /// Adds an asynchronous prototype method.
    ///
    /// The callback may inspect the payload only while creating its owned
    /// future. The future and its output must be `'static` and portable.
    #[must_use]
    pub fn async_method<F, Fut, R>(
        mut self,
        name: impl Into<String>,
        length: u16,
        callback: F,
    ) -> Self
    where
        F: for<'call> Fn(&'call T, HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = Result<R>> + 'static,
        R: IntoOwnedJsValue + 'static,
    {
        let callback = Rc::new(move |payload: &T, call: HostCall<'_>| {
            let future = callback(payload, call)?;
            let future: HostFuture = Box::pin(async move {
                let value = future.await.map_err(HostFutureError::from)?;
                value.into_owned_js_value().map_err(HostFutureError::from)
            });
            Ok(future)
        });
        self.push_prototype_property(
            name.into(),
            PrototypePropertyKind::AsyncMethod { length, callback },
        );
        self
    }

    /// Adds a synchronous prototype getter.
    #[must_use]
    pub fn getter<F, R>(mut self, name: impl Into<String>, callback: F) -> Self
    where
        F: for<'call> Fn(&'call T, HostCall<'call>) -> Result<R> + 'static,
        R: IntoJsValue + 'static,
    {
        let callback = Rc::new(move |payload: &T, call: HostCall<'_>| {
            callback(payload, call)?
                .into_js_value()
                .map(HostMethodResult::Value)
        });
        self.add_getter(name.into(), callback);
        self
    }

    /// Adds a synchronous prototype setter.
    #[must_use]
    pub fn setter<F, R>(mut self, name: impl Into<String>, callback: F) -> Self
    where
        F: for<'call> Fn(&'call T, HostCall<'call>) -> Result<R> + 'static,
        R: IntoJsValue + 'static,
    {
        let callback = Rc::new(move |payload: &T, call: HostCall<'_>| {
            callback(payload, call)?
                .into_js_value()
                .map(HostMethodResult::Value)
        });
        self.add_setter(name.into(), callback);
        self
    }

    /// Adds a synchronous static method.
    #[must_use]
    pub fn static_method<F, R>(mut self, name: impl Into<String>, length: u16, callback: F) -> Self
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<R> + 'static,
        R: IntoJsValue + 'static,
    {
        let callback = Rc::new(move |call: HostCall<'_>| {
            callback(call)?.into_js_value().map(HostMethodResult::Value)
        });
        self.push_static_property(name.into(), StaticPropertyKind::Method { length, callback });
        self
    }

    /// Adds an asynchronous static method.
    #[must_use]
    pub fn static_async_method<F, Fut, R>(
        mut self,
        name: impl Into<String>,
        length: u16,
        callback: F,
    ) -> Self
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Fut> + 'static,
        Fut: Future<Output = Result<R>> + 'static,
        R: IntoOwnedJsValue + 'static,
    {
        let callback = Rc::new(move |call: HostCall<'_>| {
            let future = callback(call)?;
            let future: HostFuture = Box::pin(async move {
                let value = future.await.map_err(HostFutureError::from)?;
                value.into_owned_js_value().map_err(HostFutureError::from)
            });
            Ok(future)
        });
        self.push_static_property(
            name.into(),
            StaticPropertyKind::AsyncMethod { length, callback },
        );
        self
    }

    fn push_prototype_property(&mut self, name: String, kind: PrototypePropertyKind<T>) {
        if self
            .prototype_properties
            .iter()
            .any(|property| property.name == name)
        {
            self.record_configuration_error(format!(
                "duplicate host class prototype property '{name}'"
            ));
            return;
        }
        self.prototype_properties
            .push(PrototypeProperty { name, kind });
    }

    fn add_getter(&mut self, name: String, callback: Rc<MethodCallback<T>>) {
        let Some(index) = self
            .prototype_properties
            .iter()
            .position(|property| property.name == name)
        else {
            self.prototype_properties.push(PrototypeProperty {
                name,
                kind: PrototypePropertyKind::Accessor {
                    getter: Some(callback),
                    setter: None,
                },
            });
            return;
        };
        let Some(property) = self.prototype_properties.get_mut(index) else {
            self.record_configuration_error(
                "host class getter property index became invalid".to_owned(),
            );
            return;
        };
        let error = match &mut property.kind {
            PrototypePropertyKind::Accessor { getter, .. } if getter.is_none() => {
                *getter = Some(callback);
                None
            }
            PrototypePropertyKind::Accessor { .. } => {
                Some(format!("duplicate host class getter '{}'", property.name))
            }
            PrototypePropertyKind::Method { .. } | PrototypePropertyKind::AsyncMethod { .. } => {
                Some(format!(
                    "host class property '{}' mixes an accessor and method",
                    property.name
                ))
            }
        };
        if let Some(error) = error {
            self.record_configuration_error(error);
        }
    }

    fn add_setter(&mut self, name: String, callback: Rc<MethodCallback<T>>) {
        let Some(index) = self
            .prototype_properties
            .iter()
            .position(|property| property.name == name)
        else {
            self.prototype_properties.push(PrototypeProperty {
                name,
                kind: PrototypePropertyKind::Accessor {
                    getter: None,
                    setter: Some(callback),
                },
            });
            return;
        };
        let Some(property) = self.prototype_properties.get_mut(index) else {
            self.record_configuration_error(
                "host class setter property index became invalid".to_owned(),
            );
            return;
        };
        let error = match &mut property.kind {
            PrototypePropertyKind::Accessor { setter, .. } if setter.is_none() => {
                *setter = Some(callback);
                None
            }
            PrototypePropertyKind::Accessor { .. } => {
                Some(format!("duplicate host class setter '{}'", property.name))
            }
            PrototypePropertyKind::Method { .. } | PrototypePropertyKind::AsyncMethod { .. } => {
                Some(format!(
                    "host class property '{}' mixes an accessor and method",
                    property.name
                ))
            }
        };
        if let Some(error) = error {
            self.record_configuration_error(error);
        }
    }

    fn push_static_property(&mut self, name: String, kind: StaticPropertyKind) {
        if self
            .static_properties
            .iter()
            .any(|property| property.name == name)
        {
            self.record_configuration_error(format!(
                "duplicate host class static property '{name}'"
            ));
            return;
        }
        self.static_properties.push(StaticProperty { name, kind });
    }

    fn record_configuration_error(&mut self, message: String) {
        if self.configuration_error.is_none() {
            self.configuration_error = Some(message);
        }
    }
}

impl<T> fmt::Debug for HostClass<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostClass")
            .field("name", &self.name)
            .field("constructor_length", &self.constructor_length)
            .field("prototype_property_count", &self.prototype_properties.len())
            .field("static_property_count", &self.static_properties.len())
            .field("configuration_error", &self.configuration_error)
            .finish_non_exhaustive()
    }
}

impl Vm {
    /// Registers a constructable JavaScript class backed by typed Rust state.
    ///
    /// The class name becomes a VM-local global lexical binding. Instances use
    /// ordinary JavaScript object, prototype, descriptor, Proxy, and GC paths.
    ///
    /// # Errors
    /// Fails for invalid or duplicate member definitions, a conflicting global
    /// binding, callback, object, property, payload, root, or storage limits,
    /// or VM storage failures.
    pub fn register_host_class<T: 'static>(&mut self, class: HostClass<T>) -> Result<()> {
        self.embedding_context_mut().register_host_class(class)
    }
}

impl Context {
    fn register_host_class<T: 'static>(&mut self, class: HostClass<T>) -> Result<()> {
        let HostClass {
            name,
            constructor_length,
            constructor,
            prototype_properties,
            static_properties,
            configuration_error,
        } = class;
        if let Some(message) = configuration_error {
            return Err(Error::runtime(message));
        }
        Self::validate_host_class_property_names(&prototype_properties, &static_properties)?;
        self.prepare_embedding_binding(&name)?;

        let function_count =
            Self::host_class_function_count(&prototype_properties, &static_properties)?;
        let (functions, function_ids) = StagedHostClass::allocate_function_storage(function_count)?;

        let prototype = self.create_embedding_object(EmbeddingObjectPrototype::Default)?;
        let prototype_value = self.resolve_retained_value(&prototype)?;
        let Value::Object(prototype_id) = prototype_value else {
            return Err(Error::runtime(
                "host class prototype allocation returned a non-object",
            ));
        };
        let mut staged = StagedHostClass::new(prototype, prototype_id, functions, function_ids);
        let constructor_name = name.clone();
        let constructor_function = HostFunction::new_constructor(
            constructor_name,
            constructor_length,
            Value::Object(prototype_id),
            move |call| constructor(call).map(HostInstance::erase),
        );
        let result = (|| {
            let constructor_value = staged.stage_function(self, constructor_function)?;
            self.define_host_class_data_property(
                &Value::Object(prototype_id),
                "constructor",
                constructor_value.clone(),
            )?;
            self.install_host_class_prototype_properties(
                &mut staged,
                &Value::Object(prototype_id),
                prototype_properties,
            )?;
            self.install_host_class_static_properties(
                &mut staged,
                &constructor_value,
                static_properties,
            )?;
            self.define(&name, constructor_value, DeclKind::Const)
        })();
        if let Err(error) = result {
            if let Err(rollback_error) = staged.rollback(self) {
                return Err(rollback_error.with_context(format!(
                    "host class rollback after registration failure: {error}"
                )));
            }
            return Err(error);
        }
        drop(staged);
        Ok(())
    }

    fn host_class_function_count<T>(
        prototype_properties: &[PrototypeProperty<T>],
        static_properties: &[StaticProperty],
    ) -> Result<usize> {
        let prototype_count =
            prototype_properties
                .iter()
                .try_fold(0_usize, |count, property| {
                    let additional = match &property.kind {
                        PrototypePropertyKind::Method { .. }
                        | PrototypePropertyKind::AsyncMethod { .. } => 1,
                        PrototypePropertyKind::Accessor { getter, setter } => {
                            usize::from(getter.is_some())
                                .checked_add(usize::from(setter.is_some()))
                                .ok_or_else(|| {
                                    Error::limit("host class accessor function count overflowed")
                                })?
                        }
                    };
                    count
                        .checked_add(additional)
                        .ok_or_else(|| Error::limit("host class function count overflowed"))
                })?;
        prototype_count
            .checked_add(static_properties.len())
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| Error::limit("host class function count overflowed"))
    }

    fn validate_host_class_property_names<T>(
        prototype_properties: &[PrototypeProperty<T>],
        static_properties: &[StaticProperty],
    ) -> Result<()> {
        if let Some(property) = prototype_properties
            .iter()
            .find(|property| property.name.is_empty())
        {
            return Err(Error::runtime(format!(
                "host class prototype property name must not be empty: {:?}",
                property.name
            )));
        }
        if let Some(property) = static_properties
            .iter()
            .find(|property| property.name.is_empty())
        {
            return Err(Error::runtime(format!(
                "host class static property name must not be empty: {:?}",
                property.name
            )));
        }
        Ok(())
    }

    fn install_host_class_prototype_properties<T: 'static>(
        &mut self,
        staged: &mut StagedHostClass,
        prototype: &Value,
        properties: Vec<PrototypeProperty<T>>,
    ) -> Result<()> {
        for property in properties {
            match property.kind {
                PrototypePropertyKind::Method { length, callback } => {
                    let function = self.create_host_class_method::<T>(
                        staged,
                        property.name.as_str(),
                        length,
                        callback,
                    )?;
                    self.define_host_class_data_property(
                        prototype,
                        property.name.as_str(),
                        function,
                    )?;
                }
                PrototypePropertyKind::AsyncMethod { length, callback } => {
                    let function = self.create_host_class_async_method::<T>(
                        staged,
                        property.name.as_str(),
                        length,
                        callback,
                    )?;
                    self.define_host_class_data_property(
                        prototype,
                        property.name.as_str(),
                        function,
                    )?;
                }
                PrototypePropertyKind::Accessor { getter, setter } => {
                    let getter = getter
                        .map(|callback| {
                            self.create_host_class_method::<T>(
                                staged,
                                format!("get {}", property.name).as_str(),
                                0,
                                callback,
                            )
                        })
                        .transpose()?;
                    let setter = setter
                        .map(|callback| {
                            self.create_host_class_method::<T>(
                                staged,
                                format!("set {}", property.name).as_str(),
                                1,
                                callback,
                            )
                        })
                        .transpose()?;
                    self.define_host_class_accessor_property(
                        prototype,
                        property.name.as_str(),
                        getter,
                        setter,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn install_host_class_static_properties(
        &mut self,
        staged: &mut StagedHostClass,
        constructor: &Value,
        properties: Vec<StaticProperty>,
    ) -> Result<()> {
        for property in properties {
            let function = match property.kind {
                StaticPropertyKind::Method { length, callback } => {
                    let name = property.name.clone();
                    let function =
                        HostFunction::new_method(name, length, move |call| callback(call));
                    staged.stage_function(self, function)?
                }
                StaticPropertyKind::AsyncMethod { length, callback } => {
                    let name = property.name.clone();
                    let function =
                        HostFunction::new_async_task_with_length(name, length, move |call| {
                            callback(call)
                        });
                    staged.stage_function(self, function)?
                }
            };
            self.define_host_class_data_property(constructor, property.name.as_str(), function)?;
        }
        Ok(())
    }

    fn create_host_class_method<T: 'static>(
        &mut self,
        staged: &mut StagedHostClass,
        name: &str,
        length: u16,
        callback: Rc<MethodCallback<T>>,
    ) -> Result<Value> {
        let function = HostFunction::new_method(name.to_owned(), length, move |call| {
            let receiver = call.receiver();
            let payload = receiver.host_payload::<T>()?;
            callback(payload, call)
        });
        staged.stage_function(self, function)
    }

    fn create_host_class_async_method<T: 'static>(
        &mut self,
        staged: &mut StagedHostClass,
        name: &str,
        length: u16,
        callback: Rc<AsyncMethodCallback<T>>,
    ) -> Result<Value> {
        let function =
            HostFunction::new_async_task_with_length(name.to_owned(), length, move |call| {
                let receiver = call.receiver();
                let payload = receiver.host_payload::<T>()?;
                callback(payload, call)
            });
        staged.stage_function(self, function)
    }

    fn define_host_class_data_property(
        &mut self,
        target: &Value,
        name: &str,
        value: Value,
    ) -> Result<()> {
        let defined = self.embedding_define_property(
            target,
            &Value::from(name),
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            true,
        )?;
        if !defined {
            return Err(Error::runtime(format!(
                "host class data property '{name}' was not defined"
            )));
        }
        Ok(())
    }

    fn define_host_class_accessor_property(
        &mut self,
        target: &Value,
        name: &str,
        getter: Option<Value>,
        setter: Option<Value>,
    ) -> Result<()> {
        let defined = self.embedding_define_property(
            target,
            &Value::from(name),
            PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                getter,
                setter,
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            true,
        )?;
        if !defined {
            return Err(Error::runtime(format!(
                "host class accessor property '{name}' was not defined"
            )));
        }
        Ok(())
    }
}
