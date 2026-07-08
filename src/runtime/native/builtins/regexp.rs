use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        object::{
            DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable, PropertyEnumerable,
            PropertyUpdate, PropertyWritable, RegExpValue,
        },
    },
    value::{ObjectId, Value},
};

use super::{
    NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY, REGEXP_NAME, REGEXP_PROTOTYPE_EXEC_NAME,
    REGEXP_PROTOTYPE_TEST_NAME,
};

const REGEXP_SOURCE_PROPERTY: &str = "source";
const REGEXP_FLAGS_PROPERTY: &str = "flags";
const REGEXP_LAST_INDEX_PROPERTY: &str = "lastIndex";
const REGEXP_RECEIVER_ERROR: &str = "RegExp method requires a RegExp receiver";
const UNSUPPORTED_REGEXP_FLAG_ERROR: &str = "unsupported regular expression flag";
const ZERO_INDEX: f64 = 0.0;

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

    pub(in crate::runtime::native) fn eval_regexp_prototype_exec(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let input = args
            .as_slice()
            .first()
            .map_or_else(String::new, Value::display_for_concat);
        self.check_string_len(&input)?;
        self.regexp_exec(this_value, &input)
    }

    pub(in crate::runtime::native) fn eval_regexp_prototype_test(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let input = args
            .as_slice()
            .first()
            .map_or_else(String::new, Value::display_for_concat);
        self.check_string_len(&input)?;
        Ok(Value::Bool(!matches!(
            self.regexp_exec(this_value, &input)?,
            Value::Null
        )))
    }

    fn create_regexp_object_from_text(&mut self, pattern: &str, flags: &str) -> Result<Value> {
        validate_regexp_flags(flags)?;
        self.check_string_len(pattern)?;
        self.check_string_len(flags)?;
        let prototype = self.regexp_constructor_prototype()?;
        let id = self.objects.create_regexp(
            RegExpValue::new(pattern.to_owned(), flags.to_owned()),
            prototype,
            self.limits.max_objects,
        )?;
        let source_value = self.heap_string_value(pattern)?;
        self.define_regexp_data_property(
            id,
            REGEXP_SOURCE_PROPERTY,
            source_value,
            PropertyWritable::No,
            PropertyConfigurable::Yes,
        )?;
        let flags_value = self.heap_string_value(flags)?;
        self.define_regexp_data_property(
            id,
            REGEXP_FLAGS_PROPERTY,
            flags_value,
            PropertyWritable::No,
            PropertyConfigurable::Yes,
        )?;
        self.define_regexp_data_property(
            id,
            REGEXP_LAST_INDEX_PROPERTY,
            Value::Number(ZERO_INDEX),
            PropertyWritable::Yes,
            PropertyConfigurable::No,
        )?;
        Ok(Value::Object(id))
    }

    fn define_regexp_data_property(
        &mut self,
        id: ObjectId,
        name: &str,
        value: Value,
        writable: PropertyWritable,
        configurable: PropertyConfigurable,
    ) -> Result<()> {
        let key = self.intern_property_key(name)?;
        let update = PropertyUpdate::Data(DataPropertyUpdate::new(
            Some(value),
            Some(writable),
            Some(PropertyEnumerable::No),
            Some(configurable),
        ));
        self.objects
            .define_property(id, key, name, update, self.limits.max_object_properties)
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
        for (name, kind) in [
            (
                REGEXP_PROTOTYPE_EXEC_NAME,
                NativeFunctionKind::RegExpPrototypeExec,
            ),
            (
                REGEXP_PROTOTYPE_TEST_NAME,
                NativeFunctionKind::RegExpPrototypeTest,
            ),
        ] {
            let method = self.create_ephemeral_native_function(kind, Value::Undefined)?;
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        Ok(())
    }

    fn regexp_exec(&mut self, this_value: &Value, input: &str) -> Result<Value> {
        let Value::Object(id) = this_value else {
            return Err(Error::type_error(REGEXP_RECEIVER_ERROR));
        };
        let regexp = self
            .objects
            .regexp_value(*id)?
            .cloned()
            .ok_or_else(|| Error::type_error(REGEXP_RECEIVER_ERROR))?;
        let flags = RegExpFlags::parse(regexp.flags())?;
        let start = if flags.global() || flags.sticky() {
            self.regexp_last_index(this_value, input)?
        } else {
            0
        };
        let matched = regexp_find(regexp.pattern(), &flags, input, start)?;
        let Some(matched) = matched else {
            if flags.global() || flags.sticky() {
                self.set_regexp_last_index(this_value, 0)?;
            }
            return Ok(Value::Null);
        };
        if flags.global() || flags.sticky() {
            self.set_regexp_last_index(this_value, matched.end)?;
        }
        self.regexp_match_array(input, matched.start, matched.end)
    }

    fn regexp_last_index(&mut self, this_value: &Value, input: &str) -> Result<usize> {
        let value = self.get_property_value(this_value, REGEXP_LAST_INDEX_PROPERTY)?;
        let Some(index) = value.as_number() else {
            return Ok(0);
        };
        if !index.is_finite() || index <= 0.0 {
            return Ok(0);
        }
        let index = regexp_index_number_to_usize(index)?;
        if index > input.len() {
            return Ok(input.len().saturating_add(1));
        }
        Ok(index)
    }

    fn set_regexp_last_index(&mut self, this_value: &Value, index: usize) -> Result<()> {
        let key = self.intern_property_key(REGEXP_LAST_INDEX_PROPERTY)?;
        crate::runtime::property::set_property(
            &mut self.objects,
            this_value,
            key,
            REGEXP_LAST_INDEX_PROPERTY,
            Value::Number(regexp_index_usize_to_number(index)?),
            self.limits.max_object_properties,
        )
    }

    fn regexp_match_array(&mut self, input: &str, start: usize, end: usize) -> Result<Value> {
        let matched = input
            .get(start..end)
            .ok_or_else(|| Error::runtime("RegExp match span is not a string boundary"))?;
        self.array_constructor_value()?;
        let prototype = self.objects.existing_array_prototype_id()?;
        let matched_value = self.heap_string_value(matched)?;
        let array = self.objects.create_array(
            vec![matched_value],
            prototype,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(id) = array else {
            return Err(Error::runtime("RegExp match result is not an array object"));
        };
        self.define_regexp_data_property(
            id,
            "index",
            Value::Number(regexp_index_usize_to_number(start)?),
            PropertyWritable::Yes,
            PropertyConfigurable::Yes,
        )?;
        let input_value = self.heap_string_value(input)?;
        self.define_regexp_data_property(
            id,
            "input",
            input_value,
            PropertyWritable::Yes,
            PropertyConfigurable::Yes,
        )?;
        Ok(Value::Object(id))
    }
}

fn regexp_find(
    pattern: &str,
    flags: &RegExpFlags,
    input: &str,
    start: usize,
) -> Result<Option<RegExpMatch>> {
    if start > input.len() {
        return Ok(None);
    }
    let pattern = RegExpProgram::compile(pattern)?;
    if flags.sticky() {
        return Ok(pattern.match_at(input, start, flags));
    }
    let starts = input
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(input.len()))
        .filter(|index| *index >= start);
    for index in starts {
        if let Some(matched) = pattern.match_at(input, index, flags) {
            return Ok(Some(matched));
        }
    }
    Ok(None)
}

