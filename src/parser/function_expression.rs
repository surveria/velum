use crate::{
    ast::{Expr, Expression, FunctionKind},
    error::Result,
    lexer::TokenKind,
};

use super::Parser;

impl Parser {
    pub(super) fn function_expression(&mut self, kind: FunctionKind) -> Result<Expression> {
        let start = self.previous_span();
        let inherited_strict = self.is_strict_mode();
        let name = self.with_await_identifier_reserved(kind.is_async(), |parser| {
            if !parser.next_is_identifier() {
                return Ok(None);
            }
            let name = parser.consume_identifier("expected function name")?;
            if kind.is_generator() && name.as_str() == super::YIELD_IDENTIFIER_NAME {
                return Err(parser.parse_error("yield is not a valid generator expression name"));
            }
            if inherited_strict {
                parser.validate_function_name_in_strict_code(&name)?;
            }
            parser.static_binding(name).map(Some)
        })?;
        self.consume(&TokenKind::LParen, "expected '(' after 'function'")?;
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
        let id = self.static_function()?;
        let (params, statements, parameter_prologue_count) =
            parameters.apply_prologue(body.statements);
        Ok(self.expression_node(
            start,
            Expr::Function {
                id,
                name,
                params: params.into(),
                body: statements.into(),
                parameter_prologue_count,
                kind,
            },
        ))
    }
}
