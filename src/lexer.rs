use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) offset: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TokenKind {
    Number(f64),
    String(String),
    Identifier(String),
    Let,
    Const,
    Var,
    True,
    False,
    Null,
    Undefined,
    Plus,
    Minus,
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
    AndAnd,
    OrOr,
    LParen,
    RParen,
    Semicolon,
    Comma,
    Eof,
}

pub(crate) fn lex(source: &str) -> Result<Vec<Token>> {
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
                ch if is_identifier_start(ch) => self.identifier(offset),
                '+' => self.simple(TokenKind::Plus),
                '-' => self.simple(TokenKind::Minus),
                '*' => self.simple(TokenKind::Star),
                '/' => self.simple(TokenKind::Slash),
                '%' => self.simple(TokenKind::Percent),
                '(' => self.simple(TokenKind::LParen),
                ')' => self.simple(TokenKind::RParen),
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
                        return Err(Error::lex("expected '&' after '&'", offset));
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

        let start_offset = self.chars[start].0;
        let end_offset = self.current_offset();
        let text = &self.source[start_offset..end_offset];
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

    fn identifier(&mut self, offset: usize) {
        let start = self.cursor;
        self.advance();
        while self.peek_char().is_some_and(is_identifier_part) {
            self.advance();
        }

        let start_offset = self.chars[start].0;
        let end_offset = self.current_offset();
        let text = &self.source[start_offset..end_offset];
        let kind = match text {
            "let" => TokenKind::Let,
            "const" => TokenKind::Const,
            "var" => TokenKind::Var,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "null" => TokenKind::Null,
            "undefined" => TokenKind::Undefined,
            _ => TokenKind::Identifier(text.to_owned()),
        };
        self.push(kind, offset);
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

    fn push(&mut self, kind: TokenKind, offset: usize) {
        self.tokens.push(Token { kind, offset });
    }

    fn advance(&mut self) -> Option<(usize, char)> {
        let value = self.peek();
        if value.is_some() {
            self.cursor += 1;
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
        self.chars.get(self.cursor + 1).map(|(_, ch)| *ch)
    }

    fn current_offset(&self) -> usize {
        self.chars
            .get(self.cursor)
            .map_or(self.source.len(), |(offset, _)| *offset)
    }
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
}

fn is_identifier_part(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}
