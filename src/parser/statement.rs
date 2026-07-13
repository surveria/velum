use std::collections::BTreeSet;

use crate::{
    ast::{CatchClause, DeclKind, Expression, FunctionKind, Statement, Stmt, SwitchCase},
    error::{Error, Result},
    lexer::TokenKind,
};

use super::{ParsedFunctionBody, Parser};

mod for_statement;
mod function_declaration;
mod resource_declaration;

struct ParsedLabel {
    name: crate::ast::StaticName,
    start: crate::SourceSpan,
}

impl Parser {
    pub(super) fn statement(&mut self) -> Result<Statement> {
        self.parse_statement(false)
    }

    pub(super) fn statement_list_item(&mut self) -> Result<Statement> {
        self.parse_statement(true)
    }

    fn parse_statement(&mut self, lexical_declaration_allowed: bool) -> Result<Statement> {
        let start = self.current_span();
        let kind = self
            .with_statement_depth(|parser| parser.statement_inner(lexical_declaration_allowed))?;
        Ok(self.statement_node(start, kind))
    }

    fn statement_inner(&mut self, lexical_declaration_allowed: bool) -> Result<Stmt> {
        if self.match_kind(&TokenKind::LBrace) {
            return self.block();
        }
        if self.match_kind(&TokenKind::Semicolon) {
            return Ok(Stmt::Empty);
        }
        if self.match_kind(&TokenKind::If) {
            return self.if_statement();
        }
        if self.match_kind(&TokenKind::Do) {
            return self.do_while_statement();
        }
        if self.match_kind(&TokenKind::With) {
            return self.with_statement();
        }
        if self.label_statement_start() {
            return self.label_statement();
        }
        if self.match_kind(&TokenKind::While) {
            return self.while_statement();
        }
        if self.match_kind(&TokenKind::For) {
            return self.for_statement();
        }
        if self.match_kind(&TokenKind::Switch) {
            return self.switch_statement();
        }
        if self.match_kind(&TokenKind::Try) {
            return self.try_statement();
        }
        if self.match_kind(&TokenKind::Break) {
            return self.break_statement();
        }
        if self.match_kind(&TokenKind::Continue) {
            return self.continue_statement();
        }
        if self.match_kind(&TokenKind::Debugger) {
            self.consume_optional_semicolon();
            return Ok(Stmt::Empty);
        }
        if self.match_kind(&TokenKind::Throw) {
            return self.throw_statement();
        }
        if self.match_kind(&TokenKind::Return) {
            return self.return_statement();
        }
        if self.check(&TokenKind::Async)
            && self.peek_kind_is_no_line_terminator(1, &TokenKind::Function)
        {
            self.consume(&TokenKind::Async, "expected 'async' before async function")?;
            self.consume(&TokenKind::Function, "expected 'function' after 'async'")?;
            let kind = if self.match_kind(&TokenKind::Star) {
                FunctionKind::AsyncGenerator
            } else {
                FunctionKind::Async
            };
            return self.function_declaration(kind);
        }
        if self.match_kind(&TokenKind::Function) {
            let kind = if self.match_kind(&TokenKind::Star) {
                FunctionKind::Generator
            } else {
                FunctionKind::Ordinary
            };
            return self.function_declaration(kind);
        }
        if self.match_kind(&TokenKind::Class) {
            return self.class_declaration();
        }
        if self.let_starts_expression_statement(lexical_declaration_allowed) {
            return self.let_expression_statement();
        }
        if self.await_using_declaration_start() {
            self.consume(&TokenKind::Await, "expected 'await'")?;
            self.consume_contextual_using()?;
            return self.resource_decl(DeclKind::AwaitUsing);
        }
        if self.using_declaration_start() {
            self.consume_contextual_using()?;
            return self.resource_decl(DeclKind::Using);
        }
        if self.match_kind(&TokenKind::Let) {
            return self.var_decl(DeclKind::Let);
        }
        if self.match_kind(&TokenKind::Const) {
            return self.var_decl(DeclKind::Const);
        }
        if self.match_kind(&TokenKind::Var) {
            return self.var_decl(DeclKind::Var);
        }

        let expr = self.expression()?;
        self.consume_statement_terminator("expected statement terminator")?;
        Ok(Stmt::Expr(expr))
    }

