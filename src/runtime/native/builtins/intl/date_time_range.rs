use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        object::{DateTimeFormatValue, TemporalValue},
    },
    value::Value,
};

use super::{
    date_time_types::{DateTimeInput, DateTimeInputKind, FormatPart},
    formatting::{date_time_number_input, format_parts},
};

const RANGE_SEPARATOR: &str = "\u{2009}–\u{2009}";

impl Context {
    pub(super) fn eval_intl_date_time_format_range(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        to_parts: bool,
    ) -> Result<Value> {
        let formatter = self.date_time_format_receiver(this_value)?;
        let Some(start_value) = args.as_slice().first() else {
            return Err(Error::type_error("startDate must be provided"));
        };
        let Some(end_value) = args.as_slice().get(1) else {
            return Err(Error::type_error("endDate must be provided"));
        };
        if matches!(start_value, Value::Undefined) || matches!(end_value, Value::Undefined) {
            return Err(Error::type_error(
                "startDate and endDate must not be undefined",
            ));
        }
        let start = self.date_time_range_formattable(start_value)?;
        let end = self.date_time_range_formattable(end_value)?;
        if start.kind() != end.kind() {
            return Err(Error::type_error(
                "startDate and endDate must have the same date-time type",
            ));
        }
        if start.kind() == DateTimeInputKind::ZonedDateTime {
            return Err(Error::type_error(
                "Temporal.ZonedDateTime is not supported by DateTimeFormat range methods",
            ));
        }
        let start = start.into_input(self, &formatter)?;
        let end = end.into_input(self, &formatter)?;
        let input_kind = start.kind;
        let start_parts = format_parts(&formatter, &start)?;
        let end_parts = format_parts(&formatter, &end)?;
        let range_parts = partition_range_parts(&formatter, input_kind, start_parts, end_parts);
        if to_parts {
            return self.date_time_range_parts_value(range_parts);
        }
        let text = range_parts
            .into_iter()
            .map(|part| part.part.value)
            .collect::<String>();
        self.heap_string_value(&text)
    }

    fn date_time_range_parts_value(&mut self, parts: Vec<SourcePart>) -> Result<Value> {
        let mut values = Vec::with_capacity(parts.len());
        for source_part in parts {
            let kind = self.heap_string_value(source_part.part.kind)?;
            let value = self.heap_string_value(&source_part.part.value)?;
            let source = self.heap_string_value(source_part.source)?;
            values.push(self.create_intl_data_object(vec![
                ("type", kind),
                ("value", value),
                ("source", source),
            ])?);
        }
        self.create_array_from_elements(values)
    }

    fn date_time_range_formattable(&mut self, value: &Value) -> Result<RangeFormattable> {
        if let Value::Object(id) = value
            && let Some(temporal) = self.objects.temporal_value(*id)?
        {
            let kind = match temporal {
                TemporalValue::Duration(_) => {
                    return Err(Error::type_error("Duration cannot be date-time formatted"));
                }
                TemporalValue::Instant(_) => DateTimeInputKind::Instant,
                TemporalValue::PlainDate(_) => DateTimeInputKind::PlainDate,
                TemporalValue::PlainDateTime(_) => DateTimeInputKind::PlainDateTime,
                TemporalValue::PlainMonthDay(_) => DateTimeInputKind::PlainMonthDay,
                TemporalValue::PlainTime(_) => DateTimeInputKind::PlainTime,
                TemporalValue::PlainYearMonth(_) => DateTimeInputKind::PlainYearMonth,
                TemporalValue::ZonedDateTime(_) => DateTimeInputKind::ZonedDateTime,
            };
            return Ok(RangeFormattable::Temporal(value.clone(), kind));
        }
        Ok(RangeFormattable::Number(self.to_number(value)?))
    }
}

enum RangeFormattable {
    Temporal(Value, DateTimeInputKind),
    Number(f64),
}

