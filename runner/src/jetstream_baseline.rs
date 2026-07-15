//! Content-addressed `QuickJS` reference measurements for `JetStream` shell cases.

use std::{
    collections::BTreeMap,
    env, fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context as _, bail};

use crate::{
    bench_measure::{MeasureConfig, MeasureSnapshot, MeasureStats},
    quickjs_baseline::{normalize_field, stable_digest},
};

pub const ENV_BASELINE_MODE: &str = "VELUM_JETSTREAM_QUICKJS_BASELINE";
pub const ENV_BASELINE_PATH: &str = "VELUM_JETSTREAM_QUICKJS_BASELINE_PATH";
pub const PROTOCOL_VERSION: &str = "jetstream-shell-v1";

const DEFAULT_BASELINE_PATH: &str = "tests/corpora/jetstream/quickjs-baseline.tsv";
const MODE_OFF: &str = "off";
const MODE_READ: &str = "read";
const MODE_REFRESH: &str = "refresh";
const SCHEMA_VERSION: &str = "1";
const HEADER: &str = "schema_version\tcontent_id\tcase_id\tsource_digest\tharness_digest\tprotocol\tmeasure_config\treference_engine\thost_profile\toutcome\tdetail\tmedian_ns\tcv_permille\titers_per_sample\tsamples\tmedian_sample_ns\twarmup_ns\ttimed_run_ns\titeration_cap";
const OUTCOME_MEASURED: &str = "measured";
const OUTCOME_UNAVAILABLE: &str = "unavailable";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum BaselineMode {
    Off,
    Read,
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
            protocol: PROTOCOL_VERSION.to_owned(),
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
    pub snapshot: MeasureSnapshot,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BaselineOutcome {
    Measured(BaselineSample),
    Unavailable(String),
}

impl BaselineSample {
    pub const fn from_measurement(stats: MeasureStats) -> Self {
        Self {
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
    outcome: BaselineOutcome,
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
            Err(error) => Err(error).with_context(|| {
                format!(
                    "failed to read JetStream QuickJS baseline '{}'",
                    path.display()
                )
            }),
        }
    }

    fn parse(text: &str) -> anyhow::Result<Self> {
        let mut lines = text.lines();
        let Some(header) = lines.next() else {
            bail!("JetStream QuickJS baseline is empty")
        };
        if header != HEADER {
            bail!("JetStream QuickJS baseline has an unsupported header")
        }
        let mut store = Self::default();
        for (offset, line) in lines.enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let line_number = offset.saturating_add(2);
            store.insert(BaselineEntry::parse(line, line_number)?)?;
        }
        Ok(store)
    }

    fn lookup(&self, key: &BaselineKey) -> Option<BaselineOutcome> {
        self.entries
            .get(&key.content_id())
            .filter(|entry| entry.key == *key)
            .map(|entry| entry.outcome.clone())
    }

    fn insert(&mut self, entry: BaselineEntry) -> anyhow::Result<()> {
        let content_id = entry.key.content_id();
        if let Some(existing) = self.entries.get(&content_id)
            && existing.key != entry.key
        {
            bail!("JetStream QuickJS baseline content id collision for '{content_id}'")
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
            bail!("JetStream QuickJS baseline line {line_number} uses schema '{schema}'")
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
            bail!("JetStream QuickJS baseline line {line_number} has a stale content id")
        }
        let outcome = next_field(&mut fields, line_number, "outcome")?;
        let detail = next_field(&mut fields, line_number, "detail")?;
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
            value => bail!(
                "JetStream QuickJS baseline line {line_number} has invalid iteration_cap '{value}'"
            ),
        };
        if let Some(extra) = fields.next() {
            bail!("JetStream QuickJS baseline line {line_number} has an extra field '{extra}'")
        }
        let sample = BaselineSample {
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
        };
        let outcome = match outcome {
            OUTCOME_MEASURED => BaselineOutcome::Measured(sample),
            OUTCOME_UNAVAILABLE => BaselineOutcome::Unavailable(detail.to_owned()),
            value => {
                bail!("JetStream QuickJS baseline line {line_number} has invalid outcome '{value}'")
            }
        };
        Ok(Self { key, outcome })
    }

    fn render(&self, content_id: &str) -> String {
        let (outcome, detail, snapshot) = match &self.outcome {
            BaselineOutcome::Measured(sample) => (OUTCOME_MEASURED, String::new(), sample.snapshot),
            BaselineOutcome::Unavailable(detail) => (
                OUTCOME_UNAVAILABLE,
                normalize_field(detail),
                MeasureSnapshot {
                    median: Duration::ZERO,
                    cv_permille: 0,
                    iters_per_sample: 0,
                    samples: 0,
                    median_sample: Duration::ZERO,
                    warmup_elapsed: Duration::ZERO,
                    timed_run_elapsed: Duration::ZERO,
                    iteration_cap_reached: false,
                },
            ),
        };
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
            outcome.to_owned(),
            detail,
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
pub struct JetStreamQuickjsBaseline {
    mode: BaselineMode,
    path: PathBuf,
    store: BaselineStore,
    dirty: bool,
}

impl JetStreamQuickjsBaseline {
    pub fn from_env() -> anyhow::Result<Self> {
        let mode = match env::var(ENV_BASELINE_MODE)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .as_deref()
        {
            None | Some(MODE_READ) => BaselineMode::Read,
            Some(MODE_REFRESH) => BaselineMode::Refresh,
            Some(MODE_OFF) => BaselineMode::Off,
            Some(value) => bail!(
                "unknown JetStream QuickJS baseline mode '{value}'; expected '{MODE_READ}', '{MODE_REFRESH}', or '{MODE_OFF}'"
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

    pub fn requires_live_reference(&self) -> bool {
        self.mode == BaselineMode::Refresh
    }

    pub fn lookup(&self, key: &BaselineKey) -> Option<BaselineOutcome> {
        if self.mode != BaselineMode::Read {
            return None;
        }
        self.store.lookup(key)
    }

    pub fn contains(&self, key: &BaselineKey) -> bool {
        self.mode == BaselineMode::Read && self.store.lookup(key).is_some()
    }

    pub fn is_read(&self) -> bool {
        self.mode == BaselineMode::Read
    }

    pub fn is_disabled(&self) -> bool {
        self.mode == BaselineMode::Off
    }

    pub fn record_measured(
        &mut self,
        key: BaselineKey,
        sample: BaselineSample,
    ) -> anyhow::Result<()> {
        if self.mode != BaselineMode::Refresh {
            return Ok(());
        }
        self.store.replace_case(BaselineEntry {
            key,
            outcome: BaselineOutcome::Measured(sample),
        })?;
        self.dirty = true;
        Ok(())
    }

    pub fn record_unavailable(&mut self, key: BaselineKey, detail: &str) -> anyhow::Result<()> {
        if self.mode != BaselineMode::Refresh {
            return Ok(());
        }
        self.store.replace_case(BaselineEntry {
            key,
            outcome: BaselineOutcome::Unavailable(normalize_field(detail)),
        })?;
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
                    "failed to create JetStream QuickJS baseline directory '{}'",
                    parent.display()
                )
            })?;
        }
        fs::write(&self.path, self.store.render()).with_context(|| {
            format!(
                "failed to write JetStream QuickJS baseline '{}'",
                self.path.display()
            )
        })?;
        self.dirty = false;
        Ok(())
    }

    #[cfg(test)]
    pub fn empty_read_for_test() -> Self {
        Self {
            mode: BaselineMode::Read,
            path: PathBuf::new(),
            store: BaselineStore::default(),
            dirty: false,
        }
    }
}

