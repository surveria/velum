mod date_time_format;
mod date_time_locale;
mod date_time_range;
mod date_time_text;
mod date_time_types;
mod display_names;
mod duration_format;
mod formatting;
mod list_format;
mod locale;
mod number_digits;
mod number_format;
mod number_formatting;
mod number_options;
mod number_range;
mod number_rounding;
mod options;
mod segmenter;

pub(in crate::runtime::native) use date_time_locale::DateLocaleDefaults;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::{INTL_NAME, IntlFunctionKind, NativeFunctionKind},
        object::PropertyEnumerable,
    },
    value::{ObjectId, Value},
};

const DURATION_FORMAT_TAG: &str = "Intl.DurationFormat";

impl Context {
    pub(in crate::runtime::native) fn intl_namespace_value(&mut self) -> Result<Value> {
        if let Some(binding) = self.get_binding(INTL_NAME) {
            return binding.value(INTL_NAME);
        }
        let constructor_key = self.object_constructor_property_key()?;
        let object_prototype = self.objects.object_prototype_id(
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let namespace = self.objects.create_with_prototype_id(
            Some(object_prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let date_time_format = self.intl_date_time_format_constructor_value()?;
        self.define_non_enumerable_object_property(namespace, "DateTimeFormat", date_time_format)?;
        let duration_format = self.intl_duration_format_constructor_value()?;
        self.define_non_enumerable_object_property(namespace, "DurationFormat", duration_format)?;
        let display_names = self.intl_display_names_constructor_value()?;
        self.define_non_enumerable_object_property(namespace, "DisplayNames", display_names)?;
        let locale = self.intl_locale_constructor_value()?;
        self.define_non_enumerable_object_property(namespace, "Locale", locale)?;
        let list_format = self.intl_list_format_constructor_value()?;
        self.define_non_enumerable_object_property(namespace, "ListFormat", list_format)?;
        let segmenter = self.intl_segmenter_constructor_value()?;
        self.define_non_enumerable_object_property(namespace, "Segmenter", segmenter)?;
        let number_format = self.intl_number_format_constructor_value()?;
        self.define_non_enumerable_object_property(namespace, "NumberFormat", number_format)?;
        for (name, kind, tag) in [
            (
                "Collator",
                IntlFunctionKind::CollatorConstructor,
                "Intl.Collator",
            ),
            (
                "PluralRules",
                IntlFunctionKind::PluralRulesConstructor,
                "Intl.PluralRules",
            ),
            (
                "RelativeTimeFormat",
                IntlFunctionKind::RelativeTimeFormatConstructor,
                "Intl.RelativeTimeFormat",
            ),
        ] {
            let constructor = self.intl_constructor_value(kind, tag, &[])?;
            self.define_non_enumerable_object_property(namespace, name, constructor)?;
        }
        let supported = self.create_native_function(
            intl_kind(IntlFunctionKind::SupportedValuesOf),
            Value::Undefined,
        )?;
        self.define_non_enumerable_object_property(namespace, "supportedValuesOf", supported)?;
        let canonical_locales = self.create_native_function(
            intl_kind(IntlFunctionKind::GetCanonicalLocales),
            Value::Undefined,
        )?;
        self.define_non_enumerable_object_property(
            namespace,
            "getCanonicalLocales",
            canonical_locales,
        )?;
        self.define_intl_to_string_tag(namespace, INTL_NAME)?;
        let value = Value::Object(namespace);
        self.insert_global_builtin(INTL_NAME, value.clone())?;
        Ok(value)
    }

    pub(in crate::runtime::native) fn construct_intl_kind(
        &mut self,
        kind: IntlFunctionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        match kind {
            IntlFunctionKind::DateTimeFormatConstructor => {
                self.construct_intl_date_time_format(args)
            }
            IntlFunctionKind::DurationFormatConstructor => self.construct_intl_duration_format(),
            IntlFunctionKind::LocaleConstructor => self.construct_intl_locale(args),
            IntlFunctionKind::ListFormatConstructor => self.construct_intl_list_format(args),
            IntlFunctionKind::SegmenterConstructor => self.construct_intl_segmenter(args),
            IntlFunctionKind::NumberFormatConstructor => self.construct_intl_number_format(args),
            IntlFunctionKind::DisplayNamesConstructor => self.construct_intl_display_names(args),
            IntlFunctionKind::CollatorConstructor
            | IntlFunctionKind::PluralRulesConstructor
            | IntlFunctionKind::RelativeTimeFormatConstructor => self.construct_intl_stub(kind),
            _ => Err(Error::type_error("Intl method is not a constructor")),
        }
    }

    pub(in crate::runtime::native) fn eval_intl_native_function_kind(
        &mut self,
        kind: IntlFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        match kind {
            IntlFunctionKind::DateTimeFormatConstructor => {
                self.call_intl_date_time_format(args, this_value)
            }
            IntlFunctionKind::DateTimeFormatFormatGetter => {
                self.eval_intl_date_time_format_getter(this_value)
            }
            IntlFunctionKind::DateTimeFormatBoundFormat(formatter) => {
                self.eval_intl_date_time_format_bound(args, formatter)
            }
            IntlFunctionKind::DateTimeFormatFormatToParts => {
                self.eval_intl_date_time_format(args, this_value, true)
            }
            IntlFunctionKind::DateTimeFormatResolvedOptions => {
                self.eval_intl_date_time_format_resolved_options(this_value)
            }
            IntlFunctionKind::DateTimeFormatFormatRange => {
                self.eval_intl_date_time_format_range(args, this_value, false)
            }
            IntlFunctionKind::DateTimeFormatFormatRangeToParts => {
                self.eval_intl_date_time_format_range(args, this_value, true)
            }
            IntlFunctionKind::DurationFormatConstructor => self.construct_intl_duration_format(),
            IntlFunctionKind::DurationFormatFormat => {
                self.eval_intl_duration_format(args, this_value)
            }
            IntlFunctionKind::LocaleConstructor => {
                Err(Error::type_error("Intl.Locale requires new"))
            }
            IntlFunctionKind::LocaleAccessor(kind) => {
                self.eval_intl_locale_accessor(kind, this_value)
            }
            IntlFunctionKind::LocaleMethod(kind) => self.eval_intl_locale_method(kind, this_value),
            IntlFunctionKind::ListFormatConstructor => {
                Err(Error::type_error("Intl.ListFormat requires new"))
            }
            IntlFunctionKind::ListFormatFormat => {
                self.eval_intl_list_format(args, this_value, false)
            }
            IntlFunctionKind::ListFormatFormatToParts => {
                self.eval_intl_list_format(args, this_value, true)
            }
            IntlFunctionKind::ListFormatResolvedOptions => {
                self.eval_intl_list_format_resolved_options(this_value)
            }
            IntlFunctionKind::ListFormatSupportedLocalesOf => {
                self.eval_intl_list_format_supported_locales(args)
            }
            IntlFunctionKind::SegmenterConstructor => {
                Err(Error::type_error("Intl.Segmenter requires new"))
            }
            IntlFunctionKind::SegmenterSegment => {
                self.eval_intl_segmenter_segment(args, this_value)
            }
            IntlFunctionKind::SegmenterResolvedOptions => {
                self.eval_intl_segmenter_resolved_options(this_value)
            }
            IntlFunctionKind::SegmenterSupportedLocalesOf => {
                self.eval_intl_segmenter_supported_locales(args)
            }
            IntlFunctionKind::SegmentsIterator => self.eval_intl_segments_iterator(this_value),
            IntlFunctionKind::SegmentsContaining => {
                self.eval_intl_segments_containing(args, this_value)
            }
            IntlFunctionKind::SegmentIteratorNext => {
                self.eval_intl_segment_iterator_next(this_value)
            }
            IntlFunctionKind::SupportedValuesOf => self.eval_intl_supported_values_of(args),
            IntlFunctionKind::GetCanonicalLocales => self.eval_intl_get_canonical_locales(args),
            IntlFunctionKind::DisplayNamesConstructor => {
                Err(Error::type_error("Intl.DisplayNames requires new"))
            }
            IntlFunctionKind::DisplayNamesOf => self.eval_intl_display_names_of(args, this_value),
            IntlFunctionKind::DisplayNamesResolvedOptions => {
                self.eval_intl_display_names_resolved_options(this_value)
            }
            IntlFunctionKind::NumberFormatConstructor => {
                self.call_intl_number_format(args, this_value)
            }
            IntlFunctionKind::NumberFormatFormatGetter => {
                self.eval_intl_number_format_getter(this_value)
            }
            IntlFunctionKind::NumberFormatBoundFormat(formatter) => {
                self.eval_intl_number_format(args, formatter, false)
            }
            IntlFunctionKind::NumberFormatFormatToParts => {
                self.eval_intl_number_format_method(args, this_value, true)
            }
            IntlFunctionKind::NumberFormatResolvedOptions => {
                self.eval_intl_number_format_resolved_options(this_value)
            }
            IntlFunctionKind::NumberFormatFormatRange => {
                self.eval_intl_number_format_range(args, this_value, false)
            }
            IntlFunctionKind::NumberFormatFormatRangeToParts => {
                self.eval_intl_number_format_range(args, this_value, true)
            }
            IntlFunctionKind::DateTimeFormatSupportedLocalesOf
            | IntlFunctionKind::NumberFormatSupportedLocalesOf => {
                self.eval_intl_number_format_supported_locales(args)
            }
            IntlFunctionKind::CollatorConstructor
            | IntlFunctionKind::PluralRulesConstructor
            | IntlFunctionKind::RelativeTimeFormatConstructor => self.construct_intl_stub(kind),
            IntlFunctionKind::PluralRulesSelect
            | IntlFunctionKind::PluralRulesSelectRange
            | IntlFunctionKind::PluralRulesResolvedOptions
            | IntlFunctionKind::PluralRulesSupportedLocalesOf
            | IntlFunctionKind::RelativeTimeFormatFormat
            | IntlFunctionKind::RelativeTimeFormatFormatToParts
            | IntlFunctionKind::RelativeTimeFormatResolvedOptions
            | IntlFunctionKind::RelativeTimeFormatSupportedLocalesOf => {
                Err(Error::runtime("Intl formatter method is not initialized"))
            }
        }
    }

    fn intl_duration_format_constructor_value(&mut self) -> Result<Value> {
        self.intl_constructor_value(
            IntlFunctionKind::DurationFormatConstructor,
            DURATION_FORMAT_TAG,
            &[("format", IntlFunctionKind::DurationFormatFormat)],
        )
    }

    fn intl_constructor_value(
        &mut self,
        constructor_kind: IntlFunctionKind,
        tag: &str,
        methods: &[(&str, IntlFunctionKind)],
    ) -> Result<Value> {
        let kind = intl_kind(constructor_kind);
        if let Some(id) = self.native_function_id(kind) {
            return Ok(Value::NativeFunction(id));
        }
        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype = self.intl_prototype(constructor.clone())?;
        let name = self.native_function_name_value(kind)?;
        self.push_native_function_with_id(id, kind, Value::Object(prototype), name)?;
        for (method_name, method_kind) in methods {
            let method = self.create_native_function(intl_kind(*method_kind), Value::Undefined)?;
            self.define_non_enumerable_object_property(prototype, method_name, method)?;
        }
        self.define_intl_to_string_tag(prototype, tag)?;
        Ok(constructor)
    }

    fn intl_prototype(&mut self, constructor: Value) -> Result<ObjectId> {
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
        self.define_non_enumerable_object_property(prototype, "constructor", constructor)?;
        Ok(prototype)
    }

    fn intl_constructor_prototype(&mut self, kind: IntlFunctionKind) -> Result<ObjectId> {
        let constructor = match kind {
            IntlFunctionKind::DateTimeFormatConstructor => {
                self.intl_date_time_format_constructor_value()?
            }
            IntlFunctionKind::DurationFormatConstructor => {
                self.intl_duration_format_constructor_value()?
            }
            IntlFunctionKind::LocaleConstructor => self.intl_locale_constructor_value()?,
            IntlFunctionKind::ListFormatConstructor => self.intl_list_format_constructor_value()?,
            IntlFunctionKind::SegmenterConstructor => self.intl_segmenter_constructor_value()?,
            IntlFunctionKind::DisplayNamesConstructor => {
                self.intl_display_names_constructor_value()?
            }
            IntlFunctionKind::CollatorConstructor => {
                self.intl_constructor_value(kind, "Intl.Collator", &[])?
            }
            IntlFunctionKind::NumberFormatConstructor => {
                self.intl_number_format_constructor_value()?
            }
            IntlFunctionKind::PluralRulesConstructor => {
                self.intl_constructor_value(kind, "Intl.PluralRules", &[])?
            }
            IntlFunctionKind::RelativeTimeFormatConstructor => {
                self.intl_constructor_value(kind, "Intl.RelativeTimeFormat", &[])?
            }
            _ => return Err(Error::runtime("Intl kind has no constructor prototype")),
        };
        let Value::NativeFunction(id) = constructor else {
            return Err(Error::runtime("Intl constructor is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime(
                "Intl constructor prototype is not an object",
            )),
        }
    }

    fn construct_intl_stub(&mut self, kind: IntlFunctionKind) -> Result<Value> {
        let prototype = self.intl_constructor_prototype(kind)?;
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_with_prototype(
            Some(prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn define_intl_to_string_tag(&mut self, object: ObjectId, tag: &str) -> Result<()> {
        let constructor = self.symbol_constructor_value()?;
        let symbol = self.get_named(&constructor, "toStringTag")?;
        let Value::Symbol(symbol) = symbol else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let value = self.heap_string_value(tag)?;
        self.objects.define_property(
            object,
            crate::runtime::object::PropertyKey::symbol(symbol.id()),
            "toStringTag",
            crate::runtime::object::PropertyUpdate::Data(
                crate::runtime::object::DataPropertyUpdate::new(
                    Some(value),
                    Some(crate::runtime::object::PropertyWritable::No),
                    Some(PropertyEnumerable::No),
                    Some(crate::runtime::object::PropertyConfigurable::Yes),
                ),
            ),
            self.limits.max_object_properties,
        )
    }
}

const fn intl_kind(kind: IntlFunctionKind) -> NativeFunctionKind {
    NativeFunctionKind::Intl(kind)
}
