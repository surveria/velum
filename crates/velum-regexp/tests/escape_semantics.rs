use velum_regexp::{CompileErrorKind, CompileLimits, ExecutionLimits, Flags, Regex};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn compile(pattern: &str, flags: Flags) -> Result<Regex, velum_regexp::CompileError> {
    Regex::compile(
        &pattern.encode_utf16().collect::<Vec<_>>(),
        flags,
        CompileLimits::default(),
    )
}

fn assert_matches(pattern: &str, input: &str, flags: Flags) -> TestResult {
    let regex = compile(pattern, flags)?;
    let units = input.encode_utf16().collect::<Vec<_>>();
    let matched = regex
        .find(&units, 0, ExecutionLimits::default())?
        .matched
        .ok_or_else(|| format!("expected {pattern:?} to match {input:?}"))?;
    if matched.span == (0..units.len()) {
        return Ok(());
    }
    Err(format!("unexpected span for {pattern:?}: {:?}", matched.span).into())
}

fn assert_compile_error(pattern: &str, flags: Flags, expected: &CompileErrorKind) -> TestResult {
    let result = compile(pattern, flags);
    if matches!(result, Err(ref error) if &error.kind == expected) {
        return Ok(());
    }
    Err(format!("unexpected compile result for {pattern:?}: {result:?}").into())
}

#[test]
fn decimal_escapes_use_the_complete_capture_count() -> TestResult {
    assert_matches(r"^\1(a)$", "a", Flags::default())?;
    assert_matches(
        r"^(a)(b)(c)(d)(e)(f)(g)(h)(i)(j)\10$",
        "abcdefghijj",
        Flags::default(),
    )?;
    assert_matches(r"^\1[(](a)$", "(a", Flags::default())
}

#[test]
fn legacy_decimal_escapes_follow_annex_b_octal_widths() -> TestResult {
    for (pattern, input) in [
        (r"^(a)\12$", "a\n"),
        (r"^(a)\18$", "a\u{0001}8"),
        (r"^\1234$", "S4"),
        (r"^\400$", " 0"),
        (r"^\8$", "8"),
        (r"^\08$", "\0"),
    ] {
        let input = if pattern == r"^\08$" {
            "\0".to_owned() + "8"
        } else {
            input.to_owned()
        };
        assert_matches(pattern, &input, Flags::default())?;
    }
    Ok(())
}

#[test]
fn unicode_modes_reject_legacy_decimal_forms() -> TestResult {
    let unicode = Flags::default().with_unicode(true);
    for (pattern, expected) in [
        (r"(a)\2", CompileErrorKind::InvalidBackreference),
        (r"\8", CompileErrorKind::InvalidBackreference),
        (r"\01", CompileErrorKind::InvalidEscape),
        (r"\08", CompileErrorKind::InvalidEscape),
    ] {
        assert_compile_error(pattern, unicode, &expected)?;
    }
    assert_matches(r"^\0$", "\0", unicode)
}

#[test]
fn control_and_malformed_hex_escapes_distinguish_legacy_mode() -> TestResult {
    assert_matches(r"^\cA$", "\u{0001}", Flags::default())?;
    assert_matches(r"^\ca$", "\u{0001}", Flags::default().with_unicode(true))?;
    assert_matches(r"^\c1$", r"\c1", Flags::default())?;
    assert_matches(r"^\xZ1$", "xZ1", Flags::default())?;
    assert_matches(r"^\uZZZZ$", "uZZZZ", Flags::default())?;

    let unicode = Flags::default().with_unicode(true);
    for pattern in [r"\c1", r"\xZ1", r"\uZZZZ"] {
        assert_compile_error(pattern, unicode, &CompileErrorKind::InvalidEscape)?;
    }
    Ok(())
}

#[test]
fn legacy_invalid_control_prefix_keeps_quantifiers_on_the_final_atom() -> TestResult {
    let regex = compile(r"\c*", Flags::default())?;
    for rejected in ["\n", "c*"] {
        let input = rejected.encode_utf16().collect::<Vec<_>>();
        if regex
            .find(&input, 0, ExecutionLimits::default())?
            .matched
            .is_some()
        {
            return Err(format!("invalid control fallback matched {rejected:?}").into());
        }
    }
    let input = r"\c*".encode_utf16().collect::<Vec<_>>();
    let matched = regex
        .find(&input, 0, ExecutionLimits::default())?
        .matched
        .ok_or("invalid control fallback did not match its source prefix")?;
    if matched.span == (0..2) {
        return Ok(());
    }
    Err(format!(
        "unexpected invalid control fallback span: {:?}",
        matched.span
    )
    .into())
}

#[test]
fn escaped_surrogate_pairs_match_one_unicode_code_point() -> TestResult {
    let unicode = Flags::default().with_unicode(true);
    assert_matches(r"^\uD83D\uDE00$", "😀", unicode)?;
    assert_matches(r"^[\uD83D\uDE00]$", "😀", unicode)
}

#[test]
fn class_escapes_preserve_legacy_control_and_octal_rules() -> TestResult {
    for (pattern, input) in [
        (r"^[\c1]$", "\u{0011}"),
        (r"^[\c_]$", "\u{001F}"),
        (r"^[\c]$", "\\"),
        (r"^[\c]$", "c"),
        (r"^[\01]$", "\u{0001}"),
        (r"^[\08]$", "\0"),
        (r"^[\08]$", "8"),
    ] {
        assert_matches(pattern, input, Flags::default())?;
    }

    let unicode = Flags::default().with_unicode(true);
    for pattern in [r"[\c1]", r"[\01]", r"[\8]"] {
        assert_compile_error(pattern, unicode, &CompileErrorKind::InvalidEscape)?;
    }
    Ok(())
}

#[test]
fn named_backreference_prefix_is_identity_only_without_named_groups() -> TestResult {
    assert_matches(r"^\k<x>$", "k<x>", Flags::default())?;
    assert_compile_error(
        r"(?<a>a)\k",
        Flags::default(),
        &CompileErrorKind::InvalidBackreference,
    )?;
    assert_compile_error(
        r"\k<x>",
        Flags::default().with_unicode(true),
        &CompileErrorKind::UnknownCaptureName,
    )
}
