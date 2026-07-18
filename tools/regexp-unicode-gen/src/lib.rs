#![forbid(unsafe_code)]

mod emit;
mod error;
mod manifest;
mod ucd;

pub use error::GeneratorError;
pub use manifest::{SourceEntry, SourceManifest};
pub use ucd::{CodePointRange, property_ranges};

use std::{
    fs,
    path::{Path, PathBuf},
};

const DERIVED_CORE_PROPERTIES: &str = "DerivedCoreProperties.txt";
const OUTPUT_FORMAT_VERSION: u32 = 1;
const GENERATOR_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Complete input and output configuration for one deterministic generation.
#[derive(Debug, Clone)]
pub struct GenerationConfig {
    pub input_directory: PathBuf,
    pub source_manifest: PathBuf,
    pub output: PathBuf,
    pub max_input_bytes: usize,
}

impl GenerationConfig {
    #[must_use]
    pub fn new(
        input_directory: impl Into<PathBuf>,
        source_manifest: impl Into<PathBuf>,
        output: impl Into<PathBuf>,
    ) -> Self {
        Self {
            input_directory: input_directory.into(),
            source_manifest: source_manifest.into(),
            output: output.into(),
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
    manifest.require_source(DERIVED_CORE_PROPERTIES)?;
    let source_path = config.input_directory.join(DERIVED_CORE_PROPERTIES);
    let source = read_bounded(&source_path, config.max_input_bytes)?;
    let id_start = ucd::property_ranges(&source, "ID_Start")?;
    let id_continue = ucd::property_ranges(&source, "ID_Continue")?;
    let output = emit::core_properties(
        &manifest,
        GENERATOR_VERSION,
        OUTPUT_FORMAT_VERSION,
        &id_start,
        &id_continue,
    )?;
    write_output(&config.output, output.as_bytes())?;
    Ok(GenerationSummary {
        unicode_version: manifest.unicode_version,
        id_start_ranges: id_start.len(),
        id_continue_ranges: id_continue.len(),
        output_bytes: output.len(),
    })
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
