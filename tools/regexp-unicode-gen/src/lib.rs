#![forbid(unsafe_code)]

mod binary;
mod case;
mod emit;
mod error;
mod manifest;
mod strings;
mod ucd;
mod values;

pub use case::{CaseMapping, legacy_reverse_mappings, legacy_uppercase, simple_case_folding};
pub use error::GeneratorError;
pub use manifest::{SourceEntry, SourceManifest};
pub use strings::{GeneratedStringProperty, generate as generate_string_properties};
pub use ucd::{
    CodePointRange, all_data_ranges, property_ranges, property_value_ranges, subtract_ranges,
};

use std::{
    fs,
    path::{Path, PathBuf},
};

const DERIVED_CORE_PROPERTIES: &str = "DerivedCoreProperties.txt";
const GENERAL_CATEGORIES: &str = "extracted/DerivedGeneralCategory.txt";
const PROPERTY_VALUE_ALIASES: &str = "PropertyValueAliases.txt";
const SCRIPTS: &str = "Scripts.txt";
const SCRIPT_EXTENSIONS: &str = "ScriptExtensions.txt";
const CASE_FOLDING: &str = "CaseFolding.txt";
const UNICODE_DATA: &str = "UnicodeData.txt";
const EMOJI_SEQUENCES: &str = "emoji/emoji-sequences.txt";
const EMOJI_ZWJ_SEQUENCES: &str = "emoji/emoji-zwj-sequences.txt";
const BINARY_PROPERTY_SOURCES: &[&str] = &[
    DERIVED_CORE_PROPERTIES,
    "PropList.txt",
    "extracted/DerivedBinaryProperties.txt",
    "DerivedNormalizationProps.txt",
    "emoji/emoji-data.txt",
];
const OUTPUT_FORMAT_VERSION: u32 = 1;
const GENERATOR_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Complete input and output configuration for one deterministic generation.
#[derive(Debug, Clone)]
pub struct GenerationConfig {
    pub input_directory: PathBuf,
    pub source_manifest: PathBuf,
    pub output_directory: PathBuf,
    pub max_input_bytes: usize,
}

impl GenerationConfig {
    #[must_use]
    pub fn new(
        input_directory: impl Into<PathBuf>,
        source_manifest: impl Into<PathBuf>,
        output_directory: impl Into<PathBuf>,
    ) -> Self {
        Self {
            input_directory: input_directory.into(),
            source_manifest: source_manifest.into(),
            output_directory: output_directory.into(),
            max_input_bytes: 64 * 1_024 * 1_024,
        }
    }
}

/// Deterministic observations emitted after successful generation.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GenerationSummary {
    pub unicode_version: String,
    pub id_start_ranges: usize,
    pub id_continue_ranges: usize,
    pub binary_properties: usize,
    pub general_categories: usize,
    pub scripts: usize,
    pub simple_case_mappings: usize,
    pub legacy_uppercase_mappings: usize,
    pub string_properties: usize,
    pub string_sequences: usize,
    pub output_bytes: usize,
}

/// Verifies all pinned sources and deterministically generates Unicode tables.
///
/// # Errors
///
/// Returns an error for invalid configuration, missing or corrupted input,
/// malformed Unicode data, violated invariants, or output I/O failures.
pub fn generate(config: &GenerationConfig) -> Result<GenerationSummary, GeneratorError> {
    let manifest = SourceManifest::read(&config.source_manifest)?;
    manifest.verify(&config.input_directory, config.max_input_bytes)?;
    let binary_sources = read_binary_sources(config, &manifest)?;
    let general_categories = read_source(config, &manifest, GENERAL_CATEGORIES)?;
    let value_aliases = read_source(config, &manifest, PROPERTY_VALUE_ALIASES)?;
    let scripts = read_source(config, &manifest, SCRIPTS)?;
    let script_extensions = read_source(config, &manifest, SCRIPT_EXTENSIONS)?;
    let case_folding = read_source(config, &manifest, CASE_FOLDING)?;
    let unicode_data = read_source(config, &manifest, UNICODE_DATA)?;
    let emoji_sequences = read_source(config, &manifest, EMOJI_SEQUENCES)?;
    let emoji_zwj_sequences = read_source(config, &manifest, EMOJI_ZWJ_SEQUENCES)?;
    let binary_source_refs = binary_sources
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let properties = binary::generate(&binary_source_refs, &general_categories)?;
    let category_values = values::generate_general_categories(&value_aliases, &general_categories)?;
    let (script_values, script_extension_values) =
        values::generate_scripts(&value_aliases, &scripts, &script_extensions)?;
    let simple_case_mappings = case::simple_case_folding(&case_folding)?;
    let legacy_uppercase_mappings = case::legacy_uppercase(&unicode_data)?;
    let string_properties = strings::generate(&emoji_sequences, &emoji_zwj_sequences)?;
    let simple_case_reverse = case::reverse_mappings(&simple_case_mappings);
    let legacy_uppercase_reverse = case::legacy_reverse_mappings(&legacy_uppercase_mappings);
    let id_start_ranges = property_range_count(&properties, "ID_Start")?;
    let id_continue_ranges = property_range_count(&properties, "ID_Continue")?;
    let core_output = emit::core_properties(&manifest, GENERATOR_VERSION, OUTPUT_FORMAT_VERSION)?;
    let binary_output = emit::binary_properties(
        &manifest,
        GENERATOR_VERSION,
        OUTPUT_FORMAT_VERSION,
        &properties,
    )?;
    let category_output = emit::value_properties(
        &manifest,
        GENERATOR_VERSION,
        OUTPUT_FORMAT_VERSION,
        &category_values,
        "GC",
        "general_category_ranges",
    )?;
    let script_output = emit::value_properties(
        &manifest,
        GENERATOR_VERSION,
        OUTPUT_FORMAT_VERSION,
        &script_values,
        "SC",
        "script_ranges",
    )?;
    let script_extension_output = emit::value_properties(
        &manifest,
        GENERATOR_VERSION,
        OUTPUT_FORMAT_VERSION,
        &script_extension_values,
        "SCX",
        "script_extension_ranges",
    )?;
    let case_output = emit::case_mappings(
        &manifest,
        GENERATOR_VERSION,
        OUTPUT_FORMAT_VERSION,
        &simple_case_mappings,
        &simple_case_reverse,
        &legacy_uppercase_mappings,
        &legacy_uppercase_reverse,
    )?;
    let string_output = emit::string_properties(
        &manifest,
        GENERATOR_VERSION,
        OUTPUT_FORMAT_VERSION,
        &string_properties,
    )?;
    let outputs = [
        ("generated_core.rs", &core_output),
        ("generated_binary.rs", &binary_output),
        ("generated_general_category.rs", &category_output),
        ("generated_script.rs", &script_output),
        ("generated_script_extensions.rs", &script_extension_output),
        ("generated_case.rs", &case_output),
        ("generated_string.rs", &string_output),
    ];
    let output_bytes = write_generated_outputs(config, &outputs)?;
    let string_sequences = string_sequence_count(&string_properties)?;
    Ok(GenerationSummary {
        unicode_version: manifest.unicode_version,
        id_start_ranges,
        id_continue_ranges,
        binary_properties: properties.len(),
        general_categories: category_values.len(),
        scripts: script_values.len(),
        simple_case_mappings: simple_case_mappings.len(),
        legacy_uppercase_mappings: legacy_uppercase_mappings.len(),
        string_properties: string_properties.len(),
        string_sequences,
        output_bytes,
    })
}