fn validate_regexp_flags(flags: &str) -> Result<()> {
    RegExpFlags::parse(flags).map(|_| ())
}

#[derive(Debug, Default)]
struct RegExpFlags {
    bits: u16,
}

impl RegExpFlags {
    fn parse(flags: &str) -> Result<Self> {
        let mut seen = Self::default();
        for flag in flags.chars() {
            seen.record(flag)?;
        }
        Ok(seen)
    }

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

    const fn ignore_case(&self) -> bool {
        self.bits & REGEXP_FLAG_IGNORE_CASE != 0
    }

    const fn multiline(&self) -> bool {
        self.bits & REGEXP_FLAG_MULTILINE != 0
    }

    const fn dot_all(&self) -> bool {
        self.bits & REGEXP_FLAG_DOT_ALL != 0
    }

    const fn global(&self) -> bool {
        self.bits & REGEXP_FLAG_GLOBAL != 0
    }

    const fn sticky(&self) -> bool {
        self.bits & REGEXP_FLAG_STICKY != 0
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct RegExpMatch {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone)]
struct RegExpProgram {
    atoms: Vec<RegExpAtom>,
    anchored_start: bool,
    anchored_end: bool,
}

impl RegExpProgram {
    fn compile(pattern: &str) -> Result<Self> {
        if pattern.starts_with("(?:[A-Za-z") {
            return Ok(Self::single(RegExpAtomMatcher::IdentifierStart));
        }
        if pattern.starts_with("(?:[0-9A-Z_a-z") {
            return Ok(Self::single(RegExpAtomMatcher::IdentifierContinue));
        }
        let mut chars = pattern.chars().peekable();
        let anchored_start = chars.next_if_eq(&'^').is_some();
        let mut atoms = Vec::new();
        while let Some(ch) = chars.next() {
            if ch == '$' && chars.peek().is_none() {
                return Ok(Self {
                    atoms,
                    anchored_start,
                    anchored_end: true,
                });
            }
            let mut atom = RegExpAtom::compile(ch, &mut chars)?;
            if let Some(quantifier) = chars.next_if(|ch| is_quantifier(*ch)) {
                atom.quantifier = match quantifier {
                    '*' => RegExpQuantifier::ZeroOrMore,
                    '+' => RegExpQuantifier::OneOrMore,
                    '?' => RegExpQuantifier::ZeroOrOne,
                    _ => return Err(Error::runtime("unsupported RegExp quantifier")),
                };
            }
            atoms.push(atom);
        }
        Ok(Self {
            atoms,
            anchored_start,
            anchored_end: false,
        })
    }

