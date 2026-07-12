use crate::ast::{
    Expr, Expression, Program, Statement, StaticBinding, StaticCallSiteId, StaticFunctionId,
    StaticName, StaticPropertyAccessId, StaticString, Stmt,
};
use crate::error::{Error, Result};
use crate::lexer::{Token, TokenKind};
use crate::runtime::limits::RuntimeLimits;
use crate::source::{SourceId, SourceSpan};

mod assignment;
mod assignment_target;
mod await_context;
mod binary;
mod class;
mod class_private;
mod early_errors;
mod expression;
mod function;
mod function_expression;
mod literal;
mod member;
mod module;
mod pattern;
mod property_name;
mod sequence;
mod statement;
mod static_tables;
mod strict;
mod yield_context;

use await_context::{AwaitExpressionContext, AwaitIdentifierContext};
use class_private::ClassPrivateScope;
use static_tables::{StaticBindingTable, StaticFunctionTable, StaticNameTable, StaticStringTable};
use yield_context::{YieldExpressionContext, YieldIdentifierContext};

pub use module::{ModuleExportEntry, ModuleImportName, ModuleSyntax};

const ASYNC_IDENTIFIER_NAME: &str = "async";
const AWAIT_IDENTIFIER_NAME: &str = "await";
const ARGUMENTS_IDENTIFIER_NAME: &str = "arguments";
const EVAL_IDENTIFIER_NAME: &str = "eval";
const SUPER_IDENTIFIER_NAME: &str = "super";
const USE_STRICT_DIRECTIVE: &str = "use strict";
const YIELD_IDENTIFIER_NAME: &str = "yield";

pub struct ParsedFunctionBody {
    pub statements: Vec<Statement>,
    pub contains_use_strict: bool,
}

pub fn parse_with_usage(tokens: Vec<Token>, limits: RuntimeLimits) -> Result<ParsedProgram> {
    parse_with_usage_in_mode(tokens, limits, false)
}

pub fn parse_with_usage_in_mode(
    tokens: Vec<Token>,
    limits: RuntimeLimits,
    strict_mode: bool,
) -> Result<ParsedProgram> {
    Parser::new(tokens, limits, strict_mode).parse()
}

pub fn parse_module_with_usage(tokens: Vec<Token>, limits: RuntimeLimits) -> Result<ParsedProgram> {
    Parser::new_module(tokens, limits).parse()
}

pub fn parse_eval_with_usage_in_context(
    tokens: Vec<Token>,
    limits: RuntimeLimits,
    strict_mode: bool,
    allow_super_property: bool,
    allow_super_call: bool,
) -> Result<ParsedProgram> {
    let mut parser = Parser::new(tokens, limits, strict_mode);
    parser.allow_super_property = allow_super_property;
    parser.allow_super_call = allow_super_call;
    parser.parse()
}

pub struct ParsedProgram {
    pub program: Program,
    pub module: Option<ModuleSyntax>,
    pub usage: ParseUsage,
    pub strict: bool,
}

