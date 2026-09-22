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
const NO_ENGINE: &str = "profile contains no observed executed Velum-related function";
const MISMATCH: &str = "Profile mismatch or invalid profile diagnostics";
const LEGACY_AS: &str = "$u20$as$u20$";

/// Checks an LLVM IR profile dump and preserves recognized executed Velum-related names.
pub fn verify_profile(show: &Path, symbols_out: &Path) -> Result<()> {
    let file = File::open(show)
        .with_context(|| format!("failed to open profile dump '{}'", show.display()))?;
    let mut ir_headers = 0_usize;
    let mut totals = BTreeSet::new();
    let mut hot_symbols = BTreeSet::new();
    let mut current_symbol: Option<String> = None;
    for line in BufReader::new(file).lines() {
        let line = line.context("failed to read profile dump")?;
        if let Some(body) = line.strip_prefix("    ") {
            if let Some(symbol) = &current_symbol
                && executed_count(body)?
            {
                hot_symbols.insert(symbol.clone());
            }
            continue;
        }
        current_symbol = profile_function(&line)
            .filter(|symbol| recognized_velum_symbol(symbol))
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
    ensure!(!hot_symbols.is_empty(), "{NO_ENGINE}");
    let file = File::create(symbols_out).with_context(|| {
        format!(
            "failed to create Velum-related symbols '{}'",
            symbols_out.display()
        )
    })?;
    let mut output = BufWriter::new(file);
    for symbol in hot_symbols {
        writeln!(output, "{symbol}").context("failed to write executed Velum-related symbol")?;
    }
    output
        .flush()
        .context("failed to flush Velum-related symbols")?;
    println!(
        "Validated nonempty IR profile with observed executed recognized Velum-related functions."
    );
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

fn recognized_velum_symbol(symbol: &str) -> bool {
    if let Some(path) = symbol.strip_prefix("_ZN") {
        return legacy_velum_symbol(path);
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

fn legacy_velum_symbol(path: &str) -> bool {
    let Some((component, tail)) = legacy_component(path) else {
        return false;
    };
    if legacy_component(tail).is_none() {
        return false;
    }
    if component == "velum" {
        return true;
    }
    // Conservatively protect implementations whose top-level self type or
    // trait is rooted in Velum, including local traits for primitive/std types.
    // This is not exhaustive crate provenance: an external crate can implement
    // a Velum trait too. Generic-argument mentions alone are not enough.
    let Some(implementation) = component
        .strip_prefix("_$LT$")
        .and_then(|name| name.strip_suffix("$GT$"))
    else {
        return false;
    };
    let Some((self_type, trait_type)) = legacy_impl_parts(implementation) else {
        return false;
    };
    legacy_velum_root(self_type) || trait_type.is_some_and(legacy_velum_root)
}

fn legacy_velum_root(name: &str) -> bool {
    name.strip_prefix("velum..").is_some_and(|path| {
        path.bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
    })
}

fn legacy_impl_parts(implementation: &str) -> Option<(&str, Option<&str>)> {
    let mut depth = 0_usize;
    let mut separator = None;
    for (offset, _) in implementation.match_indices('$') {
        let suffix = implementation.get(offset..)?;
        if suffix.starts_with("$LT$") {
            depth = depth.checked_add(1)?;
        } else if suffix.starts_with("$GT$") {
            depth = depth.checked_sub(1)?;
        } else if depth == 0 && suffix.starts_with(LEGACY_AS) && separator.replace(offset).is_some()
        {
            return None;
        }
    }
    if depth != 0 {
        return None;
    }
    let Some(offset) = separator else {
        return Some((implementation, None));
    };
    let self_type = implementation.get(..offset)?;
    let trait_offset = offset.checked_add(LEGACY_AS.len())?;
    let trait_type = implementation.get(trait_offset..)?;
    if self_type.is_empty() || trait_type.is_empty() {
        return None;
    }
    Some((self_type, Some(trait_type)))
}

fn legacy_component(path: &str) -> Option<(&str, &str)> {
    let digits = path.bytes().take_while(u8::is_ascii_digit).count();
    let length = path.get(..digits)?.parse::<usize>().ok()?;
    if length == 0 {
        return None;
    }
    let rest = path.get(digits..)?;
    Some((rest.get(..length)?, rest.get(length..)?))
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
                    "A previously executed Velum-related symbol is now missing from profile-use compilation: {symbol}"
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
    let text = fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read trained Velum-related symbols '{}'",
            path.display()
        )
    })?;
    let mut symbols = BTreeSet::new();
    for line in text.lines() {
        ensure!(
            recognized_velum_symbol(line)
                && !line.contains(char::is_whitespace)
                && !line.contains(';'),
            "invalid normalized Velum-related symbol '{line}'"
        );
        symbols.insert(line.to_owned());
    }
    ensure!(
        !symbols.is_empty(),
        "trained Velum-related symbols are empty"
    );
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