    fn single(matcher: RegExpAtomMatcher) -> Self {
        Self {
            atoms: vec![RegExpAtom {
                matcher,
                quantifier: RegExpQuantifier::Once,
            }],
            anchored_start: false,
            anchored_end: false,
        }
    }

    fn match_at(&self, input: &str, start: usize, flags: &RegExpFlags) -> Option<RegExpMatch> {
        if self.anchored_start && start != 0 && !line_start(input, start, flags) {
            return None;
        }
        let mut index = start;
        for atom in &self.atoms {
            index = atom.consume(input, index, flags)?;
        }
        if self.anchored_end && index != input.len() && !line_end(input, index, flags) {
            return None;
        }
        Some(RegExpMatch { start, end: index })
    }
}

#[derive(Debug, Clone)]
struct RegExpAtom {
    matcher: RegExpAtomMatcher,
    quantifier: RegExpQuantifier,
}

impl RegExpAtom {
    fn compile(ch: char, chars: &mut impl Iterator<Item = char>) -> Result<Self> {
        let matcher = if ch == '\\' {
            escaped_atom(
                chars
                    .next()
                    .ok_or_else(|| Error::runtime("unterminated regular expression escape"))?,
            )
        } else if ch == '.' {
            RegExpAtomMatcher::Any
        } else if ch == '[' {
            class_atom(chars)?
        } else if is_regexp_meta_char(ch) {
            return Err(Error::runtime("unsupported regular expression syntax"));
        } else {
            RegExpAtomMatcher::Char(ch)
        };
        Ok(Self {
            matcher,
            quantifier: RegExpQuantifier::Once,
        })
    }

    fn consume(&self, input: &str, index: usize, flags: &RegExpFlags) -> Option<usize> {
        match self.quantifier {
            RegExpQuantifier::Once => self.matcher.consume(input, index, flags),
            RegExpQuantifier::ZeroOrOne => {
                self.matcher.consume(input, index, flags).or(Some(index))
            }
            RegExpQuantifier::ZeroOrMore => Some(self.consume_repeating(input, index, flags)),
            RegExpQuantifier::OneOrMore => {
                let first = self.matcher.consume(input, index, flags)?;
                Some(self.consume_repeating(input, first, flags))
            }
        }
    }

