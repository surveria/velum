use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::call::RuntimeCallArgs,
    runtime::object::{PropertyConfigurable, PropertyEnumerable, PropertyWritable},
    value::{ErrorName, ObjectId, Value},
};

use super::json_parse::parse_json_text;

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
        self.validate_raw_json_text(&text)?;
        let property_value = self.heap_utf16_string_value(&text)?;
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
        let value = self.get_named(&Value::Object(id), JSON_RAW_JSON_PROPERTY)?;
        Self::raw_json_stored_text(&value).map(Some)
    }

    fn raw_json_to_string(&mut self, value: Option<&Value>) -> Result<Vec<u16>> {
        let Some(value) = value else {
            return self.to_utf16_string(&Value::Undefined);
        };
        self.to_utf16_string(value).map_err(|error| {
            if matches!(value, Value::Symbol(_)) {
                return Error::type_error(RAW_JSON_SYMBOL_ERROR);
            }
            error
        })
    }

    fn raw_json_stored_text(value: &Value) -> Result<String> {
        value
            .string_text()
            .map(str::to_owned)
            .ok_or_else(|| Error::runtime("RawJSON text property is not a string"))
    }

    fn validate_raw_json_text(&self, text: &[u16]) -> Result<()> {
        if text.is_empty() {
            return Err(Self::raw_json_syntax_error(RAW_JSON_EMPTY_ERROR));
        }
        if text
            .first()
            .zip(text.last())
            .is_none_or(|(first, last)| Self::is_forbidden_raw_json_edge(*first, *last))
        {
            return Err(Self::raw_json_syntax_error(RAW_JSON_INVALID_ERROR));
        }
        let value = parse_json_text(text, self.limits.max_expression_depth)
            .map_err(|_| Self::raw_json_syntax_error(RAW_JSON_INVALID_ERROR))?;
        if value.is_scalar() {
            return Ok(());
        }
        Err(Self::raw_json_syntax_error(RAW_JSON_INVALID_ERROR))
    }

    const fn is_forbidden_raw_json_edge(first: u16, last: u16) -> bool {
        Self::is_json_whitespace(first) || Self::is_json_whitespace(last)
    }

    const fn is_json_whitespace(value: u16) -> bool {
        matches!(value, 0x0009 | 0x000a | 0x000d | 0x0020)
    }

    fn raw_json_syntax_error(message: &str) -> Error {
        Error::exception(ErrorName::SyntaxError, message)
    }
}
