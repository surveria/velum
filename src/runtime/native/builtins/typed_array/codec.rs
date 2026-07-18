use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::to_boolean,
        object::{
            ByteBuffer, ByteBufferOrigin, DataPropertyUpdate, PropertyConfigurable,
            PropertyEnumerable, PropertyUpdate, PropertyWritable, TypedArrayElementKind,
            TypedArrayView,
        },
    },
    value::{ErrorName, ObjectId, Value},
};

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const BASE64_URL_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const HEX_ALPHABET: &[u8; 16] = b"0123456789abcdef";
const CODEC_RECEIVER_ERROR: &str = "Uint8Array codec receiver is not a Uint8Array";
const CODEC_STRING_ERROR: &str = "Uint8Array codec input must be a string";
const CODEC_OPTIONS_ERROR: &str = "Uint8Array codec options must be an object";
const BASE64_SYNTAX_ERROR: &str = "base64 input is malformed";
const HEX_SYNTAX_ERROR: &str = "hex input is malformed";
const CODEC_OUTPUT_LIMIT_ERROR: &str = "Uint8Array codec output exceeded engine limits";

#[derive(Clone, Copy)]
enum Base64Alphabet {
    Base64,
    Base64Url,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum LastChunkHandling {
    Loose,
    Strict,
    StopBeforePartial,
}

#[derive(Clone, Copy)]
enum Base64Token {
    Value(u8),
    Padding,
}

#[derive(Clone, Copy)]
enum DecodeIssue {
    Syntax(&'static str),
    Limit,
}

struct DecodeResult {
    read: usize,
    bytes: Vec<u8>,
    issue: Option<DecodeIssue>,
}

struct DecodedChunk {
    bytes: [u8; 3],
    length: usize,
    padded: bool,
}

#[derive(Clone, Copy)]
struct DecodeLimits {
    max_length: Option<usize>,
    hard_limit: usize,
}

impl Context {
    pub(super) fn eval_uint8_array_from_base64(&mut self, args: &[Value]) -> Result<Value> {
        let input = codec_string_units(args.first().unwrap_or(&Value::Undefined))?;
        let (alphabet, handling) = self.base64_decode_options(args.get(1))?;
        let decoded = decode_base64(
            &input,
            alphabet,
            handling,
            None,
            self.limits.max_object_properties,
        );
        if let Some(issue) = decoded.issue {
            return Err(decode_issue_error(issue));
        }
        self.create_uint8_array_from_bytes(decoded.bytes)
    }

    pub(super) fn eval_uint8_array_from_hex(&mut self, args: &[Value]) -> Result<Value> {
        let input = codec_string_units(args.first().unwrap_or(&Value::Undefined))?;
        let decoded = decode_hex(&input, None, self.limits.max_object_properties);
        if let Some(issue) = decoded.issue {
            return Err(decode_issue_error(issue));
        }
        self.create_uint8_array_from_bytes(decoded.bytes)
    }

    pub(super) fn eval_uint8_array_set_from_base64(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let view = self.uint8_array_receiver(this_value)?;
        view.ensure_mutable()?;
        let input = codec_string_units(args.first().unwrap_or(&Value::Undefined))?;
        let (alphabet, handling) = self.base64_decode_options(args.get(1))?;
        if view.is_out_of_bounds() {
            return Err(Error::type_error(CODEC_RECEIVER_ERROR));
        }
        let decoded = decode_base64(
            &input,
            alphabet,
            handling,
            Some(view.length()),
            view.length(),
        );
        write_decoded_bytes(&view, &decoded.bytes)?;
        if let Some(issue) = decoded.issue {
            return Err(decode_issue_error(issue));
        }
        self.create_codec_result(decoded.read, decoded.bytes.len())
    }

    pub(super) fn eval_uint8_array_set_from_hex(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let view = self.uint8_array_receiver(this_value)?;
        view.ensure_mutable()?;
        let input = codec_string_units(args.first().unwrap_or(&Value::Undefined))?;
        let decoded = decode_hex(&input, Some(view.length()), view.length());
        write_decoded_bytes(&view, &decoded.bytes)?;
        if let Some(issue) = decoded.issue {
            return Err(decode_issue_error(issue));
        }
        self.create_codec_result(decoded.read, decoded.bytes.len())
    }

