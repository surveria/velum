use crate::{
    ast::{Expr, Expression, TemplateElement},
    error::{Error, Result},
    lexer::{TemplatePart, TokenKind},
};

use super::Parser;

impl Parser {
    pub(super) fn tagged_template_suffix(&mut self, callee: Expression) -> Result<Expression> {
        let token = self.advance_token("expected tagged template")?;
        let template_span = token.span;
        let (quasis, expressions) = match token.kind {
            TokenKind::NoSubstitutionTemplate(part) => {
                (vec![self.template_element(part)?], Vec::new())
            }
            TokenKind::TemplateHead(part) => self.template_parts(part)?,
            _ => return Err(Error::parse_at("expected tagged template", template_span)),
        };
        let site = self.static_call_site()?;
        let mut args = Vec::with_capacity(expressions.len().saturating_add(1));
        args.push(Expression::new(
            Expr::TemplateObject { site, quasis },
            template_span,
        ));
        args.extend(expressions);
        let start = callee.span();
        Ok(self.expression_node(
            start,
            Expr::Call {
                callee: Box::new(callee),
                site,
                strict: self.is_strict_mode(),
                args,
            },
        ))
    }

    pub(super) fn template_literal(
        &mut self,
        head: TemplatePart,
        start: crate::SourceSpan,
    ) -> Result<Expression> {
        let (quasis, expressions) = self.template_parts(head)?;
        Self::reject_invalid_untagged_template(&quasis, start)?;
        Ok(self.expression_node(
            start,
            Expr::TemplateLiteral {
                quasis,
                expressions,
            },
        ))
    }

    fn template_parts(
        &mut self,
        head: TemplatePart,
    ) -> Result<(Vec<TemplateElement>, Vec<Expression>)> {
        let mut quasis = vec![self.template_element(head)?];
        let mut expressions = Vec::new();
        loop {
            expressions.push(self.with_in_operator_allowed(true, Self::expression)?);
            let token = self.advance_token("expected template literal continuation")?;
            let token_span = token.span;
            match token.kind {
                TokenKind::TemplateMiddle(part) => quasis.push(self.template_element(part)?),
                TokenKind::TemplateTail(part) => {
                    quasis.push(self.template_element(part)?);
                    break;
                }
                _ => {
                    return Err(Error::parse_at(
                        "expected '}' to continue template literal",
                        token_span,
                    ));
                }
            }
        }
        Ok((quasis, expressions))
    }

    fn template_element(&mut self, part: TemplatePart) -> Result<TemplateElement> {
        Ok(TemplateElement {
            cooked: part
                .cooked
                .map(|value| self.static_string_shared(value))
                .transpose()?,
            raw: self.static_string_shared(part.raw)?,
        })
    }

    pub(super) fn no_substitution_template(
        &mut self,
        part: TemplatePart,
        span: crate::SourceSpan,
    ) -> Result<Expression> {
        let quasi = self.template_element(part)?;
        Self::reject_invalid_untagged_template(std::slice::from_ref(&quasi), span)?;
        Ok(Expression::new(
            Expr::TemplateLiteral {
                quasis: vec![quasi],
                expressions: Vec::new(),
            },
            span,
        ))
    }

    fn reject_invalid_untagged_template(
        quasis: &[TemplateElement],
        span: crate::SourceSpan,
    ) -> Result<()> {
        if quasis.iter().any(|quasi| quasi.cooked.is_none()) {
            return Err(Error::parse_at(
                "invalid escape sequence in untagged template literal",
                span,
            ));
        }
        Ok(())
    }
}
