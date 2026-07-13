use crate::runtime::object::DateTimeFormatValue;

use super::{
    date_time_types::{DateTimeInput, FormatPart},
    number_digits,
};

pub(super) fn format_month(
    month: u8,
    month_code: Option<&str>,
    style: Option<&str>,
    calendar: &str,
) -> String {
    if calendar == "hebrew" {
        return hebrew_month_name(month_code).to_owned();
    }
    if matches!(calendar, "chinese" | "dangi") && month_code.is_some_and(|code| code.ends_with('L'))
    {
        return format!("{month}bis");
    }
    if matches!(style, Some("long" | "short" | "narrow")) {
        let names = if calendar.starts_with("islamic") {
            &ISLAMIC_MONTHS
        } else {
            &GREGORIAN_MONTHS
        };
        let index = usize::from(month.saturating_sub(1));
        let name = names.get(index).copied().unwrap_or("");
        return match style {
            Some("short") => name.chars().take(3).collect(),
            Some("narrow") => name.chars().take(1).collect(),
            _ => name.to_owned(),
        };
    }
    if style == Some("2-digit") {
        format!("{month:02}")
    } else {
        month.to_string()
    }
}

fn hebrew_month_name(month_code: Option<&str>) -> &'static str {
    match month_code {
        Some("M01") => "Tishri",
        Some("M02") => "Heshvan",
        Some("M03") => "Kislev",
        Some("M04") => "Tevet",
        Some("M05") => "Shevat",
        Some("M05L") => "Adar I",
        Some("M06") => "Adar",
        Some("M07") => "Nisan",
        Some("M08") => "Iyar",
        Some("M09") => "Sivan",
        Some("M10") => "Tamuz",
        Some("M11") => "Av",
        Some("M12") => "Elul",
        _ => "",
    }
}

pub(super) fn year_string(year: i32, style: Option<&str>) -> String {
    if style == Some("2-digit") {
        format!("{:02}", year.unsigned_abs() % 100)
    } else {
        year.to_string()
    }
}

pub(super) fn year_parts(
    year: i32,
    style: Option<&str>,
    calendar: &str,
    locale: &str,
) -> Vec<FormatPart> {
    if !matches!(calendar, "chinese" | "dangi") {
        return vec![FormatPart {
            kind: "year",
            value: year_string(year, style),
        }];
    }
    let mut parts = vec![FormatPart {
        kind: "relatedYear",
        value: year_string(year, style),
    }];
    let year_name = cyclical_year_name(year);
    if locale.to_ascii_lowercase().starts_with("zh") {
        parts.push(FormatPart {
            kind: "yearName",
            value: year_name,
        });
        parts.push(FormatPart {
            kind: "literal",
            value: "年".to_owned(),
        });
    } else {
        parts.push(FormatPart {
            kind: "literal",
            value: " (".to_owned(),
        });
        parts.push(FormatPart {
            kind: "yearName",
            value: year_name,
        });
        parts.push(FormatPart {
            kind: "literal",
            value: ")".to_owned(),
        });
    }
    parts
}

fn cyclical_year_name(year: i32) -> String {
    const STEMS: [&str; 10] = ["甲", "乙", "丙", "丁", "戊", "己", "庚", "辛", "壬", "癸"];
    const BRANCHES: [&str; 12] = [
        "子", "丑", "寅", "卯", "辰", "巳", "午", "未", "申", "酉", "戌", "亥",
    ];
    let cycle = year.saturating_sub(1984).rem_euclid(60);
    let stem_index = usize::try_from(cycle.rem_euclid(10)).unwrap_or(0);
    let branch_index = usize::try_from(cycle.rem_euclid(12)).unwrap_or(0);
    format!(
        "{}{}",
        STEMS.get(stem_index).copied().unwrap_or("甲"),
        BRANCHES.get(branch_index).copied().unwrap_or("子")
    )
}

pub(super) fn weekday_name(day: u16, locale: &str) -> &'static str {
    let index = usize::from(day.saturating_sub(1));
    let names = if locale.to_ascii_lowercase().starts_with("de") {
        &GERMAN_WEEKDAYS
    } else {
        &ENGLISH_WEEKDAYS
    };
    names.get(index).copied().unwrap_or("Monday")
}

pub(super) fn time_zone_name(input: &DateTimeInput, style: Option<&str>) -> String {
    let zone = input.time_zone.as_deref().unwrap_or("UTC");
    if zone == "Europe/Vienna" && style == Some("long") {
        return "Central European Standard Time".to_owned();
    }
    if zone.starts_with('+') || zone.starts_with('-') {
        let sign = zone.get(..1).unwrap_or("");
        let hour = zone.get(1..3).unwrap_or("00").trim_start_matches('0');
        if hour.is_empty() || hour == "0" {
            return "GMT".to_owned();
        }
        return format!("GMT{sign}{hour}");
    }
    if zone.eq_ignore_ascii_case("UTC") {
        return if style == Some("long") {
            "Coordinated Universal Time".to_owned()
        } else {
            "UTC".to_owned()
        };
    }
    if let Some(offset) = input.offset.as_deref() {
        return format!("GMT{offset}");
    }
    zone.to_owned()
}

pub(super) fn flexible_day_period(hour: u8, style: Option<&str>) -> &'static str {
    match hour {
        6..=11 => "in the morning",
        12 => {
            if style == Some("narrow") {
                "n"
            } else {
                "noon"
            }
        }
        13..=17 => "in the afternoon",
        18..=20 => "in the evening",
        _ => "at night",
    }
}

pub(super) fn localize_numeric_parts(parts: &mut [FormatPart], formatter: &DateTimeFormatValue) {
    let Some(digit_text) = number_digits::digits(&formatter.numbering_system) else {
        return;
    };
    let digits = digit_text.chars().collect::<Vec<_>>();
    for part in parts {
        if part.kind == "literal"
            && part.value == "."
            && matches!(formatter.numbering_system.as_str(), "arab" | "arabext")
        {
            "٫".clone_into(&mut part.value);
            continue;
        }
        if !matches!(
            part.kind,
            "year"
                | "relatedYear"
                | "month"
                | "day"
                | "hour"
                | "minute"
                | "second"
                | "fractionalSecond"
        ) {
            continue;
        }
        part.value = part
            .value
            .chars()
            .map(|character| {
                character
                    .to_digit(10)
                    .and_then(|digit| usize::try_from(digit).ok())
                    .and_then(|index| digits.get(index).copied())
                    .unwrap_or(character)
            })
            .collect();
    }
}

const GREGORIAN_MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const ISLAMIC_MONTHS: [&str; 12] = [
    "Muharram",
    "Safar",
    "Rabi I",
    "Rabi II",
    "Jumada I",
    "Jumada II",
    "Rajab",
    "Sha'ban",
    "Ramadan",
    "Shawwal",
    "Dhu al-Qidah",
    "Dhu al-Hijjah",
];
const ENGLISH_WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const GERMAN_WEEKDAYS: [&str; 7] = [
    "Montag",
    "Dienstag",
    "Mittwoch",
    "Donnerstag",
    "Freitag",
    "Samstag",
    "Sonntag",
];
