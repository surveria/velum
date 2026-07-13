use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        object::{
            DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyKey,
            PropertyUpdate, PropertyWritable,
        },
    },
    value::{ErrorName, ObjectId, Value},
};

mod calendar_types;
mod duration;
mod install;
mod plain_date;

use crate::runtime::call::RuntimeCallArgs;
use crate::runtime::native::{TEMPORAL_NAME, TemporalFunctionKind};

const SYMBOL_TO_STRING_TAG_PROPERTY: &str = "toStringTag";

impl Context {
    pub(in crate::runtime::native) fn construct_temporal_kind(
        &mut self,
        kind: TemporalFunctionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        match kind {
            TemporalFunctionKind::Constructor => self.construct_temporal_duration(args),
            TemporalFunctionKind::InstantConstructor => self.construct_temporal_instant(args),
            TemporalFunctionKind::PlainDateConstructor => self.construct_temporal_plain_date(args),
            TemporalFunctionKind::PlainDateTimeConstructor => {
                self.construct_temporal_plain_date_time(args)
            }
            TemporalFunctionKind::PlainMonthDayConstructor => {
                self.construct_temporal_plain_month_day(args)
            }
            TemporalFunctionKind::PlainTimeConstructor => self.construct_temporal_plain_time(args),
            TemporalFunctionKind::PlainYearMonthConstructor => {
                self.construct_temporal_plain_year_month(args)
            }
            TemporalFunctionKind::ZonedDateTimeConstructor => {
                self.construct_temporal_zoned_date_time(args)
            }
            _ => Err(Error::type_error("Temporal method is not a constructor")),
        }
    }

    pub(in crate::runtime::native) fn temporal_namespace_value(&mut self) -> Result<Value> {
        if let Some(binding) = self.get_binding(TEMPORAL_NAME) {
            return binding.value(TEMPORAL_NAME);
        }
        self.object_constructor_value()?;
        let constructor_key = self.object_constructor_property_key()?;
        let namespace = self.objects.create_with_prototype_id(
            None,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let duration = self.temporal_duration_constructor_value()?;
        self.define_non_enumerable_object_property(namespace, "Duration", duration)?;
        let instant = self.temporal_instant_constructor_value()?;
        self.define_non_enumerable_object_property(namespace, "Instant", instant)?;
        let plain_date = self.temporal_plain_date_constructor_value()?;
        self.define_non_enumerable_object_property(namespace, "PlainDate", plain_date)?;
        let plain_date_time = self.temporal_plain_date_time_constructor_value()?;
        self.define_non_enumerable_object_property(namespace, "PlainDateTime", plain_date_time)?;
        let plain_month_day = self.temporal_plain_month_day_constructor_value()?;
        self.define_non_enumerable_object_property(namespace, "PlainMonthDay", plain_month_day)?;
        let plain_time = self.temporal_plain_time_constructor_value()?;
        self.define_non_enumerable_object_property(namespace, "PlainTime", plain_time)?;
        let plain_year_month = self.temporal_plain_year_month_constructor_value()?;
        self.define_non_enumerable_object_property(namespace, "PlainYearMonth", plain_year_month)?;
        let zoned_date_time = self.temporal_zoned_date_time_constructor_value()?;
        self.define_non_enumerable_object_property(namespace, "ZonedDateTime", zoned_date_time)?;
        self.define_temporal_to_string_tag(namespace, TEMPORAL_NAME)?;
        let value = Value::Object(namespace);
        self.insert_global_builtin(TEMPORAL_NAME, value.clone())?;
        Ok(value)
    }

    fn define_temporal_to_string_tag(&mut self, object: ObjectId, tag: &str) -> Result<()> {
        let constructor = self.symbol_constructor_value()?;
        let symbol = self.get_named(&constructor, SYMBOL_TO_STRING_TAG_PROPERTY)?;
        let Value::Symbol(symbol) = symbol else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let value = self.heap_string_value(tag)?;
        self.objects.define_property(
            object,
            PropertyKey::symbol(symbol.id()),
            SYMBOL_TO_STRING_TAG_PROPERTY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }
}

fn temporal_error(error: temporal_rs::TemporalError) -> Error {
    let message = error.to_string();
    match error.kind() {
        temporal_rs::error::ErrorKind::Type => Error::exception(ErrorName::TypeError, message),
        temporal_rs::error::ErrorKind::Range | temporal_rs::error::ErrorKind::Syntax => {
            Error::exception(ErrorName::RangeError, message)
        }
        temporal_rs::error::ErrorKind::Generic => Error::exception(ErrorName::Base, message),
        temporal_rs::error::ErrorKind::Assert => Error::runtime(message),
    }
}

const fn temporal_kind(kind: TemporalFunctionKind) -> crate::runtime::native::NativeFunctionKind {
    crate::runtime::native::NativeFunctionKind::Temporal(kind)
}
