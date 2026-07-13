use std::str::FromStr;

use num_traits::ToPrimitive;
use temporal_rs::{
    Calendar, Instant, PlainDate, PlainDateTime, PlainMonthDay, PlainTime, PlainYearMonth,
    TimeZone, ZonedDateTime,
    options::{DisplayCalendar, Overflow},
};

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::TemporalFunctionKind,
        object::{
            AccessorPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyUpdate,
            TemporalValue,
        },
    },
    value::{ErrorName, NativeFunctionId, ObjectId, Value},
};

use super::{temporal_error, temporal_kind};

const PLAIN_DATE_TAG: &str = "Temporal.PlainDate";
const ZONED_DATE_TIME_TAG: &str = "Temporal.ZonedDateTime";
const PLAIN_DATE_RECEIVER_ERROR: &str = "Temporal.PlainDate method requires a PlainDate receiver";
const ZONED_DATE_TIME_RECEIVER_ERROR: &str =
    "Temporal.ZonedDateTime method requires a ZonedDateTime receiver";

impl Context {
    pub(super) fn temporal_plain_date_constructor_value(&mut self) -> Result<Value> {
        self.temporal_calendar_constructor(
            TemporalFunctionKind::PlainDateConstructor,
            PLAIN_DATE_TAG,
            &[
                ("from", TemporalFunctionKind::PlainDateFrom),
                ("compare", TemporalFunctionKind::PlainDateCompare),
            ],
            &[
                ("year", TemporalFunctionKind::PlainDatePrototypeYear),
                ("month", TemporalFunctionKind::PlainDatePrototypeMonth),
                (
                    "monthCode",
                    TemporalFunctionKind::PlainDatePrototypeMonthCode,
                ),
                ("day", TemporalFunctionKind::PlainDatePrototypeDay),
                (
                    "calendarId",
                    TemporalFunctionKind::PlainDatePrototypeCalendarId,
                ),
                ("era", TemporalFunctionKind::PlainDatePrototypeEra),
                ("eraYear", TemporalFunctionKind::PlainDatePrototypeEraYear),
                (
                    "dayOfWeek",
                    TemporalFunctionKind::PlainDatePrototypeDayOfWeek,
                ),
                (
                    "dayOfYear",
                    TemporalFunctionKind::PlainDatePrototypeDayOfYear,
                ),
                (
                    "weekOfYear",
                    TemporalFunctionKind::PlainDatePrototypeWeekOfYear,
                ),
                (
                    "yearOfWeek",
                    TemporalFunctionKind::PlainDatePrototypeYearOfWeek,
                ),
                (
                    "daysInWeek",
                    TemporalFunctionKind::PlainDatePrototypeDaysInWeek,
                ),
                (
                    "daysInMonth",
                    TemporalFunctionKind::PlainDatePrototypeDaysInMonth,
                ),
                (
                    "daysInYear",
                    TemporalFunctionKind::PlainDatePrototypeDaysInYear,
                ),
                (
                    "monthsInYear",
                    TemporalFunctionKind::PlainDatePrototypeMonthsInYear,
                ),
                (
                    "inLeapYear",
                    TemporalFunctionKind::PlainDatePrototypeInLeapYear,
                ),
            ],
            &[
                ("with", TemporalFunctionKind::PlainDatePrototypeWith),
                (
                    "withCalendar",
                    TemporalFunctionKind::PlainDatePrototypeWithCalendar,
                ),
                ("add", TemporalFunctionKind::PlainDatePrototypeAdd),
                ("subtract", TemporalFunctionKind::PlainDatePrototypeSubtract),
                ("until", TemporalFunctionKind::PlainDatePrototypeUntil),
                ("since", TemporalFunctionKind::PlainDatePrototypeSince),
                ("equals", TemporalFunctionKind::PlainDatePrototypeEquals),
                (
                    "toPlainDateTime",
                    TemporalFunctionKind::PlainDatePrototypeToPlainDateTime,
                ),
                (
                    "toZonedDateTime",
                    TemporalFunctionKind::PlainDatePrototypeToZonedDateTime,
                ),
                (
                    "toPlainYearMonth",
                    TemporalFunctionKind::PlainDatePrototypeToPlainYearMonth,
                ),
                (
                    "toPlainMonthDay",
                    TemporalFunctionKind::PlainDatePrototypeToPlainMonthDay,
                ),
                ("toString", TemporalFunctionKind::PlainDatePrototypeToString),
                ("toJSON", TemporalFunctionKind::PlainDatePrototypeToJson),
                (
                    "toLocaleString",
                    TemporalFunctionKind::PlainDatePrototypeToLocaleString,
                ),
                ("valueOf", TemporalFunctionKind::PlainDatePrototypeValueOf),
            ],
        )
    }

