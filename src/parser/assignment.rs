use crate::{
    ast::{BinaryOp, Expr, Expression},
    error::{Error, Result},
    lexer::TokenKind,
};

use super::Parser;

impl Parser {
    pub(super) fn assignment(&mut self) -> Result<Expression> {
        if self.yield_expression_is_allowed() && self.next_is_yield_identifier() {
            return self.yield_expression();
        }
        if let Some(function) = self.arrow_function()? {
            return Ok(function);
        }
        if self.assignment_pattern_followed_by_equal() {
            let start = self.current_span();
            let pattern = self.assignment_pattern()?;
            self.consume(
                &TokenKind::Equal,
                "expected '=' after destructuring assignment pattern",
            )?;
            let expr = self.assignment()?;
            return Ok(self.expression_node(
                start,
                Expr::DestructuringAssignment {
                    pattern: Box::new(pattern),
                    strict: self.is_strict_mode(),
                    expr: Box::new(expr),
                },
            ));
        }
        let target = self.conditional()?;
        let Some((operator, offset)) = self.assignment_operator() else {
            return Ok(target);
        };
        let value = self.assignment()?;
        self.assignment_expr(target, operator, value, offset)
    }

    fn next_is_yield_identifier(&mut self) -> bool {
        self.peek()
            .is_some_and(|token| token.is_unescaped_identifier_named(super::YIELD_IDENTIFIER_NAME))
    }

    fn yield_expression(&mut self) -> Result<Expression> {
        let start = self.current_span();
        if self.advance().is_none() {
            return Err(self.parse_error("expected yield expression"));
        }
        let has_line_terminator = self.peek_has_line_terminator_before(0);
        if has_line_terminator && self.check(&TokenKind::Star) {
            return Err(self.parse_error("line terminator is not allowed before 'yield*'"));
        }
        let delegate = !has_line_terminator && self.match_kind(&TokenKind::Star);
        let expr = if has_line_terminator
            || matches!(
                self.peek_kind(0),
                Some(
                    TokenKind::Semicolon
                        | TokenKind::Comma
                        | TokenKind::Colon
                        | TokenKind::RParen
                        | TokenKind::RBracket
                        | TokenKind::RBrace
                        | TokenKind::Eof
                )
            ) {
            None
        } else {
            Some(Box::new(self.assignment()?))
        };
        if delegate && expr.is_none() {
            return Err(self.parse_error("expected expression after 'yield*'"));
        }
        Ok(self.expression_node(start, Expr::Yield { expr, delegate }))
    }

    /// Array and object literals become assignment patterns only when the
    /// matching outer delimiter is immediately followed by `=`. This keeps
    /// ordinary literal parsing on the existing expression path without
    /// speculative parser-table mutations.
    fn assignment_pattern_followed_by_equal(&mut self) -> bool {
        let Some(offset) = self.outer_literal_closing_offset() else {
            return false;
        };
        self.peek_kind(offset.saturating_add(1))
            .is_some_and(|next| next == &TokenKind::Equal)
    }

    pub(super) fn literal_starts_assignment_target(&mut self) -> bool {
        let Some(offset) = self.outer_literal_closing_offset() else {
            return false;
        };
        matches!(
            self.peek_kind(offset.saturating_add(1)),
            Some(TokenKind::Dot | TokenKind::LBracket)
        )
    }

    pub(super) fn outer_literal_closing_offset(&mut self) -> Option<usize> {
        match self.peek_kind(0)? {
            TokenKind::LBrace | TokenKind::LBracket => self.balanced_closing_offset(0),
            _ => None,
        }
    }

    fn assignment_operator(&mut self) -> Option<(Option<BinaryOp>, crate::SourceSpan)> {
        let operator = if self.match_kind(&TokenKind::Equal) {
            None
        } else if self.match_kind(&TokenKind::PlusEqual) {
            Some(BinaryOp::Add)
        } else if self.match_kind(&TokenKind::MinusEqual) {
            Some(BinaryOp::Sub)
        } else if self.match_kind(&TokenKind::StarEqual) {
            Some(BinaryOp::Mul)
        } else if self.match_kind(&TokenKind::StarStarEqual) {
            Some(BinaryOp::Pow)
        } else if self.match_kind(&TokenKind::SlashEqual) {
            Some(BinaryOp::Div)
        } else if self.match_kind(&TokenKind::PercentEqual) {
            Some(BinaryOp::Rem)
        } else if self.match_kind(&TokenKind::AmpersandEqual) {
            Some(BinaryOp::BitAnd)
        } else if self.match_kind(&TokenKind::PipeEqual) {
            Some(BinaryOp::BitOr)
        } else if self.match_kind(&TokenKind::CaretEqual) {
            Some(BinaryOp::BitXor)
        } else if self.match_kind(&TokenKind::LessLessEqual) {
            Some(BinaryOp::ShiftLeft)
        } else if self.match_kind(&TokenKind::GreaterGreaterEqual) {
            Some(BinaryOp::ShiftRight)
        } else if self.match_kind(&TokenKind::GreaterGreaterGreaterEqual) {
            Some(BinaryOp::ShiftRightUnsigned)
        } else if self.match_kind(&TokenKind::AndAndEqual) {
            Some(BinaryOp::LogicalAnd)
        } else if self.match_kind(&TokenKind::OrOrEqual) {
            Some(BinaryOp::LogicalOr)
        } else if self.match_kind(&TokenKind::QuestionQuestionEqual) {
            Some(BinaryOp::NullishCoalescing)
        } else {
            return None;
        };
        Some((operator, self.previous_span()))
    }

