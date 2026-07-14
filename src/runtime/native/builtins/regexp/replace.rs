use crate::{
    error::{Error, Result},
    runtime::{Context, abstract_operations::to_boolean, call::RuntimeCallArgs},
    value::{ErrorName, Value},
};

use super::{REGEXP_FLAGS_PROPERTY, REGEXP_GLOBAL_PROPERTY, REGEXP_LAST_INDEX_PROPERTY};

const REGEXP_UNICODE_PROPERTY: &str = "unicode";
const DOLLAR: u16 = 0x24;

struct RegExpReplaceMatch {
    position: usize,
    matched: Vec<u16>,
    captures: Vec<Option<Vec<u16>>>,
    groups: Value,
}

impl Context {
    pub(in crate::runtime::native) fn eval_regexp_prototype_symbol_replace(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if self.semantic_object_ref(this_value)?.is_none() {
            return Err(Error::type_error(
                "RegExp.prototype[Symbol.replace] requires an object receiver",
            ));
        }
        let input = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let input_units = self.to_utf16_string(&input)?;
        let input_value = self.heap_utf16_string_value(&input_units)?;
        let replacement = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        let functional = self.semantic_is_callable(&replacement)?;
        let (replacement, global, unicode) =
            self.regexp_replace_configuration(this_value, replacement, functional)?;
        let results =
            self.regexp_replace_results(this_value, &input_value, &input_units, global, unicode)?;
        self.regexp_replace_compose(&input_units, &input_value, results, &replacement)
    }

    fn regexp_replace_configuration(
        &mut self,
        receiver: &Value,
        replacement: Value,
        functional: bool,
    ) -> Result<(Value, bool, bool)> {
        if functional {
            let global_value = self.get_named(receiver, REGEXP_GLOBAL_PROPERTY)?;
            let global = to_boolean(self, &global_value)?;
            let unicode = if global {
                let unicode_value = self.get_named(receiver, REGEXP_UNICODE_PROPERTY)?;
                to_boolean(self, &unicode_value)?
            } else {
                false
            };
            return Ok((replacement, global, unicode));
        }
        let replacement = self.to_utf16_string(&replacement)?;
        let replacement = self.heap_utf16_string_value(&replacement)?;
        let flags = self.get_named(receiver, REGEXP_FLAGS_PROPERTY)?;
        let flags = self.to_string(&flags)?;
        let global = flags.contains('g');
        let unicode = global && (flags.contains('u') || flags.contains('v'));
        Ok((replacement, global, unicode))
    }

    fn regexp_replace_results(
        &mut self,
        receiver: &Value,
        input_value: &Value,
        input_units: &[u16],
        global: bool,
        unicode: bool,
    ) -> Result<Vec<Value>> {
        if global {
            self.set_regexp_last_index(receiver, 0)?;
        }
        let mut results = Vec::new();
        while let Some(result) = self.regexp_exec_abstract(receiver, input_value, input_units)? {
            if results.len() >= self.limits.max_object_properties {
                return Err(Error::limit(
                    "RegExp replacement result count exceeded limit",
                ));
            }
            results.push(result.clone());
            if !global {
                break;
            }
            let matched = self.get_named(&result, "0")?;
            if self.to_string(&matched)?.is_empty() {
                let index = self.get_named(receiver, REGEXP_LAST_INDEX_PROPERTY)?;
                let index = self.to_length(&index)?;
                let next = advance_replace_index(input_units, index, unicode)?;
                let next = next
                    .to_string()
                    .parse::<f64>()
                    .map_err(|_| Error::limit("RegExp replacement index conversion failed"))?;
                self.set_regexp_last_index_value(receiver, Value::Number(next))?;
            }
        }
        Ok(results)
    }

    fn regexp_replace_compose(
        &mut self,
        input: &[u16],
        input_value: &Value,
        results: Vec<Value>,
        replacement: &Value,
    ) -> Result<Value> {
        let functional = self.semantic_is_callable(replacement)?;
        let mut output = Vec::new();
        let mut next_source_position = 0usize;
        for result in results {
            let matched = self.regexp_replace_match(input.len(), &result)?;
            let substitution = self.regexp_replace_substitution(
                input,
                input_value,
                &matched,
                replacement,
                functional,
            )?;
            if matched.position < next_source_position {
                continue;
            }
            let prefix = input
                .get(next_source_position..matched.position)
                .ok_or_else(|| Error::runtime("RegExp replacement prefix is invalid"))?;
            extend_replace_output(&mut output, prefix, self.limits.max_string_len)?;
            extend_replace_output(&mut output, &substitution, self.limits.max_string_len)?;
            next_source_position = matched
                .position
                .saturating_add(matched.matched.len())
                .min(input.len());
        }
        let tail = input
            .get(next_source_position..)
            .ok_or_else(|| Error::runtime("RegExp replacement tail is invalid"))?;
        extend_replace_output(&mut output, tail, self.limits.max_string_len)?;
        self.heap_utf16_string_value(&output)
    }

