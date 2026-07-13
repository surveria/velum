use crate::{
    ast::{BinaryOp, DeclKind, Expr, Expression, ForInTarget, Statement, Stmt},
    error::Result,
    lexer::TokenKind,
};

use super::super::Parser;

const FOR_OF_KEYWORD: &str = "of";

/// Distinguishes `for (target in object)` from `for (target of iterable)`
/// after the shared head target has been parsed.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum ForHeadKind {
    In,
    Of,
}

impl Parser {
    pub(super) fn for_statement(&mut self) -> Result<Stmt> {
        let asynchronous = self.match_kind(&TokenKind::Await);
        if asynchronous
            && (!self.await_expression_is_allowed() || !self.await_identifier_is_reserved())
        {
            return Err(self.parse_error("for-await-of is only valid in an async function"));
        }
        self.consume(&TokenKind::LParen, "expected '(' after 'for'")?;
        let cursor = self.cursor;
        let expression_depth = self.expression_depth;
        let static_name_count = self.static_names.len();
        let static_binding_count = self.static_bindings.len();
        let static_function_count = self.static_functions.len();
        let arguments_reference = self.arguments_reference;
        if let Some((target, object, head)) = self.for_in_header(asynchronous)? {
            if asynchronous && head == ForHeadKind::In {
                return Err(self.parse_error("for-await statement requires an 'of' head"));
            }
            self.consume(&TokenKind::RParen, "expected ')' after for-in expression")?;
            let body = Box::new(self.with_iteration_statement(Self::statement)?);
            self.reject_invalid_iteration_statement(&body)?;
            self.validate_for_in_of_declarations(&target, &body)?;
            return Ok(match head {
                ForHeadKind::In => Stmt::ForIn {
                    target,
                    object,
                    body,
                },
                ForHeadKind::Of if asynchronous => Stmt::ForOf {
                    target,
                    object,
                    body,
                    asynchronous: true,
                },
                ForHeadKind::Of => Stmt::ForOf {
                    target,
                    object,
                    body,
                    asynchronous: false,
                },
            });
        }
        if asynchronous {
            return Err(self.parse_error("expected 'of' in for-await-of statement"));
        }
        self.cursor = cursor;
        self.expression_depth = expression_depth;
        let offset = self.offset();
        self.static_names.rollback_to(static_name_count, offset)?;
        self.static_bindings
            .rollback_to(static_binding_count, offset)?;
        self.static_functions
            .rollback_to(static_function_count, offset)?;
        self.arguments_reference = arguments_reference;

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
        self.reject_invalid_iteration_statement(&body)?;
        self.validate_for_declarations(init.as_deref(), &body)?;
        Ok(Stmt::For {
            init,
            condition,
            update,
            body,
        })
    }

    fn for_in_header(
        &mut self,
        asynchronous: bool,
    ) -> Result<Option<(ForInTarget, Expression, ForHeadKind)>> {
        if self.async_of_arrow_starts_classic_for() {
            return Ok(None);
        }
        if self.await_using_declaration_start() {
            self.consume(&TokenKind::Await, "expected 'await'")?;
            self.consume_contextual_using()?;
            return self.for_resource_binding_header(DeclKind::AwaitUsing);
        }
        if self.using_declaration_start() && !self.using_of_lookahead() {
            self.consume_contextual_using()?;
            return self.for_resource_binding_header(DeclKind::Using);
        }
        if self.check(&TokenKind::Let)
            && (self.is_strict_mode() || self.let_declaration_lookahead())
            && self.advance().is_some()
        {
            return self.for_in_binding_header(DeclKind::Let);
        }
        if self.match_kind(&TokenKind::Const) {
            return self.for_in_binding_header(DeclKind::Const);
        }
        if self.match_kind(&TokenKind::Var) {
            return self.for_in_binding_header(DeclKind::Var);
        }

        if self.for_in_assignment_pattern_start() {
            let pattern = self.assignment_pattern()?;
            let Some(head) = self.match_for_head_kind() else {
                return Ok(None);
            };
            let object = self.for_head_rhs(head)?;
            return Ok(Some((
                ForInTarget::PatternAssignment {
                    pattern: Box::new(pattern),
                    strict: self.is_strict_mode(),
                },
                object,
                head,
            )));
        }

        if !self.for_in_assignment_target_start() {
            return Ok(None);
        }
        let bare_async_target = self.check(&TokenKind::Async)
            && self
                .peek_token(1)
                .is_some_and(|token| token.is_unescaped_identifier_named(FOR_OF_KEYWORD));
        let target = self.call()?;
        let Some(head) = self.match_for_head_kind() else {
            return Ok(None);
        };
        let Some(target) = self.assignment_target(target) else {
            return Err(self.parse_error("invalid for-in assignment target"));
        };
        if !asynchronous && head == ForHeadKind::Of && bare_async_target {
            return Err(self.parse_error("async is not a valid for-of assignment target"));
        }
        let object = self.for_head_rhs(head)?;
        Ok(Some((
            ForInTarget::Assignment {
                target,
                strict: self.is_strict_mode(),
            },
            object,
            head,
        )))
    }

