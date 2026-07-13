use crate::{
    compiled_script::CompiledScript,
    error::{Error, Result},
    runtime::{
        Context, VmStorageKind,
        abstract_operations::is_primitive,
        call::RuntimeCallArgs,
        control::{Completion, runtime_exception_value},
        native::{
            DEFAULT_GLOBAL_BINDING_NAMES, NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY,
            SHADOW_REALM_NAME, ShadowRealmFunctionKind,
        },
        object::{
            DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable, PropertyEnumerable,
            PropertyKey, PropertyWritable,
        },
        property::DynamicPropertyKey,
        realm::RealmIndex,
    },
    value::{NativeFunctionId, ObjectId, Value},
};

use super::dynamic_compilation_error;

const EVALUATE_NAME: &str = "evaluate";
const IMPORT_VALUE_NAME: &str = "importValue";
const LENGTH_NAME: &str = "length";
const NAME_NAME: &str = "name";
const TO_STRING_TAG_DISPLAY: &str = "[Symbol.toStringTag]";
const TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const RECEIVER_ERROR: &str = "method requires a ShadowRealm receiver";
const SOURCE_ERROR: &str = "ShadowRealm source text must be a string";
const EXPORT_NAME_ERROR: &str = "ShadowRealm export name must be a string";
const TRANSFER_ERROR: &str = "ShadowRealm cannot transfer a non-callable object";
const ABRUPT_ERROR: &str = "ShadowRealm execution failed";

