use velum_regexp::{
    CompileLimits, ExecutionControl, ExecutionLimits, Flags, InterruptReason, Regex,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Default)]
struct CountingControl {
    steps: usize,
}

impl ExecutionControl for CountingControl {
    fn charge_steps(&mut self, steps: usize) -> Result<(), InterruptReason> {
        self.steps = self
            .steps
            .checked_add(steps)
            .ok_or(InterruptReason::Cancelled)?;
        Ok(())
    }
}

#[test]
fn legacy_capture_names_accept_ecmascript_unicode_escapes() -> TestResult {
    let pattern = r"^(?<\u{1d4d3}>x)\k<\ud835\udcd3>$";
    let regex = Regex::compile(
        &pattern.encode_utf16().collect::<Vec<_>>(),
        Flags::default(),
        CompileLimits::default(),
    )?;
    if regex.capture_name(0) != Some("𝓓") {
        return Err(format!("unexpected capture name: {:?}", regex.capture_name(0)).into());
    }
    let input = "xx".encode_utf16().collect::<Vec<_>>();
    let matched = regex
        .find(&input, 0, ExecutionLimits::default())?
        .matched
        .ok_or("escaped legacy capture name did not match")?;
    if matched.span == (0..2) {
        return Ok(());
    }
    Err(format!("unexpected escaped capture-name span: {:?}", matched.span).into())
}

#[test]
fn repeated_literal_ranges_are_normalized_before_execution() -> TestResult {
    let ranges = r"\u0000-\u0001".repeat(512);
    let pattern = format!("[{ranges}]");
    let regex = Regex::compile(
        &pattern.encode_utf16().collect::<Vec<_>>(),
        Flags::default(),
        CompileLimits::default(),
    )?;
    let mut control = CountingControl::default();
    let outcome =
        regex.find_with_control(&[0], 0, true, ExecutionLimits::default(), &mut control)?;
    if outcome.matched.is_none() {
        return Err("normalized repeated range did not match".into());
    }
    if control.steps != outcome.stats.steps {
        return Err(format!(
            "host charged {} steps but matcher recorded {}",
            control.steps, outcome.stats.steps
        )
        .into());
    }
    if control.steps <= 32 {
        return Ok(());
    }
    Err(format!("normalized character class charged {} steps", control.steps).into())
}

#[test]
fn embedders_can_raise_the_default_pattern_limit_within_the_hard_ceiling() -> TestResult {
    let defaults = CompileLimits::default();
    if defaults.max_pattern_units >= CompileLimits::MAXIMUM.max_pattern_units {
        return Err("default pattern limit did not leave bounded embedding headroom".into());
    }
    let pattern = format!(r"\u{{{}41}}", "0".repeat(defaults.max_pattern_units));
    let units = pattern.encode_utf16().collect::<Vec<_>>();
    let rejected = Regex::compile(
        &units,
        Flags::default().with_unicode(true),
        CompileLimits::default(),
    );
    if !matches!(
        rejected,
        Err(ref error)
            if error.kind
                == velum_regexp::CompileErrorKind::PatternTooLong {
                    limit: defaults.max_pattern_units,
                }
    ) {
        return Err(format!("unexpected default pattern-limit result: {rejected:?}").into());
    }
    Regex::compile(
        &units,
        Flags::default().with_unicode(true),
        CompileLimits {
            max_pattern_units: CompileLimits::MAXIMUM.max_pattern_units,
            ..CompileLimits::default()
        },
    )?;
    Ok(())
}

#[test]
fn embedders_can_raise_search_limits_within_hard_execution_ceilings() -> TestResult {
    let regex = Regex::compile(
        &"b".encode_utf16().collect::<Vec<_>>(),
        Flags::default(),
        CompileLimits::default(),
    )?;
    let input_length = ExecutionLimits::default()
        .max_candidate_starts
        .checked_add(1)
        .ok_or("search-limit test length overflowed")?;
    let input = vec![u16::from(b'a'); input_length];
    let default_result = regex.find(&input, 0, ExecutionLimits::default());
    if !matches!(
        default_result,
        Err(velum_regexp::ExecutionError::CandidateStartLimit { limit })
            if limit == ExecutionLimits::default().max_candidate_starts
    ) {
        return Err(format!("unexpected default search-limit result: {default_result:?}").into());
    }
    let raised = regex.find(
        &input,
        0,
        ExecutionLimits {
            max_steps: ExecutionLimits::MAXIMUM.max_steps,
            max_candidate_starts: ExecutionLimits::MAXIMUM.max_candidate_starts,
            ..ExecutionLimits::default()
        },
    )?;
    if raised.matched.is_none() {
        return Ok(());
    }
    Err("raised bounded search limits changed no-match semantics".into())
}

#[test]
fn anchored_terminal_repetition_does_not_retain_linear_backtracking_state() -> TestResult {
    let regex = Regex::compile(
        &"^a+$".encode_utf16().collect::<Vec<_>>(),
        Flags::default(),
        CompileLimits::default(),
    )?;
    let input = vec![u16::from(b'a'); 131_072];
    let outcome = regex.find(
        &input,
        0,
        ExecutionLimits {
            max_steps: ExecutionLimits::MAXIMUM.max_steps,
            ..ExecutionLimits::default()
        },
    )?;
    if outcome.matched.is_none() {
        return Err("anchored terminal repetition did not match".into());
    }
    if outcome.stats.max_backtrack_depth == 0 && outcome.stats.max_undo_depth == 0 {
        return Ok(());
    }
    Err(format!(
        "terminal repetition retained backtrack depth {} and undo depth {}",
        outcome.stats.max_backtrack_depth, outcome.stats.max_undo_depth
    )
    .into())
}

#[test]
fn max_safe_integer_repetitions_compile_without_instruction_expansion() -> TestResult {
    let limits = CompileLimits {
        max_repeat_count: CompileLimits::MAXIMUM.max_repeat_count,
        ..CompileLimits::default()
    };
    for (pattern, flags, input) in [
        (
            "b{9007199254740991}",
            Flags::default().with_unicode(true),
            "",
        ),
        ("b{9007199254740991,}?", Flags::default(), "a"),
        (
            "b{9007199254740991,9007199254740991}",
            Flags::default(),
            "b",
        ),
    ] {
        let regex = Regex::compile(&pattern.encode_utf16().collect::<Vec<_>>(), flags, limits)?;
        let input = input.encode_utf16().collect::<Vec<_>>();
        if regex
            .find(&input, 0, ExecutionLimits::default())?
            .matched
            .is_some()
        {
            return Err(format!("oversized repetition unexpectedly matched /{pattern}/").into());
        }
    }
    Ok(())
}
