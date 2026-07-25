use crate::compare::{EngineOutcome, OutcomeStatus};

const ANNEX_B_ESCAPE_GLOBALS: [&str; 2] = ["escape", "unescape"];

pub fn is_engine262_missing_annex_b_escape_global(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    ANNEX_B_ESCAPE_GLOBALS.iter().any(|global| {
        source_contains_word(source, global)
            && is_missing_engine262_global(engine262, global)
            && !is_missing_v8_global(v8, global)
    })
}

fn is_missing_engine262_global(engine262: &EngineOutcome, global: &str) -> bool {
    engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("ReferenceError")
        && engine262
            .error_message
            .as_deref()
            .is_some_and(|message| is_missing_global_message(message, global))
}

fn is_missing_v8_global(v8: &EngineOutcome, global: &str) -> bool {
    v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("ReferenceError")
        && v8
            .error_message
            .as_deref()
            .is_some_and(|message| is_missing_global_message(message, global))
}

fn is_missing_global_message(message: &str, global: &str) -> bool {
    message.contains(&format!("\"{global}\" is not defined"))
        || message.contains(&format!("{global} is not defined"))
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
