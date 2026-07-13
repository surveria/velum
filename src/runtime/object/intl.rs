use crate::{
    error::Result,
    runtime::trace::{StrongEdgeReference, StrongEdgeVisitor, VmObjectEdgeKind},
    value::{ObjectId, Value},
};
use std::mem::size_of;

use super::{Object, ObjectHeap};

#[derive(Debug, Clone, Default)]
pub(in crate::runtime) struct DateTimeFormatOptions {
    pub default_components: bool,
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
    pub numbering_system: String,
    pub time_zone: String,
    pub options: DateTimeFormatOptions,
    pub bound_format: Option<Value>,
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct NumberFormatValue {
    pub locale: String,
    pub numbering_system: String,
    pub style: String,
    pub currency: Option<String>,
    pub currency_display: String,
    pub currency_sign: String,
    pub unit: Option<String>,
    pub unit_display: String,
    pub minimum_integer_digits: u8,
    pub minimum_fraction_digits: u8,
    pub maximum_fraction_digits: u8,
    pub minimum_significant_digits: Option<u8>,
    pub maximum_significant_digits: Option<u8>,
    pub use_grouping: Option<String>,
    pub notation: String,
    pub compact_display: String,
    pub sign_display: String,
    pub rounding_increment: u16,
    pub rounding_mode: String,
    pub rounding_priority: String,
    pub trailing_zero_display: String,
    pub bound_format: Option<Value>,
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct LocaleValue {
    pub tag: String,
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct ListFormatValue {
    pub locale: String,
    pub list_type: String,
    pub style: String,
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct DisplayNamesValue {
    pub locale: String,
    pub style: String,
    pub display_type: String,
    pub missing_code_behavior: String,
    pub language_display: Option<String>,
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct PluralRulesValue {
    pub locale: String,
    pub rule_type: String,
    pub notation: String,
    pub compact_display: Option<String>,
    pub minimum_integer_digits: u8,
    pub minimum_fraction_digits: u8,
    pub maximum_fraction_digits: u8,
    pub minimum_significant_digits: Option<u8>,
    pub maximum_significant_digits: Option<u8>,
    pub rounding_increment: u16,
    pub rounding_mode: String,
    pub rounding_priority: String,
    pub trailing_zero_display: String,
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct RelativeTimeFormatValue {
    pub locale: String,
    pub numbering_system: String,
    pub style: String,
    pub numeric: String,
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct SegmenterValue {
    pub locale: String,
    pub granularity: String,
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct SegmentBoundary {
    pub start: usize,
    pub end: usize,
    pub is_word_like: bool,
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct SegmentsValue {
    pub input: Vec<u16>,
    pub granularity: String,
    pub boundaries: Vec<SegmentBoundary>,
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct SegmentIteratorValue {
    pub segments: ObjectId,
    pub next_index: usize,
}

impl NumberFormatValue {
    fn storage_payload_bytes(&self) -> usize {
        [
            Some(&self.locale),
            Some(&self.numbering_system),
            Some(&self.style),
            self.currency.as_ref(),
            Some(&self.currency_display),
            Some(&self.currency_sign),
            self.unit.as_ref(),
            Some(&self.unit_display),
            self.use_grouping.as_ref(),
            Some(&self.notation),
            Some(&self.compact_display),
            Some(&self.sign_display),
            Some(&self.rounding_mode),
            Some(&self.rounding_priority),
            Some(&self.trailing_zero_display),
        ]
        .into_iter()
        .flatten()
        .map(String::len)
        .sum()
    }
}

#[derive(Debug, Clone)]
pub(in crate::runtime) enum IntlValue {
    DateTime(Box<DateTimeFormatValue>),
    Duration,
    DisplayNames(Box<DisplayNamesValue>),
    List(Box<ListFormatValue>),
    Locale(Box<LocaleValue>),
    Number(Box<NumberFormatValue>),
    PluralRules(Box<PluralRulesValue>),
    RelativeTimeFormat(Box<RelativeTimeFormatValue>),
    Segmenter(Box<SegmenterValue>),
    Segments(Box<SegmentsValue>),
    SegmentIterator(Box<SegmentIteratorValue>),
}

impl IntlValue {
    pub(super) fn storage_payload_bytes(&self) -> usize {
        match self {
            Self::DateTime(value) => value
                .locale
                .len()
                .saturating_add(value.calendar.len())
                .saturating_add(value.numbering_system.len())
                .saturating_add(value.time_zone.len())
                .saturating_add(value.options.storage_payload_bytes()),
            Self::Duration | Self::SegmentIterator(_) => 0,
            Self::DisplayNames(value) => [
                Some(&value.locale),
                Some(&value.style),
                Some(&value.display_type),
                Some(&value.missing_code_behavior),
                value.language_display.as_ref(),
            ]
            .into_iter()
            .flatten()
            .map(String::len)
            .sum(),
            Self::List(value) => value
                .locale
                .len()
                .saturating_add(value.list_type.len())
                .saturating_add(value.style.len()),
            Self::Locale(value) => value.tag.len(),
            Self::Number(value) => value.storage_payload_bytes(),
            Self::PluralRules(value) => [
                Some(&value.locale),
                Some(&value.rule_type),
                Some(&value.notation),
                value.compact_display.as_ref(),
                Some(&value.rounding_mode),
                Some(&value.rounding_priority),
                Some(&value.trailing_zero_display),
            ]
            .into_iter()
            .flatten()
            .map(String::len)
            .sum(),
            Self::RelativeTimeFormat(value) => value
                .locale
                .len()
                .saturating_add(value.numbering_system.len())
                .saturating_add(value.style.len())
                .saturating_add(value.numeric.len()),
            Self::Segmenter(value) => value.locale.len().saturating_add(value.granularity.len()),
            Self::Segments(value) => value
                .input
                .len()
                .saturating_mul(size_of::<u16>())
                .saturating_add(value.granularity.len())
                .saturating_add(
                    value
                        .boundaries
                        .len()
                        .saturating_mul(size_of::<SegmentBoundary>()),
                ),
        }
    }

    pub(super) fn visit_strong_edges<V: StrongEdgeVisitor<VmObjectEdgeKind>>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        let bound_format = match self {
            Self::DateTime(value) => value.bound_format.as_ref(),
            Self::Number(value) => value.bound_format.as_ref(),
            Self::SegmentIterator(value) => {
                visitor.visit(
                    VmObjectEdgeKind::InternalSlot,
                    StrongEdgeReference::Object(value.segments),
                )?;
                None
            }
            Self::Duration
            | Self::DisplayNames(_)
            | Self::List(_)
            | Self::Locale(_)
            | Self::PluralRules(_)
            | Self::RelativeTimeFormat(_)
            | Self::Segmenter(_)
            | Self::Segments(_) => None,
        };
        if let Some(bound_format) = bound_format {
            visitor.visit(
                VmObjectEdgeKind::InternalSlot,
                StrongEdgeReference::Value(bound_format),
            )?;
        }
        Ok(())
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

    pub(in crate::runtime) fn intl_value_mut(
        &mut self,
        id: ObjectId,
    ) -> Result<Option<&mut IntlValue>> {
        Ok(self.object_mut(id)?.intl_value.as_mut())
    }
}
