use crate::{
    error::{Error, Result},
    runtime::call::RuntimeCallArgs,
    runtime::object::{ObjectPrimitiveValue, ObjectPropertyInit, PropertyEnumerable},
    runtime::{Context, roots::VmRootKind},
    value::{ObjectId, Value, format_ecmascript_number},
};

use super::{
    JSON_IS_RAW_JSON_NAME, JSON_NAME, JSON_PARSE_NAME, JSON_RAW_JSON_NAME, JSON_STRINGIFY_NAME,
    NativeFunctionKind, json_parse::ParsedJson, json_parse::parse_json_text,
    json_quote::quote_json_string,
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
        self.check_utf16_string_len(&text)?;
        let parsed = parse_json_text(&text, self.limits.max_expression_depth)?;
        let Some(reviver) = args.get(1) else {
            return self.value_from_json(parsed);
        };
        if !self.semantic_is_callable(reviver)? {
            return self.value_from_json(parsed);
        }
        let (value, record) = self.value_and_record_from_json(parsed)?;
        let _value_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&value))?;
        let record_roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        record.add_original_object_roots(&record_roots)?;
        let holder = self.create_json_wrapper(value)?;
        self.internalize_json_property(&holder, JSON_EMPTY_KEY, reviver, Some(&record))
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

    fn value_from_json(&mut self, value: ParsedJson) -> Result<Value> {
        match value {
            ParsedJson::Null { .. } => Ok(Value::Null),
            ParsedJson::Bool { value, .. } => Ok(Value::Bool(value)),
            ParsedJson::Number { value, .. } => Ok(Value::Number(value)),
            ParsedJson::String { value, .. } => self.heap_utf16_string_value(&value),
            ParsedJson::Array(values) => self.array_from_json(values),
            ParsedJson::Object(object) => self.object_from_json(object),
        }
    }

    fn array_from_json(&mut self, values: Vec<ParsedJson>) -> Result<Value> {
        let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        let mut elements = Vec::with_capacity(values.len());
        for value in values {
            let value = self.value_from_json(value)?;
            roots.add_values(std::iter::once(&value))?;
            elements.push(value);
        }
        self.create_array_from_elements(elements)
    }

    fn object_from_json(&mut self, object: Vec<(Vec<u16>, ParsedJson)>) -> Result<Value> {
        let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        let mut names = Vec::with_capacity(object.len());
        let mut values = Vec::with_capacity(object.len());
        for (key, value) in object {
            self.check_utf16_string_len(&key)?;
            let key = String::from_utf16_lossy(&key);
            self.check_string_len(&key)?;
            let property = self.intern_property_key(&key)?;
            names.push(key);
            let value = self.value_from_json(value)?;
            roots.add_values(std::iter::once(&value))?;
            values.push((property, value));
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

    fn json_replacer(&mut self, value: Option<&Value>) -> Result<JsonReplacer> {
        let Some(value) = value else {
            return Ok(JsonReplacer::None);
        };
        if self.semantic_is_callable(value)? {
            return Ok(JsonReplacer::Function(value.clone()));
        }
        let Value::Object(_) = value else {
            return Ok(JsonReplacer::None);
        };
        if !self.semantic_is_array(value)? {
            return Ok(JsonReplacer::None);
        }
        self.json_replacer_property_list(value)
            .map(JsonReplacer::PropertyList)
    }

    fn json_replacer_property_list(&mut self, value: &Value) -> Result<Vec<String>> {
        let length = self.array_like_length(value)?;
        let mut keys = Vec::new();
        for index in 0..length {
            let element = self.get_named(value, &index.to_string())?;
            let Some(key) = self.json_replacer_property_name(&element)? else {
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
            Value::String(value) => Ok(Some(value.as_str().to_owned())),
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
            | Value::BigInt(_)
            | Value::Symbol(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_) => Ok(None),
        }
    }

    fn json_gap(&mut self, value: Option<&Value>) -> Result<String> {
        let Some(value) = value else {
            return Ok(String::new());
        };
        let gap = match value {
            Value::Number(value) => JSON_SPACE.repeat(Self::json_gap_space_count(*value)),
            Value::String(value) => Self::json_gap_string(value.as_str()),
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
            | Value::BigInt(_)
            | Value::Symbol(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_) => String::new(),
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
        if !matches!(value, Value::Object(_) | Value::BigInt(_)) {
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
            Value::BigInt(_) => Err(Error::type_error("Do not know how to serialize a BigInt")),
            Value::String(value) => self.stringify_json_string(value.as_utf16()).map(Some),
            Value::Object(id) => {
                if let Some(text) = self.raw_json_text(*id)? {
                    return Ok(Some(text));
                }
                self.stringify_json_object(*id, state).map(Some)
            }
        }
    }

    fn stringify_json_object(
        &mut self,
        id: ObjectId,
        state: &mut JsonStringifyState,
    ) -> Result<String> {
        Self::push_json_stack(id, &mut state.stack)?;
        let object = Value::Object(id);
        let result = if self.semantic_is_array(&object)? {
            let length = self.array_like_length(&object)?;
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
            let key = self.stringify_json_string(&key.encode_utf16().collect::<Vec<_>>())?;
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

    fn stringify_json_string(&self, value: &[u16]) -> Result<String> {
        quote_json_string(value, self.limits.max_string_len)
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
        if let Some(ObjectPrimitiveValue::BigInt(value)) = self.objects.primitive_value(*id)? {
            return Ok(Value::BigInt(value.clone()));
        }
        if self.objects.string_object_value(*id)?.is_some() {
            let units = self.to_utf16_string(value)?;
            return self.heap_utf16_string_value(&units);
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

    pub(super) fn call_json_callback(
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
