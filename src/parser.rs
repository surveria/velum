use crate::ast::{Program, StaticName};
use crate::error::{Error, Result};
use crate::lexer::{Token, TokenKind};
use crate::runtime_limits::RuntimeLimits;

#[path = "parser_assignment.rs"]
mod parser_assignment;
#[path = "parser_binary.rs"]
mod parser_binary;
#[path = "parser_expression.rs"]
mod parser_expression;
#[path = "parser_statement.rs"]
mod parser_statement;

pub fn parse_with_usage(tokens: Vec<Token>, limits: RuntimeLimits) -> Result<ParsedProgram> {
    Parser::new(tokens, limits).parse()
}

pub struct ParsedProgram {
    pub program: Program,
    pub usage: ParseUsage,
}

pub struct ParseUsage {
    pub top_level_statement_count: usize,
    pub max_expression_depth: usize,
    pub static_name_count: usize,
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    limits: RuntimeLimits,
    expression_depth: usize,
    max_expression_depth: usize,
    static_names: StaticNameTable,
}

impl Parser {
    const fn new(tokens: Vec<Token>, limits: RuntimeLimits) -> Self {
        Self {
            tokens,
            cursor: 0,
            limits,
            expression_depth: 0,
            max_expression_depth: 0,
            static_names: StaticNameTable::new(),
        }
    }

    fn parse(mut self) -> Result<ParsedProgram> {
        let mut statements = Vec::new();
        while !self.at_end() {
            if self.match_kind(&TokenKind::Semicolon) {
                continue;
            }
            if statements.len() >= self.limits.max_statements {
                return Err(Error::limit(format!(
                    "statement count exceeded {}",
                    self.limits.max_statements
                )));
            }
            statements.push(self.statement()?);
        }
        let usage = ParseUsage {
            top_level_statement_count: statements.len(),
            max_expression_depth: self.max_expression_depth,
            static_name_count: self.static_names.len(),
        };
        Ok(ParsedProgram {
            program: Program { statements },
            usage,
        })
    }

    pub(super) fn consume_identifier(&mut self, message: &str) -> Result<StaticName> {
        let token = self
            .advance()
            .ok_or_else(|| Error::parse(message, self.offset()))?;
        match token.kind {
            TokenKind::Identifier(name) => self.static_name_at(name, token.offset),
            _ => Err(Error::parse(message, token.offset)),
        }
    }

    pub(super) fn static_name(&mut self, name: String) -> Result<StaticName> {
        self.static_name_at(name, self.previous_offset())
    }

    pub(super) fn borrowed_static_name(&mut self, name: &str) -> Result<StaticName> {
        self.static_names
            .intern_borrowed(name, self.previous_offset())
    }

    fn static_name_at(&mut self, name: String, offset: usize) -> Result<StaticName> {
        self.static_names.intern_owned(name, offset)
    }

    pub(super) fn next_is_identifier(&self) -> bool {
        self.peek()
            .is_some_and(|token| matches!(&token.kind, TokenKind::Identifier(_)))
    }

    pub(super) fn consume(&mut self, expected: &TokenKind, message: &str) -> Result<()> {
        if self.check(expected) {
            if self.advance().is_some() {
                Ok(())
            } else {
                Err(Error::parse(message, self.offset()))
            }
        } else {
            Err(Error::parse(message, self.offset()))
        }
    }

    pub(super) fn consume_optional_semicolon(&mut self) {
        self.match_kind(&TokenKind::Semicolon);
    }

    pub(super) fn match_kind(&mut self, expected: &TokenKind) -> bool {
        if self.check(expected) {
            self.advance().is_some()
        } else {
            false
        }
    }

    pub(super) fn check(&self, expected: &TokenKind) -> bool {
        self.peek()
            .is_some_and(|token| token_kind_eq(&token.kind, expected))
    }

    pub(super) fn advance(&mut self) -> Option<Token> {
        let token = self.peek()?.clone();
        if !matches!(token.kind, TokenKind::Eof) {
            self.cursor = self.cursor.saturating_add(1);
        }
        Some(token)
    }

    pub(super) fn at_end(&self) -> bool {
        self.peek()
            .is_none_or(|token| matches!(token.kind, TokenKind::Eof))
    }

    pub(super) fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.cursor)
    }

    pub(super) fn offset(&self) -> usize {
        self.peek()
            .or_else(|| self.tokens.last())
            .map_or(0, |token| token.offset)
    }

    pub(super) fn previous_offset(&self) -> usize {
        self.cursor
            .checked_sub(1)
            .and_then(|cursor| self.tokens.get(cursor))
            .map_or_else(|| self.offset(), |token| token.offset)
    }
}

#[derive(Debug, Clone, Default)]
struct StaticNameTable {
    names: Vec<StaticName>,
}

impl StaticNameTable {
    const fn new() -> Self {
        Self { names: Vec::new() }
    }

    const fn len(&self) -> usize {
        self.names.len()
    }

    fn intern_owned(&mut self, name: String, offset: usize) -> Result<StaticName> {
        let position = self.static_name_position(&name);
        let position = match position {
            Ok(position) => {
                return self
                    .names
                    .get(position)
                    .cloned()
                    .ok_or_else(|| Error::parse("static name entry is not available", offset));
            }
            Err(position) => position,
        };
        if position > self.names.len() {
            return Err(Error::parse(
                "static name insert position is out of range",
                offset,
            ));
        }
        let name = StaticName::new(name);
        self.names.insert(position, name.clone());
        Ok(name)
    }

    fn intern_borrowed(&mut self, name: &str, offset: usize) -> Result<StaticName> {
        let position = self.static_name_position(name);
        let position = match position {
            Ok(position) => {
                return self
                    .names
                    .get(position)
                    .cloned()
                    .ok_or_else(|| Error::parse("static name entry is not available", offset));
            }
            Err(position) => position,
        };
        if position > self.names.len() {
            return Err(Error::parse(
                "static name insert position is out of range",
                offset,
            ));
        }
        let name = StaticName::borrowed(name);
        self.names.insert(position, name.clone());
        Ok(name)
    }

    fn static_name_position(&self, name: &str) -> std::result::Result<usize, usize> {
        self.names
            .binary_search_by(|entry| entry.as_str().cmp(name))
    }
}

fn token_kind_eq(left: &TokenKind, right: &TokenKind) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}
