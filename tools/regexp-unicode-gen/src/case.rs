use crate::GeneratorError;

const MAX_CODE_POINT: u32 = 0x10_FFFF;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CaseMapping {
    pub source: u32,
    pub target: u32,
}

/// Parses the common and simple entries used by ECMAScript Unicode folding.
///
/// # Errors
///
/// Returns an error for malformed records, invalid code points, multi-code-point
/// simple mappings, or duplicate sources.
pub fn simple_case_folding(contents: &str) -> Result<Vec<CaseMapping>, GeneratorError> {
    let mut mappings = Vec::new();
    for (line_index, raw_line) in contents.lines().enumerate() {
        let line_number = line_number(line_index)?;
        let data = raw_line.split('#').next().unwrap_or_default().trim();
        if data.is_empty() {
            continue;
        }
        let fields = data.split(';').map(str::trim).collect::<Vec<_>>();
        let Some(status) = fields.get(1).copied() else {
            return Err(line_error(line_number, "missing case-folding status"));
        };
        if !matches!(status, "C" | "S") {
            continue;
        }
        let source = parse_code_point(fields.first().copied().unwrap_or_default(), line_number)?;
        let target_text = fields
            .get(2)
            .copied()
            .ok_or_else(|| line_error(line_number, "missing simple case-folding target"))?;
        if target_text.split_ascii_whitespace().count() != 1 {
            return Err(line_error(
                line_number,
                "simple case-folding target must contain one code point",
            ));
        }
        let target = parse_code_point(target_text, line_number)?;
        mappings.push(CaseMapping { source, target });
    }
    validate_mappings(mappings, "simple case folding")
}

/// Parses `UnicodeData` simple uppercase mappings used by legacy canonicalization.
///
/// # Errors
///
/// Returns an error for malformed records, invalid code points, or duplicate
/// sources.
pub fn legacy_uppercase(contents: &str) -> Result<Vec<CaseMapping>, GeneratorError> {
    let mut mappings = Vec::new();
    for (line_index, raw_line) in contents.lines().enumerate() {
        let line_number = line_number(line_index)?;
        if raw_line.trim().is_empty() {
            continue;
        }
        let fields = raw_line.split(';').collect::<Vec<_>>();
        let source = parse_code_point(fields.first().copied().unwrap_or_default(), line_number)?;
        let uppercase = fields
            .get(12)
            .copied()
            .ok_or_else(|| line_error(line_number, "UnicodeData record has fewer than 13 fields"))?
            .trim();
        if uppercase.is_empty() {
            continue;
        }
        let target = parse_code_point(uppercase, line_number)?;
        mappings.push(CaseMapping { source, target });
    }
    validate_mappings(mappings, "legacy uppercase")
}

pub fn reverse_mappings(mappings: &[CaseMapping]) -> Vec<CaseMapping> {
    let mut reverse = mappings
        .iter()
        .map(|mapping| CaseMapping {
            source: mapping.target,
            target: mapping.source,
        })
        .collect::<Vec<_>>();
    reverse.sort_unstable_by_key(|mapping| (mapping.source, mapping.target));
    reverse
}

#[must_use]
pub fn legacy_reverse_mappings(mappings: &[CaseMapping]) -> Vec<CaseMapping> {
    let filtered = mappings
        .iter()
        .copied()
        .filter(|mapping| !(mapping.source >= 0x80 && mapping.target < 0x80))
        .collect::<Vec<_>>();
    reverse_mappings(&filtered)
}

fn validate_mappings(
    mut mappings: Vec<CaseMapping>,
    label: &str,
) -> Result<Vec<CaseMapping>, GeneratorError> {
    mappings.sort_unstable_by_key(|mapping| mapping.source);
    let mut previous = None;
    for mapping in &mappings {
        if previous == Some(mapping.source) {
            return Err(GeneratorError::new(format!(
                "{label} contains duplicate source U+{:04X}",
                mapping.source
            )));
        }
        previous = Some(mapping.source);
    }
    Ok(mappings)
}

fn parse_code_point(text: &str, line_number: usize) -> Result<u32, GeneratorError> {
    let value = u32::from_str_radix(text.trim(), 16)
        .map_err(|error| line_error(line_number, &format!("invalid code point: {error}")))?;
    if value > MAX_CODE_POINT {
        return Err(line_error(line_number, "code point exceeds Unicode range"));
    }
    Ok(value)
}

fn line_number(line_index: usize) -> Result<usize, GeneratorError> {
    line_index
        .checked_add(1)
        .ok_or_else(|| GeneratorError::new("case mapping line number overflowed"))
}

fn line_error(line_number: usize, message: &str) -> GeneratorError {
    GeneratorError::new(format!("case mapping line {line_number}: {message}"))
}
