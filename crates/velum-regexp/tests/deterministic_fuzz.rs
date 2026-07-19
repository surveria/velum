use core::ops::Range;

use velum_regexp::{
    CompileLimits, ExecutionControl, ExecutionError, ExecutionLimits, Flags, InterruptReason,
    Regex, SearchOutcome,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const CASE_COUNT: usize = 10_000;
const MIN_RAW_COMPILED: usize = 100;
const MIN_STRUCTURED_COMPILED: usize = 500;
const RAW_SEED: u64 = 0x52A7_19D4_C3E8_6B01;
const STRUCTURED_SEED: u64 = 0xEC5A_2026_0719_0042;

const RAW_PATTERN_UNITS: &[u16] = &[
    0x0000, 0x000A, 0x0028, 0x0029, 0x002A, 0x002B, 0x002D, 0x002E, 0x0030, 0x0031, 0x003F, 0x0041,
    0x005B, 0x005C, 0x005D, 0x005E, 0x0061, 0x0062, 0x0063, 0x007B, 0x007C, 0x007D, 0x00DF, 0x017F,
    0x2028, 0xD7FF, 0xD800, 0xDBFF, 0xDC00, 0xDFFF, 0xE000, 0xFFFF,
];

const STRUCTURED_FRAGMENTS: &[&str] = &[
    "a",
    "b",
    ".",
    "|",
    "(?:",
    "(?=",
    "(?!",
    "(?<=",
    "(?<!",
    "(?<name>",
    ")",
    "[a-z]",
    "[^a]",
    "[\\w--a]",
    "\\d",
    "\\w",
    "\\s",
    "\\b",
    "\\1",
    "\\k<name>",
    "\\p{ASCII}",
    "\\p{RGI_Emoji}",
    "\\q{ab|a}",
    "*",
    "+",
    "?",
    "{0,2}",
    "{1,}",
    "(?i:",
    "(?s-m:",
    "^",
    "$",
];

#[derive(Clone, Copy)]
struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
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

    fn below(&mut self, upper: usize) -> usize {
        if upper == 0 {
            return 0;
        }
        let upper = u64::try_from(upper).unwrap_or(u64::MAX);
        usize::try_from(self.next() % upper).unwrap_or_default()
    }

    const fn boolean(&mut self) -> bool {
        self.next() & 1 != 0
    }

    fn raw_unit(&mut self) -> u16 {
        if self.below(4) == 0 {
            return u16::try_from(self.next() & u64::from(u16::MAX)).unwrap_or_default();
        }
        RAW_PATTERN_UNITS
            .get(self.below(RAW_PATTERN_UNITS.len()))
            .copied()
            .unwrap_or_default()
    }
}

#[derive(Default)]
struct CountingControl {
    charged: usize,
}

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

impl ExecutionControl for CountingControl {
    fn charge_steps(&mut self, steps: usize) -> Result<(), InterruptReason> {
        self.charged = self
            .charged
            .checked_add(steps)
            .ok_or(InterruptReason::HostStepLimit)?;
        Ok(())
    }
}

#[test]
fn arbitrary_utf16_parser_and_executor_are_bounded_and_deterministic() -> TestResult {
    let mut rng = DeterministicRng::new(RAW_SEED);
    let mut compiled = 0_usize;
    for case_id in 0..CASE_COUNT {
        let pattern = raw_units(&mut rng, 65);
        let input = raw_units(&mut rng, 97);
        if exercise_case(case_id, RAW_SEED, &pattern, &input, &mut rng)? {
            compiled = compiled
                .checked_add(1)
                .ok_or("raw compiled-case count overflowed")?;
        }
    }
    ensure_compile_coverage("raw", RAW_SEED, compiled, MIN_RAW_COMPILED)
}

#[test]
fn structured_pattern_mutations_are_bounded_and_deterministic() -> TestResult {
    let mut rng = DeterministicRng::new(STRUCTURED_SEED);
    let mut compiled = 0_usize;
    for case_id in 0..CASE_COUNT {
        let pattern = structured_pattern(&mut rng);
        let input = raw_units(&mut rng, 97);
        if exercise_case(case_id, STRUCTURED_SEED, &pattern, &input, &mut rng)? {
            compiled = compiled
                .checked_add(1)
                .ok_or("structured compiled-case count overflowed")?;
        }
    }
    ensure_compile_coverage(
        "structured",
        STRUCTURED_SEED,
        compiled,
        MIN_STRUCTURED_COMPILED,
    )
}

fn raw_units(rng: &mut DeterministicRng, length_bound: usize) -> Vec<u16> {
    let length = rng.below(length_bound);
    (0..length).map(|_| rng.raw_unit()).collect()
}

fn structured_pattern(rng: &mut DeterministicRng) -> Vec<u16> {
    let count = rng.below(9);
    let mut pattern = String::new();
    for _ in 0..count {
        if let Some(fragment) = STRUCTURED_FRAGMENTS.get(rng.below(STRUCTURED_FRAGMENTS.len())) {
            pattern.push_str(fragment);
        }
    }
    pattern.encode_utf16().collect()
}

