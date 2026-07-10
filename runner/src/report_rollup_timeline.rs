use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context as _, bail};

use super::ReportRecord;

const TEST_REPORT_PREFIX: &str = "rsqjs-test-report-";
const TEST_REPORT_SUFFIX: &str = ".md";
const JETSTREAM_REPORT_PREFIX: &str = "rsqjs-jetstream-report-";
const JETSTREAM_REPORT_SUFFIX: &str = ".yaml";
const JETSTREAM_REPORT_DIRECTORY: &str = "jetstream-runs";
const MAIN_AXIS_DESCRIPTION: &str = "main first-parent commit";
const FALLBACK_AXIS_DESCRIPTION: &str = "shared report order (main history unavailable)";

#[derive(Debug)]
pub(super) struct CommitTimeline {
    report_positions: BTreeMap<String, i32>,
    labels: Vec<String>,
    description: &'static str,
}

impl CommitTimeline {
    pub(super) fn discover(report_dir: &Path, records: &[ReportRecord]) -> anyhow::Result<Self> {
        let Some(repository_root) = repository_root()? else {
            return Self::synthetic(records);
        };
        let reports_root = report_dir
            .parent()
            .context("report directory must have a reports parent")?;
        let Ok(reports_pathspec) = reports_root.strip_prefix(&repository_root) else {
            return Self::synthetic(records);
        };
        let Some(main_ref) = main_history_ref(&repository_root) else {
            return Self::synthetic(records);
        };
        let commits = main_commits(&repository_root, &main_ref)?;
        if commits.is_empty() {
            return Self::synthetic(records);
        }
        let shallow_commits = repository_shallow_commits(&repository_root)?;
        if history_has_shallow_boundary(&commits, &shallow_commits) {
            return Self::synthetic(records);
        }
        let additions = report_additions(&repository_root, &main_ref, reports_pathspec)?;
        Self::from_main_history(
            report_dir,
            reports_root,
            &repository_root,
            records,
            &commits,
            &additions,
        )
    }

    pub(super) fn position(&self, record: &ReportRecord) -> anyhow::Result<i32> {
        self.report_positions
            .get(&record.file_name)
            .copied()
            .with_context(|| {
                format!(
                    "report '{}' has no position on the shared chart timeline",
                    record.file_name
                )
            })
    }

    pub(super) fn axis_end(&self) -> anyhow::Result<i32> {
        let count = i32::try_from(self.labels.len()).context("too many commits to plot")?;
        Ok(count.max(1))
    }

    pub(super) fn label(&self, position: i32) -> String {
        let Ok(position) = usize::try_from(position) else {
            return String::new();
        };
        self.labels.get(position).cloned().unwrap_or_default()
    }

