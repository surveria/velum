use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::call::RuntimeCallArgs,
    runtime::object::{ObjectPropertyInit, PropertyEnumerable},
    value::{ErrorName, ObjectId, Value},
};

use super::{
    NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY, STRING_NAME, STRING_PROTOTYPE_CHAR_AT_NAME,
    STRING_PROTOTYPE_CHAR_CODE_AT_NAME, STRING_PROTOTYPE_CONCAT_NAME,
    STRING_PROTOTYPE_ENDS_WITH_NAME, STRING_PROTOTYPE_INCLUDES_NAME,
    STRING_PROTOTYPE_INDEX_OF_NAME, STRING_PROTOTYPE_LAST_INDEX_OF_NAME,
    STRING_PROTOTYPE_REPEAT_NAME, STRING_PROTOTYPE_SLICE_NAME, STRING_PROTOTYPE_STARTS_WITH_NAME,
    STRING_PROTOTYPE_SUBSTRING_NAME, STRING_PROTOTYPE_TO_LOWER_CASE_NAME,
    STRING_PROTOTYPE_TO_UPPER_CASE_NAME, STRING_PROTOTYPE_TRIM_END_NAME,
    STRING_PROTOTYPE_TRIM_NAME, STRING_PROTOTYPE_TRIM_START_NAME,
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
        let value = self.eval_string_argument(args.as_slice())?;
        self.heap_string_value(&value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_constructor(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let value = self.eval_string_argument(args)?;
        self.heap_string_value(&value)
    }

    pub(in crate::runtime::native) fn construct_string_object(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let value = self.eval_string_argument(args.as_slice())?;
        self.create_string_object_from_text(&value)
    }

    pub(in crate::runtime::native) fn create_string_object_from_value(
        &mut self,
        value: &Value,
    ) -> Result<Value> {
        let value = self.string_argument_text(value)?;
        self.create_string_object_from_text(&value)
    }

    fn create_string_object_from_text(&mut self, value: &str) -> Result<Value> {
        let value = self.intern_heap_string(value)?;
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
        let text = self.string_receiver_value(this_value)?;
        let position = Self::string_position_arg(args.first());
        let Some(ch) = Self::char_at(&text, position) else {
            return self.heap_string_value(EMPTY_STRING);
        };
        self.heap_string_char_value(ch)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_char_code_at(
        &self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_char_code_at(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_char_code_at(
        &self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_value(this_value)?;
        let position = Self::string_position_arg(args.first());
        let Some(ch) = Self::char_at(&text, position) else {
            return Ok(Value::Number(f64::NAN));
        };
        Ok(Value::Number(f64::from(u32::from(ch))))
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
        let mut output = self.string_receiver_value(this_value)?;
        for value in args {
            output.push_str(&self.string_argument_text(value)?);
            self.check_string_len(&output)?;
        }
        self.heap_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_includes(
        &self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_includes(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_includes(
        &self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_value(this_value)?;
        let needle = self.string_argument_or_undefined(args.first())?;
        let position = Self::clamped_start_position(args.get(1), Self::char_len(&text))?;
        Ok(Value::Bool(Self::string_contains_from(
            &text, &needle, position,
        )?))
    }

    pub(in crate::runtime::native) fn eval_string_prototype_index_of(
        &self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_index_of(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_index_of(
        &self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_value(this_value)?;
        let needle = self.string_argument_or_undefined(args.first())?;
        let position = Self::clamped_start_position(args.get(1), Self::char_len(&text))?;
        let index = Self::string_index_of_from(&text, &needle, position)?;
        Ok(Value::Number(Self::optional_index_to_number(index)?))
    }

    pub(in crate::runtime::native) fn eval_string_prototype_last_index_of(
        &self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_last_index_of(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_last_index_of(
        &self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_value(this_value)?;
        let needle = self.string_argument_or_undefined(args.first())?;
        let position = Self::last_index_position(args.get(1), Self::char_len(&text))?;
        let index = Self::string_last_index_of(&text, &needle, position)?;
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
        let text = self.string_receiver_value(this_value)?;
        let length = Self::char_len(&text);
        let start = Self::slice_bound(args.first(), length, 0)?;
        let end = Self::slice_bound(args.get(1), length, length)?;
        let output = Self::char_range(&text, start, end)?;
        self.heap_string_value(&output)
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
        let text = self.string_receiver_value(this_value)?;
        let length = Self::char_len(&text);
        let mut start = Self::substring_bound(args.first(), length, 0)?;
        let mut end = Self::substring_bound(args.get(1), length, length)?;
        if start > end {
            std::mem::swap(&mut start, &mut end);
        }
        let output = Self::char_range(&text, start, end)?;
        self.heap_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_starts_with(
        &self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_starts_with(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_starts_with(
        &self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_value(this_value)?;
        let needle = self.string_argument_or_undefined(args.first())?;
        let position = Self::clamped_start_position(args.get(1), Self::char_len(&text))?;
        Ok(Value::Bool(Self::string_starts_with_at(
            &text, &needle, position,
        )?))
    }

    pub(in crate::runtime::native) fn eval_string_prototype_ends_with(
        &self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_ends_with(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_ends_with(
        &self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_value(this_value)?;
        let needle = self.string_argument_or_undefined(args.first())?;
        let length = Self::char_len(&text);
        let end_position = Self::ends_with_position(args.get(1), length)?;
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
        let text = self.string_receiver_value(this_value)?;
        let count = Self::repeat_count(args.first())?;
        let byte_len = text
            .len()
            .checked_mul(count)
            .ok_or_else(|| Error::limit("string repeat byte length overflowed"))?;
        if byte_len > self.limits.max_string_len {
            return Err(Error::limit("string length limit exceeded"));
        }
        let mut output = String::with_capacity(byte_len);
        for _ in 0..count {
            output.push_str(&text);
        }
        self.heap_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_trim(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_string_args(args.as_slice());
        let text = self.string_receiver_value(this_value)?;
        let output = text.trim().to_owned();
        self.heap_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_trim_start(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_string_args(args.as_slice());
        let text = self.string_receiver_value(this_value)?;
        let output = text.trim_start().to_owned();
        self.heap_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_trim_end(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_string_args(args.as_slice());
        let text = self.string_receiver_value(this_value)?;
        let output = text.trim_end().to_owned();
        self.heap_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_to_lower_case(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_string_args(args.as_slice());
        let text = self.string_receiver_value(this_value)?;
        let output = text.to_lowercase();
        self.check_string_len(&output)?;
        self.heap_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_to_upper_case(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_string_args(args.as_slice());
        let text = self.string_receiver_value(this_value)?;
        let output = text.to_uppercase();
        self.check_string_len(&output)?;
        self.heap_string_value(&output)
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

    pub(in crate::runtime) fn string_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.string_constructor_value()? else {
            return Err(Error::runtime("String constructor value is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("String prototype is not an object")),
        }
    }

    fn eval_string_argument(&self, args: &[Value]) -> Result<String> {
        let value = Self::string_argument_value(args.first());
        self.check_string_len(&value)?;
        Ok(value)
    }

    fn string_argument_value(value: Option<&Value>) -> String {
        value.map_or_else(String::new, ToString::to_string)
    }

    fn install_string_prototype_methods(&mut self, prototype: ObjectId) -> Result<()> {
        for (name, kind) in STRING_PROTOTYPE_METHODS {
            let function = self.create_native_function(*kind, Value::Undefined)?;
            self.define_non_enumerable_object_property(prototype, name, function)?;
        }
        self.install_string_extra_prototype_methods(prototype)
    }

    pub(in crate::runtime::native) fn string_receiver_value(
        &self,
        value: &Value,
    ) -> Result<String> {
        let text = match value {
            Value::Undefined | Value::Null => {
                return Err(Error::type_error(STRING_METHOD_NULLISH_RECEIVER_ERROR));
            }
            Value::String(value) => value.clone(),
            Value::HeapString(value) => value.as_str().to_owned(),
            Value::Object(id) => self
                .objects
                .string_object_value(*id)?
                .map_or_else(|| value.to_string(), ToOwned::to_owned),
            Value::Symbol(_) => {
                return Err(Error::type_error(STRING_METHOD_SYMBOL_RECEIVER_ERROR));
            }
            Value::Bool(_)
            | Value::Number(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Error(_) => value.to_string(),
        };
        self.check_string_len(&text)?;
        Ok(text)
    }

    pub(in crate::runtime::native) fn string_argument_text(&self, value: &Value) -> Result<String> {
        if matches!(value, Value::Symbol(_)) {
            return Err(Error::type_error(
                "String.prototype argument cannot be converted from symbol",
            ));
        }
        let text = match value {
            Value::Object(id) => self
                .objects
                .string_object_value(*id)?
                .map_or_else(|| value.to_string(), ToOwned::to_owned),
            _ => value.to_string(),
        };
        self.check_string_len(&text)?;
        Ok(text)
    }

    fn string_argument_or_undefined(&self, value: Option<&Value>) -> Result<String> {
        if let Some(value) = value {
            self.string_argument_text(value)
        } else {
            let text = Value::Undefined.to_string();
            self.check_string_len(&text)?;
            Ok(text)
        }
    }

    fn string_position_arg(value: Option<&Value>) -> usize {
        let Some(value) = value else {
            return 0;
        };
        match Self::to_integer_or_infinity(Self::value_to_number(value)) {
            IntegerOrInfinity::Finite(value) if value < 0 => usize::MAX,
            IntegerOrInfinity::Finite(value) => {
                usize::try_from(value).map_or(usize::MAX, |index| index)
            }
            IntegerOrInfinity::NegativeInfinity | IntegerOrInfinity::PositiveInfinity => usize::MAX,
        }
    }

    fn char_at(text: &str, position: usize) -> Option<char> {
        if position >= Self::char_len(text) {
            return None;
        }
        text.chars().nth(position)
    }

    fn char_len(text: &str) -> usize {
        text.chars().count()
    }

    fn char_range(text: &str, start: usize, end: usize) -> Result<String> {
        if end <= start {
            return Ok(String::new());
        }
        let count = end
            .checked_sub(start)
            .ok_or_else(|| Error::limit("string range length overflowed"))?;
        Ok(text.chars().skip(start).take(count).collect())
    }

    fn string_contains_from(text: &str, needle: &str, position: usize) -> Result<bool> {
        Self::string_index_of_from(text, needle, position).map(|index| index.is_some())
    }

    fn string_index_of_from(text: &str, needle: &str, position: usize) -> Result<Option<usize>> {
        let length = Self::char_len(text);
        if position > length {
            return Ok(None);
        }
        if needle.is_empty() {
            return Ok(Some(position));
        }
        for index in position..=length {
            if Self::string_starts_with_at(text, needle, index)? {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    fn string_last_index_of(text: &str, needle: &str, position: usize) -> Result<Option<usize>> {
        let length = Self::char_len(text);
        let end = position.min(length);
        if needle.is_empty() {
            return Ok(Some(end));
        }
        for index in (0..=end).rev() {
            if Self::string_starts_with_at(text, needle, index)? {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    fn string_starts_with_at(text: &str, needle: &str, position: usize) -> Result<bool> {
        let needle_len = Self::char_len(needle);
        let Some(end) = position.checked_add(needle_len) else {
            return Ok(false);
        };
        if end > Self::char_len(text) {
            return Ok(false);
        }
        Self::char_range(text, position, end).map(|candidate| candidate == needle)
    }

    fn string_ends_with_at(text: &str, needle: &str, end_position: usize) -> Result<bool> {
        let needle_len = Self::char_len(needle);
        if needle_len > end_position {
            return Ok(false);
        }
        let start = end_position
            .checked_sub(needle_len)
            .ok_or_else(|| Error::limit("string end position underflowed"))?;
        Self::string_starts_with_at(text, needle, start)
    }

    fn clamped_start_position(value: Option<&Value>, length: usize) -> Result<usize> {
        let number = value.map_or(0.0, Self::value_to_number);
        Self::clamp_integer(number, length)
    }

    fn ends_with_position(value: Option<&Value>, length: usize) -> Result<usize> {
        match value {
            None | Some(Value::Undefined) => Ok(length),
            Some(value) => Self::clamp_integer(Self::value_to_number(value), length),
        }
    }

    fn slice_bound(value: Option<&Value>, length: usize, default: usize) -> Result<usize> {
        match value {
            None | Some(Value::Undefined) => Ok(default),
            Some(value) => Self::relative_bound(Self::value_to_number(value), length),
        }
    }

    fn substring_bound(value: Option<&Value>, length: usize, default: usize) -> Result<usize> {
        match value {
            None | Some(Value::Undefined) => Ok(default),
            Some(value) => Self::clamp_integer(Self::value_to_number(value), length),
        }
    }

    fn last_index_position(value: Option<&Value>, length: usize) -> Result<usize> {
        match value {
            None | Some(Value::Undefined) => Ok(length),
            Some(value) => Self::clamp_integer(Self::value_to_number(value), length),
        }
    }

    fn relative_bound(number: f64, length: usize) -> Result<usize> {
        let integer = Self::to_integer_or_infinity(number);
        match integer {
            IntegerOrInfinity::NegativeInfinity => Ok(0),
            IntegerOrInfinity::PositiveInfinity => Ok(length),
            IntegerOrInfinity::Finite(value) if value < 0 => {
                let length_i64 = i64::try_from(length)
                    .map_err(|_| Error::limit("string length exceeded supported range"))?;
                let index = length_i64.saturating_add(value);
                if index <= 0 {
                    return Ok(0);
                }
                usize::try_from(index)
                    .map(|index| index.min(length))
                    .map_err(|_| Error::limit("string index exceeded supported range"))
            }
            IntegerOrInfinity::Finite(value) => usize::try_from(value)
                .map(|index| index.min(length))
                .map_err(|_| Error::limit("string index exceeded supported range")),
        }
    }

    fn clamp_integer(number: f64, length: usize) -> Result<usize> {
        let integer = Self::to_integer_or_infinity(number);
        match integer {
            IntegerOrInfinity::NegativeInfinity => Ok(0),
            IntegerOrInfinity::PositiveInfinity => Ok(length),
            IntegerOrInfinity::Finite(value) if value <= 0 => Ok(0),
            IntegerOrInfinity::Finite(value) => usize::try_from(value)
                .map(|index| index.min(length))
                .map_err(|_| Error::limit("string index exceeded supported range")),
        }
    }

    fn repeat_count(value: Option<&Value>) -> Result<usize> {
        let number = value.map_or(0.0, Self::value_to_number);
        let integer = Self::to_integer_or_infinity(number);
        match integer {
            IntegerOrInfinity::NegativeInfinity => Err(Error::exception(
                ErrorName::RangeError,
                STRING_REPEAT_NEGATIVE_ERROR,
            )),
            IntegerOrInfinity::PositiveInfinity => Err(Error::exception(
                ErrorName::RangeError,
                STRING_REPEAT_INFINITE_ERROR,
            )),
            IntegerOrInfinity::Finite(value) if value < 0 => Err(Error::exception(
                ErrorName::RangeError,
                STRING_REPEAT_NEGATIVE_ERROR,
            )),
            IntegerOrInfinity::Finite(value) => usize::try_from(value)
                .map_err(|_| Error::limit("string repeat count exceeded supported range")),
        }
    }

    fn to_integer_or_infinity(number: f64) -> IntegerOrInfinity {
        if number.is_nan() || number == 0.0 {
            return IntegerOrInfinity::Finite(0);
        }
        if number == f64::INFINITY {
            return IntegerOrInfinity::PositiveInfinity;
        }
        if number == f64::NEG_INFINITY {
            return IntegerOrInfinity::NegativeInfinity;
        }
        let value = if number.is_sign_negative() {
            number.ceil()
        } else {
            number.floor()
        };
        format!("{value:.0}").parse::<i64>().map_or_else(
            |_| {
                if value.is_sign_negative() {
                    IntegerOrInfinity::NegativeInfinity
                } else {
                    IntegerOrInfinity::PositiveInfinity
                }
            },
            IntegerOrInfinity::Finite,
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

#[derive(Debug, Clone, Copy)]
enum IntegerOrInfinity {
    NegativeInfinity,
    Finite(i64),
    PositiveInfinity,
}

const STRING_PROTOTYPE_METHODS: &[(&str, NativeFunctionKind)] = &[
    (
        STRING_PROTOTYPE_CHAR_AT_NAME,
        NativeFunctionKind::StringPrototypeCharAt,
    ),
    (
        STRING_PROTOTYPE_CHAR_CODE_AT_NAME,
        NativeFunctionKind::StringPrototypeCharCodeAt,
    ),
    (
        STRING_PROTOTYPE_CONCAT_NAME,
        NativeFunctionKind::StringPrototypeConcat,
    ),
    (
        STRING_PROTOTYPE_ENDS_WITH_NAME,
        NativeFunctionKind::StringPrototypeEndsWith,
    ),
    (
        STRING_PROTOTYPE_INCLUDES_NAME,
        NativeFunctionKind::StringPrototypeIncludes,
    ),
    (
        STRING_PROTOTYPE_INDEX_OF_NAME,
        NativeFunctionKind::StringPrototypeIndexOf,
    ),
    (
        STRING_PROTOTYPE_LAST_INDEX_OF_NAME,
        NativeFunctionKind::StringPrototypeLastIndexOf,
    ),
    (
        STRING_PROTOTYPE_REPEAT_NAME,
        NativeFunctionKind::StringPrototypeRepeat,
    ),
    (
        STRING_PROTOTYPE_SLICE_NAME,
        NativeFunctionKind::StringPrototypeSlice,
    ),
    (
        STRING_PROTOTYPE_STARTS_WITH_NAME,
        NativeFunctionKind::StringPrototypeStartsWith,
    ),
    (
        STRING_PROTOTYPE_SUBSTRING_NAME,
        NativeFunctionKind::StringPrototypeSubstring,
    ),
    (
        STRING_PROTOTYPE_TO_LOWER_CASE_NAME,
        NativeFunctionKind::StringPrototypeToLowerCase,
    ),
    (
        STRING_PROTOTYPE_TO_UPPER_CASE_NAME,
        NativeFunctionKind::StringPrototypeToUpperCase,
    ),
    (
        STRING_PROTOTYPE_TRIM_NAME,
        NativeFunctionKind::StringPrototypeTrim,
    ),
    (
        STRING_PROTOTYPE_TRIM_END_NAME,
        NativeFunctionKind::StringPrototypeTrimEnd,
    ),
    (
        STRING_PROTOTYPE_TRIM_START_NAME,
        NativeFunctionKind::StringPrototypeTrimStart,
    ),
];
