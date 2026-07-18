use velum_regexp::{CompileErrorKind, CompileLimits, ExecutionLimits, Flags, Regex};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn compile(pattern: &str, flags: Flags) -> Result<Regex, velum_regexp::CompileError> {
    Regex::compile(
        &pattern.encode_utf16().collect::<Vec<_>>(),
        flags,
        CompileLimits::default(),
    )
}

fn full_match(pattern: &str, input: &str, flags: Flags) -> TestResult {
    let regex = compile(pattern, flags)?;
    let units = input.encode_utf16().collect::<Vec<_>>();
    let matched = regex
        .find(&units, 0, ExecutionLimits::default())?
        .matched
        .ok_or_else(|| format!("expected /{pattern}/ to match {input:?}"))?;
    if matched.span == (0..units.len()) {
        return Ok(());
    }
    Err(format!("unexpected grammar match span: {matched:?}").into())
}

#[test]
fn legacy_mode_treats_malformed_quantifiers_and_brackets_as_literals() -> TestResult {
    for (pattern, input) in [
        (r"^a{$", "a{"),
        (r"^a{b$", "a{b"),
        (r"^a{1$", "a{1"),
        (r"^a{1,$", "a{1,"),
        (r"^a{1,x}$", "a{1,x}"),
        (r"^a{,1}$", "a{,1}"),
        (r"^]{}$", "]{}"),
    ] {
        full_match(pattern, input, Flags::default())?;
    }
    Ok(())
}

#[test]
fn unicode_modes_reject_malformed_quantifiers_and_bare_syntax_characters() -> TestResult {
    for flags in [
        Flags::default().with_unicode(true),
        Flags::default().with_unicode_sets(true),
    ] {
        for pattern in [
            "a{", "a{b", "a{1", "a{1,", "a{1,x}", "a{,1}", "{", "}", "a}", "]", "a]",
        ] {
            let result = compile(pattern, flags);
            if !matches!(
                result,
                Err(ref error)
                    if error.kind == CompileErrorKind::InvalidQuantifier
                        || error.kind == CompileErrorKind::UnexpectedToken
            ) {
                return Err(format!(
                    "unexpected Unicode grammar result for {pattern:?}: {result:?}"
                )
                .into());
            }
        }
    }
    Ok(())
}

#[test]
fn unicode_modes_accept_escaped_syntax_and_valid_quantifiers() -> TestResult {
    for flags in [
        Flags::default().with_unicode(true),
        Flags::default().with_unicode_sets(true),
    ] {
        full_match(r"^\]\{\}$", "]{}", flags)?;
        full_match(r"^a{2,3}$", "aaa", flags)?;
    }
    Ok(())
}

#[test]
fn reversed_quantifier_bounds_remain_errors_in_every_mode() -> TestResult {
    for flags in [
        Flags::default(),
        Flags::default().with_unicode(true),
        Flags::default().with_unicode_sets(true),
    ] {
        let result = compile("a{2,1}", flags);
        if !matches!(
            result,
            Err(ref error) if error.kind == CompileErrorKind::InvalidQuantifier
        ) {
            return Err(format!("unexpected reversed quantifier result: {result:?}").into());
        }
    }
    Ok(())
}
