//! Content-addressed `QuickJS` reference measurements.
//!
//! Baselines are keyed by every input that can change the measured value. The
//! compact TSV is deterministic, reviewable, and deliberately independent of
//! the human-readable report format.

use std::{
    collections::BTreeMap,
    env, fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context as _, bail};

use crate::{
    bench_measure::{MeasureConfig, MeasureSnapshot, MeasureStats},
    benchmark_protocol::{BenchmarkChecksum, PREPARED_PROTOCOL_VERSION},
};

pub const ENV_BASELINE_MODE: &str = "VELUM_QUICKJS_BASELINE";
pub const ENV_BASELINE_PATH: &str = "VELUM_QUICKJS_BASELINE_PATH";

const DEFAULT_BASELINE_PATH: &str = "tests/corpora/benchmarks/quickjs-baseline.tsv";
const MODE_OFF: &str = "off";
const MODE_READ: &str = "read";
const MODE_REQUIRE: &str = "require";
const MODE_REFRESH: &str = "refresh";
const SCHEMA_VERSION: &str = "1";
const HEADER: &str = "schema_version\tcontent_id\tcase_id\tsource_digest\tharness_digest\tprotocol\tmeasure_config\treference_engine\thost_profile\tchecksum\tmedian_ns\tcv_permille\titers_per_sample\tsamples\tmedian_sample_ns\twarmup_ns\ttimed_run_ns\titeration_cap";

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum BaselineMode {
    Off,
    Read,
    Require,
    Refresh,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BaselineKey {
    case_id: String,
    source_digest: String,
    harness_digest: String,
    protocol: String,
    measure_config: String,
    reference_engine: String,
    host_profile: String,
}

impl BaselineKey {
    pub fn new(
        case_id: &str,
        source: &str,
        harness: &str,
        measure_config: MeasureConfig,
        reference_engine: &str,
        host_profile: &str,
    ) -> Self {
        Self {
            case_id: normalize_field(case_id),
            source_digest: stable_digest(source.as_bytes()),
            harness_digest: stable_digest(harness.as_bytes()),
            protocol: PREPARED_PROTOCOL_VERSION.to_owned(),
            measure_config: measure_config.fingerprint(),
            reference_engine: normalize_field(reference_engine),
            host_profile: normalize_field(host_profile),
        }
    }

    pub fn content_id(&self) -> String {
        stable_digest(self.canonical_text().as_bytes())
    }

    fn canonical_text(&self) -> String {
        [
            self.case_id.as_str(),
            self.source_digest.as_str(),
            self.harness_digest.as_str(),
            self.protocol.as_str(),
            self.measure_config.as_str(),
            self.reference_engine.as_str(),
            self.host_profile.as_str(),
        ]
        .join("\0")
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BaselineSample {
    pub checksum: BenchmarkChecksum,
    pub snapshot: MeasureSnapshot,
}

impl BaselineSample {
    pub const fn from_measurement(checksum: BenchmarkChecksum, stats: MeasureStats) -> Self {
        Self {
            checksum,
            snapshot: stats.snapshot(),
        }
    }

    pub const fn stats(&self, config: MeasureConfig) -> MeasureStats {
        MeasureStats::from_snapshot(self.snapshot, config)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct BaselineEntry {
    key: BaselineKey,
    sample: BaselineSample,
}

#[derive(Debug, Default)]
struct BaselineStore {
    entries: BTreeMap<String, BaselineEntry>,
}

impl BaselineStore {
    fn read_optional(path: &Path) -> anyhow::Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => Self::parse(&text),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error)
                .with_context(|| format!("failed to read QuickJS baseline '{}'", path.display())),
        }
    }

    fn parse(text: &str) -> anyhow::Result<Self> {
        let mut lines = text.lines();
        let Some(header) = lines.next() else {
            bail!("QuickJS baseline is empty")
        };
        if header != HEADER {
            bail!("QuickJS baseline has an unsupported header")
        }
        let mut store = Self::default();
        for (offset, line) in lines.enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let line_number = offset.saturating_add(2);
            let entry = BaselineEntry::parse(line, line_number)?;
            store.insert(entry)?;
        }
        Ok(store)
    }

    fn lookup(&self, key: &BaselineKey) -> Option<BaselineSample> {
        self.entries
            .get(&key.content_id())
            .filter(|entry| entry.key == *key)
            .map(|entry| entry.sample.clone())
    }

    fn insert(&mut self, entry: BaselineEntry) -> anyhow::Result<()> {
        let content_id = entry.key.content_id();
        if let Some(existing) = self.entries.get(&content_id)
            && existing.key != entry.key
        {
            bail!("QuickJS baseline content id collision for '{content_id}'")
        }
        self.entries.insert(content_id, entry);
        Ok(())
    }

    fn replace_case(&mut self, entry: BaselineEntry) -> anyhow::Result<()> {
        self.entries
            .retain(|_content_id, existing| existing.key.case_id != entry.key.case_id);
        self.insert(entry)
    }

    fn render(&self) -> String {
        let mut output = String::from(HEADER);
        output.push('\n');
        for (content_id, entry) in &self.entries {
            output.push_str(&entry.render(content_id));
            output.push('\n');
        }
        output
    }
}

impl BaselineEntry {
    fn parse(line: &str, line_number: usize) -> anyhow::Result<Self> {
        let mut fields = line.split('\t');
        let schema = next_field(&mut fields, line_number, "schema_version")?;
        if schema != SCHEMA_VERSION {
            bail!("QuickJS baseline line {line_number} uses schema '{schema}'")
        }
        let stored_content_id = next_field(&mut fields, line_number, "content_id")?;
        let key = BaselineKey {
            case_id: next_field(&mut fields, line_number, "case_id")?.to_owned(),
            source_digest: next_field(&mut fields, line_number, "source_digest")?.to_owned(),
            harness_digest: next_field(&mut fields, line_number, "harness_digest")?.to_owned(),
            protocol: next_field(&mut fields, line_number, "protocol")?.to_owned(),
            measure_config: next_field(&mut fields, line_number, "measure_config")?.to_owned(),
            reference_engine: next_field(&mut fields, line_number, "reference_engine")?.to_owned(),
            host_profile: next_field(&mut fields, line_number, "host_profile")?.to_owned(),
        };
        if key.content_id() != stored_content_id {
            bail!("QuickJS baseline line {line_number} has a stale content id")
        }
        let checksum = BenchmarkChecksum::from_storage_text(next_field(
            &mut fields,
            line_number,
            "checksum",
        )?)?;
        let median = parse_duration(&mut fields, line_number, "median_ns")?;
        let cv_permille = parse_field(&mut fields, line_number, "cv_permille")?;
        let iters_per_sample = parse_field(&mut fields, line_number, "iters_per_sample")?;
        let samples = parse_field(&mut fields, line_number, "samples")?;
        let median_sample = parse_duration(&mut fields, line_number, "median_sample_ns")?;
        let warmup_elapsed = parse_duration(&mut fields, line_number, "warmup_ns")?;
        let timed_run_elapsed = parse_duration(&mut fields, line_number, "timed_run_ns")?;
        let iteration_cap_reached = match next_field(&mut fields, line_number, "iteration_cap")? {
            "0" => false,
            "1" => true,
            value => {
                bail!("QuickJS baseline line {line_number} has invalid iteration_cap '{value}'")
            }
        };
        if let Some(extra) = fields.next() {
            bail!("QuickJS baseline line {line_number} has an extra field '{extra}'")
        }
        Ok(Self {
            key,
            sample: BaselineSample {
                checksum,
                snapshot: MeasureSnapshot {
                    median,
                    cv_permille,
                    iters_per_sample,
                    samples,
                    median_sample,
                    warmup_elapsed,
                    timed_run_elapsed,
                    iteration_cap_reached,
                },
            },
        })
    }

    fn render(&self, content_id: &str) -> String {
        let snapshot = self.sample.snapshot;
        [
            SCHEMA_VERSION.to_owned(),
            content_id.to_owned(),
            self.key.case_id.clone(),
            self.key.source_digest.clone(),
            self.key.harness_digest.clone(),
            self.key.protocol.clone(),
            self.key.measure_config.clone(),
            self.key.reference_engine.clone(),
            self.key.host_profile.clone(),
            self.sample.checksum.storage_text(),
            snapshot.median.as_nanos().to_string(),
            snapshot.cv_permille.to_string(),
            snapshot.iters_per_sample.to_string(),
            snapshot.samples.to_string(),
            snapshot.median_sample.as_nanos().to_string(),
            snapshot.warmup_elapsed.as_nanos().to_string(),
            snapshot.timed_run_elapsed.as_nanos().to_string(),
            if snapshot.iteration_cap_reached {
                "1".to_owned()
            } else {
                "0".to_owned()
            },
        ]
        .join("\t")
    }
}

#[derive(Debug)]
pub struct QuickjsBaseline {
    mode: BaselineMode,
    path: PathBuf,
    store: BaselineStore,
    dirty: bool,
}

impl QuickjsBaseline {
    pub fn from_env() -> anyhow::Result<Self> {
        let mode = match env::var(ENV_BASELINE_MODE)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .as_deref()
        {
            None | Some(MODE_READ) => BaselineMode::Read,
            Some(MODE_REQUIRE) => BaselineMode::Require,
            Some(MODE_REFRESH) => BaselineMode::Refresh,
            Some(MODE_OFF) => BaselineMode::Off,
            Some(value) => bail!(
                "unknown QuickJS baseline mode '{value}'; expected '{MODE_READ}', '{MODE_REQUIRE}', '{MODE_REFRESH}', or '{MODE_OFF}'"
            ),
        };
        let path = env::var_os(ENV_BASELINE_PATH)
            .map_or_else(|| PathBuf::from(DEFAULT_BASELINE_PATH), PathBuf::from);
        let store = if mode == BaselineMode::Off {
            BaselineStore::default()
        } else {
            BaselineStore::read_optional(&path)?
        };
        Ok(Self {
            mode,
            path,
            store,
            dirty: false,
        })
    }

    pub fn lookup(&self, key: &BaselineKey) -> anyhow::Result<Option<BaselineSample>> {
        if !matches!(self.mode, BaselineMode::Read | BaselineMode::Require) {
            return Ok(None);
        }
        let sample = self.store.lookup(key);
        if self.mode == BaselineMode::Require && sample.is_none() {
            bail!(
                "required QuickJS baseline entry is missing for benchmark '{}' (content id {}); refresh the committed baseline explicitly",
                key.case_id,
                key.content_id()
            );
        }
        Ok(sample)
    }

    pub fn record(&mut self, key: BaselineKey, sample: BaselineSample) -> anyhow::Result<()> {
        if self.mode != BaselineMode::Refresh {
            return Ok(());
        }
        self.store.replace_case(BaselineEntry { key, sample })?;
        self.dirty = true;
        Ok(())
    }

    pub fn finish(&mut self) -> anyhow::Result<()> {
        if self.mode != BaselineMode::Refresh || !self.dirty {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create QuickJS baseline directory '{}'",
                    parent.display()
                )
            })?;
        }
        fs::write(&self.path, self.store.render()).with_context(|| {
            format!("failed to write QuickJS baseline '{}'", self.path.display())
        })?;
        self.dirty = false;
        Ok(())
    }
}

