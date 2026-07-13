use std::cmp::Ordering;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::IteratorStep,
        call::RuntimeCallArgs,
        collection_array_iterator::ArrayIterationTarget,
        native::TypedArrayFunctionKind,
        object::{
            PropertyKey, PropertyLookup, TypedArrayContentType, TypedArrayElementKind,
            TypedArrayView,
        },
        roots::VmRootKind,
    },
    value::{ErrorName, ObjectId, Value},
};

const SPECIES_PROPERTY: &str = "species";
const SPECIES_SYMBOL_DISPLAY: &str = "[Symbol.species]";
const TYPED_ARRAY_RECEIVER_ERROR: &str = "TypedArray method receiver is not a typed array";
const TYPED_ARRAY_RESULT_ERROR: &str = "TypedArray constructor did not create a typed array";
pub(super) const TYPED_ARRAY_LENGTH_ERROR: &str = "typed array length exceeded supported range";
const TYPED_ARRAY_SET_RANGE_ERROR: &str = "source array exceeds target typed array";
const TYPED_ARRAY_OFFSET_RANGE_ERROR: &str = "typed array offset is out of range";
const TYPED_ARRAY_SPECIES_ERROR: &str = "TypedArray species is not a constructor";
const TYPED_ARRAY_CONTENT_TYPE_ERROR: &str =
    "typed array source and target must use the same numeric content type";

impl Context {
    pub(in crate::runtime::native) fn eval_typed_array_native_function_kind(
        &mut self,
        kind: TypedArrayFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let values = args.as_slice();
        if let Some(result) = self.eval_typed_array_metadata_kind(kind, values, this_value) {
            return result;
        }
        if let Some(result) = self.eval_typed_array_array_method_kind(kind, values, this_value) {
            return result;
        }
        match kind {
            TypedArrayFunctionKind::Entries => {
                self.eval_typed_array_iterator(this_value, ArrayIterationTarget::Entries)
            }
            TypedArrayFunctionKind::Filter => {
                Err(Error::runtime("typed array filter was routed incorrectly"))
            }
            TypedArrayFunctionKind::FromBase64 => self.eval_uint8_array_from_base64(values),
            TypedArrayFunctionKind::FromHex => self.eval_uint8_array_from_hex(values),
            TypedArrayFunctionKind::Keys => {
                self.eval_typed_array_iterator(this_value, ArrayIterationTarget::Keys)
            }
            TypedArrayFunctionKind::Map => {
                Err(Error::runtime("typed array map was routed incorrectly"))
            }
            TypedArrayFunctionKind::Set => self.eval_typed_array_set(values, this_value),
            TypedArrayFunctionKind::SetFromBase64 => {
                self.eval_uint8_array_set_from_base64(values, this_value)
            }
            TypedArrayFunctionKind::SetFromHex => {
                self.eval_uint8_array_set_from_hex(values, this_value)
            }
            TypedArrayFunctionKind::Slice => self.eval_typed_array_slice_record(values, this_value),
            TypedArrayFunctionKind::Sort => self.eval_typed_array_sort(values, this_value),
            TypedArrayFunctionKind::Subarray => {
                self.eval_typed_array_subarray_record(values, this_value)
            }
            TypedArrayFunctionKind::ToReversed => {
                self.eval_typed_array_to_reversed_record(values, this_value)
            }
            TypedArrayFunctionKind::ToSorted => self.eval_typed_array_to_sorted(values, this_value),
            TypedArrayFunctionKind::ToBase64 => self.eval_uint8_array_to_base64(values, this_value),
            TypedArrayFunctionKind::ToHex => self.eval_uint8_array_to_hex(this_value),
            TypedArrayFunctionKind::Values => {
                self.eval_typed_array_iterator(this_value, ArrayIterationTarget::Values)
            }
            TypedArrayFunctionKind::With => self.eval_typed_array_with_record(values, this_value),
            TypedArrayFunctionKind::At
            | TypedArrayFunctionKind::BufferGetter
            | TypedArrayFunctionKind::ByteLengthGetter
            | TypedArrayFunctionKind::ByteOffsetGetter
            | TypedArrayFunctionKind::CopyWithin
            | TypedArrayFunctionKind::Every
            | TypedArrayFunctionKind::Fill
            | TypedArrayFunctionKind::Find
            | TypedArrayFunctionKind::FindIndex
            | TypedArrayFunctionKind::FindLast
            | TypedArrayFunctionKind::FindLastIndex
            | TypedArrayFunctionKind::ForEach
            | TypedArrayFunctionKind::From
            | TypedArrayFunctionKind::Includes
            | TypedArrayFunctionKind::IndexOf
            | TypedArrayFunctionKind::Join
            | TypedArrayFunctionKind::LastIndexOf
            | TypedArrayFunctionKind::LengthGetter
            | TypedArrayFunctionKind::Of
            | TypedArrayFunctionKind::Reduce
            | TypedArrayFunctionKind::ReduceRight
            | TypedArrayFunctionKind::Reverse
            | TypedArrayFunctionKind::Some
            | TypedArrayFunctionKind::ToLocaleString
            | TypedArrayFunctionKind::ToStringTagGetter => Err(Error::runtime(
                "typed array native function kind was routed incorrectly",
            )),
        }
    }

