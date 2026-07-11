use crate::{
    ast::{BinaryOp, Expr, Expression},
    error::{Error, Result},
    lexer::TokenKind,
};

use super::Parser;

impl Parser {
    pub(super) fn assignment(&mut self) -> Result<Expression> {
        if let Some(function) = self.arrow_function()? {
            return Ok(function);
        }
        let target = self.conditional()?;
        let Some((operator, offset)) = self.assignment_operator() else {
            return Ok(target);
        };
        let value = self.assignment()?;
        self.assignment_expr(target, operator, value, offset)
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
        let Some(target) = Self::assignment_target(target) else {
            return Err(Error::parse_at("invalid assignment target", operator_span));
        };
        let kind = match target.into_kind() {
            Expr::Identifier(name) => Expr::Assignment {
                name,
                strict: self.is_strict_mode(),
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
            | Expr::Update { .. }
            | Expr::Binary { .. }
            | Expr::Conditional { .. }
            | Expr::Assignment { .. }
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
