use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        numeric::number_to_uint32,
        object::{PropertyKey, PropertyLookup},
    },
    value::Value,
};

const REGEXP_LAST_INDEX_PROPERTY: &str = "lastIndex";
const REGEXP_MATCH_INDEX_PROPERTY: &str = "index";
const REGEXP_MATCH_GROUPS_PROPERTY: &str = "groups";
const REGEXP_MATCH_LENGTH_PROPERTY: &str = "length";
const FIRST_MATCH_PROPERTY: &str = "0";
const SPLIT_SYMBOL_DISPLAY: &str = "[Symbol.split]";
const SPLIT_SYMBOL_PROPERTY: &str = "split";
const ZERO_INDEX: f64 = 0.0;

#[derive(Debug)]
struct StringRegExpMatch {
    byte_start: usize,
    byte_end: usize,
    code_unit_start: usize,
    code_unit_end: usize,
    text: String,
    captures: Vec<Value>,
    groups: Value,
}

impl Context {
    pub(in crate::runtime::native) fn eval_string_prototype_match(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_value(this_value)?;
        let Some(pattern) = args.as_slice().first() else {
            return self.string_match_plain(&text, "");
        };
        if !self.string_regexp_is_object(pattern)? {
            let pattern = self.string_argument_text(pattern)?;
            return self.string_match_plain(&text, &pattern);
        }
        if !self.string_regexp_is_global(pattern)? {
            return self.regexp_exec(pattern, &text);
        }
        self.string_regexp_set_last_index(pattern, 0)?;
        let mut matches = Vec::new();
        while let Some(matched) = self.string_regexp_exec_match(pattern, &text)? {
            let match_end = matched.code_unit_end;
            let is_empty_match = matched.text.is_empty();
            matches.push(matched.text);
            if matches.len() > self.limits.max_object_properties {
                return Err(Error::limit(
                    "String.prototype.match result exceeded array limit",
                ));
            }
            if is_empty_match {
                self.string_regexp_advance_last_index(pattern, &text, match_end)?;
            }
        }
        if matches.is_empty() {
            return Ok(Value::Null);
        }
        self.string_values_array(matches)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_search(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_value(this_value)?;
        let Some(pattern) = args.as_slice().first() else {
            return Ok(Value::Number(ZERO_INDEX));
        };
        if !self.string_regexp_is_object(pattern)? {
            return Ok(Value::Number(optional_index_to_number(
                text.find(&self.string_argument_text(pattern)?),
            )?));
        }
        let previous = self.get_named(pattern, REGEXP_LAST_INDEX_PROPERTY)?;
        self.string_regexp_set_last_index(pattern, 0)?;
        let matched = self.string_regexp_exec_match(pattern, &text)?;
        self.string_regexp_set_last_index_value(pattern, previous)?;
        Ok(Value::Number(match matched {
            Some(matched) => index_to_number(matched.code_unit_start)?,
            None => -1.0,
        }))
    }

    pub(in crate::runtime::native) fn eval_string_prototype_replace(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let text = self.string_receiver_value(this_value)?;
        let pattern = args.as_slice().first();
        let replacement = if let Some(value) = args.as_slice().get(1) {
            self.string_argument_text(value)?
        } else {
            String::new()
        };
        let Some(pattern) = pattern else {
            return self.heap_string_value(&text);
        };
        if !self.string_regexp_is_object(pattern)? {
            let needle = self.string_argument_text(pattern)?;
            let output = replace_first_plain(&text, &needle, &replacement)?;
            self.check_string_len(&output)?;
            return self.heap_string_value(&output);
        }
        if self.string_regexp_is_global(pattern)? {
            self.string_regexp_replace_global(&text, pattern, &replacement)
        } else {
            self.string_regexp_replace_first(&text, pattern, &replacement)
        }
    }

    pub(in crate::runtime::native) fn eval_string_prototype_split(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if matches!(this_value, Value::Undefined | Value::Null) {
            return Err(Error::type_error(
                "String.prototype.split requires a non-nullish receiver",
            ));
        }
        let separator = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let limit_value = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if self.semantic_object_ref(&separator)?.is_some()
            && let Some(splitter) = self.string_split_method(&separator)?
        {
            return self.call_value(&splitter, &[this_value.clone(), limit_value], separator);
        }

        let text = self.string_receiver_utf16(this_value)?;
        let limit = self.string_split_limit(args.as_slice().get(1))?;
        if matches!(separator, Value::Undefined) {
            if limit == 0 {
                return self.string_utf16_values_array(Vec::new());
            }
            return self.string_utf16_values_array(vec![text]);
        }
        let separator = self.string_argument_utf16(&separator)?;
        if limit == 0 {
            return self.string_utf16_values_array(Vec::new());
        }
        self.string_utf16_values_array(split_plain_utf16(&text, &separator, limit)?)
    }

    fn string_split_method(&mut self, separator: &Value) -> Result<Option<Value>> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let symbol = self.get_named(&symbol_constructor, SPLIT_SYMBOL_PROPERTY)?;
        let Value::Symbol(symbol) = symbol else {
            return Err(Error::runtime("Symbol.split is not initialized"));
        };
        self.get_method(
            separator,
            PropertyLookup::from_key(SPLIT_SYMBOL_DISPLAY, PropertyKey::symbol(symbol.id())),
        )
    }

