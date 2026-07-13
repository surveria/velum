use crate::runtime::object::RelativeTimeFormatValue;

use super::plural_rules::plural_category;

pub(super) struct RelativeTimePart {
    pub kind: &'static str,
    pub value: String,
    pub numeric: bool,
}

struct NumberPart {
    kind: &'static str,
    value: String,
}

pub(super) fn relative_time_parts(
    formatter: &RelativeTimeFormatValue,
    number: f64,
    unit: &str,
) -> Vec<RelativeTimePart> {
    if formatter.numeric == "auto"
        && let Some(phrase) = automatic_phrase(&formatter.locale, number, unit)
    {
        return vec![literal_part(phrase)];
    }
    let polish = locale_starts_with(&formatter.locale, "pl");
    let category = plural_category(&formatter.locale, "cardinal", number.abs(), "standard");
    let label = unit_label(formatter, unit, category);
    let mut parts = Vec::new();
    let future = !number.is_sign_negative();
    if future {
        parts.push(literal_part(if polish { "za " } else { "in " }));
    }
    parts.extend(
        format_number_parts(number.abs(), formatter)
            .into_iter()
            .map(|part| RelativeTimePart {
                kind: part.kind,
                value: part.value,
                numeric: true,
            }),
    );
    let suffix = if future {
        format!(" {label}")
    } else if polish {
        format!(" {label} temu")
    } else {
        format!(" {label} ago")
    };
    parts.push(RelativeTimePart {
        kind: "literal",
        value: suffix,
        numeric: false,
    });
    parts
}

fn automatic_phrase(locale: &str, number: f64, unit: &str) -> Option<&'static str> {
    if !locale_starts_with(locale, "en") {
        return None;
    }
    let zero = same_number(number.abs(), 0.0);
    match (unit, number, zero) {
        ("year", _, true) => Some("this year"),
        ("year", -1.0, _) => Some("last year"),
        ("year", 1.0, _) => Some("next year"),
        ("quarter", _, true) => Some("this quarter"),
        ("quarter", -1.0, _) => Some("last quarter"),
        ("quarter", 1.0, _) => Some("next quarter"),
        ("month", _, true) => Some("this month"),
        ("month", -1.0, _) => Some("last month"),
        ("month", 1.0, _) => Some("next month"),
        ("week", _, true) => Some("this week"),
        ("week", -1.0, _) => Some("last week"),
        ("week", 1.0, _) => Some("next week"),
        ("day", _, true) => Some("today"),
        ("day", -1.0, _) => Some("yesterday"),
        ("day", 1.0, _) => Some("tomorrow"),
        ("hour", _, true) => Some("this hour"),
        ("minute", _, true) => Some("this minute"),
        ("second", _, true) => Some("now"),
        _ => None,
    }
}

fn unit_label(formatter: &RelativeTimeFormatValue, unit: &str, category: &str) -> String {
    if locale_starts_with(&formatter.locale, "pl") {
        return polish_unit_label(&formatter.style, unit, category);
    }
    english_unit_label(&formatter.style, unit, category)
}

fn english_unit_label(style: &str, unit: &str, category: &str) -> String {
    if style == "long" {
        return if category == "one" {
            unit.to_owned()
        } else {
            format!("{unit}s")
        };
    }
    match (unit, category) {
        ("second", _) => "sec.".to_owned(),
        ("minute", _) => "min.".to_owned(),
        ("hour", _) => "hr.".to_owned(),
        ("day", "one") => "day".to_owned(),
        ("day", _) => "days".to_owned(),
        ("week", _) => "wk.".to_owned(),
        ("month", _) => "mo.".to_owned(),
        ("quarter", "one") => "qtr.".to_owned(),
        ("quarter", _) => "qtrs.".to_owned(),
        ("year", _) => "yr.".to_owned(),
        _ => unit.to_owned(),
    }
}

fn polish_unit_label(style: &str, unit: &str, category: &str) -> String {
    if style == "long" {
        return polish_long_unit_label(unit, category);
    }
    match (style, unit, category) {
        ("narrow", "second", _) => "s".to_owned(),
        (_, "second", _) => "sek.".to_owned(),
        (_, "minute", _) => "min".to_owned(),
        ("narrow", "hour", _) => "g.".to_owned(),
        (_, "hour", _) => "godz.".to_owned(),
        (_, "day", "one") => "dzień".to_owned(),
        (_, "day", "other") => "dnia".to_owned(),
        (_, "day", _) => "dni".to_owned(),
        (_, "week", "one") => "tydz.".to_owned(),
        (_, "week", _) => "tyg.".to_owned(),
        (_, "month", _) => "mies.".to_owned(),
        (_, "quarter", _) => "kw.".to_owned(),
        (_, "year", "one") => "rok".to_owned(),
        (_, "year", "few") => "lata".to_owned(),
        (_, "year", "other") => "roku".to_owned(),
        (_, "year", _) => "lat".to_owned(),
        _ => unit.to_owned(),
    }
}

