use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, object::PropertyEnumerable},
    value::{ErrorName, NativeFunctionId, ObjectId, Value},
};

use super::{
    NativeFunctionKind, STRING_FROM_CHAR_CODE_NAME, STRING_FROM_CODE_POINT_NAME,
    STRING_PROTOTYPE_AT_NAME, STRING_PROTOTYPE_CODE_POINT_AT_NAME, STRING_PROTOTYPE_PAD_END_NAME,
    STRING_PROTOTYPE_PAD_START_NAME, STRING_PROTOTYPE_TO_LOCALE_LOWER_CASE_NAME,
    STRING_PROTOTYPE_TO_LOCALE_UPPER_CASE_NAME, STRING_PROTOTYPE_TO_STRING_NAME,
    STRING_PROTOTYPE_TRIM_LEFT_NAME, STRING_PROTOTYPE_TRIM_RIGHT_NAME,
    STRING_PROTOTYPE_VALUE_OF_NAME, STRING_RAW_NAME,
};

const DEFAULT_PAD_STRING: &str = " ";
const MAX_CODE_POINT: f64 = 1_114_111.0;
const RAW_PROPERTY: &str = "raw";
const RANGE_CODE_POINT_ERROR: &str = "String.fromCodePoint code point must be valid";
const STRING_VALUE_RECEIVER_ERROR: &str =
    "String.prototype value method requires a string or String object";
const TO_LENGTH_LIMIT_ERROR: &str = "String length exceeded supported range";
const UINT16_MODULO: f64 = 65_536.0;

impl Context {
    pub(in crate::runtime::native) fn install_string_static_methods(
        &mut self,
        constructor: NativeFunctionId,
    ) -> Result<()> {
        self.define_string_static_method(
            constructor,
            STRING_FROM_CHAR_CODE_NAME,
            NativeFunctionKind::StringFromCharCode,
        )?;
        self.define_string_static_method(
            constructor,
            STRING_FROM_CODE_POINT_NAME,
            NativeFunctionKind::StringFromCodePoint,
        )?;
        self.define_string_static_method(
            constructor,
            STRING_RAW_NAME,
            NativeFunctionKind::StringRaw,
        )
    }

    pub(in crate::runtime::native) fn install_string_extra_prototype_methods(
        &mut self,
        prototype: ObjectId,
    ) -> Result<()> {
        for (name, kind) in STRING_EXTRA_PROTOTYPE_METHODS {
            let function = if let Some(id) = self.native_function_id(*kind) {
                Value::NativeFunction(id)
            } else {
                self.create_native_function(*kind, Value::Undefined)?
            };
            self.define_non_enumerable_object_property(prototype, name, function)?;
        }
        Ok(())
    }

