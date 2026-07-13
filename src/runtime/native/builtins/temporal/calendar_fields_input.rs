use std::str::FromStr;

use temporal_rs::{Calendar, TinyAsciiStr};

use crate::{
    error::{Error, Result},
    runtime::Context,
    value::{ErrorName, Value},
};

impl Context {
    pub(super) fn temporal_calendar_era_fields(
        &mut self,
        value: &Value,
        calendar: &Calendar,
    ) -> Result<(Option<TinyAsciiStr<19>>, Option<i64>)> {
        if matches!(calendar.identifier(), "iso8601" | "chinese" | "dangi") {
            return Ok((None, None));
        }
        let era_value = self.get_named(value, "era")?;
        let era = if matches!(era_value, Value::Undefined) {
            None
        } else {
            let text = self.to_string(&era_value)?.to_ascii_lowercase();
            Some(
                TinyAsciiStr::<19>::from_str(&text)
                    .map_err(|_| Error::exception(ErrorName::RangeError, "Invalid Temporal era"))?,
            )
        };
        let era_year = self.plain_date_optional_i64(value, "eraYear")?;
        Ok((era, era_year))
    }
}