fn polish_long_unit_label(unit: &str, category: &str) -> String {
    match (unit, category) {
        ("second", "one") => "sekundę".to_owned(),
        ("second", "few" | "other") => "sekundy".to_owned(),
        ("second", _) => "sekund".to_owned(),
        ("minute", "one") => "minutę".to_owned(),
        ("minute", "few" | "other") => "minuty".to_owned(),
        ("minute", _) => "minut".to_owned(),
        ("hour", "one") => "godzinę".to_owned(),
        ("hour", "few" | "other") => "godziny".to_owned(),
        ("hour", _) => "godzin".to_owned(),
        ("day", "one") => "dzień".to_owned(),
        ("day", "other") => "dnia".to_owned(),
        ("day", _) => "dni".to_owned(),
        ("week", "one") => "tydzień".to_owned(),
        ("week", "few") => "tygodnie".to_owned(),
        ("week", "other") => "tygodnia".to_owned(),
        ("week", _) => "tygodni".to_owned(),
        ("month", "one") => "miesiąc".to_owned(),
        ("month", "few") => "miesiące".to_owned(),
        ("month", "other") => "miesiąca".to_owned(),
        ("month", _) => "miesięcy".to_owned(),
        ("quarter", "one") => "kwartał".to_owned(),
        ("quarter", "few") => "kwartały".to_owned(),
        ("quarter", "other") => "kwartału".to_owned(),
        ("quarter", _) => "kwartałów".to_owned(),
        ("year", "one") => "rok".to_owned(),
        ("year", "few") => "lata".to_owned(),
        ("year", "other") => "roku".to_owned(),
        ("year", _) => "lat".to_owned(),
        _ => unit.to_owned(),
    }
}

fn format_number_parts(number: f64, formatter: &RelativeTimeFormatValue) -> Vec<NumberPart> {
    let text = decimal_text(number);
    let mut split = text.split('.');
    let integer = split.next().unwrap_or("0");
    let fraction = split.next();
    let polish = locale_starts_with(&formatter.locale, "pl");
    let separator = if polish { '\u{00a0}' } else { ',' };
    let groups = integer_groups(integer, polish);
    let mut parts = Vec::new();
    for (index, group) in groups.into_iter().enumerate() {
        if index > 0 {
            parts.push(NumberPart {
                kind: "group",
                value: separator.to_string(),
            });
        }
        parts.push(NumberPart {
            kind: "integer",
            value: localize_digits(&group, &formatter.numbering_system),
        });
    }
    if let Some(fraction) = fraction {
        parts.push(NumberPart {
            kind: "decimal",
            value: if polish { "," } else { "." }.to_owned(),
        });
        parts.push(NumberPart {
            kind: "fraction",
            value: localize_digits(fraction, &formatter.numbering_system),
        });
    }
    parts
}

fn decimal_text(number: f64) -> String {
    if same_number(number.fract(), 0.0) {
        return format!("{number:.0}");
    }
    let mut text = format!("{number:.3}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

fn integer_groups(integer: &str, polish: bool) -> Vec<String> {
    let should_group = if polish {
        integer.len() > 4
    } else {
        integer.len() > 3
    };
    if !should_group {
        return vec![integer.to_owned()];
    }
    let remainder = integer.len() % 3;
    let first_length = if remainder == 0 { 3 } else { remainder };
    let Some(first) = integer.get(..first_length) else {
        return vec![integer.to_owned()];
    };
    let mut groups = vec![first.to_owned()];
    let mut start = first_length;
    while start < integer.len() {
        let Some(end) = start.checked_add(3) else {
            return vec![integer.to_owned()];
        };
        let Some(group) = integer.get(start..end) else {
            return vec![integer.to_owned()];
        };
        groups.push(group.to_owned());
        start = end;
    }
    groups
}

fn localize_digits(value: &str, numbering_system: &str) -> String {
    let Some(digits) = super::number_digits::digits(numbering_system) else {
        return value.to_owned();
    };
    value
        .chars()
        .map(|character| {
            character
                .to_digit(10)
                .and_then(|digit| usize::try_from(digit).ok())
                .and_then(|digit| digits.chars().nth(digit))
                .unwrap_or(character)
        })
        .collect()
}

fn literal_part(value: &str) -> RelativeTimePart {
    RelativeTimePart {
        kind: "literal",
        value: value.to_owned(),
        numeric: false,
    }
}

fn locale_starts_with(locale: &str, language: &str) -> bool {
    locale
        .get(..language.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(language))
}

const fn same_number(left: f64, right: f64) -> bool {
    left.to_bits() == right.to_bits()
}
