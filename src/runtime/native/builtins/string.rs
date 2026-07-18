use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::call::RuntimeCallArgs,
    value::{ErrorName, ObjectId, Value},
};

use super::{
    NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY, STRING_NAME,
    string_methods::STRING_PROTOTYPE_METHODS,
};

const STRING_LENGTH_PROPERTY: &str = "length";
const EMPTY_STRING: &str = "";
const STRING_METHOD_NULLISH_RECEIVER_ERROR: &str =
    "String.prototype method cannot convert undefined or null to object";
const STRING_METHOD_SYMBOL_RECEIVER_ERROR: &str =
    "String.prototype method cannot convert a symbol to a string";
const STRING_REPEAT_NEGATIVE_ERROR: &str = "String.prototype.repeat count must be non-negative";
const STRING_REPEAT_INFINITE_ERROR: &str = "String.prototype.repeat count must be finite";

impl Context {
    pub(in crate::runtime) fn string_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::String) {
            return Ok(Value::NativeFunction(id));
        }

        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype_id = self.string_prototype_id_with_constructor(constructor.clone())?;
        let prototype = Value::Object(prototype_id);
        let name = self.native_function_name_value(NativeFunctionKind::String)?;
        self.push_native_function_with_id(id, NativeFunctionKind::String, prototype, name)?;
        self.install_string_static_methods(id)?;
        self.install_string_prototype_methods(prototype_id)?;
        self.insert_global_builtin(STRING_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime::native) fn eval_string_constructor(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let value = self.eval_string_argument_utf16(args.as_slice())?;
        self.heap_utf16_string_value(&value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_constructor(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let value = self.eval_string_argument_utf16(args)?;
        self.heap_utf16_string_value(&value)
    }

    pub(in crate::runtime::native) fn construct_string_object(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let value = match args.as_slice().first() {
            Some(value) => self.to_utf16_string(value)?,
            None => Vec::new(),
        };
        self.create_string_object_from_utf16(&value)
    }

    pub(in crate::runtime::native) fn create_string_object_from_value(
        &mut self,
        value: &Value,
    ) -> Result<Value> {
        let value = self.string_argument_utf16(value)?;
        self.create_string_object_from_utf16(&value)
    }

    fn create_string_object_from_utf16(&mut self, value: &[u16]) -> Result<Value> {
        let value = self.intern_utf16_heap_string(value)?;
        let prototype = self.string_constructor_prototype()?;
        let length_key = self.intern_property_key(STRING_LENGTH_PROPERTY)?;
        self.objects.create_string_object(
            value,
            prototype,
            length_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    pub(in crate::runtime::native) fn eval_string_prototype_char_at(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_char_at(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_char_at(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_utf16(this_value)?;
        let position = self.string_position_arg(args.first())?;
        let Some(unit) = Self::char_at(&text, position) else {
            return self.heap_string_value(EMPTY_STRING);
        };
        self.heap_string_code_unit_value(unit)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_char_code_at(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_char_code_at(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_char_code_at(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_utf16(this_value)?;
        let position = self.string_position_arg(args.first())?;
        let Some(unit) = Self::char_at(&text, position) else {
            return Ok(Value::Number(f64::NAN));
        };
        Ok(Value::Number(f64::from(unit)))
    }

    pub(in crate::runtime::native) fn eval_string_prototype_concat(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_concat(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_concat(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let mut output = self.string_receiver_utf16(this_value)?;
        for value in args {
            output.extend(self.string_argument_utf16(value)?);
            self.check_utf16_string_len(&output)?;
        }
        self.heap_utf16_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_includes(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_includes(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_includes(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_utf16(this_value)?;
        let search = args.first().cloned().unwrap_or(Value::Undefined);
        if self.string_is_regexp(&search)? {
            return Err(Error::type_error(
                "String.prototype.includes does not accept a RegExp",
            ));
        }
        let needle = self.string_argument_utf16_or_undefined(args.first())?;
        let position = self.clamped_start_position(args.get(1), Self::char_len(&text))?;
        Ok(Value::Bool(Self::string_contains_from(
            &text, &needle, position,
        )))
    }

    pub(in crate::runtime::native) fn eval_string_prototype_index_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_index_of(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_index_of(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_utf16(this_value)?;
        let needle = self.string_argument_utf16_or_undefined(args.first())?;
        let position = self.clamped_start_position(args.get(1), Self::char_len(&text))?;
        let index = Self::string_index_of_from(&text, &needle, position);
        Ok(Value::Number(Self::optional_index_to_number(index)?))
    }

    pub(in crate::runtime::native) fn eval_string_prototype_last_index_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_last_index_of(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_last_index_of(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_utf16(this_value)?;
        let needle = self.string_argument_utf16_or_undefined(args.first())?;
        let position = self.last_index_position(args.get(1), Self::char_len(&text))?;
        let index = Self::string_last_index_of(&text, &needle, position);
        Ok(Value::Number(Self::optional_index_to_number(index)?))
    }

    pub(in crate::runtime::native) fn eval_string_prototype_slice(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_slice(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_slice(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_utf16(this_value)?;
        let length = Self::char_len(&text);
        let start = self.slice_bound(args.first(), length, 0)?;
        let end = self.slice_bound(args.get(1), length, length)?;
        let output = Self::char_range(&text, start, end)?;
        self.heap_utf16_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_substring(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_substring(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_substring(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_utf16(this_value)?;
        let length = Self::char_len(&text);
        let mut start = self.substring_bound(args.first(), length, 0)?;
        let mut end = self.substring_bound(args.get(1), length, length)?;
        if start > end {
            core::mem::swap(&mut start, &mut end);
        }
        let output = Self::char_range(&text, start, end)?;
        self.heap_utf16_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_starts_with(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_starts_with(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_starts_with(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_utf16(this_value)?;
        let search = args.first().cloned().unwrap_or(Value::Undefined);
        if self.string_is_regexp(&search)? {
            return Err(Error::type_error(
                "String.prototype.startsWith does not accept a RegExp",
            ));
        }
        let needle = self.string_argument_utf16_or_undefined(args.first())?;
        let position = self.clamped_start_position(args.get(1), Self::char_len(&text))?;
        Ok(Value::Bool(Self::string_starts_with_at(
            &text, &needle, position,
        )))
    }

    pub(in crate::runtime::native) fn eval_string_prototype_ends_with(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_ends_with(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_ends_with(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_utf16(this_value)?;
        let search = args.first().cloned().unwrap_or(Value::Undefined);
        if self.string_is_regexp(&search)? {
            return Err(Error::type_error(
                "String.prototype.endsWith does not accept a RegExp",
            ));
        }
        let needle = self.string_argument_utf16_or_undefined(args.first())?;
        let length = Self::char_len(&text);
        let end_position = self.ends_with_position(args.get(1), length)?;
        Ok(Value::Bool(Self::string_ends_with_at(
            &text,
            &needle,
            end_position,
        )?))
    }

    pub(in crate::runtime::native) fn eval_string_prototype_repeat(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_repeat(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_repeat(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_utf16(this_value)?;
        let count = self.repeat_count(args.first())?;
        let unit_len = text
            .len()
            .checked_mul(count)
            .ok_or_else(|| Error::limit("string repeat length overflowed"))?;
        if unit_len > self.limits.max_string_len {
            return Err(Error::limit("string length limit exceeded"));
        }
        let mut output = Vec::new();
        output
            .try_reserve(unit_len)
            .map_err(|_| Error::limit("string repeat allocation exceeded supported range"))?;
        for _ in 0..count {
            output.extend_from_slice(&text);
        }
        self.heap_utf16_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_trim(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_string_args(args.as_slice());
        let text = self.string_receiver_value(this_value)?;
        let output = text.trim_matches(Self::is_ecmascript_whitespace).to_owned();
        self.heap_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_trim_start(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_string_args(args.as_slice());
        let text = self.string_receiver_value(this_value)?;
        let output = text
            .trim_start_matches(Self::is_ecmascript_whitespace)
            .to_owned();
        self.heap_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_trim_end(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_string_args(args.as_slice());
        let text = self.string_receiver_value(this_value)?;
        let output = text
            .trim_end_matches(Self::is_ecmascript_whitespace)
            .to_owned();
        self.heap_string_value(&output)
    }

    const fn is_ecmascript_whitespace(character: char) -> bool {
        matches!(
            character,
            '\u{0009}' | '\u{000B}' | '\u{000C}' | '\u{0020}' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
                ..='\u{200A}'
                    | '\u{2028}'
                    | '\u{2029}'
                    | '\u{202F}'
                    | '\u{205F}'
                    | '\u{3000}'
                    | '\u{FEFF}'
                    | '\n'
                    | '\r'
        )
    }

    pub(in crate::runtime::native) fn eval_string_prototype_to_lower_case(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_string_args(args.as_slice());
        let text = self.string_receiver_utf16(this_value)?;
        self.string_case_map_utf16(&text, &icu_locale::langid!("und"), false)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_to_upper_case(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_string_args(args.as_slice());
        let text = self.string_receiver_utf16(this_value)?;
        self.string_case_map_utf16(&text, &icu_locale::langid!("und"), true)
    }

    pub(in crate::runtime) fn string_prototype_property_value(
        &mut self,
        receiver: &Value,
        property: &str,
    ) -> Result<Value> {
        let prototype = self.string_constructor_prototype()?;
        self.get_prototype_property_value_with_receiver(prototype, receiver, property)
    }

    fn string_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<ObjectId> {
        let constructor_key = self.object_constructor_property_key()?;
        let object_prototype = self.objects.object_prototype_id(
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let empty = self.intern_utf16_heap_string(&[])?;
        let length_key = self.intern_property_key(STRING_LENGTH_PROPERTY)?;
        let Value::Object(prototype) = self.objects.create_string_object(
            empty,
            object_prototype,
            length_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?
        else {
            return Err(Error::runtime("String prototype is not an object"));
        };
        self.define_non_enumerable_object_property(
            prototype,
            OBJECT_CONSTRUCTOR_PROPERTY,
            constructor,
        )?;
        Ok(prototype)
    }

    pub(in crate::runtime) fn string_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.string_constructor_value()? else {
            return Err(Error::runtime("String constructor value is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("String prototype is not an object")),
        }
    }

    fn eval_string_argument_utf16(&mut self, args: &[Value]) -> Result<Vec<u16>> {
        let Some(value) = args.first() else {
            return Ok(Vec::new());
        };
        let units = if let Value::Symbol(symbol) = value {
            symbol.display_name().encode_utf16().collect()
        } else {
            self.to_utf16_string(value)?
        };
        self.check_utf16_string_len(&units)?;
        Ok(units)
    }

    fn install_string_prototype_methods(&mut self, prototype: ObjectId) -> Result<()> {
        for (name, kind) in STRING_PROTOTYPE_METHODS {
            let function = self.create_native_function(*kind, Value::Undefined)?;
            self.define_non_enumerable_object_property(prototype, name, function)?;
        }
        self.install_string_annex_b_prototype_methods(prototype)?;
        self.install_string_extra_prototype_methods(prototype)?;
        self.install_string_modern_prototype_methods(prototype)
    }

    pub(in crate::runtime::native) fn string_receiver_value(
        &mut self,
        value: &Value,
    ) -> Result<String> {
        if matches!(value, Value::Undefined | Value::Null) {
            return Err(Error::type_error(STRING_METHOD_NULLISH_RECEIVER_ERROR));
        }
        self.to_string(value).map_err(|error| {
            if matches!(value, Value::Symbol(_)) {
                return Error::type_error(STRING_METHOD_SYMBOL_RECEIVER_ERROR);
            }
            error
        })
    }

    pub(in crate::runtime::native) fn string_receiver_utf16(
        &mut self,
        value: &Value,
    ) -> Result<Vec<u16>> {
        if matches!(value, Value::Undefined | Value::Null) {
            return Err(Error::type_error(STRING_METHOD_NULLISH_RECEIVER_ERROR));
        }
        self.to_utf16_string(value).map_err(|error| {
            if matches!(value, Value::Symbol(_)) {
                return Error::type_error(STRING_METHOD_SYMBOL_RECEIVER_ERROR);
            }
            error
        })
    }

    pub(in crate::runtime::native) fn string_argument_text(
        &mut self,
        value: &Value,
    ) -> Result<String> {
        self.to_string(value)
    }

    pub(in crate::runtime::native) fn string_argument_utf16(
        &mut self,
        value: &Value,
    ) -> Result<Vec<u16>> {
        self.to_utf16_string(value)
    }

    fn string_argument_utf16_or_undefined(&mut self, value: Option<&Value>) -> Result<Vec<u16>> {
        if let Some(value) = value {
            self.string_argument_utf16(value)
        } else {
            self.to_utf16_string(&Value::Undefined)
        }
    }

    fn string_position_arg(&mut self, value: Option<&Value>) -> Result<usize> {
        let Some(value) = value else {
            return Ok(0);
        };
        let integer = self.to_integer_or_infinity(value)?;
        if integer < 0.0 || !integer.is_finite() {
            return Ok(usize::MAX);
        }
        Ok(Self::finite_nonnegative_integer_to_usize(
            integer,
            "string index exceeded supported range",
        )
        .map_or(usize::MAX, |index| index))
    }

    fn char_at(text: &[u16], position: usize) -> Option<u16> {
        if position >= Self::char_len(text) {
            return None;
        }
        text.get(position).copied()
    }

    const fn char_len(text: &[u16]) -> usize {
        text.len()
    }

    fn char_range(text: &[u16], start: usize, end: usize) -> Result<Vec<u16>> {
        if end <= start {
            return Ok(Vec::new());
        }
        text.get(start..end)
            .map(<[u16]>::to_vec)
            .ok_or_else(|| Error::runtime("string range is outside the code-unit sequence"))
    }

    fn string_contains_from(text: &[u16], needle: &[u16], position: usize) -> bool {
        Self::string_index_of_from(text, needle, position).is_some()
    }

    fn string_index_of_from(text: &[u16], needle: &[u16], position: usize) -> Option<usize> {
        let length = Self::char_len(text);
        if position > length {
            return None;
        }
        if needle.is_empty() {
            return Some(position);
        }
        (position..=length).find(|index| Self::string_starts_with_at(text, needle, *index))
    }

    fn string_last_index_of(text: &[u16], needle: &[u16], position: usize) -> Option<usize> {
        let length = Self::char_len(text);
        let end = position.min(length);
        if needle.is_empty() {
            return Some(end);
        }
        (0..=end)
            .rev()
            .find(|index| Self::string_starts_with_at(text, needle, *index))
    }

    fn string_starts_with_at(text: &[u16], needle: &[u16], position: usize) -> bool {
        let needle_len = Self::char_len(needle);
        let Some(end) = position.checked_add(needle_len) else {
            return false;
        };
        if end > Self::char_len(text) {
            return false;
        }
        text.get(position..end)
            .is_some_and(|candidate| candidate == needle)
    }

    fn string_ends_with_at(text: &[u16], needle: &[u16], end_position: usize) -> Result<bool> {
        let needle_len = Self::char_len(needle);
        if needle_len > end_position {
            return Ok(false);
        }
        let start = end_position
            .checked_sub(needle_len)
            .ok_or_else(|| Error::limit("string end position underflowed"))?;
        Ok(Self::string_starts_with_at(text, needle, start))
    }

    fn clamped_start_position(&mut self, value: Option<&Value>, length: usize) -> Result<usize> {
        let integer = match value {
            Some(value) => self.to_integer_or_infinity(value)?,
            None => 0.0,
        };
        Self::clamp_integer(integer, length)
    }

    fn ends_with_position(&mut self, value: Option<&Value>, length: usize) -> Result<usize> {
        match value {
            None | Some(Value::Undefined) => Ok(length),
            Some(value) => {
                let number = self.to_number(value)?;
                let integer = if number.is_nan() {
                    f64::INFINITY
                } else {
                    self.to_integer_or_infinity(&Value::Number(number))?
                };
                Self::clamp_integer(integer, length)
            }
        }
    }

    fn slice_bound(
        &mut self,
        value: Option<&Value>,
        length: usize,
        default: usize,
    ) -> Result<usize> {
        match value {
            None | Some(Value::Undefined) => Ok(default),
            Some(value) => {
                let integer = self.to_integer_or_infinity(value)?;
                Self::relative_bound(integer, length)
            }
        }
    }

    fn substring_bound(
        &mut self,
        value: Option<&Value>,
        length: usize,
        default: usize,
    ) -> Result<usize> {
        match value {
            None | Some(Value::Undefined) => Ok(default),
            Some(value) => {
                let integer = self.to_integer_or_infinity(value)?;
                Self::clamp_integer(integer, length)
            }
        }
    }

    fn last_index_position(&mut self, value: Option<&Value>, length: usize) -> Result<usize> {
        match value {
            None | Some(Value::Undefined) => Ok(length),
            Some(value) => {
                let number = self.to_number(value)?;
                let integer = if number.is_nan() {
                    f64::INFINITY
                } else {
                    self.to_integer_or_infinity(&Value::Number(number))?
                };
                Self::clamp_integer(integer, length)
            }
        }
    }

    fn relative_bound(integer: f64, length: usize) -> Result<usize> {
        if integer == f64::NEG_INFINITY {
            return Ok(0);
        }
        if integer == f64::INFINITY {
            return Ok(length);
        }
        let length_number =
            Self::usize_to_number(length, "string length exceeded supported range")?;
        let index = if integer < 0.0 {
            (length_number + integer).max(0.0)
        } else {
            integer.min(length_number)
        };
        Self::finite_nonnegative_integer_to_usize(index, "string index exceeded supported range")
    }

    fn clamp_integer(integer: f64, length: usize) -> Result<usize> {
        if integer <= 0.0 {
            return Ok(0);
        }
        if integer == f64::INFINITY {
            return Ok(length);
        }
        let length_number =
            Self::usize_to_number(length, "string length exceeded supported range")?;
        Self::finite_nonnegative_integer_to_usize(
            integer.min(length_number),
            "string index exceeded supported range",
        )
    }

    fn repeat_count(&mut self, value: Option<&Value>) -> Result<usize> {
        let argument = value.cloned().unwrap_or(Value::Undefined);
        let integer = self.to_integer_or_infinity(&argument)?;
        if integer < 0.0 {
            return Err(Error::exception(
                ErrorName::RangeError,
                STRING_REPEAT_NEGATIVE_ERROR,
            ));
        }
        if !integer.is_finite() {
            return Err(Error::exception(
                ErrorName::RangeError,
                STRING_REPEAT_INFINITE_ERROR,
            ));
        }
        Self::finite_nonnegative_integer_to_usize(
            integer,
            "string repeat count exceeded supported range",
        )
    }

    fn optional_index_to_number(index: Option<usize>) -> Result<f64> {
        let Some(index) = index else {
            return Ok(-1.0);
        };
        let index = u32::try_from(index)
            .map_err(|_| Error::limit("string index exceeded supported range"))?;
        Ok(f64::from(index))
    }

    const fn discard_string_args(_args: &[Value]) {}
}