    fn assignment_expr(
        &self,
        target: Expression,
        operator: Option<BinaryOp>,
        value: Expression,
        operator_span: crate::SourceSpan,
    ) -> Result<Expression> {
        if let Some(op) = operator {
            return self.compound_assignment_expr(target, op, value, operator_span);
        }
        let start = target.span();
        let infer_name = matches!(target.kind(), Expr::Identifier(_));
        let Some(target) = self.assignment_target(target) else {
            return Err(Error::parse_at("invalid assignment target", operator_span));
        };
        self.validate_assignment_target(&target)?;
        if matches!(target.kind(), Expr::Call { .. }) {
            return Ok(self.web_compat_call_assignment(start, target, value));
        }
        let kind = self.assignment_kind(target.into_kind(), value, infer_name, operator_span)?;
        Ok(self.expression_node(start, kind))
    }

    fn assignment_kind(
        &self,
        target: Expr,
        value: Expression,
        infer_name: bool,
        operator_span: crate::SourceSpan,
    ) -> Result<Expr> {
        Ok(match target {
            Expr::Identifier(name) => Expr::Assignment {
                name,
                strict: self.is_strict_mode(),
                infer_name,
                expr: Box::new(value),
            },
            Expr::Member {
                object,
                property,
                access,
            } => Expr::PropertyAssignment {
                object,
                property,
                access,
                strict: self.is_strict_mode(),
                expr: Box::new(value),
            },
            Expr::ComputedMember {
                object,
                property,
                access,
            } => Expr::ComputedPropertyAssignment {
                object,
                property,
                access,
                strict: self.is_strict_mode(),
                expr: Box::new(value),
            },
            Expr::PrivateMember { object, name } => Expr::PrivateAssignment {
                object,
                name,
                expr: Box::new(value),
            },
            Expr::SuperMember { property, access } => Expr::SuperPropertyAssignment {
                property,
                access,
                strict: self.is_strict_mode(),
                expr: Box::new(value),
            },
            Expr::SuperComputedMember { property, access } => {
                Expr::SuperComputedPropertyAssignment {
                    property,
                    access,
                    strict: self.is_strict_mode(),
                    expr: Box::new(value),
                }
            }
            Expr::Literal(_)
            | Expr::StringLiteral { .. }
            | Expr::Spread(_)
            | Expr::Class(_)
            | Expr::SuperCall { .. }
            | Expr::TemplateLiteral { .. }
            | Expr::TemplateObject { .. }
            | Expr::RegExpLiteral { .. }
            | Expr::This
            | Expr::ImportMeta
            | Expr::NewTarget
            | Expr::Parenthesized(_)
            | Expr::OptionalChain(_)
            | Expr::Sequence(_)
            | Expr::Unary { .. }
            | Expr::Await(_)
            | Expr::Yield { .. }
            | Expr::Update { .. }
            | Expr::Binary { .. }
            | Expr::Conditional { .. }
            | Expr::Assignment { .. }
            | Expr::DestructuringAssignment { .. }
            | Expr::CompoundAssignment { .. }
            | Expr::WebCompatCallAssignment { .. }
            | Expr::PropertyAssignment { .. }
            | Expr::ComputedPropertyAssignment { .. }
            | Expr::SuperPropertyAssignment { .. }
            | Expr::SuperComputedPropertyAssignment { .. }
            | Expr::PrivateAssignment { .. }
            | Expr::PrivateIn { .. }
            | Expr::OptionalMember { .. }
            | Expr::OptionalComputedMember { .. }
            | Expr::OptionalPrivateMember { .. }
            | Expr::Call { .. }
            | Expr::OptionalCall { .. }
            | Expr::DynamicImport { .. }
            | Expr::Function { .. }
            | Expr::ArrowFunction { .. }
            | Expr::MethodFunction { .. }
            | Expr::Object(_)
            | Expr::ArrayHole
            | Expr::Array(_)
            | Expr::New { .. } => {
                return Err(Error::parse_at("invalid assignment target", operator_span));
            }
        })
    }

    fn compound_assignment_expr(
        &self,
        target: Expression,
        op: BinaryOp,
        value: Expression,
        operator_span: crate::SourceSpan,
    ) -> Result<Expression> {
        let start = target.span();
        let target_was_parenthesized = matches!(target.kind(), Expr::Parenthesized(_));
        let Some(target) = self.assignment_target(target) else {
            return Err(Error::parse_at("invalid assignment target", operator_span));
        };
        self.validate_assignment_target(&target)?;
        if matches!(target.kind(), Expr::Call { .. })
            && matches!(
                op,
                BinaryOp::LogicalAnd | BinaryOp::LogicalOr | BinaryOp::NullishCoalescing
            )
        {
            return Err(Error::parse_at(
                "invalid logical assignment target",
                operator_span,
            ));
        }
        let target = if target_was_parenthesized {
            self.expression_node(start, Expr::Parenthesized(Box::new(target)))
        } else {
            target
        };
        if matches!(target.kind(), Expr::Call { .. }) {
            return Ok(self.web_compat_call_assignment(start, target, value));
        }
        Ok(self.expression_node(
            start,
            Expr::CompoundAssignment {
                op,
                strict: self.is_strict_mode(),
                target: Box::new(target),
                expr: Box::new(value),
            },
        ))
    }

    fn web_compat_call_assignment(
        &self,
        start: crate::SourceSpan,
        target: Expression,
        discarded: Expression,
    ) -> Expression {
        self.expression_node(
            start,
            Expr::WebCompatCallAssignment {
                target: Box::new(target),
                discarded: Some(Box::new(discarded)),
            },
        )
    }
}