fn write_generated_outputs(
    config: &GenerationConfig,
    outputs: &[(&str, &String)],
) -> Result<usize, GeneratorError> {
    let mut output_bytes = 0_usize;
    for (name, output) in outputs {
        write_generated_output(config, name, output.as_bytes())?;
        output_bytes = output_bytes
            .checked_add(output.len())
            .ok_or_else(|| GeneratorError::new("generated output byte count overflowed"))?;
    }
    Ok(output_bytes)
}

fn read_binary_sources(
    config: &GenerationConfig,
    manifest: &SourceManifest,
) -> Result<Vec<String>, GeneratorError> {
    BINARY_PROPERTY_SOURCES
        .iter()
        .map(|relative_path| read_source(config, manifest, relative_path))
        .collect()
}

fn string_sequence_count(properties: &[GeneratedStringProperty]) -> Result<usize, GeneratorError> {
    properties.iter().try_fold(0_usize, |total, property| {
        total
            .checked_add(property.sequences.len())
            .ok_or_else(|| GeneratorError::new("generated string sequence count overflowed"))
    })
}

fn read_source(
    config: &GenerationConfig,
    manifest: &SourceManifest,
    relative_path: &str,
) -> Result<String, GeneratorError> {
    manifest.require_source(relative_path)?;
    read_bounded(
        &config.input_directory.join(relative_path),
        config.max_input_bytes,
    )
}

fn property_range_count(
    properties: &[binary::GeneratedProperty],
    name: &str,
) -> Result<usize, GeneratorError> {
    properties
        .iter()
        .find(|property| property.spec.canonical == name)
        .map(|property| property.ranges.len())
        .ok_or_else(|| GeneratorError::new(format!("generated property {name} is missing")))
}

fn write_generated_output(
    config: &GenerationConfig,
    file_name: &str,
    bytes: &[u8],
) -> Result<(), GeneratorError> {
    write_output(&config.output_directory.join(file_name), bytes)
}

fn read_bounded(path: &Path, limit: usize) -> Result<String, GeneratorError> {
    let metadata = fs::metadata(path).map_err(|error| {
        GeneratorError::new(format!("failed to inspect {}: {error}", path.display()))
    })?;
    let length = usize::try_from(metadata.len()).map_err(|error| {
        GeneratorError::new(format!(
            "input size did not fit usize for {}: {error}",
            path.display()
        ))
    })?;
    if length > limit {
        return Err(GeneratorError::new(format!(
            "input {} has {length} bytes, exceeding {limit}",
            path.display()
        )));
    }
    fs::read_to_string(path)
        .map_err(|error| GeneratorError::new(format!("failed to read {}: {error}", path.display())))
}

fn write_output(path: &Path, bytes: &[u8]) -> Result<(), GeneratorError> {
    let parent = path.parent().ok_or_else(|| {
        GeneratorError::new(format!("output {} has no parent directory", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        GeneratorError::new(format!("failed to create {}: {error}", parent.display()))
    })?;
    fs::write(path, bytes).map_err(|error| {
        GeneratorError::new(format!("failed to write {}: {error}", path.display()))
    })
}