    pub(super) fn eval_uint8_array_to_base64(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let view = self.uint8_array_branded_receiver(this_value)?;
        let (alphabet, omit_padding) = self.base64_encode_options(args.first())?;
        if view.is_out_of_bounds() {
            return Err(Error::type_error(CODEC_RECEIVER_ERROR));
        }
        let bytes = copy_view_bytes(&view)?;
        let encoded = encode_base64(&bytes, alphabet, omit_padding, self.limits.max_string_len)?;
        self.heap_string_value(&encoded)
    }

    pub(super) fn eval_uint8_array_to_hex(&mut self, this_value: &Value) -> Result<Value> {
        let view = self.uint8_array_receiver(this_value)?;
        let bytes = copy_view_bytes(&view)?;
        let encoded = encode_hex(&bytes, self.limits.max_string_len)?;
        self.heap_string_value(&encoded)
    }

    fn create_uint8_array_from_bytes(&mut self, bytes: Vec<u8>) -> Result<Value> {
        self.check_byte_buffer_length(bytes.len())?;
        self.create_typed_array_with_buffer(
            TypedArrayElementKind::Uint8,
            ByteBuffer::from_bytes(bytes, ByteBufferOrigin::EngineOwned),
        )
    }

    fn uint8_array_branded_receiver(&self, value: &Value) -> Result<TypedArrayView> {
        let (_, view) = self.typed_array_branded_receiver(value)?;
        if view.element_kind() != TypedArrayElementKind::Uint8 {
            return Err(Error::type_error(CODEC_RECEIVER_ERROR));
        }
        Ok(view)
    }

    fn uint8_array_receiver(&self, value: &Value) -> Result<TypedArrayView> {
        let (_, view) = self.typed_array_receiver(value)?;
        if view.element_kind() != TypedArrayElementKind::Uint8 {
            return Err(Error::type_error(CODEC_RECEIVER_ERROR));
        }
        Ok(view)
    }

    fn base64_decode_options(
        &mut self,
        options: Option<&Value>,
    ) -> Result<(Base64Alphabet, LastChunkHandling)> {
        let Some(options) = self.codec_options_object(options)? else {
            return Ok((Base64Alphabet::Base64, LastChunkHandling::Loose));
        };
        let alphabet = self.get_named(options, "alphabet")?;
        let alphabet = parse_alphabet(&alphabet)?;
        let handling = self.get_named(options, "lastChunkHandling")?;
        let handling = parse_last_chunk_handling(&handling)?;
        Ok((alphabet, handling))
    }

    fn base64_encode_options(&mut self, options: Option<&Value>) -> Result<(Base64Alphabet, bool)> {
        let Some(options) = self.codec_options_object(options)? else {
            return Ok((Base64Alphabet::Base64, false));
        };
        let alphabet = self.get_named(options, "alphabet")?;
        let alphabet = parse_alphabet(&alphabet)?;
        let omit_padding = self.get_named(options, "omitPadding")?;
        let omit_padding = if matches!(omit_padding, Value::Undefined) {
            false
        } else {
            to_boolean(self, &omit_padding)?
        };
        Ok((alphabet, omit_padding))
    }

    fn codec_options_object<'value>(
        &self,
        options: Option<&'value Value>,
    ) -> Result<Option<&'value Value>> {
        let Some(options) = options.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(None);
        };
        if self.semantic_object_ref(options)?.is_none() {
            return Err(Error::type_error(CODEC_OPTIONS_ERROR));
        }
        Ok(Some(options))
    }

    fn create_codec_result(&mut self, read: usize, written: usize) -> Result<Value> {
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype_id(
            None,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        self.define_codec_result_property(object, "read", read)?;
        self.define_codec_result_property(object, "written", written)?;
        Ok(Value::Object(object))
    }

    fn define_codec_result_property(
        &mut self,
        object: ObjectId,
        name: &str,
        value: usize,
    ) -> Result<()> {
        let value = u32::try_from(value)
            .map(f64::from)
            .map(Value::Number)
            .map_err(|_| Error::limit(CODEC_OUTPUT_LIMIT_ERROR))?;
        let key = self.intern_property_key(name)?;
        self.objects.define_property(
            object,
            key,
            name,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::Yes),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }
}