    fn string_regexp_is_object(&self, value: &Value) -> Result<bool> {
        let Value::Object(id) = value else {
            return Ok(false);
        };
        self.objects
            .regexp_value(*id)
            .map(|regexp| regexp.is_some())
    }

    fn string_regexp_is_global(&self, value: &Value) -> Result<bool> {
        let Value::Object(id) = value else {
            return Ok(false);
        };
        Ok(self
            .objects
            .regexp_value(*id)?
            .is_some_and(|regexp| regexp.flags().contains('g')))
    }

    fn string_regexp_exec_match(
        &mut self,
        pattern: &Value,
        text: &str,
    ) -> Result<Option<StringRegExpMatch>> {
        let result = self.regexp_exec(pattern, text)?;
        self.string_regexp_match_from_value(text, &result)
    }

    fn string_match_plain(&mut self, text: &str, needle: &str) -> Result<Value> {
        if !needle.is_empty() && !text.contains(needle) {
            return Ok(Value::Null);
        }
        self.string_values_array(vec![needle.to_owned()])
    }

    pub(in crate::runtime::native) fn string_regexp_replace_first(
        &mut self,
        text: &str,
        pattern: &Value,
        replacement: &str,
    ) -> Result<Value> {
        let Some(matched) = self.string_regexp_exec_match(pattern, text)? else {
            return self.heap_string_value(text);
        };
        let output = replace_span(text, matched.byte_start, matched.byte_end, replacement)?;
        self.check_string_len(&output)?;
        self.heap_string_value(&output)
    }

    pub(in crate::runtime::native) fn string_regexp_replace_global(
        &mut self,
        text: &str,
        pattern: &Value,
        replacement: &str,
    ) -> Result<Value> {
        self.string_regexp_set_last_index(pattern, 0)?;
        let mut output = String::new();
        let mut cursor = 0usize;
        while let Some(matched) = self.string_regexp_exec_match(pattern, text)? {
            if matched.byte_start < cursor {
                return Err(Error::runtime("RegExp global match moved backwards"));
            }
            push_checked_slice(&mut output, text, cursor, matched.byte_start)?;
            output.push_str(replacement);
            cursor = matched.byte_end;
            self.check_string_len(&output)?;
            if matched.text.is_empty() {
                self.string_regexp_advance_last_index(pattern, text, matched.code_unit_end)?;
            }
        }
        push_checked_slice(&mut output, text, cursor, text.len())?;
        self.check_string_len(&output)?;
        self.heap_string_value(&output)
    }

    pub(in crate::runtime::native) fn string_regexp_replace_first_value(
        &mut self,
        text: &str,
        pattern: &Value,
        replacement: &Value,
    ) -> Result<Value> {
        let Some(matched) = self.string_regexp_exec_match(pattern, text)? else {
            return self.heap_string_value(text);
        };
        let substitution = self.regexp_replacement_text(text, &matched, replacement)?;
        let output = replace_span(text, matched.byte_start, matched.byte_end, &substitution)?;
        self.check_string_len(&output)?;
        self.heap_string_value(&output)
    }