    pub(in crate::runtime::native) fn eval_string_from_char_code(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_string_from_char_code(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_string_from_char_code(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let mut output = Vec::with_capacity(args.len());
        for value in args {
            let unit = Self::to_uint16(self.to_number(value)?);
            output.push(unit);
            self.check_utf16_string_len(&output)?;
        }
        self.heap_utf16_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_from_code_point(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_string_from_code_point(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_string_from_code_point(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let mut output = Vec::with_capacity(args.len());
        for value in args {
            let code_point = self.code_point_argument(value)?;
            append_code_point_utf16(&mut output, code_point)?;
            self.check_utf16_string_len(&output)?;
        }
        self.heap_utf16_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_raw(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_string_raw(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_string_raw(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let template = Self::argument_or_undefined(args.first());
        Self::ensure_object_like(&template, "String.raw template must be object coercible")?;
        let raw = self.get_named(&template, RAW_PROPERTY)?;
        Self::ensure_object_like(&raw, "String.raw raw property must be object coercible")?;
        let raw_length = self.raw_length(&raw)?;
        if raw_length == 0 {
            return self.heap_string_value("");
        }

        let mut output = String::new();
        for index in 0..raw_length {
            let raw_part = self.raw_part(&raw, index)?;
            output.push_str(&raw_part);
            self.check_string_len(&output)?;
            if let Some(substitution) = args.get(index.saturating_add(1)) {
                let text = self.string_argument_text(substitution)?;
                output.push_str(&text);
                self.check_string_len(&output)?;
            }
        }
        self.heap_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_at(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_at(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_at(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_utf16(this_value)?;
        let length = text.len();
        let Some(index) = self.relative_index(args.first(), length)? else {
            return Ok(Value::Undefined);
        };
        let Some(unit) = text.get(index).copied() else {
            return Ok(Value::Undefined);
        };
        self.heap_string_code_unit_value(unit)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_code_point_at(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_code_point_at(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_code_point_at(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let units = self.string_receiver_utf16(this_value)?;
        let position = self.position_arg(args.first())?;
        let Some(unit) = units.get(position).copied() else {
            return Ok(Value::Undefined);
        };
        if Self::is_high_surrogate(unit)
            && let Some(next) = units.get(position.saturating_add(1)).copied()
            && Self::is_low_surrogate(next)
        {
            return Ok(Value::Number(f64::from(Self::decode_surrogate_pair(
                unit, next,
            ))));
        }
        Ok(Value::Number(f64::from(unit)))
    }

    pub(in crate::runtime::native) fn eval_string_prototype_pad_start(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_pad(args.as_slice(), this_value, PadSide::Start)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_pad_end(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_string_prototype_pad(args.as_slice(), this_value, PadSide::End)
    }

    pub(in crate::runtime::native) fn eval_direct_string_prototype_pad(
        &mut self,
        args: &[Value],
        this_value: &Value,
        side: PadSide,
    ) -> Result<Value> {
        let text = self.string_receiver_value(this_value)?;
        let target_length = self.string_length_arg(args.first())?;
        let current_length = text.chars().count();
        if target_length <= current_length {
            return self.heap_string_value(&text);
        }
        let filler = match args.get(1) {
            None | Some(Value::Undefined) => DEFAULT_PAD_STRING.to_owned(),
            Some(value) => self.string_argument_text(value)?,
        };
        if filler.is_empty() {
            return self.heap_string_value(&text);
        }
        let fill_count = target_length
            .checked_sub(current_length)
            .ok_or_else(|| Error::limit("string pad length underflowed"))?;
        let padding = self.repeat_to_char_len(&filler, fill_count)?;
        let output = match side {
            PadSide::Start => format!("{padding}{text}"),
            PadSide::End => format!("{text}{padding}"),
        };
        self.check_string_len(&output)?;
        self.heap_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_to_string(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_extra_args(args.as_slice());
        let text = self.strict_string_value(this_value)?;
        self.heap_utf16_string_value(&text)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_value_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_extra_args(args.as_slice());
        let text = self.strict_string_value(this_value)?;
        self.heap_utf16_string_value(&text)
    }

    fn define_string_static_method(
        &mut self,
        constructor: NativeFunctionId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let function = self.create_native_function(kind, Value::Undefined)?;
        let key = self.intern_property_key(name)?;
        self.native_function_mut(constructor)?
            .properties_mut()
            .define_builtin(key, function, PropertyEnumerable::No)?;
        Ok(())
    }

    fn raw_length(&mut self, raw: &Value) -> Result<usize> {
        let value = self.get_named(raw, "length")?;
        self.string_length_value(&value)
    }

    fn raw_part(&mut self, raw: &Value, index: usize) -> Result<String> {
        let key = index.to_string();
        let value = self.get_named(raw, &key)?;
        self.string_argument_text(&value)
    }

    fn repeat_to_char_len(&self, filler: &str, target_len: usize) -> Result<String> {
        let mut output = String::new();
        let mut length = 0_usize;
        for ch in filler.chars().cycle() {
            if length >= target_len {
                break;
            }
            output.push(ch);
            length = length
                .checked_add(1)
                .ok_or_else(|| Error::limit("string pad character count overflowed"))?;
            self.check_string_len(&output)?;
        }
        Ok(output)
    }

    fn strict_string_value(&self, value: &Value) -> Result<Vec<u16>> {
        match value {
            Value::String(value) => Ok(value.encode_utf16().collect()),
            Value::HeapString(value) => Ok(value.as_utf16().to_vec()),
            Value::Object(id) => self
                .objects
                .string_object_utf16_value(*id)?
                .map(<[u16]>::to_vec)
                .ok_or_else(|| Error::type_error(STRING_VALUE_RECEIVER_ERROR)),
            _ => Err(Error::type_error(STRING_VALUE_RECEIVER_ERROR)),
        }
    }

    fn ensure_object_like(value: &Value, message: &str) -> Result<()> {
        match value {
            Value::Undefined | Value::Null => Err(Error::type_error(message)),
            Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Object(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Symbol(_) => Ok(()),
        }
    }

    fn relative_index(&mut self, value: Option<&Value>, length: usize) -> Result<Option<usize>> {
        let argument = value.cloned().unwrap_or(Value::Undefined);
        let integer = self.to_integer_or_infinity(&argument)?;
        let length_number =
            Self::usize_to_number(length, "string length exceeded supported range")?;
        let index = if integer < 0.0 {
            length_number + integer
        } else {
            integer
        };
        if index < 0.0 || index >= length_number {
            return Ok(None);
        }
        Self::finite_nonnegative_integer_to_usize(index, "string index exceeded range").map(Some)
    }

    fn position_arg(&mut self, value: Option<&Value>) -> Result<usize> {
        let argument = value.cloned().unwrap_or(Value::Undefined);
        let integer = self.to_integer_or_infinity(&argument)?;
        if integer <= 0.0 {
            return Ok(0);
        }
        if !integer.is_finite() {
            return Ok(usize::MAX);
        }
        Ok(
            Self::finite_nonnegative_integer_to_usize(integer, "string index exceeded range")
                .map_or(usize::MAX, |index| index),
        )
    }

    fn string_length_arg(&mut self, value: Option<&Value>) -> Result<usize> {
        let value = Self::argument_or_undefined(value);
        self.string_length_value(&value)
    }

    fn string_length_value(&mut self, value: &Value) -> Result<usize> {
        let length = self.to_length(value)?;
        Self::length_to_usize(length, TO_LENGTH_LIMIT_ERROR)
    }

    fn code_point_argument(&mut self, value: &Value) -> Result<u32> {
        let number = if let Value::Number(number) = value {
            *number
        } else {
            self.to_number(value)?
        };
        Self::validated_code_point(number)
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn validated_code_point(number: f64) -> Result<u32> {
        if !number.is_finite() || !(0.0..=MAX_CODE_POINT).contains(&number) || number.fract() != 0.0
        {
            return Err(Error::exception(
                ErrorName::RangeError,
                RANGE_CODE_POINT_ERROR,
            ));
        }
        Ok(number as u32)
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn to_uint16(number: f64) -> u16 {
        if !number.is_finite() || number == 0.0 {
            return 0;
        }
        let integer = if number.is_sign_negative() {
            number.ceil()
        } else {
            number.floor()
        };
        let unit = integer.rem_euclid(UINT16_MODULO);
        unit as u16
    }

    const fn is_high_surrogate(unit: u16) -> bool {
        unit >= 0xD800 && unit <= 0xDBFF
    }

    const fn is_low_surrogate(unit: u16) -> bool {
        unit >= 0xDC00 && unit <= 0xDFFF
    }

    fn decode_surrogate_pair(high: u16, low: u16) -> u32 {
        let high = u32::from(high) - 0xD800;
        let low = u32::from(low) - 0xDC00;
        0x1_0000 + ((high << 10) | low)
    }

    const fn discard_extra_args(_args: &[Value]) {}
}

fn append_code_point_utf16(output: &mut Vec<u16>, code_point: u32) -> Result<()> {
    if let Ok(unit) = u16::try_from(code_point) {
        output.push(unit);
        return Ok(());
    }
    let supplementary = code_point
        .checked_sub(0x1_0000)
        .ok_or_else(|| Error::exception(ErrorName::RangeError, RANGE_CODE_POINT_ERROR))?;
    let high = u16::try_from(0xD800 + (supplementary >> 10))
        .map_err(|_| Error::exception(ErrorName::RangeError, RANGE_CODE_POINT_ERROR))?;
    let low = u16::try_from(0xDC00 + (supplementary & 0x3FF))
        .map_err(|_| Error::exception(ErrorName::RangeError, RANGE_CODE_POINT_ERROR))?;
    output.push(high);
    output.push(low);
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub(in crate::runtime::native) enum PadSide {
    Start,
    End,
}

pub(super) const STRING_EXTRA_PROTOTYPE_METHODS: &[(&str, NativeFunctionKind)] = &[
    (
        STRING_PROTOTYPE_AT_NAME,
        NativeFunctionKind::StringPrototypeAt,
    ),
    (
        STRING_PROTOTYPE_CODE_POINT_AT_NAME,
        NativeFunctionKind::StringPrototypeCodePointAt,
    ),
    (
        STRING_PROTOTYPE_PAD_END_NAME,
        NativeFunctionKind::StringPrototypePadEnd,
    ),
    (
        STRING_PROTOTYPE_PAD_START_NAME,
        NativeFunctionKind::StringPrototypePadStart,
    ),
    (
        STRING_PROTOTYPE_TO_LOCALE_LOWER_CASE_NAME,
        NativeFunctionKind::StringPrototypeToLocaleLowerCase,
    ),
    (
        STRING_PROTOTYPE_TO_LOCALE_UPPER_CASE_NAME,
        NativeFunctionKind::StringPrototypeToLocaleUpperCase,
    ),
    (
        STRING_PROTOTYPE_TO_STRING_NAME,
        NativeFunctionKind::StringPrototypeToString,
    ),
    (
        STRING_PROTOTYPE_TRIM_LEFT_NAME,
        NativeFunctionKind::StringPrototypeTrimStart,
    ),
    (
        STRING_PROTOTYPE_TRIM_RIGHT_NAME,
        NativeFunctionKind::StringPrototypeTrimEnd,
    ),
    (
        STRING_PROTOTYPE_VALUE_OF_NAME,
        NativeFunctionKind::StringPrototypeValueOf,
    ),
];
