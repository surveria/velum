use crate::ast::{Expr, Statement, StaticBinding, StaticName, Stmt};
use crate::error::Result;

use super::{
    ARGUMENTS_IDENTIFIER_NAME, EVAL_IDENTIFIER_NAME, Parser, USE_STRICT_DIRECTIVE,
    YIELD_IDENTIFIER_NAME,
};

impl Parser {
    pub(super) fn update_directive_prologue(
        &mut self,
        directive_prologue: &mut bool,
        legacy_escape_seen: &mut bool,
        statement: &Statement,
    ) -> Result<()> {
        if !*directive_prologue {
            return Ok(());
        }
        if Self::is_use_strict_directive(statement) {
            if *legacy_escape_seen {
                return Err(crate::Error::parse_at(
                    "legacy escape sequence is not allowed in a strict directive prologue",
                    statement.span(),
                ));
            }
            self.set_strict_mode(true);
        }
        if Self::string_directive(statement).is_some_and(|(_, _, legacy_escape)| legacy_escape) {
            *legacy_escape_seen = true;
        }
        if !Self::is_string_directive(statement) {
            *directive_prologue = false;
        }
        Ok(())
    }

    pub(super) fn validate_function_name_in_strict_code(&self, name: &StaticName) -> Result<()> {
        self.reject_strict_binding_name(name.as_str())
    }

    pub(super) fn validate_function_binding_in_strict_code(
        &self,
        name: &StaticBinding,
    ) -> Result<()> {
        self.reject_strict_binding_name(name.as_str())
    }

    pub(super) fn validate_function_parameters(
        &self,
        bound_names: &[StaticBinding],
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
            self.reject_duplicate_parameters(bound_names)?;
            for name in bound_names {
                self.reject_strict_binding_name(name.as_str())?;
            }
        }

        Ok(())
    }

    pub(super) fn reject_duplicate_non_simple_parameters(
        &self,
        bound_names: &[StaticBinding],
        parameters_are_simple: bool,
    ) -> Result<()> {
        if !parameters_are_simple {
            self.reject_duplicate_parameters(bound_names)?;
        }
        Ok(())
    }

    pub(super) fn reject_duplicate_parameters(&self, bound_names: &[StaticBinding]) -> Result<()> {
        let mut seen = Vec::new();
        for binding in bound_names {
            let name = binding.as_str();
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

    fn reject_strict_binding_name(&self, name: &str) -> Result<()> {
        self.reject_restricted_strict_name(name)?;
        if is_strict_future_reserved_word(name) {
            return Err(self.parse_error("future reserved word is not a valid strict binding name"));
        }
        Ok(())
    }

    pub(super) fn validate_strict_identifier_reference(&self, name: &str) -> Result<()> {
        if self.is_strict_mode() && name == YIELD_IDENTIFIER_NAME {
            return Err(self.parse_error("yield is not a valid strict identifier reference"));
        }
        if self.is_strict_mode() && is_strict_future_reserved_word(name) {
            return Err(
                self.parse_error("reserved word is not a valid strict identifier reference")
            );
        }
        Ok(())
    }

    pub(super) fn validate_assignment_identifier(&self, name: &str) -> Result<()> {
        if (self.yield_identifier_is_reserved() && name == YIELD_IDENTIFIER_NAME)
            || (self.await_identifier_is_reserved() && name == "await")
        {
            return Err(self.parse_error("invalid contextual assignment target"));
        }
        if !self.is_strict_mode() {
            return Ok(());
        }
        if Self::is_restricted_strict_name(name) || is_strict_future_reserved_word(name) {
            return Err(self.parse_error("invalid strict assignment target"));
        }
        Ok(())
    }

    pub(super) fn validate_assignment_target(&self, target: &crate::ast::Expression) -> Result<()> {
        if let Expr::Identifier(name) = target.kind() {
            self.validate_assignment_identifier(name.as_str())?;
        }
        Ok(())
    }

    fn string_directive(statement: &Statement) -> Option<(&str, bool, bool)> {
        let Stmt::Expr(expression) = statement.kind() else {
            return None;
        };
        let Expr::StringLiteral {
            value,
            escape_free,
            legacy_escape,
        } = expression.kind()
        else {
            return None;
        };
        Some((value.as_str(), *escape_free, *legacy_escape))
    }

    fn is_string_directive(statement: &Statement) -> bool {
        Self::string_directive(statement).is_some()
    }

    pub(super) fn is_use_strict_directive(statement: &Statement) -> bool {
        Self::string_directive(statement)
            .is_some_and(|(value, escape_free, _)| escape_free && value == USE_STRICT_DIRECTIVE)
    }

    fn is_restricted_strict_name(name: &str) -> bool {
        matches!(
            name,
            EVAL_IDENTIFIER_NAME | ARGUMENTS_IDENTIFIER_NAME | YIELD_IDENTIFIER_NAME
        )
    }
}

fn is_strict_future_reserved_word(name: &str) -> bool {
    matches!(
        name,
        "implements" | "interface" | "package" | "private" | "protected" | "public" | "static"
    )
}
