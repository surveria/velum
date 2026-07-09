use crate::ast::{Expr, FunctionParam, StaticBinding, StaticName, Stmt};
use crate::error::{Error, Result};

use super::{ARGUMENTS_IDENTIFIER_NAME, EVAL_IDENTIFIER_NAME, Parser, USE_STRICT_DIRECTIVE};

impl Parser {
    pub(super) fn update_directive_prologue(
        &mut self,
        directive_prologue: &mut bool,
        statement: &Stmt,
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
        inherited_strict: bool,
        body_contains_use_strict: bool,
    ) -> Result<()> {
        if body_contains_use_strict && Self::parameter_list_is_non_simple(params) {
            return Err(Error::parse(
                "use strict directive is not allowed with non-simple parameters",
                self.offset(),
            ));
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
    ) -> Result<()> {
        if Self::parameter_list_is_non_simple(params) {
            self.reject_duplicate_parameters(params)?;
        }
        Ok(())
    }

    fn reject_duplicate_parameters(&self, params: &[FunctionParam]) -> Result<()> {
        let mut seen = Vec::new();
        for param in params {
            let name = param.name.as_str();
            if seen.contains(&name) {
                return Err(Error::parse("duplicate parameter name", self.offset()));
            }
            seen.push(name);
        }
        Ok(())
    }

    fn reject_restricted_strict_name(&self, name: &str) -> Result<()> {
        if Self::is_restricted_strict_name(name) {
            return Err(Error::parse(
                "eval and arguments are not valid strict binding names",
                self.offset(),
            ));
        }
        Ok(())
    }

    fn string_directive_value(statement: &Stmt) -> Option<&str> {
        let Stmt::Expr(Expr::StringLiteral(value)) = statement else {
            return None;
        };
        Some(value.as_str())
    }

    fn is_string_directive(statement: &Stmt) -> bool {
        Self::string_directive_value(statement).is_some()
    }

    pub(super) fn is_use_strict_directive(statement: &Stmt) -> bool {
        Self::string_directive_value(statement).is_some_and(|value| value == USE_STRICT_DIRECTIVE)
    }

    fn is_restricted_strict_name(name: &str) -> bool {
        matches!(name, EVAL_IDENTIFIER_NAME | ARGUMENTS_IDENTIFIER_NAME)
    }

    fn parameter_list_is_non_simple(params: &[FunctionParam]) -> bool {
        params.iter().any(|param| param.default.is_some())
    }
}
