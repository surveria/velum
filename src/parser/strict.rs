use crate::ast::{Expr, FunctionParam, Statement, StaticBinding, StaticName, Stmt};
use crate::error::Result;

use super::{
    ARGUMENTS_IDENTIFIER_NAME, EVAL_IDENTIFIER_NAME, Parser, USE_STRICT_DIRECTIVE,
    YIELD_IDENTIFIER_NAME,
};

impl Parser {
    pub(super) fn update_directive_prologue(
        &mut self,
        directive_prologue: &mut bool,
        statement: &Statement,
    ) {
        if !*directive_prologue {
            return;
        }
        if Self::is_use_strict_directive(statement) {
            self.set_strict_mode(true);
        }
        if !Self::is_string_directive(statement) {
            *directive_prologue = false;
        }
    }

    pub(super) fn validate_function_name_in_strict_code(&self, name: &StaticName) -> Result<()> {
        self.reject_restricted_strict_name(name.as_str())
    }

    pub(super) fn validate_function_binding_in_strict_code(
        &self,
        name: &StaticBinding,
    ) -> Result<()> {
        self.reject_restricted_strict_name(name.as_str())
    }

    pub(super) fn validate_function_parameters(
        &self,
        params: &[FunctionParam],
        parameters_are_simple: bool,
        inherited_strict: bool,
        body_contains_use_strict: bool,
    ) -> Result<()> {
        if body_contains_use_strict && !parameters_are_simple {
            return Err(
                self.parse_error("use strict directive is not allowed with non-simple parameters")
            );
        }

        if inherited_strict || body_contains_use_strict {
            self.reject_duplicate_parameters(params)?;
            for param in params {
                self.reject_restricted_strict_name(param.name.as_str())?;
            }
        }

        Ok(())
    }

    pub(super) fn reject_duplicate_non_simple_parameters(
        &self,
        params: &[FunctionParam],
        parameters_are_simple: bool,
    ) -> Result<()> {
        if !parameters_are_simple {
            self.reject_duplicate_parameters(params)?;
        }
        Ok(())
    }

    pub(super) fn reject_duplicate_parameters(&self, params: &[FunctionParam]) -> Result<()> {
        let mut seen = Vec::new();
        for param in params {
            let name = param.name.as_str();
            if seen.contains(&name) {
                return Err(self.parse_error("duplicate parameter name"));
            }
            seen.push(name);
        }
        Ok(())
    }

    fn reject_restricted_strict_name(&self, name: &str) -> Result<()> {
        if Self::is_restricted_strict_name(name) {
            return Err(self.parse_error("eval and arguments are not valid strict binding names"));
        }
        Ok(())
    }

    pub(super) fn validate_strict_identifier_reference(&self, name: &str) -> Result<()> {
        if self.is_strict_mode() && name == YIELD_IDENTIFIER_NAME {
            return Err(self.parse_error("yield is not a valid strict identifier reference"));
        }
        Ok(())
    }

    fn string_directive_value(statement: &Statement) -> Option<&str> {
        let Stmt::Expr(expression) = statement.kind() else {
            return None;
        };
        let Expr::StringLiteral(value) = expression.kind() else {
            return None;
        };
        Some(value.as_str())
    }

    fn is_string_directive(statement: &Statement) -> bool {
        Self::string_directive_value(statement).is_some()
    }

    pub(super) fn is_use_strict_directive(statement: &Statement) -> bool {
        Self::string_directive_value(statement).is_some_and(|value| value == USE_STRICT_DIRECTIVE)
    }

    fn is_restricted_strict_name(name: &str) -> bool {
        matches!(
            name,
            EVAL_IDENTIFIER_NAME | ARGUMENTS_IDENTIFIER_NAME | YIELD_IDENTIFIER_NAME
        )
    }
}
