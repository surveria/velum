use std::rc::Rc;

use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::call::RuntimeCallArgs,
    runtime::object::{
        DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable, PropertyEnumerable,
        PropertyKey, PropertyUpdate, PropertyWritable,
    },
    value::{ErrorName, ObjectId, Value},
};

use super::{
    FUNCTION_NAME, FUNCTION_PROTOTYPE_APPLY_NAME, FUNCTION_PROTOTYPE_BIND_NAME,
    FUNCTION_PROTOTYPE_CALL_NAME, FUNCTION_PROTOTYPE_TO_STRING_NAME, NativeFunctionKind,
    OBJECT_CONSTRUCTOR_PROPERTY,
};

const GENERATED_FUNCTION_NAME: &str = "anonymous";
const SYMBOL_TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const SYMBOL_HAS_INSTANCE_PROPERTY: &str = "hasInstance";
const SYMBOL_HAS_INSTANCE_DISPLAY: &str = "[Symbol.hasInstance]";

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

    pub(in crate::runtime::native) fn async_function_constructor_value(&mut self) -> Result<Value> {
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

    pub(in crate::runtime::native) fn eval_function_constructor(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_generated_function_constructor(args.as_slice(), false)
    }

    pub(in crate::runtime) fn eval_direct_function_constructor(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        self.eval_generated_function_constructor(args, false)
    }

    pub(in crate::runtime::native) fn eval_async_function_constructor(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_generated_function_constructor(args.as_slice(), true)
    }

    pub(in crate::runtime) fn function_constructor_prototype_value(&mut self) -> Result<Value> {
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
        is_async: bool,
    ) -> Result<Value> {
        let source = self.function_constructor_source(args, is_async)?;
        self.check_string_len(&source.compile)?;
        let script = self
            .compile(&source.compile)
            .map_err(Self::generated_function_syntax_error)?;
        let caller_locals = std::mem::take(&mut self.locals);
        let caller_upvalue_frames = std::mem::take(&mut self.upvalue_frames);
        let caller_this_values = std::mem::take(&mut self.this_values);
        let result = self.eval_compiled(&script);
        self.locals = caller_locals;
        self.upvalue_frames = caller_upvalue_frames;
        self.this_values = caller_this_values;
        let value = result?;
        let Value::Function(id) = value.clone() else {
            return Err(Error::runtime(
                "Function constructor did not produce a function",
            ));
        };
        self.set_function_source(id, Rc::from(source.display.into_boxed_str()))?;
        Ok(value)
    }

    fn generated_function_syntax_error(error: Error) -> Error {
        match error {
            Error::Lex { .. } | Error::Parse { .. } => {
                Error::exception(ErrorName::SyntaxError, error.to_string())
            }
            Error::Runtime { .. }
            | Error::JavaScript { .. }
            | Error::JavaScriptError { .. }
            | Error::ResourceLimit { .. } => error,
        }
    }

    fn function_constructor_source(
        &mut self,
        args: &[Value],
        is_async: bool,
    ) -> Result<GeneratedFunctionSource> {
        let Some((body, params)) = args.split_last() else {
            return Ok(generated_function_source("", "", is_async));
        };
        let mut converted_params = Vec::with_capacity(params.len());
        for param in params {
            converted_params.push(self.to_string(param)?);
        }
        let body = self.to_string(body)?;
        let params = converted_params.join(",");
        Ok(generated_function_source(&params, &body, is_async))
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
        self.objects.create_with_prototype_property(
            None,
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

        self.install_function_prototype_has_instance(prototype, prototype_value)
    }

    pub(in crate::runtime::native) fn eval_function_prototype_to_string(
        &mut self,
        _args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let source = if let Value::Function(id) = this_value {
            self.function_source_text(*id)?
        } else if self.semantic_is_callable(this_value)? {
            "function()".to_owned()
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
    compile: String,
    display: String,
}

fn generated_function_source(params: &str, body: &str, is_async: bool) -> GeneratedFunctionSource {
    let display = function_display_source(params, body, is_async);
    let compile = format!("({display})");
    GeneratedFunctionSource { compile, display }
}

fn function_display_source(params: &str, body: &str, is_async: bool) -> String {
    let async_prefix = if is_async { "async " } else { "" };
    let parameter_line_terminator = if params.contains("//") { "\n" } else { "" };
    format!(
        "{async_prefix}function {GENERATED_FUNCTION_NAME}({params}{parameter_line_terminator}) {{\n{body}\n}}"
    )
}
