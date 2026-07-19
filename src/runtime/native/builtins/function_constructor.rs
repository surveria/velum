#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::rc::Rc;

use crate::{
    error::{Error, Result},
    runtime::call::RuntimeCallArgs,
    runtime::object::{
        AccessorPropertyUpdate, DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable,
        PropertyEnumerable, PropertyKey, PropertyUpdate, PropertyWritable,
    },
    runtime::{Context, function::NATIVE_FUNCTION_SOURCE_TEXT},
    syntax::FunctionKind,
    value::{ObjectId, Value},
};

use super::{
    FUNCTION_NAME, FUNCTION_PROTOTYPE_APPLY_NAME, FUNCTION_PROTOTYPE_BIND_NAME,
    FUNCTION_PROTOTYPE_CALL_NAME, FUNCTION_PROTOTYPE_TO_STRING_NAME, NativeFunctionKind,
    OBJECT_CONSTRUCTOR_PROPERTY, dynamic_compilation_error,
};

const GENERATED_FUNCTION_NAME: &str = "anonymous";
const SYMBOL_TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const SYMBOL_HAS_INSTANCE_PROPERTY: &str = "hasInstance";
const SYMBOL_HAS_INSTANCE_DISPLAY: &str = "[Symbol.hasInstance]";
const FUNCTION_RESTRICTED_ARGUMENTS_PROPERTY: &str = "arguments";
const FUNCTION_RESTRICTED_CALLER_PROPERTY: &str = "caller";
const FUNCTION_PROTOTYPE_LENGTH_PROPERTY: &str = "length";
const FUNCTION_PROTOTYPE_NAME_PROPERTY: &str = "name";

