use velum_regexp::{CompileErrorKind, CompileLimits, ExecutionLimits, Flags, Regex};

type TestResult = Result<(), Box<dyn std::error::Error>>;

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
fn modifiers_scope_dot_all_and_multiline_without_leaking() -> TestResult {
    matches(r"(?s:^.$)", "\n", Flags::default())?;
    rejects_match(r"(?-s:^.$)", "\n", Flags::default().with_dot_all(true))?;
    rejects_match(r"(?s:^.$)", "𐌀", Flags::default())?;
    matches(r"(?m:^b)", "a\nb", Flags::default())?;
    rejects_match(r"(?-m:^b)", "a\nb", Flags::default().with_multiline(true))?;
    matches(r"^(?i:a(?-i:b)c)d$", "AbCd", Flags::default())?;
    rejects_match(r"^(?i:a(?-i:b)c)d$", "ABCd", Flags::default())?;
    rejects_match(r"^(?i:a(?-i:b)c)d$", "AbCD", Flags::default())
}

#[test]
fn modifiers_apply_to_literals_classes_backreferences_and_boundaries() -> TestResult {
    matches(r"^(a)(?i:\1)$", "aA", Flags::default())?;
    rejects_match(
        r"^(a)(?-i:\1)$",
        "aA",
        Flags::default().with_ignore_case(true),
    )?;
    matches(r"(?i:\b)", "ſ", Flags::default().with_unicode(true))?;
    rejects_match(
        r"(?-i:\b)",
        "ſ",
        Flags::default().with_unicode(true).with_ignore_case(true),
    )?;
    matches(r"(?i:\p{Lu})", "a", Flags::default().with_unicode(true))?;
    matches(r"(?i:\P{Lu})", "A", Flags::default().with_unicode(true))?;
    rejects_match(
        r"(?-i:\p{Lu})",
        "a",
        Flags::default().with_unicode(true).with_ignore_case(true),
    )
}

#[test]
fn modifiers_apply_before_unicode_string_set_algebra() -> TestResult {
    let flags = Flags::default().with_unicode_sets(true);
    matches(r"^(?i:[\q{ab}&&\q{AB}])$", "aB", flags)?;
    rejects_match(r"^(?i:[\q{a}--[A]])$", "a", flags)?;
    matches(r"^(?i:[\q{ab}]|x)$", "AB", flags)
}

#[test]
fn modifiers_work_inside_reverse_execution_and_quantifiers() -> TestResult {
    matches(r"(?<=(?i:ab))X", "aBX", Flags::default())?;
    matches(r"^(?i:a)+$", "aAaA", Flags::default())?;
    rejects_match(r"^(?i:a)+b$", "AAB", Flags::default())
}

#[test]
fn invalid_modifier_lists_report_structured_errors() -> TestResult {
    for pattern in [
        "(?i-i:a)", "(?ii:a)", "(?i-:a)", "(?-:a)", "(?--i:a)", "(?i=a)", "(?i)",
    ] {
        let result = compile(pattern, Flags::default());
        if !matches!(result, Err(ref error) if error.kind == CompileErrorKind::InvalidModifier) {
            return Err(format!("unexpected modifier error for {pattern}: {result:?}").into());
        }
    }
    let unsupported = compile("(?x:a)", Flags::default());
    if matches!(
        unsupported,
        Err(ref error) if error.kind == CompileErrorKind::UnsupportedSyntax
    ) {
        return Ok(());
    }
    Err(format!("unexpected unsupported modifier result: {unsupported:?}").into())
}
