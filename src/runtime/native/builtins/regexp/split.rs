#[cfg(not(feature = "std"))]
use crate::prelude::*;

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

use super::{REGEXP_FLAGS_PROPERTY, REGEXP_LAST_INDEX_PROPERTY};

const REGEXP_EXEC_PROPERTY: &str = "exec";
const REGEXP_MATCH_LENGTH_PROPERTY: &str = "length";
const SPECIES_PROPERTY: &str = "species";
const SPECIES_SYMBOL_DISPLAY: &str = "[Symbol.species]";

impl Context {
    pub(in crate::runtime::native) fn eval_regexp_prototype_symbol_split(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if self.semantic_object_ref(this_value)?.is_none() {
            return Err(Error::type_error(
                "RegExp.prototype[Symbol.split] requires an object receiver",
            ));
        }

        let input = self.regexp_argument_utf16_or_undefined(args.as_slice().first())?;
        let constructor = self.regexp_species_constructor(this_value)?;
        let flags_value = self.get_named(this_value, REGEXP_FLAGS_PROPERTY)?;
        let mut flags = self.to_string(&flags_value)?;
        let unicode_matching = flags.contains('u') || flags.contains('v');
        if !flags.contains('y') {
            flags.push('y');
        }
        let flags_value = self.heap_string_value(&flags)?;
        let splitter = self.semantic_construct(
            &constructor,
            &[this_value.clone(), flags_value],
            constructor.clone(),
        )?;
        let limit = self.regexp_split_limit(args.as_slice().get(1))?;
        let values = self.regexp_split_values(&splitter, &input, unicode_matching, limit)?;
        self.regexp_split_array(values)
    }

    pub(super) fn regexp_species_constructor(&mut self, receiver: &Value) -> Result<Value> {
        let default = self.regexp_constructor_value()?;
        let constructor = self.get_named(receiver, "constructor")?;
        if matches!(constructor, Value::Undefined) {
            return Ok(default);
        }
        if self.semantic_object_ref(&constructor)?.is_none() {
            return Err(Error::type_error(
                "RegExp species constructor property must be an object",
            ));
        }
        let symbol_constructor = self.symbol_constructor_value()?;
        let species = self.get_named(&symbol_constructor, SPECIES_PROPERTY)?;
        let Value::Symbol(species) = species else {
            return Err(Error::runtime("Symbol.species is not initialized"));
        };
        let species = self.get(
            &constructor,
            PropertyLookup::from_key(SPECIES_SYMBOL_DISPLAY, PropertyKey::symbol(species.id())),
        )?;
        if matches!(species, Value::Undefined | Value::Null) {
            return Ok(default);
        }
        if !self.semantic_is_constructor(&species)? {
            return Err(Error::type_error(
                "RegExp species value must be a constructor",
            ));
        }
        Ok(species)
    }

    fn regexp_split_values(
        &mut self,
        splitter: &Value,
        input: &[u16],
        unicode_matching: bool,
        limit: usize,
    ) -> Result<Vec<Value>> {
        let mut values = Vec::new();
        if limit == 0 {
            return Ok(values);
        }
        let input_value = self.heap_utf16_string_value(input)?;
        if input.is_empty() {
            if self
                .regexp_exec_abstract(splitter, &input_value, input)?
                .is_none()
            {
                self.regexp_split_push_text(&mut values, input, limit)?;
            }
            return Ok(values);
        }

        let mut previous_end = 0usize;
        let mut search_index = 0usize;
        while search_index < input.len() {
            self.regexp_split_set_last_index(splitter, search_index)?;
            let Some(result) = self.regexp_exec_abstract(splitter, &input_value, input)? else {
                search_index = advance_split_index(input, search_index, unicode_matching)?;
                continue;
            };
            let match_end = self.regexp_split_match_end(splitter, input.len())?;
            if match_end == previous_end {
                search_index = advance_split_index(input, search_index, unicode_matching)?;
                continue;
            }
            if self.regexp_split_push_text(
                &mut values,
                input
                    .get(previous_end..search_index)
                    .ok_or_else(|| Error::runtime("RegExp split span is out of bounds"))?,
                limit,
            )? {
                return Ok(values);
            }
            previous_end = match_end;
            if self.regexp_split_push_captures(&mut values, &result, limit)? {
                return Ok(values);
            }
            search_index = previous_end;
        }
        self.regexp_split_push_text(
            &mut values,
            input
                .get(previous_end..)
                .ok_or_else(|| Error::runtime("RegExp split tail is out of bounds"))?,
            limit,
        )?;
        Ok(values)
    }