impl Context {
    pub(in crate::runtime) fn function_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::Function) {
            return Ok(Value::NativeFunction(id));
        }

        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype_id = self.function_prototype_id_with_constructor(constructor.clone())?;
        let prototype = Value::Object(prototype_id);
        let name = self.native_function_name_value(NativeFunctionKind::Function)?;
        self.push_native_function_with_id(id, NativeFunctionKind::Function, prototype, name)?;
        self.insert_global_builtin(FUNCTION_NAME, constructor.clone())?;
        self.install_function_prototype_methods(prototype_id)?;
        Ok(constructor)
    }

    pub(in crate::runtime) fn async_function_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::AsyncFunction) {
            return Ok(Value::NativeFunction(id));
        }

        self.function_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype_id =
            self.async_function_prototype_id_with_constructor(constructor.clone())?;
        let prototype = Value::Object(prototype_id);
        let name = self.native_function_name_value(NativeFunctionKind::AsyncFunction)?;
        self.push_native_function_with_id(id, NativeFunctionKind::AsyncFunction, prototype, name)?;
        self.install_async_function_prototype_properties(prototype_id)?;
        Ok(constructor)
    }

    pub(in crate::runtime) fn async_generator_function_constructor_value(
        &mut self,
    ) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::AsyncGeneratorFunction) {
            return Ok(Value::NativeFunction(id));
        }

        self.async_function_constructor_value()?;
        let prototype_id = self.create_async_generator_function_prototype()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype = Value::Object(prototype_id);
        let name = self.native_function_name_value(NativeFunctionKind::AsyncGeneratorFunction)?;
        self.push_native_function_with_id(
            id,
            NativeFunctionKind::AsyncGeneratorFunction,
            prototype,
            name,
        )?;
        self.install_async_generator_function_constructor(prototype_id, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime::native) fn eval_function_constructor(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_generated_function_constructor(args.as_slice(), FunctionKind::Ordinary)
    }

    pub(in crate::runtime) fn eval_direct_function_constructor(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        self.eval_generated_function_constructor(args, FunctionKind::Ordinary)
    }

    pub(in crate::runtime::native) fn eval_async_function_constructor(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_generated_function_constructor(args.as_slice(), FunctionKind::Async)
    }

    pub(in crate::runtime::native) fn eval_generator_function_constructor(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_generated_function_constructor(args.as_slice(), FunctionKind::Generator)
    }

    pub(in crate::runtime::native) fn eval_async_generator_function_constructor(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_generated_function_constructor(args.as_slice(), FunctionKind::AsyncGenerator)
    }

    pub(crate) fn function_constructor_prototype_value(&mut self) -> Result<Value> {
        let Value::NativeFunction(id) = self.function_constructor_value()? else {
            return Err(Error::runtime("Function constructor value is not native"));
        };
        Ok(self.native_function(id)?.properties().prototype())
    }

    pub(in crate::runtime) fn async_function_constructor_prototype_value(
        &mut self,
    ) -> Result<Value> {
        let Value::NativeFunction(id) = self.async_function_constructor_value()? else {
            return Err(Error::runtime(
                "AsyncFunction constructor value is not native",
            ));
        };
        Ok(self.native_function(id)?.properties().prototype())
    }

    fn eval_generated_function_constructor(
        &mut self,
        args: &[Value],
        kind: FunctionKind,
    ) -> Result<Value> {
        let source = self.function_constructor_source(args, kind)?;
        self.check_string_len(&source.parameters)?;
        self.compile(&source.parameters)
            .map_err(dynamic_compilation_error)?;
        self.check_string_len(&source.compile)?;
        let script = self
            .compile(&source.compile)
            .map_err(dynamic_compilation_error)?;
        let boundary = self.push_eval_activation_boundary()?;
        let result = self.eval_compiled(&script);
        let boundary_result = self.pop_eval_activation_boundary(boundary);
        boundary_result?;
        let value = result?;
        let Value::Function(id) = value.clone() else {
            return Err(Error::runtime(
                "Function constructor did not produce a function",
            ));
        };
        self.set_generated_function_name(id, GENERATED_FUNCTION_NAME)?;
        self.set_function_source(id, Rc::from(source.display.into_boxed_str()))?;
        Ok(value)
    }

    fn function_constructor_source(
        &mut self,
        args: &[Value],
        kind: FunctionKind,
    ) -> Result<GeneratedFunctionSource> {
        let Some((body, params)) = args.split_last() else {
            return Ok(generated_function_source("", "", kind));
        };
        let mut converted_params = Vec::with_capacity(params.len());
        for param in params {
            converted_params.push(self.to_string(param)?);
        }
        let body = self.to_string(body)?;
        let params = converted_params.join(",");
        Ok(generated_function_source(&params, &body, kind))
    }

    fn function_constructor_prototype_id(&mut self) -> Result<ObjectId> {
        let prototype = self.function_constructor_prototype_value()?;
        let Value::Object(id) = prototype else {
            return Err(Error::runtime("Function prototype value is not an object"));
        };
        Ok(id)
    }

    fn function_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<ObjectId> {
        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.create_with_prototype_id(
            None,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        self.objects.mark_function_prototype(prototype)?;
        for (name, value, writable) in [
            (
                FUNCTION_PROTOTYPE_LENGTH_PROPERTY,
                Value::Number(0.0),
                PropertyWritable::No,
            ),
            (
                FUNCTION_PROTOTYPE_NAME_PROPERTY,
                self.heap_string_value("")?,
                PropertyWritable::No,
            ),
            (
                OBJECT_CONSTRUCTOR_PROPERTY,
                constructor,
                PropertyWritable::Yes,
            ),
        ] {
            let key = if name == OBJECT_CONSTRUCTOR_PROPERTY {
                constructor_key
            } else {
                self.intern_property_key(name)?
            };
            self.objects.define_property(
                prototype,
                key,
                name,
                PropertyUpdate::Data(DataPropertyUpdate::new(
                    Some(value),
                    Some(writable),
                    Some(PropertyEnumerable::No),
                    Some(PropertyConfigurable::Yes),
                )),
                self.limits.max_object_properties,
            )?;
        }
        Ok(prototype)
    }

    fn install_function_prototype_methods(&mut self, prototype: ObjectId) -> Result<()> {
        let prototype_value = Value::Object(prototype);
        let call = self.create_ephemeral_native_function(
            NativeFunctionKind::FunctionPrototypeCall,
            prototype_value.clone(),
        )?;
        self.define_non_enumerable_object_property(prototype, FUNCTION_PROTOTYPE_CALL_NAME, call)?;

        let bind = self.create_ephemeral_native_function(
            NativeFunctionKind::FunctionPrototypeBind,
            prototype_value.clone(),
        )?;
        self.define_non_enumerable_object_property(prototype, FUNCTION_PROTOTYPE_BIND_NAME, bind)?;

        let apply = self.create_ephemeral_native_function(
            NativeFunctionKind::FunctionPrototypeApply,
            prototype_value.clone(),
        )?;
        self.define_non_enumerable_object_property(
            prototype,
            FUNCTION_PROTOTYPE_APPLY_NAME,
            apply,
        )?;

        let to_string = self.create_ephemeral_native_function(
            NativeFunctionKind::FunctionPrototypeToString,
            prototype_value.clone(),
        )?;
        self.define_non_enumerable_object_property(
            prototype,
            FUNCTION_PROTOTYPE_TO_STRING_NAME,
            to_string,
        )?;

        self.install_function_prototype_restricted_properties(prototype, &prototype_value)?;
        self.install_function_prototype_has_instance(prototype, prototype_value)
    }

    fn install_function_prototype_restricted_properties(
        &mut self,
        prototype: ObjectId,
        prototype_value: &Value,
    ) -> Result<()> {
        let thrower = self.create_realm_throw_type_error(prototype_value.clone())?;
        for property in [
            FUNCTION_RESTRICTED_ARGUMENTS_PROPERTY,
            FUNCTION_RESTRICTED_CALLER_PROPERTY,
        ] {
            let key = self.intern_property_key(property)?;
            self.objects.define_property(
                prototype,
                key,
                property,
                PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                    Some(thrower.clone()),
                    Some(thrower.clone()),
                    Some(PropertyEnumerable::No),
                    Some(PropertyConfigurable::Yes),
                )),
                self.limits.max_object_properties,
            )?;
        }
        Ok(())
    }

    fn create_realm_throw_type_error(&mut self, prototype: Value) -> Result<Value> {
        if let Some(id) = self.realm.throw_type_error {
            return Ok(Value::NativeFunction(id));
        }
        let reservation = self
            .storage_ledger
            .reserve_count(crate::runtime::VmStorageKind::Association, 1)?;
        let thrower =
            self.create_ephemeral_native_function(NativeFunctionKind::ThrowTypeError, prototype)?;
        let Value::NativeFunction(id) = thrower else {
            return Err(Error::runtime("ThrowTypeError intrinsic is not native"));
        };
        self.native_function_mut(id)?.properties_mut().freeze();
        self.realm.throw_type_error = Some(id);
        reservation.commit()?;
        Ok(Value::NativeFunction(id))
    }

    pub(in crate::runtime) fn realm_throw_type_error(&mut self) -> Result<Value> {
        if let Some(id) = self.realm.throw_type_error {
            return Ok(Value::NativeFunction(id));
        }
        self.function_constructor_value()?;
        self.realm
            .throw_type_error
            .map(Value::NativeFunction)
            .ok_or_else(|| Error::runtime("ThrowTypeError intrinsic is not initialized"))
    }

    pub(in crate::runtime::native) fn eval_function_prototype_to_string(
        &mut self,
        _args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let source = if let Value::Function(id) = this_value {
            self.function_source_text(*id)?
        } else if let Value::NativeFunction(id) = this_value {
            let kind = self.native_function(*id)?.kind();
            crate::runtime::function::native_function_source_text(kind)
        } else if self.semantic_is_callable(this_value)? {
            NATIVE_FUNCTION_SOURCE_TEXT.to_owned()
        } else {
            return Err(Error::type_error(
                "Function.prototype.toString requires a callable receiver",
            ));
        };
        self.heap_string_value(&source)
    }

    fn install_function_prototype_has_instance(
        &mut self,
        prototype: ObjectId,
        prototype_value: Value,
    ) -> Result<()> {
        let has_instance = self.create_ephemeral_native_function(
            NativeFunctionKind::FunctionPrototypeHasInstance,
            prototype_value,
        )?;
        let key = self.well_known_symbol_property_key(SYMBOL_HAS_INSTANCE_PROPERTY)?;
        self.objects.define_property(
            prototype,
            key,
            SYMBOL_HAS_INSTANCE_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(has_instance),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::No),
            )),
            self.limits.max_object_properties,
        )
    }

    fn async_function_prototype_id_with_constructor(
        &mut self,
        constructor: Value,
    ) -> Result<ObjectId> {
        let constructor_key = self.object_constructor_property_key()?;
        let function_prototype = self.function_constructor_prototype_id()?;
        self.objects.create_with_prototype_property(
            Some(function_prototype),
            ObjectPropertyInit::new(
                constructor_key,
                OBJECT_CONSTRUCTOR_PROPERTY,
                constructor,
                PropertyEnumerable::No,
            ),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn install_async_function_prototype_properties(&mut self, prototype: ObjectId) -> Result<()> {
        let to_string_tag = self.async_function_to_string_tag_value()?;
        let key = self.well_known_symbol_property_key(SYMBOL_TO_STRING_TAG_PROPERTY)?;
        self.objects.define_property(
            prototype,
            key,
            SYMBOL_TO_STRING_TAG_PROPERTY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(to_string_tag),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn async_function_to_string_tag_value(&mut self) -> Result<Value> {
        self.heap_string_value(NativeFunctionKind::AsyncFunction.name())
    }

    fn well_known_symbol_property_key(&mut self, property: &str) -> Result<PropertyKey> {
        let constructor = self.symbol_constructor_value()?;
        let value = self.get_named(&constructor, property)?;
        let Value::Symbol(symbol) = value else {
            return Err(Error::runtime("well-known Symbol property is not a symbol"));
        };
        Ok(PropertyKey::symbol(symbol.id()))
    }
}

struct GeneratedFunctionSource {
    parameters: String,
    compile: String,
    display: String,
}

#[derive(Clone, Copy)]
enum GeneratedFunctionSourceUse {
    Compilation,
    Display,
}

fn generated_function_source(
    params: &str,
    body: &str,
    kind: FunctionKind,
) -> GeneratedFunctionSource {
    let display = function_source(
        params,
        body,
        kind,
        Some(GENERATED_FUNCTION_NAME),
        GeneratedFunctionSourceUse::Display,
    );
    let compile = format!(
        "({})",
        function_source(
            params,
            body,
            kind,
            None,
            GeneratedFunctionSourceUse::Compilation,
        )
    );
    let parameters = format!(
        "({})",
        function_source(
            params,
            "",
            kind,
            None,
            GeneratedFunctionSourceUse::Compilation,
        )
    );
    GeneratedFunctionSource {
        parameters,
        compile,
        display,
    }
}

fn function_source(
    params: &str,
    body: &str,
    kind: FunctionKind,
    name: Option<&str>,
    source_use: GeneratedFunctionSourceUse,
) -> String {
    let async_prefix = if kind.is_async() { "async " } else { "" };
    let generator_marker = if kind.is_generator() { "*" } else { "" };
    let name = name.map_or("", |name| name);
    let name_separator = if name.is_empty() { "" } else { " " };
    let parameter_line_terminator = if matches!(source_use, GeneratedFunctionSourceUse::Display)
        || dynamic_parameters_need_line_terminator(params)
    {
        "\n"
    } else {
        ""
    };
    format!(
        "{async_prefix}function{generator_marker}{name_separator}{name}({params}{parameter_line_terminator}) {{\n{body}\n}}"
    )
}

fn dynamic_parameters_need_line_terminator(params: &str) -> bool {
    params.contains("//") || params.contains("<!--") || params.contains("-->")
}