fn next_field<'a>(
    fields: &mut impl Iterator<Item = &'a str>,
    line_number: usize,
    name: &str,
) -> anyhow::Result<&'a str> {
    fields.next().with_context(|| {
        format!("JetStream QuickJS baseline line {line_number} is missing '{name}'")
    })
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
    value.parse::<T>().with_context(|| {
        format!("JetStream QuickJS baseline line {line_number} has invalid '{name}'")
    })
}

fn parse_duration<'a>(
    fields: &mut impl Iterator<Item = &'a str>,
    line_number: usize,
    name: &str,
) -> anyhow::Result<Duration> {
    parse_field(fields, line_number, name).map(Duration::from_nanos)
}

#[cfg(test)]
mod tests {
    use super::{BaselineEntry, BaselineKey, BaselineOutcome, BaselineSample, BaselineStore};
    use crate::bench_measure::{MeasureConfig, MeasureSnapshot};
    use std::time::Duration;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn baseline_round_trip_preserves_key_and_sample() -> TestResult {
        let key = test_key("source", "harness");
        let sample = test_sample();
        let mut store = BaselineStore::default();
        store.insert(BaselineEntry {
            key: key.clone(),
            outcome: BaselineOutcome::Measured(sample.clone()),
        })?;
        let decoded = BaselineStore::parse(&store.render())?;
        let Some(actual) = decoded.lookup(&key) else {
            return Err("round-tripped baseline did not contain its key".into());
        };
        if actual != BaselineOutcome::Measured(sample) {
            return Err("round-tripped baseline changed its sample".into());
        }
        Ok(())
    }

    #[test]
    fn unavailable_outcome_round_trips_without_live_fallback_data() -> TestResult {
        let key = test_key("source", "harness");
        let mut store = BaselineStore::default();
        store.insert(BaselineEntry {
            key: key.clone(),
            outcome: BaselineOutcome::Unavailable("unsupported\tfeature".to_owned()),
        })?;
        let decoded = BaselineStore::parse(&store.render())?;
        let Some(actual) = decoded.lookup(&key) else {
            return Err("round-tripped baseline did not contain unavailable outcome".into());
        };
        if actual != BaselineOutcome::Unavailable("unsupported feature".to_owned()) {
            return Err("round-tripped baseline changed unavailable detail".into());
        }
        Ok(())
    }

    #[test]
    fn source_harness_config_engine_and_host_are_content_addressed() -> TestResult {
        let base = test_key("source", "harness");
        let alternatives = [
            test_key("source-2", "harness"),
            test_key("source", "harness-2"),
            BaselineKey::new("case", "source", "harness", test_config(), "qjs-2", "host"),
            BaselineKey::new("case", "source", "harness", test_config(), "qjs", "host-2"),
        ];
        for alternative in alternatives {
            if alternative.content_id() == base.content_id() {
                return Err("baseline key dimension did not change the content id".into());
            }
        }
        Ok(())
    }

    fn test_key(source: &str, harness: &str) -> BaselineKey {
        BaselineKey::new("case", source, harness, test_config(), "qjs", "host")
    }

    fn test_config() -> MeasureConfig {
        MeasureConfig::new(Duration::from_millis(1), Duration::from_millis(3), 3)
    }

    fn test_sample() -> BaselineSample {
        BaselineSample {
            snapshot: MeasureSnapshot {
                median: Duration::from_millis(2),
                cv_permille: 3,
                iters_per_sample: 4,
                samples: 5,
                median_sample: Duration::from_millis(8),
                warmup_elapsed: Duration::from_millis(1),
                timed_run_elapsed: Duration::from_millis(40),
                iteration_cap_reached: false,
            },
        }
    }
}
