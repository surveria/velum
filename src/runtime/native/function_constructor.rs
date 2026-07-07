use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::call_args::RuntimeCallArgs,
    runtime::object::{ObjectPropertyInit, PropertyEnumerable},
    value::{ObjectId, Value},
};

use super::{FUNCTION_NAME, NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY};

const GENERATED_FUNCTION_NAME: &str = "anonymous";

impl Context {
    pub(super) fn function_constructor_value(&mut self) -> Result<Value> {
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
        Ok(constructor)
    }

    pub(super) fn eval_function_constructor(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let source = Self::function_constructor_source(args.as_slice());
        self.check_string_len(&source)?;
        let script = self.compile(&source)?;
        let caller_locals = std::mem::take(&mut self.locals);
        let caller_upvalue_frames = std::mem::take(&mut self.upvalue_frames);
        let caller_this_values = std::mem::take(&mut self.this_values);
        let result = self.eval_compiled(&script);
        self.locals = caller_locals;
        self.upvalue_frames = caller_upvalue_frames;
        self.this_values = caller_this_values;
        let value = result?;
        let Value::Function(_) = value else {
            return Err(Error::runtime(
                "Function constructor did not produce a function",
            ));
        };
        Ok(value)
    }

    fn function_constructor_source(args: &[Value]) -> String {
        let Some((body, params)) = args.split_last() else {
            return function_expression_source("", "");
        };
        let body = function_constructor_argument_text(body);
        if params.is_empty() {
            return function_expression_source("", &body);
        }
        let params = params
            .iter()
            .map(function_constructor_argument_text)
            .collect::<Vec<_>>()
            .join(",");
        function_expression_source(&params, &body)
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
}

fn function_expression_source(params: &str, body: &str) -> String {
    format!("(function {GENERATED_FUNCTION_NAME}({params}) {{\n{body}\n}})")
}

fn function_constructor_argument_text(value: &Value) -> String {
    value.display_for_concat()
}
