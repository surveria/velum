use std::ops::Range;

use regress::{Flags as RegressFlags, Regex as RegressRegex};
use velum_regexp::{
    CompileLimits, ExecutionLimits, Flags as NativeFlags, NoopExecutionControl,
    Regex as NativeRegex,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Debug, Default, Eq, PartialEq)]
struct MatchOutcome {
    span: Option<Range<usize>>,
    captures: Vec<Option<Range<usize>>>,
    named_captures: Vec<(String, Option<Range<usize>>)>,
}

#[derive(Clone, Copy)]
struct Case {
    pattern: &'static str,
    flags: &'static str,
    input: &'static str,
    start: usize,
    sticky: bool,
}

#[test]
fn curated_semantics_match_the_current_backend() -> TestResult {
    for case in [
        Case {
            pattern: "ab+",
            flags: "",
            input: "zabbb!",
            start: 0,
            sticky: false,
        },
        Case {
            pattern: "^(a|bc)+$",
            flags: "",
            input: "abca",
            start: 0,
            sticky: false,
        },
        Case {
            pattern: r"(?<word>a+)b\k<word>",
            flags: "",
            input: "zaabaa!",
            start: 0,
            sticky: false,
        },
        Case {
            pattern: r"(?=(a+))a+b\1",
            flags: "",
            input: "zaaabaaa!",
            start: 0,
            sticky: false,
        },
        Case {
            pattern: r"(?<=a)b|(?<!a)c",
            flags: "",
            input: "zabc",
            start: 0,
            sticky: false,
        },
        Case {
            pattern: r"\bcat\B",
            flags: "i",
            input: "CATS",
            start: 0,
            sticky: false,
        },
        Case {
            pattern: r"^[\p{Alphabetic}--\p{ASCII}]+$",
            flags: "v",
            input: "ЖΩ",
            start: 0,
            sticky: false,
        },
        Case {
            pattern: r"^.$",
            flags: "u",
            input: "🐸",
            start: 0,
            sticky: false,
        },
        Case {
            pattern: r"(?i:a)b",
            flags: "",
            input: "zAb",
            start: 1,
            sticky: true,
        },
        Case {
            pattern: "^.$",
            flags: "s",
            input: "\n",
            start: 0,
            sticky: false,
        },
    ] {
        compare_case(case)?;
    }
    Ok(())
}

#[test]
fn structured_short_patterns_match_the_current_backend() -> TestResult {
    let inputs = ["", "a", "b", "ab", "ba", "aa", "abc", "1a", "_", "\n"];
    for flags in ["", "i", "m", "s"] {
        for atom in ["a", "b", ".", "[ab]", "[^c]", r"\d", r"\w"] {
            for quantifier in ["", "?", "+", "{1,2}"] {
                let pattern = format!("{atom}{quantifier}");
                for input in inputs {
                    compare_owned_case(&pattern, flags, input, 0, false)?;
                    compare_owned_case(&pattern, flags, input, 0, true)?;
                }
            }
        }
    }
    Ok(())
}

#[test]
fn syntax_acceptance_matches_for_shared_grammar_cases() -> TestResult {
    for (pattern, flags) in [
        ("(", ""),
        ("[a", ""),
        ("a**", ""),
        ("(?<1>x)", "u"),
        (r"\p{not_a_property}", "u"),
        ("a{2,1}", ""),
        ("a{", "u"),
        ("]", "u"),
        (r"\a", "u"),
    ] {
        let pattern_units = pattern.encode_utf16().collect::<Vec<_>>();
        let native = NativeRegex::compile(
            &pattern_units,
            native_flags(flags),
            CompileLimits::default(),
        );
        let regress = compile_regress(&pattern_units, flags);
        if native.is_ok() != regress.is_ok() {
            return Err(format!(
                "syntax acceptance differed for /{pattern}/{flags}: native={native:?}, regress={regress:?}"
            )
            .into());
        }
    }
    Ok(())
}

fn compare_case(case: Case) -> TestResult {
    compare_owned_case(
        case.pattern,
        case.flags,
        case.input,
        case.start,
        case.sticky,
    )
}

