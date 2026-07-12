use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        numeric::number_to_uint32,
        object::{PropertyKey, PropertyLookup},
        roots::VmRootKind,
    },
    value::Value,
};

const MATCH_SYMBOL_DISPLAY: &str = "[Symbol.match]";
const MATCH_SYMBOL_PROPERTY: &str = "match";
const REPLACE_SYMBOL_DISPLAY: &str = "[Symbol.replace]";
const REPLACE_SYMBOL_PROPERTY: &str = "replace";
const SEARCH_SYMBOL_DISPLAY: &str = "[Symbol.search]";
const SEARCH_SYMBOL_PROPERTY: &str = "search";
const SPLIT_SYMBOL_DISPLAY: &str = "[Symbol.split]";
const SPLIT_SYMBOL_PROPERTY: &str = "split";

#[derive(Debug)]
pub(super) struct StringRegExpMatch {
    pub(super) byte_start: usize,
    pub(super) byte_end: usize,
    pub(super) code_unit_start: usize,
    pub(super) text: String,
    pub(super) captures: Vec<Value>,
    pub(super) groups: Value,
}

impl Context {
    pub(in crate::runtime::native) fn eval_string_prototype_match(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_string_regexp_protocol(
            args,
            this_value,
            MATCH_SYMBOL_PROPERTY,
            MATCH_SYMBOL_DISPLAY,
        )
    }

    pub(in crate::runtime::native) fn eval_string_prototype_search(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_string_regexp_protocol(
            args,
            this_value,
            SEARCH_SYMBOL_PROPERTY,
            SEARCH_SYMBOL_DISPLAY,
        )
    }

    fn eval_string_regexp_protocol(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        symbol_property: &str,
        symbol_display: &str,
    ) -> Result<Value> {
        if matches!(this_value, Value::Undefined | Value::Null) {
            return Err(Error::type_error(
                "String RegExp protocol requires a non-nullish receiver",
            ));
        }
        let pattern = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if self.semantic_object_ref(&pattern)?.is_some()
            && let Some(method) =
                self.string_regexp_protocol_method(&pattern, symbol_property, symbol_display)?
        {
            return self.call_value(&method, std::slice::from_ref(this_value), pattern);
        }

        let string = self.string_receiver_utf16(this_value)?;
        let string = self.heap_utf16_string_value(&string)?;
        let _string_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&string))?;
        let constructor = self.regexp_constructor_value()?;
        let regexp = self.semantic_construct(&constructor, &[pattern], constructor.clone())?;
        let _regexp_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&regexp))?;
        let method = self
            .string_regexp_protocol_method(&regexp, symbol_property, symbol_display)?
            .ok_or_else(|| {
                Error::type_error(format!("RegExp {symbol_display} method is not callable"))
            })?;
        self.call_value(&method, &[string], regexp)
    }

    fn string_regexp_protocol_method(
        &mut self,
        value: &Value,
        symbol_property: &str,
        symbol_display: &str,
    ) -> Result<Option<Value>> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let symbol = self.get_named(&symbol_constructor, symbol_property)?;
        let Value::Symbol(symbol) = symbol else {
            return Err(Error::runtime(format!(
                "Symbol.{symbol_property} is not initialized"
            )));
        };
        self.get_method(
            value,
            PropertyLookup::from_key(symbol_display, PropertyKey::symbol(symbol.id())),
        )
    }

    pub(in crate::runtime::native) fn eval_string_prototype_replace(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if matches!(this_value, Value::Undefined | Value::Null) {
            return Err(Error::type_error(
                "String.prototype.replace requires a non-nullish receiver",
            ));
        }
        let pattern = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let replacement = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if self.semantic_object_ref(&pattern)?.is_some()
            && let Some(replacer) = self.string_replace_method(&pattern)?
        {
            return self.call_value(&replacer, &[this_value.clone(), replacement], pattern);
        }

        let text = self.string_receiver_value(this_value)?;
        let needle = self.string_argument_text(&pattern)?;
        let replacement = if self.semantic_is_callable(&replacement)? {
            replacement
        } else {
            let replacement = self.string_argument_text(&replacement)?;
            self.heap_string_value(&replacement)?
        };
        let Some(byte_start) = text.find(&needle) else {
            return self.heap_string_value(&text);
        };
        let byte_end = byte_start
            .checked_add(needle.len())
            .ok_or_else(|| Error::limit("String.prototype.replace match end overflowed"))?;
        let code_unit_start = text
            .get(..byte_start)
            .ok_or_else(|| Error::runtime("String replacement prefix is invalid"))?
            .encode_utf16()
            .count();
        let matched = StringRegExpMatch {
            byte_start,
            byte_end,
            code_unit_start,
            text: needle,
            captures: Vec::new(),
            groups: Value::Undefined,
        };
        let substitution = self.regexp_replacement_text(&text, &matched, &replacement)?;
        let output = replace_span(&text, byte_start, byte_end, &substitution)?;
        self.check_string_len(&output)?;
        self.heap_string_value(&output)
    }

    fn string_replace_method(&mut self, pattern: &Value) -> Result<Option<Value>> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let symbol = self.get_named(&symbol_constructor, REPLACE_SYMBOL_PROPERTY)?;
        let Value::Symbol(symbol) = symbol else {
            return Err(Error::runtime("Symbol.replace is not initialized"));
        };
        self.get_method(
            pattern,
            PropertyLookup::from_key(REPLACE_SYMBOL_DISPLAY, PropertyKey::symbol(symbol.id())),
        )
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

    pub(super) fn regexp_replacement_text(
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
        if matches!(matched.groups, Value::Null) {
            return Err(Error::type_error(
                "RegExp replacement named captures cannot be null",
            ));
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
                '0'..='9' => self.append_regexp_numeric_capture(
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
        let (capture, consumed) =
            if combined.is_some_and(|capture| capture >= 1 && capture <= captures.len()) {
                (combined.unwrap_or(first), 3)
            } else if first >= 1 && first <= captures.len() {
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

fn index_to_number(index: usize) -> Result<f64> {
    let index = u32::try_from(index).map_err(|_| Error::limit("index exceeded supported range"))?;
    Ok(f64::from(index))
}
