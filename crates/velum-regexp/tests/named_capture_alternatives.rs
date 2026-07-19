use velum_regexp::{CompileErrorKind, CompileLimits, ExecutionLimits, Flags, Regex};

type TestResult = Result<(), Box<dyn std::error::Error>>;

// Behavioral cases include adaptations from TC39 Test262 at commit
// 64ff467c0c1d60c077995bb7c5f93a9d8cc8ade1, under its BSD license:
// test/built-ins/RegExp/duplicate-named-capturing-groups-syntax.js and
// test/staging/built-ins/RegExp/named-groups/duplicate-named-groups.js.

fn compile(pattern: &str, flags: Flags) -> Result<Regex, velum_regexp::CompileError> {
    Regex::compile(
        &pattern.encode_utf16().collect::<Vec<_>>(),
        flags,
        CompileLimits::default(),
    )
}

fn matches(pattern: &str, input: &str, flags: Flags) -> TestResult {
    let regex = compile(pattern, flags)?;
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

fn rejects_match(pattern: &str, input: &str, flags: Flags) -> TestResult {
    let regex = compile(pattern, flags)?;
    let units = input.encode_utf16().collect::<Vec<_>>();
    if regex
        .find(&units, 0, ExecutionLimits::default())?
        .matched
        .is_none()
    {
        return Ok(());
    }
    Err(format!("expected /{pattern}/ to reject {input:?}").into())
}

#[test]
fn duplicate_names_are_allowed_only_on_disjoint_alternative_paths() -> TestResult {
    for pattern in [
        r"(?<x>a)|(?<x>b)",
        r"(?:(?<x>a)|(?<x>b))",
        r"(?:(?<x>a)|(?:(?<x>b)|(?<x>c)))",
        r"(?:(?<x>a)|b)|(?<x>c)",
    ] {
        compile(pattern, Flags::default())?;
    }

    for pattern in [
        r"(?<x>a)(?<x>b)",
        r"(?:(?<x>a)|b)(?<x>c)",
        r"(?<x>a(?:(?<x>b)|c))",
        r"(?:(?<x>a)|(?<x>b))(?<x>c)",
        r"(?:(?<x>a)(?<x>b)|c)",
    ] {
        let result = compile(pattern, Flags::default());
        if !matches!(
            result,
            Err(ref error) if error.kind == CompileErrorKind::DuplicateCaptureName
        ) {
            return Err(
                format!("unexpected duplicate-name result for {pattern}: {result:?}").into(),
            );
        }
    }
    Ok(())
}

#[test]
fn named_backreferences_select_the_participating_duplicate_capture() -> TestResult {
    let pattern = r"^(?:(?<x>a)|(?<x>b))\k<x>$";
    matches(pattern, "aa", Flags::default())?;
    matches(pattern, "bb", Flags::default())?;
    rejects_match(pattern, "ab", Flags::default())?;
    rejects_match(pattern, "ba", Flags::default())?;
    matches(r"^(?:(?<x>a)|(?<x>b))(?i:\k<x>)$", "bB", Flags::default())
}

#[test]
fn duplicate_capture_slots_remain_independently_observable() -> TestResult {
    let regex = compile(r"^(?:(?<x>a)|(?<x>b))$", Flags::default())?;
    if regex.capture_count() != 2
        || regex.capture_name(0) != Some("x")
        || regex.capture_name(1) != Some("x")
        || regex.capture_index("x") != Some(0)
    {
        return Err("duplicate capture metadata was not retained".into());
    }
    let input = "b".encode_utf16().collect::<Vec<_>>();
    let matched = regex
        .find(&input, 0, ExecutionLimits::default())?
        .matched
        .ok_or("expected the second named capture alternative")?;
    let first = matched
        .captures
        .first()
        .ok_or("missing first duplicate capture slot")?;
    let second = matched
        .captures
        .get(1)
        .ok_or("missing second duplicate capture slot")?;
    if first.span.is_none() && second.span == Some(0..1) {
        return Ok(());
    }
    Err(format!("unexpected duplicate capture spans: {matched:?}").into())
}

#[test]
fn duplicate_named_backreferences_follow_the_last_repetition() -> TestResult {
    matches(r"^(?:(?<x>a)|(?<x>b))+\k<x>$", "abaa", Flags::default())?;
    matches(r"^(?:(?<x>a)|b)\k<x>$", "b", Flags::default())?;
    matches(
        r"^(?:(?:(?<x>a)|(?<x>b)|c)\k<x>){2}$",
        "aac",
        Flags::default(),
    )?;
    matches(r"(?:(?:(?<a>x)|(?<a>y))\k<a>){2}", "xxyy", Flags::default())
}
