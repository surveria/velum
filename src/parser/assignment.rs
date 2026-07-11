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

    fn next_is_yield_identifier(&self) -> bool {
        self.peek().is_some_and(|token| {
            matches!(&token.kind, TokenKind::Identifier(name) if name == super::YIELD_IDENTIFIER_NAME)
        })
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
    fn assignment_pattern_followed_by_equal(&self) -> bool {
        let Some(offset) = self.outer_literal_closing_offset() else {
            return false;
        };
        self.peek_kind(offset.saturating_add(1))
            .is_some_and(|next| next == &TokenKind::Equal)
    }

    pub(super) fn literal_starts_assignment_target(&self) -> bool {
        let Some(offset) = self.outer_literal_closing_offset() else {
            return false;
        };
        matches!(
            self.peek_kind(offset.saturating_add(1)),
            Some(TokenKind::Dot | TokenKind::LBracket)
        )
    }

    pub(super) fn outer_literal_closing_offset(&self) -> Option<usize> {
        let first = self.peek_kind(0)?;
        let first = match first {
            TokenKind::LBrace => Delimiter::Brace,
            TokenKind::LBracket => Delimiter::Bracket,
            _ => return None,
        };
        let mut delimiters = vec![first];
        let mut offset = 1usize;
        while let Some(kind) = self.peek_kind(offset) {
            let closing = match kind {
                TokenKind::RParen => Some(Delimiter::Paren),
                TokenKind::RBrace => Some(Delimiter::Brace),
                TokenKind::RBracket => Some(Delimiter::Bracket),
                _ => None,
            };
            if let Some(closing) = closing {
                if delimiters.pop() != Some(closing) {
                    return None;
                }
            } else {
                match kind {
                    TokenKind::LParen => delimiters.push(Delimiter::Paren),
                    TokenKind::LBrace => delimiters.push(Delimiter::Brace),
                    TokenKind::LBracket => delimiters.push(Delimiter::Bracket),
                    _ => {}
                }
            }
            if delimiters.is_empty() {
                return Some(offset);
            }
            offset = offset.saturating_add(1);
        }
        None
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
        let Some(target) = Self::assignment_target(target) else {
            return Err(Error::parse_at("invalid assignment target", operator_span));
        };
        let kind = match target.into_kind() {
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
                expr: Box::new(value),
            },
            Expr::Literal(_)
            | Expr::StringLiteral(_)
            | Expr::Spread(_)
            | Expr::Class(_)
            | Expr::SuperCall { .. }
            | Expr::SuperMember { .. }
            | Expr::TemplateLiteral { .. }
            | Expr::RegExpLiteral { .. }
            | Expr::This
            | Expr::NewTarget
            | Expr::Parenthesized(_)
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
            | Expr::PropertyAssignment { .. }
            | Expr::ComputedPropertyAssignment { .. }
            | Expr::Call { .. }
            | Expr::Function { .. }
            | Expr::ArrowFunction { .. }
            | Expr::MethodFunction { .. }
            | Expr::Object(_)
            | Expr::ArrayHole
            | Expr::Array(_)
            | Expr::New { .. } => {
                return Err(Error::parse_at("invalid assignment target", operator_span));
            }
        };
        Ok(self.expression_node(start, kind))
    }

    fn compound_assignment_expr(
        &self,
        target: Expression,
        op: BinaryOp,
        value: Expression,
        operator_span: crate::SourceSpan,
    ) -> Result<Expression> {
        let start = target.span();
        let Some(target) = Self::assignment_target(target) else {
            return Err(Error::parse_at("invalid assignment target", operator_span));
        };
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
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Delimiter {
    Paren,
    Brace,
    Bracket,
}
