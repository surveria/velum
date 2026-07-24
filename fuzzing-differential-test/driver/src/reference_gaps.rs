use crate::compare::{EngineOutcome, OutcomeStatus};

const RESIZABLE_ARRAY_BUFFER_MARKER: &str = "maxByteLength";
const RESOURCE_MANAGEMENT_KEYWORD: &str = "using";
const RESOURCE_FOR_OF_KEYWORD: &str = "of";
const SYMBOL_DISPOSE_ACCESS: &str = "Symbol.dispose";
const SYMBOL_ASYNC_DISPOSE_ACCESS: &str = "Symbol.asyncDispose";
const SYNTAX_ERROR_NAME: &str = "SyntaxError";
const ANNEX_B_STRING_HTML_METHODS: [&str; 13] = [
    "anchor",
    "big",
    "blink",
    "bold",
    "fixed",
    "fontcolor",
    "fontsize",
    "italics",
    "link",
    "small",
    "strike",
    "sub",
    "sup",
];

pub fn outcomes_equivalent(left: &EngineOutcome, right: &EngineOutcome) -> bool {
    if left.status != right.status {
        return false;
    }
    match left.status {
        OutcomeStatus::Ok => left.stdout_sha256 == right.stdout_sha256,
        OutcomeStatus::JsError => left.error_name == right.error_name,
        OutcomeStatus::Timeout | OutcomeStatus::Crash => true,
    }
}

pub fn is_engine262_unsupported(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    is_engine262_missing_global(engine262)
        || (source.contains(RESIZABLE_ARRAY_BUFFER_MARKER)
            && !outcomes_equivalent(velum, engine262)
            && !outcomes_equivalent(engine262, v8))
        || is_reference_unsupported_resource_management_syntax(source, engine262, v8)
        || is_reference_unsupported_resource_management_symbols(source, velum, engine262, v8)
        || is_engine262_missing_annex_b_string_html_method(source, velum, engine262, v8)
        || (engine262.error_name.as_deref() == Some(SYNTAX_ERROR_NAME)
            && !outcomes_equivalent(velum, engine262)
            && !outcomes_equivalent(engine262, v8))
}

pub fn correctness_oracle<'a>(
    source: &str,
    engine262: &'a EngineOutcome,
    v8: &'a EngineOutcome,
    engine262_unsupported: bool,
) -> Option<&'a EngineOutcome> {
    if !engine262_unsupported {
        return Some(engine262);
    }
    if is_reference_unsupported_resource_management_syntax(source, engine262, v8)
        || source_contains_resource_management_symbol_access(source)
            && references_complete_equivalently(engine262, v8)
        || is_v8_fallback_unavailable(v8)
    {
        return None;
    }
    Some(v8)
}

fn is_engine262_missing_annex_b_string_html_method(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("TypeError")
        && outcomes_equivalent(velum, v8)
        && engine262.error_message.as_deref().is_some_and(|message| {
            message.contains("is not a function")
                && ANNEX_B_STRING_HTML_METHODS
                    .iter()
                    .any(|method| source_contains_method_call(source, method))
        })
}

fn source_contains_method_call(source: &str, method: &str) -> bool {
    let Some(pattern_len) = method.len().checked_add(2) else {
        return false;
    };
    source.as_bytes().windows(pattern_len).any(|window| {
        window.first() == Some(&b'.')
            && window.get(1..1 + method.len()) == Some(method.as_bytes())
            && window.last() == Some(&b'(')
    })
}

fn is_reference_unsupported_resource_management_syntax(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    references_reject_as_syntax(engine262, v8) && source_contains_resource_management_syntax(source)
}

fn is_reference_unsupported_resource_management_symbols(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source_contains_resource_management_symbol_access(source)
        && references_complete_equivalently(engine262, v8)
        && !outcomes_equivalent(velum, engine262)
}

fn references_reject_as_syntax(engine262: &EngineOutcome, v8: &EngineOutcome) -> bool {
    engine262.status == OutcomeStatus::JsError
        && v8.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some(SYNTAX_ERROR_NAME)
        && v8.error_name.as_deref() == Some(SYNTAX_ERROR_NAME)
}

