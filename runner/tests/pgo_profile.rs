//! Exercise profile validation CLI paths with synthetic files, never LLVM tools.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, bail, ensure};

const ENGINE: &str = "_ZN5velum7runtime7execute17h1234567890abcdefE";
const OTHER_ENGINE: &str = "_ZN5velum6parser5parse17h1234567890abcdefE";
const RUNNER: &str = "_ZN17velum_test_runner4main17h1234567890abcdefE";
const MODULE: &str = "velum_test_runner.243cb84678a5d0f8-cgu.0";
const NO_ENGINE: &str = "no observed executed Velum engine function";
const TIMEOUT: Duration = Duration::from_secs(5);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Result<Self> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let root =
            std::env::temp_dir().join(format!("velum pgo profile-{}-{stamp}", std::process::id()));
        fs::create_dir(&root)?;
        Ok(Self { root })
    }

    fn file(&self, name: &str, text: &str) -> Result<PathBuf> {
        let path = self.root.join(name);
        fs::write(&path, text)?;
        Ok(path)
    }

    fn profile(&self, text: &str) -> Result<Output> {
        let input = self.file("show.txt", text)?;
        let symbols = self.root.join("symbols.txt");
        run("--pgo-profile-check", &[&input, &symbols])
    }

    fn diagnostics(&self, text: &str, symbols: &str) -> Result<Output> {
        let input = self.file("stderr.txt", text)?;
        let symbols = self.file("symbols.txt", symbols)?;
        let report = self.root.join("diagnostics.txt");
        run("--pgo-diagnostics-check", &[&input, &symbols, &report])
    }

    fn read(&self, name: &str) -> Result<String> {
        fs::read_to_string(self.root.join(name)).with_context(|| format!("missing fixture {name}"))
    }
}

fn with_fixture(test: impl FnOnce(&Fixture) -> Result<()>) -> Result<()> {
    let fixture = Fixture::new()?;
    let result = test(&fixture);
    let cleanup = fs::remove_dir_all(&fixture.root);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error).context("fixture cleanup failed"),
        (Err(error), Err(cleanup)) => bail!("{error:#}; fixture cleanup failed: {cleanup}"),
    }
}

fn run(flag: &str, paths: &[&Path]) -> Result<Output> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_velum-test-runner"))
        .arg(flag)
        .args(paths)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child
                .wait_with_output()
                .context("failed to collect validator output");
        }
        if started.elapsed() >= TIMEOUT {
            child.kill().context("failed to terminate validator")?;
            child.wait().context("failed to reap validator")?;
            bail!("profile validator exceeded its test deadline");
        }
        thread::sleep(Duration::from_millis(2));
    }
}

fn success(output: &Output) -> Result<()> {
    ensure!(
        output.status.success(),
        "validator failed: {}; stdout={}; stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn rejected(output: &Output, reason: &str) -> Result<()> {
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        !output.status.success(),
        "validator accepted invalid fixture: {reason}"
    );
    ensure!(
        stderr.contains(reason),
        "missing diagnostic '{reason}': {stderr}"
    );
    Ok(())
}

fn record(symbol: &str, counts: &str) -> String {
    format!(
        "  {symbol}:\n    Hash: 0x0123456789abcdef\n    Counters: 3\n    Block counts: [{counts}]\n"
    )
}

fn profile(records: &str) -> String {
    format!(
        "Counters:\n{records}Instrumentation level: IR  entry_first = 0  instrument_loop_entries = 0\nFunctions shown: 1\nTotal functions: 1\nMaximum function count: 0\nMaximum internal block count: 7\nTotal number of blocks: 3\nTotal count: 7\nProfile version: 13\n"
    )
}

fn valid_profile() -> String {
    profile(&record(ENGINE, "0, 7, 0"))
}

fn missing(symbol: &str) -> String {
    format!(
        "warning: {MODULE}: no profile data available for function {symbol} Hash = 123 up to 0 count discarded\n"
    )
}

#[test]
fn profile_accepts_positive_internal_count_with_zero_function_maximum() -> Result<()> {
    with_fixture(|fixture| {
        success(&fixture.profile(&valid_profile())?)?;
        ensure!(fixture.read("symbols.txt")? == format!("{ENGINE}\n"));
        Ok(())
    })
}

#[test]
fn profile_accepts_bare_ir_header_and_function_count() -> Result<()> {
    with_fixture(|fixture| {
        let text = valid_profile()
            .replace("IR  entry_first = 0  instrument_loop_entries = 0", "IR")
            .replace("Block counts: [0, 7, 0]", "Function count: 7");
        success(&fixture.profile(&text)?)
    })
}

#[test]
fn profile_normalizes_cgu_prefixes_and_sorts_distinct_engine_symbols() -> Result<()> {
    with_fixture(|fixture| {
        let records = [ENGINE, RUNNER, OTHER_ENGINE, ENGINE]
            .iter()
            .map(|symbol| record(&format!("{MODULE};{symbol}"), "1"))
            .collect::<String>();
        success(&fixture.profile(&profile(&records))?)?;
        let mut expected = [ENGINE, OTHER_ENGINE];
        expected.sort_unstable();
        ensure!(fixture.read("symbols.txt")? == format!("{}\n", expected.join("\n")));
        Ok(())
    })
}

