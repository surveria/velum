use serde_json::{Map as JsonMap, Value as JsonValue};

use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::call::RuntimeCallArgs,
    runtime::object::{ObjectPrimitiveValue, ObjectPropertyInit, PropertyEnumerable},
    value::{ErrorName, ObjectId, Value, format_ecmascript_number},
};

use super::{
    JSON_IS_RAW_JSON_NAME, JSON_NAME, JSON_PARSE_NAME, JSON_RAW_JSON_NAME, JSON_STRINGIFY_NAME,
    NativeFunctionKind,
};

const JSON_ARRAY_CLOSE: &str = "]";
const JSON_ARRAY_OPEN: &str = "[";
const JSON_COLON: &str = ":";
const JSON_COMMA: &str = ",";
const JSON_EMPTY_KEY: &str = "";
const JSON_FALSE: &str = "false";
const JSON_INDENT_LIMIT: usize = 10;
const JSON_NEWLINE: &str = "\n";
const JSON_NULL: &str = "null";
const JSON_OBJECT_CLOSE: &str = "}";
const JSON_OBJECT_OPEN: &str = "{";
const JSON_SPACE: &str = " ";
const JSON_TO_JSON_NAME: &str = "toJSON";
const JSON_TRUE: &str = "true";
const JSON_UNSUPPORTED_NUMBER: &str = "JSON number cannot be represented as f64";

#[derive(Debug, Clone)]
enum JsonReplacer {
    None,
    Function(Value),
    PropertyList(Vec<String>),
}

#[derive(Debug)]
struct JsonStringifyState {
    replacer: JsonReplacer,
    gap: String,
    indent: String,
    stack: Vec<ObjectId>,
}

impl JsonStringifyState {
    const fn new(replacer: JsonReplacer, gap: String) -> Self {
        Self {
            replacer,
            gap,
            indent: String::new(),
            stack: Vec::new(),
        }
    }

    const fn is_compact(&self) -> bool {
        self.gap.is_empty()
    }
}

#[derive(Debug)]
struct JsonObjectMember {
    key: String,
    value: String,
}

impl JsonObjectMember {
    const fn new(key: String, value: String) -> Self {
        Self { key, value }
    }
}

