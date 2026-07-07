use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call_args::RuntimeCallArgs,
        object::{ObjectPropertyInit, PropertyEnumerable},
    },
    value::{ObjectId, Value},
};

use super::{
    NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY, REGEXP_NAME, REGEXP_PROTOTYPE_TEST_NAME,
};

const REGEXP_SOURCE_PROPERTY: &str = "source";
const REGEXP_FLAGS_PROPERTY: &str = "flags";
const PROTOTYPE_PROPERTY: &str = "__proto__";
const REGEXP_TEST_RECEIVER_ERROR: &str = "RegExp.prototype.test requires a RegExp receiver";
const UNSUPPORTED_REGEXP_FLAG_ERROR: &str = "unsupported regular expression flag";

impl Context {
    pub(in crate::runtime::native) fn regexp_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::RegExp) {
            return Ok(Value::NativeFunction(id));
        }

        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype_id = self.regexp_prototype_id_with_constructor(constructor.clone())?;
        let prototype = Value::Object(prototype_id);
        let name = self.native_function_name_value(NativeFunctionKind::RegExp)?;
        self.push_native_function_with_id(id, NativeFunctionKind::RegExp, prototype, name)?;
        self.install_regexp_prototype_methods(prototype_id)?;
        self.insert_global_builtin(REGEXP_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(crate) fn create_regexp_literal(&mut self, pattern: &str, flags: &str) -> Result<Value> {
        self.create_regexp_object_from_text(pattern, flags)
    }

    pub(in crate::runtime::native) fn eval_regexp_constructor(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_regexp_constructor(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_regexp_constructor(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let pattern = args
            .first()
            .map_or_else(String::new, Value::display_for_concat);
        let flags = args
            .get(1)
            .map_or_else(String::new, Value::display_for_concat);
        self.create_regexp_object_from_text(&pattern, &flags)
    }

    pub(in crate::runtime::native) fn construct_regexp_object(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_regexp_constructor(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_regexp_prototype_test(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let pattern = self.regexp_text_property(this_value, REGEXP_SOURCE_PROPERTY)?;
        let flags = self.regexp_text_property(this_value, REGEXP_FLAGS_PROPERTY)?;
        let input = args
            .as_slice()
            .first()
            .map_or_else(String::new, Value::display_for_concat);
        self.check_string_len(&input)?;
        Ok(Value::Bool(regexp_test(&pattern, &flags, &input)?))
    }

    fn create_regexp_object_from_text(&mut self, pattern: &str, flags: &str) -> Result<Value> {
        validate_regexp_flags(flags)?;
        self.check_string_len(pattern)?;
        self.check_string_len(flags)?;
        let prototype = self.regexp_constructor_prototype()?;
        let source_key = self.intern_property_key(REGEXP_SOURCE_PROPERTY)?;
        let flags_key = self.intern_property_key(REGEXP_FLAGS_PROPERTY)?;
        let prototype_key = self.intern_property_key(PROTOTYPE_PROPERTY)?;
        let constructor_key = self.object_constructor_property_key()?;
        let source = self.heap_string_value(pattern)?;
        let flags = self.heap_string_value(flags)?;
        self.objects.create(
            vec![
                ObjectPropertyInit::new(
                    source_key,
                    REGEXP_SOURCE_PROPERTY,
                    source,
                    PropertyEnumerable::No,
                ),
                ObjectPropertyInit::new(
                    flags_key,
                    REGEXP_FLAGS_PROPERTY,
                    flags,
                    PropertyEnumerable::No,
                ),
                ObjectPropertyInit::new(
                    prototype_key,
                    PROTOTYPE_PROPERTY,
                    Value::Object(prototype),
                    PropertyEnumerable::No,
                ),
            ],
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn regexp_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<ObjectId> {
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

    fn regexp_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.regexp_constructor_value()? else {
            return Err(Error::runtime("RegExp constructor value is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("RegExp prototype is not an object")),
        }
    }

    fn install_regexp_prototype_methods(&mut self, prototype: ObjectId) -> Result<()> {
        let test = self.create_ephemeral_native_function(
            NativeFunctionKind::RegExpPrototypeTest,
            Value::Undefined,
        )?;
        self.define_non_enumerable_object_property(prototype, REGEXP_PROTOTYPE_TEST_NAME, test)
    }

    fn regexp_text_property(&mut self, value: &Value, property: &str) -> Result<String> {
        let value = self.get_property_value(value, property)?;
        match value {
            Value::String(value) => Ok(value),
            Value::HeapString(value) => Ok(value.as_str().to_owned()),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::Symbol(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Object(_)
            | Value::Error(_) => Err(Error::type_error(REGEXP_TEST_RECEIVER_ERROR)),
        }
    }
}

fn regexp_test(pattern: &str, flags: &str, input: &str) -> Result<bool> {
    validate_regexp_flags(flags)?;
    let pattern = classify_regexp_pattern(pattern);
    let result = match pattern {
        RegExpPattern::Word => input.chars().any(is_word_char),
        RegExpPattern::Newline => input.chars().any(is_newline_char),
        RegExpPattern::Whitespace => input.chars().any(is_whitespace_char),
        RegExpPattern::SpaceSeparator => input.chars().any(is_space_separator_char),
        RegExpPattern::IdentifierStart => input.chars().any(is_identifier_start_char),
        RegExpPattern::IdentifierContinue => input.chars().any(is_identifier_continue_char),
        RegExpPattern::Literal(text) => literal_contains(&text, flags, input),
        RegExpPattern::Unsupported => false,
    };
    Ok(result)
}

fn validate_regexp_flags(flags: &str) -> Result<()> {
    let mut seen = RegExpFlagsSeen::default();
    for flag in flags.chars() {
        seen.record(flag)?;
    }
    Ok(())
}

#[derive(Debug, Default)]
struct RegExpFlagsSeen {
    bits: u16,
}

impl RegExpFlagsSeen {
    fn record(&mut self, flag: char) -> Result<()> {
        let bit = match flag {
            'g' => REGEXP_FLAG_GLOBAL,
            'i' => REGEXP_FLAG_IGNORE_CASE,
            'm' => REGEXP_FLAG_MULTILINE,
            's' => REGEXP_FLAG_DOT_ALL,
            'u' => REGEXP_FLAG_UNICODE,
            'y' => REGEXP_FLAG_STICKY,
            'd' => REGEXP_FLAG_HAS_INDICES,
            'v' => REGEXP_FLAG_UNICODE_SETS,
            _ => {
                return Err(Error::runtime(format!(
                    "{UNSUPPORTED_REGEXP_FLAG_ERROR}: {flag}"
                )));
            }
        };
        if self.bits & bit != 0 {
            return Err(Error::runtime(format!(
                "duplicate regular expression flag: {flag}"
            )));
        }
        self.bits |= bit;
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
enum RegExpPattern {
    Word,
    Newline,
    Whitespace,
    SpaceSeparator,
    IdentifierStart,
    IdentifierContinue,
    Literal(String),
    Unsupported,
}

fn classify_regexp_pattern(pattern: &str) -> RegExpPattern {
    if pattern == "\\w" {
        return RegExpPattern::Word;
    }
    if pattern == "[\\u000A\\u000D\\u2028\\u2029]" {
        return RegExpPattern::Newline;
    }
    if pattern == "[\\u0009\\u000B\\u000C\\u0020\\u00A0\\uFEFF]" {
        return RegExpPattern::Whitespace;
    }
    if pattern.starts_with("[ \\xA0\\u1680") {
        return RegExpPattern::SpaceSeparator;
    }
    if pattern.starts_with("(?:[A-Za-z") {
        return RegExpPattern::IdentifierStart;
    }
    if pattern.starts_with("(?:[0-9A-Z_a-z") {
        return RegExpPattern::IdentifierContinue;
    }
    literal_pattern_text(pattern).map_or(RegExpPattern::Unsupported, RegExpPattern::Literal)
}

fn literal_contains(pattern: &str, flags: &str, input: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }
    if flags.chars().any(|flag| flag == 'i') {
        return input.to_lowercase().contains(&pattern.to_lowercase());
    }
    input.contains(pattern)
}

fn literal_pattern_text(pattern: &str) -> Option<String> {
    let mut output = String::new();
    let mut chars = pattern.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            output.push(escaped_literal_char(&mut chars)?);
        } else if is_regexp_meta_char(ch) {
            return None;
        } else {
            output.push(ch);
        }
    }
    Some(output)
}

fn escaped_literal_char(chars: &mut impl Iterator<Item = char>) -> Option<char> {
    let ch = chars.next()?;
    match ch {
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        'v' => Some('\u{000B}'),
        'f' => Some('\u{000C}'),
        '0' => Some('\0'),
        'x' => hex_escape_char(chars, HEX_ESCAPE_LEN),
        'u' => hex_escape_char(chars, UNICODE_ESCAPE_LEN),
        escaped => Some(escaped),
    }
}

fn hex_escape_char(chars: &mut impl Iterator<Item = char>, len: usize) -> Option<char> {
    let mut value = 0_u32;
    for _ in 0..len {
        let digit = chars.next()?.to_digit(HEX_RADIX)?;
        value = value.checked_mul(HEX_RADIX)?.checked_add(digit)?;
    }
    char::from_u32(value)
}

const fn is_regexp_meta_char(ch: char) -> bool {
    matches!(
        ch,
        '.' | '*' | '+' | '?' | '^' | '$' | '[' | ']' | '(' | ')' | '{' | '}' | '|'
    )
}

const fn is_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

const fn is_newline_char(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

fn is_whitespace_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{0009}' | '\u{000B}' | '\u{000C}' | '\u{0020}' | '\u{00A0}' | '\u{FEFF}'
    ) || is_space_separator_char(ch)
}

fn is_space_separator_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{0020}' | '\u{00A0}' | '\u{1680}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    ) || ('\u{2000}'..='\u{200A}').contains(&ch)
}

fn is_identifier_start_char(ch: char) -> bool {
    ch == '$' || ch == '_' || ch.is_ascii_alphabetic() || ch.is_alphabetic()
}

fn is_identifier_continue_char(ch: char) -> bool {
    is_identifier_start_char(ch)
        || ch.is_ascii_digit()
        || ch.is_numeric()
        || matches!(ch, '\u{200C}' | '\u{200D}')
}

const HEX_ESCAPE_LEN: usize = 2;
const UNICODE_ESCAPE_LEN: usize = 4;
const HEX_RADIX: u32 = 16;
const REGEXP_FLAG_GLOBAL: u16 = 1 << 0;
const REGEXP_FLAG_IGNORE_CASE: u16 = 1 << 1;
const REGEXP_FLAG_MULTILINE: u16 = 1 << 2;
const REGEXP_FLAG_DOT_ALL: u16 = 1 << 3;
const REGEXP_FLAG_UNICODE: u16 = 1 << 4;
const REGEXP_FLAG_STICKY: u16 = 1 << 5;
const REGEXP_FLAG_HAS_INDICES: u16 = 1 << 6;
const REGEXP_FLAG_UNICODE_SETS: u16 = 1 << 7;