impl Context {
    pub(in crate::runtime) fn shadow_realm_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.realm.shadow_realm_constructor {
            return Ok(Value::NativeFunction(id));
        }
        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.create_with_prototype_property(
            None,
            ObjectPropertyInit::new(
                constructor_key,
                OBJECT_CONSTRUCTOR_PROPERTY,
                constructor.clone(),
                PropertyEnumerable::No,
            ),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let kind = NativeFunctionKind::ShadowRealm(ShadowRealmFunctionKind::Constructor);
        let name = self.native_function_name_value(kind)?;
        self.push_native_function_with_id(id, kind, Value::Object(prototype), name)?;
        self.install_shadow_realm_prototype(prototype)?;
        self.storage_ledger
            .grow_count(VmStorageKind::Association, 2)?;
        self.realm.shadow_realm_constructor = Some(id);
        self.realm.shadow_realm_prototype = Some(prototype);
        self.insert_global_builtin(SHADOW_REALM_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime) fn construct_shadow_realm(&mut self) -> Result<Value> {
        self.shadow_realm_constructor_value()?;
        let prototype = self
            .realm
            .shadow_realm_prototype
            .ok_or_else(|| Error::runtime("ShadowRealm prototype is not initialized"))?;
        let constructor_key = self.object_constructor_property_key()?;
        let value = self.objects.create_with_prototype(
            Some(prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object) = value else {
            return Err(Error::runtime("ShadowRealm allocation failed"));
        };
        let realm = self.create_realm_index()?;
        self.with_realm(realm, Self::initialize_shadow_realm_global_bindings)?;
        self.objects.bind_shadow_realm(object, realm)?;
        Ok(Value::Object(object))
    }

    fn initialize_shadow_realm_global_bindings(&mut self) -> Result<()> {
        let global = self.global_object_id()?;
        for name in DEFAULT_GLOBAL_BINDING_NAMES {
            if self.builtin_value(name)?.is_none() {
                continue;
            }
            let lookup = self.property_lookup(name);
            self.global_object_property_descriptor(global, lookup)?;
        }
        Ok(())
    }

    pub(in crate::runtime) fn eval_shadow_realm(
        &mut self,
        kind: ShadowRealmFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        match kind {
            ShadowRealmFunctionKind::Constructor => {
                Err(Error::type_error("ShadowRealm constructor requires new"))
            }
            ShadowRealmFunctionKind::Evaluate => self.eval_shadow_realm_evaluate(args, this_value),
            ShadowRealmFunctionKind::ImportValue => {
                self.eval_shadow_realm_import_value(args, this_value)
            }
        }
    }

    fn install_shadow_realm_prototype(&mut self, prototype: ObjectId) -> Result<()> {
        for (name, kind) in [
            (EVALUATE_NAME, ShadowRealmFunctionKind::Evaluate),
            (IMPORT_VALUE_NAME, ShadowRealmFunctionKind::ImportValue),
        ] {
            let method = self.create_ephemeral_native_function(
                NativeFunctionKind::ShadowRealm(kind),
                Value::Undefined,
            )?;
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        let symbol_constructor = self.symbol_constructor_value()?;
        let tag_symbol = self.get_named(&symbol_constructor, TO_STRING_TAG_PROPERTY)?;
        let Value::Symbol(tag_symbol) = tag_symbol else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let tag = self.heap_string_value(SHADOW_REALM_NAME)?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(tag_symbol.id()),
            TO_STRING_TAG_DISPLAY,
            crate::runtime::object::PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(tag),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn eval_shadow_realm_evaluate(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let realm = self.shadow_realm_receiver(this_value)?;
        let source = args
            .as_slice()
            .first()
            .unwrap_or(&Value::Undefined)
            .string_text()
            .map(str::to_owned)
            .ok_or_else(|| Error::type_error(SOURCE_ERROR))?;
        let script = CompiledScript::compile_eval(
            &source,
            self.limits.clone(),
            crate::compiled_script::EvalCompileContext::new(
                false,
                crate::compiled_script::EvalSuperContext::None,
                crate::compiled_script::EvalClassFieldContext::None,
                std::rc::Rc::from([]),
            ),
        )
        .map_err(dynamic_compilation_error)?;
        let result = self.with_realm(realm, |context| {
            let boundary = context.push_eval_activation_boundary()?;
            let result = context.eval_compiled_eval_completion(&script, script.strict());
            let boundary_result = context.pop_eval_activation_boundary(boundary);
            boundary_result?;
            result.and_then(Completion::into_result)
        });
        let value = Self::shadow_boundary_result(result)?;
        self.shadow_wrapped_value(self.active_realm_index(), value)
    }

    fn shadow_realm_receiver(&self, this_value: &Value) -> Result<RealmIndex> {
        let Value::Object(object) = this_value else {
            return Err(Error::type_error(RECEIVER_ERROR));
        };
        self.objects
            .shadow_realm(*object)?
            .ok_or_else(|| Error::type_error(RECEIVER_ERROR))
    }

    fn eval_shadow_realm_import_value(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let realm = self.shadow_realm_receiver(this_value)?;
        let specifier = self.to_string(args.as_slice().first().unwrap_or(&Value::Undefined))?;
        let export_name = args
            .as_slice()
            .get(1)
            .unwrap_or(&Value::Undefined)
            .string_text()
            .map(str::to_owned)
            .ok_or_else(|| Error::type_error(EXPORT_NAME_ERROR))?;
        let (promise, promise_object) = self.create_pending_promise()?;
        let imported = self.with_realm(realm, |context| {
            context.load_dynamic_module_export(&specifier, &export_name)
        });
        let imported =
            imported.and_then(|value| self.shadow_wrapped_value(self.active_realm_index(), value));
        match imported {
            Ok(value) => self.fulfill_promise(promise, value)?,
            Err(error) => {
                let reason = self.shadow_import_rejection_value(&error)?;
                self.reject_promise(promise, reason)?;
            }
        }
        Ok(promise_object)
    }

    fn shadow_import_rejection_value(&mut self, error: &Error) -> Result<Value> {
        let request = Error::type_error(format!("{ABRUPT_ERROR}: {error}"));
        runtime_exception_value(self, &request)?.ok_or_else(|| {
            Error::runtime("ShadowRealm import rejection did not produce an exception value")
        })
    }

    pub(in crate::runtime) fn eval_shadow_realm_wrapped_call(
        &mut self,
        target: &Value,
        args: &[Value],
    ) -> Result<Value> {
        let target_realm = self.callable_realm_index(target)?;
        let mut wrapped_args = Vec::with_capacity(args.len());
        for argument in args {
            wrapped_args.push(self.shadow_wrapped_value(target_realm, argument.clone())?);
        }
        let result = self.call_value(target, &wrapped_args, Value::Undefined);
        let value = Self::shadow_boundary_result(result)?;
        self.shadow_wrapped_value(self.active_realm_index(), value)
    }

    fn shadow_wrapped_value(&mut self, destination: RealmIndex, value: Value) -> Result<Value> {
        if is_primitive(&value) {
            return Ok(value);
        }
        if !self.semantic_is_callable(&value)? {
            return Err(Error::type_error(TRANSFER_ERROR));
        }
        if destination == self.active_realm_index() {
            return self.create_shadow_realm_wrapper(value);
        }
        self.with_realm(destination, |context| {
            context.create_shadow_realm_wrapper(value)
        })
    }

    fn create_shadow_realm_wrapper(&mut self, target: Value) -> Result<Value> {
        let metadata = Self::shadow_boundary_result(self.shadow_wrapper_metadata(&target))?;
        let wrapper = self.create_shadow_realm_wrapper_record(target)?;
        let Value::NativeFunction(id) = wrapper else {
            return Err(Error::runtime("ShadowRealm wrapper is not native"));
        };
        self.define_shadow_wrapper_metadata(id, metadata.0, metadata.1)?;
        Ok(Value::NativeFunction(id))
    }

    fn shadow_wrapper_metadata(&mut self, target: &Value) -> Result<(Value, Value)> {
        let length_property =
            DynamicPropertyKey::new(LENGTH_NAME.to_owned(), self.known_property_key(LENGTH_NAME));
        let length = if self
            .semantic_own_property_descriptor(target, &length_property)?
            .is_some()
        {
            match self.get_named(target, LENGTH_NAME)? {
                Value::Number(value) if value == f64::INFINITY => Value::Number(value),
                Value::Number(value) => {
                    Value::Number(self.to_integer_or_infinity(&Value::Number(value))?.max(0.0))
                }
                _ => Value::Number(0.0),
            }
        } else {
            Value::Number(0.0)
        };
        let name = match self.get_named(target, NAME_NAME)? {
            value @ Value::String(_) => value,
            _ => self.heap_string_value("")?,
        };
        Ok((length, name))
    }

    fn define_shadow_wrapper_metadata(
        &mut self,
        id: NativeFunctionId,
        length: Value,
        name: Value,
    ) -> Result<()> {
        for (property, value) in [(LENGTH_NAME, length), (NAME_NAME, name)] {
            let key = self.intern_property_key(property)?;
            self.define_native_function_property_key(
                id,
                property,
                key,
                DataPropertyUpdate::new(
                    Some(value),
                    Some(PropertyWritable::No),
                    Some(PropertyEnumerable::No),
                    Some(PropertyConfigurable::Yes),
                ),
            )?;
        }
        Ok(())
    }

    fn shadow_boundary_result<T>(result: Result<T>) -> Result<T> {
        result.map_err(|error| match error {
            Error::Lex { .. }
            | Error::Parse { .. }
            | Error::JavaScript { .. }
            | Error::JavaScriptError { .. } => {
                Error::type_error(format!("{ABRUPT_ERROR}: {error}"))
            }
            Error::Runtime { .. } | Error::ResourceLimit { .. } => error,
        })
    }
}
