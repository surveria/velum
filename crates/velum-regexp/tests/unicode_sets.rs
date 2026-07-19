use velum_regexp::{
    CompileErrorKind, CompileLimits, ExecutionError, ExecutionLimits, Flags, Regex,
    unicode_string_property,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn compile(pattern: &str, limits: CompileLimits) -> Result<Regex, velum_regexp::CompileError> {
    Regex::compile(
        &pattern.encode_utf16().collect::<Vec<_>>(),
        Flags::default().with_unicode_sets(true),
        limits,
    )
}

fn full_match(pattern: &str, input: &str) -> TestResult {
    let regex = compile(pattern, CompileLimits::default())?;
    let units = input.encode_utf16().collect::<Vec<_>>();
    let matched = regex
        .find(&units, 0, ExecutionLimits::default())?
        .matched
        .ok_or_else(|| format!("expected {pattern:?} to match {input:?}"))?;
    if matched.span == (0..units.len()) {
        return Ok(());
    }
    Err(format!("unexpected string-property span: {matched:?}").into())
}

#[test]
fn generated_string_property_view_exposes_exact_sequences() -> TestResult {
    let property = unicode_string_property("RGI_Emoji_ZWJ_Sequence")
        .ok_or("RGI_Emoji_ZWJ_Sequence was not generated")?;
    let family = [0x1F468, 0x200D, 0x1F466];
    let mut found = false;
    for index in 0..property.sequence_count() {
        if property.sequence(index) == Some(family.as_slice()) {
            found = true;
            break;
        }
    }
    if found && unicode_string_property("rgi_emoji").is_none() {
        return Ok(());
    }
    Err("packed Unicode string property lookup was incorrect".into())
}

#[test]
fn string_properties_match_single_and_multi_code_point_emoji() -> TestResult {
    full_match(r"^\p{Basic_Emoji}$", "©️")?;
    full_match(r"^\p{Basic_Emoji}$", "😀")?;
    full_match(r"^\p{RGI_Emoji_ZWJ_Sequence}$", "👨‍👦")?;
    full_match(r"^[\p{RGI_Emoji_ZWJ_Sequence}]$", "👨‍👦")?;

    let basic = compile(r"^\p{Basic_Emoji}$", CompileLimits::default())?;
    let copyright = "©".encode_utf16().collect::<Vec<_>>();
    if basic
        .find(&copyright, 0, ExecutionLimits::default())?
        .matched
        .is_none()
    {
        return Ok(());
    }
    Err("Basic_Emoji incorrectly accepted a text presentation prefix".into())
}

#[test]
fn string_property_alternatives_backtrack_from_longest_match() -> TestResult {
    full_match(r"^\p{RGI_Emoji}🏻$", "👋🏻")?;
    full_match(r"^\p{RGI_Emoji}🏻$", "👋🏻🏻")
}

#[test]
fn string_properties_execute_in_reverse_lookbehind() -> TestResult {
    let regex = compile(
        r"(?<=\p{RGI_Emoji_ZWJ_Sequence})X",
        CompileLimits::default(),
    )?;
    let input = "👨‍👦X".encode_utf16().collect::<Vec<_>>();
    let matched = regex
        .find(&input, 0, ExecutionLimits::default())?
        .matched
        .ok_or("expected a string-property lookbehind match")?;
    if matched.span == (5..6) {
        return Ok(());
    }
    Err(format!("unexpected reverse string-property match: {matched:?}").into())
}

#[test]
fn string_property_complements_and_non_v_modes_are_rejected() -> TestResult {
    for pattern in [r"\P{RGI_Emoji}", r"[^\p{RGI_Emoji}]"] {
        let result = compile(pattern, CompileLimits::default());
        if !matches!(result, Err(ref error) if error.kind == CompileErrorKind::InvalidUnicodeProperty || error.kind == CompileErrorKind::InvalidCharacterClass)
        {
            return Err(format!("unexpected complement result for {pattern}: {result:?}").into());
        }
    }

    let unicode = Regex::compile(
        &r"\p{RGI_Emoji}".encode_utf16().collect::<Vec<_>>(),
        Flags::default().with_unicode(true),
        CompileLimits::default(),
    );
    if matches!(unicode, Err(ref error) if error.kind == CompileErrorKind::InvalidUnicodeProperty) {
        return Ok(());
    }
    Err(format!("string property was accepted outside v mode: {unicode:?}").into())
}

#[test]
fn string_property_compilation_and_execution_are_bounded() -> TestResult {
    let compile_result = compile(
        r"\p{RGI_Emoji}",
        CompileLimits {
            max_class_strings: 8,
            ..CompileLimits::default()
        },
    );
    if !matches!(
        compile_result,
        Err(velum_regexp::CompileError {
            kind: CompileErrorKind::ClassStringLimit { limit: 8 },
            ..
        })
    ) {
        return Err(format!("unexpected string compile limit: {compile_result:?}").into());
    }

    let regex = compile(r"\p{RGI_Emoji}", CompileLimits::default())?;
    let input = "👋".encode_utf16().collect::<Vec<_>>();
    let result = regex.find(
        &input,
        0,
        ExecutionLimits {
            max_steps: 1,
            ..ExecutionLimits::default()
        },
    );
    if matches!(result, Err(ExecutionError::StepLimit { limit: 1 })) {
        return Ok(());
    }
    Err(format!("unexpected string execution limit: {result:?}").into())
}

#[test]
fn unicode_set_union_intersection_and_subtraction_match_code_points() -> TestResult {
    full_match(r"^[a-cx]+$", "abcx")?;
    full_match(r"^[\p{ASCII}&&\p{Letter}]+$", "Velum")?;
    full_match(r"^[[a-z]--[aeiou]]+$", "bcdf")?;
    full_match(r"^[\p{Script=Greek}&&\p{Letter}]+$", "λΩ")?;

    let consonants = compile(r"^[[a-z]--[aeiou]]+$", CompileLimits::default())?;
    let input = "safe".encode_utf16().collect::<Vec<_>>();
    if consonants
        .find(&input, 0, ExecutionLimits::default())?
        .matched
        .is_none()
    {
        return Ok(());
    }
    Err("Unicode set subtraction retained a removed vowel".into())
}

#[test]
fn q_disjunction_supports_strings_empty_values_and_backtracking() -> TestResult {
    full_match(r"^[\q{ab|a}]b$", "ab")?;
    full_match(r"^[x\q{ab}]$", "ab")?;
    full_match(r"^[\q{|a}]$", "")?;
    full_match(r"^[\q{|a}]$", "a")
}

#[test]
fn set_operations_apply_to_q_string_members() -> TestResult {
    full_match(r"^[\q{ab|cd}&&\q{cd|ef}]$", "cd")?;
    full_match(r"^[\q{ab|cd}--\q{cd|ef}]$", "ab")?;

    let intersection = compile(r"^[\q{ab|cd}&&\q{cd|ef}]$", CompileLimits::default())?;
    let input = "ab".encode_utf16().collect::<Vec<_>>();
    if intersection
        .find(&input, 0, ExecutionLimits::default())?
        .matched
        .is_none()
    {
        return Ok(());
    }
    Err("Unicode string-set intersection retained a non-member".into())
}

#[test]
fn singleton_strings_share_the_code_point_set_domain() -> TestResult {
    full_match(r"^[\p{Basic_Emoji}&&[😀]]$", "😀")?;
    full_match(r"^[\q{a|ab}&&[a]]$", "a")?;
    full_match(r"^[\q{a|ab}--[a]]$", "ab")?;

    for (pattern, input) in [
        (r"^[[a]--\q{a}]$", "a"),
        (r"^[\p{Basic_Emoji}--[😀]]$", "😀"),
    ] {
        let regex = compile(pattern, CompileLimits::default())?;
        let units = input.encode_utf16().collect::<Vec<_>>();
        if regex
            .find(&units, 0, ExecutionLimits::default())?
            .matched
            .is_some()
        {
            return Err(format!("cross-domain subtraction retained {input:?}").into());
        }
    }
    Ok(())
}

#[test]
fn nested_complements_and_escaped_set_punctuators_are_supported() -> TestResult {
    full_match(r"^[^[a-z]]+$", "123")?;
    full_match(r"^[\-\&\/\(\)]+$", "-&/()")
}

#[test]
fn unicode_set_operator_and_reserved_punctuator_errors_are_structured() -> TestResult {
    for pattern in [
        "[a&&bc]",
        "[ab--b]",
        "[a&&b--c]",
        "[a-]",
        "[a!!b]",
        r"[\q{a&&b}]",
    ] {
        let result = compile(pattern, CompileLimits::default());
        if !matches!(result, Err(ref error) if error.kind == CompileErrorKind::InvalidCharacterClass)
        {
            return Err(format!("unexpected Unicode set error for {pattern}: {result:?}").into());
        }
    }
    Ok(())
}

#[test]
fn unicode_set_expression_depth_and_retained_memory_are_bounded() -> TestResult {
    let limited = compile(
        "[a&&a&&a]",
        CompileLimits {
            max_nesting_depth: 2,
            ..CompileLimits::default()
        },
    );
    if !matches!(
        limited,
        Err(ref error)
            if error.kind == CompileErrorKind::NestingLimit { limit: 2 }
    ) {
        return Err(format!("unexpected set expression depth result: {limited:?}").into());
    }

    let simple = compile("[a]", CompileLimits::default())?;
    let nested = compile("[a&&b]", CompileLimits::default())?;
    if nested.retained_payload_bytes()? > simple.retained_payload_bytes()? {
        let expensive = compile("[a&&a]", CompileLimits::default())?;
        let input = "a".encode_utf16().collect::<Vec<_>>();
        let execution = expensive.find(
            &input,
            0,
            ExecutionLimits {
                max_steps: 3,
                ..ExecutionLimits::default()
            },
        );
        if !matches!(execution, Err(ExecutionError::StepLimit { limit: 3 })) {
            return Err(format!("set evaluation work was not charged: {execution:?}").into());
        }
        for (pattern, limits, expected) in [
            (
                r"[\q{a|b}]",
                CompileLimits {
                    max_class_strings: 1,
                    ..CompileLimits::default()
                },
                CompileErrorKind::ClassStringLimit { limit: 1 },
            ),
            (
                r"[\q{abc}]",
                CompileLimits {
                    max_class_string_units: 2,
                    ..CompileLimits::default()
                },
                CompileErrorKind::ClassStringUnitLimit { limit: 2 },
            ),
            (
                "[ab]",
                CompileLimits {
                    max_character_class_terms: 1,
                    ..CompileLimits::default()
                },
                CompileErrorKind::NodeLimit { limit: 1 },
            ),
        ] {
            let result = compile(pattern, limits);
            if !matches!(result, Err(ref error) if error.kind == expected) {
                return Err(
                    format!("unexpected set resource result for {pattern}: {result:?}").into(),
                );
            }
        }
        return Ok(());
    }
    Err("nested Unicode set allocations were omitted from retained memory".into())
}

#[test]
fn unicode_set_ignore_case_is_applied_before_set_operations() -> TestResult {
    let flags = Flags::default()
        .with_unicode_sets(true)
        .with_ignore_case(true);
    let regex = Regex::compile(
        &r"^[a&&A]$".encode_utf16().collect::<Vec<_>>(),
        flags,
        CompileLimits::default(),
    )?;
    for input in ["a", "A"] {
        let units = input.encode_utf16().collect::<Vec<_>>();
        if regex
            .find(&units, 0, ExecutionLimits::default())?
            .matched
            .is_none()
        {
            return Err(format!("ignore-case set intersection rejected {input}").into());
        }
    }
    Ok(())
}
