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

#[test]
fn deterministic_structural_generation_matches_the_current_backend() -> TestResult {
    let mut generator = DeterministicGenerator::new(0xA5C3_91E7_42D8_6B1F);
    for case_id in 0..5_000_usize {
        let first = generator.choose(&["a", "b", ".", "[ab]", "[^b]", r"\d", r"\w"])?;
        let second = generator.choose(&["a", "c", ".", "[a-c]", r"\D", r"\W"])?;
        let quantifier = generator.choose(&["", "?", "*", "+", "{0,2}", "{1,3}?"])?;
        let pattern = match generator.next() % 10 {
            0 => format!("{first}{quantifier}"),
            1 => format!("(?:{first}{quantifier}){second}"),
            2 => format!("({first}|{second}){quantifier}"),
            3 => format!("^{first}{quantifier}{second}$"),
            4 => format!("(?={first}){first}{quantifier}"),
            5 => format!(r"({first}){second}\1"),
            6 => format!("(?<={first}){second}"),
            7 => format!("(?<!{first}){second}"),
            8 => format!("(?:{first}|{second}){quantifier}"),
            _ => format!("(?i:{first}){second}"),
        };
        let flags = generator.choose(&["", "i", "m", "s", "im", "is", "ms"])?;
        let input = generator.input()?;
        let input_length = input.encode_utf16().count();
        let start_modulus = input_length
            .checked_add(1)
            .ok_or("generated input start modulus overflowed")?;
        let start = usize::try_from(generator.next())? % start_modulus;
        let sticky = generator.next() & 1 != 0;
        compare_owned_case(&pattern, flags, &input, start, sticky).map_err(
            |error| -> Box<dyn std::error::Error> {
                format!("generated differential case {case_id} failed: {error}").into()
            },
        )?;
    }
    Ok(())
}

#[test]
fn deterministic_unicode_generation_matches_the_current_backend() -> TestResult {
    let mut generator = DeterministicGenerator::new(0x6D2F_B183_C947_AE51);
    for case_id in 0..2_000_usize {
        let flags = generator.choose(&["u", "iu", "v", "iv"])?;
        let first = generator.choose(&[
            ".",
            r"\p{Alphabetic}",
            r"\p{ASCII}",
            r"[\p{Letter}\d]",
            r"\u{1F438}",
            "[ЖΩ]",
        ])?;
        let second = generator.choose(&["a", "Ж", ".", r"\p{Number}", "[🐸a]"])?;
        let quantifier = generator.choose(&["", "?", "+", "{1,2}"])?;
        let pattern = match generator.next() % 6 {
            0 => format!("{first}{quantifier}"),
            1 => format!("^{first}{quantifier}$"),
            2 => format!("({first}){second}"),
            3 => format!("(?={first}){first}{quantifier}"),
            4 => format!("(?<={first}){second}"),
            _ if flags.contains('v') => r"^[\p{Alphabetic}--\p{ASCII}]+$".to_owned(),
            _ => format!("(?:{first}|{second}){quantifier}"),
        };
        let input = generator.unicode_input()?;
        let input_length = input.encode_utf16().count();
        let start_modulus = input_length
            .checked_add(1)
            .ok_or("generated Unicode input start modulus overflowed")?;
        let start = usize::try_from(generator.next())? % start_modulus;
        let sticky = generator.next() & 1 != 0;
        compare_owned_case(&pattern, flags, input, start, sticky).map_err(
            |error| -> Box<dyn std::error::Error> {
                format!("generated Unicode differential case {case_id} failed: {error}").into()
            },
        )?;
    }
    Ok(())
}

#[test]
fn exact_utf16_surrogate_cases_match_the_current_backend() -> TestResult {
    let high = [0xD800_u16];
    let low = [0xDC00_u16];
    let dot = [u16::from(b'.')];
    let frog = "🐸".encode_utf16().collect::<Vec<_>>();
    for (label, pattern, flags, input, start, sticky) in [
        ("legacy dot high", &dot[..], "", &high[..], 0, false),
        ("Unicode dot high", &dot[..], "u", &high[..], 0, false),
        ("Unicode dot low", &dot[..], "u", &low[..], 0, false),
        ("legacy literal high", &high[..], "", &high[..], 0, true),
        ("Unicode literal high", &high[..], "u", &high[..], 0, true),
        ("Unicode frog middle", &dot[..], "u", &frog[..], 1, true),
        ("legacy frog middle", &dot[..], "", &frog[..], 1, true),
    ] {
        compare_units(label, pattern, flags, input, start, sticky)?;
    }
    Ok(())
}

#[test]
fn nested_repetition_capture_rollback_matches_the_current_backend() -> TestResult {
    let patterns = [
        "(a|b)*",
        "((a)|(b))*",
        "((a)?b)*",
        "(a*)*",
        "(?:a?)*",
        "(a|aa)*b",
        r"(a+)?b\1",
        r"(a|(b))*\2",
        r"(?=(a*))\1",
        r"(?!(a))b\1",
        "((a|b)+?)c",
        "(a(b)?)+",
        "(a|ab)+b",
        "((ab)*)*",
        "(?:(a)|b)+",
        r"(a*)(b*)\2\1",
        r"(?=(a+))a*b\1",
        "(?<=(a|ab))b",
    ];
    let inputs = [
        "", "a", "b", "c", "aa", "ab", "ba", "bb", "aaa", "aab", "aba", "abb", "bab", "bba", "abc",
    ];
    for pattern in patterns {
        for flags in ["", "i"] {
            for input in inputs {
                let input_length = input.encode_utf16().count();
                for start in 0..=input_length {
                    compare_owned_case(pattern, flags, input, start, false)?;
                    compare_owned_case(pattern, flags, input, start, true)?;
                }
            }
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
    compare_units(pattern, &pattern_units, flags, &input_units, start, sticky)
}

fn compare_units(
    label: &str,
    pattern: &[u16],
    flags: &str,
    input: &[u16],
    start: usize,
    sticky: bool,
) -> TestResult {
    let native = native_outcome(pattern, flags, input, start, sticky)?;
    let regress = regress_outcome(pattern, flags, input, start, sticky)?;
    if native == regress {
        return Ok(());
    }
    Err(format!(
        "backend mismatch for {label} /{pattern:?}/{flags} on {input:?} at {start}, sticky={sticky}: native={native:?}, regress={regress:?}"
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

struct DeterministicGenerator {
    state: u64,
}

impl DeterministicGenerator {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    const fn next(&mut self) -> u64 {
        let mut value = self.state;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.state = value;
        value
    }

    fn choose<'a>(&mut self, values: &'a [&'a str]) -> Result<&'a str, &'static str> {
        let length = u64::try_from(values.len()).map_err(|_| "choice length exceeded u64")?;
        if length == 0 {
            return Err("cannot choose from an empty slice");
        }
        let index =
            usize::try_from(self.next() % length).map_err(|_| "choice index exceeded usize")?;
        values.get(index).copied().ok_or("choice index was missing")
    }

    fn input(&mut self) -> Result<String, &'static str> {
        let length = usize::try_from(self.next() % 7)
            .map_err(|_| "generated input length exceeded usize")?;
        let mut input = String::new();
        for _ in 0..length {
            input.push_str(self.choose(&["a", "b", "c", "0", "_", "\n"])?);
        }
        Ok(input)
    }

    fn unicode_input(&mut self) -> Result<&'static str, &'static str> {
        self.choose(&["", "a", "Ж", "Ω", "🐸", "Жa", "🐸a", "１２", "\n"])
    }
}