    fn let_starts_expression_statement(&mut self, lexical_declaration_allowed: bool) -> bool {
        if self.is_strict_mode() || !self.check(&TokenKind::Let) {
            return false;
        }
        if self.peek().is_some_and(|token| token.identifier_escaped) {
            return true;
        }
        if !lexical_declaration_allowed {
            return !self.peek_kind_is(1, &TokenKind::LBracket);
        }
        !self.let_declaration_lookahead()
    }

    fn let_declaration_lookahead(&mut self) -> bool {
        self.peek_kind_is(1, &TokenKind::LBrace)
            || self.peek_kind_is(1, &TokenKind::LBracket)
            || self.peek_kind_is(1, &TokenKind::Let)
            || self.peek_token(1).is_some_and(|token| {
                matches!(
                    token.kind,
                    TokenKind::Identifier(_) | TokenKind::Async | TokenKind::Super
                ) || token.kind == TokenKind::Await
            })
    }

    fn let_starts_disallowed_declaration(&mut self) -> bool {
        self.check(&TokenKind::Let)
            && !self.peek().is_some_and(|token| token.identifier_escaped)
            && (!self.let_starts_expression_statement(false)
                || (!self.peek_has_line_terminator_before(1) && self.let_declaration_lookahead()))
    }

    fn let_expression_statement(&mut self) -> Result<Stmt> {
        let expression = self.expression()?;
        self.consume_statement_terminator("expected statement terminator after 'let'")?;
        Ok(Stmt::Expr(expression))
    }

