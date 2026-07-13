use unicode_segmentation::UnicodeSegmentation;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::IntlFunctionKind,
        object::{
            DataPropertyUpdate, IntlValue, OwnPropertyDescriptor, PropertyConfigurable,
            PropertyEnumerable, PropertyKey, PropertyUpdate, PropertyWritable, SegmentBoundary,
            SegmentIteratorValue, SegmenterValue, SegmentsValue,
        },
        property::DynamicPropertyKey,
    },
    value::{ErrorName, NativeFunctionId, ObjectId, Value},
};

const SEGMENTER_TAG: &str = "Intl.Segmenter";
const SEGMENT_ITERATOR_TAG: &str = "Segmenter String Iterator";
const SUPPORTED_LOCALES_OF: &str = "supportedLocalesOf";
const DEFAULT_LOCALE: &str = "en-US";
const SEGMENTS_PROTOTYPE_ANCHOR: &str = "\0IntlSegmentsPrototype";
const SEGMENT_ITERATOR_PROTOTYPE_ANCHOR: &str = "\0IntlSegmentIteratorPrototype";
const ITERATOR_SYMBOL_DISPLAY: &str = "[Symbol.iterator]";

impl Context {
    pub(in crate::runtime) fn intl_segmenter_constructor_value(&mut self) -> Result<Value> {
        let constructor_kind = IntlFunctionKind::SegmenterConstructor;
        let native_kind = super::intl_kind(constructor_kind);
        let existed = self.native_function_id(native_kind).is_some();
        let constructor = self.intl_constructor_value(
            constructor_kind,
            SEGMENTER_TAG,
            &[
                ("segment", IntlFunctionKind::SegmenterSegment),
                (
                    "resolvedOptions",
                    IntlFunctionKind::SegmenterResolvedOptions,
                ),
            ],
        )?;
        if existed {
            return Ok(constructor);
        }
        let Value::NativeFunction(constructor_id) = constructor else {
            return Err(Error::runtime("Intl.Segmenter constructor is not native"));
        };
        self.install_segmenter_static_method(constructor_id)?;
        Ok(Value::NativeFunction(constructor_id))
    }