    fn eval_typed_array_metadata_kind(
        &mut self,
        kind: TypedArrayFunctionKind,
        args: &[Value],
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            TypedArrayFunctionKind::BufferGetter => Some(
                self.typed_array_branded_receiver(this_value)
                    .map(|(_, view)| Value::Object(view.buffer_object())),
            ),
            TypedArrayFunctionKind::ByteLengthGetter => Some((|| {
                let (_, view) = self.typed_array_branded_receiver(this_value)?;
                Self::typed_array_usize_value(view.byte_length()?)
            })()),
            TypedArrayFunctionKind::ByteOffsetGetter => Some((|| {
                let (_, view) = self.typed_array_branded_receiver(this_value)?;
                Self::typed_array_usize_value(view.byte_offset())
            })()),
            TypedArrayFunctionKind::LengthGetter => Some((|| {
                let (_, view) = self.typed_array_branded_receiver(this_value)?;
                Self::typed_array_usize_value(view.length())
            })()),
            TypedArrayFunctionKind::ToStringTagGetter => {
                Some(self.eval_typed_array_to_string_tag(this_value))
            }
            TypedArrayFunctionKind::From => Some(self.eval_typed_array_from(args, this_value)),
            TypedArrayFunctionKind::Of => Some(self.eval_typed_array_of(args, this_value)),
            _ => None,
        }
    }

    fn eval_typed_array_array_method_kind(
        &mut self,
        kind: TypedArrayFunctionKind,
        args: &[Value],
        this_value: &Value,
    ) -> Option<Result<Value>> {
        if let Some(result) = self.eval_typed_array_iteration_kind(kind, args, this_value) {
            return Some(result);
        }
        if let Some(result) = self.eval_typed_array_copy_mutation_kind(kind, args, this_value) {
            return Some(result);
        }
        if !matches!(
            kind,
            TypedArrayFunctionKind::At
                | TypedArrayFunctionKind::Fill
                | TypedArrayFunctionKind::Join
                | TypedArrayFunctionKind::ToLocaleString
        ) {
            return None;
        }
        if let Err(error) = self.typed_array_receiver(this_value) {
            return Some(Err(error));
        }
        let result = match kind {
            TypedArrayFunctionKind::At => self.eval_direct_array_at(args, this_value),
            TypedArrayFunctionKind::Fill => self.eval_typed_array_fill(args, this_value),
            TypedArrayFunctionKind::Join => match self.typed_array_receiver(this_value) {
                Ok((_, view)) => {
                    self.eval_direct_array_join_with_length(args, this_value, Some(view.length()))
                }
                Err(error) => Err(error),
            },
            TypedArrayFunctionKind::ToLocaleString => match self.typed_array_receiver(this_value) {
                Ok((_, view)) => {
                    self.eval_array_to_locale_string_with_length(this_value, view.length())
                }
                Err(error) => Err(error),
            },
            _ => {
                return Some(Err(Error::runtime(
                    "typed array array method was routed incorrectly",
                )));
            }
        };
        Some(result)
    }

    pub(super) fn typed_array_iterable_values(
        &mut self,
        source: &Value,
    ) -> Result<Option<Vec<Value>>> {
        if matches!(source, Value::Undefined | Value::Null) {
            return Ok(None);
        }
        self.symbol_constructor_value()?;
        let Some(symbol) = self.iterator_symbol() else {
            return Ok(None);
        };
        let lookup = PropertyLookup::from_key("[Symbol.iterator]", PropertyKey::symbol(symbol));
        let Some(method) = self.get_method(source, lookup)? else {
            return Ok(None);
        };
        let mut iterator = self.get_iterator_from_method(source, &method)?;
        let mut values = Vec::new();
        loop {
            match self.iterator_step(&mut iterator)? {
                IteratorStep::Value(value) => {
                    if values.len() >= self.limits.max_object_properties {
                        return Err(self.iterator_close_on_error(
                            &mut iterator,
                            Error::limit(TYPED_ARRAY_LENGTH_ERROR),
                        ));
                    }
                    values.push(value);
                }
                IteratorStep::Done => return Ok(Some(values)),
                IteratorStep::Abrupt(completion) => {
                    return completion.into_result().map(|_| None);
                }
            }
        }
    }

    pub(super) fn typed_array_receiver(&self, value: &Value) -> Result<(ObjectId, TypedArrayView)> {
        let (id, view) = self.typed_array_branded_receiver(value)?;
        if view.is_out_of_bounds() {
            return Err(Error::type_error(TYPED_ARRAY_RECEIVER_ERROR));
        }
        Ok((id, view))
    }

    pub(super) fn typed_array_branded_receiver(
        &self,
        value: &Value,
    ) -> Result<(ObjectId, TypedArrayView)> {
        let Value::Object(id) = value else {
            return Err(Error::type_error(TYPED_ARRAY_RECEIVER_ERROR));
        };
        let Some(view) = self.objects.typed_array(*id)? else {
            return Err(Error::type_error(TYPED_ARRAY_RECEIVER_ERROR));
        };
        Ok((*id, view))
    }

    fn eval_typed_array_to_string_tag(&mut self, this_value: &Value) -> Result<Value> {
        let Value::Object(id) = this_value else {
            return Ok(Value::Undefined);
        };
        let Some(view) = self.objects.typed_array(*id)? else {
            return Ok(Value::Undefined);
        };
        self.heap_string_value(view.element_kind().name())
    }

    fn eval_typed_array_iterator(
        &mut self,
        this_value: &Value,
        target: ArrayIterationTarget,
    ) -> Result<Value> {
        self.typed_array_receiver(this_value)?;
        self.create_array_iterator(this_value, target)
    }

    fn eval_typed_array_set(&mut self, args: &[Value], this_value: &Value) -> Result<Value> {
        let (target_id, target) = self.typed_array_receiver(this_value)?;
        target.ensure_mutable()?;
        let target_length = target.length();
        let source = args.first().cloned().unwrap_or(Value::Undefined);
        let offset_value = args.get(1).unwrap_or(&Value::Undefined);
        let offset_number = self.to_integer_or_infinity(offset_value)?;
        if !offset_number.is_finite() || offset_number < 0.0 {
            return Err(Error::exception(
                ErrorName::RangeError,
                TYPED_ARRAY_OFFSET_RANGE_ERROR,
            ));
        }
        let offset =
            Self::finite_nonnegative_integer_to_usize(offset_number, TYPED_ARRAY_LENGTH_ERROR)?;
        if target.is_out_of_bounds() {
            return Err(Error::type_error(TYPED_ARRAY_RECEIVER_ERROR));
        }
        if let Value::Object(source_id) = source
            && let Some(source_view) = self.objects.typed_array(source_id)?
        {
            return self.set_typed_array_from_typed_array(
                &target,
                target_length,
                offset,
                &source_view,
            );
        }
        let source = self.object_to_object(&source)?;
        let _source_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&source))?;
        let length_value = self.get_named(&source, "length")?;
        let source_length =
            Self::length_to_usize(self.to_length(&length_value)?, TYPED_ARRAY_LENGTH_ERROR)?;
        Self::ensure_typed_array_set_range(offset, source_length, target_length)?;
        for source_index in 0..source_length {
            self.step()?;
            let value = self.get_named(&source, &source_index.to_string())?;
            let element = self.convert_typed_array_element_value(target.element_kind(), &value)?;
            let target_index = offset
                .checked_add(source_index)
                .ok_or_else(|| Error::limit(TYPED_ARRAY_LENGTH_ERROR))?;
            // A later resize may invalidate this slot without suppressing
            // subsequent source property effects.
            self.objects
                .set_typed_array_value(target_id, target_index, &element)?;
        }
        Ok(Value::Undefined)
    }

    fn set_typed_array_from_typed_array(
        &mut self,
        target: &TypedArrayView,
        target_length: usize,
        offset: usize,
        source: &TypedArrayView,
    ) -> Result<Value> {
        if source.is_out_of_bounds() {
            return Err(Error::type_error(super::TYPED_ARRAY_SOURCE_ERROR));
        }
        if source.element_kind().content_type() != target.element_kind().content_type() {
            return Err(Error::type_error(TYPED_ARRAY_CONTENT_TYPE_ERROR));
        }
        if source.element_kind() == target.element_kind() {
            let source_length = source.length();
            let source_bytes = self.typed_array_view_bytes(source)?;
            Self::ensure_typed_array_set_range(offset, source_length, target_length)?;
            let bytes_per_element = target.element_kind().bytes_per_element();
            let target_start = offset
                .checked_mul(bytes_per_element)
                .ok_or_else(|| Error::limit(TYPED_ARRAY_LENGTH_ERROR))?;
            let target_end = target_start
                .checked_add(source_bytes.len())
                .ok_or_else(|| Error::limit(TYPED_ARRAY_LENGTH_ERROR))?;
            target.with_bytes_mut(|target_bytes| {
                let Some(destination) = target_bytes.get_mut(target_start..target_end) else {
                    return Err(Error::runtime("typed array set index is out of bounds"));
                };
                destination.copy_from_slice(&source_bytes);
                Ok(())
            })?;
            return Ok(Value::Undefined);
        }
        let values = self.typed_array_view_values(source)?;
        Self::ensure_typed_array_set_range(offset, values.len(), target_length)?;
        let target_kind = target.element_kind();
        let bytes_per_element = target_kind.bytes_per_element();
        target.with_bytes_mut(|target_bytes| {
            for (source_index, value) in values.iter().enumerate() {
                let target_index = offset
                    .checked_add(source_index)
                    .ok_or_else(|| Error::limit(TYPED_ARRAY_LENGTH_ERROR))?;
                let target_offset = target_index
                    .checked_mul(bytes_per_element)
                    .ok_or_else(|| Error::limit(TYPED_ARRAY_LENGTH_ERROR))?;
                let encoded = target_kind.encode(value)?;
                target_kind.write_encoded(target_bytes, target_offset, &encoded)?;
            }
            Ok(())
        })?;
        Ok(Value::Undefined)
    }

    fn ensure_typed_array_set_range(
        offset: usize,
        source_length: usize,
        target_length: usize,
    ) -> Result<()> {
        let end = offset
            .checked_add(source_length)
            .ok_or_else(|| Error::exception(ErrorName::RangeError, TYPED_ARRAY_SET_RANGE_ERROR))?;
        if end <= target_length {
            return Ok(());
        }
        Err(Error::exception(
            ErrorName::RangeError,
            TYPED_ARRAY_SET_RANGE_ERROR,
        ))
    }

    fn eval_typed_array_fill(&mut self, args: &[Value], this_value: &Value) -> Result<Value> {
        let (_, view) = self.typed_array_receiver(this_value)?;
        view.ensure_mutable()?;
        let length = view.length();
        let value = args.first().unwrap_or(&Value::Undefined);
        let element = self.convert_typed_array_element_value(view.element_kind(), value)?;
        let start = self.typed_array_relative_index(args.get(1), length, 0)?;
        let end = self.typed_array_relative_index(args.get(2), length, length)?;
        if view.is_out_of_bounds() {
            return Err(Error::type_error(TYPED_ARRAY_RECEIVER_ERROR));
        }
        let kind = view.element_kind();
        let encoded = kind.encode(&element)?;
        let bytes_per_element = kind.bytes_per_element();
        view.with_bytes_mut(|bytes| {
            let current_length = bytes.len() / bytes_per_element;
            for index in start..end {
                self.step()?;
                if index >= current_length {
                    return Err(Error::type_error(TYPED_ARRAY_RECEIVER_ERROR));
                }
                let offset = index
                    .checked_mul(bytes_per_element)
                    .ok_or_else(|| Error::limit(TYPED_ARRAY_LENGTH_ERROR))?;
                kind.write_encoded(bytes, offset, &encoded)?;
            }
            Ok(())
        })?;
        Ok(this_value.clone())
    }

    pub(super) fn typed_array_relative_index(
        &mut self,
        value: Option<&Value>,
        length: usize,
        default: usize,
    ) -> Result<usize> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(default);
        };
        let relative = self.to_integer_or_infinity(value)?;
        if relative == f64::NEG_INFINITY {
            return Ok(0);
        }
        if relative == f64::INFINITY {
            return Ok(length);
        }
        let length_number = Self::typed_array_usize_number(length)?;
        let absolute = if relative < 0.0 {
            (length_number + relative).max(0.0)
        } else {
            relative.min(length_number)
        };
        Self::finite_nonnegative_integer_to_usize(absolute, TYPED_ARRAY_LENGTH_ERROR)
    }

    fn eval_typed_array_sort(&mut self, args: &[Value], this_value: &Value) -> Result<Value> {
        let (_, view) = self.typed_array_receiver(this_value)?;
        view.ensure_mutable()?;
        if args
            .first()
            .is_some_and(|value| !matches!(value, Value::Undefined))
        {
            return self.eval_direct_array_sort(args, this_value);
        }
        let mut values = self.typed_array_view_values(&view)?;
        values.sort_by(typed_array_element_order);
        self.write_typed_array_values(this_value, &values)?;
        Ok(this_value.clone())
    }

    fn eval_typed_array_to_sorted(&mut self, args: &[Value], this_value: &Value) -> Result<Value> {
        let (_, view) = self.typed_array_receiver(this_value)?;
        let values = self.typed_array_view_values(&view)?;
        let result = self.create_typed_array_from_values(view.element_kind(), values)?;
        if args
            .first()
            .is_some_and(|value| !matches!(value, Value::Undefined))
        {
            self.eval_direct_array_sort(args, &result)?;
            return Ok(result);
        }
        let (_, result_view) = self.typed_array_receiver(&result)?;
        let mut elements = self.typed_array_view_values(&result_view)?;
        elements.sort_by(typed_array_element_order);
        self.write_typed_array_values(&result, &elements)?;
        Ok(result)
    }

    fn eval_typed_array_from(&mut self, args: &[Value], this_value: &Value) -> Result<Value> {
        self.ensure_typed_array_constructor(this_value)?;
        let source = args.first().cloned().unwrap_or(Value::Undefined);
        let mapping = args
            .get(1)
            .filter(|value| !matches!(value, Value::Undefined));
        if let Some(callback) = mapping
            && !self.semantic_is_callable(callback)?
        {
            return Err(Error::type_error(
                "TypedArray.from map function is not callable",
            ));
        }
        let callback_this = args.get(2).cloned().unwrap_or(Value::Undefined);
        if let Some(values) = self.typed_array_iterable_values(&source)? {
            return self.typed_array_create_from_values_with_constructor_mapped(
                this_value,
                values,
                None,
                mapping,
                &callback_this,
            );
        }
        let source = self.object_to_object(&source)?;
        let length_value = self.get_named(&source, "length")?;
        let length =
            Self::length_to_usize(self.to_length(&length_value)?, TYPED_ARRAY_LENGTH_ERROR)?;
        self.typed_array_create_from_array_like_mapped(
            this_value,
            &source,
            length,
            mapping,
            &callback_this,
        )
    }

    fn eval_typed_array_of(&mut self, args: &[Value], this_value: &Value) -> Result<Value> {
        self.ensure_typed_array_constructor(this_value)?;
        self.typed_array_create_from_values_with_constructor(this_value, args.to_vec(), None)
    }

    fn ensure_typed_array_constructor(&self, constructor: &Value) -> Result<()> {
        if self.semantic_is_constructor(constructor)? {
            return Ok(());
        }
        Err(Error::type_error(TYPED_ARRAY_SPECIES_ERROR))
    }

    pub(super) fn typed_array_species_create_from_values(
        &mut self,
        source: &Value,
        values: Vec<Value>,
    ) -> Result<Value> {
        let (_, view) = self.typed_array_branded_receiver(source)?;
        let constructor = self.typed_array_species_constructor(source, view.element_kind())?;
        self.typed_array_create_from_values_with_constructor(
            &constructor,
            values,
            Some(view.element_kind().content_type()),
        )
    }

    pub(super) fn typed_array_species_create_with_length(
        &mut self,
        source: &Value,
        length: usize,
        require_mutable: bool,
    ) -> Result<(Value, ObjectId, TypedArrayView)> {
        let (_, source_view) = self.typed_array_branded_receiver(source)?;
        let constructor =
            self.typed_array_species_constructor(source, source_view.element_kind())?;
        self.typed_array_create_with_constructor_length(
            &constructor,
            length,
            Some(source_view.element_kind().content_type()),
            require_mutable,
        )
    }

    pub(super) fn typed_array_species_construct(
        &mut self,
        source: &Value,
        args: &[Value],
    ) -> Result<Value> {
        let (_, source_view) = self.typed_array_branded_receiver(source)?;
        let constructor =
            self.typed_array_species_constructor(source, source_view.element_kind())?;
        let result = self.semantic_construct(&constructor, args, constructor.clone())?;
        let (_, result_view) = self
            .typed_array_receiver(&result)
            .map_err(|_| Error::type_error(TYPED_ARRAY_RESULT_ERROR))?;
        Self::ensure_typed_array_content_type(
            source_view.element_kind(),
            result_view.element_kind(),
        )?;
        Ok(result)
    }

    fn typed_array_species_constructor(
        &mut self,
        source: &Value,
        default_kind: TypedArrayElementKind,
    ) -> Result<Value> {
        let default = self.typed_array_constructor_value(default_kind)?;
        let constructor = self.get_named(source, "constructor")?;
        if matches!(constructor, Value::Undefined) {
            return Ok(default);
        }
        if self.semantic_object_ref(&constructor)?.is_none() {
            return Err(Error::type_error(TYPED_ARRAY_SPECIES_ERROR));
        }
        let symbol_constructor = self.symbol_constructor_value()?;
        let species_symbol = self.get_named(&symbol_constructor, SPECIES_PROPERTY)?;
        let Value::Symbol(species_symbol) = species_symbol else {
            return Err(Error::runtime("Symbol.species is not initialized"));
        };
        let lookup = PropertyLookup::from_key(
            SPECIES_SYMBOL_DISPLAY,
            PropertyKey::symbol(species_symbol.id()),
        );
        let species = self.get(&constructor, lookup)?;
        if matches!(species, Value::Undefined | Value::Null) {
            return Ok(default);
        }
        self.ensure_typed_array_constructor(&species)?;
        Ok(species)
    }

    fn typed_array_create_from_values_with_constructor(
        &mut self,
        constructor: &Value,
        values: Vec<Value>,
        expected_content_type: Option<TypedArrayContentType>,
    ) -> Result<Value> {
        self.typed_array_create_from_values_with_constructor_mapped(
            constructor,
            values,
            expected_content_type,
            None,
            &Value::Undefined,
        )
    }

    fn typed_array_create_from_values_with_constructor_mapped(
        &mut self,
        constructor: &Value,
        values: Vec<Value>,
        expected_content_type: Option<TypedArrayContentType>,
        mapping: Option<&Value>,
        callback_this: &Value,
    ) -> Result<Value> {
        let (result, id, view) = self.typed_array_create_with_constructor_length(
            constructor,
            values.len(),
            expected_content_type,
            true,
        )?;
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&result))?;
        for (index, value) in values.into_iter().enumerate() {
            let value = if let Some(callback) = mapping {
                let call_args = [value, Self::typed_array_usize_value(index)?];
                self.call_value(callback, &call_args, callback_this.clone())?
            } else {
                value
            };
            let element = self.convert_typed_array_element_value(view.element_kind(), &value)?;
            self.objects.set_typed_array_value(id, index, &element)?;
        }
        Ok(result)
    }

    fn typed_array_create_with_constructor_length(
        &mut self,
        constructor: &Value,
        length: usize,
        expected_content_type: Option<TypedArrayContentType>,
        require_mutable: bool,
    ) -> Result<(Value, ObjectId, TypedArrayView)> {
        let length_value = Self::typed_array_usize_value(length)?;
        let result = self.semantic_construct(
            constructor,
            std::slice::from_ref(&length_value),
            constructor.clone(),
        )?;
        let (id, view) = self
            .typed_array_receiver(&result)
            .map_err(|_| Error::type_error(TYPED_ARRAY_RESULT_ERROR))?;
        if view.length() < length {
            return Err(Error::type_error(TYPED_ARRAY_RESULT_ERROR));
        }
        if expected_content_type
            .is_some_and(|expected| expected != view.element_kind().content_type())
        {
            return Err(Error::type_error(TYPED_ARRAY_CONTENT_TYPE_ERROR));
        }
        if require_mutable {
            view.ensure_mutable()?;
        }
        Ok((result, id, view))
    }

    fn typed_array_create_from_array_like_mapped(
        &mut self,
        constructor: &Value,
        source: &Value,
        length: usize,
        mapping: Option<&Value>,
        callback_this: &Value,
    ) -> Result<Value> {
        let (result, id, view) =
            self.typed_array_create_with_constructor_length(constructor, length, None, true)?;
        let _scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, [source, &result])?;
        for index in 0..length {
            self.step()?;
            let value = self.get_named(source, &index.to_string())?;
            let value = if let Some(callback) = mapping {
                let call_args = [value, Self::typed_array_usize_value(index)?];
                self.call_value(callback, &call_args, callback_this.clone())?
            } else {
                value
            };
            let element = self.convert_typed_array_element_value(view.element_kind(), &value)?;
            self.objects.set_typed_array_value(id, index, &element)?;
        }
        Ok(result)
    }

    fn write_typed_array_values(&mut self, target: &Value, values: &[Value]) -> Result<()> {
        let (_, view) = self.typed_array_receiver(target)?;
        let kind = view.element_kind();
        let bytes_per_element = kind.bytes_per_element();
        view.with_bytes_mut(|bytes| {
            for (index, value) in values.iter().enumerate() {
                self.step()?;
                let offset = index
                    .checked_mul(bytes_per_element)
                    .ok_or_else(|| Error::limit(TYPED_ARRAY_LENGTH_ERROR))?;
                let encoded = kind.encode(value)?;
                kind.write_encoded(bytes, offset, &encoded)?;
            }
            Ok(())
        })
    }

    pub(super) fn typed_array_usize_value(value: usize) -> Result<Value> {
        Self::typed_array_usize_number(value).map(Value::Number)
    }

    pub(super) fn typed_array_usize_number(value: usize) -> Result<f64> {
        let value = u32::try_from(value).map_err(|_| Error::limit(TYPED_ARRAY_LENGTH_ERROR))?;
        Ok(f64::from(value))
    }

    fn ensure_typed_array_content_type(
        expected: TypedArrayElementKind,
        actual: TypedArrayElementKind,
    ) -> Result<()> {
        if expected.content_type() == actual.content_type() {
            return Ok(());
        }
        Err(Error::type_error(TYPED_ARRAY_CONTENT_TYPE_ERROR))
    }
}

fn typed_array_numeric_order(left: f64, right: f64) -> Ordering {
    match (left.is_nan(), right.is_nan()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) if left == 0.0 && right == 0.0 => {
            right.is_sign_negative().cmp(&left.is_sign_negative())
        }
        (false, false) => left.partial_cmp(&right).unwrap_or(Ordering::Equal),
    }
}

fn typed_array_element_order(left: &Value, right: &Value) -> Ordering {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => typed_array_numeric_order(*left, *right),
        (Value::BigInt(left), Value::BigInt(right)) => left.cmp(right),
        _ => Ordering::Equal,
    }
}
