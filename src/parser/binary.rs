use crate::{
    ast::{BinaryOp, Expr},
    error::{Error, Result},
    lexer::TokenKind,
};

use super::Parser;

impl Parser {
    pub(super) fn conditional(&mut self) -> Result<Expr> {
        let condition = self.coalesce()?;
        if !self.match_kind(&TokenKind::Question) {
            return Ok(condition);
        }

        let consequent = self.assignment()?;
        self.consume(&TokenKind::Colon, "expected ':' in conditional expression")?;
        let alternate = self.assignment()?;
        Ok(Expr::Conditional {
            condition: Box::new(condition),
            consequent: Box::new(consequent),
            alternate: Box::new(alternate),
        })
    }

    fn coalesce(&mut self) -> Result<Expr> {
        let mut expr = self.logical_or()?;
        while self.match_kind(&TokenKind::QuestionQuestion) {
            if Self::contains_unparenthesized_logical(&expr) {
                return Err(Error::parse(
                    "'??' cannot be mixed with '&&' or '||' without parentheses",
                    self.previous_offset(),
                ));
            }
            let right = self.bitwise_or()?;
            expr = Expr::Binary {
                op: BinaryOp::NullishCoalescing,
                left: Box::new(expr),
                right: Box::new(right),
                property_access: None,
            };
        }
        Ok(expr)
    }

    fn logical_or(&mut self) -> Result<Expr> {
        self.left_assoc(
            Self::logical_and,
            &[(&TokenKind::OrOr, BinaryOp::LogicalOr)],
        )
    }

    fn logical_and(&mut self) -> Result<Expr> {
        self.left_assoc(
            Self::bitwise_or,
            &[(&TokenKind::AndAnd, BinaryOp::LogicalAnd)],
        )
    }

    fn bitwise_or(&mut self) -> Result<Expr> {
        self.left_assoc(Self::bitwise_xor, &[(&TokenKind::Pipe, BinaryOp::BitOr)])
    }

    fn bitwise_xor(&mut self) -> Result<Expr> {
        self.left_assoc(Self::bitwise_and, &[(&TokenKind::Caret, BinaryOp::BitXor)])
    }

    fn bitwise_and(&mut self) -> Result<Expr> {
        self.left_assoc(Self::equality, &[(&TokenKind::Ampersand, BinaryOp::BitAnd)])
    }

    fn equality(&mut self) -> Result<Expr> {
        self.left_assoc(
            Self::comparison,
            &[
                (&TokenKind::EqualEqual, BinaryOp::Equal),
                (&TokenKind::BangEqual, BinaryOp::NotEqual),
                (&TokenKind::StrictEqual, BinaryOp::StrictEqual),
                (&TokenKind::StrictNotEqual, BinaryOp::StrictNotEqual),
            ],
        )
    }

    fn comparison(&mut self) -> Result<Expr> {
        self.left_assoc(
            Self::shift,
            &[
                (&TokenKind::Less, BinaryOp::Less),
                (&TokenKind::LessEqual, BinaryOp::LessEqual),
                (&TokenKind::Greater, BinaryOp::Greater),
                (&TokenKind::GreaterEqual, BinaryOp::GreaterEqual),
                (&TokenKind::In, BinaryOp::In),
                (&TokenKind::InstanceOf, BinaryOp::InstanceOf),
            ],
        )
    }

    fn shift(&mut self) -> Result<Expr> {
        self.left_assoc(
            Self::term,
            &[
                (&TokenKind::LessLess, BinaryOp::ShiftLeft),
                (&TokenKind::GreaterGreater, BinaryOp::ShiftRight),
                (
                    &TokenKind::GreaterGreaterGreater,
                    BinaryOp::ShiftRightUnsigned,
                ),
            ],
        )
    }

    fn term(&mut self) -> Result<Expr> {
        self.left_assoc(
            Self::factor,
            &[
                (&TokenKind::Plus, BinaryOp::Add),
                (&TokenKind::Minus, BinaryOp::Sub),
            ],
        )
    }

    fn factor(&mut self) -> Result<Expr> {
        self.left_assoc(
            Self::power,
            &[
                (&TokenKind::Star, BinaryOp::Mul),
                (&TokenKind::Slash, BinaryOp::Div),
                (&TokenKind::Percent, BinaryOp::Rem),
            ],
        )
    }

    fn power(&mut self) -> Result<Expr> {
        let left = self.unary()?;
        if !self.match_kind(&TokenKind::StarStar) {
            return Ok(left);
        }
        if matches!(left, Expr::Unary { .. }) {
            return Err(Error::parse(
                "unary expression cannot be the left operand of '**'",
                self.previous_offset(),
            ));
        }
        let right = self.power()?;
        Ok(Expr::Binary {
            op: BinaryOp::Pow,
            left: Box::new(left),
            right: Box::new(right),
            property_access: None,
        })
    }

    fn left_assoc(
        &mut self,
        next: fn(&mut Self) -> Result<Expr>,
        ops: &[(&TokenKind, BinaryOp)],
    ) -> Result<Expr> {
        let mut expr = next(self)?;
        while let Some((_, op)) = ops.iter().find(|(kind, _)| self.check(kind)) {
            let op = *op;
            if self.advance().is_none() {
                return Err(self.parse_error("expected operator"));
            }
            let right = next(self)?;
            let property_access = if op == BinaryOp::In {
                Some(self.static_property_access()?)
            } else {
                None
            };
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
                property_access,
            };
        }
        Ok(expr)
    }

    fn contains_unparenthesized_logical(expr: &Expr) -> bool {
        match expr {
            Expr::Binary {
                op: BinaryOp::LogicalAnd | BinaryOp::LogicalOr,
                ..
            } => true,
            Expr::Binary { left, right, .. } => {
                Self::contains_unparenthesized_logical(left)
                    || Self::contains_unparenthesized_logical(right)
            }
            Expr::Conditional {
                condition,
                consequent,
                alternate,
            } => {
                Self::contains_unparenthesized_logical(condition)
                    || Self::contains_unparenthesized_logical(consequent)
                    || Self::contains_unparenthesized_logical(alternate)
            }
            _ => false,
        }
    }
}