    fn for_resource_binding_header(
        &mut self,
        kind: DeclKind,
    ) -> Result<Option<(ForInTarget, Expression, ForHeadKind)>> {
        let name = self.consume_binding_identifier("expected resource binding name")?;
        let Some(head) = self.match_for_head_kind() else {
            return Ok(None);
        };
        if head == ForHeadKind::In {
            return Err(self.parse_error("resource declarations are not allowed in for-in heads"));
        }
        let object = self.for_head_rhs(head)?;
        Ok(Some((ForInTarget::Binding { name, kind }, object, head)))
    }

    fn using_of_lookahead(&mut self) -> bool {
        self.peek_token(1)
            .is_some_and(|token| token.is_unescaped_identifier_named(FOR_OF_KEYWORD))
            && self
                .peek_token(2)
                .is_some_and(|token| token.is_unescaped_identifier_named(FOR_OF_KEYWORD))
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
        let name =
            self.consume_declaration_binding_identifier(kind, "expected for-in binding name")?;
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

    fn next_is_contextual_of(&mut self) -> bool {
        self.peek()
            .is_some_and(|token| token.is_unescaped_identifier_named(FOR_OF_KEYWORD))
    }

    fn for_in_assignment_target_start(&mut self) -> bool {
        self.peek().is_some_and(|token| {
            matches!(
                &token.kind,
                TokenKind::Identifier(_)
                    | TokenKind::Let
                    | TokenKind::Async
                    | TokenKind::This
                    | TokenKind::Super
                    | TokenKind::LBracket
                    | TokenKind::LParen
            )
        })
    }

    fn async_of_arrow_starts_classic_for(&mut self) -> bool {
        self.peek_kind_is(0, &TokenKind::Async)
            && self
                .peek_token(1)
                .is_some_and(|token| token.is_unescaped_identifier_named(FOR_OF_KEYWORD))
            && self.peek_kind_is_no_line_terminator(2, &TokenKind::Arrow)
    }

    fn for_in_assignment_pattern_start(&mut self) -> bool {
        let Some(closing) = self.outer_literal_closing_offset() else {
            return false;
        };
        matches!(
            self.peek_kind(closing.saturating_add(1)),
            Some(TokenKind::In)
        ) || self
            .peek_token(closing.saturating_add(1))
            .is_some_and(|token| token.is_unescaped_identifier_named(FOR_OF_KEYWORD))
    }

    fn for_init(&mut self) -> Result<Option<Box<Statement>>> {
        let start = self.current_span();
        if self.match_kind(&TokenKind::Semicolon) {
            return Ok(None);
        }
        if self.starts_private_in_expression() {
            return Err(self.parse_error(
                "private brand checks are not allowed in an unparenthesized for initializer",
            ));
        }
        if self.check(&TokenKind::Let)
            && (self.is_strict_mode() || self.let_declaration_lookahead())
            && self.advance().is_some()
        {
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
        if self.await_using_declaration_start() {
            self.consume(&TokenKind::Await, "expected 'await'")?;
            self.consume_contextual_using()?;
            let declarations = self.resource_declarations(DeclKind::AwaitUsing)?;
            self.consume(&TokenKind::Semicolon, "expected ';' after for initializer")?;
            let kind = self.declarations_stmt(declarations)?;
            return Ok(Some(Box::new(self.statement_node(start, kind))));
        }
        if self.using_declaration_start() {
            self.consume_contextual_using()?;
            let declarations = self.resource_declarations(DeclKind::Using)?;
            self.consume(&TokenKind::Semicolon, "expected ';' after for initializer")?;
            let kind = self.declarations_stmt(declarations)?;
            return Ok(Some(Box::new(self.statement_node(start, kind))));
        }
        let expr = self.expression()?;
        if matches!(
            expr.kind(),
            Expr::Binary {
                op: BinaryOp::In,
                ..
            }
        ) {
            return Err(
                self.parse_error("unparenthesized 'in' is not allowed in a for initializer")
            );
        }
        self.consume(&TokenKind::Semicolon, "expected ';' after for initializer")?;
        Ok(Some(Box::new(self.statement_node(start, Stmt::Expr(expr)))))
    }
}