    pub(super) fn temporal_zoned_date_time_constructor_value(&mut self) -> Result<Value> {
        self.temporal_calendar_constructor(
            TemporalFunctionKind::ZonedDateTimeConstructor,
            ZONED_DATE_TIME_TAG,
            super::zoned_date_time::STATIC_METHODS,
            super::zoned_date_time::ACCESSORS,
            super::zoned_date_time::METHODS,
        )
    }

    pub(super) fn temporal_plain_date_time_constructor_value(&mut self) -> Result<Value> {
        self.temporal_calendar_constructor(
            TemporalFunctionKind::PlainDateTimeConstructor,
            "Temporal.PlainDateTime",
            super::plain_date_time::STATIC_METHODS,
            super::plain_date_time::ACCESSORS,
            super::plain_date_time::METHODS,
        )
    }

    pub(super) fn temporal_plain_month_day_constructor_value(&mut self) -> Result<Value> {
        self.temporal_calendar_constructor(
            TemporalFunctionKind::PlainMonthDayConstructor,
            "Temporal.PlainMonthDay",
            super::plain_month_day::STATIC_METHODS,
            super::plain_month_day::ACCESSORS,
            super::plain_month_day::METHODS,
        )
    }

    pub(super) fn temporal_plain_year_month_constructor_value(&mut self) -> Result<Value> {
        self.temporal_calendar_constructor(
            TemporalFunctionKind::PlainYearMonthConstructor,
            "Temporal.PlainYearMonth",
            super::plain_year_month::STATIC_METHODS,
            super::plain_year_month::ACCESSORS,
            super::plain_year_month::METHODS,
        )
    }

    pub(super) fn temporal_instant_constructor_value(&mut self) -> Result<Value> {
        self.temporal_calendar_constructor(
            TemporalFunctionKind::InstantConstructor,
            "Temporal.Instant",
            super::instant::STATIC_METHODS,
            super::instant::ACCESSORS,
            super::instant::METHODS,
        )
    }

    pub(super) fn temporal_plain_time_constructor_value(&mut self) -> Result<Value> {
        self.temporal_calendar_constructor(
            TemporalFunctionKind::PlainTimeConstructor,
            "Temporal.PlainTime",
            super::plain_time::STATIC_METHODS,
            super::plain_time::ACCESSORS,
            super::plain_time::METHODS,
        )
    }

    fn temporal_calendar_constructor(
        &mut self,
        constructor_kind: TemporalFunctionKind,
        tag: &str,
        static_methods: &[(&str, TemporalFunctionKind)],
        accessors: &[(&str, TemporalFunctionKind)],
        methods: &[(&str, TemporalFunctionKind)],
    ) -> Result<Value> {
        let kind = temporal_kind(constructor_kind);
        if let Some(id) = self.native_function_id(kind) {
            return Ok(Value::NativeFunction(id));
        }
        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype = self.temporal_ordinary_prototype(constructor.clone())?;
        let name = self.native_function_name_value(kind)?;
        self.push_native_function_with_id(id, kind, Value::Object(prototype), name)?;
        self.install_temporal_static_methods(id, static_methods)?;
        self.install_temporal_prototype_members(prototype, accessors, methods)?;
        self.define_temporal_to_string_tag(prototype, tag)?;
        Ok(constructor)
    }

