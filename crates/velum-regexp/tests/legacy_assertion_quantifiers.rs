use velum_regexp::{CompileErrorKind, CompileLimits, ExecutionLimits, Flags, Regex};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn compile(
    pattern: &str,
    flags: Flags,
    limits: CompileLimits,
) -> Result<Regex, velum_regexp::CompileError> {
    Regex::compile(&pattern.encode_utf16().collect::<Vec<_>>(), flags, limits)
}

fn matched(pattern: &str, input: &str) -> TestResult {
    let regex = compile(pattern, Flags::default(), CompileLimits::default())?;
    let units = input.encode_utf16().collect::<Vec<_>>();
    if regex
        .find(&units, 0, ExecutionLimits::default())?
        .matched
        .is_some()
    {
        return Ok(());
    }
    Err(format!("expected /{pattern}/ to match {input:?}").into())
}

#[test]
fn legacy_lookaheads_accept_quantifiers_and_preserve_zero_width_matching() -> TestResult {
    for pattern in [
        r"^(?=a)?a$",
        r"^(?=a)+a$",
        r"^(?=a)*a$",
        r"^(?=a){0,2}a$",
        r"^(?!b){2}a$",
        r"^(?!b){1,}a$",
    ] {
        matched(pattern, "a")?;
    }
    Ok(())
}

#[test]
fn empty_assertion_repetition_runs_only_its_required_iterations() -> TestResult {
    for quantifier in ["?", "??", "*", "{0,}", "{0,2}"] {
        let pattern = format!(r"^(?=(a)){quantifier}a$");
        let regex = compile(&pattern, Flags::default(), CompileLimits::default())?;
        let input = "a".encode_utf16().collect::<Vec<_>>();
        let result = regex
            .find(&input, 0, ExecutionLimits::default())?
            .matched
            .ok_or_else(|| format!("expected {pattern} to match"))?;
        let capture = result
            .captures
            .first()
            .ok_or("missing optional lookahead capture")?;
        if capture.span.is_some() {
            return Err(format!("optional lookahead captured for {pattern}: {result:?}").into());
        }
    }

    for quantifier in ["+", "{1,}", "{2,}", "{1,2}", "{2}"] {
        let pattern = format!(r"^(?=(a)){quantifier}a$");
        let regex = compile(&pattern, Flags::default(), CompileLimits::default())?;
        let input = "a".encode_utf16().collect::<Vec<_>>();
        let result = regex
            .find(&input, 0, ExecutionLimits::default())?
            .matched
            .ok_or_else(|| format!("expected {pattern} to match"))?;
        let capture = result
            .captures
            .first()
            .ok_or("missing required lookahead capture")?;
        if capture.span != Some(0..1) {
            return Err(
                format!("required lookahead did not capture for {pattern}: {result:?}").into(),
            );
        }
    }
    Ok(())
}

#[test]
fn unicode_modes_and_other_assertions_reject_quantifiers() -> TestResult {
    for (pattern, flags) in [
        (r"(?=a)?", Flags::default().with_unicode(true)),
        (r"(?=a)?", Flags::default().with_unicode_sets(true)),
        (r"(?<=a)?", Flags::default()),
        (r"^?", Flags::default()),
        (r"\b+", Flags::default()),
    ] {
        let result = compile(pattern, flags, CompileLimits::default());
        if !matches!(
            result,
            Err(ref error) if error.kind == CompileErrorKind::InvalidQuantifier
        ) {
            return Err(format!(
                "unexpected assertion quantifier result for {pattern}: {result:?}"
            )
            .into());
        }
    }
    Ok(())
}

#[test]
fn required_lookahead_repetition_obeys_instruction_limits() -> TestResult {
    let result = compile(
        r"(?=a){100}a",
        Flags::default(),
        CompileLimits {
            max_instructions: 16,
            ..CompileLimits::default()
        },
    );
    if matches!(
        result,
        Err(ref error) if error.kind == CompileErrorKind::InstructionLimit { limit: 16 }
    ) {
        return Ok(());
    }
    Err(format!("unexpected assertion instruction limit: {result:?}").into())
}
