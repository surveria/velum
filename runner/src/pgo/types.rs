use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{BufReader, Read as _},
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub(super) const CASE_SUFFIXES: [&str; 6] = [
    "object_transform",
    "method_dispatch",
    "json_ingestion",
    "string_processing",
    "collection_index",
    "tree_allocation",
];
pub(super) const VARIANTS: [&str; 3] = ["ordinary", "instrumented", "pgo"];
pub(super) const BASELINE_PATH: &str = "tests/corpora/test262/full-pass-baseline.txt";

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Experiment {
    pub schema_version: u32,
    pub protocol_version: String,
    pub commit: String,
    pub tree: String,
    pub host: String,
    pub cpu: String,
    pub rounds: u32,
    pub preset: String,
    pub source: PathBuf,
    pub cases: Vec<CaseSource>,
    pub binaries: BTreeMap<String, Binary>,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CaseSource {
    pub id: String,
    pub source: String,
    pub sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Binary {
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
}

impl Experiment {
    pub fn load(run: &Path) -> Result<Self> {
        let result: Self = read_json(&run.join("experiment.json"))?;
        ensure!(
            result.schema_version == 1,
            "unsupported PGO experiment schema"
        );
        ensure!(
            result.protocol_version == "2-frozen-before-pgo-training",
            "unsupported PGO measurement protocol"
        );
        ensure!(
            hex_digest(&result.commit, 40) && hex_digest(&result.tree, 40),
            "invalid source identity"
        );
        ensure!(
            result.host.starts_with("x86_64-") && result.host.contains("linux"),
            "unsupported PGO host"
        );
        ensure!(
            matches!(result.preset.as_str(), "release" | "thin-lto"),
            "unknown PGO preset"
        );
        ensure!(
            (1..=5).contains(&result.rounds),
            "PGO rounds must be between one and five"
        );
        ensure!(
            result.cpu == "inherit" || result.cpu.parse::<u32>().is_ok(),
            "invalid PGO CPU selection"
        );
        ensure!(
            result.source.is_absolute() && result.source.canonicalize()? == result.source,
            "source path must be absolute and canonical"
        );
        let expected: BTreeSet<_> = ["representative", "holdout"]
            .into_iter()
            .flat_map(|cohort| CASE_SUFFIXES.map(|suffix| format!("{cohort}_{suffix}")))
            .collect();
        let actual: BTreeSet<_> = result.cases.iter().map(|case| case.id.clone()).collect();
        ensure!(
            result.cases.len() == expected.len() && actual == expected,
            "frozen workload membership mismatch"
        );
        for case in &result.cases {
            ensure!(
                case.source == format!("tests/corpora/benchmarks/prepared/{}.js", case.id),
                "invalid workload path for {}",
                case.id
            );
            let path = result.source.join(&case.source);
            ensure!(
                path.canonicalize()? == path,
                "workload must not resolve through a symlink"
            );
            check_hash(&path, &case.sha256)?;
        }
        ensure!(
            result
                .binaries
                .keys()
                .all(|variant| VARIANTS.contains(&variant.as_str())),
            "unknown executable variant"
        );
        Ok(result)
    }

    pub fn binary(&self, variant: &str) -> Result<&Binary> {
        ensure!(VARIANTS.contains(&variant), "unknown PGO variant {variant}");
        let binary = self
            .binaries
            .get(variant)
            .with_context(|| format!("missing {variant} executable"))?;
        ensure!(
            binary.path.is_absolute() && binary.path.canonicalize()? == binary.path,
            "executable path must be absolute and canonical"
        );
        check_hash(&binary.path, &binary.sha256)?;
        ensure!(
            binary.bytes > 0 && fs::metadata(&binary.path)?.len() == binary.bytes,
            "executable size changed"
        );
        Ok(binary)
    }

    pub fn case(&self, id: &str) -> Result<&CaseSource> {
        self.cases
            .iter()
            .find(|case| case.id == id)
            .with_context(|| format!("unknown exact workload ID {id}"))
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Checksum {
    Number { bits: u64 },
    Boolean { value: bool },
    String { value: String },
}

impl Checksum {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !matches!(self, Self::String { value } if value.is_empty()),
            "empty workload checksum"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct VerifiedReport {
    pub schema_version: u32,
    pub commit: String,
    pub tree: String,
    pub host: String,
    pub preset: String,
    pub rounds: u32,
    pub requested_cpu: String,
    pub cpu_affinity: String,
    pub source_root: PathBuf,
    pub sources: Vec<CaseSource>,
    pub source_manifest_sha256: String,
    pub variant: String,
    pub binary_path: PathBuf,
    pub binary_sha256: String,
    pub report_path: PathBuf,
    pub report_sha256: String,
    pub kind: String,
    pub expected_case: Option<String>,
    pub performance: Option<PerformanceEvidence>,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PerformanceEvidence {
    pub checksum: Checksum,
    pub settings: Sampling,
    pub engine_ns: u64,
    pub reference_ns: u64,
    pub engine_cv_permille: u32,
    pub reference_cv_permille: u32,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Sampling {
    pub warmup_duration_ns: u64,
    pub minimum_sample_duration_ns: u64,
    pub samples: u64,
    pub minimum_operation_duration_ns: u64,
    pub maximum_cv_permille: u32,
    pub attempts: u64,
    pub maximum_operation_duration_ns: u64,
    pub maximum_total_duration_ns: u64,
}

impl Sampling {
    pub const fn for_case(long_case: bool) -> Self {
        Self {
            warmup_duration_ns: 150_000_000,
            minimum_sample_duration_ns: if long_case {
                15_000_000_000
            } else {
                5_000_000_000
            },
            samples: if long_case { 3 } else { 5 },
            minimum_operation_duration_ns: 1_000_000,
            maximum_cv_permille: 100,
            attempts: 3,
            maximum_operation_duration_ns: 2_000_000_000,
            maximum_total_duration_ns: if long_case {
                60_000_000_000
            } else {
                30_000_000_000
            },
        }
    }
}

pub(super) fn label_path(run: &Path, label: &str, extension: &str) -> Result<PathBuf> {
    ensure!(
        !label.is_empty()
            && label.len() <= 160
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
        "invalid PGO report label"
    );
    Ok(run.join("reports").join(format!("{label}{extension}")))
}

pub(super) fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_reader(BufReader::new(
        File::open(path).with_context(|| format!("failed to open {}", path.display()))?,
    ))
    .with_context(|| format!("failed to parse {}", path.display()))
}

pub(super) fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut text =
        serde_json::to_string_pretty(value).context("failed to serialize PGO evidence")?;
    text.push('\n');
    fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))
}

pub(super) fn hex_digest(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn sha256(path: &Path) -> Result<String> {
    ensure!(path.is_file(), "missing evidence file {}", path.display());
    let output = Command::new("sha256sum")
        .arg("--binary")
        .arg("--")
        .arg(path)
        .output()
        .with_context(|| format!("failed to hash {} with sha256sum", path.display()))?;
    ensure!(
        output.status.success() && output.stderr.is_empty(),
        "sha256sum failed for {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).context("sha256sum returned non-UTF-8 output")?;
    let digest = text
        .split_whitespace()
        .next()
        .context("sha256sum returned no digest")?;
    ensure!(hex_digest(digest, 64), "sha256sum returned invalid digest");
    Ok(digest.to_owned())
}

pub(super) fn check_hash(path: &Path, expected: &str) -> Result<()> {
    ensure!(
        hex_digest(expected, 64),
        "invalid expected SHA-256 for {}",
        path.display()
    );
    ensure!(
        sha256(path)? == expected,
        "SHA-256 changed for {}",
        path.display()
    );
    Ok(())
}

pub(super) fn fnv_digest(path: &Path) -> Result<String> {
    let mut file = File::open(path).context("failed to open executable for memory identity")?;
    let mut buffer = [0_u8; 16_384];
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    loop {
        let count = file
            .read(&mut buffer)
            .context("failed to read memory executable")?;
        if count == 0 {
            break;
        }
        for byte in buffer
            .get(..count)
            .context("executable read exceeded buffer")?
        {
            hash ^= u64::from(*byte);
            hash = hash.overflowing_mul(0x0000_0100_0000_01b3).0;
        }
    }
    Ok(format!("fnv1a64-{hash:016x}"))
}
