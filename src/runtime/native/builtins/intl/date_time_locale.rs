use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs},
    value::Value,
};

#[derive(Clone, Copy)]
pub(in crate::runtime::native) enum DateLocaleDefaults {
    All,
    Date,
    Time,
}

impl Context {
    pub(in crate::runtime::native) fn format_date_locale_string(
        &mut self,
        this_value: &Value,
        args: RuntimeCallArgs<'_>,
        defaults: DateLocaleDefaults,
    ) -> Result<Value> {
        let Value::Object(id) = this_value else {
            return Err(Error::type_error("Date method receiver must be a Date"));
        };
        let date = self
            .objects
            .date_value(*id)?
            .ok_or_else(|| Error::type_error("Date method receiver must be a Date"))?;
        if date.millis().is_none() {
            return self.heap_string_value("Invalid Date");
        }
        let locale = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let options = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if matches!(options, Value::Null) {
            return Err(Error::type_error("Date locale options cannot be null"));
        }
        let mut fields = Vec::new();
        let mut has_date = false;
        let mut has_time = false;
        let mut has_style = false;
        for (name, group) in DATE_TIME_OPTION_PROPERTIES {
            let value = if matches!(options, Value::Undefined) {
                Value::Undefined
            } else {
                self.get_named(&options, name)?
            };
            if matches!(value, Value::Undefined) {
                continue;
            }
            match group {
                OptionGroup::Date => has_date = true,
                OptionGroup::Time => has_time = true,
                OptionGroup::Style => has_style = true,
                OptionGroup::Other => {}
            }
            fields.push((*name, value));
        }
        let (add_date, add_time) = match defaults {
            DateLocaleDefaults::All => {
                let add = !has_date && !has_time && !has_style;
                (add, add)
            }
            DateLocaleDefaults::Date => (!has_date && !has_style, false),
            DateLocaleDefaults::Time => (false, !has_time && !has_style),
        };
        if add_date {
            fields.push(("year", self.heap_string_value("numeric")?));
            fields.push(("month", self.heap_string_value("numeric")?));
            fields.push(("day", self.heap_string_value("numeric")?));
        }
        if add_time {
            fields.push(("hour", self.heap_string_value("numeric")?));
            fields.push(("minute", self.heap_string_value("numeric")?));
            fields.push(("second", self.heap_string_value("numeric")?));
        }
        let options = self.create_intl_data_object(fields)?;
        let format_args = [locale, options];
        self.format_temporal_locale_string(this_value, RuntimeCallArgs::values(&format_args))
    }
}

#[derive(Clone, Copy)]
enum OptionGroup {
    Date,
    Time,
    Style,
    Other,
}

const DATE_TIME_OPTION_PROPERTIES: &[(&str, OptionGroup)] = &[
    ("localeMatcher", OptionGroup::Other),
    ("calendar", OptionGroup::Other),
    ("numberingSystem", OptionGroup::Other),
    ("hour12", OptionGroup::Other),
    ("hourCycle", OptionGroup::Other),
    ("timeZone", OptionGroup::Other),
    ("weekday", OptionGroup::Date),
    ("era", OptionGroup::Other),
    ("year", OptionGroup::Date),
    ("month", OptionGroup::Date),
    ("day", OptionGroup::Date),
    ("dayPeriod", OptionGroup::Time),
    ("hour", OptionGroup::Time),
    ("minute", OptionGroup::Time),
    ("second", OptionGroup::Time),
    ("fractionalSecondDigits", OptionGroup::Time),
    ("timeZoneName", OptionGroup::Other),
    ("formatMatcher", OptionGroup::Other),
    ("dateStyle", OptionGroup::Style),
    ("timeStyle", OptionGroup::Style),
];
