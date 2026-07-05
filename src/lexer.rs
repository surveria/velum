use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Number(f64),
    String(String),
    Identifier(String),
    Let,
    Const,
    Var,
    If,
    Else,
    While,
    For,
    Switch,
    Case,
    Default,
    Break,
    Continue,
    Try,
    Catch,
    Finally,
    Throw,
    Return,
    Function,
    New,
    Typeof,
    Void,
    Delete,
    True,
    False,
    Null,
    Undefined,
    Plus,
    PlusPlus,
    Minus,
    MinusMinus,
    Star,
    Slash,
    Percent,
    Bang,
    Equal,
    EqualEqual,
    BangEqual,
    StrictEqual,
    StrictNotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Ampersand,
    AndAnd,
    OrOr,
    Question,
    Colon,
    Dot,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Semicolon,
    Comma,
    Eof,
}

pub fn lex(source: &str) -> Result<Vec<Token>> {
    Lexer::new(source).lex()
}

struct Lexer<'a> {
    source: &'a str,
    chars: Vec<(usize, char)>,
    cursor: usize,
    tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.char_indices().collect(),
            cursor: 0,
            tokens: Vec::new(),
        }
    }

    fn lex(mut self) -> Result<Vec<Token>> {
        while let Some((offset, ch)) = self.peek() {
            match ch {
                ch if ch.is_whitespace() => {
                    self.advance();
                }
                '/' if self.peek_next_char() == Some('/') => self.line_comment(),
                '/' if self.peek_next_char() == Some('*') => self.block_comment(offset)?,
                '0'..='9' => self.number(offset)?,
                '"' | '\'' => self.string(offset, ch)?,
                ch if is_identifier_start(ch) => self.identifier(offset)?,
                '+' => self.plus_or_increment(offset),
                '-' => self.minus_or_decrement(offset),
                '*' => self.simple(TokenKind::Star),
                '/' => self.simple(TokenKind::Slash),
                '%' => self.simple(TokenKind::Percent),
                '?' => self.simple(TokenKind::Question),
                ':' => self.simple(TokenKind::Colon),
                '.' => self.simple(TokenKind::Dot),
                '(' => self.simple(TokenKind::LParen),
                ')' => self.simple(TokenKind::RParen),
                '{' => self.simple(TokenKind::LBrace),
                '}' => self.simple(TokenKind::RBrace),
                '[' => self.simple(TokenKind::LBracket),
                ']' => self.simple(TokenKind::RBracket),
                ';' => self.simple(TokenKind::Semicolon),
                ',' => self.simple(TokenKind::Comma),
                '!' => {
                    self.advance();
                    if self.match_char('=') {
                        if self.match_char('=') {
                            self.push(TokenKind::StrictNotEqual, offset);
                        } else {
                            self.push(TokenKind::BangEqual, offset);
                        }
                    } else {
                        self.push(TokenKind::Bang, offset);
                    }
                }
                '=' => {
                    self.advance();
                    if self.match_char('=') {
                        if self.match_char('=') {
                            self.push(TokenKind::StrictEqual, offset);
                        } else {
                            self.push(TokenKind::EqualEqual, offset);
                        }
                    } else {
                        self.push(TokenKind::Equal, offset);
                    }
                }
                '<' => {
                    self.advance();
                    if self.match_char('=') {
                        self.push(TokenKind::LessEqual, offset);
                    } else {
                        self.push(TokenKind::Less, offset);
                    }
                }
                '>' => {
                    self.advance();
                    if self.match_char('=') {
                        self.push(TokenKind::GreaterEqual, offset);
                    } else {
                        self.push(TokenKind::Greater, offset);
                    }
                }
                '&' => {
                    self.advance();
                    if self.match_char('&') {
                        self.push(TokenKind::AndAnd, offset);
                    } else {
                        self.push(TokenKind::Ampersand, offset);
                    }
                }
                '|' => {
                    self.advance();
                    if self.match_char('|') {
                        self.push(TokenKind::OrOr, offset);
                    } else {
                        return Err(Error::lex("expected '|' after '|'", offset));
                    }
                }
                _ => return Err(Error::lex(format!("unexpected character '{ch}'"), offset)),
            }
        }

        self.tokens.push(Token {
            kind: TokenKind::Eof,
            offset: self.source.len(),
        });
        Ok(self.tokens)
    }

    fn number(&mut self, offset: usize) -> Result<()> {
        let start = self.cursor;
        while matches!(self.peek_char(), Some('0'..='9')) {
            self.advance();
        }

        if self.peek_char() == Some('.') && matches!(self.peek_next_char(), Some('0'..='9')) {
            self.advance();
            while matches!(self.peek_char(), Some('0'..='9')) {
                self.advance();
            }
        }

        let start_offset = self.char_offset(start, offset, "number")?;
        let end_offset = self.current_offset();
        let text = self.source_slice(start_offset, end_offset, offset, "number")?;
        let value = text
            .parse::<f64>()
            .map_err(|_| Error::lex("invalid number literal", offset))?;
        self.push(TokenKind::Number(value), offset);
        Ok(())
    }

    fn string(&mut self, offset: usize, quote: char) -> Result<()> {
        self.advance();
        let mut output = String::new();

        while let Some((current_offset, ch)) = self.peek() {
            self.advance();
            match ch {
                ch if ch == quote => {
                    self.push(TokenKind::String(output), offset);
                    return Ok(());
                }
                '\\' => {
                    let Some((escape_offset, escaped)) = self.peek() else {
                        return Err(Error::lex("unterminated escape sequence", current_offset));
                    };
                    self.advance();
                    let escaped = match escaped {
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        '\\' => '\\',
                        '"' => '"',
                        '\'' => '\'',
                        other => {
                            return Err(Error::lex(
                                format!("unsupported escape sequence '\\{other}'"),
                                escape_offset,
                            ));
                        }
                    };
                    output.push(escaped);
                }
                '\n' | '\r' => return Err(Error::lex("unterminated string literal", offset)),
                other => output.push(other),
            }
        }

        Err(Error::lex("unterminated string literal", offset))
    }

    fn identifier(&mut self, offset: usize) -> Result<()> {
        let start = self.cursor;
        self.advance();
        while self.peek_char().is_some_and(is_identifier_part) {
            self.advance();
        }

        let start_offset = self.char_offset(start, offset, "identifier")?;
        let end_offset = self.current_offset();
        let text = self.source_slice(start_offset, end_offset, offset, "identifier")?;
        let kind = match text {
            "let" => TokenKind::Let,
            "const" => TokenKind::Const,
            "var" => TokenKind::Var,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "switch" => TokenKind::Switch,
            "case" => TokenKind::Case,
            "default" => TokenKind::Default,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "try" => TokenKind::Try,
            "catch" => TokenKind::Catch,
            "finally" => TokenKind::Finally,
            "throw" => TokenKind::Throw,
            "return" => TokenKind::Return,
            "function" => TokenKind::Function,
            "new" => TokenKind::New,
            "typeof" => TokenKind::Typeof,
            "void" => TokenKind::Void,
            "delete" => TokenKind::Delete,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "null" => TokenKind::Null,
            "undefined" => TokenKind::Undefined,
            _ => TokenKind::Identifier(text.to_owned()),
        };
        self.push(kind, offset);
        Ok(())
    }

    fn line_comment(&mut self) {
        while let Some(ch) = self.peek_char() {
            self.advance();
            if ch == '\n' {
                break;
            }
        }
    }

    fn block_comment(&mut self, offset: usize) -> Result<()> {
        self.advance();
        self.advance();

        while self.peek().is_some() {
            if self.peek_char() == Some('*') && self.peek_next_char() == Some('/') {
                self.advance();
                self.advance();
                return Ok(());
            }
            self.advance();
        }

        Err(Error::lex("unterminated block comment", offset))
    }

    fn simple(&mut self, kind: TokenKind) {
        let offset = self.peek().map_or(self.source.len(), |(offset, _)| offset);
        self.advance();
        self.push(kind, offset);
    }

    fn plus_or_increment(&mut self, offset: usize) {
        self.advance();
        if self.match_char('+') {
            self.push(TokenKind::PlusPlus, offset);
        } else {
            self.push(TokenKind::Plus, offset);
        }
    }

    fn minus_or_decrement(&mut self, offset: usize) {
        self.advance();
        if self.match_char('-') {
            self.push(TokenKind::MinusMinus, offset);
        } else {
            self.push(TokenKind::Minus, offset);
        }
    }

    fn push(&mut self, kind: TokenKind, offset: usize) {
        self.tokens.push(Token { kind, offset });
    }

    fn advance(&mut self) -> Option<(usize, char)> {
        let value = self.peek();
        if value.is_some() {
            self.cursor = self.cursor.saturating_add(1);
        }
        value
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<(usize, char)> {
        self.chars.get(self.cursor).copied()
    }

    fn peek_char(&self) -> Option<char> {
        self.peek().map(|(_, ch)| ch)
    }

    fn peek_next_char(&self) -> Option<char> {
        let next_cursor = self.cursor.checked_add(1)?;
        self.chars.get(next_cursor).map(|(_, ch)| *ch)
    }

    fn current_offset(&self) -> usize {
        self.chars
            .get(self.cursor)
            .map_or(self.source.len(), |(offset, _)| *offset)
    }

    fn char_offset(&self, cursor: usize, offset: usize, description: &str) -> Result<usize> {
        self.chars
            .get(cursor)
            .map(|(offset, _)| *offset)
            .ok_or_else(|| Error::lex(format!("invalid {description} start"), offset))
    }

    fn source_slice(
        &self,
        start: usize,
        end: usize,
        offset: usize,
        description: &str,
    ) -> Result<&str> {
        self.source
            .get(start..end)
            .ok_or_else(|| Error::lex(format!("invalid {description} span"), offset))
    }
}

const fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
}

const fn is_identifier_part(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}
