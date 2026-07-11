use crate::{
    ast::{BinaryOp, Expr, Expression},
    error::{Error, Result},
    lexer::TokenKind,
};

use super::Parser;

impl Parser {
    pub(super) fn conditional(&mut self) -> Result<Expression> {
        let condition = self.coalesce()?;
        if !self.match_kind(&TokenKind::Question) {
            return Ok(condition);
        }

        let consequent = self.assignment()?;
        self.consume(&TokenKind::Colon, "expected ':' in conditional expression")?;
        let alternate = self.assignment()?;
        let start = condition.span();
        Ok(self.expression_node(
            start,
            Expr::Conditional {
                condition: Box::new(condition),
                consequent: Box::new(consequent),
                alternate: Box::new(alternate),
            },
        ))
    }

    fn coalesce(&mut self) -> Result<Expression> {
        let mut expr = self.logical_or()?;
        while self.match_kind(&TokenKind::QuestionQuestion) {
            if Self::contains_unparenthesized_logical(&expr) {
                return Err(Error::parse_at(
                    "'??' cannot be mixed with '&&' or '||' without parentheses",
                    self.previous_span(),
                ));
            }
            let right = self.bitwise_or()?;
            let start = expr.span();
            expr = self.expression_node(
                start,
                Expr::Binary {
                    op: BinaryOp::NullishCoalescing,
                    left: Box::new(expr),
                    right: Box::new(right),
                    property_access: None,
                },
            );
        }
        Ok(expr)
    }

    fn logical_or(&mut self) -> Result<Expression> {
        self.left_assoc(
            Self::logical_and,
            &[(&TokenKind::OrOr, BinaryOp::LogicalOr)],
        )
    }

    fn logical_and(&mut self) -> Result<Expression> {
        self.left_assoc(
            Self::bitwise_or,
            &[(&TokenKind::AndAnd, BinaryOp::LogicalAnd)],
        )
    }

    fn bitwise_or(&mut self) -> Result<Expression> {
        self.left_assoc(Self::bitwise_xor, &[(&TokenKind::Pipe, BinaryOp::BitOr)])
    }

    fn bitwise_xor(&mut self) -> Result<Expression> {
        self.left_assoc(Self::bitwise_and, &[(&TokenKind::Caret, BinaryOp::BitXor)])
    }

    fn bitwise_and(&mut self) -> Result<Expression> {
        self.left_assoc(Self::equality, &[(&TokenKind::Ampersand, BinaryOp::BitAnd)])
    }

    fn equality(&mut self) -> Result<Expression> {
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

    fn comparison(&mut self) -> Result<Expression> {
        const COMPARISON_OPS: &[(&TokenKind, BinaryOp)] = &[
            (&TokenKind::Less, BinaryOp::Less),
            (&TokenKind::LessEqual, BinaryOp::LessEqual),
            (&TokenKind::Greater, BinaryOp::Greater),
            (&TokenKind::GreaterEqual, BinaryOp::GreaterEqual),
            (&TokenKind::In, BinaryOp::In),
            (&TokenKind::InstanceOf, BinaryOp::InstanceOf),
        ];
        let seed = if let Some(private_in) = self.private_in_seed()? {
            private_in
        } else {
            self.shift()?
        };
        self.left_assoc_from(seed, Self::shift, COMPARISON_OPS)
    }

    /// Parses the `#name in object` ergonomic brand check that may seed a
    /// relational chain when a private name directly precedes `in`.
    fn private_in_seed(&mut self) -> Result<Option<Expression>> {
        let is_private_in = matches!(self.peek_kind(0), Some(TokenKind::PrivateName(_)))
            && self.peek_kind_is(1, &TokenKind::In);
        if !is_private_in {
            return Ok(None);
        }
        let start = self.current_span();
        let Some(name) = self.match_private_name()? else {
            return Err(self.parse_error("expected private name before 'in'"));
        };
        self.consume(&TokenKind::In, "expected 'in' after private name")?;
        let object = self.shift()?;
        Ok(Some(self.expression_node(
            start,
            Expr::PrivateIn {
                name,
                object: Box::new(object),
            },
        )))
    }

    pub(super) fn starts_private_in_expression(&self) -> bool {
        matches!(self.peek_kind(0), Some(TokenKind::PrivateName(_)))
            && self.peek_kind_is(1, &TokenKind::In)
    }

    fn shift(&mut self) -> Result<Expression> {
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

    fn term(&mut self) -> Result<Expression> {
        self.left_assoc(
            Self::factor,
            &[
                (&TokenKind::Plus, BinaryOp::Add),
                (&TokenKind::Minus, BinaryOp::Sub),
            ],
        )
    }

    fn factor(&mut self) -> Result<Expression> {
        self.left_assoc(
            Self::power,
            &[
                (&TokenKind::Star, BinaryOp::Mul),
                (&TokenKind::Slash, BinaryOp::Div),
                (&TokenKind::Percent, BinaryOp::Rem),
            ],
        )
    }

    fn power(&mut self) -> Result<Expression> {
        let left = self.unary()?;
        if !self.match_kind(&TokenKind::StarStar) {
            return Ok(left);
        }
        if matches!(left.kind(), Expr::Unary { .. }) {
            return Err(Error::parse_at(
                "unary expression cannot be the left operand of '**'",
                self.previous_span(),
            ));
        }
        let right = self.power()?;
        let start = left.span();
        Ok(self.expression_node(
            start,
            Expr::Binary {
                op: BinaryOp::Pow,
                left: Box::new(left),
                right: Box::new(right),
                property_access: None,
            },
        ))
    }

    fn left_assoc(
        &mut self,
        next: fn(&mut Self) -> Result<Expression>,
        ops: &[(&TokenKind, BinaryOp)],
    ) -> Result<Expression> {
        let expr = next(self)?;
        self.left_assoc_from(expr, next, ops)
    }

    fn left_assoc_from(
        &mut self,
        seed: Expression,
        next: fn(&mut Self) -> Result<Expression>,
        ops: &[(&TokenKind, BinaryOp)],
    ) -> Result<Expression> {
        let mut expr = seed;
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
            let start = expr.span();
            expr = self.expression_node(
                start,
                Expr::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(right),
                    property_access,
                },
            );
        }
        Ok(expr)
    }

    fn contains_unparenthesized_logical(expr: &Expression) -> bool {
        match expr.kind() {
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