#[test]
fn profile_rejects_missing_frontend_duplicate_or_unknown_ir_headers() -> Result<()> {
    for text in [
        String::new(),
        valid_profile().replace("level: IR", "level: Front-end"),
        valid_profile().replace("level: IR", "level: IRISH"),
        format!("{}Instrumentation level: IR\n", valid_profile()),
    ] {
        with_fixture(|fixture| {
            rejected(&fixture.profile(&text)?, "IR instrumentation profile")?;
            ensure!(!fixture.root.join("symbols.txt").exists());
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn profile_rejects_missing_zero_duplicate_or_malformed_summary_counts() -> Result<()> {
    for (field, value) in [
        ("Total functions", "1"),
        ("Total number of blocks", "3"),
        ("Total count", "7"),
    ] {
        let original = format!("{field}: {value}\n");
        for (replacement, reason) in [
            (String::new(), field),
            (format!("{field}: 0\n"), field),
            (format!("{original}{original}"), field),
            (format!("{field}: invalid\n"), "invalid LLVM profile count"),
            (format!("{field}: -1\n"), "invalid LLVM profile count"),
            (format!("{field}: 18446744073709551616\n"), "exceeds u64"),
        ] {
            with_fixture(|fixture| {
                rejected(
                    &fixture.profile(&valid_profile().replace(&original, &replacement))?,
                    reason,
                )
            })?;
        }
    }
    Ok(())
}

#[test]
fn profile_requires_executed_engine_records_not_runner_or_standard_library() -> Result<()> {
    for records in [
        record(RUNNER, "7"),
        record("_ZN3std2io5writeE", "7"),
        record(ENGINE, "0, 0, 0"),
        format!("{}{}", record(ENGINE, "0"), record(RUNNER, "7")),
        format!("  {ENGINE}:\n    Hash: 0x123\n    Counters: 3\n"),
    ] {
        with_fixture(|fixture| rejected(&fixture.profile(&profile(&records))?, NO_ENGINE))?;
    }
    Ok(())
}

#[test]
fn profile_rejects_foreign_roots_and_engine_mentions_inside_generics() -> Result<()> {
    for symbol in [
        "_ZN3std6thread17spawn_unchecked$LT$velum..Value$GT$E",
        "_ZN4core3ptr37drop_in_place$LT$velum..Value$GT$E",
        "velum.243cb84678a5d0f8-cgu.0;_ZN3std2rt10lang_startE",
        "std.243cb84678a5d0f8-cgu.0;_ZN4core3ptr37drop_in_place$LT$velum..Value$GT$E",
        "_ZN16velum_tool_probe8workload17h2643f89c5cffb271E",
        "_RNvC3std7executeINtC5velum5ValueE",
        "_RNvC4core7executeINtCs123_5velum5ValueE",
        "_RNvC17velum_test_runner4main",
        "_RNvC16velum_tool_probe8workload",
        "_RNvCsbad!_5velum7execute",
        "_RNvC5velumother",
    ] {
        with_fixture(|fixture| {
            rejected(&fixture.profile(&profile(&record(symbol, "7")))?, NO_ENGINE)
        })?;
    }
    Ok(())
}

#[test]
fn profile_accepts_v0_engine_roots_with_optional_disambiguator_and_cgu() -> Result<()> {
    for symbol in ["_RNvC5velum7execute", "_RNvCs123abc_5velum7execute"] {
        with_fixture(|fixture| {
            let records = record(&format!("{MODULE};{symbol}"), "7");
            success(&fixture.profile(&profile(&records))?)?;
            ensure!(fixture.read("symbols.txt")? == format!("{symbol}\n"));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn profile_rejects_malformed_engine_counter_values() -> Result<()> {
    for counts in ["7, invalid", "-1", "7,", "18446744073709551616"] {
        with_fixture(|fixture| {
            rejected(
                &fixture.profile(&profile(&record(ENGINE, counts)))?,
                "profile count",
            )
        })?;
    }
    with_fixture(|fixture| {
        let text = valid_profile().replace("Block counts: [0, 7, 0]", "Block counts: 7");
        rejected(&fixture.profile(&text)?, "expected brackets")
    })
}

#[test]
fn diagnostics_accept_clean_build_and_preserve_zero_missing_count() -> Result<()> {
    with_fixture(|fixture| {
        success(&fixture.diagnostics("", ENGINE)?)?;
        let report = fixture.read("diagnostics.txt")?;
        ensure!(report.contains("human review: 0\n"));
        ensure!(report.contains("No known LLVM 22 profile mismatch"));
        Ok(())
    })
}

#[test]
fn diagnostics_report_untrained_missing_functions_and_count_discarded() -> Result<()> {
    with_fixture(|fixture| {
        let warning = missing(OTHER_ENGINE);
        success(&fixture.diagnostics(&warning, ENGINE)?)?;
        let report = fixture.read("diagnostics.txt")?;
        ensure!(report.contains("human review: 1\n") && report.contains(&warning));
        Ok(())
    })
}

#[test]
fn diagnostics_reject_missing_trained_symbol_with_separate_module_prefix() -> Result<()> {
    with_fixture(|fixture| {
        let warning = missing(ENGINE);
        rejected(
            &fixture.diagnostics(&warning, ENGINE)?,
            "previously executed engine symbol",
        )?;
        let report = fixture.read("diagnostics.txt")?;
        ensure!(report.contains(&warning) && report.contains("human review: 1\n"));
        Ok(())
    })
}

#[test]
fn diagnostics_match_normalized_symbols_from_profile_output() -> Result<()> {
    with_fixture(|fixture| {
        let records = record(&format!("{MODULE};{ENGINE}"), "7");
        success(&fixture.profile(&profile(&records))?)?;
        let trained = fixture.read("symbols.txt")?;
        rejected(
            &fixture.diagnostics(&missing(ENGINE), &trained)?,
            "previously executed engine symbol",
        )
    })
}

#[test]
fn diagnostics_reject_known_mismatches_and_invalid_profiles() -> Result<()> {
    for warning in [
        format!(
            "warning: {MODULE}: function control flow change detected (hash mismatch) {ENGINE} Hash = 1096621587845996077 up to 16385 count discarded\nwarning: 1 warning emitted\n"
        ),
        "warning: counter mismatch\n".to_owned(),
        "warning: bitmap size mismatch\n".to_owned(),
        "warning: inconsistent number of counts\n".to_owned(),
        "warning: profile is out of date\n".to_owned(),
        "warning: malformed profile data\n".to_owned(),
        "error: cannot read profile data\n".to_owned(),
        "error: unable to load profile\n".to_owned(),
        "error: failed to load profile\n".to_owned(),
    ] {
        with_fixture(|fixture| {
            rejected(
                &fixture.diagnostics(&warning, ENGINE)?,
                "Profile mismatch or invalid profile",
            )?;
            ensure!(
                fixture
                    .read("diagnostics.txt")?
                    .contains(warning.lines().next().context("empty warning")?)
            );
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn diagnostics_reject_unknown_profile_and_pgo_warnings() -> Result<()> {
    for warning in [
        "warning: profile reader changed behavior",
        "warning: PGO fallback selected",
    ] {
        with_fixture(|fixture| {
            rejected(
                &fixture.diagnostics(warning, ENGINE)?,
                "Unclassified PGO warning",
            )
        })?;
    }
    Ok(())
}

#[test]
fn diagnostics_ignore_verbose_commands_but_not_real_warnings() -> Result<()> {
    let command = "     Running `rustc -Cprofile-use=/tmp/profile-mismatch/merged.profdata -Cllvm-args=-no-pgo-warn-mismatch=false`\n";
    with_fixture(|fixture| {
        success(&fixture.diagnostics(command, ENGINE)?)?;
        rejected(
            &fixture.diagnostics(&format!("{command}warning: hash mismatch\n"), ENGINE)?,
            "hash mismatch",
        )
    })
}

#[test]
fn diagnostics_allow_unrelated_warnings_but_missing_records_do_not_mask_mismatch() -> Result<()> {
    with_fixture(|fixture| {
        success(&fixture.diagnostics(
            "warning: unused variable\nwarning: 1 warning emitted\n",
            ENGINE,
        )?)?;
        let warnings = format!("{}warning: counter mismatch\n", missing(OTHER_ENGINE));
        rejected(&fixture.diagnostics(&warnings, ENGINE)?, "counter mismatch")
    })
}

#[test]
fn diagnostics_reject_empty_or_invalid_trained_symbol_manifests() -> Result<()> {
    for (symbols, reason) in [
        ("", "trained engine symbols are empty"),
        (RUNNER, "invalid normalized engine symbol"),
        (
            "_ZN5velum7execute;foreign",
            "invalid normalized engine symbol",
        ),
    ] {
        with_fixture(|fixture| rejected(&fixture.diagnostics("", symbols)?, reason))?;
    }
    Ok(())
}

#[test]
fn diagnostics_replay_actual_probe_grammar_without_claiming_probe_engine_coverage() -> Result<()> {
    const PROBE: &str = "_ZN16velum_tool_probe8workload17h2643f89c5cffb271E";
    with_fixture(|fixture| {
        rejected(
            &fixture.profile(&profile(&record(PROBE, "13107, 3277, 1")))?,
            NO_ENGINE,
        )?;
        let warning = format!(
            "warning: velum_tool_probe.243cb84678a5d0f8-cgu.0: function control flow change detected (hash mismatch) {PROBE} Hash = 1096621587845996077 up to 16385 count discarded\n\nwarning: 1 warning emitted\n"
        );
        rejected(&fixture.diagnostics(&warning, ENGINE)?, "hash mismatch")
    })
}
