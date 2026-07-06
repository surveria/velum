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
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    limits: RuntimeLimits,
    expression_depth: usize,
    max_expression_depth: usize,
}

impl Parser {
    const fn new(tokens: Vec<Token>, limits: RuntimeLimits) -> Self {
        Self {
            tokens,
            cursor: 0,
            limits,
            expression_depth: 0,
            max_expression_depth: 0,
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
            TokenKind::Identifier(name) => Ok(StaticName::new(name)),
            _ => Err(Error::parse(message, token.offset)),
        }
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

fn token_kind_eq(left: &TokenKind, right: &TokenKind) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}
