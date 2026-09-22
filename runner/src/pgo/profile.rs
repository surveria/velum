//! Conservative readers for the LLVM 22 instrumentation diagnostics used by PGO.

use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{BufRead as _, BufReader, BufWriter, Write as _},
    path::Path,
};

use anyhow::{Context as _, Result, ensure};

const TOTAL_FIELDS: [&str; 3] = ["Total functions", "Total number of blocks", "Total count"];
const MISSING_FUNCTION: &str = "no profile data available for function";
const NO_ENGINE: &str = "profile contains no observed executed Velum engine function";
const MISMATCH: &str = "Profile mismatch or invalid profile diagnostics";

/// Checks an LLVM IR profile dump and preserves normalized executed engine names.
pub fn verify_profile(show: &Path, symbols_out: &Path) -> Result<()> {
    let file = File::open(show)
        .with_context(|| format!("failed to open profile dump '{}'", show.display()))?;
    let mut ir_headers = 0_usize;
    let mut totals = BTreeSet::new();
    let mut hot_engine = BTreeSet::new();
    let mut current_engine: Option<String> = None;
    for line in BufReader::new(file).lines() {
        let line = line.context("failed to read profile dump")?;
        if let Some(body) = line.strip_prefix("    ") {
            if let Some(symbol) = &current_engine
                && executed_count(body)?
            {
                hot_engine.insert(symbol.clone());
            }
            continue;
        }
        current_engine = profile_function(&line)
            .filter(|symbol| engine_owned(symbol))
            .map(str::to_owned);
        if let Some(level) = line.strip_prefix("Instrumentation level:") {
            ensure!(
                level.split_whitespace().next() == Some("IR"),
                "expected LLVM IR instrumentation profile"
            );
            ir_headers = ir_headers
                .checked_add(1)
                .context("IR header count overflowed")?;
        }
        for field in TOTAL_FIELDS {
            let Some(value) = line
                .strip_prefix(field)
                .and_then(|tail| tail.strip_prefix(':'))
            else {
                continue;
            };
            ensure!(totals.insert(field), "duplicate {field}");
            ensure!(parse_count(value)? > 0, "missing or zero {field}");
        }
    }
    ensure!(
        ir_headers == 1,
        "expected one LLVM IR instrumentation profile header"
    );
    for field in TOTAL_FIELDS {
        ensure!(totals.contains(field), "missing or zero {field}");
    }
    ensure!(!hot_engine.is_empty(), "{NO_ENGINE}");
    let file = File::create(symbols_out).with_context(|| {
        format!(
            "failed to create engine symbols '{}'",
            symbols_out.display()
        )
    })?;
    let mut output = BufWriter::new(file);
    for symbol in hot_engine {
        writeln!(output, "{symbol}").context("failed to write executed engine symbol")?;
    }
    output.flush().context("failed to flush engine symbols")?;
    println!("Validated nonempty IR profile with observed executed engine functions.");
    Ok(())
}

fn profile_function(line: &str) -> Option<&str> {
    let record = line.strip_prefix("  ")?;
    if record.starts_with(char::is_whitespace) {
        return None;
    }
    record.strip_suffix(':')?.rsplit(';').next()
}

fn parse_count(text: &str) -> Result<u64> {
    let text = text.trim();
    ensure!(
        !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()),
        "invalid LLVM profile count '{text}'"
    );
    text.parse().context("LLVM profile count exceeds u64")
}

fn executed_count(body: &str) -> Result<bool> {
    if let Some(value) = body.strip_prefix("Function count:") {
        return Ok(parse_count(value)? > 0);
    }
    let Some(counts) = body.strip_prefix("Block counts:") else {
        return Ok(false);
    };
    let counts = counts
        .trim()
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .context("invalid LLVM block counts: expected brackets")?;
    if counts.trim().is_empty() {
        return Ok(false);
    }
    let mut executed = false;
    for count in counts.split(',') {
        executed |= parse_count(count)? > 0;
    }
    Ok(executed)
}

fn engine_owned(symbol: &str) -> bool {
    if let Some(tail) = symbol.strip_prefix("_ZN5velum") {
        return tail
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_digit());
    }
    // Inspect the first v0 crate root, never an engine type in another crate's
    // generic arguments. Unknown mangling forms deliberately fail closed.
    let Some((_, mut root)) = symbol
        .strip_prefix("_R")
        .and_then(|name| name.split_once('C'))
    else {
        return false;
    };
    if let Some(disambiguated) = root.strip_prefix('s') {
        let Some((disambiguator, tail)) = disambiguated.split_once('_') else {
            return false;
        };
        if !disambiguator
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric())
        {
            return false;
        }
        root = tail;
    }
    root.strip_prefix("5velum").is_some_and(|tail| {
        tail.bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_digit() || byte.is_ascii_uppercase() || byte == b'_')
    })
}

