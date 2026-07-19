#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    runtime::{Context, abstract_operations::same_value, call::RuntimeCallArgs},
    value::Value,
};

use super::{REGEXP_FLAGS_PROPERTY, REGEXP_LAST_INDEX_PROPERTY};

impl Context {
    pub(in crate::runtime::native) fn eval_regexp_prototype_symbol_match(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        if self.semantic_object_ref(receiver)?.is_none() {
            return Err(Error::type_error(
                "RegExp.prototype[Symbol.match] requires an object receiver",
            ));
        }
        let input = self.regexp_argument_utf16_or_undefined(args.as_slice().first())?;
        let input_value = self.heap_utf16_string_value(&input)?;
        let flags = self.get_named(receiver, REGEXP_FLAGS_PROPERTY)?;
        let flags = self.to_string(&flags)?;
        let global = flags.contains('g');
        if !global {
            return self
                .regexp_exec_abstract(receiver, &input_value, &input)
                .map(|result| result.unwrap_or(Value::Null));
        }

        let unicode = flags.contains('u') || flags.contains('v');
        self.set_regexp_last_index(receiver, 0)?;
        let mut collected = Vec::new();
        loop {
            let Some(result) = self.regexp_exec_abstract(receiver, &input_value, &input)? else {
                if collected.is_empty() {
                    return Ok(Value::Null);
                }
                return self.regexp_match_strings_array(collected);
            };
            let match_value = self.get_named(&result, "0")?;
            let match_units = self.to_utf16_string(&match_value)?;
            if collected.len() >= self.limits.max_object_properties {
                return Err(Error::limit("RegExp match result exceeded array limit"));
            }
            let empty = match_units.is_empty();
            collected.push(match_units);
            if empty {
                let last_index = self.get_named(receiver, REGEXP_LAST_INDEX_PROPERTY)?;
                let last_index = Self::length_to_usize(
                    self.to_length(&last_index)?,
                    "RegExp match lastIndex exceeded supported range",
                )?;
                let next = advance_match_index(&input, last_index, unicode)?;
                self.set_regexp_last_index(receiver, next)?;
            }
        }
    }

    pub(in crate::runtime::native) fn eval_regexp_prototype_symbol_search(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        if self.semantic_object_ref(receiver)?.is_none() {
            return Err(Error::type_error(
                "RegExp.prototype[Symbol.search] requires an object receiver",
            ));
        }
        let input = self.regexp_argument_utf16_or_undefined(args.as_slice().first())?;
        let input_value = self.heap_utf16_string_value(&input)?;
        let previous = self.get_named(receiver, REGEXP_LAST_INDEX_PROPERTY)?;
        if !same_value(&previous, &Value::Number(0.0)) {
            self.set_regexp_last_index(receiver, 0)?;
        }
        let result = self.regexp_exec_abstract(receiver, &input_value, &input)?;
        let current = self.get_named(receiver, REGEXP_LAST_INDEX_PROPERTY)?;
        if !same_value(&current, &previous) {
            self.set_regexp_last_index_value(receiver, previous)?;
        }
        let Some(result) = result else {
            return Ok(Value::Number(-1.0));
        };
        self.get_named(&result, "index")
    }

    fn regexp_match_strings_array(&mut self, matches: Vec<Vec<u16>>) -> Result<Value> {
        let mut elements = Vec::with_capacity(matches.len());
        for matched in matches {
            elements.push(self.heap_utf16_string_value(&matched)?);
        }
        self.array_constructor_value()?;
        let prototype = self.objects.existing_array_prototype_id()?;
        self.objects.create_array(
            elements,
            prototype,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }
}

pub(super) fn advance_match_index(input: &[u16], index: usize, unicode: bool) -> Result<usize> {
    let Some(first) = input.get(index).copied() else {
        return index
            .checked_add(1)
            .ok_or_else(|| Error::limit("RegExp match string index overflowed"));
    };
    let width = if unicode
        && (0xD800..=0xDBFF).contains(&first)
        && input
            .get(index.saturating_add(1))
            .is_some_and(|second| (0xDC00..=0xDFFF).contains(second))
    {
        2
    } else {
        1
    };
    index
        .checked_add(width)
        .ok_or_else(|| Error::limit("RegExp match string index overflowed"))
}