    pub(in crate::runtime::native) fn string_regexp_replace_global_value(
        &mut self,
        text: &str,
        pattern: &Value,
        replacement: &Value,
    ) -> Result<Value> {
        self.string_regexp_set_last_index(pattern, 0)?;
        let mut output = String::new();
        let mut cursor = 0usize;
        while let Some(matched) = self.string_regexp_exec_match(pattern, text)? {
            if matched.byte_start < cursor {
                return Err(Error::runtime("RegExp global match moved backwards"));
            }
            push_checked_slice(&mut output, text, cursor, matched.byte_start)?;
            let substitution = self.regexp_replacement_text(text, &matched, replacement)?;
            output.push_str(&substitution);
            cursor = matched.byte_end;
            self.check_string_len(&output)?;
            if matched.text.is_empty() {
                self.string_regexp_advance_last_index(pattern, text, matched.code_unit_end)?;
            }
        }
        push_checked_slice(&mut output, text, cursor, text.len())?;
        self.check_string_len(&output)?;
        self.heap_string_value(&output)
    }

    fn regexp_replacement_text(
        &mut self,
        text: &str,
        matched: &StringRegExpMatch,
        replacement: &Value,
    ) -> Result<String> {
        if self.semantic_is_callable(replacement)? {
            let mut args = Vec::with_capacity(matched.captures.len().saturating_add(4));
            args.push(self.heap_string_value(&matched.text)?);
            args.extend(matched.captures.iter().cloned());
            args.push(Value::Number(index_to_number(matched.code_unit_start)?));
            args.push(self.heap_string_value(text)?);
            if !matches!(matched.groups, Value::Undefined) {
                args.push(matched.groups.clone());
            }
            let value = self.call_value(replacement, &args, Value::Undefined)?;
            return self.to_string(&value);
        }
        let replacement = self.to_string(replacement)?;
        self.regexp_substitution(text, matched, &replacement)
    }

    fn regexp_substitution(
        &mut self,
        text: &str,
        matched: &StringRegExpMatch,
        replacement: &str,
    ) -> Result<String> {
        let chars = replacement.chars().collect::<Vec<_>>();
        let mut output = String::new();
        let mut index = 0usize;
        while let Some(ch) = chars.get(index).copied() {
            if ch != '$' {
                output.push(ch);
                index = index.saturating_add(1);
                continue;
            }
            let Some(next) = chars.get(index.saturating_add(1)).copied() else {
                output.push(ch);
                break;
            };
            let consumed = match next {
                '$' => {
                    output.push('$');
                    2
                }
                '&' => {
                    output.push_str(&matched.text);
                    2
                }
                '`' => {
                    push_checked_slice(&mut output, text, 0, matched.byte_start)?;
                    2
                }
                '\'' => {
                    push_checked_slice(&mut output, text, matched.byte_end, text.len())?;
                    2
                }
                '1'..='9' => self.append_regexp_numeric_capture(
                    &mut output,
                    &chars,
                    index.saturating_add(1),
                    &matched.captures,
                )?,
                '<' if !matches!(matched.groups, Value::Undefined) => self
                    .append_regexp_named_capture(
                        &mut output,
                        &chars,
                        index.saturating_add(2),
                        &matched.groups,
                    )?,
                _ => {
                    output.push('$');
                    1
                }
            };
            index = index.saturating_add(consumed);
        }
        Ok(output)
    }

    fn append_regexp_numeric_capture(
        &mut self,
        output: &mut String,
        chars: &[char],
        digit_index: usize,
        captures: &[Value],
    ) -> Result<usize> {
        let first = chars
            .get(digit_index)
            .and_then(|digit| digit.to_digit(10))
            .and_then(|digit| usize::try_from(digit).ok())
            .ok_or_else(|| Error::runtime("RegExp replacement capture digit disappeared"))?;
        let second = chars
            .get(digit_index.saturating_add(1))
            .and_then(|digit| digit.to_digit(10))
            .and_then(|digit| usize::try_from(digit).ok());
        let combined = second.and_then(|second| first.checked_mul(10)?.checked_add(second));
        let (capture, consumed) = if combined.is_some_and(|capture| capture <= captures.len()) {
            (combined.unwrap_or(first), 3)
        } else if first <= captures.len() {
            (first, 2)
        } else {
            output.push('$');
            return Ok(1);
        };
        if let Some(value) = captures.get(capture.saturating_sub(1))
            && !matches!(value, Value::Undefined)
        {
            output.push_str(&self.to_string(value)?);
        }
        Ok(consumed)
    }

