use std::collections::BTreeSet;

use crate::{
    ast::{FunctionKind, FunctionParam, Statement, Stmt},
    error::Result,
    lexer::TokenKind,
    syntax::StaticNameId,
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
            kind,
            strict,
        })
    }

    pub(in crate::parser) fn suppress_parameter_conflicting_annex_b_bindings(
        statements: &mut [Statement],
        params: &[FunctionParam],
        strict: bool,
    ) -> Result<()> {
        if strict {
            return Ok(());
        }
        let mut excluded_names = BTreeSet::new();
        for param in params {
            param.target.for_each_binding(&mut |binding| {
                excluded_names.insert(binding.name().id());
                Ok::<(), crate::Error>(())
            })?;
        }
        Self::suppress_annex_b_bindings(statements, &excluded_names);
        Ok(())
    }

    fn suppress_annex_b_bindings(
        statements: &mut [Statement],
        excluded_names: &BTreeSet<StaticNameId>,
    ) {
        for statement in statements {
            Self::suppress_annex_b_binding(statement, excluded_names);
        }
    }

    fn suppress_annex_b_binding(
        statement: &mut Statement,
        excluded_names: &BTreeSet<StaticNameId>,
    ) {
        match statement.kind_mut() {
            Stmt::FunctionDecl {
                name,
                annex_b_var_binding,
                ..
            } => {
                if excluded_names.contains(&name.name().id()) {
                    *annex_b_var_binding = None;
                }
            }
            Stmt::Block(statements) | Stmt::DeclList(statements) => {
                Self::suppress_annex_b_bindings(statements, excluded_names);
            }
            Stmt::If {
                consequent,
                alternate,
                ..
            } => {
                Self::suppress_annex_b_binding(consequent, excluded_names);
                if let Some(alternate) = alternate {
                    Self::suppress_annex_b_binding(alternate, excluded_names);
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::With { body, .. }
            | Stmt::Label { body, .. }
            | Stmt::For { body, .. }
            | Stmt::ForIn { body, .. }
            | Stmt::ForOf { body, .. } => {
                Self::suppress_annex_b_binding(body, excluded_names);
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    Self::suppress_annex_b_bindings(&mut case.statements, excluded_names);
                }
            }
            Stmt::Try {
                body,
                catch,
                finally_body,
            } => {
                Self::suppress_annex_b_bindings(body, excluded_names);
                if let Some(catch) = catch {
                    Self::suppress_annex_b_bindings(&mut catch.body, excluded_names);
                }
                if let Some(finally_body) = finally_body {
                    Self::suppress_annex_b_bindings(finally_body, excluded_names);
                }
            }
            Stmt::Empty
            | Stmt::Debugger
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::VarDecl { .. }
            | Stmt::ImportBinding { .. }
            | Stmt::PatternDecl { .. }
            | Stmt::ClassDecl { .. }
            | Stmt::Expr(_) => {}
        }
    }
}
