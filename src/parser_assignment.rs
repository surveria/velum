use crate::{
    ast::{BinaryOp, Expr},
    error::{Error, Result},
    lexer::TokenKind,
};

use super::Parser;

impl Parser {
    pub(super) fn assignment(&mut self) -> Result<Expr> {
        let target = self.conditional()?;
        let Some((operator, offset)) = self.assignment_operator() else {
            return Ok(target);
        };
        let value = self.assignment()?;
        Self::assignment_expr(target, operator, value, offset)
    }

    fn assignment_operator(&mut self) -> Option<(Option<BinaryOp>, usize)> {
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
        } else {
            return None;
        };
        Some((operator, self.previous_offset()))
    }

    fn assignment_expr(
        target: Expr,
        operator: Option<BinaryOp>,
        value: Expr,
        offset: usize,
    ) -> Result<Expr> {
        if let Some(op) = operator {
            return Self::compound_assignment_expr(target, op, value, offset);
        }
        let Some(target) = Self::assignment_target(target) else {
            return Err(Error::parse("invalid assignment target", offset));
        };
        match target {
            Expr::Identifier(name) => Ok(Expr::Assignment {
                name,
                expr: Box::new(value),
            }),
            Expr::Member {
                object,
                property,
                access,
            } => Ok(Expr::PropertyAssignment {
                object,
                property,
                access,
                expr: Box::new(value),
            }),
            Expr::ComputedMember { object, property } => Ok(Expr::ComputedPropertyAssignment {
                object,
                property,
                expr: Box::new(value),
            }),
            _ => Err(Error::parse("invalid assignment target", offset)),
        }
    }

    fn compound_assignment_expr(
        target: Expr,
        op: BinaryOp,
        value: Expr,
        offset: usize,
    ) -> Result<Expr> {
        let Some(target) = Self::assignment_target(target) else {
            return Err(Error::parse("invalid assignment target", offset));
        };
        Ok(Expr::CompoundAssignment {
            op,
            target: Box::new(target),
            expr: Box::new(value),
        })
    }
}
