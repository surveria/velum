#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    runtime::Context,
    value::ErrorName,
};

impl Context {
    pub(super) fn exact_relative_offset(offset: String) -> String {
        let minute_precision = offset
            .strip_prefix('+')
            .or_else(|| offset.strip_prefix('-'))
            .and_then(|body| body.split_once(':'))
            .is_some_and(|(hour, minute)| {
                hour.len() == 2
                    && minute.len() == 2
                    && hour.bytes().all(|byte| byte.is_ascii_digit())
                    && minute.bytes().all(|byte| byte.is_ascii_digit())
            });
        if minute_precision {
            return format!("{offset}:00");
        }
        offset
    }

    pub(super) fn resolve_relative_month(
        month: Option<i64>,
        month_code: Option<&str>,
    ) -> Result<i64> {
        let code_month = month_code
            .map(|code| {
                code.strip_prefix('M')
                    .and_then(|digits| digits.parse::<i64>().ok())
                    .filter(|value| (1..=12).contains(value))
                    .ok_or_else(|| {
                        Error::exception(ErrorName::RangeError, "Invalid relativeTo monthCode")
                    })
            })
            .transpose()?;
        match (month, code_month) {
            (Some(month), Some(code)) if month != code => Err(Error::exception(
                ErrorName::RangeError,
                "relativeTo month and monthCode do not agree",
            )),
            (Some(month), _) => Ok(month),
            (None, Some(code)) => Ok(code),
            (None, None) => Err(Error::type_error(
                "Temporal relativeTo requires month or monthCode",
            )),
        }
    }
}