fn compare_owned_case(
    pattern: &str,
    flags: &str,
    input: &str,
    start: usize,
    sticky: bool,
) -> TestResult {
    let pattern_units = pattern.encode_utf16().collect::<Vec<_>>();
    let input_units = input.encode_utf16().collect::<Vec<_>>();
    let native = native_outcome(&pattern_units, flags, &input_units, start, sticky)?;
    let regress = regress_outcome(&pattern_units, flags, &input_units, start, sticky)?;
    if native == regress {
        return Ok(());
    }
    Err(format!(
        "backend mismatch for /{pattern}/{flags} on {input:?} at {start}, sticky={sticky}: native={native:?}, regress={regress:?}"
    )
    .into())
}

fn native_outcome(
    pattern: &[u16],
    flags: &str,
    input: &[u16],
    start: usize,
    sticky: bool,
) -> Result<MatchOutcome, Box<dyn std::error::Error>> {
    let regex = NativeRegex::compile(pattern, native_flags(flags), CompileLimits::default())?;
    let matched = regex
        .find_with_control(
            input,
            start,
            sticky,
            ExecutionLimits::default(),
            &mut NoopExecutionControl,
        )?
        .matched;
    let Some(matched) = matched else {
        return Ok(MatchOutcome::default());
    };
    let captures = matched
        .captures
        .into_iter()
        .map(|capture| capture.span)
        .collect::<Vec<_>>();
    let mut named_captures = Vec::new();
    for index in 0..regex.capture_count() {
        let Some(name) = regex.capture_name(index) else {
            continue;
        };
        let span = captures.get(index).cloned().flatten();
        named_captures.push((name.to_owned(), span));
    }
    named_captures.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(MatchOutcome {
        span: Some(matched.span),
        captures,
        named_captures,
    })
}

fn regress_outcome(
    pattern: &[u16],
    flags: &str,
    input: &[u16],
    start: usize,
    sticky: bool,
) -> Result<MatchOutcome, Box<dyn std::error::Error>> {
    let regex = compile_regress(pattern, flags)?;
    let matched = if has_unicode_mode(flags) {
        regex.find_from_utf16(input, start).next()
    } else {
        regex.find_from_ucs2(input, start).next()
    };
    let Some(matched) = matched.filter(|matched| !sticky || matched.start() == start) else {
        return Ok(MatchOutcome::default());
    };
    let mut named_captures = matched
        .named_groups()
        .map(|(name, span)| (name.to_owned(), span))
        .collect::<Vec<_>>();
    named_captures.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(MatchOutcome {
        span: Some(matched.range()),
        captures: matched.captures,
        named_captures,
    })
}

fn compile_regress(pattern: &[u16], flags: &str) -> Result<RegressRegex, regress::Error> {
    let pattern_values = if has_unicode_mode(flags) {
        char::decode_utf16(pattern.iter().copied())
            .map(|value| {
                value.map_or_else(|error| u32::from(error.unpaired_surrogate()), u32::from)
            })
            .collect::<Vec<_>>()
    } else {
        pattern.iter().copied().map(u32::from).collect()
    };
    RegressRegex::from_unicode(pattern_values.into_iter(), regress_flags(flags))
}

fn regress_flags(flags: &str) -> RegressFlags {
    RegressFlags {
        icase: flags.contains('i'),
        multiline: flags.contains('m'),
        dot_all: flags.contains('s'),
        no_opt: false,
        unicode: flags.contains('u'),
        unicode_sets: flags.contains('v'),
    }
}

fn native_flags(flags: &str) -> NativeFlags {
    NativeFlags::default()
        .with_ignore_case(flags.contains('i'))
        .with_multiline(flags.contains('m'))
        .with_dot_all(flags.contains('s'))
        .with_unicode(flags.contains('u'))
        .with_unicode_sets(flags.contains('v'))
}

fn has_unicode_mode(flags: &str) -> bool {
    flags.contains('u') || flags.contains('v')
}
