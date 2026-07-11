use crate::{
    ast::{Expr, Expression, FunctionParam, Statement, Stmt},
    error::Result,
    lexer::TokenKind,
    syntax::DeclKind,
};

use super::Parser;

/// Prefix for synthesized parameter names that hold a destructured argument
/// before the body-prologue pattern declaration unpacks it. The `%` characters
/// keep the name outside the user identifier space.
const PATTERN_PARAM_NAME_PREFIX: &str = "%pattern";
const PATTERN_PARAM_NAME_SUFFIX: &str = "%";

/// Prefix for the synthesized rest parameter that holds the packed argument
/// array before a body-prologue pattern declaration unpacks it.
const REST_PATTERN_PARAM_NAME: &str = "%rest%";

/// Parsed parameter list plus the pattern-unpacking statements that must run
/// before the function body.
pub(super) struct ParsedParameters {
    pub(super) params: Vec<FunctionParam>,
    pub(super) pattern_prologue: Vec<Statement>,
    pub(super) is_simple: bool,
}

impl ParsedParameters {
    /// Prepends the pattern-unpacking prologue to the parsed body statements.
    pub(super) fn apply_prologue(
        self,
        mut body: Vec<Statement>,
    ) -> (Vec<FunctionParam>, Vec<Statement>) {
        if self.pattern_prologue.is_empty() {
            return (self.params, body);
        }
        let mut statements = self.pattern_prologue;
        statements.append(&mut body);
        (self.params, statements)
    }
}

impl Parser {
    pub(super) fn function_parameters(&mut self) -> Result<ParsedParameters> {
        let mut params = Vec::new();
        let mut pattern_prologue = Vec::new();
        let mut is_simple = true;
        if self.check(&TokenKind::RParen) {
            return Ok(ParsedParameters {
                params,
                pattern_prologue,
                is_simple,
            });
        }

        loop {
            if self.check(&TokenKind::RParen) {
                break;
            }
            if self.match_kind(&TokenKind::DotDotDot) {
                is_simple = false;
                self.rest_parameter(&mut params, &mut pattern_prologue)?;
                break;
            }
            if self.next_is_binding_pattern() {
                is_simple = false;
                self.pattern_parameter(&mut params, &mut pattern_prologue)?;
            } else {
                let name = self.consume_binding_identifier("expected function parameter name")?;
                let default = if self.match_kind(&TokenKind::Equal) {
                    is_simple = false;
                    Some(self.assignment()?)
                } else {
                    None
                };
                params.push(FunctionParam::new(name, default));
            }
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }

        self.reject_duplicate_non_simple_parameters(&params, is_simple)?;
        Ok(ParsedParameters {
            params,
            pattern_prologue,
            is_simple,
        })
    }

    /// Parses one rest parameter after its consumed `...` marker: identifier
    /// rests bind directly, pattern rests bind through a synthesized parameter
    /// plus a body-prologue pattern declaration.
    fn rest_parameter(
        &mut self,
        params: &mut Vec<FunctionParam>,
        pattern_prologue: &mut Vec<Statement>,
    ) -> Result<()> {
        let start = self.previous_span();
        if self.next_is_binding_pattern() {
            let pattern = self.binding_pattern()?;
            let synthetic = self.static_binding_name(REST_PATTERN_PARAM_NAME.to_owned())?;
            params.push(FunctionParam::rest(synthetic.clone()));
            let span = self.span_since(start);
            pattern_prologue.push(Statement::new(
                Stmt::PatternDecl {
                    pattern,
                    kind: DeclKind::Var,
                    init: Expression::new(Expr::Identifier(synthetic), span),
                },
                span,
            ));
        } else {
            let name = self.consume_binding_identifier("expected rest parameter name")?;
            params.push(FunctionParam::rest(name));
        }
        if self.check(&TokenKind::Equal) {
            return Err(self.parse_error("rest parameter cannot have a default value"));
        }
        if self.check(&TokenKind::Comma) {
            return Err(self.parse_error("rest parameter must be the last parameter"));
        }
        Ok(())
    }

    /// Parses one destructuring parameter as a synthesized plain parameter
    /// plus a body-prologue `var` pattern declaration that unpacks it.
    fn pattern_parameter(
        &mut self,
        params: &mut Vec<FunctionParam>,
        pattern_prologue: &mut Vec<Statement>,
    ) -> Result<()> {
        let start = self.current_span();
        let pattern = self.binding_pattern()?;
        let default = if self.match_kind(&TokenKind::Equal) {
            Some(self.assignment()?)
        } else {
            None
        };
        let synthetic_name = format!(
            "{PATTERN_PARAM_NAME_PREFIX}{}{PATTERN_PARAM_NAME_SUFFIX}",
            params.len()
        );
        let synthetic = self.static_binding_name(synthetic_name)?;
        params.push(FunctionParam::new(synthetic.clone(), default));
        let span = self.span_since(start);
        pattern_prologue.push(Statement::new(
            Stmt::PatternDecl {
                pattern,
                kind: DeclKind::Var,
                init: Expression::new(Expr::Identifier(synthetic), span),
            },
            span,
        ));
        Ok(())
    }
}
