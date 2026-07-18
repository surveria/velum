use core::fmt::Write;

use crate::{
    CodePointRange, GeneratorError, SourceManifest,
    binary::{GeneratedProperty, PropertySpec},
};

const RANGES_PER_LINE: usize = 32;

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
