use core::fmt::Write;

use crate::{
    CodePointRange, GeneratorError, SourceManifest,
    binary::{GeneratedProperty, PropertySpec},
    case::CaseMapping,
    strings::GeneratedStringProperty,
    values::GeneratedValue,
};

const RANGES_PER_LINE: usize = 128;
const VALUES_PER_LOOKUP: usize = 64;

pub fn core_properties(
    manifest: &SourceManifest,
    generator_version: &str,
    format_version: u32,
) -> Result<String, GeneratorError> {
    let mut output = String::new();
    emit_header(&mut output, manifest, generator_version, format_version)?;
    writeln!(output).map_err(format_error)?;
    writeln!(
        output,
        "pub const UNICODE_VERSION: &str = \"{}\";",
        manifest.unicode_version
    )
    .map_err(format_error)?;
    Ok(output)
}

pub fn binary_properties(
    manifest: &SourceManifest,
    generator_version: &str,
    format_version: u32,
    properties: &[GeneratedProperty],
) -> Result<String, GeneratorError> {
    let mut output = String::new();
    emit_header(&mut output, manifest, generator_version, format_version)?;
    for property in properties {
        emit_ranges(
            &mut output,
            &rust_name(property.spec.canonical),
            &property.ranges,
        )?;
    }
    emit_binary_lookup(&mut output, properties)?;
    Ok(output)
}

pub fn value_properties(
    manifest: &SourceManifest,
    generator_version: &str,
    format_version: u32,
    values: &[GeneratedValue],
    constant_prefix: &str,
    function_name: &str,
) -> Result<String, GeneratorError> {
    let mut output = String::new();
    emit_header(&mut output, manifest, generator_version, format_version)?;
    for value in values {
        let name = format!("{constant_prefix}_{}", rust_name(&value.spec.short));
        emit_compact_ranges(&mut output, &name, &value.ranges)?;
    }
    emit_value_lookup(&mut output, values, constant_prefix, function_name)?;
    Ok(output)
}

pub fn string_properties(
    manifest: &SourceManifest,
    generator_version: &str,
    format_version: u32,
    properties: &[GeneratedStringProperty],
) -> Result<String, GeneratorError> {
    let mut output = String::new();
    emit_header(&mut output, manifest, generator_version, format_version)?;
    for property in properties {
        let mut data = Vec::new();
        let mut offsets = vec![0_u32];
        for sequence in &property.sequences {
            data.extend(sequence.iter().copied());
            offsets.push(u32::try_from(data.len()).map_err(|error| {
                GeneratorError::new(format!(
                    "string property {} offset overflowed: {error}",
                    property.name
                ))
            })?);
        }
        let name = rust_name(&property.name);
        emit_code_point_array(&mut output, &format!("{name}_DATA"), &data)?;
        emit_offset_array(&mut output, &format!("{name}_OFFSETS"), &offsets)?;
    }
    writeln!(output).map_err(format_error)?;
    writeln!(output, "#[rustfmt::skip]").map_err(format_error)?;
    writeln!(
        output,
        "pub fn string_property(name: &str) -> Option<(&'static [u32], &'static [u32])> {{"
    )
    .map_err(format_error)?;
    writeln!(output, "    match name {{").map_err(format_error)?;
    for property in properties {
        let name = rust_name(&property.name);
        writeln!(
            output,
            "        {:?} => Some(({name}_DATA, {name}_OFFSETS)),",
            property.name
        )
        .map_err(format_error)?;
    }
    writeln!(output, "        _ => None,").map_err(format_error)?;
    writeln!(output, "    }}").map_err(format_error)?;
    writeln!(output, "}}").map_err(format_error)?;
    Ok(output)
}