pub fn detect_host_profile() -> String {
    let cpu_model = read_cpu_model().unwrap_or_else(|| "unknown".to_owned());
    let logical_cpus = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let kernel = fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map_or_else(
            || "unknown".to_owned(),
            |value| normalize_field(value.trim()),
        );
    let governor = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
        .ok()
        .map_or_else(
            || "unknown".to_owned(),
            |value| normalize_field(value.trim()),
        );
    format!(
        "arch={}|os={}|cpu={}|logical_cpus={logical_cpus}|kernel={kernel}|governor={governor}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        normalize_field(&cpu_model),
    )
}

pub fn harness_descriptor(mode: &str, input: &str) -> String {
    format!(
        "protocol={PREPARED_PROTOCOL_VERSION}|mode={}|input={}|setup=__velumBenchSetup|run=__velumBenchRun|verify=__velumBenchVerify|timer=rust-instant",
        normalize_field(mode),
        normalize_field(input),
    )
}

fn read_cpu_model() -> Option<String> {
    let cpuinfo = fs::read_to_string("/proc/cpuinfo").ok()?;
    cpuinfo.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if matches!(name.trim(), "model name" | "Hardware") {
            return Some(value.trim().to_owned());
        }
        None
    })
}

fn next_field<'a>(
    fields: &mut impl Iterator<Item = &'a str>,
    line_number: usize,
    name: &str,
) -> anyhow::Result<&'a str> {
    fields
        .next()
        .with_context(|| format!("QuickJS baseline line {line_number} is missing '{name}'"))
}

