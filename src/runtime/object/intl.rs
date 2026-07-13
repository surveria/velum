use crate::{
    error::Result,
    value::{ObjectId, Value},
};

use super::{Object, ObjectHeap};

#[derive(Debug, Clone, Default)]
pub(in crate::runtime) struct DateTimeFormatOptions {
    pub date_style: Option<String>,
    pub time_style: Option<String>,
    pub weekday: Option<String>,
    pub era: Option<String>,
    pub year: Option<String>,
    pub month: Option<String>,
    pub day: Option<String>,
    pub hour: Option<String>,
    pub minute: Option<String>,
    pub second: Option<String>,
    pub day_period: Option<String>,
    pub fractional_second_digits: Option<u8>,
    pub time_zone_name: Option<String>,
    pub hour_cycle: Option<String>,
    pub hour12: Option<bool>,
}

impl DateTimeFormatOptions {
    pub const fn has_explicit_date_fields(&self) -> bool {
        self.weekday.is_some()
            || self.era.is_some()
            || self.year.is_some()
            || self.month.is_some()
            || self.day.is_some()
    }

    pub const fn has_explicit_time_fields(&self) -> bool {
        self.hour.is_some()
            || self.minute.is_some()
            || self.second.is_some()
            || self.day_period.is_some()
            || self.fractional_second_digits.is_some()
            || self.time_zone_name.is_some()
    }

    fn storage_payload_bytes(&self) -> usize {
        [
            self.date_style.as_ref(),
            self.time_style.as_ref(),
            self.weekday.as_ref(),
            self.era.as_ref(),
            self.year.as_ref(),
            self.month.as_ref(),
            self.day.as_ref(),
            self.hour.as_ref(),
            self.minute.as_ref(),
            self.second.as_ref(),
            self.day_period.as_ref(),
            self.time_zone_name.as_ref(),
            self.hour_cycle.as_ref(),
        ]
        .into_iter()
        .flatten()
        .map(String::len)
        .sum()
    }
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct DateTimeFormatValue {
    pub locale: String,
    pub calendar: String,
    pub time_zone: String,
    pub options: DateTimeFormatOptions,
}

#[derive(Debug, Clone)]
pub(in crate::runtime) enum IntlValue {
    DateTimeFormat(Box<DateTimeFormatValue>),
    DurationFormat,
}

impl IntlValue {
    pub(super) fn storage_payload_bytes(&self) -> usize {
        match self {
            Self::DateTimeFormat(value) => value
                .locale
                .len()
                .saturating_add(value.calendar.len())
                .saturating_add(value.time_zone.len())
                .saturating_add(value.options.storage_payload_bytes()),
            Self::DurationFormat => 0,
        }
    }
}

impl ObjectHeap {
    pub(in crate::runtime) fn create_intl_object(
        &mut self,
        value: IntlValue,
        prototype: ObjectId,
        max_objects: usize,
    ) -> Result<Value> {
        let mut object = Object::ordinary();
        object.prototype = Some(prototype);
        object.intl_value = Some(value);
        self.push_object(object, max_objects).map(Value::Object)
    }

    pub(in crate::runtime) fn intl_value(&self, id: ObjectId) -> Result<Option<&IntlValue>> {
        Ok(self.object(id)?.intl_value.as_ref())
    }
}