    fn append_regexp_named_capture(
        &mut self,
        output: &mut String,
        chars: &[char],
        name_start: usize,
        groups: &Value,
    ) -> Result<usize> {
        let Some(relative_end) = chars
            .get(name_start..)
            .and_then(|tail| tail.iter().position(|ch| *ch == '>'))
        else {
            output.push_str("$<");
            return Ok(2);
        };
        let name_end = name_start.saturating_add(relative_end);
        let name = chars
            .get(name_start..name_end)
            .ok_or_else(|| Error::runtime("RegExp replacement group name is out of bounds"))?
            .iter()
            .collect::<String>();
        let value = self.get_named(groups, &name)?;
        if !matches!(value, Value::Undefined) {
            output.push_str(&self.to_string(&value)?);
        }
        Ok(relative_end.saturating_add(3))
    }

    fn string_regexp_match_from_value(
        &mut self,
        text: &str,
        result: &Value,
    ) -> Result<Option<StringRegExpMatch>> {
        let Value::Object(id) = result else {
            return Ok(None);
        };
        let index = self
            .get_named(&Value::Object(*id), REGEXP_MATCH_INDEX_PROPERTY)?
            .as_number()
            .ok_or_else(|| Error::runtime("RegExp match index is not numeric"))?;
        let code_unit_start = number_to_usize(index)?;
        let matched_value = self.get_named(&Value::Object(*id), FIRST_MATCH_PROPERTY)?;
        let matched = self.to_string(&matched_value)?;
        let length_value = self.get_named(&Value::Object(*id), REGEXP_MATCH_LENGTH_PROPERTY)?;
        let length = Self::length_to_usize(
            self.to_length(&length_value)?,
            "RegExp match capture count exceeded supported range",
        )?;
        let mut captures = Vec::with_capacity(length.saturating_sub(1));
        for capture in 1..length {
            captures.push(self.get_named(&Value::Object(*id), &capture.to_string())?);
        }
        let groups = self.get_named(&Value::Object(*id), REGEXP_MATCH_GROUPS_PROPERTY)?;
        let byte_start = utf16_index_to_byte_boundary(text, code_unit_start).ok_or_else(|| {
            Error::runtime("RegExp match index is not a valid UTF-16 string boundary")
        })?;
        let byte_end = byte_start
            .checked_add(matched.len())
            .ok_or_else(|| Error::limit("RegExp match end overflowed"))?;
        let code_unit_end = code_unit_start
            .checked_add(matched.encode_utf16().count())
            .ok_or_else(|| Error::limit("RegExp match code-unit end overflowed"))?;
        Ok(Some(StringRegExpMatch {
            byte_start,
            byte_end,
            code_unit_start,
            code_unit_end,
            text: matched,
            captures,
            groups,
        }))
    }

    fn string_regexp_set_last_index(&mut self, pattern: &Value, index: usize) -> Result<()> {
        self.string_regexp_set_last_index_value(pattern, Value::Number(index_to_number(index)?))
    }

    fn string_regexp_set_last_index_value(&mut self, pattern: &Value, value: Value) -> Result<()> {
        let lookup = self.property_lookup(REGEXP_LAST_INDEX_PROPERTY);
        self.set(
            pattern,
            lookup,
            value,
            pattern,
            crate::runtime::abstract_operations::SetFailureBehavior::Throw,
        )
        .map(|_| ())
    }

    fn string_regexp_advance_last_index(
        &mut self,
        pattern: &Value,
        text: &str,
        index: usize,
    ) -> Result<()> {
        let unicode = self.string_regexp_is_unicode(pattern)?;
        let next = advance_utf16_index(text, index, unicode)?;
        self.string_regexp_set_last_index(pattern, next)
    }

    fn string_regexp_is_unicode(&self, value: &Value) -> Result<bool> {
        let Value::Object(id) = value else {
            return Ok(false);
        };
        Ok(self
            .objects
            .regexp_value(*id)?
            .is_some_and(|regexp| regexp.flags().contains('u') || regexp.flags().contains('v')))
    }

    fn string_values_array(&mut self, values: Vec<String>) -> Result<Value> {
        let mut elements = Vec::with_capacity(values.len());
        for value in values {
            elements.push(self.heap_string_value(&value)?);
        }
        self.string_value_array(elements)
    }

    fn string_utf16_values_array(&mut self, values: Vec<Vec<u16>>) -> Result<Value> {
        let mut elements = Vec::with_capacity(values.len());
        for value in values {
            elements.push(self.heap_utf16_string_value(&value)?);
        }
        self.string_value_array(elements)
    }

