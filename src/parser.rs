use crate::ast::{
    Program, StaticBinding, StaticBindingId, StaticFunctionId, StaticName, StaticNameId,
    StaticPropertyAccessId, StaticString, StaticStringId,
};
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
    pub static_string_count: usize,
    pub static_binding_count: usize,
    pub static_function_count: usize,
    pub static_property_access_count: usize,
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    limits: RuntimeLimits,
    expression_depth: usize,
    max_expression_depth: usize,
    static_names: StaticNameTable,
    static_strings: StaticStringTable,
    static_bindings: StaticBindingTable,
    static_functions: StaticFunctionTable,
    static_property_access_count: usize,
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
            static_strings: StaticStringTable::new(),
            static_bindings: StaticBindingTable::new(),
            static_functions: StaticFunctionTable::new(),
            static_property_access_count: 0,
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
            static_string_count: self.static_strings.len(),
            static_binding_count: self.static_bindings.len(),
            static_function_count: self.static_functions.len(),
            static_property_access_count: self.static_property_access_count,
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

    pub(super) fn consume_binding_identifier(&mut self, message: &str) -> Result<StaticBinding> {
        let name = self.consume_identifier(message)?;
        self.static_binding(name)
    }

    pub(super) fn static_name(&mut self, name: String) -> Result<StaticName> {
        self.static_name_at(name, self.previous_offset())
    }

    pub(super) fn static_binding(&mut self, name: StaticName) -> Result<StaticBinding> {
        self.static_bindings.intern(name)
    }

    pub(super) fn static_binding_name(&mut self, name: String) -> Result<StaticBinding> {
        let name = self.static_name(name)?;
        self.static_binding(name)
    }

    pub(super) fn static_string(&mut self, value: String) -> Result<StaticString> {
        self.static_strings
            .intern_owned(value, self.previous_offset())
    }

    pub(super) fn static_function(&mut self) -> Result<StaticFunctionId> {
        self.static_functions.intern()
    }

    pub(super) fn static_property_access(&mut self) -> Result<StaticPropertyAccessId> {
        let access = StaticPropertyAccessId::from_index(self.static_property_access_count)?;
        self.static_property_access_count = self
            .static_property_access_count
            .checked_add(1)
            .ok_or_else(|| Error::limit("static property access count overflowed"))?;
        Ok(access)
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
struct StaticStringTable {
    strings: Vec<StaticString>,
    index: Vec<StaticStringIndexEntry>,
}

impl StaticStringTable {
    const fn new() -> Self {
        Self {
            strings: Vec::new(),
            index: Vec::new(),
        }
    }

    const fn len(&self) -> usize {
        self.strings.len()
    }

    fn intern_owned(&mut self, value: String, offset: usize) -> Result<StaticString> {
        let position = self.static_string_position(&value);
        let position = match position {
            Ok(position) => return self.static_string_at_index_position(position, offset),
            Err(position) => position,
        };
        if position > self.index.len() {
            return Err(Error::parse(
                "static string insert position is out of range",
                offset,
            ));
        }
        let id = StaticStringId::from_index(self.strings.len())?;
        let value = StaticString::new(id, value);
        self.strings.push(value.clone());
        self.index
            .insert(position, StaticStringIndexEntry::new(value.clone()));
        Ok(value)
    }

    fn static_string_at_index_position(
        &self,
        position: usize,
        offset: usize,
    ) -> Result<StaticString> {
        let entry = self
            .index
            .get(position)
            .ok_or_else(|| Error::parse("static string index entry is not available", offset))?;
        self.static_string_by_id(entry.id(), offset)
    }

    fn static_string_by_id(&self, id: StaticStringId, offset: usize) -> Result<StaticString> {
        self.strings
            .get(id.index()?)
            .cloned()
            .ok_or_else(|| Error::parse("static string id is not defined", offset))
    }

    fn static_string_position(&self, value: &str) -> std::result::Result<usize, usize> {
        self.index
            .binary_search_by(|entry| entry.as_str().cmp(value))
    }
}

#[derive(Debug, Clone)]
struct StaticStringIndexEntry {
    value: StaticString,
}

impl StaticStringIndexEntry {
    const fn new(value: StaticString) -> Self {
        Self { value }
    }

    fn as_str(&self) -> &str {
        self.value.as_str()
    }

    const fn id(&self) -> StaticStringId {
        self.value.id()
    }
}

#[derive(Debug, Clone, Default)]
struct StaticNameTable {
    names: Vec<StaticName>,
    index: Vec<StaticNameIndexEntry>,
}

impl StaticNameTable {
    const fn new() -> Self {
        Self {
            names: Vec::new(),
            index: Vec::new(),
        }
    }

    const fn len(&self) -> usize {
        self.names.len()
    }

    fn intern_owned(&mut self, name: String, offset: usize) -> Result<StaticName> {
        let position = self.static_name_position(&name);
        let position = match position {
            Ok(position) => return self.static_name_at_index_position(position, offset),
            Err(position) => position,
        };
        if position > self.index.len() {
            return Err(Error::parse(
                "static name insert position is out of range",
                offset,
            ));
        }
        let id = StaticNameId::from_index(self.names.len())?;
        let name = StaticName::new(id, name);
        self.names.push(name.clone());
        self.index
            .insert(position, StaticNameIndexEntry::new(name.clone()));
        Ok(name)
    }

    fn intern_borrowed(&mut self, name: &str, offset: usize) -> Result<StaticName> {
        let position = self.static_name_position(name);
        let position = match position {
            Ok(position) => return self.static_name_at_index_position(position, offset),
            Err(position) => position,
        };
        if position > self.index.len() {
            return Err(Error::parse(
                "static name insert position is out of range",
                offset,
            ));
        }
        let id = StaticNameId::from_index(self.names.len())?;
        let name = StaticName::borrowed(id, name);
        self.names.push(name.clone());
        self.index
            .insert(position, StaticNameIndexEntry::new(name.clone()));
        Ok(name)
    }

    fn static_name_at_index_position(&self, position: usize, offset: usize) -> Result<StaticName> {
        let entry = self
            .index
            .get(position)
            .ok_or_else(|| Error::parse("static name index entry is not available", offset))?;
        self.static_name_by_id(entry.id(), offset)
    }

    fn static_name_by_id(&self, id: StaticNameId, offset: usize) -> Result<StaticName> {
        self.names
            .get(id.index()?)
            .cloned()
            .ok_or_else(|| Error::parse("static name id is not defined", offset))
    }

    fn static_name_position(&self, name: &str) -> std::result::Result<usize, usize> {
        self.index
            .binary_search_by(|entry| entry.as_str().cmp(name))
    }
}

#[derive(Debug, Clone)]
struct StaticNameIndexEntry {
    name: StaticName,
}

impl StaticNameIndexEntry {
    const fn new(name: StaticName) -> Self {
        Self { name }
    }

    fn as_str(&self) -> &str {
        self.name.as_str()
    }

    const fn id(&self) -> StaticNameId {
        self.name.id()
    }
}

#[derive(Debug, Clone, Default)]
struct StaticBindingTable {
    count: usize,
}

impl StaticBindingTable {
    const fn new() -> Self {
        Self { count: 0 }
    }

    const fn len(&self) -> usize {
        self.count
    }

    fn intern(&mut self, name: StaticName) -> Result<StaticBinding> {
        let id = StaticBindingId::from_index(self.count)?;
        self.count = self
            .count
            .checked_add(1)
            .ok_or_else(|| Error::limit("static binding count overflowed"))?;
        Ok(StaticBinding::new(id, name))
    }
}

#[derive(Debug, Clone, Default)]
struct StaticFunctionTable {
    count: usize,
}

impl StaticFunctionTable {
    const fn new() -> Self {
        Self { count: 0 }
    }

    const fn len(&self) -> usize {
        self.count
    }

    fn intern(&mut self) -> Result<StaticFunctionId> {
        let id = StaticFunctionId::from_index(self.count)?;
        self.count = self
            .count
            .checked_add(1)
            .ok_or_else(|| Error::limit("static function count overflowed"))?;
        Ok(id)
    }
}

fn token_kind_eq(left: &TokenKind, right: &TokenKind) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}
