use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, numeric::number_to_uint32},
    value::Value,
};

const REGEXP_LAST_INDEX_PROPERTY: &str = "lastIndex";
const REGEXP_MATCH_INDEX_PROPERTY: &str = "index";
const FIRST_MATCH_PROPERTY: &str = "0";
const ZERO_INDEX: f64 = 0.0;

#[derive(Debug)]
struct StringRegExpMatch {
    start: usize,
    end: usize,
    text: String,
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
            let match_end = matched.end;
            let is_empty_match = matched.start == matched.end;
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
            Some(matched) => index_to_number(matched.start)?,
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
        let text = self.string_receiver_value(this_value)?;
        let limit = self.string_split_limit(args.as_slice().get(1))?;
        if limit == 0 {
            return self.string_values_array(Vec::new());
        }
        let Some(separator) = args.as_slice().first() else {
            return self.string_values_array(vec![text]);
        };
        if !self.string_regexp_is_object(separator)? {
            let separator = self.string_argument_text(separator)?;
            return self.string_values_array(split_plain(&text, &separator, limit)?);
        }
        self.string_regexp_split(&text, separator, limit)
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
        let Value::Object(id) = result else {
            return Ok(None);
        };
        let index = self
            .get_named(&Value::Object(id), REGEXP_MATCH_INDEX_PROPERTY)?
            .as_number()
            .ok_or_else(|| Error::runtime("RegExp match index is not numeric"))?;
        let start = number_to_usize(index)?;
        let matched_value = self.get_named(&Value::Object(id), FIRST_MATCH_PROPERTY)?;
        let matched = self.to_string(&matched_value)?;
        let end = start
            .checked_add(matched.len())
            .ok_or_else(|| Error::limit("RegExp match end overflowed"))?;
        Ok(Some(StringRegExpMatch {
            start,
            end,
            text: matched,
        }))
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
        let output = replace_span(text, matched.start, matched.end, replacement)?;
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
            if matched.start < cursor {
                return Err(Error::runtime("RegExp global match moved backwards"));
            }
            push_checked_slice(&mut output, text, cursor, matched.start)?;
            output.push_str(replacement);
            cursor = matched.end;
            self.check_string_len(&output)?;
            if matched.start == matched.end {
                self.string_regexp_advance_last_index(pattern, text, matched.end)?;
            }
        }
        push_checked_slice(&mut output, text, cursor, text.len())?;
        self.check_string_len(&output)?;
        self.heap_string_value(&output)
    }

    fn string_regexp_split(
        &mut self,
        text: &str,
        separator: &Value,
        limit: usize,
    ) -> Result<Value> {
        let mut parts = Vec::new();
        let mut cursor = 0usize;
        while parts.len() < limit {
            let Some(matched) = self.string_regexp_exec_match_from(separator, text, cursor)? else {
                break;
            };
            if matched.start < cursor {
                return Err(Error::runtime("RegExp split match moved backwards"));
            }
            parts.push(slice_to_string(text, cursor, matched.start)?);
            cursor = matched.end;
            if matched.start == matched.end {
                cursor = next_char_boundary(text, matched.end);
            }
        }
        if parts.len() < limit {
            parts.push(slice_to_string(text, cursor, text.len())?);
        }
        self.string_values_array(parts)
    }

    fn string_regexp_exec_match_from(
        &mut self,
        pattern: &Value,
        text: &str,
        start: usize,
    ) -> Result<Option<StringRegExpMatch>> {
        let result = self.regexp_exec_from(pattern, text, start)?;
        self.string_regexp_match_from_value(&result)
    }

    fn string_regexp_match_from_value(
        &mut self,
        result: &Value,
    ) -> Result<Option<StringRegExpMatch>> {
        let Value::Object(id) = result else {
            return Ok(None);
        };
        let index = self
            .get_named(&Value::Object(*id), REGEXP_MATCH_INDEX_PROPERTY)?
            .as_number()
            .ok_or_else(|| Error::runtime("RegExp match index is not numeric"))?;
        let start = number_to_usize(index)?;
        let matched_value = self.get_named(&Value::Object(*id), FIRST_MATCH_PROPERTY)?;
        let matched = self.to_string(&matched_value)?;
        let end = start
            .checked_add(matched.len())
            .ok_or_else(|| Error::limit("RegExp match end overflowed"))?;
        Ok(Some(StringRegExpMatch {
            start,
            end,
            text: matched,
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
        let next = next_char_boundary(text, index);
        self.string_regexp_set_last_index(pattern, next)
    }

    fn string_values_array(&mut self, values: Vec<String>) -> Result<Value> {
        self.array_constructor_value()?;
        let prototype = self.objects.existing_array_prototype_id()?;
        let mut elements = Vec::with_capacity(values.len());
        for value in values {
            elements.push(self.heap_string_value(&value)?);
        }
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

fn split_plain(text: &str, separator: &str, limit: usize) -> Result<Vec<String>> {
    if separator.is_empty() {
        let mut values = Vec::new();
        for ch in text.chars().take(limit) {
            values.push(ch.to_string());
        }
        return Ok(values);
    }
    let mut values = Vec::new();
    let mut cursor = 0usize;
    while values.len() + 1 < limit {
        let Some(relative) = text
            .get(cursor..)
            .ok_or_else(|| Error::runtime("String.prototype.split cursor is invalid"))?
            .find(separator)
        else {
            break;
        };
        let start = cursor
            .checked_add(relative)
            .ok_or_else(|| Error::limit("String.prototype.split index overflowed"))?;
        values.push(slice_to_string(text, cursor, start)?);
        cursor = start
            .checked_add(separator.len())
            .ok_or_else(|| Error::limit("String.prototype.split cursor overflowed"))?;
    }
    values.push(slice_to_string(text, cursor, text.len())?);
    Ok(values)
}

fn push_checked_slice(output: &mut String, text: &str, start: usize, end: usize) -> Result<()> {
    output.push_str(
        text.get(start..end)
            .ok_or_else(|| Error::runtime("String.prototype RegExp span is invalid"))?,
    );
    Ok(())
}

fn slice_to_string(text: &str, start: usize, end: usize) -> Result<String> {
    text.get(start..end)
        .map(ToOwned::to_owned)
        .ok_or_else(|| Error::runtime("String.prototype RegExp span is invalid"))
}

fn next_char_boundary(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    text.get(index..)
        .and_then(|tail| tail.chars().next())
        .map_or(text.len(), |ch| index.saturating_add(ch.len_utf8()))
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