    fn temporal_ordinary_prototype(&mut self, constructor: Value) -> Result<ObjectId> {
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

    fn install_temporal_static_methods(
        &mut self,
        constructor: NativeFunctionId,
        methods: &[(&str, TemporalFunctionKind)],
    ) -> Result<()> {
        for (name, kind) in methods {
            let function = self.create_native_function(temporal_kind(*kind), Value::Undefined)?;
            let key = self.intern_property_key(name)?;
            self.native_function_mut(constructor)?
                .properties_mut()
                .define_builtin(key, function, PropertyEnumerable::No)?;
        }
        Ok(())
    }

    fn install_temporal_prototype_members(
        &mut self,
        prototype: ObjectId,
        accessors: &[(&str, TemporalFunctionKind)],
        methods: &[(&str, TemporalFunctionKind)],
    ) -> Result<()> {
        for (name, kind) in accessors {
            let getter = self.create_native_function(temporal_kind(*kind), Value::Undefined)?;
            let key = self.intern_property_key(name)?;
            self.objects.define_property(
                prototype,
                key,
                name,
                PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                    Some(getter),
                    None,
                    Some(PropertyEnumerable::No),
                    Some(PropertyConfigurable::Yes),
                )),
                self.limits.max_object_properties,
            )?;
        }
        for (name, kind) in methods {
            let method = self.create_native_function(temporal_kind(*kind), Value::Undefined)?;
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        Ok(())
    }

    pub(in crate::runtime::native) fn construct_temporal_plain_date(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
        let year = self.temporal_i32(values.first(), "PlainDate year")?;
        let month = self.temporal_u8(values.get(1), "PlainDate month")?;
        let day = self.temporal_u8(values.get(2), "PlainDate day")?;
        let calendar = Self::temporal_calendar_identifier(values.get(3))?;
        let date = PlainDate::try_new(year, month, day, calendar).map_err(temporal_error)?;
        self.create_plain_date_value(date)
    }

    pub(in crate::runtime::native) fn construct_temporal_zoned_date_time(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
        let Some(epoch) = values.first() else {
            return Err(Error::type_error(
                "ZonedDateTime epochNanoseconds is required",
            ));
        };
        let bigint = self.to_bigint(epoch)?;
        let nanos = bigint.to_string().parse::<i128>().map_err(|_| {
            Error::exception(
                ErrorName::RangeError,
                "ZonedDateTime epochNanoseconds is out of range",
            )
        })?;
        let zone_value = values.get(1).cloned().unwrap_or(Value::Undefined);
        let zone_text = zone_value
            .string_text()
            .ok_or_else(|| Error::type_error("ZonedDateTime timeZone must be a string"))?;
        let time_zone = TimeZone::try_from_identifier_str(zone_text).map_err(temporal_error)?;
        let calendar = Self::temporal_calendar_identifier(values.get(2))?;
        let zoned = ZonedDateTime::try_new(nanos, time_zone, calendar).map_err(temporal_error)?;
        self.create_zoned_date_time_value(zoned)
    }

    pub(in crate::runtime::native) fn construct_temporal_plain_date_time(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
        let date_time = PlainDateTime::try_new(
            self.temporal_i32(values.first(), "PlainDateTime year")?,
            self.temporal_u8(values.get(1), "PlainDateTime month")?,
            self.temporal_u8(values.get(2), "PlainDateTime day")?,
            self.temporal_optional_u8(values.get(3), "PlainDateTime hour")?,
            self.temporal_optional_u8(values.get(4), "PlainDateTime minute")?,
            self.temporal_optional_u8(values.get(5), "PlainDateTime second")?,
            self.temporal_optional_u16(values.get(6), "PlainDateTime millisecond")?,
            self.temporal_optional_u16(values.get(7), "PlainDateTime microsecond")?,
            self.temporal_optional_u16(values.get(8), "PlainDateTime nanosecond")?,
            Self::temporal_calendar_identifier(values.get(9))?,
        )
        .map_err(temporal_error)?;
        self.create_temporal_calendar_value(
            TemporalValue::PlainDateTime(date_time),
            TemporalFunctionKind::PlainDateTimeConstructor,
        )
    }

    pub(in crate::runtime::native) fn construct_temporal_plain_month_day(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
        let month = self.temporal_u8(values.first(), "PlainMonthDay month")?;
        let day = self.temporal_u8(values.get(1), "PlainMonthDay day")?;
        let calendar = Self::temporal_calendar_identifier(values.get(2))?;
        let reference_year = self.temporal_optional_i32(values.get(3), "reference year")?;
        let month_day = PlainMonthDay::new_with_overflow(
            month,
            day,
            calendar,
            Overflow::Reject,
            reference_year,
        )
        .map_err(temporal_error)?;
        self.create_temporal_calendar_value(
            TemporalValue::PlainMonthDay(month_day),
            TemporalFunctionKind::PlainMonthDayConstructor,
        )
    }