    pub(super) const fn description(&self) -> &'static str {
        self.description
    }

    fn from_main_history(
        report_dir: &Path,
        reports_root: &Path,
        repository_root: &Path,
        records: &[ReportRecord],
        commits: &[String],
        additions: &BTreeMap<String, String>,
    ) -> anyhow::Result<Self> {
        let mut commit_positions = BTreeMap::new();
        let mut labels = Vec::with_capacity(commits.len().saturating_add(1));
        for (position, commit) in commits.iter().enumerate() {
            let position = i32::try_from(position).context("too many main commits to plot")?;
            commit_positions.insert(commit.clone(), position);
            labels.push(short_commit(commit));
        }

        let mut report_positions = BTreeMap::new();
        let mut pending_reports = Vec::new();
        for record in records {
            if let Some(commit) = additions.get(&record.file_name) {
                let position = commit_positions.get(commit).copied().with_context(|| {
                    format!(
                        "report '{}' was added by commit '{}' outside the main first-parent history",
                        record.file_name, commit
                    )
                })?;
                report_positions.insert(record.file_name.clone(), position);
                continue;
            }
            let path = record_path(report_dir, reports_root, &record.file_name);
            if git_tracks(repository_root, &path)? {
                bail!(
                    "tracked report '{}' is absent from the complete main first-parent history",
                    record.file_name
                );
            }
            pending_reports.push(record.file_name.clone());
        }

        if !pending_reports.is_empty() {
            let pending_position =
                i32::try_from(labels.len()).context("too many main commits to plot")?;
            labels.push("pending".to_owned());
            for file_name in pending_reports {
                report_positions.insert(file_name, pending_position);
            }
        }

        Ok(Self {
            report_positions,
            labels,
            description: MAIN_AXIS_DESCRIPTION,
        })
    }

    fn synthetic(records: &[ReportRecord]) -> anyhow::Result<Self> {
        let mut report_positions = BTreeMap::new();
        let mut labels = Vec::with_capacity(records.len());
        for (position, record) in records.iter().enumerate() {
            let chart_position =
                i32::try_from(position).context("too many reports for fallback chart order")?;
            report_positions.insert(record.file_name.clone(), chart_position);
            labels.push(format!("r{}", position.saturating_add(1)));
        }
        Ok(Self {
            report_positions,
            labels,
            description: FALLBACK_AXIS_DESCRIPTION,
        })
    }

    #[cfg(test)]
    pub(super) fn for_test(commit_count: usize, positions: &[(&str, i32)]) -> Self {
        let labels = (0..commit_count)
            .map(|position| format!("c{position}"))
            .collect();
        let report_positions = positions
            .iter()
            .map(|(file_name, position)| ((*file_name).to_owned(), *position))
            .collect();
        Self {
            report_positions,
            labels,
            description: MAIN_AXIS_DESCRIPTION,
        }
    }
}

pub(super) fn repository_root() -> anyhow::Result<Option<PathBuf>> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("failed to inspect the git repository root")?;
    if !output.status.success() {
        return Ok(None);
    }
    let root = String::from_utf8(output.stdout).context("git repository root is not UTF-8")?;
    Ok(Some(PathBuf::from(root.trim())))
}

fn repository_shallow_commits(repository_root: &Path) -> anyhow::Result<BTreeSet<String>> {
    let output = git_output(
        repository_root,
        &["rev-parse", "--is-shallow-repository"],
        "inspect repository history depth",
    )?;
    if output.trim() != "true" {
        return Ok(BTreeSet::new());
    }
    let path = git_output(
        repository_root,
        &["rev-parse", "--git-path", "shallow"],
        "locate shallow repository boundaries",
    )?;
    let path = PathBuf::from(path.trim());
    let path = if path.is_absolute() {
        path
    } else {
        repository_root.join(path)
    };
    let contents = fs::read_to_string(&path).with_context(|| {
        format!(
            "failed to read shallow repository boundaries from '{}'",
            path.display()
        )
    })?;
    Ok(contents.lines().map(str::to_owned).collect())
}

fn history_has_shallow_boundary(commits: &[String], shallow_commits: &BTreeSet<String>) -> bool {
    commits
        .iter()
        .any(|commit| shallow_commits.contains(commit))
}

fn main_history_ref(repository_root: &Path) -> Option<String> {
    ["refs/remotes/origin/main", "refs/heads/main"]
        .into_iter()
        .find(|candidate| {
            Command::new("git")
                .arg("-C")
                .arg(repository_root)
                .args(["show-ref", "--verify", "--quiet", candidate])
                .status()
                .is_ok_and(|status| status.success())
        })
        .map(str::to_owned)
}

