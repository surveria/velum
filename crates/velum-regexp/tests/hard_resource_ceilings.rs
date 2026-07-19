use velum_regexp::{
    CompileErrorKind, CompileLimits, ExecutionError, ExecutionLimits, Flags, Regex,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn callers_cannot_raise_the_native_parser_depth_ceiling() -> TestResult {
    let depth = CompileLimits::MAXIMUM
        .max_nesting_depth
        .checked_add(1)
        .ok_or("nesting test depth overflowed")?;
    let mut pattern = "(".repeat(depth);
    pattern.push('a');
    pattern.push_str(&")".repeat(depth));
    let result = Regex::compile(
        &pattern.encode_utf16().collect::<Vec<_>>(),
        Flags::default(),
        CompileLimits {
            max_nesting_depth: usize::MAX,
            ..CompileLimits::MAXIMUM
        },
    );
    if matches!(
        result,
        Err(ref error)
            if error.kind
                == CompileErrorKind::NestingLimit {
                    limit: CompileLimits::MAXIMUM.max_nesting_depth,
                }
    ) {
        return Ok(());
    }
    Err(format!("unexpected hard nesting ceiling result: {result:?}").into())
}

#[test]
fn callers_cannot_raise_the_pattern_size_ceiling() -> TestResult {
    let length = CompileLimits::MAXIMUM
        .max_pattern_units
        .checked_add(1)
        .ok_or("pattern test length overflowed")?;
    let pattern = vec![u16::from(b'a'); length];
    let result = Regex::compile(
        &pattern,
        Flags::default(),
        CompileLimits {
            max_pattern_units: usize::MAX,
            ..CompileLimits::MAXIMUM
        },
    );
    if matches!(
        result,
        Err(ref error)
            if error.kind
                == CompileErrorKind::PatternTooLong {
                    limit: CompileLimits::MAXIMUM.max_pattern_units,
                }
    ) {
        return Ok(());
    }
    Err(format!("unexpected hard pattern ceiling result: {result:?}").into())
}

#[test]
fn callers_cannot_raise_the_ecmascript_repeat_ceiling() -> TestResult {
    let pattern = "a{9007199254740992}".encode_utf16().collect::<Vec<_>>();
    let result = Regex::compile(
        &pattern,
        Flags::default(),
        CompileLimits {
            max_repeat_count: u64::MAX,
            ..CompileLimits::MAXIMUM
        },
    );
    if matches!(
        result,
        Err(ref error)
            if error.kind
                == CompileErrorKind::RepeatLimit {
                    limit: CompileLimits::MAXIMUM.max_repeat_count,
                }
    ) {
        return Ok(());
    }
    Err(format!("unexpected hard repeat ceiling result: {result:?}").into())
}

#[test]
fn callers_cannot_raise_the_backtrack_storage_ceiling() -> TestResult {
    let regex = Regex::compile(
        &"(?:a|aa)*b".encode_utf16().collect::<Vec<_>>(),
        Flags::default(),
        CompileLimits::default(),
    )?;
    let length = ExecutionLimits::MAXIMUM
        .max_backtrack_frames
        .checked_add(1)
        .ok_or("backtrack test length overflowed")?;
    let input = vec![u16::from(b'a'); length];
    let result = regex.find(
        &input,
        0,
        ExecutionLimits {
            max_steps: usize::MAX,
            max_candidate_starts: usize::MAX,
            max_backtrack_frames: usize::MAX,
            max_undo_records: usize::MAX,
            max_capture_slots: usize::MAX,
        },
    );
    if matches!(
        result,
        Err(ExecutionError::BacktrackLimit { limit })
            if limit == ExecutionLimits::MAXIMUM.max_backtrack_frames
    ) {
        return Ok(());
    }
    Err(format!("unexpected hard execution ceiling result: {result:?}").into())
}

#[test]
fn wide_alternations_compile_without_recursive_tail_descent() -> TestResult {
    let alternatives = 10_000_usize;
    let mut pattern = String::new();
    for index in 0..alternatives {
        if index > 0 {
            pattern.push('|');
        }
        pattern.push('a');
    }
    let regex = Regex::compile(
        &pattern.encode_utf16().collect::<Vec<_>>(),
        Flags::default(),
        CompileLimits::default(),
    )?;
    let outcome = regex.find(
        &"a".encode_utf16().collect::<Vec<_>>(),
        0,
        ExecutionLimits::default(),
    )?;
    if outcome.matched.is_some() {
        return Ok(());
    }
    Err("wide alternation did not match".into())
}
