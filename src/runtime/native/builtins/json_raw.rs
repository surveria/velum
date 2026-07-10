use serde_json::Value as JsonValue;

use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::call::RuntimeCallArgs,
    runtime::object::{PropertyConfigurable, PropertyEnumerable, PropertyWritable},
    value::{ErrorName, ObjectId, Value},
};

const JSON_RAW_JSON_PROPERTY: &str = "rawJSON";
const RAW_JSON_EMPTY_ERROR: &str = "JSON.rawJSON text must not be empty";
const RAW_JSON_INVALID_ERROR: &str = "JSON.rawJSON text must be a valid scalar JSON text";
const RAW_JSON_SYMBOL_ERROR: &str = "Cannot convert a Symbol value to a string";

impl Context {
    pub(in crate::runtime::native) fn eval_json_raw_json(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_json_raw_json(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_json_raw_json(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let text = self.raw_json_to_string(args.first())?;
        Self::validate_raw_json_text(&text)?;
        let property_value = self.heap_string_value(&text)?;
        let Value::Object(id) = self
            .objects
            .create_with_exact_prototype(None, self.limits.max_objects)?
        else {
            return Err(Error::runtime(
                "RawJSON allocation did not return an object",
            ));
        };
        self.define_global_object_data_property(
            id,
            JSON_RAW_JSON_PROPERTY,
            property_value,
            PropertyWritable::Yes,
            PropertyEnumerable::Yes,
            PropertyConfigurable::Yes,
        )?;
        self.objects.mark_raw_json(id)?;
        self.objects.freeze(id)?;
        Ok(Value::Object(id))
    }

    pub(in crate::runtime::native) fn eval_json_is_raw_json(
        &self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_json_is_raw_json(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_json_is_raw_json(
        &self,
        args: &[Value],
    ) -> Result<Value> {
        let is_raw_json = if let Some(Value::Object(id)) = args.first() {
            self.objects.is_raw_json(*id)?
        } else {
            false
        };
        Ok(Value::Bool(is_raw_json))
    }

    pub(in crate::runtime::native) fn raw_json_text(
        &mut self,
        id: ObjectId,
    ) -> Result<Option<String>> {
        if !self.objects.is_raw_json(id)? {
            return Ok(None);
        }
        let value = self.get_property_value(&Value::Object(id), JSON_RAW_JSON_PROPERTY)?;
        Self::raw_json_stored_text(&value).map(Some)
    }

    fn raw_json_to_string(&mut self, value: Option<&Value>) -> Result<String> {
        let Some(value) = value else {
            return self.to_string(&Value::Undefined);
        };
        self.to_string(value).map_err(|error| {
            if matches!(value, Value::Symbol(_)) {
                return Error::type_error(RAW_JSON_SYMBOL_ERROR);
            }
            error
        })
    }

    fn raw_json_stored_text(value: &Value) -> Result<String> {
        match value {
            Value::String(value) => Ok(value.clone()),
            Value::HeapString(value) => Ok(value.as_str().to_owned()),
            _ => Err(Error::runtime("RawJSON text property is not a string")),
        }
    }

    fn validate_raw_json_text(text: &str) -> Result<()> {
        if text.is_empty() {
            return Err(Self::raw_json_syntax_error(RAW_JSON_EMPTY_ERROR));
        }
        if text
            .as_bytes()
            .first()
            .zip(text.as_bytes().last())
            .is_none_or(|(first, last)| Self::is_forbidden_raw_json_edge(*first, *last))
        {
            return Err(Self::raw_json_syntax_error(RAW_JSON_INVALID_ERROR));
        }
        match serde_json::from_str::<JsonValue>(text) {
            Ok(JsonValue::Array(_) | JsonValue::Object(_)) | Err(_) => {
                Err(Self::raw_json_syntax_error(RAW_JSON_INVALID_ERROR))
            }
            Ok(
                JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) | JsonValue::String(_),
            ) => Ok(()),
        }
    }

    const fn is_forbidden_raw_json_edge(first: u8, last: u8) -> bool {
        Self::is_json_whitespace(first) || Self::is_json_whitespace(last)
    }

    const fn is_json_whitespace(value: u8) -> bool {
        matches!(value, b'\t' | b'\n' | b'\r' | b' ')
    }

    fn raw_json_syntax_error(message: &str) -> Error {
        Error::exception(ErrorName::SyntaxError, message)
    }
}