fn main_commits(repository_root: &Path, main_ref: &str) -> anyhow::Result<Vec<String>> {
    let output = git_output(
        repository_root,
        &["rev-list", "--reverse", "--first-parent", main_ref],
        "read main first-parent history",
    )?;
    Ok(output
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

fn report_additions(
    repository_root: &Path,
    main_ref: &str,
    reports_pathspec: &Path,
) -> anyhow::Result<BTreeMap<String, String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository_root)
        .args([
            "log",
            main_ref,
            "--reverse",
            "--first-parent",
            "--diff-filter=A",
            "--format=commit%x09%H",
            "--name-only",
            "--",
        ])
        .arg(reports_pathspec)
        .output()
        .context("failed to read report additions from main history")?;
    if !output.status.success() {
        bail!("git failed to read report additions from main history");
    }
    let text = String::from_utf8(output.stdout).context("git report history is not UTF-8")?;
    Ok(parse_report_additions(&text))
}

fn parse_report_additions(text: &str) -> BTreeMap<String, String> {
    let mut additions = BTreeMap::new();
    let mut current_commit = String::new();
    for line in text.lines() {
        if let Some(commit) = line.strip_prefix("commit\t") {
            commit.clone_into(&mut current_commit);
            continue;
        }
        let Some(file_name) = Path::new(line).file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if is_canonical_report_name(file_name) && !current_commit.is_empty() {
            additions.insert(file_name.to_owned(), current_commit.clone());
        }
    }
    additions
}

fn is_canonical_report_name(file_name: &str) -> bool {
    valid_timestamped_name(file_name, TEST_REPORT_PREFIX, TEST_REPORT_SUFFIX)
        || valid_timestamped_name(file_name, JETSTREAM_REPORT_PREFIX, JETSTREAM_REPORT_SUFFIX)
}

fn valid_timestamped_name(file_name: &str, prefix: &str, suffix: &str) -> bool {
    let Some(timestamp) = file_name
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(suffix))
    else {
        return false;
    };
    timestamp.len() == 16
        && timestamp
            .bytes()
            .enumerate()
            .all(|(position, byte)| matches!(position, 8 | 15) || byte.is_ascii_digit())
        && timestamp.as_bytes().get(8) == Some(&b'T')
        && timestamp.as_bytes().get(15) == Some(&b'Z')
}

fn record_path(report_dir: &Path, reports_root: &Path, file_name: &str) -> PathBuf {
    if file_name.starts_with(JETSTREAM_REPORT_PREFIX) {
        return reports_root
            .join(JETSTREAM_REPORT_DIRECTORY)
            .join(file_name);
    }
    report_dir.join(file_name)
}

fn git_tracks(repository_root: &Path, path: &Path) -> anyhow::Result<bool> {
    let relative = path.strip_prefix(repository_root).with_context(|| {
        format!(
            "report '{}' is outside repository '{}'",
            path.display(),
            repository_root.display()
        )
    })?;
    let output = Command::new("git")
        .arg("-C")
        .arg(repository_root)
        .args(["ls-files", "--error-unmatch", "--"])
        .arg(relative)
        .output()
        .context("failed to inspect whether a report is tracked")?;
    Ok(output.status.success())
}

fn git_output(repository_root: &Path, args: &[&str], action: &str) -> anyhow::Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository_root)
        .args(args)
        .output()
        .with_context(|| format!("failed to {action}"))?;
    if !output.status.success() {
        bail!("git failed to {action}");
    }
    String::from_utf8(output.stdout)
        .with_context(|| format!("git output for {action} is not UTF-8"))
}