    fn with_statement_depth(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<Stmt>,
    ) -> Result<Stmt> {
        self.statement_depth = self
            .statement_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("statement nesting overflowed"))?;
        if self.statement_depth > self.limits.max_expression_depth {
            self.statement_depth = self.statement_depth.saturating_sub(1);
            return Err(Error::limit(format!(
                "statement nesting exceeded {}",
                self.limits.max_expression_depth
            )));
        }
        let result = parse(self);
        self.statement_depth = self.statement_depth.saturating_sub(1);
        result
    }

    pub(super) fn block_statements(&mut self) -> Result<Vec<Statement>> {
        let mut statements = Vec::new();
        while !self.check(&TokenKind::RBrace) {
            if self.at_end() {
                return Err(self.parse_error("expected '}' after block"));
            }
            statements.push(self.with_lexical_function_declarations(Self::statement_list_item)?);
        }
        self.consume(&TokenKind::RBrace, "expected '}' after block")?;
        self.validate_generator_block_declarations(&statements)?;
        Ok(statements)
    }

    fn block(&mut self) -> Result<Stmt> {
        Ok(Stmt::Block(self.block_statements()?))
    }

    pub(super) fn function_body(&mut self, inherited_strict: bool) -> Result<ParsedFunctionBody> {
        self.function_body_depth = self
            .function_body_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("function body nesting overflowed"))?;
        let previous_strict = self.is_strict_mode();
        let previous_function_scope = self.function_declaration_context;
        self.set_strict_mode(inherited_strict);
        self.function_declaration_context = super::FunctionDeclarationContext::Var;
        let result = self.function_body_inner();
        self.function_declaration_context = previous_function_scope;
        self.set_strict_mode(previous_strict);
        self.function_body_depth = self.function_body_depth.saturating_sub(1);
        result
    }

    fn function_body_inner(&mut self) -> Result<ParsedFunctionBody> {
        let mut statements = Vec::new();
        let mut directive_prologue = true;
        let mut contains_use_strict = false;

        while !self.check(&TokenKind::RBrace) {
            if self.at_end() {
                return Err(self.parse_error("expected '}' after block"));
            }
            let statement = self.statement_list_item()?;
            if directive_prologue && Self::is_use_strict_directive(&statement) {
                contains_use_strict = true;
            }
            self.update_directive_prologue(&mut directive_prologue, &statement);
            statements.push(statement);
        }

        self.consume(&TokenKind::RBrace, "expected '}' after block")?;
        if statements.is_empty() {
            statements.push(Statement::new(Stmt::Empty, self.previous_span()));
        }
        Ok(ParsedFunctionBody {
            statements,
            contains_use_strict,
        })
    }

    fn if_statement(&mut self) -> Result<Stmt> {
        self.consume(&TokenKind::LParen, "expected '(' after 'if'")?;
        let condition = self.expression()?;
        self.consume(&TokenKind::RParen, "expected ')' after if condition")?;
        let consequent = self.with_lexical_function_declarations(Self::statement)?;
        self.reject_invalid_single_statement(&consequent)?;
        let consequent = Box::new(Self::wrap_single_statement_function(consequent));
        let alternate = if self.match_kind(&TokenKind::Else) {
            let alternate = self.with_lexical_function_declarations(Self::statement)?;
            self.reject_invalid_single_statement(&alternate)?;
            Some(Box::new(Self::wrap_single_statement_function(alternate)))
        } else {
            None
        };
        Ok(Stmt::If {
            condition,
            consequent,
            alternate,
        })
    }

    fn while_statement(&mut self) -> Result<Stmt> {
        self.consume(&TokenKind::LParen, "expected '(' after 'while'")?;
        let condition = self.expression()?;
        self.consume(&TokenKind::RParen, "expected ')' after while condition")?;
        let body = Box::new(self.with_iteration_statement(Self::statement)?);
        self.reject_invalid_single_statement(&body)?;
        Ok(Stmt::While { condition, body })
    }

    fn do_while_statement(&mut self) -> Result<Stmt> {
        if self.let_starts_disallowed_declaration()
            || self.check(&TokenKind::Const)
            || self.check(&TokenKind::Function)
            || (self.check(&TokenKind::Async)
                && self.peek_kind_is_no_line_terminator(1, &TokenKind::Function))
        {
            return Err(self.parse_error("declaration is not allowed as a do-while body"));
        }
        let body = Box::new(self.with_iteration_statement(Self::statement)?);
        if Self::invalid_do_while_body(&body) {
            return Err(self.parse_error("declaration is not allowed as a do-while body"));
        }
        self.consume(&TokenKind::While, "expected 'while' after do body")?;
        self.consume(&TokenKind::LParen, "expected '(' after 'while'")?;
        let condition = self.expression()?;
        self.consume(&TokenKind::RParen, "expected ')' after do-while condition")?;
        self.consume_optional_semicolon();
        Ok(Stmt::DoWhile { body, condition })
    }

    fn with_statement(&mut self) -> Result<Stmt> {
        if self.is_strict_mode() {
            return Err(self.parse_error("with statement is not allowed in strict mode"));
        }
        self.consume(&TokenKind::LParen, "expected '(' after 'with'")?;
        let object = self.expression()?;
        self.consume(&TokenKind::RParen, "expected ')' after with object")?;
        let body = Box::new(self.statement()?);
        if Self::invalid_with_body(&body) {
            return Err(self.parse_error("declaration is not allowed as a with body"));
        }
        Ok(Stmt::With { object, body })
    }

    fn invalid_with_body(statement: &Statement) -> bool {
        match statement.kind() {
            Stmt::FunctionDecl { .. }
            | Stmt::ClassDecl { .. }
            | Stmt::VarDecl {
                kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
                ..
            }
            | Stmt::PatternDecl {
                kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
                ..
            } => true,
            Stmt::Label { body, .. } => Self::invalid_with_body(body),
            Stmt::Empty
            | Stmt::Block(_)
            | Stmt::DeclList(_)
            | Stmt::If { .. }
            | Stmt::While { .. }
            | Stmt::DoWhile { .. }
            | Stmt::With { .. }
            | Stmt::For { .. }
            | Stmt::ForIn { .. }
            | Stmt::ForOf { .. }
            | Stmt::Switch { .. }
            | Stmt::Try { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::VarDecl {
                kind: DeclKind::Var,
                ..
            }
            | Stmt::PatternDecl {
                kind: DeclKind::Var,
                ..
            }
            | Stmt::Expr(_) => false,
        }
    }

    fn invalid_do_while_body(statement: &Statement) -> bool {
        match statement.kind() {
            Stmt::VarDecl {
                kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
                ..
            }
            | Stmt::FunctionDecl { .. } => true,
            Stmt::Label { body, .. } => Self::invalid_do_while_body(body),
            Stmt::Block(_)
            | Stmt::DeclList(_)
            | Stmt::Empty
            | Stmt::If { .. }
            | Stmt::While { .. }
            | Stmt::DoWhile { .. }
            | Stmt::With { .. }
            | Stmt::For { .. }
            | Stmt::ForIn { .. }
            | Stmt::ForOf { .. }
            | Stmt::Switch { .. }
            | Stmt::Try { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::PatternDecl { .. }
            | Stmt::ClassDecl { .. }
            | Stmt::VarDecl {
                kind: DeclKind::Var,
                ..
            }
            | Stmt::Expr(_) => false,
        }
    }

    fn label_statement_start(&mut self) -> bool {
        self.peek_is_identifier_name(0) && self.peek_kind_is(1, &TokenKind::Colon)
    }

    fn label_statement(&mut self) -> Result<Stmt> {
        let labels = self.consume_label_chain()?;
        self.reject_invalid_labeled_item()?;
        let is_iteration_target = self.labeled_item_is_iteration_statement();
        let label_names: Vec<_> = labels.iter().map(|label| label.name.clone()).collect();
        let body =
            self.with_labeled_statement(&label_names, is_iteration_target, Self::statement)?;
        self.reject_invalid_single_statement(&body)?;
        Ok(Self::nest_labeled_statements(labels, body))
    }

    fn consume_label_chain(&mut self) -> Result<Vec<ParsedLabel>> {
        let mut labels = Vec::new();
        loop {
            let start = self.current_span();
            let name = self.consume_identifier("expected label name")?;
            if self.yield_identifier_is_reserved() && name.as_str() == super::YIELD_IDENTIFIER_NAME
            {
                return Err(self.parse_error("yield is not a valid label name"));
            }
            self.consume(&TokenKind::Colon, "expected ':' after label name")?;
            if labels.iter().any(|label: &ParsedLabel| label.name == name) {
                return Err(self.parse_error("duplicate label in labeled statement"));
            }
            labels.push(ParsedLabel { name, start });
            if !self.label_statement_start() {
                break;
            }
        }
        Ok(labels)
    }

    fn reject_invalid_labeled_item(&mut self) -> Result<()> {
        if self.let_starts_disallowed_declaration() || self.check(&TokenKind::Const) {
            return Err(self.parse_error("lexical declaration is not allowed as a label body"));
        }
        if self.check(&TokenKind::Async)
            && self.peek_kind_is_no_line_terminator(1, &TokenKind::Function)
        {
            return Err(
                self.parse_error("async function declaration is not allowed as a label body")
            );
        }
        Ok(())
    }

    fn labeled_item_is_iteration_statement(&mut self) -> bool {
        self.check(&TokenKind::Do) || self.check(&TokenKind::While) || self.check(&TokenKind::For)
    }

    fn nest_labeled_statements(labels: Vec<ParsedLabel>, body: Statement) -> Stmt {
        let mut statement = body;
        for label in labels.into_iter().rev() {
            let span = if let Some(span) = label.start.cover(statement.span()) {
                span
            } else {
                label.start
            };
            statement = Statement::new(
                Stmt::Label {
                    label: label.name,
                    body: Box::new(statement),
                },
                span,
            );
        }
        statement.into_kind()
    }

    fn break_statement(&mut self) -> Result<Stmt> {
        let label = self.optional_jump_label("expected break label")?;
        self.validate_break_statement(label.as_ref())?;
        self.consume_statement_terminator("expected statement terminator after break")?;
        Ok(Stmt::Break(label))
    }

    fn continue_statement(&mut self) -> Result<Stmt> {
        let label = self.optional_jump_label("expected continue label")?;
        self.validate_continue_statement(label.as_ref())?;
        self.consume_statement_terminator("expected statement terminator after continue")?;
        Ok(Stmt::Continue(label))
    }

    fn optional_jump_label(&mut self, message: &str) -> Result<Option<crate::ast::StaticName>> {
        if self.check(&TokenKind::Semicolon)
            || self.check(&TokenKind::RBrace)
            || self.at_end()
            || self.peek_has_line_terminator_before(0)
        {
            return Ok(None);
        }
        if self.next_is_identifier() {
            return self.consume_identifier(message).map(Some);
        }
        Ok(None)
    }

    fn switch_statement(&mut self) -> Result<Stmt> {
        self.consume(&TokenKind::LParen, "expected '(' after 'switch'")?;
        let discriminant = self.expression()?;
        self.consume(&TokenKind::RParen, "expected ')' after switch discriminant")?;
        self.consume(&TokenKind::LBrace, "expected '{' before switch body")?;

        let cases = self.with_switch_statement(Self::switch_cases)?;
        self.validate_generator_switch_declarations(&cases)?;
        Ok(Stmt::Switch {
            discriminant,
            cases,
        })
    }

    fn switch_cases(&mut self) -> Result<Vec<SwitchCase>> {
        let mut cases = Vec::new();
        let mut default_seen = false;
        while !self.check(&TokenKind::RBrace) {
            if self.match_kind(&TokenKind::Case) {
                let test = self.expression()?;
                cases.push(self.switch_case(Some(test))?);
                continue;
            }
            if self.match_kind(&TokenKind::Default) {
                if default_seen {
                    return Err(self.parse_error("switch contains multiple defaults"));
                }
                default_seen = true;
                cases.push(self.switch_case(None)?);
                continue;
            }
            return Err(self.parse_error("expected 'case', 'default', or '}' in switch"));
        }
        self.consume(&TokenKind::RBrace, "expected '}' after switch body")?;
        Ok(cases)
    }

    fn switch_case(&mut self, test: Option<Expression>) -> Result<SwitchCase> {
        self.consume(&TokenKind::Colon, "expected ':' after switch label")?;
        let mut statements = Vec::new();
        while !self.check(&TokenKind::Case)
            && !self.check(&TokenKind::Default)
            && !self.check(&TokenKind::RBrace)
        {
            if self.at_end() {
                return Err(self.parse_error("expected '}' after switch body"));
            }
            let statement = self.with_lexical_function_declarations(Self::statement_list_item)?;
            if Self::direct_resource_declaration(&statement) {
                return Err(self.parse_error(
                    "resource declaration is not allowed directly in a switch clause",
                ));
            }
            statements.push(statement);
        }
        Ok(SwitchCase { test, statements })
    }

    fn direct_resource_declaration(statement: &Statement) -> bool {
        match statement.kind() {
            Stmt::DeclList(declarations) => {
                declarations.iter().any(Self::direct_resource_declaration)
            }
            Stmt::VarDecl { kind, .. } | Stmt::PatternDecl { kind, .. } => kind.is_resource(),
            _ => false,
        }
    }

    fn try_statement(&mut self) -> Result<Stmt> {
        self.consume(&TokenKind::LBrace, "expected '{' after 'try'")?;
        let body = self.block_statements()?;
        let catch = if self.match_kind(&TokenKind::Catch) {
            Some(self.catch_clause()?)
        } else {
            None
        };
        let finally_body = if self.match_kind(&TokenKind::Finally) {
            self.consume(&TokenKind::LBrace, "expected '{' after 'finally'")?;
            Some(self.block_statements()?)
        } else {
            None
        };
        if catch.is_none() && finally_body.is_none() {
            return Err(self.parse_error("expected 'catch' or 'finally' after try block"));
        }
        Ok(Stmt::Try {
            body,
            catch,
            finally_body,
        })
    }

    fn catch_clause(&mut self) -> Result<CatchClause> {
        if self.match_kind(&TokenKind::LBrace) {
            let body = self.block_statements()?;
            return Ok(CatchClause { param: None, body });
        }
        self.consume(&TokenKind::LParen, "expected '(' or '{' after 'catch'")?;
        let param = self.binding_pattern()?;
        let mut names = BTreeSet::new();
        param.for_each_binding(&mut |binding| {
            if !names.insert(binding.name().as_str().to_owned()) {
                return Err(self.parse_error("duplicate catch binding"));
            }
            Ok(())
        })?;
        self.consume(&TokenKind::RParen, "expected ')' after catch binding")?;
        self.consume(&TokenKind::LBrace, "expected '{' after catch binding")?;
        let body = self.block_statements()?;
        Ok(CatchClause {
            param: Some(param),
            body,
        })
    }

    fn throw_statement(&mut self) -> Result<Stmt> {
        if self.peek_has_line_terminator_before(0) {
            return Err(self.parse_error("line terminator is not allowed after 'throw'"));
        }
        let value = self.expression()?;
        self.consume_restricted_statement_terminator(
            "expected statement terminator after throw expression",
        )?;
        Ok(Stmt::Throw(value))
    }

    fn return_statement(&mut self) -> Result<Stmt> {
        if self.function_body_depth == 0 {
            return Err(self.parse_error("return statement outside function"));
        }
        let value = if self.peek_has_line_terminator_before(0)
            || self.check(&TokenKind::Semicolon)
            || self.check(&TokenKind::RBrace)
            || self.at_end()
        {
            None
        } else {
            Some(self.expression()?)
        };
        self.consume_restricted_statement_terminator(
            "expected statement terminator after return statement",
        )?;
        Ok(Stmt::Return(value))
    }

    fn consume_restricted_statement_terminator(&mut self, message: &str) -> Result<()> {
        // Tagged templates are a documented deferred grammar. The expression
        // parser leaves their TemplateHead suffix at the cursor, so it must not
        // be misclassified as a separate statement missing a terminator.
        if matches!(self.peek_kind(0), Some(TokenKind::TemplateHead(_))) {
            return Ok(());
        }
        self.consume_statement_terminator(message)
    }

    fn with_lexical_function_declarations<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let previous = self.function_declaration_context;
        self.function_declaration_context = super::FunctionDeclarationContext::Lexical;
        let result = parse(self);
        self.function_declaration_context = previous;
        result
    }

    fn wrap_single_statement_function(statement: Statement) -> Statement {
        if !matches!(statement.kind(), Stmt::FunctionDecl { .. }) {
            return statement;
        }
        let span = statement.span();
        Statement::new(Stmt::Block(vec![statement]), span)
    }

    pub(super) fn var_decl(&mut self, kind: DeclKind) -> Result<Stmt> {
        let declarations = self.var_declarations(kind)?;
        self.consume_statement_terminator(
            "expected statement terminator after variable declaration",
        )?;
        self.declarations_stmt(declarations)
    }

    fn for_var_decl(&mut self, kind: DeclKind) -> Result<Stmt> {
        let declarations = self.var_declarations(kind)?;
        self.consume(&TokenKind::Semicolon, "expected ';' after for initializer")?;
        self.declarations_stmt(declarations)
    }

    fn var_declarations(&mut self, kind: DeclKind) -> Result<Vec<Statement>> {
        let mut declarations = Vec::new();
        loop {
            declarations.push(self.var_declaration(kind)?);
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }
        Ok(declarations)
    }

    fn var_declaration(&mut self, kind: DeclKind) -> Result<Statement> {
        let start = self.current_span();
        if self.next_is_binding_pattern() {
            let pattern = self.binding_pattern()?;
            self.consume(
                &TokenKind::Equal,
                "destructuring declaration requires an initializer",
            )?;
            let init = self.assignment_expression()?;
            return Ok(self.statement_node(
                start,
                Stmt::PatternDecl {
                    pattern,
                    kind,
                    init,
                },
            ));
        }
        let name = self.consume_binding_identifier("expected binding name")?;
        let init = if self.match_kind(&TokenKind::Equal) {
            Some(self.assignment_expression()?)
        } else if kind.requires_initializer() {
            return Err(self.parse_error("const declaration requires an initializer"));
        } else {
            None
        };
        Ok(self.statement_node(start, Stmt::VarDecl { name, kind, init }))
    }

    fn declarations_stmt(&self, declarations: Vec<Statement>) -> Result<Stmt> {
        if declarations.len() == 1 {
            let mut declarations = declarations.into_iter();
            let Some(declaration) = declarations.next() else {
                return Err(self.parse_error("expected binding declaration"));
            };
            Ok(declaration.into_kind())
        } else {
            Ok(Stmt::DeclList(declarations))
        }
    }
}