fn codec_string_units(value: &Value) -> Result<Vec<u16>> {
    match value {
        Value::String(value) => Ok(value.as_utf16().to_vec()),
        _ => Err(Error::type_error(CODEC_STRING_ERROR)),
    }
}

fn parse_alphabet(value: &Value) -> Result<Base64Alphabet> {
    if matches!(value, Value::Undefined) || codec_option_text(value) == Some("base64") {
        return Ok(Base64Alphabet::Base64);
    }
    if codec_option_text(value) == Some("base64url") {
        return Ok(Base64Alphabet::Base64Url);
    }
    Err(Error::type_error(
        "Uint8Array base64 alphabet option is invalid",
    ))
}

fn parse_last_chunk_handling(value: &Value) -> Result<LastChunkHandling> {
    if matches!(value, Value::Undefined) || codec_option_text(value) == Some("loose") {
        return Ok(LastChunkHandling::Loose);
    }
    if codec_option_text(value) == Some("strict") {
        return Ok(LastChunkHandling::Strict);
    }
    if codec_option_text(value) == Some("stop-before-partial") {
        return Ok(LastChunkHandling::StopBeforePartial);
    }
    Err(Error::type_error(
        "Uint8Array base64 lastChunkHandling option is invalid",
    ))
}

fn codec_option_text(value: &Value) -> Option<&str> {
    match value {
        Value::String(value) => value.as_utf8(),
        _ => None,
    }
}

fn decode_base64(
    input: &[u16],
    alphabet: Base64Alphabet,
    handling: LastChunkHandling,
    max_length: Option<usize>,
    hard_limit: usize,
) -> DecodeResult {
    if max_length == Some(0) {
        return decoded_success(0, Vec::new());
    }
    let capacity = input
        .len()
        .saturating_mul(3)
        .checked_div(4)
        .unwrap_or(0)
        .min(max_length.unwrap_or(hard_limit))
        .min(hard_limit);
    let mut bytes = Vec::with_capacity(capacity);
    let mut chunk = Vec::with_capacity(4);
    let mut cursor = 0_usize;
    let mut committed_read = 0_usize;
    while cursor < input.len() {
        let Some(unit) = input.get(cursor).copied() else {
            return decoded_issue(
                committed_read,
                bytes,
                DecodeIssue::Syntax(BASE64_SYNTAX_ERROR),
            );
        };
        cursor = cursor.saturating_add(1);
        if is_base64_whitespace(unit) {
            continue;
        }
        let Some(token) = base64_token(unit, alphabet) else {
            return decoded_issue(
                committed_read,
                bytes,
                DecodeIssue::Syntax(BASE64_SYNTAX_ERROR),
            );
        };
        chunk.push(token);
        if chunk.len() != 4 {
            continue;
        }
        let Ok(decoded) = decode_complete_base64_chunk(&chunk, handling) else {
            return decoded_issue(
                committed_read,
                bytes,
                DecodeIssue::Syntax(BASE64_SYNTAX_ERROR),
            );
        };
        if !decoded_chunk_fits(bytes.len(), decoded.length, max_length) {
            return decoded_success(committed_read, bytes);
        }
        let reaches_capacity = max_length.is_some_and(|maximum| {
            bytes
                .len()
                .checked_add(decoded.length)
                .is_some_and(|length| length == maximum)
        });
        if decoded.padded && !reaches_capacity && !padded_base64_tail_is_whitespace(input, cursor) {
            return decoded_issue(
                committed_read,
                bytes,
                DecodeIssue::Syntax(BASE64_SYNTAX_ERROR),
            );
        }
        if append_decoded_chunk(&mut bytes, &decoded, hard_limit).is_err() {
            return decoded_issue(committed_read, bytes, DecodeIssue::Limit);
        }
        committed_read = if decoded.padded && !reaches_capacity {
            input.len()
        } else {
            cursor
        };
        chunk.clear();
        if max_length == Some(bytes.len()) {
            return decoded_success(committed_read, bytes);
        }
        if decoded.padded {
            return decoded_success(committed_read, bytes);
        }
    }
    finish_partial_base64(
        &chunk,
        handling,
        (cursor, committed_read),
        bytes,
        DecodeLimits {
            max_length,
            hard_limit,
        },
    )
}