pub fn case_mappings(
    manifest: &SourceManifest,
    generator_version: &str,
    format_version: u32,
    simple: &[CaseMapping],
    simple_reverse: &[CaseMapping],
    legacy: &[CaseMapping],
    legacy_reverse: &[CaseMapping],
) -> Result<String, GeneratorError> {
    let mut output = String::new();
    emit_header(&mut output, manifest, generator_version, format_version)?;
    emit_mapping_array(&mut output, "SIMPLE_CASE_FOLD", simple)?;
    emit_mapping_array(&mut output, "SIMPLE_CASE_FOLD_REVERSE", simple_reverse)?;
    emit_mapping_array(&mut output, "LEGACY_UPPERCASE", legacy)?;
    emit_mapping_array(&mut output, "LEGACY_UPPERCASE_REVERSE", legacy_reverse)?;
    output.push_str(
        r"
pub fn simple_case_fold(value: u32) -> u32 {
    mapping_target(SIMPLE_CASE_FOLD, value)
}

pub fn legacy_uppercase(value: u32) -> u32 {
    mapping_target(LEGACY_UPPERCASE, value)
}

pub fn simple_fold_source_in_ranges(canonical: u32, ranges: &[(u32, u32)]) -> bool {
    reverse_source_in_ranges(SIMPLE_CASE_FOLD_REVERSE, canonical, ranges)
}

pub fn legacy_uppercase_source_in_ranges(canonical: u32, ranges: &[(u32, u32)]) -> bool {
    reverse_source_in_ranges(LEGACY_UPPERCASE_REVERSE, canonical, ranges)
}

pub fn simple_fold_sources_all_in_ranges(canonical: u32, ranges: &[(u32, u32)]) -> bool {
    reverse_sources_all_in_ranges(SIMPLE_CASE_FOLD_REVERSE, canonical, ranges)
}

fn mapping_target(mappings: &[(u32, u32)], value: u32) -> u32 {
    mappings
        .binary_search_by_key(&value, |(source, _)| *source)
        .ok()
        .and_then(|index| mappings.get(index))
        .map_or(value, |(_, target)| *target)
}

fn reverse_source_in_ranges(
    mappings: &[(u32, u32)],
    canonical: u32,
    ranges: &[(u32, u32)],
) -> bool {
    let start = mappings.partition_point(|(target, _)| *target < canonical);
    mappings.get(start..).is_some_and(|remaining| {
        remaining
            .iter()
            .take_while(|(target, _)| *target == canonical)
            .any(|(_, source)| contains(ranges, *source))
    })
}

fn reverse_sources_all_in_ranges(
    mappings: &[(u32, u32)],
    canonical: u32,
    ranges: &[(u32, u32)],
) -> bool {
    let start = mappings.partition_point(|(target, _)| *target < canonical);
    mappings.get(start..).is_some_and(|remaining| {
        remaining
            .iter()
            .take_while(|(target, _)| *target == canonical)
            .all(|(_, source)| contains(ranges, *source))
    })
}

fn contains(ranges: &[(u32, u32)], value: u32) -> bool {
    ranges
        .binary_search_by(|(start, end)| {
            if value < *start {
                core::cmp::Ordering::Greater
            } else if value > *end {
                core::cmp::Ordering::Less
            } else {
                core::cmp::Ordering::Equal
            }
        })
        .is_ok()
}
",
    );
    Ok(output)
}

fn emit_value_lookup(
    output: &mut String,
    values: &[GeneratedValue],
    constant_prefix: &str,
    function_name: &str,
) -> Result<(), GeneratorError> {
    for (chunk_index, chunk) in values.chunks(VALUES_PER_LOOKUP).enumerate() {
        writeln!(output).map_err(format_error)?;
        writeln!(output, "#[rustfmt::skip]").map_err(format_error)?;
        writeln!(
            output,
            "fn {function_name}_{chunk_index}(name: &str) -> Option<&'static [(u32, u32)]> {{"
        )
        .map_err(format_error)?;
        writeln!(output, "    match name {{").map_err(format_error)?;
        for value in chunk {
            output.push_str("        ");
            emit_owned_alias_pattern(output, &value.spec.aliases)?;
            writeln!(
                output,
                " => Some({constant_prefix}_{}),",
                rust_name(&value.spec.short)
            )
            .map_err(format_error)?;
        }
        writeln!(output, "        _ => None,").map_err(format_error)?;
        writeln!(output, "    }}").map_err(format_error)?;
        writeln!(output, "}}").map_err(format_error)?;
    }
    writeln!(output).map_err(format_error)?;
    writeln!(
        output,
        "pub fn {function_name}(name: &str) -> Option<&'static [(u32, u32)]> {{"
    )
    .map_err(format_error)?;
    for chunk_index in 0..values.chunks(VALUES_PER_LOOKUP).len() {
        writeln!(
            output,
            "    if let Some(ranges) = {function_name}_{chunk_index}(name) {{"
        )
        .map_err(format_error)?;
        writeln!(output, "        return Some(ranges);").map_err(format_error)?;
        writeln!(output, "    }}").map_err(format_error)?;
    }
    writeln!(output, "    None").map_err(format_error)?;
    writeln!(output, "}}").map_err(format_error)
}

fn emit_header(
    output: &mut String,
    manifest: &SourceManifest,
    generator_version: &str,
    format_version: u32,
) -> Result<(), GeneratorError> {
    writeln!(output, "// This file is generated. Do not edit manually.").map_err(format_error)?;
    writeln!(output, "// Unicode version: {}", manifest.unicode_version).map_err(format_error)?;
    writeln!(output, "// Generator version: {generator_version}").map_err(format_error)?;
    writeln!(output, "// Output format: {format_version}").map_err(format_error)?;
    for source in &manifest.sources {
        writeln!(
            output,
            "// Source: {} {} {}",
            source.sha256, source.relative_path, source.source_url
        )
        .map_err(format_error)?;
    }
    Ok(())
}

fn emit_binary_lookup(
    output: &mut String,
    properties: &[GeneratedProperty],
) -> Result<(), GeneratorError> {
    writeln!(output).map_err(format_error)?;
    writeln!(output, "#[rustfmt::skip]").map_err(format_error)?;
    writeln!(
        output,
        "pub fn binary_property_ranges(name: &str) -> Option<&'static [(u32, u32)]> {{"
    )
    .map_err(format_error)?;
    writeln!(output, "    match name {{").map_err(format_error)?;
    for property in properties {
        output.push_str("        ");
        emit_alias_pattern(output, property.spec)?;
        writeln!(output, " => Some({}),", rust_name(property.spec.canonical))
            .map_err(format_error)?;
    }
    writeln!(output, "        _ => None,").map_err(format_error)?;
    writeln!(output, "    }}").map_err(format_error)?;
    writeln!(output, "}}").map_err(format_error)
}

