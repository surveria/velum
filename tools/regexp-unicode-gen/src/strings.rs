use std::collections::{BTreeMap, BTreeSet};

use crate::GeneratorError;

const SOURCE_PROPERTIES: [&str; 6] = [
    "Basic_Emoji",
    "Emoji_Keycap_Sequence",
    "RGI_Emoji_Flag_Sequence",
    "RGI_Emoji_Tag_Sequence",
    "RGI_Emoji_Modifier_Sequence",
    "RGI_Emoji_ZWJ_Sequence",
];
const UNION_PROPERTY: &str = "RGI_Emoji";
const MAX_CODE_POINT: u32 = 0x10_FFFF;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GeneratedStringProperty {
    pub name: String,
    pub sequences: Vec<Vec<u32>>,
}

/// Parses the two pinned emoji sequence sources into ECMAScript string properties.
///
/// # Errors
///
/// Returns an error for malformed code points, ranges, property names, or missing data.
pub fn generate(
    emoji_sequences: &str,
    emoji_zwj_sequences: &str,
) -> Result<Vec<GeneratedStringProperty>, GeneratorError> {
    let mut properties = SOURCE_PROPERTIES
        .iter()
        .map(|name| ((*name).to_owned(), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    parse_source(emoji_sequences, &mut properties)?;
    parse_source(emoji_zwj_sequences, &mut properties)?;
    let mut union = BTreeSet::new();
    let mut generated = Vec::with_capacity(
        SOURCE_PROPERTIES
            .len()
            .checked_add(1)
            .ok_or_else(|| GeneratorError::new("string property count overflowed"))?,
    );
    for name in SOURCE_PROPERTIES {
        let sequences = properties.remove(name).ok_or_else(|| {
            GeneratorError::new(format!("Unicode string property {name} is missing"))
        })?;
        if sequences.is_empty() {
            return Err(GeneratorError::new(format!(
                "Unicode string property {name} has no sequences"
            )));
        }
        union.extend(sequences.iter().cloned());
        generated.push(GeneratedStringProperty {
            name: name.to_owned(),
            sequences: sequences.into_iter().collect(),
        });
    }
    generated.push(GeneratedStringProperty {
        name: UNION_PROPERTY.to_owned(),
        sequences: union.into_iter().collect(),
    });
    Ok(generated)
}

fn parse_source(
    contents: &str,
    properties: &mut BTreeMap<String, BTreeSet<Vec<u32>>>,
) -> Result<(), GeneratorError> {
    for (line_index, raw_line) in contents.lines().enumerate() {
        let line_number = line_index
            .checked_add(1)
            .ok_or_else(|| GeneratorError::new("emoji sequence line number overflowed"))?;
        let data = raw_line
            .split('#')
            .next()
            .ok_or_else(|| line_error(line_number, "failed to read source data"))?
            .trim();
        if data.is_empty() {
            continue;
        }
        let (code_points, remainder) = data
            .split_once(';')
            .ok_or_else(|| line_error(line_number, "missing property separator"))?;
        let property = remainder
            .split(';')
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| line_error(line_number, "missing string property"))?;
        let target = properties.get_mut(property).ok_or_else(|| {
            line_error(
                line_number,
                &format!("unsupported Unicode string property {property}"),
            )
        })?;
        for sequence in parse_sequence_field(code_points.trim(), line_number)? {
            target.insert(sequence);
        }
    }
    Ok(())
}

fn parse_sequence_field(field: &str, line_number: usize) -> Result<Vec<Vec<u32>>, GeneratorError> {
    if let Some((start, end)) = field.split_once("..") {
        if start.contains(char::is_whitespace) || end.contains(char::is_whitespace) {
            return Err(line_error(line_number, "range contains whitespace"));
        }
        let start = parse_code_point(start, line_number)?;
        let end = parse_code_point(end, line_number)?;
        if start > end {
            return Err(line_error(line_number, "range is descending"));
        }
        let capacity = end
            .checked_sub(start)
            .and_then(|value| value.checked_add(1))
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| line_error(line_number, "range size overflowed"))?;
        let mut sequences = Vec::with_capacity(capacity);
        let mut value = start;
        loop {
            sequences.push(vec![value]);
            if value == end {
                break;
            }
            value = value
                .checked_add(1)
                .ok_or_else(|| line_error(line_number, "range iteration overflowed"))?;
        }
        return Ok(sequences);
    }
    let sequence = field
        .split_ascii_whitespace()
        .map(|value| parse_code_point(value, line_number))
        .collect::<Result<Vec<_>, _>>()?;
    if sequence.is_empty() {
        return Err(line_error(line_number, "empty string sequence"));
    }
    Ok(vec![sequence])
}

fn parse_code_point(value: &str, line_number: usize) -> Result<u32, GeneratorError> {
    let code_point = u32::from_str_radix(value, 16)
        .map_err(|error| line_error(line_number, &format!("invalid code point: {error}")))?;
    if code_point > MAX_CODE_POINT {
        return Err(line_error(line_number, "code point exceeds Unicode range"));
    }
    Ok(code_point)
}

fn line_error(line_number: usize, message: &str) -> GeneratorError {
    GeneratorError::new(format!("emoji sequence line {line_number}: {message}"))
}
