use std::ops::Range;

use velum_regexp::{CompileLimits, ExecutionLimits, Flags, Regex};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn zero_length_repetition_rolls_back_iteration_captures() -> TestResult {
    for pattern in [r"(a*)*", r"(a?)*", r"(a*?)*?", r"((?:))*"] {
        assert_match(pattern, "", 0..0, &[None])?;
        assert_match(pattern, "b", 0..0, &[None])?;
    }
    Ok(())
}

#[test]
fn zero_length_alternatives_can_backtrack_to_consuming_paths() -> TestResult {
    assert_match(r"(?:|a)*", "a", 0..1, &[])?;
    assert_match(r"((?:)|a)*", "a", 0..1, &[Some(0..1)])?;
    assert_match(r"(?:a*|b)*", "b", 0..1, &[])
}

fn assert_match(
    pattern: &str,
    input: &str,
    expected_span: Range<usize>,
    expected_captures: &[Option<Range<usize>>],
) -> TestResult {
    let regex = Regex::compile(
        &pattern.encode_utf16().collect::<Vec<_>>(),
        Flags::default(),
        CompileLimits::default(),
    )?;
    let matched = regex
        .find(
            &input.encode_utf16().collect::<Vec<_>>(),
            0,
            ExecutionLimits::default(),
        )?
        .matched
        .ok_or_else(|| format!("expected /{pattern}/ to match {input:?}"))?;
    let captures = matched
        .captures
        .into_iter()
        .map(|capture| capture.span)
        .collect::<Vec<_>>();
    if matched.span == expected_span && captures == expected_captures {
        return Ok(());
    }
    Err(format!(
        "unexpected repetition result for /{pattern}/ on {input:?}: span={:?}, captures={captures:?}",
        matched.span
    )
    .into())
}