fn emit_alias_pattern(output: &mut String, spec: &PropertySpec) -> Result<(), GeneratorError> {
    write!(output, "{:?}", spec.canonical).map_err(format_error)?;
    for alias in spec.aliases {
        write!(output, " | {alias:?}").map_err(format_error)?;
    }
    Ok(())
}

fn emit_owned_alias_pattern(output: &mut String, aliases: &[String]) -> Result<(), GeneratorError> {
    let mut separator = "";
    for alias in aliases {
        write!(output, "{separator}{alias:?}").map_err(format_error)?;
        separator = " | ";
    }
    Ok(())
}

fn emit_ranges(
    output: &mut String,
    name: &str,
    ranges: &[CodePointRange],
) -> Result<(), GeneratorError> {
    writeln!(output).map_err(format_error)?;
    writeln!(output, "#[rustfmt::skip]").map_err(format_error)?;
    writeln!(output, "pub const {name}: &[(u32, u32)] = &[").map_err(format_error)?;
    for chunk in ranges.chunks(RANGES_PER_LINE) {
        output.push_str("    ");
        for (index, range) in chunk.iter().enumerate() {
            if index > 0 {
                output.push(' ');
            }
            output.push('(');
            emit_code_point(output, range.start)?;
            output.push_str(", ");
            emit_code_point(output, range.end)?;
            output.push_str("),");
        }
        output.push('\n');
    }
    output.push_str("];\n");
    Ok(())
}

fn emit_compact_ranges(
    output: &mut String,
    name: &str,
    ranges: &[CodePointRange],
) -> Result<(), GeneratorError> {
    writeln!(
        output,
        "#[rustfmt::skip] pub const {name}: &[(u32, u32)] = &["
    )
    .map_err(format_error)?;
    for chunk in ranges.chunks(RANGES_PER_LINE) {
        output.push_str("    ");
        for (index, range) in chunk.iter().enumerate() {
            if index > 0 {
                output.push(' ');
            }
            output.push('(');
            emit_code_point(output, range.start)?;
            output.push_str(", ");
            emit_code_point(output, range.end)?;
            output.push_str("),");
        }
        output.push('\n');
    }
    output.push_str("];\n");
    Ok(())
}

fn emit_mapping_array(
    output: &mut String,
    name: &str,
    mappings: &[CaseMapping],
) -> Result<(), GeneratorError> {
    writeln!(output, "#[rustfmt::skip]").map_err(format_error)?;
    writeln!(output, "const {name}: &[(u32, u32)] = &[").map_err(format_error)?;
    for chunk in mappings.chunks(RANGES_PER_LINE) {
        output.push_str("    ");
        for (index, mapping) in chunk.iter().enumerate() {
            if index > 0 {
                output.push(' ');
            }
            output.push('(');
            emit_code_point(output, mapping.source)?;
            output.push_str(", ");
            emit_code_point(output, mapping.target)?;
            output.push_str("),");
        }
        output.push('\n');
    }
    output.push_str("];\n");
    Ok(())
}

fn emit_code_point_array(
    output: &mut String,
    name: &str,
    values: &[u32],
) -> Result<(), GeneratorError> {
    writeln!(output).map_err(format_error)?;
    writeln!(output, "#[rustfmt::skip]").map_err(format_error)?;
    writeln!(output, "const {name}: &[u32] = &[").map_err(format_error)?;
    for chunk in values.chunks(RANGES_PER_LINE) {
        output.push_str("    ");
        for value in chunk {
            emit_code_point(output, *value)?;
            output.push_str(", ");
        }
        output.push('\n');
    }
    output.push_str("];\n");
    Ok(())
}

fn emit_offset_array(
    output: &mut String,
    name: &str,
    values: &[u32],
) -> Result<(), GeneratorError> {
    writeln!(output, "#[rustfmt::skip]").map_err(format_error)?;
    writeln!(output, "const {name}: &[u32] = &[").map_err(format_error)?;
    for chunk in values.chunks(RANGES_PER_LINE) {
        output.push_str("    ");
        for value in chunk {
            write!(output, "{value}, ").map_err(format_error)?;
        }
        output.push('\n');
    }
    output.push_str("];\n");
    Ok(())
}

fn emit_code_point(output: &mut String, value: u32) -> Result<(), GeneratorError> {
    if value > 0xFFFF {
        write!(output, "0x{:X}_{:04X}", value >> 16, value & 0xFFFF).map_err(format_error)
    } else {
        write!(output, "0x{value:X}").map_err(format_error)
    }
}

fn rust_name(name: &str) -> String {
    name.to_ascii_uppercase()
}

fn format_error(error: core::fmt::Error) -> GeneratorError {
    GeneratorError::new(format!("failed to format generated Unicode data: {error}"))
}