fn references_complete_equivalently(engine262: &EngineOutcome, v8: &EngineOutcome) -> bool {
    engine262.is_completed() && v8.is_completed() && outcomes_equivalent(engine262, v8)
}

fn source_contains_resource_management_symbol_access(source: &str) -> bool {
    source.contains(SYMBOL_DISPOSE_ACCESS) || source.contains(SYMBOL_ASYNC_DISPOSE_ACCESS)
}

fn source_contains_resource_management_syntax(source: &str) -> bool {
    let mut search_start = 0;
    while let Some(relative_start) = source
        .get(search_start..)
        .and_then(|tail| tail.find(RESOURCE_MANAGEMENT_KEYWORD))
    {
        let start = search_start.saturating_add(relative_start);
        let end = start.saturating_add(RESOURCE_MANAGEMENT_KEYWORD.len());
        if is_keyword_boundary(source, start, end)
            && resource_binding_follows_using(source.get(end..).unwrap_or_default())
        {
            return true;
        }
        search_start = end;
    }
    false
}

fn is_keyword_boundary(source: &str, start: usize, end: usize) -> bool {
    let previous = source
        .get(..start)
        .and_then(|prefix| prefix.chars().next_back());
    let next = source.get(end..).and_then(|suffix| suffix.chars().next());
    !previous.is_some_and(is_ascii_identifier_part) && !next.is_some_and(is_ascii_identifier_part)
}

fn resource_binding_follows_using(rest: &str) -> bool {
    let tail = rest.trim_start();
    let Some(first) = tail.chars().next() else {
        return false;
    };
    if !is_ascii_identifier_start(first) {
        return false;
    }
    let name_end = tail
        .char_indices()
        .find_map(|(index, value)| (!is_ascii_identifier_part(value)).then_some(index))
        .unwrap_or(tail.len());
    let Some(after_name) = tail.get(name_end..) else {
        return false;
    };
    let after_name = after_name.trim_start();
    after_name.starts_with('=') || starts_with_word(after_name, RESOURCE_FOR_OF_KEYWORD)
}

fn starts_with_word(source: &str, word: &str) -> bool {
    let Some(after) = source.get(word.len()..) else {
        return false;
    };
    source.starts_with(word) && !after.chars().next().is_some_and(is_ascii_identifier_part)
}

const fn is_ascii_identifier_start(value: char) -> bool {
    value == '_' || value == '$' || value.is_ascii_alphabetic()
}

const fn is_ascii_identifier_part(value: char) -> bool {
    is_ascii_identifier_start(value) || value.is_ascii_digit()
}

fn is_engine262_missing_global(engine262: &EngineOutcome) -> bool {
    engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("ReferenceError")
        && engine262
            .error_message
            .as_deref()
            .is_some_and(is_engine262_missing_global_message)
}

fn is_engine262_missing_global_message(message: &str) -> bool {
    message.contains("\"Intl\" is not defined")
        || message.contains("Intl is not defined")
        || message.contains("\"SharedArrayBuffer\" is not defined")
        || message.contains("SharedArrayBuffer is not defined")
        || message.contains("\"Temporal\" is not defined")
        || message.contains("Temporal is not defined")
}

fn is_v8_fallback_unavailable(v8: &EngineOutcome) -> bool {
    is_v8_missing_global(v8) || is_v8_missing_typed_array_base64_or_hex(v8)
}

fn is_v8_missing_global(v8: &EngineOutcome) -> bool {
    v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("ReferenceError")
        && v8
            .error_message
            .as_deref()
            .is_some_and(is_v8_missing_global_message)
}

fn is_v8_missing_global_message(message: &str) -> bool {
    message.contains("Iterator is not defined")
        || message.contains("AsyncIterator is not defined")
        || message.contains("DisposableStack is not defined")
        || message.contains("AsyncDisposableStack is not defined")
        || message.contains("SuppressedError is not defined")
        || message.contains("Temporal is not defined")
        || message.contains("Float16Array is not defined")
}

