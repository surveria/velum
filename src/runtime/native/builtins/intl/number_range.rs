#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::runtime::object::NumberFormatValue;

pub(super) fn range_separator(formatter: &NumberFormatValue) -> &'static str {
    if locale_starts_with(formatter, "pt") {
        " - "
    } else if formatter.style == "currency" && formatter.sign_display != "always" {
        " – "
    } else {
        "–"
    }
}

pub(super) fn format_range_text(
    formatter: &NumberFormatValue,
    start: &str,
    end: &str,
    separator: &str,
) -> String {
    if locale_starts_with(formatter, "pt") && formatter.style == "currency" {
        return format_portuguese_currency_range(formatter, start, end, separator);
    }
    if formatter.style == "currency" && formatter.sign_display == "always" {
        let currency = currency_symbol(formatter.currency.as_deref().unwrap_or(""));
        let shared = format!("+{currency}");
        let end = end.strip_prefix(&shared).unwrap_or(end);
        return format!("{start}{separator}{end}");
    }
    format!("{start}{separator}{end}")
}

fn format_portuguese_currency_range(
    formatter: &NumberFormatValue,
    start: &str,
    end: &str,
    separator: &str,
) -> String {
    let currency = format!(
        "\u{00a0}{}",
        currency_symbol(formatter.currency.as_deref().unwrap_or(""))
    );
    let start = start.strip_suffix(&currency).unwrap_or(start);
    let end = end.strip_suffix(&currency).unwrap_or(end);
    let end = if formatter.sign_display == "always" {
        end.strip_prefix('+').unwrap_or(end)
    } else {
        end
    };
    format!("{start}{separator}{end}{currency}")
}

fn currency_symbol(currency: &str) -> &str {
    match currency {
        "EUR" => "€",
        "USD" => "$",
        _ => currency,
    }
}

fn locale_starts_with(formatter: &NumberFormatValue, language: &str) -> bool {
    formatter
        .locale
        .get(..language.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(language))
}
