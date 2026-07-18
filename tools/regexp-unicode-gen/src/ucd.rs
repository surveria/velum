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
    if ranges.is_empty() {
        return Err(GeneratorError::new(format!(
            "UCD property {requested_property} has no ranges"
        )));
    }
    ranges.sort_unstable_by_key(|range| (range.start, range.end));
    Ok(merge_ranges(ranges))
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