    fn regexp_replace_match(
        &mut self,
        input_length: usize,
        result: &Value,
    ) -> Result<RegExpReplaceMatch> {
        let length = self.get_named(result, "length")?;
        let length = Self::length_to_usize(
            self.to_length(&length)?,
            "RegExp replacement capture count exceeded supported range",
        )?;
        let matched = self.get_named(result, "0")?;
        let matched = self.to_utf16_string(&matched)?;
        let position = self.get_named(result, "index")?;
        let position = self.to_integer_or_infinity(&position)?;
        let position = if position <= 0.0 {
            0
        } else if position.is_infinite() {
            input_length
        } else {
            Self::finite_nonnegative_integer_to_usize(
                position,
                "RegExp replacement position exceeded supported range",
            )?
            .min(input_length)
        };
        let mut captures = Vec::with_capacity(length.saturating_sub(1));
        for index in 1..length {
            let capture = self.get_named(result, &index.to_string())?;
            if matches!(capture, Value::Undefined) {
                captures.push(None);
            } else {
                captures.push(Some(self.to_utf16_string(&capture)?));
            }
        }
        let groups = self.get_named(result, "groups")?;
        Ok(RegExpReplaceMatch {
            position,
            matched,
            captures,
            groups,
        })
    }

    fn regexp_replace_substitution(
        &mut self,
        input: &[u16],
        input_value: &Value,
        matched: &RegExpReplaceMatch,
        replacement: &Value,
        functional: bool,
    ) -> Result<Vec<u16>> {
        if functional {
            let mut args = Vec::with_capacity(matched.captures.len().saturating_add(4));
            args.push(self.heap_utf16_string_value(&matched.matched)?);
            for capture in &matched.captures {
                args.push(match capture {
                    Some(capture) => self.heap_utf16_string_value(capture)?,
                    None => Value::Undefined,
                });
            }
            let position = Self::usize_to_number(
                matched.position,
                "RegExp replacement position exceeded numeric range",
            )?;
            args.push(Value::Number(position));
            args.push(input_value.clone());
            if !matches!(matched.groups, Value::Undefined) {
                args.push(matched.groups.clone());
            }
            let value = self.call_value(replacement, &args, Value::Undefined)?;
            return self.to_utf16_string(&value);
        }
        if matches!(matched.groups, Value::Null) {
            return Err(Error::type_error(
                "RegExp replacement named captures cannot be null",
            ));
        }
        let template = self.to_utf16_string(replacement)?;
        self.regexp_get_substitution(input, matched, &template)
    }

    fn regexp_get_substitution(
        &mut self,
        input: &[u16],
        matched: &RegExpReplaceMatch,
        template: &[u16],
    ) -> Result<Vec<u16>> {
        let mut output = Vec::new();
        let mut index = 0usize;
        while let Some(unit) = template.get(index).copied() {
            if unit != DOLLAR {
                push_replace_output(&mut output, unit, self.limits.max_string_len)?;
                index = index.saturating_add(1);
                continue;
            }
            let Some(next) = template.get(index.saturating_add(1)).copied() else {
                push_replace_output(&mut output, unit, self.limits.max_string_len)?;
                break;
            };
            let consumed = match next {
                0x24 => {
                    push_replace_output(&mut output, DOLLAR, self.limits.max_string_len)?;
                    2
                }
                0x26 => {
                    extend_replace_output(
                        &mut output,
                        &matched.matched,
                        self.limits.max_string_len,
                    )?;
                    2
                }
                0x60 => {
                    let prefix = input
                        .get(..matched.position)
                        .ok_or_else(|| Error::runtime("RegExp replacement prefix is invalid"))?;
                    extend_replace_output(&mut output, prefix, self.limits.max_string_len)?;
                    2
                }
                0x27 => {
                    let end = matched
                        .position
                        .saturating_add(matched.matched.len())
                        .min(input.len());
                    let suffix = input
                        .get(end..)
                        .ok_or_else(|| Error::runtime("RegExp replacement suffix is invalid"))?;
                    extend_replace_output(&mut output, suffix, self.limits.max_string_len)?;
                    2
                }
                0x30..=0x39 => append_capture(
                    &mut output,
                    template,
                    index,
                    &matched.captures,
                    self.limits.max_string_len,
                )?,
                0x3C if !matches!(matched.groups, Value::Undefined) => {
                    self.append_named_capture(&mut output, template, index, &matched.groups)?
                }
                _ => {
                    push_replace_output(&mut output, DOLLAR, self.limits.max_string_len)?;
                    1
                }
            };
            index = index.saturating_add(consumed);
        }
        Ok(output)
    }