fn exercise_case(
    case_id: usize,
    seed: u64,
    pattern: &[u16],
    input: &[u16],
    rng: &mut DeterministicRng,
) -> Result<bool, Box<dyn std::error::Error>> {
    let flags = generated_flags(rng);
    let regex = match Regex::compile(pattern, flags, compile_limits()) {
        Ok(regex) => regex,
        Err(error) => {
            if error.pattern_offset > pattern.len() {
                let detail = format!("compile error escaped the pattern: {error:?}");
                return Err(case_error(case_id, seed, pattern, input, &detail));
            }
            return Ok(false);
        }
    };
    let _retained_bytes = regex.retained_payload_bytes()?;
    let start_bound = input.len().saturating_add(3);
    let start = rng.below(start_bound);
    let anchored = rng.boolean();
    let limits = execution_limits();
    let mut first_control = CountingControl::default();
    let first = regex.find_with_control(input, start, anchored, limits, &mut first_control);
    let mut second_control = CountingControl::default();
    let second = regex.find_with_control(input, start, anchored, limits, &mut second_control);
    if first != second || first_control.charged != second_control.charged {
        return Err(case_error(
            case_id,
            seed,
            pattern,
            input,
            "execution was not deterministic",
        ));
    }
    match first {
        Ok(outcome) => {
            if start > input.len() {
                return Err(case_error(
                    case_id,
                    seed,
                    pattern,
                    input,
                    "out-of-bounds start unexpectedly executed",
                ));
            }
            validate_outcome(
                &outcome,
                input.len(),
                start,
                anchored,
                limits,
                first_control.charged,
            )
            .map_err(|detail| case_error(case_id, seed, pattern, input, &detail))?;
        }
        Err(ExecutionError::InvalidProgram | ExecutionError::SizeOverflow) => {
            return Err(case_error(
                case_id,
                seed,
                pattern,
                input,
                "a compiled program failed an internal execution invariant",
            ));
        }
        Err(ExecutionError::StartOutOfBounds) if start <= input.len() => {
            return Err(case_error(
                case_id,
                seed,
                pattern,
                input,
                "an in-bounds start was rejected",
            ));
        }
        Err(_) => {}
    }
    if case_id.is_multiple_of(4) {
        let budget = rng.below(33);
        let mut first_interrupt = InterruptAfter { remaining: budget };
        let interrupted =
            regex.find_with_control(input, start, anchored, limits, &mut first_interrupt);
        let mut second_interrupt = InterruptAfter { remaining: budget };
        let repeated =
            regex.find_with_control(input, start, anchored, limits, &mut second_interrupt);
        if interrupted != repeated || first_interrupt.remaining != second_interrupt.remaining {
            return Err(case_error(
                case_id,
                seed,
                pattern,
                input,
                "host interruption was not deterministic",
            ));
        }
    }
    Ok(true)
}

fn generated_flags(rng: &mut DeterministicRng) -> Flags {
    let mut flags = Flags::default()
        .with_ignore_case(rng.boolean())
        .with_multiline(rng.boolean())
        .with_dot_all(rng.boolean());
    match rng.below(3) {
        1 => flags = flags.with_unicode(true),
        2 => flags = flags.with_unicode_sets(true),
        _ => {}
    }
    flags
}

const fn compile_limits() -> CompileLimits {
    CompileLimits {
        max_pattern_units: 64,
        max_nesting_depth: 16,
        max_nodes: 128,
        max_instructions: 256,
        max_captures: 16,
        max_capture_name_units: 32,
        max_character_class_terms: 128,
        max_class_strings: 64,
        max_class_string_units: 256,
        max_repeat_count: 1_000,
    }
}

const fn execution_limits() -> ExecutionLimits {
    ExecutionLimits {
        max_steps: 1_024,
        max_candidate_starts: 128,
        max_backtrack_frames: 128,
        max_undo_records: 256,
        max_capture_slots: 16,
    }
}

fn validate_outcome(
    outcome: &SearchOutcome,
    input_len: usize,
    start: usize,
    anchored: bool,
    limits: ExecutionLimits,
    charged: usize,
) -> Result<(), String> {
    if outcome.stats.steps != charged
        || outcome.stats.steps > limits.max_steps
        || outcome.stats.candidate_starts > limits.max_candidate_starts
        || outcome.stats.max_backtrack_depth > limits.max_backtrack_frames
        || outcome.stats.max_undo_depth > limits.max_undo_records
    {
        return Err(format!(
            "resource accounting escaped its limits: {outcome:?}"
        ));
    }
    let Some(matched) = &outcome.matched else {
        return Ok(());
    };
    if !valid_range(&matched.span, input_len)
        || matched.span.start < start
        || (anchored && matched.span.start != start)
        || matched.captures.len() > limits.max_capture_slots
        || matched
            .captures
            .iter()
            .filter_map(|capture| capture.span.as_ref())
            .any(|span| !valid_range(span, input_len))
    {
        return Err(format!("match coordinates escaped the input: {outcome:?}"));
    }
    Ok(())
}

const fn valid_range(range: &Range<usize>, input_len: usize) -> bool {
    range.start <= range.end && range.end <= input_len
}

fn ensure_compile_coverage(label: &str, seed: u64, compiled: usize, minimum: usize) -> TestResult {
    if compiled >= minimum {
        return Ok(());
    }
    Err(format!(
        "deterministic {label} fuzz seed {seed:#018x} compiled only {compiled} cases; expected at least {minimum}"
    )
    .into())
}

fn case_error(
    case_id: usize,
    seed: u64,
    pattern: &[u16],
    input: &[u16],
    detail: &str,
) -> Box<dyn std::error::Error> {
    format!(
        "deterministic fuzz case {case_id} seed {seed:#018x}: {detail}; pattern={pattern:?}; input={input:?}"
    )
    .into()
}