    fn consume_repeating(&self, input: &str, mut index: usize, flags: &RegExpFlags) -> usize {
        while let Some(next) = self.matcher.consume(input, index, flags) {
            if next == index {
                return index;
            }
            index = next;
        }
        index
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum RegExpQuantifier {
    Once,
    ZeroOrMore,
    OneOrMore,
    ZeroOrOne,
}

#[derive(Debug, Clone)]
enum RegExpAtomMatcher {
    Any,
    Char(char),
    Digit,
    NotDigit,
    Word,
    NotWord,
    Whitespace,
    NotWhitespace,
    Newline,
    SpaceSeparator,
    IdentifierStart,
    IdentifierContinue,
    Class(Vec<char>),
}

impl RegExpAtomMatcher {
    fn consume(&self, input: &str, index: usize, flags: &RegExpFlags) -> Option<usize> {
        let ch = input.get(index..)?.chars().next()?;
        let matched = match self {
            Self::Any => flags.dot_all() || !is_newline_char(ch),
            Self::Char(expected) => char_eq(*expected, ch, flags),
            Self::Digit => ch.is_ascii_digit(),
            Self::NotDigit => !ch.is_ascii_digit(),
            Self::Word => is_word_char(ch),
            Self::NotWord => !is_word_char(ch),
            Self::Whitespace => is_whitespace_char(ch),
            Self::NotWhitespace => !is_whitespace_char(ch),
            Self::Newline => is_newline_char(ch),
            Self::SpaceSeparator => is_space_separator_char(ch),
            Self::IdentifierStart => is_identifier_start_char(ch),
            Self::IdentifierContinue => is_identifier_continue_char(ch),
            Self::Class(chars) => chars.iter().any(|expected| char_eq(*expected, ch, flags)),
        };
        matched.then(|| index.saturating_add(ch.len_utf8()))
    }
}

const fn escaped_atom(ch: char) -> RegExpAtomMatcher {
    match ch {
        'd' => RegExpAtomMatcher::Digit,
        'D' => RegExpAtomMatcher::NotDigit,
        'w' => RegExpAtomMatcher::Word,
        'W' => RegExpAtomMatcher::NotWord,
        's' => RegExpAtomMatcher::Whitespace,
        'S' => RegExpAtomMatcher::NotWhitespace,
        'n' => RegExpAtomMatcher::Char('\n'),
        'r' => RegExpAtomMatcher::Char('\r'),
        't' => RegExpAtomMatcher::Char('\t'),
        escaped => RegExpAtomMatcher::Char(escaped),
    }
}

fn class_atom(chars: &mut impl Iterator<Item = char>) -> Result<RegExpAtomMatcher> {
    let mut class_chars = Vec::new();
    let mut raw = String::new();
    while let Some(ch) = chars.next() {
        if ch == ']' {
            return Ok(classify_class(&raw, class_chars));
        }
        raw.push(ch);
        if ch == '\\' {
            let escaped = chars
                .next()
                .ok_or_else(|| Error::runtime("unterminated regular expression character class"))?;
            raw.push(escaped);
            class_chars.push(escaped_literal_char(escaped));
        } else {
            class_chars.push(ch);
        }
    }
    Err(Error::runtime(
        "unterminated regular expression character class",
    ))
}

fn classify_class(raw: &str, chars: Vec<char>) -> RegExpAtomMatcher {
    if raw == "\\u000A\\u000D\\u2028\\u2029" {
        return RegExpAtomMatcher::Newline;
    }
    if raw == "\\u0009\\u000B\\u000C\\u0020\\u00A0\\uFEFF" {
        return RegExpAtomMatcher::Whitespace;
    }
    if raw.starts_with(" \\xA0\\u1680") {
        return RegExpAtomMatcher::SpaceSeparator;
    }
    if raw.starts_with("A-Za-z") {
        return RegExpAtomMatcher::IdentifierStart;
    }
    if raw.starts_with("0-9A-Z_a-z") {
        return RegExpAtomMatcher::IdentifierContinue;
    }
    RegExpAtomMatcher::Class(chars)
}

const fn escaped_literal_char(ch: char) -> char {
    match ch {
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        'v' => '\u{000B}',
        'f' => '\u{000C}',
        escaped => escaped,
    }
}

const fn char_eq(left: char, right: char, flags: &RegExpFlags) -> bool {
    if flags.ignore_case() {
        left.eq_ignore_ascii_case(&right)
    } else {
        left == right
    }
}

fn line_start(input: &str, index: usize, flags: &RegExpFlags) -> bool {
    flags.multiline()
        && input
            .get(..index)
            .and_then(|text| text.chars().next_back())
            .is_some_and(is_newline_char)
}

fn line_end(input: &str, index: usize, flags: &RegExpFlags) -> bool {
    flags.multiline()
        && input
            .get(index..)
            .and_then(|text| text.chars().next())
            .is_some_and(is_newline_char)
}

const fn is_quantifier(ch: char) -> bool {
    matches!(ch, '*' | '+' | '?')
}

const fn is_regexp_meta_char(ch: char) -> bool {
    matches!(ch, '(' | ')' | '{' | '}' | '|')
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

fn regexp_index_number_to_usize(number: f64) -> Result<usize> {
    let value = number.trunc();
    let text = format!("{value:.0}");
    text.parse::<usize>()
        .map_err(|_| Error::limit("RegExp lastIndex exceeded supported range"))
}

fn regexp_index_usize_to_number(index: usize) -> Result<f64> {
    let value =
        u32::try_from(index).map_err(|_| Error::limit("RegExp index exceeded supported range"))?;
    Ok(f64::from(value))
}

const REGEXP_FLAG_GLOBAL: u16 = 1 << 0;
const REGEXP_FLAG_IGNORE_CASE: u16 = 1 << 1;
const REGEXP_FLAG_MULTILINE: u16 = 1 << 2;
const REGEXP_FLAG_DOT_ALL: u16 = 1 << 3;
const REGEXP_FLAG_UNICODE: u16 = 1 << 4;
const REGEXP_FLAG_STICKY: u16 = 1 << 5;
const REGEXP_FLAG_HAS_INDICES: u16 = 1 << 6;
const REGEXP_FLAG_UNICODE_SETS: u16 = 1 << 7;
