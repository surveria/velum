use serde_json::{Map as JsonMap, Value as JsonValue};

use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::call_args::RuntimeCallArgs,
    runtime::object::{ObjectPropertyInit, PropertyEnumerable},
    value::{ObjectId, Value},
};

use super::{JSON_NAME, JSON_PARSE_NAME, JSON_STRINGIFY_NAME, NativeFunctionKind};

const JSON_ARRAY_CLOSE: &str = "]";
const JSON_ARRAY_OPEN: &str = "[";
const JSON_COLON: &str = ":";
const JSON_COMMA: &str = ",";
const JSON_FALSE: &str = "false";
const JSON_NULL: &str = "null";
const JSON_OBJECT_CLOSE: &str = "}";
const JSON_OBJECT_OPEN: &str = "{";
const JSON_TRUE: &str = "true";
const JSON_UNSUPPORTED_NUMBER: &str = "JSON number cannot be represented as f64";

impl Context {
    pub(super) fn json_object_value(&mut self) -> Result<Value> {
        if let Some(binding) = self.get_binding(JSON_NAME) {
            return Ok(binding.value());
        }

        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_empty_data_object(
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        self.define_json_method(object, JSON_PARSE_NAME, NativeFunctionKind::JsonParse)?;
        self.define_json_method(
            object,
            JSON_STRINGIFY_NAME,
            NativeFunctionKind::JsonStringify,
        )?;

        let value = Value::Object(object);
        self.insert_global_builtin(JSON_NAME, value.clone())?;
        Ok(value)
    }

    pub(super) fn eval_json_parse(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let text = args
            .as_slice()
            .first()
            .map_or_else(|| Value::Undefined.to_string(), ToString::to_string);
        self.check_string_len(&text)?;
        let value = serde_json::from_str(&text)
            .map_err(|error| Error::runtime(format!("JSON.parse failed: {error}")))?;
        self.value_from_json(value)
    }

    pub(super) fn eval_json_stringify(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let Some(value) = args.as_slice().first() else {
            return Ok(Value::Undefined);
        };

        let mut stack = Vec::new();
        let Some(text) = self.stringify_json_value(value, &mut stack)? else {
            return Ok(Value::Undefined);
        };
        self.heap_string_value(&text)
    }

    fn define_json_method(
        &mut self,
        object: ObjectId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let function = self.create_native_function(kind, Value::Undefined)?;
        self.define_non_enumerable_object_property(object, name, function)
    }

    fn value_from_json(&mut self, value: JsonValue) -> Result<Value> {
        match value {
            JsonValue::Null => Ok(Value::Null),
            JsonValue::Bool(value) => Ok(Value::Bool(value)),
            JsonValue::Number(value) => value
                .as_f64()
                .map(Value::Number)
                .ok_or_else(|| Error::runtime(JSON_UNSUPPORTED_NUMBER)),
            JsonValue::String(value) => self.heap_string_value(&value),
            JsonValue::Array(values) => self.array_from_json(values),
            JsonValue::Object(object) => self.object_from_json(object),
        }
    }

    fn array_from_json(&mut self, values: Vec<JsonValue>) -> Result<Value> {
        let mut elements = Vec::with_capacity(values.len());
        for value in values {
            elements.push(self.value_from_json(value)?);
        }
        self.create_array_from_elements(elements)
    }

    fn object_from_json(&mut self, object: JsonMap<String, JsonValue>) -> Result<Value> {
        let mut names = Vec::with_capacity(object.len());
        let mut values = Vec::with_capacity(object.len());
        for (key, value) in object {
            self.check_string_len(&key)?;
            let property = self.intern_property_key(&key)?;
            names.push(key);
            values.push((property, self.value_from_json(value)?));
        }
        let properties = names
            .iter()
            .zip(values)
            .map(|(name, (property, value))| {
                ObjectPropertyInit::new(property, name.as_str(), value, PropertyEnumerable::Yes)
            })
            .collect();
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_data_object(
            properties,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn stringify_json_value(
        &mut self,
        value: &Value,
        stack: &mut Vec<ObjectId>,
    ) -> Result<Option<String>> {
        match value {
            Value::Undefined
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_) => Ok(None),
            Value::Null => Ok(Some(JSON_NULL.to_owned())),
            Value::Bool(value) => Ok(Some(Self::stringify_json_bool(*value))),
            Value::Number(value) => Ok(Some(Self::stringify_json_number(*value))),
            Value::String(value) => self.stringify_json_string(value).map(Some),
            Value::HeapString(value) => self.stringify_json_string(value.as_str()).map(Some),
            Value::Object(id) => self.stringify_json_object(*id, stack).map(Some),
            Value::Error(_) => Ok(Some(self.stringify_empty_json_object()?)),
        }
    }

    fn stringify_json_object(&mut self, id: ObjectId, stack: &mut Vec<ObjectId>) -> Result<String> {
        if let Some(length) = self.objects.array_len_if_array(id)? {
            return self.stringify_json_array(id, length, stack);
        }
        self.stringify_plain_json_object(id, stack)
    }

    fn stringify_json_array(
        &mut self,
        id: ObjectId,
        length: usize,
        stack: &mut Vec<ObjectId>,
    ) -> Result<String> {
        Self::push_json_stack(id, stack)?;
        let mut output = String::from(JSON_ARRAY_OPEN);
        self.check_string_len(&output)?;

        for index in 0..length {
            if index > 0 {
                self.push_json_fragment(&mut output, JSON_COMMA)?;
            }
            let value = self.objects.array_get_index(id, index)?;
            let element = self
                .stringify_json_value(&value, stack)?
                .unwrap_or_else(|| JSON_NULL.to_owned());
            self.push_json_fragment(&mut output, &element)?;
        }

        self.push_json_fragment(&mut output, JSON_ARRAY_CLOSE)?;
        Self::pop_json_stack(id, stack)?;
        Ok(output)
    }

    fn stringify_plain_json_object(
        &mut self,
        id: ObjectId,
        stack: &mut Vec<ObjectId>,
    ) -> Result<String> {
        Self::push_json_stack(id, stack)?;
        let mut output = String::from(JSON_OBJECT_OPEN);
        self.check_string_len(&output)?;
        let mut has_property = false;

        for key in self.objects.own_keys(id, &self.atoms)? {
            let value = self.get_property_value(&Value::Object(id), &key)?;
            let Some(serialized_value) = self.stringify_json_value(&value, stack)? else {
                continue;
            };
            if has_property {
                self.push_json_fragment(&mut output, JSON_COMMA)?;
            }
            let serialized_key = self.stringify_json_string(&key)?;
            self.push_json_fragment(&mut output, &serialized_key)?;
            self.push_json_fragment(&mut output, JSON_COLON)?;
            self.push_json_fragment(&mut output, &serialized_value)?;
            has_property = true;
        }

        self.push_json_fragment(&mut output, JSON_OBJECT_CLOSE)?;
        Self::pop_json_stack(id, stack)?;
        Ok(output)
    }

    fn stringify_empty_json_object(&self) -> Result<String> {
        let text = format!("{JSON_OBJECT_OPEN}{JSON_OBJECT_CLOSE}");
        self.check_string_len(&text)?;
        Ok(text)
    }

    fn stringify_json_string(&self, value: &str) -> Result<String> {
        let text = serde_json::to_string(value)
            .map_err(|error| Error::runtime(format!("JSON.stringify string failed: {error}")))?;
        self.check_string_len(&text)?;
        Ok(text)
    }

    fn push_json_fragment(&self, output: &mut String, fragment: &str) -> Result<()> {
        output.push_str(fragment);
        self.check_string_len(output)
    }

    fn push_json_stack(id: ObjectId, stack: &mut Vec<ObjectId>) -> Result<()> {
        if stack.contains(&id) {
            return Err(Error::runtime(
                "JSON.stringify cannot serialize circular objects",
            ));
        }
        stack.push(id);
        Ok(())
    }

    fn pop_json_stack(id: ObjectId, stack: &mut Vec<ObjectId>) -> Result<()> {
        let removed = stack.pop();
        if removed == Some(id) {
            return Ok(());
        }
        Err(Error::runtime(
            "JSON.stringify object stack became inconsistent",
        ))
    }

    fn stringify_json_bool(value: bool) -> String {
        if value {
            return JSON_TRUE.to_owned();
        }
        JSON_FALSE.to_owned()
    }

    fn stringify_json_number(value: f64) -> String {
        if !value.is_finite() {
            return JSON_NULL.to_owned();
        }
        if value == 0.0 {
            return "0".to_owned();
        }
        Value::Number(value).to_string()
    }
}