    pub(in crate::runtime::native) fn construct_temporal_plain_year_month(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
        let year = self.temporal_i32(values.first(), "PlainYearMonth year")?;
        let month = self.temporal_u8(values.get(1), "PlainYearMonth month")?;
        let calendar = Self::temporal_calendar_identifier(values.get(2))?;
        let reference_day = self.temporal_optional_u8_value(values.get(3), "reference day")?;
        let year_month = PlainYearMonth::try_new(year, month, reference_day, calendar)
            .map_err(temporal_error)?;
        self.create_temporal_calendar_value(
            TemporalValue::PlainYearMonth(year_month),
            TemporalFunctionKind::PlainYearMonthConstructor,
        )
    }

    pub(in crate::runtime::native) fn construct_temporal_instant(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let Some(epoch) = args.as_slice().first() else {
            return Err(Error::type_error(
                "Temporal.Instant requires epochNanoseconds",
            ));
        };
        let bigint = self.to_bigint(epoch)?;
        let nanos = bigint.to_string().parse::<i128>().map_err(|_| {
            Error::exception(
                ErrorName::RangeError,
                "Temporal.Instant epochNanoseconds is out of range",
            )
        })?;
        let instant = Instant::try_new(nanos).map_err(temporal_error)?;
        self.create_temporal_calendar_value(
            TemporalValue::Instant(instant),
            TemporalFunctionKind::InstantConstructor,
        )
    }

    pub(in crate::runtime::native) fn construct_temporal_plain_time(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
        let time = PlainTime::try_new(
            self.temporal_optional_u8(values.first(), "PlainTime hour")?,
            self.temporal_optional_u8(values.get(1), "PlainTime minute")?,
            self.temporal_optional_u8(values.get(2), "PlainTime second")?,
            self.temporal_optional_u16(values.get(3), "PlainTime millisecond")?,
            self.temporal_optional_u16(values.get(4), "PlainTime microsecond")?,
            self.temporal_optional_u16(values.get(5), "PlainTime nanosecond")?,
        )
        .map_err(temporal_error)?;
        self.create_temporal_calendar_value(
            TemporalValue::PlainTime(time),
            TemporalFunctionKind::PlainTimeConstructor,
        )
    }

    fn temporal_i32(&mut self, value: Option<&Value>, name: &str) -> Result<i32> {
        let number = self.temporal_integer(value, name)?;
        number.to_i32().ok_or_else(|| {
            Error::exception(ErrorName::RangeError, format!("{name} is out of range"))
        })
    }

    fn temporal_u8(&mut self, value: Option<&Value>, name: &str) -> Result<u8> {
        let number = self.temporal_integer(value, name)?;
        number.to_u8().ok_or_else(|| {
            Error::exception(ErrorName::RangeError, format!("{name} is out of range"))
        })
    }

    fn temporal_optional_u8(&mut self, value: Option<&Value>, name: &str) -> Result<u8> {
        self.temporal_optional_u8_value(value, name)
            .map(Option::unwrap_or_default)
    }

