use crate::{
    ast::{DeclKind, Statement, Stmt},
    error::Result,
    lexer::TokenKind,
};

use super::super::Parser;

const USING_IDENTIFIER_NAME: &str = "using";

impl Parser {
    pub(super) fn using_declaration_start(&self) -> bool {
        self.contextual_using_at(0)
            && !self.peek_has_line_terminator_before(1)
            && self.resource_binding_starts_at(1)
    }

    pub(super) fn await_using_declaration_start(&self) -> bool {
        self.await_identifier_is_reserved()
            && self.check(&TokenKind::Await)
            && !self.peek_has_line_terminator_before(1)
            && self.contextual_using_at(1)
            && !self.peek_has_line_terminator_before(2)
            && self.resource_binding_starts_at(2)
    }

    fn resource_binding_starts_at(&self, offset: usize) -> bool {
        self.peek_token(offset).is_some_and(|token| {
            matches!(
                &token.kind,
                TokenKind::Identifier(_) | TokenKind::Async | TokenKind::Await
            )
        })
    }

    fn contextual_using_at(&self, offset: usize) -> bool {
        self.peek_token(offset).is_some_and(|token| {
            !token.identifier_escaped
                && matches!(&token.kind, TokenKind::Identifier(name) if name == USING_IDENTIFIER_NAME)
        })
    }

    pub(super) fn consume_contextual_using(&mut self) -> Result<()> {
        if !self.contextual_using_at(0) {
            return Err(self.parse_error("expected contextual 'using' keyword"));
        }
        let _token = self.advance_token("expected contextual 'using' keyword")?;
        Ok(())
    }

    pub(super) fn resource_decl(&mut self, kind: DeclKind) -> Result<Stmt> {
        if self.statement_depth <= 1 {
            return Err(self.parse_error(
                "resource declarations are not allowed at the top level of a script",
            ));
        }
        let declarations = self.resource_declarations(kind)?;
        self.consume_statement_terminator(
            "expected statement terminator after resource declaration",
        )?;
        self.declarations_stmt(declarations)
    }

    pub(super) fn resource_declarations(&mut self, kind: DeclKind) -> Result<Vec<Statement>> {
        let mut declarations = Vec::new();
        loop {
            let start = self.current_span();
            let name = self.consume_binding_identifier("expected resource binding name")?;
            self.consume(
                &TokenKind::Equal,
                "resource declaration requires an initializer",
            )?;
            let init = self.assignment_expression()?;
            declarations.push(self.statement_node(
                start,
                Stmt::VarDecl {
                    name,
                    kind,
                    init: Some(init),
                },
            ));
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }
        Ok(declarations)
    }
}