fn padded_base64_tail_is_whitespace(input: &[u16], mut cursor: usize) -> bool {
    while cursor < input.len() {
        let Some(unit) = input.get(cursor).copied() else {
            return false;
        };
        cursor = cursor.saturating_add(1);
        if !is_base64_whitespace(unit) {
            return false;
        }
    }
    true
}

fn finish_partial_base64(
    chunk: &[Base64Token],
    handling: LastChunkHandling,
    positions: (usize, usize),
    mut bytes: Vec<u8>,
    limits: DecodeLimits,
) -> DecodeResult {
    let (cursor, committed_read) = positions;
    if chunk.is_empty() {
        return decoded_success(cursor, bytes);
    }
    if handling == LastChunkHandling::StopBeforePartial {
        return match chunk {
            [Base64Token::Value(_)]
            | [Base64Token::Value(_), Base64Token::Value(_)]
            | [
                Base64Token::Value(_),
                Base64Token::Value(_),
                Base64Token::Value(_) | Base64Token::Padding,
            ] => decoded_success(committed_read, bytes),
            _ => decoded_issue(
                committed_read,
                bytes,
                DecodeIssue::Syntax(BASE64_SYNTAX_ERROR),
            ),
        };
    }
    if handling == LastChunkHandling::Strict {
        return decoded_issue(
            committed_read,
            bytes,
            DecodeIssue::Syntax(BASE64_SYNTAX_ERROR),
        );
    }
    let decoded = match chunk {
        [Base64Token::Value(first), Base64Token::Value(second)] => DecodedChunk {
            bytes: [(*first << 2) | (*second >> 4), 0, 0],
            length: 1,
            padded: false,
        },
        [
            Base64Token::Value(first),
            Base64Token::Value(second),
            Base64Token::Value(third),
        ] => DecodedChunk {
            bytes: [
                (*first << 2) | (*second >> 4),
                (*second << 4) | (*third >> 2),
                0,
            ],
            length: 2,
            padded: false,
        },
        _ => {
            return decoded_issue(
                committed_read,
                bytes,
                DecodeIssue::Syntax(BASE64_SYNTAX_ERROR),
            );
        }
    };
    if !decoded_chunk_fits(bytes.len(), decoded.length, limits.max_length) {
        return decoded_success(committed_read, bytes);
    }
    if append_decoded_chunk(&mut bytes, &decoded, limits.hard_limit).is_err() {
        return decoded_issue(committed_read, bytes, DecodeIssue::Limit);
    }
    decoded_success(cursor, bytes)
}

fn decode_complete_base64_chunk(
    chunk: &[Base64Token],
    handling: LastChunkHandling,
) -> core::result::Result<DecodedChunk, ()> {
    match chunk {
        [
            Base64Token::Value(first),
            Base64Token::Value(second),
            Base64Token::Value(third),
            Base64Token::Value(fourth),
        ] => Ok(DecodedChunk {
            bytes: [
                (*first << 2) | (*second >> 4),
                (*second << 4) | (*third >> 2),
                (*third << 6) | *fourth,
            ],
            length: 3,
            padded: false,
        }),
        [
            Base64Token::Value(first),
            Base64Token::Value(second),
            Base64Token::Padding,
            Base64Token::Padding,
        ] if handling != LastChunkHandling::Strict || second.trailing_zeros() >= 4 => {
            Ok(DecodedChunk {
                bytes: [(*first << 2) | (*second >> 4), 0, 0],
                length: 1,
                padded: true,
            })
        }
        [
            Base64Token::Value(first),
            Base64Token::Value(second),
            Base64Token::Value(third),
            Base64Token::Padding,
        ] if handling != LastChunkHandling::Strict || third.trailing_zeros() >= 2 => {
            Ok(DecodedChunk {
                bytes: [
                    (*first << 2) | (*second >> 4),
                    (*second << 4) | (*third >> 2),
                    0,
                ],
                length: 2,
                padded: true,
            })
        }
        _ => Err(()),
    }
}

