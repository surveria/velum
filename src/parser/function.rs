use crate::{
    ast::{FunctionParam, StaticBinding},
    error::Result,
    lexer::TokenKind,
};

use super::Parser;

/// Parsed parameter list and its early-error metadata.
pub(super) struct ParsedParameters {
    pub(super) params: Vec<FunctionParam>,
    pub(super) bound_names: Vec<StaticBinding>,
    pub(super) is_simple: bool,
}

impl ParsedParameters {
    pub(super) fn into_params(self) -> Vec<FunctionParam> {
        self.params
    }
}

impl Parser {
    pub(super) fn function_parameters(&mut self) -> Result<ParsedParameters> {
        let mut params = Vec::new();
        let mut bound_names = Vec::new();
        let mut is_simple = true;
        if self.check(&TokenKind::RParen) {
            return Ok(ParsedParameters {
                params,
                bound_names,
                is_simple,
            });
        }

        loop {
            if self.check(&TokenKind::RParen) {
                break;
            }
            if self.match_kind(&TokenKind::DotDotDot) {
                is_simple = false;
                self.rest_parameter(&mut params, &mut bound_names)?;
                break;
            }
            if self.next_is_binding_pattern() {
                is_simple = false;
                self.pattern_parameter(&mut params, &mut bound_names)?;
            } else {
                let name = self.consume_binding_identifier("expected function parameter name")?;
                let default = if self.match_kind(&TokenKind::Equal) {
                    is_simple = false;
                    Some(self.assignment()?)
                } else {
                    None
                };
                bound_names.push(name.clone());
                params.push(FunctionParam::new(name, default));
            }
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }

        self.reject_duplicate_non_simple_parameters(&bound_names, is_simple)?;
        Ok(ParsedParameters {
            params,
            bound_names,
            is_simple,
        })
    }

    /// Parses one rest parameter after its consumed `...` marker.
    fn rest_parameter(
        &mut self,
        params: &mut Vec<FunctionParam>,
        bound_names: &mut Vec<StaticBinding>,
    ) -> Result<()> {
        if self.next_is_binding_pattern() {
            let pattern = self.binding_pattern()?;
            Self::collect_parameter_pattern_names(&pattern, bound_names)?;
            params.push(FunctionParam::rest_pattern(pattern));
        } else {
            let name = self.consume_binding_identifier("expected rest parameter name")?;
            bound_names.push(name.clone());
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

    /// Parses one destructuring parameter without lowering it into the body.
    fn pattern_parameter(
        &mut self,
        params: &mut Vec<FunctionParam>,
        bound_names: &mut Vec<StaticBinding>,
    ) -> Result<()> {
        let pattern = self.binding_pattern()?;
        Self::collect_parameter_pattern_names(&pattern, bound_names)?;
        let default = if self.match_kind(&TokenKind::Equal) {
            Some(self.assignment()?)
        } else {
            None
        };
        params.push(FunctionParam::pattern(pattern, default));
        Ok(())
    }

    fn collect_parameter_pattern_names(
        pattern: &crate::ast::BindingPattern,
        names: &mut Vec<StaticBinding>,
    ) -> Result<()> {
        pattern.for_each_binding(&mut |binding| {
            names.push(binding.clone());
            Ok(())
        })
    }
}
