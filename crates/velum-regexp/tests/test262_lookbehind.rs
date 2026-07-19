use std::ops::Range;

use velum_regexp::{CompileLimits, ExecutionLimits, Flags, Regex};

type TestResult = Result<(), Box<dyn std::error::Error>>;

// Adapted from Test262 at commit 64ff467c0c1d60c077995bb7c5f93a9d8cc8ade1,
// test/built-ins/RegExp/lookBehind/*.js. Test262 is BSD-3-Clause licensed.

#[test]
fn lookbehind_captures_and_alternation_follow_reverse_order() -> TestResult {
    for (pattern, input, span, captures) in [
        (r"(?<=(c))def", "abcdef", 3..6, vec![Some(2..3)]),
        (r"(?<=(\w{2}))def", "abcdef", 3..6, vec![Some(1..3)]),
        (
            r"(?<=(\w(\w)))def",
            "abcdef",
            3..6,
            vec![Some(1..3), Some(2..3)],
        ),
        (r"(?<=(\w){3})def", "abcdef", 3..6, vec![Some(0..1)]),
        (r"(?<=(bc)|(cd)).", "abcdef", 3..4, vec![Some(1..3), None]),
        (
            r"(?<=([ab]{1,2})\D|(abc))\w",
            "abcdef",
            2..3,
            vec![Some(0..1), None],
        ),
        (
            r"\D(?<=([ab]+))(\w)",
            "abcdef",
            0..2,
            vec![Some(0..1), Some(1..2)],
        ),
        (
            r".*(?<=(..|...|....))(.*)",
            "xabcd",
            0..5,
            vec![Some(3..5), Some(5..5)],
        ),
        (
            r".*(?<=(xx|...|....))(.*)",
            "xabcd",
            0..5,
            vec![Some(2..5), Some(5..5)],
        ),
        (
            r".*(?<=(xx|...))(.*)",
            "xxabcd",
            0..6,
            vec![Some(3..6), Some(6..6)],
        ),
        (
            r".*(?<=(xx|xxx))(.*)",
            "xxabcd",
            0..6,
            vec![Some(0..2), Some(2..6)],
        ),
    ] {
        assert_match(pattern, input, Flags::default(), Some(&span), &captures)?;
    }
    assert_match(
        r"(?<!(^|[ab]))\w{2}",
        "abcdef",
        Flags::default(),
        Some(&(3..5)),
        &[None],
    )
}

#[test]
fn lookbehind_backreferences_follow_captured_utf16_ranges() -> TestResult {
    for (pattern, input, flags, span, captures) in [
        (
            r"(.)(?<=(\1\1))",
            "abb",
            Flags::default(),
            2..3,
            vec![Some(2..3), Some(1..3)],
        ),
        (
            r"(.)(?<=(\1\1))",
            "abB",
            Flags::default().with_ignore_case(true),
            2..3,
            vec![Some(2..3), Some(1..3)],
        ),
        (
            r"((\w)\w)(?<=\1\2\1)",
            "aabAaBa",
            Flags::default().with_ignore_case(true),
            4..6,
            vec![Some(4..6), Some(4..5)],
        ),
        (
            r"(\w(\w))(?<=\1\2\1)",
            "aabAaBa",
            Flags::default().with_ignore_case(true),
            5..7,
            vec![Some(5..7), Some(6..7)],
        ),
        (
            r"(?=(\w))(?<=(\1)).",
            "abaBbAa",
            Flags::default().with_ignore_case(true),
            4..5,
            vec![Some(4..5), Some(3..4)],
        ),
        (
            r"(?<=(.))(\w+)(?=\1)",
            "  'foo'  ",
            Flags::default(),
            3..6,
            vec![Some(2..3), Some(3..6)],
        ),
        (
            r"(.)(?<=\1\1\1)",
            "abbb",
            Flags::default(),
            3..4,
            vec![Some(3..4)],
        ),
        (
            r"(..)(?<=\1\1\1)",
            "fababab",
            Flags::default(),
            5..7,
            vec![Some(5..7)],
        ),
    ] {
        assert_match(pattern, input, flags, Some(&span), &captures)?;
    }
    for (pattern, input) in [
        (r"(?<=(.))(\w+)(?=\1)", "  .foo\"  "),
        (r"(.)(?<=\1\1\1)", "ab"),
        (r"(.)(?<=\1\1\1)", "abb"),
        (r"(..)(?<=\1\1\1)", "aabb"),
        (r"(..)(?<=\1\1\1)", "abab"),
        (r"(..)(?<=\1\1\1)", "fabxbab"),
        (r"(?<=([abc]+)).\1", "abcdbc"),
    ] {
        assert_match(pattern, input, Flags::default(), None, &[])?;
    }
    Ok(())
}

