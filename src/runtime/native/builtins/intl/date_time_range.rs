use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs},
    value::Value,
};

use super::formatting::{DateTimeInputKind, FormatPart, format_parts};

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
        let start = self.intl_date_time_input(&formatter, start_value)?;
        let end = self.intl_date_time_input(&formatter, end_value)?;
        if start.kind != end.kind {
            return Err(Error::type_error(
                "startDate and endDate must have the same date-time type",
            ));
        }
        if start.kind == DateTimeInputKind::ZonedDateTime {
            return Err(Error::type_error(
                "Temporal.ZonedDateTime is not supported by DateTimeFormat range methods",
            ));
        }
        let start_parts = format_parts(&formatter, &start)?;
        let end_parts = format_parts(&formatter, &end)?;
        let range_parts = if start_parts == end_parts {
            source_parts(start_parts, "shared")
        } else {
            let mut parts = source_parts(start_parts, "startRange");
            parts.push(SourcePart {
                part: FormatPart {
                    kind: "literal",
                    value: RANGE_SEPARATOR.to_owned(),
                },
                source: "shared",
            });
            parts.extend(source_parts(end_parts, "endRange"));
            parts
        };
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