pub struct ParseUsage {
    pub top_level_statement_count: usize,
    pub max_expression_depth: usize,
    pub static_name_count: usize,
    pub static_string_count: usize,
    pub static_binding_count: usize,
    pub static_function_count: usize,
    pub static_property_access_count: usize,
    pub static_call_site_count: usize,
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    limits: RuntimeLimits,
    expression_depth: usize,
    statement_depth: usize,
    new_target_scope_depth: usize,
    control_context: ControlContext,
    max_expression_depth: usize,
    static_names: StaticNameTable,
    static_strings: StaticStringTable,
    static_bindings: StaticBindingTable,
    static_functions: StaticFunctionTable,
    static_property_access_count: usize,
    allow_super_property: bool,
    allow_super_call: bool,
    static_call_site_count: usize,
    arguments_reference_count: usize,
    strict_mode: bool,
    function_declaration_context: FunctionDeclarationContext,
    await_expression_context: AwaitExpressionContext,
    await_identifier_context: AwaitIdentifierContext,
    yield_expression_context: YieldExpressionContext,
    yield_identifier_context: YieldIdentifierContext,
    class_arguments: ClassArgumentsContext,
    /// Private-name scopes for the class bodies currently being parsed.
    /// Unlike other contexts this stack must stay visible across nested
    /// function boundaries, so function entry never resets it.
    class_private_scopes: Vec<ClassPrivateScope>,
    source_goal: SourceGoal,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum SourceGoal {
    Script,
    Module,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum ClassArgumentsContext {
    Allowed,
    Restricted,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum FunctionDeclarationContext {
    Var,
    Lexical,
}

impl Parser {
    const fn new(tokens: Vec<Token>, limits: RuntimeLimits, strict_mode: bool) -> Self {
        Self {
            tokens,
            cursor: 0,
            limits,
            expression_depth: 0,
            statement_depth: 0,
            new_target_scope_depth: 0,
            control_context: ControlContext::new(),
            max_expression_depth: 0,
            static_names: StaticNameTable::new(),
            static_strings: StaticStringTable::new(),
            static_bindings: StaticBindingTable::new(),
            static_functions: StaticFunctionTable::new(),
            static_property_access_count: 0,
            allow_super_property: false,
            allow_super_call: false,
            static_call_site_count: 0,
            arguments_reference_count: 0,
            strict_mode,
            function_declaration_context: FunctionDeclarationContext::Var,
            await_expression_context: AwaitExpressionContext::Allowed,
            await_identifier_context: AwaitIdentifierContext::Allowed,
            yield_expression_context: YieldExpressionContext::Forbidden,
            yield_identifier_context: YieldIdentifierContext::Allowed,
            class_arguments: ClassArgumentsContext::Allowed,
            class_private_scopes: Vec::new(),
            source_goal: SourceGoal::Script,
        }
    }

    const fn new_module(tokens: Vec<Token>, limits: RuntimeLimits) -> Self {
        let mut parser = Self::new(tokens, limits, true);
        parser.await_identifier_context = AwaitIdentifierContext::Reserved;
        parser.source_goal = SourceGoal::Module;
        parser
    }

    fn parse(mut self) -> Result<ParsedProgram> {
        let mut statements = Vec::new();
        let mut module = self.is_module_goal().then(ModuleSyntax::default);
        let mut directive_prologue = true;
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
            if let Some(module) = module.as_mut()
                && self.parse_module_declaration(module, &mut statements)?
            {
                continue;
            }
            let statement = self.statement_list_item()?;
            self.update_directive_prologue(&mut directive_prologue, &statement);
            statements.push(statement);
        }
        if let Some(module) = module.as_ref() {
            self.validate_module_declarations(&statements)?;
            Self::validate_module_syntax(module, &statements)?;
        }
        let usage = ParseUsage {
            top_level_statement_count: statements.len(),
            max_expression_depth: self.max_expression_depth,
            static_name_count: self.static_names.len(),
            static_string_count: self.static_strings.len(),
            static_binding_count: self.static_bindings.len(),
            static_function_count: self.static_functions.len(),
            static_property_access_count: self.static_property_access_count,
            static_call_site_count: self.static_call_site_count,
        };
        Ok(ParsedProgram {
            program: Program { statements },
            module,
            usage,
            strict: self.strict_mode,
        })
    }

    const fn is_module_goal(&self) -> bool {
        matches!(self.source_goal, SourceGoal::Module)
    }

    pub(super) fn consume_identifier(&mut self, message: &str) -> Result<StaticName> {
        let token = self.advance().ok_or_else(|| self.parse_error(message))?;
        let token_span = token.span;
        let token_offset = token.offset();
        match token.kind {
            TokenKind::Identifier(name) if name == SUPER_IDENTIFIER_NAME => Err(Error::parse_at(
                "super is not a valid identifier",
                token_span,
            )),
            TokenKind::Super => Err(Error::parse_at(
                "super is not a valid identifier",
                token_span,
            )),
            TokenKind::Identifier(name) => self.static_name_at(name, token_offset),
            TokenKind::Async => self.static_name_borrowed_at(ASYNC_IDENTIFIER_NAME, token_offset),
            TokenKind::Await if !self.await_identifier_is_reserved() => {
                self.static_name_borrowed_at(AWAIT_IDENTIFIER_NAME, token_offset)
            }
            TokenKind::Let if !self.is_strict_mode() => {
                self.static_name_borrowed_at("let", token_offset)
            }
            _ => Err(Error::parse_at(message, token_span)),
        }
    }

    pub(super) fn consume_binding_identifier(&mut self, message: &str) -> Result<StaticBinding> {
        if self.peek().is_some_and(|token| {
            token.kind == TokenKind::Super
                || matches!(&token.kind, TokenKind::Identifier(name) if name == SUPER_IDENTIFIER_NAME)
        }) {
            return Err(self.parse_error("super is not a valid binding identifier"));
        }
        let name = self.consume_identifier(message)?;
        if name.as_str() == "let" {
            return Err(self.parse_error("let is not a valid binding identifier"));
        }
        if (self.yield_identifier_is_reserved() || self.is_strict_mode())
            && name.as_str() == YIELD_IDENTIFIER_NAME
        {
            return Err(self.parse_error("yield is not a valid binding identifier"));
        }
        if self.is_strict_mode() {
            self.validate_function_name_in_strict_code(&name)?;
        }
        self.static_binding(name)
    }

    pub(super) const fn is_strict_mode(&self) -> bool {
        self.strict_mode
    }

    pub(super) const fn set_strict_mode(&mut self, strict_mode: bool) {
        self.strict_mode = strict_mode;
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

    pub(super) fn implicit_arguments_binding(&mut self) -> Result<StaticBinding> {
        self.static_binding_name(ARGUMENTS_IDENTIFIER_NAME.to_owned())
    }

    pub(super) const fn arguments_reference_snapshot(&self) -> usize {
        self.arguments_reference_count
    }

    pub(super) const fn arguments_referenced_since(&self, snapshot: usize) -> bool {
        self.arguments_reference_count > snapshot
    }

    pub(super) fn note_arguments_reference(&mut self, name: &str) -> Result<()> {
        if name != ARGUMENTS_IDENTIFIER_NAME {
            return Ok(());
        }
        self.arguments_reference_count = self
            .arguments_reference_count
            .checked_add(1)
            .ok_or_else(|| Error::limit("arguments reference count overflowed"))?;
        Ok(())
    }

    pub(super) fn static_string(&mut self, value: Vec<u16>) -> Result<StaticString> {
        self.static_strings
            .intern_owned(value, self.previous_offset())
    }

    pub(super) fn static_function(&mut self) -> Result<StaticFunctionId> {
        self.static_functions.intern()
    }

    /// Runs a parse step with the given super-usage permissions, restoring
    /// the previous permissions afterwards. Ordinary functions clear both;
    /// class members enable property access and derived constructors enable
    /// super calls; arrow functions inherit by not switching contexts.
    pub(super) fn with_super_context<T>(
        &mut self,
        allow_property: bool,
        allow_call: bool,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let previous = (self.allow_super_property, self.allow_super_call);
        self.allow_super_property = allow_property;
        self.allow_super_call = allow_call;
        let result = parse(self);
        self.allow_super_property = previous.0;
        self.allow_super_call = previous.1;
        result
    }

    pub(super) fn static_property_access(&mut self) -> Result<StaticPropertyAccessId> {
        let access = StaticPropertyAccessId::from_index(self.static_property_access_count)?;
        self.static_property_access_count = self
            .static_property_access_count
            .checked_add(1)
            .ok_or_else(|| Error::limit("static property access count overflowed"))?;
        Ok(access)
    }

    pub(super) fn static_call_site(&mut self) -> Result<StaticCallSiteId> {
        let site = StaticCallSiteId::from_index(self.static_call_site_count)?;
        self.static_call_site_count = self
            .static_call_site_count
            .checked_add(1)
            .ok_or_else(|| Error::limit("static call site count overflowed"))?;
        Ok(site)
    }

    pub(super) fn borrowed_static_name(&mut self, name: &str) -> Result<StaticName> {
        self.static_names
            .intern_borrowed(name, self.previous_offset())
    }

    pub(super) fn contextual_async_binding(&mut self, offset: usize) -> Result<StaticBinding> {
        let name = self.static_name_borrowed_at(ASYNC_IDENTIFIER_NAME, offset)?;
        self.static_binding(name)
    }

    pub(super) fn contextual_await_binding(&mut self, offset: usize) -> Result<StaticBinding> {
        let name = self.static_name_borrowed_at(AWAIT_IDENTIFIER_NAME, offset)?;
        self.static_binding(name)
    }

    fn static_name_at(&mut self, name: String, offset: usize) -> Result<StaticName> {
        self.static_names.intern_owned(name, offset)
    }

    fn static_name_borrowed_at(&mut self, name: &str, offset: usize) -> Result<StaticName> {
        self.static_names.intern_borrowed(name, offset)
    }

    pub(super) fn next_is_identifier(&self) -> bool {
        self.peek().is_some_and(|token| {
            Self::is_identifier_name(&token.kind)
                || (token.kind == TokenKind::Await && !self.await_identifier_is_reserved())
                || (token.kind == TokenKind::Let && !self.is_strict_mode())
        })
    }

    pub(super) fn peek_kind(&self, offset: usize) -> Option<&TokenKind> {
        let cursor = self.cursor.checked_add(offset)?;
        self.tokens.get(cursor).map(|token| &token.kind)
    }

    pub(super) fn peek_kind_is(&self, offset: usize, expected: &TokenKind) -> bool {
        self.peek_kind(offset)
            .is_some_and(|kind| token_kind_eq(kind, expected))
    }

    pub(super) fn peek_kind_is_no_line_terminator(
        &self,
        offset: usize,
        expected: &TokenKind,
    ) -> bool {
        self.peek_token(offset).is_some_and(|token| {
            !token.line_terminator_before && token_kind_eq(&token.kind, expected)
        })
    }

    pub(super) fn peek_has_line_terminator_before(&self, offset: usize) -> bool {
        self.peek_token(offset)
            .is_some_and(|token| token.line_terminator_before)
    }

    pub(super) fn peek_is_identifier_name(&self, offset: usize) -> bool {
        self.peek_kind(offset).is_some_and(|kind| {
            Self::is_identifier_name(kind)
                || (*kind == TokenKind::Await && !self.await_identifier_is_reserved())
                || (*kind == TokenKind::Let && !self.is_strict_mode())
        })
    }

    const fn is_identifier_name(kind: &TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::Identifier(_) | TokenKind::Async | TokenKind::Super
        )
    }

    pub(super) fn consume(&mut self, expected: &TokenKind, message: &str) -> Result<()> {
        if self.check(expected) {
            if self.advance().is_some() {
                Ok(())
            } else {
                Err(self.parse_error(message))
            }
        } else {
            Err(self.parse_error(message))
        }
    }

    pub(super) fn consume_optional_semicolon(&mut self) {
        self.match_kind(&TokenKind::Semicolon);
    }

    pub(super) fn consume_statement_terminator(&mut self, message: &str) -> Result<()> {
        if self.match_kind(&TokenKind::Semicolon)
            || self.at_end()
            || self.check(&TokenKind::RBrace)
            || self.peek_has_line_terminator_before(0)
        {
            return Ok(());
        }
        if !self.await_expression_is_allowed()
            && self
                .cursor
                .checked_sub(1)
                .and_then(|cursor| self.tokens.get(cursor))
                .is_some_and(|token| token.kind == TokenKind::Await)
        {
            return Err(self.parse_error("await expression is not allowed in this function"));
        }

        Err(self.parse_error(message))
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

    fn peek_token(&self, offset: usize) -> Option<&Token> {
        let cursor = self.cursor.checked_add(offset)?;
        self.tokens.get(cursor)
    }

    pub(super) fn offset(&self) -> usize {
        self.current_span().start()
    }

    pub(super) fn previous_offset(&self) -> usize {
        self.previous_span().start()
    }

    pub(super) fn current_span(&self) -> SourceSpan {
        self.peek()
            .or_else(|| self.tokens.last())
            .map_or(SourceSpan::point(SourceId::UNKNOWN, 0), |token| token.span)
    }

    pub(super) fn previous_span(&self) -> SourceSpan {
        self.cursor
            .checked_sub(1)
            .and_then(|cursor| self.tokens.get(cursor))
            .map_or_else(|| self.current_span(), |token| token.span)
    }

    pub(super) fn parse_error(&self, message: impl Into<String>) -> Error {
        Error::parse_at(message, self.current_span())
    }

    pub(super) fn span_since(&self, start: SourceSpan) -> SourceSpan {
        let Some(span) = start.cover(self.previous_span()) else {
            return start;
        };
        span
    }

    pub(super) fn expression_node(&self, start: SourceSpan, kind: Expr) -> Expression {
        Expression::new(kind, self.span_since(start))
    }

    pub(super) fn statement_node(&self, start: SourceSpan, kind: Stmt) -> Statement {
        Statement::new(kind, self.span_since(start))
    }

    pub(super) fn with_new_target_scope<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        self.new_target_scope_depth = self
            .new_target_scope_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("new.target scope depth overflowed"))?;
        let previous_control_context = std::mem::take(&mut self.control_context);
        let previous_class_arguments = self.class_arguments;
        self.class_arguments = ClassArgumentsContext::Allowed;
        let result = parse(self);
        self.class_arguments = previous_class_arguments;
        self.control_context = previous_control_context;
        self.new_target_scope_depth = self.new_target_scope_depth.saturating_sub(1);
        result
    }

    pub(super) const fn allows_new_target(&self) -> bool {
        self.new_target_scope_depth > 0
    }

    pub(super) fn with_isolated_control_context<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let previous = std::mem::take(&mut self.control_context);
        let result = parse(self);
        self.control_context = previous;
        result
    }