fn parse_field<'a, T>(
    fields: &mut impl Iterator<Item = &'a str>,
    line_number: usize,
    name: &str,
) -> anyhow::Result<T>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let value = next_field(fields, line_number, name)?;
    value
        .parse::<T>()
        .with_context(|| format!("QuickJS baseline line {line_number} has invalid '{name}'"))
}

fn parse_duration<'a>(
    fields: &mut impl Iterator<Item = &'a str>,
    line_number: usize,
    name: &str,
) -> anyhow::Result<Duration> {
    parse_field(fields, line_number, name).map(Duration::from_nanos)
}

pub fn normalize_field(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '\t' | '\n' | '\r' => ' ',
            _ => character,
        })
        .collect()
}

pub fn stable_digest(bytes: &[u8]) -> String {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        let (next, _overflowed) = hash.overflowing_mul(FNV_PRIME);
        hash = next;
    }
    format!("fnv1a64-{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::{
        BaselineEntry, BaselineKey, BaselineMode, BaselineSample, BaselineStore, QuickjsBaseline,
        stable_digest,
    };
    use crate::{
        bench_measure::{MeasureConfig, MeasureSnapshot},
        benchmark_protocol::BenchmarkChecksum,
    };
    use std::time::Duration;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn digest_is_stable_and_content_sensitive() -> TestResult {
        let first = stable_digest(b"benchmark");
        let second = stable_digest(b"benchmark");
        if first != second {
            return Err("stable digest changed for identical input".into());
        }
        if first == stable_digest(b"benchmark-2") {
            return Err("stable digest ignored changed content".into());
        }
        Ok(())
    }

    #[test]
    fn baseline_round_trip_preserves_typed_key_and_sample() -> TestResult {
        let config = test_config();
        let key = BaselineKey::new("case", "source", "harness", config, "quickjs", "host");
        let sample = test_sample();
        let mut store = BaselineStore::default();
        store.insert(BaselineEntry {
            key: key.clone(),
            sample: sample.clone(),
        })?;
        let encoded = store.render();
        let decoded = BaselineStore::parse(&encoded)?;
        let Some(actual) = decoded.lookup(&key) else {
            return Err("round-tripped baseline did not contain its key".into());
        };
        if actual != sample {
            return Err("round-tripped baseline changed its sample".into());
        }
        Ok(())
    }

    #[test]
    fn every_key_dimension_invalidates_the_content_id() -> TestResult {
        let config = test_config();
        let base = BaselineKey::new("case", "source", "harness", config, "quickjs", "host");
        let alternatives = [
            BaselineKey::new("case-2", "source", "harness", config, "quickjs", "host"),
            BaselineKey::new("case", "source-2", "harness", config, "quickjs", "host"),
            BaselineKey::new("case", "source", "harness-2", config, "quickjs", "host"),
            BaselineKey::new("case", "source", "harness", config, "quickjs-2", "host"),
            BaselineKey::new("case", "source", "harness", config, "quickjs", "host-2"),
        ];
        for alternative in alternatives {
            if alternative.content_id() == base.content_id() {
                return Err("baseline key dimension did not change the content id".into());
            }
        }
        Ok(())
    }

    #[test]
    fn refresh_replaces_stale_entry_for_the_same_case() -> TestResult {
        let config = test_config();
        let old_key = BaselineKey::new("case", "old", "harness", config, "quickjs", "host");
        let new_key = BaselineKey::new("case", "new", "harness", config, "quickjs", "host");
        let mut store = BaselineStore::default();
        store.replace_case(BaselineEntry {
            key: old_key.clone(),
            sample: test_sample(),
        })?;
        store.replace_case(BaselineEntry {
            key: new_key.clone(),
            sample: test_sample(),
        })?;
        if store.lookup(&old_key).is_some() {
            return Err("refresh retained a stale entry for the same case".into());
        }
        if store.lookup(&new_key).is_none() {
            return Err("refresh removed the replacement baseline entry".into());
        }
        Ok(())
    }

    #[test]
    fn require_mode_rejects_a_missing_content_addressed_entry() -> TestResult {
        let config = test_config();
        let key = BaselineKey::new("missing", "source", "harness", config, "quickjs", "host");
        let baseline = QuickjsBaseline {
            mode: BaselineMode::Require,
            path: std::path::PathBuf::from("unused"),
            store: BaselineStore::default(),
            dirty: false,
        };
        if baseline.lookup(&key).is_err() {
            return Ok(());
        }
        Err("require mode accepted a missing QuickJS baseline entry".into())
    }

    fn test_config() -> MeasureConfig {
        MeasureConfig::new(Duration::from_millis(1), Duration::from_millis(3), 3)
    }

    fn test_sample() -> BaselineSample {
        BaselineSample {
            checksum: BenchmarkChecksum::number(42.0),
            snapshot: MeasureSnapshot {
                median: Duration::from_millis(2),
                cv_permille: 12,
                iters_per_sample: 3,
                samples: 5,
                median_sample: Duration::from_millis(6),
                warmup_elapsed: Duration::from_millis(1),
                timed_run_elapsed: Duration::from_millis(30),
                iteration_cap_reached: false,
            },
        }
    }
}
