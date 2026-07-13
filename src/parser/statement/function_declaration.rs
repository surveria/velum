use crate::{
    ast::{FunctionKind, Stmt},
    error::Result,
    lexer::TokenKind,
};

use super::super::Parser;

impl Parser {
    pub(in crate::parser) fn function_declaration(&mut self, kind: FunctionKind) -> Result<Stmt> {
        let name_await_reserved = kind.is_async() || self.await_identifier_is_reserved();
        let name = self.with_await_identifier_reserved(name_await_reserved, |parser| {
            parser.consume_binding_identifier("expected function declaration name")
        })?;
        let inherited_strict = self.is_strict_mode();
        if inherited_strict {
            self.validate_function_binding_in_strict_code(&name)?;
        }
        self.consume(&TokenKind::LParen, "expected '(' after function name")?;
        let ((parameters, body), uses_arguments) =
            self.with_function_arguments_context(|parser| {
                let parameters = parser.with_await_context(false, kind.is_async(), |parser| {
                    parser.with_yield_expression(false, |parser| {
                        parser.with_yield_identifier_reserved(
                            kind.is_generator(),
                            Self::function_parameters,
                        )
                    })
                })?;
                parser.consume(&TokenKind::RParen, "expected ')' after function parameters")?;
                parser.consume(&TokenKind::LBrace, "expected '{' before function body")?;
                let body = parser.with_new_target_scope(|parser| {
                    parser.with_super_context(false, false, |parser| {
                        parser.with_await_context(kind.is_async(), kind.is_async(), |parser| {
                            parser.with_yield_expression(kind.is_generator(), |parser| {
                                parser.function_body(inherited_strict)
                            })
                        })
                    })
                })?;
                Ok((parameters, body))
            })?;
        self.validate_function_parameters(
            &parameters.bound_names,
            parameters.is_simple,
            inherited_strict,
            body.contains_use_strict,
        )?;
        if kind.is_generator() {
            self.validate_generator_parameter_lexicals(&parameters.params, &body.statements)?;
        }
        let id = self.static_function()?;
        let strict = inherited_strict || body.contains_use_strict;
        let arguments_binding = if uses_arguments {
            Some(self.implicit_arguments_binding()?)
        } else {
            None
        };
        let (params, statements, parameter_prologue_count) =
            parameters.apply_prologue(body.statements);
        let block_scoped =
            self.function_declaration_context == super::super::FunctionDeclarationContext::Lexical;
        let annex_b_var_binding =
            if block_scoped && !self.is_strict_mode() && kind == FunctionKind::Ordinary {
                Some(self.static_binding(name.name().clone())?)
            } else {
                None
            };
        Ok(Stmt::FunctionDecl {
            name,
            block_scoped,
            annex_b_var_binding,
            arguments_binding,
            id,
            params: params.into(),
            body: statements.into(),
            parameter_prologue_count,
            kind,
            strict,
        })
    }
}