    pub(super) fn with_restricted_class_arguments<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let previous = self.class_arguments;
        self.class_arguments = ClassArgumentsContext::Restricted;
        let result = parse(self);
        self.class_arguments = previous;
        result
    }

    pub(super) const fn class_arguments_are_restricted(&self) -> bool {
        matches!(self.class_arguments, ClassArgumentsContext::Restricted)
    }

    pub(super) fn with_iteration_statement<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        self.control_context.enter_iteration()?;
        let result = parse(self);
        self.control_context.exit_iteration();
        result
    }

    pub(super) fn with_switch_statement<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        self.control_context.enter_breakable()?;
        let result = parse(self);
        self.control_context.exit_breakable();
        result
    }

    pub(super) fn with_labeled_statement<T>(
        &mut self,
        labels: &[StaticName],
        is_iteration_target: bool,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        if labels
            .iter()
            .any(|label| self.control_context.has_label(label))
        {
            return Err(self.parse_error("duplicate label in labeled statement"));
        }
        let previous_len = self.control_context.labels.len();
        self.control_context
            .push_labels(labels, is_iteration_target)?;
        let result = parse(self);
        self.control_context.labels.truncate(previous_len);
        result
    }

    pub(super) fn validate_break_statement(&self, label: Option<&StaticName>) -> Result<()> {
        match label {
            Some(label) if self.control_context.has_label(label) => Ok(()),
            Some(_) => Err(self.parse_error("break target label is not defined")),
            None if self.control_context.breakable_depth > 0 => Ok(()),
            None => Err(self.parse_error("break statement outside loop")),
        }
    }

    pub(super) fn validate_continue_statement(&self, label: Option<&StaticName>) -> Result<()> {
        match label {
            Some(label) if self.control_context.has_iteration_label(label) => Ok(()),
            Some(label) if self.control_context.has_label(label) => {
                Err(self.parse_error("continue target is not an iteration statement"))
            }
            Some(_) => Err(self.parse_error("continue target label is not defined")),
            None if self.control_context.iteration_depth > 0 => Ok(()),
            None => Err(self.parse_error("continue statement outside loop")),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ControlContext {
    iteration_depth: usize,
    breakable_depth: usize,
    labels: Vec<LabelContext>,
}

impl ControlContext {
    const fn new() -> Self {
        Self {
            iteration_depth: 0,
            breakable_depth: 0,
            labels: Vec::new(),
        }
    }

    fn enter_iteration(&mut self) -> Result<()> {
        let iteration_depth = self
            .iteration_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("iteration statement depth overflowed"))?;
        let breakable_depth = self
            .breakable_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("breakable statement depth overflowed"))?;
        self.iteration_depth = iteration_depth;
        self.breakable_depth = breakable_depth;
        Ok(())
    }

    const fn exit_iteration(&mut self) {
        self.iteration_depth = self.iteration_depth.saturating_sub(1);
        self.breakable_depth = self.breakable_depth.saturating_sub(1);
    }

    fn enter_breakable(&mut self) -> Result<()> {
        self.breakable_depth = self
            .breakable_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("breakable statement depth overflowed"))?;
        Ok(())
    }

    const fn exit_breakable(&mut self) {
        self.breakable_depth = self.breakable_depth.saturating_sub(1);
    }

    fn push_labels(&mut self, labels: &[StaticName], is_iteration_target: bool) -> Result<()> {
        for label in labels {
            self.labels
                .try_reserve(1)
                .map_err(|_| Error::limit("label stack allocation failed"))?;
            self.labels.push(LabelContext {
                name: label.clone(),
                is_iteration_target,
            });
        }
        Ok(())
    }

    fn has_label(&self, label: &StaticName) -> bool {
        self.labels.iter().rev().any(|entry| entry.name == *label)
    }

    fn has_iteration_label(&self, label: &StaticName) -> bool {
        self.labels
            .iter()
            .rev()
            .any(|entry| entry.name == *label && entry.is_iteration_target)
    }
}

#[derive(Debug, Clone)]
struct LabelContext {
    name: StaticName,
    is_iteration_target: bool,
}

fn token_kind_eq(left: &TokenKind, right: &TokenKind) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}
