use crate::{
    ast::{
        CatchClause, DeclKind, Expression, ForInTarget, FunctionKind, Statement, Stmt, SwitchCase,
    },
    error::{Error, Result},
    lexer::TokenKind,
};

use super::{ParsedFunctionBody, Parser};

const FOR_OF_KEYWORD: &str = "of";

/// Distinguishes `for (target in object)` from `for (target of iterable)`
/// after the shared head target has been parsed.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum ForHeadKind {
    In,
    Of,
}

struct ParsedLabel {
    name: crate::ast::StaticName,
    start: crate::SourceSpan,
}

impl Parser {
    pub(super) fn statement(&mut self) -> Result<Statement> {
        let start = self.current_span();
        let kind = self.with_statement_depth(Self::statement_inner)?;
        Ok(self.statement_node(start, kind))
    }

    fn statement_inner(&mut self) -> Result<Stmt> {
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
            statements.push(self.statement()?);
        }
        self.consume(&TokenKind::RBrace, "expected '}' after block")?;
        self.validate_generator_block_declarations(&statements)?;
        Ok(statements)
    }

    fn block(&mut self) -> Result<Stmt> {
        Ok(Stmt::Block(self.block_statements()?))
    }

    pub(super) fn function_body(&mut self, inherited_strict: bool) -> Result<ParsedFunctionBody> {
        let previous_strict = self.is_strict_mode();
        self.set_strict_mode(inherited_strict);
        let result = self.function_body_inner();
        self.set_strict_mode(previous_strict);
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
            let statement = self.statement()?;
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
        let consequent = Box::new(self.statement()?);
        self.reject_generator_single_statement(&consequent)?;
        let alternate = if self.match_kind(&TokenKind::Else) {
            let alternate = Box::new(self.statement()?);
            self.reject_generator_single_statement(&alternate)?;
            Some(alternate)
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
        self.reject_generator_single_statement(&body)?;
        Ok(Stmt::While { condition, body })
    }

    fn do_while_statement(&mut self) -> Result<Stmt> {
        if self.check(&TokenKind::Let)
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

    fn invalid_do_while_body(statement: &Statement) -> bool {
        match statement.kind() {
            Stmt::VarDecl {
                kind: DeclKind::Let | DeclKind::Const,
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

    fn label_statement_start(&self) -> bool {
        self.peek_is_identifier_name(0) && self.peek_kind_is(1, &TokenKind::Colon)
    }

    fn label_statement(&mut self) -> Result<Stmt> {
        let labels = self.consume_label_chain()?;
        self.reject_invalid_labeled_item()?;
        let is_iteration_target = self.labeled_item_is_iteration_statement();
        let label_names: Vec<_> = labels.iter().map(|label| label.name.clone()).collect();
        let body =
            self.with_labeled_statement(&label_names, is_iteration_target, Self::statement)?;
        self.reject_generator_single_statement(&body)?;
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

    fn reject_invalid_labeled_item(&self) -> Result<()> {
        if self.check(&TokenKind::Let) || self.check(&TokenKind::Const) {
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

    fn labeled_item_is_iteration_statement(&self) -> bool {
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

    fn for_statement(&mut self) -> Result<Stmt> {
        self.consume(&TokenKind::LParen, "expected '(' after 'for'")?;
        let cursor = self.cursor;
        let expression_depth = self.expression_depth;
        let static_names = self.static_names.clone();
        let static_bindings = self.static_bindings.clone();
        let static_functions = self.static_functions.clone();
        if let Some((target, object, head)) = self.for_in_header()? {
            self.consume(&TokenKind::RParen, "expected ')' after for-in expression")?;
            let body = Box::new(self.with_iteration_statement(Self::statement)?);
            self.reject_generator_single_statement(&body)?;
            return Ok(match head {
                ForHeadKind::In => Stmt::ForIn {
                    target,
                    object,
                    body,
                },
                ForHeadKind::Of => Stmt::ForOf {
                    target,
                    object,
                    body,
                },
            });
        }
        self.cursor = cursor;
        self.expression_depth = expression_depth;
        self.static_names = static_names;
        self.static_bindings = static_bindings;
        self.static_functions = static_functions;

        let init = self.for_init()?;
        let condition = if self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.expression()?)
        };
        self.consume(&TokenKind::Semicolon, "expected ';' after for condition")?;
        let update = if self.check(&TokenKind::RParen) {
            None
        } else {
            Some(self.expression()?)
        };
        self.consume(&TokenKind::RParen, "expected ')' after for clauses")?;
        let body = Box::new(self.with_iteration_statement(Self::statement)?);
        self.reject_generator_single_statement(&body)?;
        Ok(Stmt::For {
            init,
            condition,
            update,
            body,
        })
    }

    fn for_in_header(&mut self) -> Result<Option<(ForInTarget, Expression, ForHeadKind)>> {
        if self.match_kind(&TokenKind::Let) {
            return self.for_in_binding_header(DeclKind::Let);
        }
        if self.match_kind(&TokenKind::Const) {
            return self.for_in_binding_header(DeclKind::Const);
        }
        if self.match_kind(&TokenKind::Var) {
            return self.for_in_binding_header(DeclKind::Var);
        }

        if !self.for_in_assignment_target_start() {
            return Ok(None);
        }
        let target = self.call()?;
        let Some(head) = self.match_for_head_kind() else {
            return Ok(None);
        };
        let Some(target) = Self::assignment_target(target) else {
            return Err(self.parse_error("invalid for-in assignment target"));
        };
        let object = self.for_head_rhs(head)?;
        Ok(Some((ForInTarget::Assignment(target), object, head)))
    }

    fn for_in_binding_header(
        &mut self,
        kind: DeclKind,
    ) -> Result<Option<(ForInTarget, Expression, ForHeadKind)>> {
        if self.next_is_binding_pattern() {
            let pattern = self.binding_pattern()?;
            let Some(head) = self.match_for_head_kind() else {
                return Ok(None);
            };
            let object = self.for_head_rhs(head)?;
            let target = ForInTarget::PatternBinding {
                pattern: Box::new(pattern),
                kind,
            };
            return Ok(Some((target, object, head)));
        }
        let name = self.consume_binding_identifier("expected for-in binding name")?;
        let Some(head) = self.match_for_head_kind() else {
            return Ok(None);
        };
        let object = self.for_head_rhs(head)?;
        Ok(Some((ForInTarget::Binding { name, kind }, object, head)))
    }

    fn for_head_rhs(&mut self, head: ForHeadKind) -> Result<Expression> {
        match head {
            ForHeadKind::In => self.expression(),
            ForHeadKind::Of => self.assignment_expression(),
        }
    }

    fn match_for_head_kind(&mut self) -> Option<ForHeadKind> {
        if self.match_kind(&TokenKind::In) {
            return Some(ForHeadKind::In);
        }
        if self.next_is_contextual_of() && self.advance().is_some() {
            return Some(ForHeadKind::Of);
        }
        None
    }

    fn next_is_contextual_of(&self) -> bool {
        self.peek().is_some_and(
            |token| matches!(&token.kind, TokenKind::Identifier(name) if name == FOR_OF_KEYWORD),
        )
    }

    fn for_in_assignment_target_start(&self) -> bool {
        self.peek().is_some_and(|token| {
            matches!(
                &token.kind,
                TokenKind::Identifier(_) | TokenKind::Async | TokenKind::LParen
            )
        })
    }

    fn for_init(&mut self) -> Result<Option<Box<Statement>>> {
        let start = self.current_span();
        if self.match_kind(&TokenKind::Semicolon) {
            return Ok(None);
        }
        if self.match_kind(&TokenKind::Let) {
            let kind = self.for_var_decl(DeclKind::Let)?;
            return Ok(Some(Box::new(self.statement_node(start, kind))));
        }
        if self.match_kind(&TokenKind::Const) {
            let kind = self.for_var_decl(DeclKind::Const)?;
            return Ok(Some(Box::new(self.statement_node(start, kind))));
        }
        if self.match_kind(&TokenKind::Var) {
            let kind = self.for_var_decl(DeclKind::Var)?;
            return Ok(Some(Box::new(self.statement_node(start, kind))));
        }
        let expr = self.expression()?;
        self.consume(&TokenKind::Semicolon, "expected ';' after for initializer")?;
        Ok(Some(Box::new(self.statement_node(start, Stmt::Expr(expr)))))
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
            statements.push(self.statement()?);
        }
        Ok(SwitchCase { test, statements })
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
        let param = self.consume_binding_identifier("expected catch binding name")?;
        self.consume(&TokenKind::RParen, "expected ')' after catch binding")?;
        self.consume(&TokenKind::LBrace, "expected '{' after catch binding")?;
        let body = self.block_statements()?;
        Ok(CatchClause {
            param: Some(param),
            body,
        })
    }

    fn throw_statement(&mut self) -> Result<Stmt> {
        let value = self.expression()?;
        self.consume_optional_semicolon();
        Ok(Stmt::Throw(value))
    }

    fn return_statement(&mut self) -> Result<Stmt> {
        let value =
            if self.check(&TokenKind::Semicolon) || self.check(&TokenKind::RBrace) || self.at_end()
            {
                None
            } else {
                Some(self.expression()?)
            };
        self.consume_optional_semicolon();
        Ok(Stmt::Return(value))
    }

    fn function_declaration(&mut self, kind: FunctionKind) -> Result<Stmt> {
        let name_await_reserved = kind.is_async() || self.await_identifier_is_reserved();
        let name = self.with_await_identifier_reserved(name_await_reserved, |parser| {
            parser.consume_binding_identifier("expected function declaration name")
        })?;
        let inherited_strict = self.is_strict_mode();
        if inherited_strict {
            self.validate_function_binding_in_strict_code(&name)?;
        }
        self.consume(&TokenKind::LParen, "expected '(' after function name")?;
        let parameters = self.with_await_context(false, kind.is_async(), |parser| {
            parser.with_yield_expression(false, |parser| {
                parser
                    .with_yield_identifier_reserved(kind.is_generator(), Self::function_parameters)
            })
        })?;
        self.consume(&TokenKind::RParen, "expected ')' after function parameters")?;
        self.consume(&TokenKind::LBrace, "expected '{' before function body")?;
        let body = self.with_new_target_scope(|parser| {
            parser.with_super_context(false, false, |parser| {
                parser.with_await_context(kind.is_async(), kind.is_async(), |parser| {
                    parser.with_yield_expression(kind.is_generator(), |parser| {
                        parser.function_body(inherited_strict)
                    })
                })
            })
        })?;
        self.validate_function_parameters(
            &parameters.params,
            parameters.is_simple,
            inherited_strict,
            body.contains_use_strict,
        )?;
        if kind.is_generator() {
            self.validate_generator_parameter_lexicals(&parameters.params, &body.statements)?;
        }
        let id = self.static_function()?;
        let (params, statements, parameter_prologue_count) =
            parameters.apply_prologue(body.statements);
        Ok(Stmt::FunctionDecl {
            name,
            id,
            params: params.into(),
            body: statements.into(),
            parameter_prologue_count,
            kind,
        })
    }

    fn var_decl(&mut self, kind: DeclKind) -> Result<Stmt> {
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
        } else if kind == DeclKind::Const {
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
