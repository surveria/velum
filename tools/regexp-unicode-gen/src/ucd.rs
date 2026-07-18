use crate::GeneratorError;

const MAX_CODE_POINT: u32 = 0x10_FFFF;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CodePointRange {
    pub start: u32,
    pub end: u32,
}

/// Parses, sorts, and merges ranges for one exact UCD property name.
///
/// # Errors
///
/// Returns an error for malformed data, invalid scalar bounds, or an absent
/// requested property.
pub fn property_ranges(
    contents: &str,
    requested_property: &str,
) -> Result<Vec<CodePointRange>, GeneratorError> {
    let mut ranges = Vec::new();
    collect_property_ranges(contents, requested_property, &mut ranges)?;
    if ranges.is_empty() {
        return Err(GeneratorError::new(format!(
            "UCD property {requested_property} has no ranges"
        )));
    }
    Ok(normalize_ranges(ranges))
}

pub fn collect_property_ranges(
    contents: &str,
    requested_property: &str,
    ranges: &mut Vec<CodePointRange>,
) -> Result<(), GeneratorError> {
    for (line_index, raw_line) in contents.lines().enumerate() {
        let line_number = line_index
            .checked_add(1)
            .ok_or_else(|| GeneratorError::new("UCD line number overflowed"))?;
        let data = raw_line.split('#').next().unwrap_or_default().trim();
        if data.is_empty() {
            continue;
        }
        let Some((range_text, property_text)) = data.split_once(';') else {
            return Err(GeneratorError::new(format!(
                "UCD line {line_number}: missing property separator"
            )));
        };
        if property_text.trim() != requested_property {
            continue;
        }
        ranges.push(parse_range(range_text.trim(), line_number)?);
    }
    Ok(())
}

pub fn normalize_ranges(mut ranges: Vec<CodePointRange>) -> Vec<CodePointRange> {
    ranges.sort_unstable_by_key(|range| (range.start, range.end));
    merge_ranges(ranges)
}

pub fn complement_ranges(ranges: Vec<CodePointRange>) -> Vec<CodePointRange> {
    let ranges = normalize_ranges(ranges);
    let mut complement = Vec::new();
    let mut next = 0_u32;
    for range in ranges {
        if next < range.start {
            complement.push(CodePointRange {
                start: next,
                end: range.start.saturating_sub(1),
            });
        }
        let Some(after) = range.end.checked_add(1) else {
            return complement;
        };
        next = after;
    }
    if next <= MAX_CODE_POINT {
        complement.push(CodePointRange {
            start: next,
            end: MAX_CODE_POINT,
        });
    }
    complement
}

/// Collects every data range whose property field contains one exact
/// whitespace-separated value.
///
/// # Errors
///
/// Returns an error for malformed UCD data or invalid code point ranges.
pub fn property_value_ranges(
    contents: &str,
    requested_value: &str,
) -> Result<Vec<CodePointRange>, GeneratorError> {
    let mut ranges = Vec::new();
    for_each_data_line(contents, |range, property| {
        if property
            .split_ascii_whitespace()
            .any(|value| value == requested_value)
        {
            ranges.push(range);
        }
        Ok(())
    })?;
    Ok(normalize_ranges(ranges))
}

/// Collects all explicit ranges in one semicolon-delimited UCD file.
///
/// # Errors
///
/// Returns an error for malformed UCD data or invalid code point ranges.
pub fn all_data_ranges(contents: &str) -> Result<Vec<CodePointRange>, GeneratorError> {
    let mut ranges = Vec::new();
    for_each_data_line(contents, |range, _| {
        ranges.push(range);
        Ok(())
    })?;
    Ok(normalize_ranges(ranges))
}

#[must_use]
pub fn subtract_ranges(
    source: Vec<CodePointRange>,
    removed: &[CodePointRange],
) -> Vec<CodePointRange> {
    let mut result = Vec::new();
    for source_range in normalize_ranges(source) {
        let mut pending_start = source_range.start;
        let mut exhausted = false;
        for removal in removed {
            if removal.end < pending_start {
                continue;
            }
            if removal.start > source_range.end {
                break;
            }
            if removal.start > pending_start {
                result.push(CodePointRange {
                    start: pending_start,
                    end: removal.start.saturating_sub(1),
                });
            }
            let Some(after_removal) = removal.end.checked_add(1) else {
                exhausted = true;
                break;
            };
            pending_start = pending_start.max(after_removal);
            if pending_start > source_range.end {
                break;
            }
        }
        if !exhausted && pending_start <= source_range.end {
            result.push(CodePointRange {
                start: pending_start,
                end: source_range.end,
            });
        }
    }
    normalize_ranges(result)
}

fn for_each_data_line(
    contents: &str,
    mut visitor: impl FnMut(CodePointRange, &str) -> Result<(), GeneratorError>,
) -> Result<(), GeneratorError> {
    for (line_index, raw_line) in contents.lines().enumerate() {
        let line_number = line_index
            .checked_add(1)
            .ok_or_else(|| GeneratorError::new("UCD line number overflowed"))?;
        let data = raw_line.split('#').next().unwrap_or_default().trim();
        if data.is_empty() {
            continue;
        }
        let Some((range_text, property_text)) = data.split_once(';') else {
            return Err(GeneratorError::new(format!(
                "UCD line {line_number}: missing property separator"
            )));
        };
        visitor(
            parse_range(range_text.trim(), line_number)?,
            property_text.trim(),
        )?;
    }
    Ok(())
}

fn parse_range(text: &str, line_number: usize) -> Result<CodePointRange, GeneratorError> {
    let (start, end) = if let Some((start, end)) = text.split_once("..") {
        (
            parse_code_point(start, line_number)?,
            parse_code_point(end, line_number)?,
        )
    } else {
        let value = parse_code_point(text, line_number)?;
        (value, value)
    };
    if start > end {
        return Err(GeneratorError::new(format!(
            "UCD line {line_number}: descending code point range"
        )));
    }
    Ok(CodePointRange { start, end })
}

fn parse_code_point(text: &str, line_number: usize) -> Result<u32, GeneratorError> {
    let value = u32::from_str_radix(text.trim(), 16).map_err(|error| {
        GeneratorError::new(format!(
            "UCD line {line_number}: invalid code point: {error}"
        ))
    })?;
    if value > MAX_CODE_POINT {
        return Err(GeneratorError::new(format!(
            "UCD line {line_number}: code point exceeds Unicode scalar range"
        )));
    }
    Ok(value)
}

fn merge_ranges(ranges: Vec<CodePointRange>) -> Vec<CodePointRange> {
    let mut merged: Vec<CodePointRange> = Vec::new();
    for range in ranges {
        let Some(previous) = merged.last_mut() else {
            merged.push(range);
            continue;
        };
        let adjacent_end = previous.end.checked_add(1).unwrap_or(previous.end);
        if range.start <= adjacent_end {
            previous.end = previous.end.max(range.end);
        } else {
            merged.push(range);
        }
    }
    merged
}
