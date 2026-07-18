use velum_regexp::{
    CompileErrorKind, CompileLimits, ExecutionControl, ExecutionError, ExecutionLimits, Flags,
    InterruptReason, Regex, is_id_continue, is_id_start, unicode_version,
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
