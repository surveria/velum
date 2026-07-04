use crate::ast::{BinaryOp, Expr, Program, Stmt, UnaryOp};
use crate::error::{Error, Result};
use crate::lexer::{Token, TokenKind};
use crate::runtime::RuntimeLimits;
use crate::value::Value;

pub(crate) fn parse(tokens: Vec<Token>, limits: RuntimeLimits) -> Result<Program> {
    Parser::new(tokens, limits).parse()
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    limits: RuntimeLimits,
    expression_depth: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>, limits: RuntimeLimits) -> Self {
        Self {
            tokens,
            cursor: 0,
            limits,
            expression_depth: 0,
        }
    }

    fn parse(mut self) -> Result<Program> {
        let mut statements = Vec::new();
        while !self.at_end() {
            if self.match_kind(&TokenKind::Semicolon) {
                continue;
            }
            if statements.len() >= self.limits.max_statements {
                return Err(Error::limit(format!(
                    "statement count exceeded {}",
                    self.limits.max_statements
                )));
            }
            statements.push(self.statement()?);
        }
        Ok(Program { statements })
    }

    fn statement(&mut self) -> Result<Stmt> {
        if self.match_kind(&TokenKind::Let) {
            return self.var_decl(true, false);
        }
        if self.match_kind(&TokenKind::Const) {
            return self.var_decl(false, true);
        }
        if self.match_kind(&TokenKind::Var) {
            return self.var_decl(true, false);
        }

        let expr = self.expression()?;
        self.consume_optional_semicolon();
        Ok(Stmt::Expr(expr))
    }

    fn var_decl(&mut self, mutable: bool, require_init: bool) -> Result<Stmt> {
        let name = self.consume_identifier("expected binding name")?;
        let init = if self.match_kind(&TokenKind::Equal) {
            self.expression()?
        } else if require_init {
            return Err(Error::parse(
                "const declaration requires an initializer",
                self.offset(),
            ));
        } else {
            Expr::Literal(Value::Undefined)
        };

        self.consume_optional_semicolon();
        Ok(Stmt::VarDecl {
            name,
            mutable,
            init,
        })
    }

    fn expression(&mut self) -> Result<Expr> {
        self.with_expression_depth(Self::assignment)
    }

    fn assignment(&mut self) -> Result<Expr> {
        let expr = self.logical_or()?;
        if self.match_kind(&TokenKind::Equal) {
            let offset = self.previous_offset();
            let Expr::Identifier(name) = expr else {
                return Err(Error::parse("invalid assignment target", offset));
            };
            let value = self.assignment()?;
            return Ok(Expr::Assignment {
                name,
                expr: Box::new(value),
            });
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
            Self::equality,
            &[(&TokenKind::AndAnd, BinaryOp::LogicalAnd)],
        )
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
            Self::term,
            &[
                (&TokenKind::Less, BinaryOp::Less),
                (&TokenKind::LessEqual, BinaryOp::LessEqual),
                (&TokenKind::Greater, BinaryOp::Greater),
                (&TokenKind::GreaterEqual, BinaryOp::GreaterEqual),
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
            Self::unary,
            &[
                (&TokenKind::Star, BinaryOp::Mul),
                (&TokenKind::Slash, BinaryOp::Div),
                (&TokenKind::Percent, BinaryOp::Rem),
            ],
        )
    }

    fn unary(&mut self) -> Result<Expr> {
        if self.match_kind(&TokenKind::Bang) {
            let expr = self.unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
            });
        }
        if self.match_kind(&TokenKind::Minus) {
            let expr = self.unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Negate,
                expr: Box::new(expr),
            });
        }
        if self.match_kind(&TokenKind::Plus) {
            let expr = self.unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Plus,
                expr: Box::new(expr),
            });
        }
        self.call()
    }

    fn call(&mut self) -> Result<Expr> {
        let mut expr = self.primary()?;
        loop {
            if !self.match_kind(&TokenKind::LParen) {
                break;
            }

            let mut args = Vec::new();
            if !self.check(&TokenKind::RParen) {
                loop {
                    args.push(self.expression()?);
                    if !self.match_kind(&TokenKind::Comma) {
                        break;
                    }
                }
            }

            self.consume(&TokenKind::RParen, "expected ')' after arguments")?;
            expr = Expr::Call {
                callee: Box::new(expr),
                args,
            };
        }
        Ok(expr)
    }

    fn primary(&mut self) -> Result<Expr> {
        let token = self.advance().clone();
        let expr = match token.kind {
            TokenKind::Number(value) => Expr::Literal(Value::Number(value)),
            TokenKind::String(value) => Expr::Literal(Value::String(value)),
            TokenKind::True => Expr::Literal(Value::Bool(true)),
            TokenKind::False => Expr::Literal(Value::Bool(false)),
            TokenKind::Null => Expr::Literal(Value::Null),
            TokenKind::Undefined => Expr::Literal(Value::Undefined),
            TokenKind::Identifier(name) => Expr::Identifier(name),
            TokenKind::LParen => {
                let expr = self.expression()?;
                self.consume(&TokenKind::RParen, "expected ')' after expression")?;
                expr
            }
            _ => return Err(Error::parse("expected expression", token.offset)),
        };
        Ok(expr)
    }

    fn left_assoc(
        &mut self,
        next: fn(&mut Self) -> Result<Expr>,
        ops: &[(&TokenKind, BinaryOp)],
    ) -> Result<Expr> {
        let mut expr = next(self)?;
        while let Some((_, op)) = ops.iter().find(|(kind, _)| self.check(kind)) {
            let op = *op;
            self.advance();
            let right = next(self)?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn with_expression_depth(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<Expr>,
    ) -> Result<Expr> {
        self.expression_depth += 1;
        if self.expression_depth > self.limits.max_expression_depth {
            self.expression_depth -= 1;
            return Err(Error::limit(format!(
                "expression nesting exceeded {}",
                self.limits.max_expression_depth
            )));
        }
        let result = parse(self);
        self.expression_depth -= 1;
        result
    }

    fn consume_identifier(&mut self, message: &str) -> Result<String> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Identifier(name) => Ok(name),
            _ => Err(Error::parse(message, token.offset)),
        }
    }

    fn consume(&mut self, expected: &TokenKind, message: &str) -> Result<()> {
        if self.check(expected) {
            self.advance();
            Ok(())
        } else {
            Err(Error::parse(message, self.offset()))
        }
    }

    fn consume_optional_semicolon(&mut self) {
        let _ = self.match_kind(&TokenKind::Semicolon);
    }

    fn match_kind(&mut self, expected: &TokenKind) -> bool {
        if self.check(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check(&self, expected: &TokenKind) -> bool {
        token_kind_eq(&self.peek().kind, expected)
    }

    fn advance(&mut self) -> &Token {
        if !self.at_end() {
            self.cursor += 1;
        }
        self.previous()
    }

    fn at_end(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.cursor]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.cursor - 1]
    }

    fn offset(&self) -> usize {
        self.peek().offset
    }

    fn previous_offset(&self) -> usize {
        self.previous().offset
    }
}

fn token_kind_eq(left: &TokenKind, right: &TokenKind) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}