fn append_decoded_chunk(
    output: &mut Vec<u8>,
    decoded: &DecodedChunk,
    hard_limit: usize,
) -> core::result::Result<(), ()> {
    let length = output.len().checked_add(decoded.length).ok_or(())?;
    if length > hard_limit {
        return Err(());
    }
    output.extend(decoded.bytes.iter().take(decoded.length).copied());
    Ok(())
}

fn decoded_chunk_fits(current: usize, additional: usize, max_length: Option<usize>) -> bool {
    let Some(maximum) = max_length else {
        return true;
    };
    current
        .checked_add(additional)
        .is_some_and(|length| length <= maximum)
}

fn base64_token(unit: u16, alphabet: Base64Alphabet) -> Option<Base64Token> {
    if unit == u16::from(b'=') {
        return Some(Base64Token::Padding);
    }
    let byte = u8::try_from(unit).ok()?;
    let alphabet = match alphabet {
        Base64Alphabet::Base64 => BASE64_ALPHABET,
        Base64Alphabet::Base64Url => BASE64_URL_ALPHABET,
    };
    alphabet
        .iter()
        .position(|candidate| *candidate == byte)
        .and_then(|index| u8::try_from(index).ok())
        .map(Base64Token::Value)
}

const fn is_base64_whitespace(unit: u16) -> bool {
    matches!(unit, 0x0009 | 0x000A | 0x000C | 0x000D | 0x0020)
}

fn decode_hex(input: &[u16], max_length: Option<usize>, hard_limit: usize) -> DecodeResult {
    if !input.len().is_multiple_of(2) {
        return decoded_issue(0, Vec::new(), DecodeIssue::Syntax(HEX_SYNTAX_ERROR));
    }
    let capacity = input
        .len()
        .checked_div(2)
        .unwrap_or(0)
        .min(max_length.unwrap_or(hard_limit))
        .min(hard_limit);
    let mut bytes = Vec::with_capacity(capacity);
    let mut cursor = 0_usize;
    while cursor < input.len() {
        if max_length == Some(bytes.len()) {
            return decoded_success(cursor, bytes);
        }
        let Some(high) = input.get(cursor).copied().and_then(hex_nibble) else {
            return decoded_issue(cursor, bytes, DecodeIssue::Syntax(HEX_SYNTAX_ERROR));
        };
        let low_index = cursor.saturating_add(1);
        let Some(low) = input.get(low_index).copied().and_then(hex_nibble) else {
            return decoded_issue(cursor, bytes, DecodeIssue::Syntax(HEX_SYNTAX_ERROR));
        };
        if bytes.len() >= hard_limit {
            return decoded_issue(cursor, bytes, DecodeIssue::Limit);
        }
        bytes.push((high << 4) | low);
        cursor = cursor.saturating_add(2);
    }
    decoded_success(cursor, bytes)
}

fn hex_nibble(unit: u16) -> Option<u8> {
    match unit {
        0x30..=0x39 => u8::try_from(unit - 0x30).ok(),
        0x41..=0x46 => u8::try_from(unit - 0x41 + 10).ok(),
        0x61..=0x66 => u8::try_from(unit - 0x61 + 10).ok(),
        _ => None,
    }
}