    fn string_value_array(&mut self, elements: Vec<Value>) -> Result<Value> {
        self.array_constructor_value()?;
        let prototype = self.objects.existing_array_prototype_id()?;
        self.objects.create_array(
            elements,
            prototype,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn string_split_limit(&mut self, value: Option<&Value>) -> Result<usize> {
        let limit = match value {
            None | Some(Value::Undefined) => u32::MAX,
            Some(value) => {
                number_to_uint32(self.to_number(value)?, "String.prototype.split limit")?
            }
        };
        usize::try_from(limit)
            .map_err(|_| Error::limit("String.prototype.split limit exceeded supported range"))
    }
}

fn replace_first_plain(text: &str, needle: &str, replacement: &str) -> Result<String> {
    let Some(start) = text.find(needle) else {
        return Ok(text.to_owned());
    };
    let end = start
        .checked_add(needle.len())
        .ok_or_else(|| Error::limit("String.prototype.replace end overflowed"))?;
    replace_span(text, start, end, replacement)
}

fn replace_span(text: &str, start: usize, end: usize, replacement: &str) -> Result<String> {
    let mut output = String::new();
    push_checked_slice(&mut output, text, 0, start)?;
    output.push_str(replacement);
    push_checked_slice(&mut output, text, end, text.len())?;
    Ok(output)
}

fn split_plain_utf16(text: &[u16], separator: &[u16], limit: usize) -> Result<Vec<Vec<u16>>> {
    if separator.is_empty() {
        return Ok(text.iter().take(limit).map(|unit| vec![*unit]).collect());
    }
    let mut values = Vec::new();
    let mut cursor = 0usize;
    while values.len() < limit {
        let Some(relative) = text
            .get(cursor..)
            .ok_or_else(|| Error::runtime("String.prototype.split cursor is invalid"))?
            .windows(separator.len())
            .position(|window| window == separator)
        else {
            break;
        };
        let start = cursor
            .checked_add(relative)
            .ok_or_else(|| Error::limit("String.prototype.split index overflowed"))?;
        values.push(
            text.get(cursor..start)
                .ok_or_else(|| Error::runtime("String.prototype.split span is invalid"))?
                .to_vec(),
        );
        if values.len() >= limit {
            return Ok(values);
        }
        cursor = start
            .checked_add(separator.len())
            .ok_or_else(|| Error::limit("String.prototype.split cursor overflowed"))?;
    }
    values.push(
        text.get(cursor..)
            .ok_or_else(|| Error::runtime("String.prototype.split tail is invalid"))?
            .to_vec(),
    );
    Ok(values)
}

fn push_checked_slice(output: &mut String, text: &str, start: usize, end: usize) -> Result<()> {
    output.push_str(
        text.get(start..end)
            .ok_or_else(|| Error::runtime("String.prototype RegExp span is invalid"))?,
    );
    Ok(())
}

fn utf16_index_to_byte_boundary(text: &str, index: usize) -> Option<usize> {
    let mut code_units = 0usize;
    for (byte_index, ch) in text.char_indices() {
        if code_units == index {
            return Some(byte_index);
        }
        code_units = code_units.checked_add(ch.len_utf16())?;
        if code_units == index {
            return byte_index.checked_add(ch.len_utf8());
        }
        if code_units > index {
            return None;
        }
    }
    (code_units == index).then_some(text.len())
}

fn advance_utf16_index(text: &str, index: usize, unicode: bool) -> Result<usize> {
    let units = text.encode_utf16().collect::<Vec<_>>();
    let Some(first) = units.get(index).copied() else {
        return index
            .checked_add(1)
            .ok_or_else(|| Error::limit("RegExp string index overflowed"));
    };
    let width = if unicode
        && (0xD800..=0xDBFF).contains(&first)
        && units
            .get(index.saturating_add(1))
            .is_some_and(|second| (0xDC00..=0xDFFF).contains(second))
    {
        2
    } else {
        1
    };
    index
        .checked_add(width)
        .ok_or_else(|| Error::limit("RegExp string index overflowed"))
}

fn number_to_usize(number: f64) -> Result<usize> {
    if !number.is_finite() || number <= 0.0 {
        return Ok(0);
    }
    format!("{:.0}", number.trunc())
        .parse::<usize>()
        .map_err(|_| Error::limit("numeric index exceeded supported range"))
}

fn optional_index_to_number(index: Option<usize>) -> Result<f64> {
    let Some(index) = index else {
        return Ok(-1.0);
    };
    index_to_number(index)
}

fn index_to_number(index: usize) -> Result<f64> {
    let index = u32::try_from(index).map_err(|_| Error::limit("index exceeded supported range"))?;
    Ok(f64::from(index))
}