/// Reports missing records and rejects mismatches, missing hot symbols or unknown warnings.
pub fn verify_diagnostics(stderr: &Path, symbols: &Path, report_out: &Path) -> Result<()> {
    let trained = read_trained_symbols(symbols)?;
    let file = File::open(stderr)
        .with_context(|| format!("failed to open PGO diagnostics '{}'", stderr.display()))?;
    let mut missing = Vec::new();
    let mut errors = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line.context("failed to read PGO diagnostics")?;
        if verbose_command(&line) {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if mismatch_diagnostic(&lower) {
            errors.push(line.clone());
        }
        if let Some((_, tail)) = lower.split_once(MISSING_FUNCTION) {
            // Use the original line's symbol spelling; the module before the
            // diagnostic is not part of the function's LLVM profile identity.
            let symbol = line
                .len()
                .checked_sub(tail.len())
                .and_then(|offset| line.get(offset..))
                .and_then(|original| original.split_whitespace().next())
                .and_then(|name| name.rsplit(';').next());
            match symbol {
                Some(symbol) if trained.contains(symbol) => errors.push(format!(
                    "A previously executed engine symbol is now missing from profile-use compilation: {symbol}"
                )),
                Some(_) => {}
                None => errors.push("Malformed missing-function diagnostic".to_owned()),
            }
            missing.push(line);
        } else if lower.contains("warning:") && profile_warning(&lower) {
            errors.push(format!("Unclassified PGO warning: {line}"));
        }
    }
    let mut report = format!(
        "Missing-function diagnostic lines requiring human review: {}\n",
        missing.len()
    );
    for line in missing {
        report.push_str(&line);
        report.push('\n');
    }
    if errors.is_empty() {
        report.push_str("No known LLVM 22 profile mismatch diagnostic was found.\n");
    } else {
        report.push_str(MISMATCH);
        report.push_str(":\n");
        for line in &errors {
            report.push_str(line);
            report.push('\n');
        }
    }
    fs::write(report_out, &report).with_context(|| {
        format!(
            "failed to preserve PGO diagnostics '{}'",
            report_out.display()
        )
    })?;
    print!("{report}");
    ensure!(errors.is_empty(), "{MISMATCH}: {}", errors.join("\n"));
    Ok(())
}

fn read_trained_symbols(path: &Path) -> Result<BTreeSet<String>> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read trained engine symbols '{}'", path.display()))?;
    let mut symbols = BTreeSet::new();
    for line in text.lines() {
        ensure!(
            engine_owned(line) && !line.contains(char::is_whitespace) && !line.contains(';'),
            "invalid normalized engine symbol '{line}'"
        );
        symbols.insert(line.to_owned());
    }
    ensure!(!symbols.is_empty(), "trained engine symbols are empty");
    Ok(symbols)
}

fn verbose_command(line: &str) -> bool {
    line.trim_start()
        .strip_prefix("Running")
        .is_some_and(|tail| tail.starts_with(char::is_whitespace))
}

fn mismatch_diagnostic(lower: &str) -> bool {
    const DIRECT: [&str; 5] = [
        "function control flow change detected",
        "hash mismatch",
        "counter mismatch",
        "bitmap size mismatch",
        "inconsistent number of counts",
    ];
    const PROFILE_ERRORS: [&str; 4] = ["mismatch", "out of date", "malformed", "invalid"];
    DIRECT.iter().any(|pattern| lower.contains(*pattern))
        || lower
            .split_once("profile")
            .is_some_and(|(_, tail)| PROFILE_ERRORS.iter().any(|pattern| tail.contains(*pattern)))
        || ["failed", "unable", "cannot"].iter().any(|prefix| {
            lower
                .split_once(*prefix)
                .is_some_and(|(_, tail)| tail.contains("profile"))
        })
}

fn profile_warning(lower: &str) -> bool {
    lower.contains("profile")
        || lower
            .split(|character: char| !character.is_alphanumeric() && character != '_')
            .any(|word| word == "pgo")
}
