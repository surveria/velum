mod date_time_format;
mod duration_format;
mod formatting;
mod options;

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

const DATE_TIME_FORMAT_TAG: &str = "Intl.DateTimeFormat";
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
        let supported = self.create_native_function(
            intl_kind(IntlFunctionKind::SupportedValuesOf),
            Value::Undefined,
        )?;
        self.define_non_enumerable_object_property(namespace, "supportedValuesOf", supported)?;
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
                self.construct_intl_date_time_format(args)
            }
            IntlFunctionKind::DateTimeFormatFormat => {
                self.eval_intl_date_time_format(args, this_value, false)
            }
            IntlFunctionKind::DateTimeFormatFormatToParts => {
                self.eval_intl_date_time_format(args, this_value, true)
            }
            IntlFunctionKind::DateTimeFormatResolvedOptions => {
                self.eval_intl_date_time_format_resolved_options(this_value)
            }
            IntlFunctionKind::DurationFormatConstructor => self.construct_intl_duration_format(),
            IntlFunctionKind::DurationFormatFormat => {
                self.eval_intl_duration_format(args, this_value)
            }
            IntlFunctionKind::SupportedValuesOf => self.eval_intl_supported_values_of(args),
        }
    }

    fn intl_date_time_format_constructor_value(&mut self) -> Result<Value> {
        self.intl_constructor_value(
            IntlFunctionKind::DateTimeFormatConstructor,
            DATE_TIME_FORMAT_TAG,
            &[
                ("format", IntlFunctionKind::DateTimeFormatFormat),
                (
                    "formatToParts",
                    IntlFunctionKind::DateTimeFormatFormatToParts,
                ),
                (
                    "resolvedOptions",
                    IntlFunctionKind::DateTimeFormatResolvedOptions,
                ),
            ],
        )
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
