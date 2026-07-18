use core::fmt::Write;

use crate::{CodePointRange, GeneratorError, SourceManifest};

const RANGES_PER_LINE: usize = 8;

pub fn core_properties(
    manifest: &SourceManifest,
    generator_version: &str,
    format_version: u32,
    id_start: &[CodePointRange],
    id_continue: &[CodePointRange],
) -> Result<String, GeneratorError> {
    let mut output = String::new();
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
    writeln!(output).map_err(format_error)?;
    writeln!(
        output,
        "pub const UNICODE_VERSION: &str = \"{}\";",
        manifest.unicode_version
    )
    .map_err(format_error)?;
    emit_ranges(&mut output, "ID_START", id_start)?;
    emit_ranges(&mut output, "ID_CONTINUE", id_continue)?;
    Ok(output)
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
            write!(output, "(0x{:X}, 0x{:X}),", range.start, range.end).map_err(format_error)?;
        }
        output.push('\n');
    }
    output.push_str("];\n");
    Ok(())
}

fn format_error(error: core::fmt::Error) -> GeneratorError {
    GeneratorError::new(format!("failed to format generated Unicode data: {error}"))
}
