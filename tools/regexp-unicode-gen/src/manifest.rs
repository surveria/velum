use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path},
};

use sha2::{Digest, Sha256};

use crate::GeneratorError;

const FORMAT_PREFIX: &str = "format=";
const UNICODE_PREFIX: &str = "unicode=";
const SOURCE_PREFIX: &str = "sha256 ";
const DIGEST_HEX_LENGTH: usize = 64;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceEntry {
    pub relative_path: String,
    pub sha256: String,
    pub source_url: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceManifest {
    pub format_version: u32,
    pub unicode_version: String,
    pub sources: Vec<SourceEntry>,
}

impl SourceManifest {
    /// Reads and parses a source manifest.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read or its contents are invalid.
    pub fn read(path: &Path) -> Result<Self, GeneratorError> {
        let contents = fs::read_to_string(path).map_err(|error| {
            GeneratorError::new(format!("failed to read {}: {error}", path.display()))
        })?;
        Self::parse(&contents)
    }

    /// Parses a source manifest from UTF-8 text.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed headers, digests, paths, or duplicates.
    pub fn parse(contents: &str) -> Result<Self, GeneratorError> {
        let mut format_version = None;
        let mut unicode_version = None;
        let mut sources = Vec::new();
        let mut paths = BTreeSet::new();
        for (line_index, raw_line) in contents.lines().enumerate() {
            let line_number = line_index
                .checked_add(1)
                .ok_or_else(|| GeneratorError::new("source manifest line number overflowed"))?;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(value) = line.strip_prefix(FORMAT_PREFIX) {
                if format_version.is_some() {
                    return Err(line_error(line_number, "duplicate format version"));
                }
                format_version = Some(value.parse::<u32>().map_err(|error| {
                    line_error(line_number, &format!("invalid format version: {error}"))
                })?);
                continue;
            }
            if let Some(value) = line.strip_prefix(UNICODE_PREFIX) {
                if unicode_version.is_some() || value.is_empty() {
                    return Err(line_error(line_number, "invalid Unicode version"));
                }
                unicode_version = Some(value.to_owned());
                continue;
            }
            let Some(entry) = line.strip_prefix(SOURCE_PREFIX) else {
                return Err(line_error(line_number, "unknown manifest directive"));
            };
            let mut fields = entry.split_ascii_whitespace();
            let digest = fields
                .next()
                .ok_or_else(|| line_error(line_number, "missing SHA-256 digest"))?;
            let relative_path = fields
                .next()
                .ok_or_else(|| line_error(line_number, "missing source path"))?;
            let source_url = fields
                .next()
                .ok_or_else(|| line_error(line_number, "missing source URL"))?;
            if fields.next().is_some() {
                return Err(line_error(line_number, "unexpected trailing fields"));
            }
            validate_digest(digest, line_number)?;
            validate_relative_path(relative_path, line_number)?;
            validate_source_url(source_url, line_number)?;
            if !paths.insert(relative_path.to_owned()) {
                return Err(line_error(line_number, "duplicate source path"));
            }
            sources.push(SourceEntry {
                relative_path: relative_path.to_owned(),
                sha256: digest.to_ascii_lowercase(),
                source_url: source_url.to_owned(),
            });
        }
        let format_version = format_version
            .ok_or_else(|| GeneratorError::new("source manifest is missing format version"))?;
        if format_version != 1 {
            return Err(GeneratorError::new(format!(
                "unsupported source manifest format {format_version}"
            )));
        }
        let unicode_version = unicode_version
            .ok_or_else(|| GeneratorError::new("source manifest is missing Unicode version"))?;
        if sources.is_empty() {
            return Err(GeneratorError::new("source manifest has no source files"));
        }
        Ok(Self {
            format_version,
            unicode_version,
            sources,
        })
    }

    /// Verifies every source size and SHA-256 digest.
    ///
    /// # Errors
    ///
    /// Returns an error when a source is missing, oversized, unreadable, or
    /// does not match its pinned digest.
    pub fn verify(&self, input_directory: &Path, limit: usize) -> Result<(), GeneratorError> {
        for source in &self.sources {
            let path = input_directory.join(&source.relative_path);
            let metadata = fs::metadata(&path).map_err(|error| {
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
            let bytes = fs::read(&path).map_err(|error| {
                GeneratorError::new(format!("failed to read {}: {error}", path.display()))
            })?;
            let digest = hex_digest(&bytes)?;
            if digest != source.sha256 {
                return Err(GeneratorError::new(format!(
                    "SHA-256 mismatch for {}: expected {}, got {digest}",
                    source.relative_path, source.sha256
                )));
            }
        }
        Ok(())
    }

    /// Requires one named source to be present in the manifest.
    ///
    /// # Errors
    ///
    /// Returns an error when the source is not pinned.
    pub fn require_source(&self, relative_path: &str) -> Result<(), GeneratorError> {
        if self
            .sources
            .iter()
            .any(|source| source.relative_path == relative_path)
        {
            return Ok(());
        }
        Err(GeneratorError::new(format!(
            "source manifest is missing {relative_path}"
        )))
    }
}

fn validate_digest(digest: &str, line_number: usize) -> Result<(), GeneratorError> {
    if digest.len() != DIGEST_HEX_LENGTH || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(line_error(line_number, "invalid SHA-256 digest"));
    }
    Ok(())
}

fn validate_relative_path(path: &str, line_number: usize) -> Result<(), GeneratorError> {
    let path = Path::new(path);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(line_error(
            line_number,
            "source path must be relative and normalized",
        ));
    }
    Ok(())
}

fn validate_source_url(url: &str, line_number: usize) -> Result<(), GeneratorError> {
    if !url.starts_with("https://") || url.contains("/latest/") {
        return Err(line_error(
            line_number,
            "source URL must be archival HTTPS and must not contain /latest/",
        ));
    }
    Ok(())
}

fn line_error(line_number: usize, message: &str) -> GeneratorError {
    GeneratorError::new(format!("source manifest line {line_number}: {message}"))
}

fn hex_digest(bytes: &[u8]) -> Result<String, GeneratorError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(DIGEST_HEX_LENGTH);
    for byte in digest {
        let high = HEX
            .get(usize::from(byte >> 4))
            .copied()
            .ok_or_else(|| GeneratorError::new("SHA-256 high nibble exceeded hex alphabet"))?;
        let low = HEX
            .get(usize::from(byte & 0x0F))
            .copied()
            .ok_or_else(|| GeneratorError::new("SHA-256 low nibble exceeded hex alphabet"))?;
        output.push(char::from(high));
        output.push(char::from(low));
    }
    Ok(output)
}