    pub(super) fn construct_intl_segmenter(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let requested = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let locales = self.intl_locale_list(&requested)?;
        let options = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if matches!(options, Value::Null) {
            return Err(Error::type_error("Intl.Segmenter options cannot be null"));
        }
        let _matcher = self.segmenter_option(
            &options,
            "localeMatcher",
            &["lookup", "best fit"],
            "best fit",
        )?;
        let granularity = self.segmenter_option(
            &options,
            "granularity",
            &["grapheme", "word", "sentence"],
            "grapheme",
        )?;
        let locale = locales
            .into_iter()
            .find(|locale| segmenter_locale_is_supported(locale))
            .unwrap_or_else(|| DEFAULT_LOCALE.to_owned());
        let prototype = self.intl_constructor_prototype(IntlFunctionKind::SegmenterConstructor)?;
        self.objects.create_intl_object(
            IntlValue::Segmenter(Box::new(SegmenterValue {
                locale,
                granularity,
            })),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(super) fn eval_intl_segmenter_segment(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let segmenter = self.segmenter_receiver(this_value)?;
        let input = self.to_utf16_string(args.as_slice().first().unwrap_or(&Value::Undefined))?;
        let boundaries = segment_boundaries(&input, &segmenter.granularity)?;
        let prototype = self.segments_prototype_id()?;
        self.objects.create_intl_object(
            IntlValue::Segments(Box::new(SegmentsValue {
                input,
                granularity: segmenter.granularity,
                boundaries,
            })),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(super) fn eval_intl_segmenter_resolved_options(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let segmenter = self.segmenter_receiver(this_value)?;
        let locale = self.heap_string_value(&segmenter.locale)?;
        let granularity = self.heap_string_value(&segmenter.granularity)?;
        self.create_intl_data_object(vec![("locale", locale), ("granularity", granularity)])
    }

    pub(super) fn eval_intl_segmenter_supported_locales(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let requested = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let locales = self.intl_locale_list(&requested)?;
        let options = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if matches!(options, Value::Null) {
            return Err(Error::type_error("Intl locale options cannot be null"));
        }
        let _matcher = self.segmenter_option(
            &options,
            "localeMatcher",
            &["lookup", "best fit"],
            "best fit",
        )?;
        let mut supported = Vec::new();
        for locale in locales {
            if segmenter_locale_is_supported(&locale) {
                supported.push(self.heap_string_value(&locale)?);
            }
        }
        self.create_array_from_elements(supported)
    }

    pub(super) fn eval_intl_segments_iterator(&mut self, this_value: &Value) -> Result<Value> {
        let segments = self.segments_receiver_id(this_value)?;
        let prototype = self.segment_iterator_prototype_id()?;
        self.objects.create_intl_object(
            IntlValue::SegmentIterator(Box::new(SegmentIteratorValue {
                segments,
                next_index: 0,
            })),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(super) fn eval_intl_segments_containing(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let segments = self.segments_receiver(this_value)?;
        let index_value = args.as_slice().first().unwrap_or(&Value::Undefined);
        let integer = self.to_integer_or_infinity(index_value)?;
        if !integer.is_finite() || integer < 0.0 {
            return Ok(Value::Undefined);
        }
        let index = Self::finite_nonnegative_integer_to_usize(
            integer,
            "Intl.Segments index exceeded supported range",
        )?;
        let Some(boundary) = segments
            .boundaries
            .iter()
            .find(|boundary| boundary.start <= index && index < boundary.end)
        else {
            return Ok(Value::Undefined);
        };
        self.segment_data_value(&segments, boundary)
    }

    pub(super) fn eval_intl_segment_iterator_next(&mut self, this_value: &Value) -> Result<Value> {
        let iterator_id = self.segment_iterator_receiver_id(this_value)?;
        let iterator = self.segment_iterator_receiver(this_value)?;
        let segments = self.segments_receiver(&Value::Object(iterator.segments))?;
        let Some(boundary) = segments.boundaries.get(iterator.next_index) else {
            return self.create_iterator_result_object(Value::Undefined, true);
        };
        let next_index = iterator
            .next_index
            .checked_add(1)
            .ok_or_else(|| Error::limit("Intl.Segmenter iterator cursor overflowed"))?;
        let Some(IntlValue::SegmentIterator(state)) = self.objects.intl_value_mut(iterator_id)?
        else {
            return Err(Error::runtime(
                "Intl.Segmenter iterator receiver disappeared",
            ));
        };
        state.next_index = next_index;
        let value = self.segment_data_value(&segments, boundary)?;
        self.create_iterator_result_object(value, false)
    }

    fn segment_data_value(
        &mut self,
        segments: &SegmentsValue,
        boundary: &SegmentBoundary,
    ) -> Result<Value> {
        let Some(units) = segments.input.get(boundary.start..boundary.end) else {
            return Err(Error::runtime(
                "Intl.Segmenter produced an invalid boundary",
            ));
        };
        let segment = self.heap_utf16_string_value(units)?;
        let index = Value::Number(Self::usize_to_number(
            boundary.start,
            "Intl.Segmenter index exceeded supported range",
        )?);
        let input = self.heap_utf16_string_value(&segments.input)?;
        let mut fields = vec![("segment", segment), ("index", index), ("input", input)];
        if segments.granularity == "word" {
            fields.push(("isWordLike", Value::Bool(boundary.is_word_like)));
        }
        self.create_intl_data_object(fields)
    }

    fn segmenter_receiver(&self, this_value: &Value) -> Result<SegmenterValue> {
        let Value::Object(id) = this_value else {
            return Err(Error::type_error("Intl.Segmenter receiver is invalid"));
        };
        let Some(IntlValue::Segmenter(value)) = self.objects.intl_value(*id)? else {
            return Err(Error::type_error("Intl.Segmenter receiver is invalid"));
        };
        Ok(value.as_ref().clone())
    }

    fn segments_receiver_id(&self, this_value: &Value) -> Result<ObjectId> {
        let Value::Object(id) = this_value else {
            return Err(Error::type_error("Intl.Segments receiver is invalid"));
        };
        if matches!(self.objects.intl_value(*id)?, Some(IntlValue::Segments(_))) {
            return Ok(*id);
        }
        Err(Error::type_error("Intl.Segments receiver is invalid"))
    }

    fn segments_receiver(&self, this_value: &Value) -> Result<SegmentsValue> {
        let id = self.segments_receiver_id(this_value)?;
        let Some(IntlValue::Segments(value)) = self.objects.intl_value(id)? else {
            return Err(Error::type_error("Intl.Segments receiver is invalid"));
        };
        Ok(value.as_ref().clone())
    }

    fn segment_iterator_receiver_id(&self, this_value: &Value) -> Result<ObjectId> {
        let Value::Object(id) = this_value else {
            return Err(Error::type_error(
                "Intl.Segment Iterator receiver is invalid",
            ));
        };
        if matches!(
            self.objects.intl_value(*id)?,
            Some(IntlValue::SegmentIterator(_))
        ) {
            return Ok(*id);
        }
        Err(Error::type_error(
            "Intl.Segment Iterator receiver is invalid",
        ))
    }

    fn segment_iterator_receiver(&self, this_value: &Value) -> Result<SegmentIteratorValue> {
        let id = self.segment_iterator_receiver_id(this_value)?;
        let Some(IntlValue::SegmentIterator(value)) = self.objects.intl_value(id)? else {
            return Err(Error::type_error(
                "Intl.Segment Iterator receiver is invalid",
            ));
        };
        Ok(value.as_ref().clone())
    }

    fn segmenter_option(
        &mut self,
        options: &Value,
        name: &str,
        allowed: &[&str],
        default: &str,
    ) -> Result<String> {
        if matches!(options, Value::Undefined) {
            return Ok(default.to_owned());
        }
        let value = self.get_named(options, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(default.to_owned());
        }
        let text = self.to_string(&value)?;
        if !allowed.contains(&text.as_str()) {
            return Err(Error::exception(
                ErrorName::RangeError,
                format!("{name} has an unsupported value"),
            ));
        }
        Ok(text)
    }

    fn install_segmenter_static_method(&mut self, constructor: NativeFunctionId) -> Result<()> {
        let method = self.create_native_function(
            super::intl_kind(IntlFunctionKind::SegmenterSupportedLocalesOf),
            Value::Undefined,
        )?;
        let key = self.intern_property_key(SUPPORTED_LOCALES_OF)?;
        self.define_native_function_property_key(
            constructor,
            SUPPORTED_LOCALES_OF,
            key,
            DataPropertyUpdate::new(
                Some(method),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            ),
        )
    }

    fn segments_prototype_id(&mut self) -> Result<ObjectId> {
        let holder = self.intl_constructor_prototype(IntlFunctionKind::SegmenterConstructor)?;
        if let Some(prototype) = self.intl_cached_prototype(holder, SEGMENTS_PROTOTYPE_ANCHOR)? {
            return Ok(prototype);
        }
        let constructor_key = self.object_constructor_property_key()?;
        let object_prototype = self.objects.object_prototype_id(
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let prototype = self.objects.create_with_prototype_id(
            Some(object_prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        self.install_intl_method(
            prototype,
            "containing",
            IntlFunctionKind::SegmentsContaining,
        )?;
        self.symbol_constructor_value()?;
        let Some(iterator_symbol) = self.iterator_symbol() else {
            return Err(Error::runtime("Symbol.iterator is not initialized"));
        };
        let iterator = self.create_native_function(
            super::intl_kind(IntlFunctionKind::SegmentsIterator),
            Value::Undefined,
        )?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(iterator_symbol),
            ITERATOR_SYMBOL_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(iterator),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        self.cache_intl_prototype(holder, SEGMENTS_PROTOTYPE_ANCHOR, prototype)?;
        Ok(prototype)
    }

    fn segment_iterator_prototype_id(&mut self) -> Result<ObjectId> {
        let holder = self.segments_prototype_id()?;
        if let Some(prototype) =
            self.intl_cached_prototype(holder, SEGMENT_ITERATOR_PROTOTYPE_ANCHOR)?
        {
            return Ok(prototype);
        }
        let parent = self.iterator_prototype_object_id()?;
        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.create_with_prototype_id(
            Some(parent),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        self.install_intl_method(prototype, "next", IntlFunctionKind::SegmentIteratorNext)?;
        self.define_intl_to_string_tag(prototype, SEGMENT_ITERATOR_TAG)?;
        self.cache_intl_prototype(holder, SEGMENT_ITERATOR_PROTOTYPE_ANCHOR, prototype)?;
        Ok(prototype)
    }

    fn intl_cached_prototype(
        &mut self,
        holder: ObjectId,
        anchor: &str,
    ) -> Result<Option<ObjectId>> {
        let key = self.intern_property_key(anchor)?;
        let property = DynamicPropertyKey::new(anchor.to_owned(), Some(key));
        let value = Value::Object(holder);
        let Some(OwnPropertyDescriptor::Data(descriptor)) =
            self.semantic_own_property_descriptor(&value, &property)?
        else {
            return Ok(None);
        };
        let Value::Object(prototype) = descriptor.value() else {
            return Err(Error::runtime("Intl intrinsic prototype anchor is invalid"));
        };
        Ok(Some(prototype))
    }

    fn cache_intl_prototype(
        &mut self,
        holder: ObjectId,
        anchor: &str,
        prototype: ObjectId,
    ) -> Result<()> {
        let key = self.intern_property_key(anchor)?;
        self.objects.define_property(
            holder,
            key,
            anchor,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(Value::Object(prototype)),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::No),
            )),
            self.limits.max_object_properties,
        )
    }

    fn install_intl_method(
        &mut self,
        prototype: ObjectId,
        name: &'static str,
        kind: IntlFunctionKind,
    ) -> Result<()> {
        let function = self.create_native_function(super::intl_kind(kind), Value::Undefined)?;
        self.define_non_enumerable_object_property(prototype, name, function)
    }
}

const fn segmenter_locale_is_supported(locale: &str) -> bool {
    !locale.eq_ignore_ascii_case("zxx") && !locale.eq_ignore_ascii_case("xyz")
}

fn segment_boundaries(input: &[u16], granularity: &str) -> Result<Vec<SegmentBoundary>> {
    let text = String::from_utf16_lossy(input);
    let segments = match granularity {
        "grapheme" => UnicodeSegmentation::grapheme_indices(text.as_str(), true)
            .map(|(start, segment)| (start, segment, false))
            .collect::<Vec<_>>(),
        "word" => UnicodeSegmentation::split_word_bound_indices(text.as_str())
            .map(|(start, segment)| {
                let is_word_like = UnicodeSegmentation::unicode_words(segment).next().is_some();
                (start, segment, is_word_like)
            })
            .collect::<Vec<_>>(),
        "sentence" => UnicodeSegmentation::split_sentence_bound_indices(text.as_str())
            .map(|(start, segment)| (start, segment, false))
            .collect::<Vec<_>>(),
        _ => return Err(Error::runtime("Intl.Segmenter granularity is invalid")),
    };
    let mut boundaries = Vec::with_capacity(segments.len());
    let mut byte_cursor = 0_usize;
    let mut utf16_cursor = 0_usize;
    for (byte_start, segment, is_word_like) in segments {
        if byte_start != byte_cursor {
            return Err(Error::runtime(
                "Intl.Segmenter produced a non-contiguous boundary",
            ));
        }
        let end = utf16_cursor
            .checked_add(segment.encode_utf16().count())
            .ok_or_else(|| Error::limit("Intl.Segmenter boundary overflowed"))?;
        boundaries.push(SegmentBoundary {
            start: utf16_cursor,
            end,
            is_word_like,
        });
        byte_cursor = byte_cursor
            .checked_add(segment.len())
            .ok_or_else(|| Error::limit("Intl.Segmenter byte boundary overflowed"))?;
        utf16_cursor = end;
    }
    if byte_cursor != text.len() || utf16_cursor != input.len() {
        return Err(Error::runtime(
            "Intl.Segmenter boundaries did not cover the input",
        ));
    }
    Ok(boundaries)
}
