use velum_regexp::{
    CompileErrorKind, CompileLimits, ExecutionControl, ExecutionError, ExecutionLimits, Flags,
    InterruptReason, Regex, binary_property_contains, binary_property_ranges, is_id_continue,
    is_id_start, unicode_version,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct InterruptAfter {
    remaining: usize,
}

impl ExecutionControl for InterruptAfter {
    fn charge_steps(&mut self, steps: usize) -> Result<(), InterruptReason> {
        self.remaining = self
            .remaining
            .checked_sub(steps)
            .ok_or(InterruptReason::Cancelled)?;
        Ok(())
    }
}

fn compile(pattern: &str) -> Result<Regex, velum_regexp::CompileError> {
    Regex::compile(
        &pattern.encode_utf16().collect::<Vec<_>>(),
        Flags::default(),
        CompileLimits::default(),
    )
}

fn compile_with_flags(pattern: &str, flags: Flags) -> Result<Regex, velum_regexp::CompileError> {
    Regex::compile(
        &pattern.encode_utf16().collect::<Vec<_>>(),
        flags,
        CompileLimits::default(),
    )
}

#[test]
fn generated_unicode_identifier_tables_are_available() -> TestResult {
    if unicode_version() != "17.0.0" {
        return Err(format!("unexpected Unicode version: {}", unicode_version()).into());
    }
    if is_id_start(u32::from('A'))
        && is_id_start(u32::from('λ'))
        && !is_id_start(u32::from('0'))
        && is_id_continue(u32::from('0'))
        && !is_id_continue(u32::from('-'))
    {
        return Ok(());
    }
    Err("generated Unicode identifier membership was incorrect".into())
}

#[test]
fn unicode_binary_property_names_follow_ecmascript_exact_matching() -> TestResult {
    if binary_property_contains("Alphabetic", u32::from('λ'))
        && binary_property_contains("Alpha", u32::from('A'))
        && binary_property_contains("Emoji", 0x1F600)
        && binary_property_contains("White_Space", u32::from(' '))
        && binary_property_ranges("alphabetic").is_none()
        && binary_property_ranges("Unsupported_Property").is_none()
    {
        return Ok(());
    }
    Err("binary Unicode property lookup did not use exact ECMAScript names".into())
}

#[test]
fn matches_character_ranges_predefined_classes_and_negation() -> TestResult {
    let mixed = compile(r"[a-c\d]+")?;
    let input = "xxb29!".encode_utf16().collect::<Vec<_>>();
    let matched = mixed
        .find(&input, 0, ExecutionLimits::default())?
        .matched
        .ok_or("expected a mixed character class match")?;
    if matched.span != (2..5) {
        return Err(format!("unexpected mixed class span: {:?}", matched.span).into());
    }

    let non_space = compile(r"[^\s]+")?;
    let input = " \tfoo ".encode_utf16().collect::<Vec<_>>();
    let matched = non_space
        .find(&input, 0, ExecutionLimits::default())?
        .matched
        .ok_or("expected a non-space match")?;
    if matched.span == (2..5) {
        return Ok(());
    }
    Err(format!("unexpected negated class span: {:?}", matched.span).into())
}

#[test]
fn matches_unicode_property_escapes_using_utf16_positions() -> TestResult {
    let regex = compile_with_flags(r"\p{Emoji}+", Flags::default().with_unicode(true))?;
    let input = "x😀🐸!".encode_utf16().collect::<Vec<_>>();
    let matched = regex
        .find(&input, 0, ExecutionLimits::default())?
        .matched
        .ok_or("expected an emoji property match")?;
    if matched.span == (1..5) {
        return Ok(());
    }
    Err(format!("unexpected property escape span: {:?}", matched.span).into())
}

#[test]
fn matches_general_categories_scripts_and_script_extensions() -> TestResult {
    let letters = compile_with_flags(r"\p{Letter}+", Flags::default().with_unicode(true))?;
    let greek = "42λΩ!".encode_utf16().collect::<Vec<_>>();
    let matched = letters
        .find(&greek, 0, ExecutionLimits::default())?
        .matched
        .ok_or("expected a General_Category match")?;
    if matched.span != (2..4) {
        return Err(format!("unexpected category span: {:?}", matched.span).into());
    }

    let script = compile_with_flags(r"\p{Script=Greek}", Flags::default().with_unicode(true))?;
    let extensions = compile_with_flags(r"\p{scx=Grek}", Flags::default().with_unicode(true))?;
    let middle_dot = "·".encode_utf16().collect::<Vec<_>>();
    if script
        .find(&middle_dot, 0, ExecutionLimits::default())?
        .matched
        .is_some()
    {
        return Err("Script=Greek incorrectly included a Common character".into());
    }
    if extensions
        .find(&middle_dot, 0, ExecutionLimits::default())?
        .matched
        .is_some()
    {
        return Ok(());
    }
    Err("Script_Extensions=Greek omitted the shared middle dot".into())
}

#[test]
fn ignore_case_uses_distinct_legacy_and_unicode_canonicalization() -> TestResult {
    let legacy_flags = Flags::default().with_ignore_case(true);
    let unicode_flags = legacy_flags.with_unicode(true);

    let ascii_range = compile_with_flags("^[A-Z]+$", legacy_flags)?;
    let lowercase = "velum".encode_utf16().collect::<Vec<_>>();
    if ascii_range
        .find(&lowercase, 0, ExecutionLimits::default())?
        .matched
        .is_none()
    {
        return Err("legacy ignore-case range did not match ASCII lowercase".into());
    }

    let unicode_word = compile_with_flags(r"^\w$", unicode_flags)?;
    let kelvin = "\u{212A}".encode_utf16().collect::<Vec<_>>();
    if unicode_word
        .find(&kelvin, 0, ExecutionLimits::default())?
        .matched
        .is_none()
    {
        return Err("Unicode simple folding did not include Kelvin sign in word characters".into());
    }

    let legacy_s = compile_with_flags("^s$", legacy_flags)?;
    let unicode_s = compile_with_flags("^s$", unicode_flags)?;
    let long_s = "ſ".encode_utf16().collect::<Vec<_>>();
    if legacy_s
        .find(&long_s, 0, ExecutionLimits::default())?
        .matched
        .is_some()
    {
        return Err("legacy canonicalization incorrectly folded non-ASCII to ASCII".into());
    }
    if unicode_s
        .find(&long_s, 0, ExecutionLimits::default())?
        .matched
        .is_some()
    {
        return Ok(());
    }
    Err("Unicode canonicalization did not apply simple case folding".into())
}

#[test]
fn property_complements_follow_unicode_and_unicode_sets_fold_order() -> TestResult {
    let unicode = compile_with_flags(
        r"^\P{Lowercase_Letter}$",
        Flags::default().with_ignore_case(true).with_unicode(true),
    )?;
    let unicode_sets = compile_with_flags(
        r"^\P{Lowercase_Letter}$",
        Flags::default()
            .with_ignore_case(true)
            .with_unicode_sets(true),
    )?;
    let lowercase = "a".encode_utf16().collect::<Vec<_>>();
    if unicode
        .find(&lowercase, 0, ExecutionLimits::default())?
        .matched
        .is_none()
    {
        return Err("Unicode mode complemented after folding instead of before it".into());
    }
    if unicode_sets
        .find(&lowercase, 0, ExecutionLimits::default())?
        .matched
        .is_none()
    {
        return Ok(());
    }
    Err("Unicode Sets mode complemented before folding instead of after it".into())
}

#[test]
fn supports_word_boundaries_and_any_character_classes() -> TestResult {
    let boundary = compile(r"\bcat\B")?;
    let input = "cats".encode_utf16().collect::<Vec<_>>();
    let matched = boundary
        .find(&input, 0, ExecutionLimits::default())?
        .matched
        .ok_or("expected a word-boundary match")?;
    if matched.span != (0..3) {
        return Err(format!("unexpected boundary span: {:?}", matched.span).into());
    }

    let any = compile("[^]")?;
    let newline = "\n".encode_utf16().collect::<Vec<_>>();
    if any
        .find(&newline, 0, ExecutionLimits::default())?
        .matched
        .is_some()
    {
        return Ok(());
    }
    Err("a negated empty class did not match a line terminator".into())
}

#[test]
fn rejects_invalid_classes_properties_and_class_resource_exhaustion() -> TestResult {
    let invalid_range = compile("[z-a]");
    if !matches!(
        invalid_range,
        Err(velum_regexp::CompileError {
            kind: CompileErrorKind::InvalidCharacterClass,
            ..
        })
    ) {
        return Err(format!("unexpected invalid range result: {invalid_range:?}").into());
    }

    let invalid_property =
        compile_with_flags(r"\p{alphabetic}", Flags::default().with_unicode(true));
    if !matches!(
        invalid_property,
        Err(velum_regexp::CompileError {
            kind: CompileErrorKind::InvalidUnicodeProperty,
            ..
        })
    ) {
        return Err(format!("unexpected invalid property result: {invalid_property:?}").into());
    }

    let limited = Regex::compile(
        &"[abc]".encode_utf16().collect::<Vec<_>>(),
        Flags::default(),
        CompileLimits {
            max_character_class_terms: 2,
            ..CompileLimits::default()
        },
    );
    if matches!(
        limited,
        Err(velum_regexp::CompileError {
            kind: CompileErrorKind::NodeLimit { limit: 2 },
            ..
        })
    ) {
        return Ok(());
    }
    Err(format!("unexpected class limit result: {limited:?}").into())
}

#[test]
fn searches_literals_alternation_and_greedy_repetition() -> TestResult {
    let regex = compile("(ab|a)+c")?;
    let input = "zzabac!".encode_utf16().collect::<Vec<_>>();
    let outcome = regex.find(&input, 0, ExecutionLimits::default())?;
    let matched = outcome.matched.ok_or("expected a match")?;
    if matched.span != (2..6) {
        return Err(format!("unexpected match span: {:?}", matched.span).into());
    }
    let capture = matched
        .captures
        .first()
        .and_then(|capture| capture.span.clone())
        .ok_or("expected capture one")?;
    if capture != (4..5) {
        return Err(format!("unexpected capture span: {capture:?}").into());
    }
    Ok(())
}

#[test]
fn supports_lazy_bounded_repetition_and_anchors() -> TestResult {
    let regex = compile("^a{2,4}?a$")?;
    let input = "aaa".encode_utf16().collect::<Vec<_>>();
    let outcome = regex.find(&input, 0, ExecutionLimits::default())?;
    let matched = outcome.matched.ok_or("expected a match")?;
    if matched.span == (0..3) {
        return Ok(());
    }
    Err(format!("unexpected match span: {:?}", matched.span).into())
}

#[test]
fn unicode_mode_uses_code_points_but_reports_code_units() -> TestResult {
    let pattern = "🐸.".encode_utf16().collect::<Vec<_>>();
    let input = "x🐸y".encode_utf16().collect::<Vec<_>>();
    let regex = Regex::compile(
        &pattern,
        Flags::default().with_unicode(true),
        CompileLimits::default(),
    )?;
    let outcome = regex.find(&input, 0, ExecutionLimits::default())?;
    let matched = outcome.matched.ok_or("expected a match")?;
    if matched.span == (1..4) {
        return Ok(());
    }
    Err(format!("unexpected UTF-16 span: {:?}", matched.span).into())
}

#[test]
fn empty_unbounded_repetition_terminates() -> TestResult {
    let regex = compile("(?:)*")?;
    let outcome = regex.find(&[], 0, ExecutionLimits::default())?;
    let matched = outcome.matched.ok_or("expected an empty match")?;
    if matched.span != (0..0) {
        return Err(format!("unexpected empty span: {:?}", matched.span).into());
    }
    if outcome.stats.steps > 16 {
        return Err(format!("empty loop used {} steps", outcome.stats.steps).into());
    }
    Ok(())
}

#[test]
fn compile_limits_fail_before_unbounded_growth() -> TestResult {
    let error = Regex::compile(
        &"a{100}".encode_utf16().collect::<Vec<_>>(),
        Flags::default(),
        CompileLimits {
            max_repeat_count: 10,
            ..CompileLimits::default()
        },
    )
    .err()
    .ok_or("expected repeat limit failure")?;
    if matches!(error.kind, CompileErrorKind::RepeatLimit { limit: 10 }) {
        return Ok(());
    }
    Err(format!("unexpected compile error: {error:?}").into())
}

#[test]
fn execution_limits_and_host_interrupts_are_distinct() -> TestResult {
    let regex = compile("(a|aa)*b")?;
    let input = "aaaaaaaaaaaaaaaa".encode_utf16().collect::<Vec<_>>();
    let local = regex.find(
        &input,
        0,
        ExecutionLimits {
            max_steps: 20,
            ..ExecutionLimits::default()
        },
    );
    if !matches!(local, Err(ExecutionError::StepLimit { limit: 20 })) {
        return Err(format!("unexpected local limit result: {local:?}").into());
    }

    let mut control = InterruptAfter { remaining: 5 };
    let interrupted =
        regex.find_with_control(&input, 0, false, ExecutionLimits::default(), &mut control);
    if matches!(
        interrupted,
        Err(ExecutionError::Interrupted(InterruptReason::Cancelled))
    ) {
        return Ok(());
    }
    Err(format!("unexpected interruption result: {interrupted:?}").into())
}