fn short_commit(commit: &str) -> String {
    commit.chars().take(7).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{
        CommitTimeline, MAIN_AXIS_DESCRIPTION, history_has_shallow_boundary,
        parse_report_additions, repository_root,
    };
    use crate::report_rollup::{ReportContext, ReportRecord, parse_records};

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn parses_test_and_jetstream_additions_without_artifact_variants() -> TestResult {
        let first = "1111111111111111111111111111111111111111";
        let second = "2222222222222222222222222222222222222222";
        let text = format!(
            "commit\t{first}\n\nreports/test-runs/rsqjs-test-report-20260710T000000Z.md\nreports/test-runs/rsqjs-test-report-20260710T000000Z.yaml\n\ncommit\t{second}\n\nreports/jetstream-runs/rsqjs-jetstream-report-20260710T010000Z.yaml\nreports/jetstream-runs/rsqjs-jetstream-report-20260710T010000Z-component.yaml\n"
        );
        let additions = parse_report_additions(&text);
        ensure_commit(
            additions.get("rsqjs-test-report-20260710T000000Z.md"),
            first,
        )?;
        ensure_commit(
            additions.get("rsqjs-jetstream-report-20260710T010000Z.yaml"),
            second,
        )?;
        if additions.len() == 2 {
            return Ok(());
        }
        Err("report addition parser accepted a derived artifact variant".into())
    }

    #[test]
    fn shallow_boundary_only_truncates_the_history_that_contains_it() -> TestResult {
        let main_commit = "1111111111111111111111111111111111111111".to_owned();
        let unrelated_commit = "2222222222222222222222222222222222222222".to_owned();
        let commits = vec![main_commit.clone()];
        let unrelated = BTreeSet::from([unrelated_commit]);
        let matching = BTreeSet::from([main_commit]);
        if !history_has_shallow_boundary(&commits, &unrelated)
            && history_has_shallow_boundary(&commits, &matching)
        {
            return Ok(());
        }
        Err("an unrelated shallow boundary compressed the main timeline".into())
    }

    #[test]
    fn every_tracked_report_maps_to_the_current_main_commit_domain() -> TestResult {
        let repository_root = repository_root()?.ok_or("test is outside a git repository")?;
        let report_dir = repository_root.join("reports/test-runs");
        let records = parse_records(&report_dir)?;
        let timeline = CommitTimeline::discover(&report_dir, &records)?;
        for record in &records {
            let position = timeline.position(record)?;
            if position < 0 || position >= timeline.axis_end()? {
                return Err(format!(
                    "report '{}' mapped outside the main commit domain",
                    record.file_name
                )
                .into());
            }
        }
        if timeline.description() == MAIN_AXIS_DESCRIPTION
            && timeline.axis_end()? > i32::try_from(records.len())?
        {
            return Ok(());
        }
        Err("tracked reports did not use the expanded main commit domain".into())
    }

    #[test]
    fn untracked_publisher_report_uses_one_pending_commit_slot() -> TestResult {
        let repository_root = repository_root()?.ok_or("test is outside a git repository")?;
        let reports_root = repository_root.join("target/rollup-pending");
        let report_dir = reports_root.join("test-runs");
        let records = vec![empty_record("rsqjs-test-report-20260710T020000Z.md")];
        let timeline = CommitTimeline::from_main_history(
            &report_dir,
            &reports_root,
            &repository_root,
            &records,
            &["1111111111111111111111111111111111111111".to_owned()],
            &BTreeMap::new(),
        )?;
        if timeline.axis_end()? == 2
            && timeline.position(
                records
                    .first()
                    .ok_or("pending timeline fixture has no record")?,
            )? == 1
            && timeline.label(1) == "pending"
        {
            return Ok(());
        }
        Err("untracked publisher report did not use the next main commit slot".into())
    }

    fn ensure_commit(actual: Option<&String>, expected: &str) -> TestResult {
        if actual.is_some_and(|value| value == expected) {
            return Ok(());
        }
        Err(format!("expected report addition commit {expected}, got {actual:?}").into())
    }

    fn empty_record(file_name: &str) -> ReportRecord {
        ReportRecord {
            file_name: file_name.to_owned(),
            timestamp: String::new(),
            benchmark_count: 0,
            latency_geomean: None,
            memory_geomean: None,
            jetstream_count: 0,
            jetstream_latency_geomean: None,
            latency_over: 0,
            memory_over: 0,
            jetstream_latency_over: 0,
            benchmark_report: false,
            jetstream_report: false,
            full_test262: None,
            context: ReportContext::default(),
        }
    }
}
