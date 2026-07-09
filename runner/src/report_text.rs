const MAX_TABLE_DETAIL_CHARS: usize = 240;

#[must_use]
pub fn table_detail(value: &str) -> String {
    let normalized = normalize_table_text(value);
    truncate_chars(&normalized, MAX_TABLE_DETAIL_CHARS)
}

fn normalize_table_text(value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_space = false;
    for character in value.chars() {
        if character.is_whitespace() {
            push_space(&mut normalized, &mut previous_space);
            continue;
        }
        if character.is_control() {
            push_escaped_control(&mut normalized, character);
            previous_space = false;
            continue;
        }
        normalized.push(character);
        previous_space = false;
    }
    normalized.trim().to_owned()
}

fn push_space(output: &mut String, previous_space: &mut bool) {
    if output.is_empty() || *previous_space {
        return;
    }
    output.push(' ');
    *previous_space = true;
}

fn push_escaped_control(output: &mut String, character: char) {
    output.push_str("\\u{");
    output.push_str(&format!("{:04X}", u32::from(character)));
    output.push('}');
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut truncated = String::new();
    let mut chars = value.chars();
    for _ in 0..max_chars {
        let Some(character) = chars.next() else {
            return truncated;
        };
        truncated.push(character);
    }
    if chars.next().is_some() {
        truncated.push_str("...");
    }
    truncated
}

#[cfg(test)]
mod tests {
    use super::table_detail;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn table_detail_flattens_multiline_output() -> TestResult {
        let detail = table_detail("first line\n    at stack\tframe\r\nsecond line");

        ensure_text(&detail, "first line at stack frame second line")
    }

    #[test]
    fn table_detail_escapes_control_characters() -> TestResult {
        let detail = table_detail("lexer error: unexpected character '\u{1b}\u{0}'");

        ensure_text(
            &detail,
            "lexer error: unexpected character '\\u{001B}\\u{0000}'",
        )
    }

    #[test]
    fn table_detail_truncates_long_diagnostics() -> TestResult {
        let detail = table_detail(&"x".repeat(260));

        ensure_usize(detail.chars().count(), 243)?;
        ensure_bool(detail.ends_with("..."), "detail should mark truncation")
    }

    fn ensure_text(actual: &str, expected: &str) -> TestResult {
        if actual == expected {
            return Ok(());
        }
        Err(format!("expected '{expected}', got '{actual}'").into())
    }

    fn ensure_bool(actual: bool, message: &str) -> TestResult {
        if actual {
            return Ok(());
        }
        Err(message.to_owned().into())
    }

    fn ensure_usize(actual: usize, expected: usize) -> TestResult {
        if actual == expected {
            return Ok(());
        }
        Err(format!("expected {expected}, got {actual}").into())
    }
}
