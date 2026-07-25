use crate::compare::{EngineOutcome, OutcomeStatus};

pub fn is_engine262_missing_unescape_global(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source_contains_word(source, "unescape")
        && is_missing_engine262_unescape(engine262)
        && !is_missing_v8_unescape(v8)
}

fn is_missing_engine262_unescape(engine262: &EngineOutcome) -> bool {
    engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("ReferenceError")
        && engine262
            .error_message
            .as_deref()
            .is_some_and(is_missing_unescape_message)
}

fn is_missing_v8_unescape(v8: &EngineOutcome) -> bool {
    v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("ReferenceError")
        && v8.error_message.as_deref().is_some_and(|message| {
            message.contains("unescape is not defined") || message.contains("\"unescape\" is not defined")
        })
}

fn is_missing_unescape_message(message: &str) -> bool {
    message.contains("\"unescape\" is not defined") || message.contains("unescape is not defined")
}

fn source_contains_word(source: &str, word: &str) -> bool {
    source.match_indices(word).any(|(start, _)| {
        let before = source
            .get(..start)
            .and_then(|prefix| prefix.chars().next_back());
        let after = source
            .get(start.saturating_add(word.len())..)
            .and_then(|suffix| suffix.chars().next());
        !before.is_some_and(is_ascii_identifier_part)
            && !after.is_some_and(is_ascii_identifier_part)
    })
}

const fn is_ascii_identifier_part(value: char) -> bool {
    value == '_' || value == '$' || value.is_ascii_alphanumeric()
}
