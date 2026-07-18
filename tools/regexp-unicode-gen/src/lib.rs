#![forbid(unsafe_code)]

mod binary;
mod emit;
mod error;
mod manifest;
mod ucd;
mod values;

pub use error::GeneratorError;
pub use manifest::{SourceEntry, SourceManifest};
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
    let mut binary_sources = Vec::with_capacity(BINARY_PROPERTY_SOURCES.len());
    for relative_path in BINARY_PROPERTY_SOURCES {
        binary_sources.push(read_source(config, &manifest, relative_path)?);
    }
    let general_categories = read_source(config, &manifest, GENERAL_CATEGORIES)?;
    let value_aliases = read_source(config, &manifest, PROPERTY_VALUE_ALIASES)?;
    let scripts = read_source(config, &manifest, SCRIPTS)?;
    let script_extensions = read_source(config, &manifest, SCRIPT_EXTENSIONS)?;
    let binary_source_refs = binary_sources
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let properties = binary::generate(&binary_source_refs, &general_categories)?;
    let category_values = values::generate_general_categories(&value_aliases, &general_categories)?;
    let (script_values, script_extension_values) =
        values::generate_scripts(&value_aliases, &scripts, &script_extensions)?;
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
    write_generated_output(config, "generated_core.rs", core_output.as_bytes())?;
    write_generated_output(config, "generated_binary.rs", binary_output.as_bytes())?;
    write_generated_output(
        config,
        "generated_general_category.rs",
        category_output.as_bytes(),
    )?;
    write_generated_output(config, "generated_script.rs", script_output.as_bytes())?;
    write_generated_output(
        config,
        "generated_script_extensions.rs",
        script_extension_output.as_bytes(),
    )?;
    let output_bytes = [
        core_output.len(),
        binary_output.len(),
        category_output.len(),
        script_output.len(),
        script_extension_output.len(),
    ]
    .into_iter()
    .try_fold(0_usize, usize::checked_add)
    .ok_or_else(|| GeneratorError::new("generated output byte count overflowed"))?;
    Ok(GenerationSummary {
        unicode_version: manifest.unicode_version,
        id_start_ranges,
        id_continue_ranges,
        binary_properties: properties.len(),
        general_categories: category_values.len(),
        scripts: script_values.len(),
        output_bytes,
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