impl RangeFormattable {
    const fn kind(&self) -> DateTimeInputKind {
        match self {
            Self::Temporal(_, kind) => *kind,
            Self::Number(_) => DateTimeInputKind::LegacyDate,
        }
    }

    fn into_input(
        self,
        context: &mut Context,
        formatter: &DateTimeFormatValue,
    ) -> Result<DateTimeInput> {
        match self {
            Self::Temporal(value, _) => context.intl_date_time_input(formatter, &value),
            Self::Number(number) => date_time_number_input(formatter, number),
        }
    }
}

struct SourcePart {
    part: FormatPart,
    source: &'static str,
}

fn source_parts(parts: Vec<FormatPart>, source: &'static str) -> Vec<SourcePart> {
    parts
        .into_iter()
        .map(|part| SourcePart { part, source })
        .collect()
}

fn partition_range_parts(
    formatter: &DateTimeFormatValue,
    input_kind: DateTimeInputKind,
    start: Vec<FormatPart>,
    end: Vec<FormatPart>,
) -> Vec<SourcePart> {
    if start == end {
        return source_parts(start, "shared");
    }
    let prefix_len = start
        .iter()
        .zip(&end)
        .take_while(|(start_part, end_part)| start_part == end_part)
        .count();
    let shared_date_prefix = input_kind == DateTimeInputKind::PlainDateTime
        && prefix_len
            .checked_sub(1)
            .and_then(|index| start.get(index))
            .is_some_and(|part| part.kind == "literal" && part.value == ", ");
    if !uses_textual_month(formatter) && !shared_date_prefix {
        return uncollapsed_range_parts(start, end);
    }
    let suffix_limit = start.len().min(end.len()).saturating_sub(prefix_len);
    let suffix_len = start
        .iter()
        .rev()
        .zip(end.iter().rev())
        .take(suffix_limit)
        .take_while(|(start_part, end_part)| start_part == end_part)
        .count();
    if suffix_len == 0 && !shared_date_prefix {
        return uncollapsed_range_parts(start, end);
    }
    let start_middle_len = start
        .len()
        .saturating_sub(prefix_len)
        .saturating_sub(suffix_len);
    let end_middle_len = end
        .len()
        .saturating_sub(prefix_len)
        .saturating_sub(suffix_len);
    let mut parts = sourced_iter(start.iter().take(prefix_len).cloned(), "shared");
    parts.extend(sourced_iter(
        start
            .iter()
            .skip(prefix_len)
            .take(start_middle_len)
            .cloned(),
        "startRange",
    ));
    parts.push(range_separator());
    parts.extend(sourced_iter(
        end.iter().skip(prefix_len).take(end_middle_len).cloned(),
        "endRange",
    ));
    parts.extend(sourced_iter(
        end.into_iter()
            .skip(prefix_len.saturating_add(end_middle_len)),
        "shared",
    ));
    parts
}

fn uncollapsed_range_parts(start: Vec<FormatPart>, end: Vec<FormatPart>) -> Vec<SourcePart> {
    let mut parts = source_parts(start, "startRange");
    parts.push(range_separator());
    parts.extend(source_parts(end, "endRange"));
    parts
}

fn sourced_iter(
    parts: impl IntoIterator<Item = FormatPart>,
    source: &'static str,
) -> Vec<SourcePart> {
    parts
        .into_iter()
        .map(|part| SourcePart { part, source })
        .collect()
}

fn range_separator() -> SourcePart {
    SourcePart {
        part: FormatPart {
            kind: "literal",
            value: RANGE_SEPARATOR.to_owned(),
        },
        source: "shared",
    }
}

fn uses_textual_month(formatter: &DateTimeFormatValue) -> bool {
    matches!(
        formatter.options.month.as_deref(),
        Some("long" | "short" | "narrow")
    ) || matches!(
        formatter.options.date_style.as_deref(),
        Some("full" | "long" | "medium")
    )
}