fn encode_base64(
    bytes: &[u8],
    alphabet: Base64Alphabet,
    omit_padding: bool,
    max_length: usize,
) -> Result<String> {
    let complete = bytes.len().checked_div(3).unwrap_or(0);
    let remainder = bytes.len() % 3;
    let tail = if remainder == 0 {
        0
    } else if omit_padding {
        remainder.saturating_add(1)
    } else {
        4
    };
    let output_length = complete
        .checked_mul(4)
        .and_then(|length| length.checked_add(tail))
        .ok_or_else(|| Error::limit(CODEC_OUTPUT_LIMIT_ERROR))?;
    if output_length > max_length {
        return Err(Error::limit(CODEC_OUTPUT_LIMIT_ERROR));
    }
    let alphabet = match alphabet {
        Base64Alphabet::Base64 => BASE64_ALPHABET,
        Base64Alphabet::Base64Url => BASE64_URL_ALPHABET,
    };
    let mut output = String::with_capacity(output_length);
    for chunk in bytes.chunks(3) {
        let first = chunk.first().copied().unwrap_or(0);
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        push_alphabet(&mut output, alphabet, usize::from(first >> 2))?;
        push_alphabet(
            &mut output,
            alphabet,
            usize::from(((first & 0x03) << 4) | (second >> 4)),
        )?;
        if chunk.len() > 1 {
            push_alphabet(
                &mut output,
                alphabet,
                usize::from(((second & 0x0F) << 2) | (third >> 6)),
            )?;
        } else if !omit_padding {
            output.push('=');
        }
        if chunk.len() > 2 {
            push_alphabet(&mut output, alphabet, usize::from(third & 0x3F))?;
        } else if !omit_padding {
            output.push('=');
        }
    }
    Ok(output)
}

fn push_alphabet(output: &mut String, alphabet: &[u8; 64], index: usize) -> Result<()> {
    let byte = alphabet
        .get(index)
        .copied()
        .ok_or_else(|| Error::runtime("base64 alphabet index is out of range"))?;
    output.push(char::from(byte));
    Ok(())
}

fn encode_hex(bytes: &[u8], max_length: usize) -> Result<String> {
    let output_length = bytes
        .len()
        .checked_mul(2)
        .ok_or_else(|| Error::limit(CODEC_OUTPUT_LIMIT_ERROR))?;
    if output_length > max_length {
        return Err(Error::limit(CODEC_OUTPUT_LIMIT_ERROR));
    }
    let mut output = String::with_capacity(output_length);
    for byte in bytes {
        push_hex(&mut output, usize::from(byte >> 4))?;
        push_hex(&mut output, usize::from(byte & 0x0F))?;
    }
    Ok(output)
}

fn push_hex(output: &mut String, index: usize) -> Result<()> {
    let byte = HEX_ALPHABET
        .get(index)
        .copied()
        .ok_or_else(|| Error::runtime("hex alphabet index is out of range"))?;
    output.push(char::from(byte));
    Ok(())
}

fn write_decoded_bytes(view: &TypedArrayView, bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    view.buffer().write(view.byte_offset(), bytes)
}

fn copy_view_bytes(view: &TypedArrayView) -> Result<Vec<u8>> {
    let start = view.byte_offset();
    let end = start
        .checked_add(view.byte_length()?)
        .ok_or_else(|| Error::limit(CODEC_OUTPUT_LIMIT_ERROR))?;
    view.buffer().copy_bytes(start, end)
}

const fn decoded_success(read: usize, bytes: Vec<u8>) -> DecodeResult {
    DecodeResult {
        read,
        bytes,
        issue: None,
    }
}

const fn decoded_issue(read: usize, bytes: Vec<u8>, issue: DecodeIssue) -> DecodeResult {
    DecodeResult {
        read,
        bytes,
        issue: Some(issue),
    }
}

fn decode_issue_error(issue: DecodeIssue) -> Error {
    match issue {
        DecodeIssue::Syntax(message) => Error::exception(ErrorName::SyntaxError, message),
        DecodeIssue::Limit => Error::limit(CODEC_OUTPUT_LIMIT_ERROR),
    }
}