#[test]
fn lookbehind_greedy_and_nested_assertions_are_atomic() -> TestResult {
    for (pattern, input, span, captures) in [
        (r"(?<=(b+))c", "abbbbbbc", 7..8, vec![Some(1..7)]),
        (r"(?<=(b\d+))c", "ab1234c", 6..7, vec![Some(1..6)]),
        (
            r"(?<=((?:b\d{2})+))c",
            "ab12b23b34c",
            10..11,
            vec![Some(1..10)],
        ),
        (r"(?<=ab(?=c)\wd)\w\w", "abcdef", 4..6, vec![]),
        (
            r"(?<=a(?=([^a]{2})d)\w{3})\w\w",
            "abcdef",
            4..6,
            vec![Some(1..3)],
        ),
        (
            r"(?<=a(?=([bc]{2}(?<!a{2}))d)\w{3})\w\w",
            "abcdef",
            4..6,
            vec![Some(1..3)],
        ),
        (r"^faaao?(?<=^f[oa]+(?=o))", "faaao", 0..4, vec![]),
    ] {
        assert_match(pattern, input, Flags::default(), Some(&span), &captures)?;
    }
    assert_match(
        r"(?<=a(?=([bc]{2}(?<!a*))d)\w{3})\w\w",
        "abcdef",
        Flags::default(),
        None,
        &[],
    )
}

#[test]
fn lookbehind_variable_length_and_boundaries_match_test262() -> TestResult {
    for (pattern, input, span) in [
        (r"(?<=[a|b|c]*)[^a|b|c]{3}", "abcdef", 3..6),
        (r"(?<=\w*)[^a|b|c]{3}", "abcdef", 3..6),
        (r"(?<=\b)[d-f]{3}", "abc def", 4..7),
        (r"(?<=\B)\w{3}", "ab cdef", 4..7),
        (r"(?<=\B)(?<=c(?<=\w))\w{3}", "ab cdef", 4..7),
    ] {
        assert_match(pattern, input, Flags::default(), Some(&span), &[])?;
    }
    assert_match(r"(?<=\b)[d-f]{3}", "abcdef", Flags::default(), None, &[])
}

fn assert_match(
    pattern: &str,
    input: &str,
    flags: Flags,
    expected_span: Option<&Range<usize>>,
    expected_captures: &[Option<Range<usize>>],
) -> TestResult {
    let regex = Regex::compile(
        &pattern.encode_utf16().collect::<Vec<_>>(),
        flags,
        CompileLimits::default(),
    )?;
    let matched = regex
        .find(
            &input.encode_utf16().collect::<Vec<_>>(),
            0,
            ExecutionLimits::default(),
        )?
        .matched;
    let actual_span = matched.as_ref().map(|value| value.span.clone());
    let actual_captures = matched
        .map(|value| {
            value
                .captures
                .into_iter()
                .map(|capture| capture.span)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if actual_span.as_ref() == expected_span
        && (expected_span.is_none() || actual_captures == expected_captures)
    {
        return Ok(());
    }
    Err(format!(
        "Test262 lookbehind mismatch for /{pattern}/ on {input:?}: span={actual_span:?}, captures={actual_captures:?}, expected_span={expected_span:?}, expected_captures={expected_captures:?}"
    )
    .into())
}