    pub(super) fn regexp_exec_abstract(
        &mut self,
        splitter: &Value,
        input_value: &Value,
        input: &[u16],
    ) -> Result<Option<Value>> {
        let exec = self.get_named(splitter, REGEXP_EXEC_PROPERTY)?;
        let result = if self.semantic_is_callable(&exec)? {
            self.call_value(&exec, core::slice::from_ref(input_value), splitter.clone())?
        } else if let Value::Object(id) = splitter
            && self.objects.regexp_value(*id)?.is_some()
        {
            self.regexp_exec_code_units(splitter, input)?
        } else {
            return Err(Error::type_error("RegExp exec method is not callable"));
        };
        if matches!(result, Value::Null) {
            return Ok(None);
        }
        if self.semantic_object_ref(&result)?.is_none() {
            return Err(Error::type_error(
                "RegExp exec method must return an object or null",
            ));
        }
        Ok(Some(result))
    }

    fn regexp_split_match_end(&mut self, splitter: &Value, input_len: usize) -> Result<usize> {
        let value = self.get_named(splitter, REGEXP_LAST_INDEX_PROPERTY)?;
        let end = Self::length_to_usize(
            self.to_length(&value)?,
            "RegExp split lastIndex exceeded supported range",
        )?;
        Ok(end.min(input_len))
    }

    fn regexp_split_push_captures(
        &mut self,
        values: &mut Vec<Value>,
        result: &Value,
        limit: usize,
    ) -> Result<bool> {
        let length = self.get_named(result, REGEXP_MATCH_LENGTH_PROPERTY)?;
        let length = Self::length_to_usize(
            self.to_length(&length)?,
            "RegExp split capture count exceeded supported range",
        )?;
        for index in 1..length {
            let capture = self.get_named(result, &index.to_string())?;
            if self.regexp_split_push(values, capture, limit)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn regexp_split_push_text(
        &mut self,
        values: &mut Vec<Value>,
        text: &[u16],
        limit: usize,
    ) -> Result<bool> {
        let value = self.heap_utf16_string_value(text)?;
        self.regexp_split_push(values, value, limit)
    }

    fn regexp_split_push(
        &self,
        values: &mut Vec<Value>,
        value: Value,
        limit: usize,
    ) -> Result<bool> {
        if values.len() >= self.limits.max_object_properties {
            return Err(Error::limit("RegExp split result exceeded array limit"));
        }
        values.push(value);
        Ok(values.len() >= limit)
    }

    fn regexp_split_set_last_index(&mut self, splitter: &Value, index: usize) -> Result<()> {
        let index = u32::try_from(index)
            .map(f64::from)
            .map_err(|_| Error::limit("RegExp split index exceeded supported range"))?;
        self.set_regexp_last_index_value(splitter, Value::Number(index))
    }

    fn regexp_split_array(&mut self, values: Vec<Value>) -> Result<Value> {
        self.array_constructor_value()?;
        let prototype = self.objects.existing_array_prototype_id()?;
        self.objects.create_array(
            values,
            prototype,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn regexp_split_limit(&mut self, value: Option<&Value>) -> Result<usize> {
        let limit = match value {
            None | Some(Value::Undefined) => u32::MAX,
            Some(value) => number_to_uint32(self.to_number(value)?, "RegExp split limit")?,
        };
        usize::try_from(limit)
            .map_err(|_| Error::limit("RegExp split limit exceeded supported range"))
    }
}

fn advance_split_index(input: &[u16], index: usize, unicode: bool) -> Result<usize> {
    let Some(first) = input.get(index).copied() else {
        return index
            .checked_add(1)
            .ok_or_else(|| Error::limit("RegExp split index overflowed"));
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
        .ok_or_else(|| Error::limit("RegExp split index overflowed"))
}
