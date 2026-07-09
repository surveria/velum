const MAX_TABLE_DETAIL_CHARS: usize = 240;

#[must_use]
pub fn table_detail(value: &str) -> String {
    table_detail_with_limit(value, MAX_TABLE_DETAIL_CHARS)
}

#[must_use]
pub fn table_detail_with_limit(value: &str, max_chars: usize) -> String {
    let normalized = normalize_table_text(value);
    truncate_chars(&normalized, max_chars)
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
        if character == '|' {
            normalized.push('/');
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
    push_upper_hex(output, u32::from(character));
    output.push('}');
}

fn push_upper_hex(output: &mut String, value: u32) {
    let mut emitted = false;
    for shift in [20_u32, 16, 12, 8, 4, 0] {
        let nibble = (value >> shift) & 0xF;
        if nibble == 0 && !emitted && shift > 12 {
            continue;
        }
        output.push(hex_digit(nibble));
        emitted = true;
    }
}

const fn hex_digit(value: u32) -> char {
    match value {
        0 => '0',
        1 => '1',
        2 => '2',
        3 => '3',
        4 => '4',
        5 => '5',
        6 => '6',
        7 => '7',
        8 => '8',
        9 => '9',
        10 => 'A',
        11 => 'B',
        12 => 'C',
        13 => 'D',
        14 => 'E',
        _ => 'F',
    }
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
    fn table_detail_replaces_table_delimiters() -> TestResult {
        let detail = table_detail("expected a | b");

        ensure_text(&detail, "expected a / b")
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
