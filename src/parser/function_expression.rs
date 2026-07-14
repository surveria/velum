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
        let name_await_reserved = kind.is_async()
            || (self.await_expression_is_allowed() && self.await_identifier_is_reserved());
        let name = self.with_await_identifier_reserved(name_await_reserved, |parser| {
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
        let ((parameters, body), uses_arguments) =
            self.with_function_arguments_context(|parser| {
                parser.with_new_target_scope(|parser| {
                    parser.with_super_context(false, false, |parser| {
                        let parameters =
                            parser.with_await_context(false, kind.is_async(), |parser| {
                                parser.with_yield_expression(false, |parser| {
                                    parser.with_yield_identifier_reserved(
                                        kind.is_generator(),
                                        Self::function_parameters,
                                    )
                                })
                            })?;
                        parser.consume(
                            &TokenKind::RParen,
                            "expected ')' after function parameters",
                        )?;
                        parser.consume(&TokenKind::LBrace, "expected '{' before function body")?;
                        let body = parser.with_await_context(
                            kind.is_async(),
                            kind.is_async(),
                            |parser| {
                                parser.with_yield_expression(kind.is_generator(), |parser| {
                                    parser.function_body(inherited_strict)
                                })
                            },
                        )?;
                        Ok((parameters, body))
                    })
                })
            })?;
        self.validate_function_parameters(
            &parameters.bound_names,
            parameters.is_simple,
            inherited_strict,
            body.contains_use_strict,
        )?;
        self.validate_function_parameter_lexicals(&parameters.params, &body.statements)?;
        let id = self.static_function()?;
        let strict = inherited_strict || body.contains_use_strict;
        let arguments_binding = if uses_arguments {
            Some(self.implicit_arguments_binding()?)
        } else {
            None
        };
        let mut statements = body.statements;
        Self::suppress_parameter_conflicting_annex_b_bindings(
            &mut statements,
            &parameters.params,
            strict,
        )?;
        let params = parameters.into_params();
        Ok(self.expression_node(
            start,
            Expr::Function {
                id,
                name,
                arguments_binding,
                params: params.into(),
                body: statements.into(),
                kind,
                strict,
            },
        ))
    }
}
