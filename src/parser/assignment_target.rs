use crate::{
    ast::{Expr, Expression},
    lexer::TokenKind,
};

use super::Parser;

impl Parser {
    pub(super) fn simple_assignment_target(expr: Expression) -> Option<Expression> {
        let span = expr.span();
        match expr.into_kind() {
            kind @ (Expr::Identifier(_)
            | Expr::Member { .. }
            | Expr::ComputedMember { .. }
            | Expr::SuperMember { .. }
            | Expr::SuperComputedMember { .. }
            | Expr::PrivateMember { .. }) => Some(Expression::new(kind, span)),
            Expr::Parenthesized(inner) => Self::simple_assignment_target(*inner),
            _ => None,
        }
    }

    pub(super) fn assignment_target(&self, expr: Expression) -> Option<Expression> {
        let span = expr.span();
        match expr.into_kind() {
            Expr::Call {
                callee,
                site,
                strict,
                args,
            } if !self.is_strict_mode() => Some(Expression::new(
                Expr::Call {
                    callee,
                    site,
                    strict,
                    args,
                },
                span,
            )),
            Expr::Parenthesized(inner) => self.assignment_target(*inner),
            kind => Self::simple_assignment_target(Expression::new(kind, span)),
        }
    }

    pub(super) fn await_starts_identifier_assignment(&mut self) -> bool {
        !self.await_identifier_is_reserved()
            && self.peek_kind(1).is_some_and(await_identifier_continuation)
    }
}

const fn await_identifier_continuation(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Equal
            | TokenKind::PlusEqual
            | TokenKind::MinusEqual
            | TokenKind::StarEqual
            | TokenKind::StarStarEqual
            | TokenKind::SlashEqual
            | TokenKind::PercentEqual
            | TokenKind::AmpersandEqual
            | TokenKind::PipeEqual
            | TokenKind::CaretEqual
            | TokenKind::LessLessEqual
            | TokenKind::GreaterGreaterEqual
            | TokenKind::GreaterGreaterGreaterEqual
            | TokenKind::AndAndEqual
            | TokenKind::OrOrEqual
            | TokenKind::QuestionQuestionEqual
            | TokenKind::PlusPlus
            | TokenKind::MinusMinus
            | TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::StarStar
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::EqualEqual
            | TokenKind::BangEqual
            | TokenKind::StrictEqual
            | TokenKind::StrictNotEqual
            | TokenKind::Less
            | TokenKind::LessEqual
            | TokenKind::LessLess
            | TokenKind::Greater
            | TokenKind::GreaterEqual
            | TokenKind::GreaterGreater
            | TokenKind::GreaterGreaterGreater
            | TokenKind::Ampersand
            | TokenKind::Pipe
            | TokenKind::Caret
            | TokenKind::AndAnd
            | TokenKind::OrOr
            | TokenKind::QuestionQuestion
            | TokenKind::Question
            | TokenKind::Dot
            | TokenKind::In
            | TokenKind::InstanceOf
            | TokenKind::Semicolon
            | TokenKind::Comma
            | TokenKind::Colon
            | TokenKind::RParen
            | TokenKind::RBracket
            | TokenKind::RBrace
            | TokenKind::Eof
    )
}