fn is_v8_missing_typed_array_base64_or_hex(v8: &EngineOutcome) -> bool {
    v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("TypeError")
        && v8
            .error_message
            .as_deref()
            .is_some_and(is_v8_missing_typed_array_base64_or_hex_message)
}

fn is_v8_missing_typed_array_base64_or_hex_message(message: &str) -> bool {
    message.contains("toBase64 is not a function")
        || message.contains("fromBase64 is not a function")
        || message.contains("setFromBase64 is not a function")
        || message.contains("toHex is not a function")
        || message.contains("fromHex is not a function")
        || message.contains("setFromHex is not a function")
}

#[cfg(test)]
mod tests {
    use anyhow::ensure;

    use crate::compare::{OutcomeStatus, outcome};

    use super::{
        correctness_oracle, is_engine262_unsupported, outcomes_equivalent,
        source_contains_resource_management_syntax,
    };

    #[test]
    fn missing_float16_v8_fallback_global_disables_oracle() -> anyhow::Result<()> {
        let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
        let engine262 = reference_error("ReferenceError: \"SharedArrayBuffer\" is not defined");
        let v8 = reference_error("Float16Array is not defined");
        let unsupported = is_engine262_unsupported(
            "new SharedArrayBuffer(8); new Float16Array(1);",
            &velum,
            &engine262,
            &v8,
        );
        ensure!(unsupported);
        ensure!(correctness_oracle("new Float16Array(1)", &engine262, &v8, unsupported).is_none());
        Ok(())
    }

    #[test]
    fn missing_typed_array_base64_v8_fallback_disables_oracle() -> anyhow::Result<()> {
        let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
        let engine262 = reference_error("ReferenceError: \"SharedArrayBuffer\" is not defined");
        let v8 = type_error("Uint8Array.of(...).toBase64 is not a function");
        let unsupported = is_engine262_unsupported(
            "new SharedArrayBuffer(8); Uint8Array.of(1).toBase64();",
            &velum,
            &engine262,
            &v8,
        );
        ensure!(unsupported);
        ensure!(
            correctness_oracle("Uint8Array.of(1).toBase64()", &engine262, &v8, unsupported)
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn annex_b_string_html_engine262_gap_falls_back_to_v8() -> anyhow::Result<()> {
        let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
        let engine262 = type_error("TypeError: (\"\").bold is not a function");
        let v8 = outcome(OutcomeStatus::Ok, 1, "", None, None);
        let source = "(\"\").bold()";
        let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
        let Some(oracle) = correctness_oracle(source, &engine262, &v8, unsupported) else {
            anyhow::bail!("expected V8 fallback oracle");
        };
        ensure!(unsupported);
        ensure!(outcomes_equivalent(oracle, &v8));
        Ok(())
    }

    #[test]
    fn resource_management_symbol_gap_disables_oracle() -> anyhow::Result<()> {
        let velum = outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some("TypeError".to_owned()),
            None,
        );
        let engine262 = outcome(OutcomeStatus::Ok, 1, "", None, None);
        let v8 = outcome(OutcomeStatus::Ok, 1, "", None, None);
        let source = "new Float64Array(Symbol.dispose, Symbol.dispose, Symbol.dispose)";
        let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
        ensure!(unsupported);
        ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
        Ok(())
    }

    #[test]
    fn resource_management_syntax_detector_ignores_plain_identifiers() -> anyhow::Result<()> {
        ensure!(source_contains_resource_management_syntax(
            "for (using value of []) {}"
        ));
        ensure!(!source_contains_resource_management_syntax(
            "const usingValue = 1;"
        ));
        Ok(())
    }

    #[test]
    fn outcome_equivalence_uses_error_names_only_for_js_errors() -> anyhow::Result<()> {
        let left = reference_error("ReferenceError: left");
        let right = reference_error("ReferenceError: right");
        ensure!(outcomes_equivalent(&left, &right));
        Ok(())
    }

    fn reference_error(message: &str) -> crate::compare::EngineOutcome {
        outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some("ReferenceError".to_owned()),
            Some(message.to_owned()),
        )
    }

    fn type_error(message: &str) -> crate::compare::EngineOutcome {
        outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some("TypeError".to_owned()),
            Some(message.to_owned()),
        )
    }
}