    fn temporal_optional_u8_value(
        &mut self,
        value: Option<&Value>,
        name: &str,
    ) -> Result<Option<u8>> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(None);
        };
        self.temporal_u8(Some(value), name).map(Some)
    }

    fn temporal_optional_u16(&mut self, value: Option<&Value>, name: &str) -> Result<u16> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(0);
        };
        let number = self.temporal_integer(Some(value), name)?;
        number.to_u16().ok_or_else(|| {
            Error::exception(ErrorName::RangeError, format!("{name} is out of range"))
        })
    }

    fn temporal_optional_i32(&mut self, value: Option<&Value>, name: &str) -> Result<Option<i32>> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(None);
        };
        self.temporal_i32(Some(value), name).map(Some)
    }

    fn temporal_integer(&mut self, value: Option<&Value>, name: &str) -> Result<f64> {
        let value = value.cloned().unwrap_or(Value::Undefined);
        let number = self.to_number(&value)?;
        if number.is_finite() {
            return Ok(number.trunc());
        }
        Err(Error::exception(
            ErrorName::RangeError,
            format!("{name} must be finite"),
        ))
    }

    pub(super) fn temporal_calendar(&self, value: Option<&Value>) -> Result<Calendar> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(Calendar::default());
        };
        if let Value::Object(id) = value {
            let calendar = match self.objects.temporal_value(*id)? {
                Some(TemporalValue::PlainDate(date)) => Some(date.calendar()),
                Some(TemporalValue::PlainDateTime(date_time)) => Some(date_time.calendar()),
                Some(TemporalValue::PlainMonthDay(month_day)) => Some(month_day.calendar()),
                Some(TemporalValue::PlainYearMonth(year_month)) => Some(year_month.calendar()),
                Some(TemporalValue::ZonedDateTime(zoned)) => Some(zoned.calendar()),
                Some(
                    TemporalValue::Duration(_)
                    | TemporalValue::Instant(_)
                    | TemporalValue::PlainTime(_),
                )
                | None => None,
            };
            if let Some(calendar) = calendar {
                return Ok(calendar.clone());
            }
        }
        let Some(text) = value.string_text() else {
            return Err(Error::type_error("Temporal calendar must be a string"));
        };
        Self::temporal_calendar_from_text(text)
    }

    fn temporal_calendar_identifier(value: Option<&Value>) -> Result<Calendar> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(Calendar::default());
        };
        let Some(text) = value.string_text() else {
            return Err(Error::type_error("Temporal calendar must be a string"));
        };
        Self::temporal_calendar_from_text(text)
    }

    pub(super) fn temporal_calendar_from_text(text: &str) -> Result<Calendar> {
        if text.eq_ignore_ascii_case("islamic") {
            return Err(Error::exception(
                ErrorName::RangeError,
                "The islamic calendar alias is not valid for Temporal",
            ));
        }
        Calendar::from_str(text).map_err(temporal_error)
    }

    pub(super) fn create_plain_date_value(&mut self, date: PlainDate) -> Result<Value> {
        let prototype =
            self.temporal_constructor_prototype(TemporalFunctionKind::PlainDateConstructor)?;
        self.objects.create_temporal_object(
            TemporalValue::PlainDate(date),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(super) fn create_zoned_date_time_value(&mut self, zoned: ZonedDateTime) -> Result<Value> {
        let prototype =
            self.temporal_constructor_prototype(TemporalFunctionKind::ZonedDateTimeConstructor)?;
        self.objects.create_temporal_object(
            TemporalValue::ZonedDateTime(zoned),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(super) fn create_temporal_calendar_value(
        &mut self,
        value: TemporalValue,
        kind: TemporalFunctionKind,
    ) -> Result<Value> {
        let prototype = self.temporal_constructor_prototype(kind)?;
        self.objects
            .create_temporal_object(value, prototype, self.limits.max_objects)
    }

    fn temporal_constructor_prototype(&mut self, kind: TemporalFunctionKind) -> Result<ObjectId> {
        let constructor = match kind {
            TemporalFunctionKind::PlainDateConstructor => {
                self.temporal_plain_date_constructor_value()?
            }
            TemporalFunctionKind::ZonedDateTimeConstructor => {
                self.temporal_zoned_date_time_constructor_value()?
            }
            TemporalFunctionKind::PlainDateTimeConstructor => {
                self.temporal_plain_date_time_constructor_value()?
            }
            TemporalFunctionKind::PlainMonthDayConstructor => {
                self.temporal_plain_month_day_constructor_value()?
            }
            TemporalFunctionKind::PlainYearMonthConstructor => {
                self.temporal_plain_year_month_constructor_value()?
            }
            TemporalFunctionKind::InstantConstructor => {
                self.temporal_instant_constructor_value()?
            }
            TemporalFunctionKind::PlainTimeConstructor => {
                self.temporal_plain_time_constructor_value()?
            }
            _ => return Err(Error::runtime("Temporal constructor kind has no prototype")),
        };
        let Value::NativeFunction(id) = constructor else {
            return Err(Error::runtime("Temporal constructor is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime(
                "Temporal constructor prototype is not an object",
            )),
        }
    }

    pub(super) fn eval_temporal_calendar_kind(
        &mut self,
        kind: TemporalFunctionKind,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        if kind.is_plain_date() {
            return self.eval_plain_date_kind(kind, args, receiver);
        }
        if kind.is_plain_date_time() {
            return self.eval_plain_date_time_kind(kind, args, receiver);
        }
        if kind.is_plain_time() {
            return self.eval_plain_time_kind(kind, args, receiver);
        }
        if kind.is_plain_month_day() {
            return self.eval_plain_month_day_kind(kind, args, receiver);
        }
        if kind.is_plain_year_month() {
            return self.eval_plain_year_month_kind(kind, args, receiver);
        }
        if kind.is_instant() {
            return self.eval_instant_kind(kind, args, receiver);
        }
        if kind.is_zoned_date_time() {
            return self.eval_zoned_date_time_kind(kind, args, receiver);
        }
        if kind.is_temporal_now() {
            return self.eval_temporal_now_kind(kind, args);
        }
        match kind {
            TemporalFunctionKind::PlainDateConstructor => Err(Error::type_error(
                "Temporal.PlainDate constructor requires 'new'",
            )),
            TemporalFunctionKind::PlainDateFrom => self.eval_plain_date_from(args),
            TemporalFunctionKind::PlainDatePrototypeYear => {
                self.plain_date_number(receiver, PlainDate::year)
            }
            TemporalFunctionKind::PlainDatePrototypeMonth => {
                self.plain_date_number(receiver, PlainDate::month)
            }
            TemporalFunctionKind::PlainDatePrototypeDay => {
                self.plain_date_number(receiver, PlainDate::day)
            }
            TemporalFunctionKind::PlainDatePrototypeCalendarId => {
                let date = self.plain_date_receiver(receiver)?;
                self.heap_string_value(date.calendar().identifier())
            }
            TemporalFunctionKind::PlainDatePrototypeToString
            | TemporalFunctionKind::PlainDatePrototypeToJson => {
                let date = self.plain_date_receiver(receiver)?;
                self.heap_string_value(&date.to_ixdtf_string(DisplayCalendar::Auto))
            }
            TemporalFunctionKind::PlainDatePrototypeValueOf => Err(Error::type_error(
                "Temporal.PlainDate cannot be converted to a primitive",
            )),
            TemporalFunctionKind::PlainDateTimeConstructor => Err(Error::type_error(
                "Temporal.PlainDateTime constructor requires 'new'",
            )),
            TemporalFunctionKind::PlainMonthDayConstructor => Err(Error::type_error(
                "Temporal.PlainMonthDay constructor requires 'new'",
            )),
            TemporalFunctionKind::PlainYearMonthConstructor => Err(Error::type_error(
                "Temporal.PlainYearMonth constructor requires 'new'",
            )),
            TemporalFunctionKind::PlainTimeConstructor => Err(Error::type_error(
                "Temporal.PlainTime constructor requires 'new'",
            )),
            _ => Err(Error::runtime(
                "Temporal calendar function kind was not handled",
            )),
        }
    }

    fn eval_plain_date_from(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let Some(value) = args.as_slice().first() else {
            return Err(Error::type_error(
                "Temporal.PlainDate.from requires an argument",
            ));
        };
        let date = if let Ok(date) = self.plain_date_receiver(value) {
            date
        } else if let Some(text) = value.string_text() {
            PlainDate::from_utf8(text.as_bytes()).map_err(temporal_error)?
        } else {
            return Err(Error::type_error(
                "Temporal.PlainDate.from argument must be a string or PlainDate",
            ));
        };
        self.create_plain_date_value(date)
    }

    pub(super) fn plain_date_receiver(&self, value: &Value) -> Result<PlainDate> {
        let Value::Object(id) = value else {
            return Err(Error::type_error(PLAIN_DATE_RECEIVER_ERROR));
        };
        match self.objects.temporal_value(*id)? {
            Some(TemporalValue::PlainDate(date)) => Ok(date.clone()),
            _ => Err(Error::type_error(PLAIN_DATE_RECEIVER_ERROR)),
        }
    }

    pub(super) fn zoned_date_time_receiver(&self, value: &Value) -> Result<ZonedDateTime> {
        let Value::Object(id) = value else {
            return Err(Error::type_error(ZONED_DATE_TIME_RECEIVER_ERROR));
        };
        match self.objects.temporal_value(*id)? {
            Some(TemporalValue::ZonedDateTime(zoned)) => Ok(zoned.clone()),
            _ => Err(Error::type_error(ZONED_DATE_TIME_RECEIVER_ERROR)),
        }
    }

    fn plain_date_number<T>(&self, receiver: &Value, getter: fn(&PlainDate) -> T) -> Result<Value>
    where
        T: ToPrimitive,
    {
        let date = self.plain_date_receiver(receiver)?;
        let number = getter(&date)
            .to_f64()
            .ok_or_else(|| Error::runtime("PlainDate field cannot become Number"))?;
        Ok(Value::Number(number))
    }
}