impl Context {
    pub(in crate::runtime::native) fn json_object_value(&mut self) -> Result<Value> {
        if let Some(binding) = self.get_binding(JSON_NAME) {
            return binding.value(JSON_NAME);
        }

        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_empty_data_object(
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        self.define_json_method(
            object,
            JSON_IS_RAW_JSON_NAME,
            NativeFunctionKind::JsonIsRawJson,
        )?;
        self.define_json_method(object, JSON_PARSE_NAME, NativeFunctionKind::JsonParse)?;
        self.define_json_method(object, JSON_RAW_JSON_NAME, NativeFunctionKind::JsonRawJson)?;
        self.define_json_method(
            object,
            JSON_STRINGIFY_NAME,
            NativeFunctionKind::JsonStringify,
        )?;
        self.define_json_to_string_tag(object)?;

        let value = Value::Object(object);
        self.insert_global_builtin(JSON_NAME, value.clone())?;
        Ok(value)
    }

    pub(in crate::runtime::native) fn eval_json_parse(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_json_parse(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_json_parse(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let text = self.json_parse_text(args.first())?;
        self.check_string_len(&text)?;
        let value = serde_json::from_str(&text)
            .map_err(|error| Error::exception(ErrorName::SyntaxError, error.to_string()))?;
        let value = self.value_from_json(value)?;
        let Some(reviver) = args.get(1) else {
            return Ok(value);
        };
        if !self.semantic_is_callable(reviver)? {
            return Ok(value);
        }
        let holder = self.create_json_wrapper(value)?;
        self.internalize_json_property(&holder, JSON_EMPTY_KEY, reviver)
    }

    pub(in crate::runtime::native) fn eval_json_stringify(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_json_stringify(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_json_stringify(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let Some(value) = args.first() else {
            return Ok(Value::Undefined);
        };

        let replacer = self.json_replacer(args.get(1))?;
        let gap = self.json_gap(args.get(2))?;
        let holder = self.create_json_wrapper(value.clone())?;
        let mut state = JsonStringifyState::new(replacer, gap);
        let Some(text) = self.stringify_json_property(&holder, JSON_EMPTY_KEY, &mut state)? else {
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

    fn create_json_wrapper(&mut self, value: Value) -> Result<Value> {
        let key = self.intern_property_key(JSON_EMPTY_KEY)?;
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_data_object(
            vec![ObjectPropertyInit::new(
                key,
                JSON_EMPTY_KEY,
                value,
                PropertyEnumerable::Yes,
            )],
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn internalize_json_property(
        &mut self,
        holder: &Value,
        key: &str,
        reviver: &Value,
    ) -> Result<Value> {
        let value = self.get_named(holder, key)?;
        if let Value::Object(id) = value.clone() {
            self.internalize_json_children(&value, id, reviver)?;
        }
        let key_value = self.heap_string_value(key)?;
        let args = [key_value, value];
        self.call_json_callback(reviver, holder.clone(), &args)
    }

    fn internalize_json_children(
        &mut self,
        holder: &Value,
        id: ObjectId,
        reviver: &Value,
    ) -> Result<()> {
        if let Some(length) = self.objects.array_len_if_array(id)? {
            for index in 0..length {
                self.internalize_json_child(holder, &index.to_string(), reviver)?;
            }
            return Ok(());
        }
        for key in self.semantic_own_enumerable_string_keys(holder)? {
            self.internalize_json_child(holder, &key, reviver)?;
        }
        Ok(())
    }

    fn internalize_json_child(&mut self, holder: &Value, key: &str, reviver: &Value) -> Result<()> {
        let value = self.internalize_json_property(holder, key, reviver)?;
        if matches!(value, Value::Undefined) {
            return self.delete_json_property(holder, key);
        }
        self.set_json_property(holder, key, value)
    }

    fn delete_json_property(&mut self, holder: &Value, key: &str) -> Result<()> {
        let lookup = self.property_lookup(key);
        let deleted = self.delete_property_value_with_lookup(holder, lookup)?;
        if deleted {
            return Ok(());
        }
        Err(Error::type_error("JSON reviver could not delete property"))
    }

    fn set_json_property(&mut self, holder: &Value, key: &str, value: Value) -> Result<()> {
        let property = self.intern_property_key(key)?;
        self.set_property_value_with_accessors(holder, property, key, value)
    }

    fn json_replacer(&mut self, value: Option<&Value>) -> Result<JsonReplacer> {
        let Some(value) = value else {
            return Ok(JsonReplacer::None);
        };
        if self.semantic_is_callable(value)? {
            return Ok(JsonReplacer::Function(value.clone()));
        }
        let Value::Object(id) = value else {
            return Ok(JsonReplacer::None);
        };
        if self.objects.array_len_if_array(*id)?.is_none() {
            return Ok(JsonReplacer::None);
        }
        self.json_replacer_property_list(*id)
            .map(JsonReplacer::PropertyList)
    }

    fn json_replacer_property_list(&mut self, id: ObjectId) -> Result<Vec<String>> {
        let length = self.objects.array_len(id)?;
        let mut keys = Vec::new();
        for index in 0..length {
            let value = self.objects.array_get_index(id, index)?;
            let Some(key) = self.json_replacer_property_name(&value)? else {
                continue;
            };
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        Ok(keys)
    }

    fn json_replacer_property_name(&mut self, value: &Value) -> Result<Option<String>> {
        match value {
            Value::String(value) => Ok(Some(value.clone())),
            Value::HeapString(value) => Ok(Some(value.as_str().to_owned())),
            Value::Number(value) => Ok(Some(Self::json_number_property_name(*value))),
            Value::Object(id) if self.objects.string_object_value(*id)?.is_some() => {
                self.json_object_to_string(value).map(Some)
            }
            Value::Object(id)
                if matches!(
                    self.objects.primitive_value(*id)?,
                    Some(ObjectPrimitiveValue::Number(_))
                ) =>
            {
                self.json_object_to_string(value).map(Some)
            }
            Value::Object(_)
            | Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Symbol(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Error(_) => Ok(None),
        }
    }

    fn json_gap(&mut self, value: Option<&Value>) -> Result<String> {
        let Some(value) = value else {
            return Ok(String::new());
        };
        let gap = match value {
            Value::Number(value) => JSON_SPACE.repeat(Self::json_gap_space_count(*value)),
            Value::String(value) => Self::json_gap_string(value),
            Value::HeapString(value) => Self::json_gap_string(value.as_str()),
            Value::Object(id) if self.objects.string_object_value(*id)?.is_some() => {
                Self::json_gap_string(&self.json_object_to_string(value)?)
            }
            Value::Object(id)
                if matches!(
                    self.objects.primitive_value(*id)?,
                    Some(ObjectPrimitiveValue::Number(_))
                ) =>
            {
                JSON_SPACE.repeat(Self::json_gap_space_count(
                    self.json_object_to_number(value)?,
                ))
            }
            Value::Object(_)
            | Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Symbol(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Error(_) => String::new(),
        };
        self.check_string_len(&gap)?;
        Ok(gap)
    }

    fn json_gap_space_count(value: f64) -> usize {
        if !value.is_finite() || value <= 0.0 {
            return 0;
        }
        let mut count = 0usize;
        let mut remaining = value.floor();
        while count < JSON_INDENT_LIMIT && remaining >= 1.0 {
            count = count.saturating_add(1);
            remaining -= 1.0;
        }
        count
    }

    fn json_gap_string(value: &str) -> String {
        value.chars().take(JSON_INDENT_LIMIT).collect()
    }

    fn stringify_json_property(
        &mut self,
        holder: &Value,
        key: &str,
        state: &mut JsonStringifyState,
    ) -> Result<Option<String>> {
        let mut value = self.get_named(holder, key)?;
        value = self.apply_json_to_json(value, key)?;
        value = self.apply_json_replacer(holder, key, value, &state.replacer)?;
        self.stringify_json_value(&value, state)
    }

    fn apply_json_to_json(&mut self, value: Value, key: &str) -> Result<Value> {
        if !matches!(value, Value::Object(_)) {
            return Ok(value);
        }
        let to_json = self.get_named(&value, JSON_TO_JSON_NAME)?;
        if !self.semantic_is_callable(&to_json)? {
            return Ok(value);
        }
        let key = self.heap_string_value(key)?;
        self.call_json_callback(&to_json, value, &[key])
    }

    fn apply_json_replacer(
        &mut self,
        holder: &Value,
        key: &str,
        value: Value,
        replacer: &JsonReplacer,
    ) -> Result<Value> {
        let JsonReplacer::Function(function) = replacer else {
            return Ok(value);
        };
        let key = self.heap_string_value(key)?;
        let args = [key, value];
        self.call_json_callback(function, holder.clone(), &args)
    }

    fn stringify_json_value(
        &mut self,
        value: &Value,
        state: &mut JsonStringifyState,
    ) -> Result<Option<String>> {
        let value = self.json_boxed_stringify_value(value)?;
        match &value {
            Value::Undefined
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Symbol(_) => Ok(None),
            Value::Null => Ok(Some(JSON_NULL.to_owned())),
            Value::Bool(value) => Ok(Some(Self::stringify_json_bool(*value))),
            Value::Number(value) => Ok(Some(Self::stringify_json_number(*value))),
            Value::String(value) => self.stringify_json_string(value).map(Some),
            Value::HeapString(value) => self.stringify_json_string(value.as_str()).map(Some),
            Value::Object(id) => {
                if let Some(text) = self.raw_json_text(*id)? {
                    return Ok(Some(text));
                }
                self.stringify_json_object(*id, state).map(Some)
            }
            Value::Error(_) => Ok(Some(self.stringify_empty_json_object()?)),
        }
    }

    fn stringify_json_object(
        &mut self,
        id: ObjectId,
        state: &mut JsonStringifyState,
    ) -> Result<String> {
        Self::push_json_stack(id, &mut state.stack)?;
        let result = if let Some(length) = self.objects.array_len_if_array(id)? {
            self.stringify_json_array(id, length, state)
        } else {
            self.stringify_plain_json_object(id, state)
        };
        let pop_result = Self::pop_json_stack(id, &mut state.stack);
        match (result, pop_result) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }

    fn stringify_json_array(
        &mut self,
        id: ObjectId,
        length: usize,
        state: &mut JsonStringifyState,
    ) -> Result<String> {
        let stepback = state.indent.clone();
        state.indent.push_str(&state.gap);
        let holder = Value::Object(id);
        let mut elements = Vec::with_capacity(length);
        for index in 0..length {
            let element = self
                .stringify_json_property(&holder, &index.to_string(), state)?
                .unwrap_or_else(|| JSON_NULL.to_owned());
            elements.push(element);
        }
        let result = self.format_json_array(&elements, state, &stepback);
        state.indent = stepback;
        result
    }

    fn stringify_plain_json_object(
        &mut self,
        id: ObjectId,
        state: &mut JsonStringifyState,
    ) -> Result<String> {
        let stepback = state.indent.clone();
        state.indent.push_str(&state.gap);
        let holder = Value::Object(id);
        let keys = match &state.replacer {
            JsonReplacer::PropertyList(keys) => keys.clone(),
            JsonReplacer::None | JsonReplacer::Function(_) => {
                self.semantic_own_enumerable_string_keys(&holder)?
            }
        };
        let mut members = Vec::new();
        for key in keys {
            let Some(value) = self.stringify_json_property(&holder, &key, state)? else {
                continue;
            };
            let key = self.stringify_json_string(&key)?;
            members.push(JsonObjectMember::new(key, value));
        }
        let result = self.format_json_object(&members, state, &stepback);
        state.indent = stepback;
        result
    }

    fn format_json_array(
        &self,
        elements: &[String],
        state: &JsonStringifyState,
        stepback: &str,
    ) -> Result<String> {
        let mut output = String::from(JSON_ARRAY_OPEN);
        self.check_string_len(&output)?;
        if elements.is_empty() {
            self.push_json_fragment(&mut output, JSON_ARRAY_CLOSE)?;
            return Ok(output);
        }
        if state.is_compact() {
            self.push_compact_json_list(&mut output, elements)?;
        } else {
            self.push_pretty_json_list(&mut output, elements, &state.indent, stepback)?;
        }
        self.push_json_fragment(&mut output, JSON_ARRAY_CLOSE)?;
        Ok(output)
    }

    fn format_json_object(
        &self,
        members: &[JsonObjectMember],
        state: &JsonStringifyState,
        stepback: &str,
    ) -> Result<String> {
        let mut output = String::from(JSON_OBJECT_OPEN);
        self.check_string_len(&output)?;
        if members.is_empty() {
            self.push_json_fragment(&mut output, JSON_OBJECT_CLOSE)?;
            return Ok(output);
        }
        if state.is_compact() {
            self.push_compact_json_members(&mut output, members)?;
        } else {
            self.push_pretty_json_members(&mut output, members, &state.indent, stepback)?;
        }
        self.push_json_fragment(&mut output, JSON_OBJECT_CLOSE)?;
        Ok(output)
    }

    fn push_compact_json_list(&self, output: &mut String, elements: &[String]) -> Result<()> {
        for (index, element) in elements.iter().enumerate() {
            if index > 0 {
                self.push_json_fragment(output, JSON_COMMA)?;
            }
            self.push_json_fragment(output, element)?;
        }
        Ok(())
    }

    fn push_pretty_json_list(
        &self,
        output: &mut String,
        elements: &[String],
        indent: &str,
        stepback: &str,
    ) -> Result<()> {
        self.push_json_fragment(output, JSON_NEWLINE)?;
        for (index, element) in elements.iter().enumerate() {
            if index > 0 {
                self.push_json_fragment(output, JSON_COMMA)?;
                self.push_json_fragment(output, JSON_NEWLINE)?;
            }
            self.push_json_fragment(output, indent)?;
            self.push_json_fragment(output, element)?;
        }
        self.push_json_fragment(output, JSON_NEWLINE)?;
        self.push_json_fragment(output, stepback)
    }

    fn push_compact_json_members(
        &self,
        output: &mut String,
        members: &[JsonObjectMember],
    ) -> Result<()> {
        for (index, member) in members.iter().enumerate() {
            if index > 0 {
                self.push_json_fragment(output, JSON_COMMA)?;
            }
            self.push_json_member(output, member, false)?;
        }
        Ok(())
    }

    fn push_pretty_json_members(
        &self,
        output: &mut String,
        members: &[JsonObjectMember],
        indent: &str,
        stepback: &str,
    ) -> Result<()> {
        self.push_json_fragment(output, JSON_NEWLINE)?;
        for (index, member) in members.iter().enumerate() {
            if index > 0 {
                self.push_json_fragment(output, JSON_COMMA)?;
                self.push_json_fragment(output, JSON_NEWLINE)?;
            }
            self.push_json_fragment(output, indent)?;
            self.push_json_member(output, member, true)?;
        }
        self.push_json_fragment(output, JSON_NEWLINE)?;
        self.push_json_fragment(output, stepback)
    }

    fn push_json_member(
        &self,
        output: &mut String,
        member: &JsonObjectMember,
        pretty: bool,
    ) -> Result<()> {
        self.push_json_fragment(output, &member.key)?;
        self.push_json_fragment(output, JSON_COLON)?;
        if pretty {
            self.push_json_fragment(output, JSON_SPACE)?;
        }
        self.push_json_fragment(output, &member.value)
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
            return Err(Error::type_error(
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
        format_ecmascript_number(value)
    }

    fn json_number_property_name(value: f64) -> String {
        if value == 0.0 {
            return "0".to_owned();
        }
        format_ecmascript_number(value)
    }

    fn json_boxed_stringify_value(&mut self, value: &Value) -> Result<Value> {
        let Value::Object(id) = value else {
            return Ok(value.clone());
        };
        if let Some(ObjectPrimitiveValue::Bool(value)) = self.objects.primitive_value(*id)? {
            return Ok(Value::Bool(*value));
        }
        if matches!(
            self.objects.primitive_value(*id)?,
            Some(ObjectPrimitiveValue::Number(_))
        ) {
            return self.json_object_to_number(value).map(Value::Number);
        }
        if self.objects.string_object_value(*id)?.is_some() {
            return self
                .json_object_to_string(value)
                .and_then(|text| self.heap_string_value(&text));
        }
        Ok(value.clone())
    }

    fn json_object_to_number(&mut self, value: &Value) -> Result<f64> {
        self.to_number(value)
    }

    pub(in crate::runtime::native) fn json_object_to_string(
        &mut self,
        value: &Value,
    ) -> Result<String> {
        self.to_string(value)
    }

    fn call_json_callback(
        &mut self,
        function: &Value,
        this_value: Value,
        args: &[Value],
    ) -> Result<Value> {
        let value = self
            .call(function, args, this_value)?
            .into_native_value_result()?;
        self.runtime_value(value)
    }
}