    fn append_named_capture(
        &mut self,
        output: &mut Vec<u16>,
        template: &[u16],
        dollar_index: usize,
        groups: &Value,
    ) -> Result<usize> {
        let name_start = dollar_index.saturating_add(2);
        let Some(relative_end) = template
            .get(name_start..)
            .and_then(|tail| tail.iter().position(|unit| *unit == 0x3E))
        else {
            extend_replace_output(output, &[DOLLAR, 0x3C], self.limits.max_string_len)?;
            return Ok(2);
        };
        let name_end = name_start.saturating_add(relative_end);
        let name = String::from_utf16(
            template
                .get(name_start..name_end)
                .ok_or_else(|| Error::runtime("RegExp capture name is invalid"))?,
        )
        .map_err(|_| Error::runtime("RegExp capture name is not well-formed UTF-16"))?;
        let capture = self.get_named(groups, &name)?;
        if !matches!(capture, Value::Undefined) {
            let capture = self.to_utf16_string(&capture)?;
            extend_replace_output(output, &capture, self.limits.max_string_len)?;
        }
        Ok(relative_end.saturating_add(3))
    }
}

fn append_capture(
    output: &mut Vec<u16>,
    template: &[u16],
    dollar_index: usize,
    captures: &[Option<Vec<u16>>],
    max_string_len: usize,
) -> Result<usize> {
    let first = template
        .get(dollar_index.saturating_add(1))
        .map_or(0, |unit| usize::from(unit.saturating_sub(0x30)));
    let second = template
        .get(dollar_index.saturating_add(2))
        .copied()
        .filter(|unit| (0x30..=0x39).contains(unit))
        .map(|unit| usize::from(unit.saturating_sub(0x30)));
    let combined = second.and_then(|second| first.checked_mul(10)?.checked_add(second));
    let (capture, consumed) =
        if combined.is_some_and(|capture| capture >= 1 && capture <= captures.len()) {
            (combined.unwrap_or(first), 3)
        } else if first >= 1 && first <= captures.len() {
            (first, 2)
        } else {
            push_replace_output(output, DOLLAR, max_string_len)?;
            return Ok(1);
        };
    if let Some(Some(capture)) = captures.get(capture.saturating_sub(1)) {
        extend_replace_output(output, capture, max_string_len)?;
    }
    Ok(consumed)
}

fn push_replace_output(output: &mut Vec<u16>, unit: u16, max_string_len: usize) -> Result<()> {
    extend_replace_output(output, &[unit], max_string_len)
}

fn extend_replace_output(
    output: &mut Vec<u16>,
    value: &[u16],
    max_string_len: usize,
) -> Result<()> {
    let length = output.len().checked_add(value.len()).ok_or_else(|| {
        Error::exception(
            ErrorName::RangeError,
            "RegExp replacement length overflowed",
        )
    })?;
    if length > max_string_len {
        return Err(Error::exception(
            ErrorName::RangeError,
            format!("RegExp replacement length {length} exceeded {max_string_len}"),
        ));
    }
    output.extend_from_slice(value);
    Ok(())
}

fn advance_replace_index(input: &[u16], index: u64, unicode: bool) -> Result<u64> {
    let input_index = usize::try_from(index).ok();
    let Some(first) = input_index.and_then(|index| input.get(index)).copied() else {
        return index
            .checked_add(1)
            .ok_or_else(|| Error::limit("RegExp replacement index overflowed"));
    };
    let width = if unicode
        && (0xD800..=0xDBFF).contains(&first)
        && input_index
            .and_then(|index| input.get(index.saturating_add(1)))
            .is_some_and(|second| (0xDC00..=0xDFFF).contains(second))
    {
        2_u64
    } else {
        1_u64
    };
    index
        .checked_add(width)
        .ok_or_else(|| Error::limit("RegExp replacement index overflowed"))
}
