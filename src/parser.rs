use crate::ast::{
    BinaryOp, CatchClause, DeclKind, Expr, ObjectProperty, Program, Stmt, SwitchCase, UnaryOp,
    UpdateOp,
};
use crate::error::{Error, Result};
use crate::lexer::{Token, TokenKind};
use crate::runtime_limits::RuntimeLimits;
use crate::value::Value;

#[path = "parser_assignment.rs"]
mod parser_assignment;

pub fn parse(tokens: Vec<Token>, limits: RuntimeLimits) -> Result<Program> {
    Parser::new(tokens, limits).parse()
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    limits: RuntimeLimits,
    expression_depth: usize,
}

impl Parser {
    const fn new(tokens: Vec<Token>, limits: RuntimeLimits) -> Self {
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
        if self.match_kind(&TokenKind::LBrace) {
            return self.block();
        }
        if self.match_kind(&TokenKind::If) {
            return self.if_statement();
        }
        if self.match_kind(&TokenKind::While) {
            return self.while_statement();
        }
        if self.match_kind(&TokenKind::For) {
            return self.for_statement();
        }
        if self.match_kind(&TokenKind::Switch) {
            return self.switch_statement();
        }
        if self.match_kind(&TokenKind::Try) {
            return self.try_statement();
        }
        if self.match_kind(&TokenKind::Break) {
            self.consume_optional_semicolon();
            return Ok(Stmt::Break);
        }
        if self.match_kind(&TokenKind::Continue) {
            self.consume_optional_semicolon();
            return Ok(Stmt::Continue);
        }
        if self.match_kind(&TokenKind::Throw) {
            return self.throw_statement();
        }
        if self.match_kind(&TokenKind::Return) {
            return self.return_statement();
        }
        if self.match_kind(&TokenKind::Let) {
            return self.var_decl(DeclKind::Let);
        }
        if self.match_kind(&TokenKind::Const) {
            return self.var_decl(DeclKind::Const);
        }
        if self.match_kind(&TokenKind::Var) {
            return self.var_decl(DeclKind::Var);
        }

        let expr = self.expression()?;
        self.consume_optional_semicolon();
        Ok(Stmt::Expr(expr))
    }

    fn block(&mut self) -> Result<Stmt> {
        Ok(Stmt::Block(self.block_statements()?))
    }

    fn block_statements(&mut self) -> Result<Vec<Stmt>> {
        let mut statements = Vec::new();
        while !self.check(&TokenKind::RBrace) {
            if self.at_end() {
                return Err(Error::parse("expected '}' after block", self.offset()));
            }
            statements.push(self.statement()?);
        }
        self.consume(&TokenKind::RBrace, "expected '}' after block")?;
        Ok(statements)
    }

    fn if_statement(&mut self) -> Result<Stmt> {
        self.consume(&TokenKind::LParen, "expected '(' after 'if'")?;
        let condition = self.expression()?;
        self.consume(&TokenKind::RParen, "expected ')' after if condition")?;
        let consequent = Box::new(self.statement()?);
        let alternate = if self.match_kind(&TokenKind::Else) {
            Some(Box::new(self.statement()?))
        } else {
            None
        };
        Ok(Stmt::If {
            condition,
            consequent,
            alternate,
        })
    }

    fn while_statement(&mut self) -> Result<Stmt> {
        self.consume(&TokenKind::LParen, "expected '(' after 'while'")?;
        let condition = self.expression()?;
        self.consume(&TokenKind::RParen, "expected ')' after while condition")?;
        let body = Box::new(self.statement()?);
        Ok(Stmt::While { condition, body })
    }

    fn for_statement(&mut self) -> Result<Stmt> {
        self.consume(&TokenKind::LParen, "expected '(' after 'for'")?;
        let init = self.for_init()?;
        let condition = if self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.expression()?)
        };
        self.consume(&TokenKind::Semicolon, "expected ';' after for condition")?;
        let update = if self.check(&TokenKind::RParen) {
            None
        } else {
            Some(self.expression()?)
        };
        self.consume(&TokenKind::RParen, "expected ')' after for clauses")?;
        let body = Box::new(self.statement()?);
        Ok(Stmt::For {
            init,
            condition,
            update,
            body,
        })
    }

    fn for_init(&mut self) -> Result<Option<Box<Stmt>>> {
        if self.match_kind(&TokenKind::Semicolon) {
            return Ok(None);
        }
        if self.match_kind(&TokenKind::Let) {
            return self.for_var_decl(DeclKind::Let).map(Box::new).map(Some);
        }
        if self.match_kind(&TokenKind::Const) {
            return self.for_var_decl(DeclKind::Const).map(Box::new).map(Some);
        }
        if self.match_kind(&TokenKind::Var) {
            return self.for_var_decl(DeclKind::Var).map(Box::new).map(Some);
        }
        let expr = self.expression()?;
        self.consume(&TokenKind::Semicolon, "expected ';' after for initializer")?;
        Ok(Some(Box::new(Stmt::Expr(expr))))
    }

    fn switch_statement(&mut self) -> Result<Stmt> {
        self.consume(&TokenKind::LParen, "expected '(' after 'switch'")?;
        let discriminant = self.expression()?;
        self.consume(&TokenKind::RParen, "expected ')' after switch discriminant")?;
        self.consume(&TokenKind::LBrace, "expected '{' before switch body")?;

        let mut cases = Vec::new();
        let mut default_seen = false;
        while !self.check(&TokenKind::RBrace) {
            if self.match_kind(&TokenKind::Case) {
                let test = self.expression()?;
                cases.push(self.switch_case(Some(test))?);
                continue;
            }
            if self.match_kind(&TokenKind::Default) {
                if default_seen {
                    return Err(Error::parse(
                        "switch contains multiple defaults",
                        self.offset(),
                    ));
                }
                default_seen = true;
                cases.push(self.switch_case(None)?);
                continue;
            }
            return Err(Error::parse(
                "expected 'case', 'default', or '}' in switch",
                self.offset(),
            ));
        }
        self.consume(&TokenKind::RBrace, "expected '}' after switch body")?;
        Ok(Stmt::Switch {
            discriminant,
            cases,
        })
    }

    fn switch_case(&mut self, test: Option<Expr>) -> Result<SwitchCase> {
        self.consume(&TokenKind::Colon, "expected ':' after switch label")?;
        let mut statements = Vec::new();
        while !self.check(&TokenKind::Case)
            && !self.check(&TokenKind::Default)
            && !self.check(&TokenKind::RBrace)
        {
            if self.at_end() {
                return Err(Error::parse(
                    "expected '}' after switch body",
                    self.offset(),
                ));
            }
            statements.push(self.statement()?);
        }
        Ok(SwitchCase { test, statements })
    }

    fn try_statement(&mut self) -> Result<Stmt> {
        self.consume(&TokenKind::LBrace, "expected '{' after 'try'")?;
        let body = self.block_statements()?;
        let catch = if self.match_kind(&TokenKind::Catch) {
            Some(self.catch_clause()?)
        } else {
            None
        };
        let finally_body = if self.match_kind(&TokenKind::Finally) {
            self.consume(&TokenKind::LBrace, "expected '{' after 'finally'")?;
            Some(self.block_statements()?)
        } else {
            None
        };
        if catch.is_none() && finally_body.is_none() {
            return Err(Error::parse(
                "expected 'catch' or 'finally' after try block",
                self.offset(),
            ));
        }
        Ok(Stmt::Try {
            body,
            catch,
            finally_body,
        })
    }

    fn catch_clause(&mut self) -> Result<CatchClause> {
        self.consume(&TokenKind::LParen, "expected '(' after 'catch'")?;
        let param = self.consume_identifier("expected catch binding name")?;
        self.consume(&TokenKind::RParen, "expected ')' after catch binding")?;
        self.consume(&TokenKind::LBrace, "expected '{' after catch binding")?;
        let body = self.block_statements()?;
        Ok(CatchClause { param, body })
    }

    fn throw_statement(&mut self) -> Result<Stmt> {
        let value = self.expression()?;
        self.consume_optional_semicolon();
        Ok(Stmt::Throw(value))
    }

    fn return_statement(&mut self) -> Result<Stmt> {
        let value =
            if self.check(&TokenKind::Semicolon) || self.check(&TokenKind::RBrace) || self.at_end()
            {
                None
            } else {
                Some(self.expression()?)
            };
        self.consume_optional_semicolon();
        Ok(Stmt::Return(value))
    }

    fn var_decl(&mut self, kind: DeclKind) -> Result<Stmt> {
        let declarations = self.var_declarations(kind)?;
        self.consume_optional_semicolon();
        self.declarations_stmt(declarations)
    }

    fn for_var_decl(&mut self, kind: DeclKind) -> Result<Stmt> {
        let declarations = self.var_declarations(kind)?;
        self.consume(&TokenKind::Semicolon, "expected ';' after for initializer")?;
        self.declarations_stmt(declarations)
    }

    fn var_declarations(&mut self, kind: DeclKind) -> Result<Vec<Stmt>> {
        let mut declarations = Vec::new();
        loop {
            let name = self.consume_identifier("expected binding name")?;
            let init = if self.match_kind(&TokenKind::Equal) {
                Some(self.expression()?)
            } else if kind == DeclKind::Const {
                return Err(Error::parse(
                    "const declaration requires an initializer",
                    self.offset(),
                ));
            } else {
                None
            };
            declarations.push(Stmt::VarDecl { name, kind, init });
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }
        Ok(declarations)
    }

    fn declarations_stmt(&self, declarations: Vec<Stmt>) -> Result<Stmt> {
        if declarations.len() == 1 {
            let mut declarations = declarations.into_iter();
            let Some(declaration) = declarations.next() else {
                return Err(Error::parse("expected binding declaration", self.offset()));
            };
            Ok(declaration)
        } else {
            Ok(Stmt::DeclList(declarations))
        }
    }

    fn expression(&mut self) -> Result<Expr> {
        self.with_expression_depth(Self::assignment)
    }

    fn conditional(&mut self) -> Result<Expr> {
        let condition = self.logical_or()?;
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

    fn logical_or(&mut self) -> Result<Expr> {
        self.left_assoc(
            Self::logical_and,
            &[(&TokenKind::OrOr, BinaryOp::LogicalOr)],
        )
    }

    fn logical_and(&mut self) -> Result<Expr> {
        self.left_assoc(
            Self::bitwise_and,
            &[(&TokenKind::AndAnd, BinaryOp::LogicalAnd)],
        )
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
        if self.match_kind(&TokenKind::New) {
            return self.new_expr();
        }
        if self.match_kind(&TokenKind::PlusPlus) {
            let offset = self.previous_offset();
            let expr = self.unary()?;
            return Self::update_expr(UpdateOp::Increment, true, expr, offset);
        }
        if self.match_kind(&TokenKind::MinusMinus) {
            let offset = self.previous_offset();
            let expr = self.unary()?;
            return Self::update_expr(UpdateOp::Decrement, true, expr, offset);
        }
        if self.match_kind(&TokenKind::Typeof) {
            let expr = self.unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Typeof,
                expr: Box::new(expr),
            });
        }
        if self.match_kind(&TokenKind::Void) {
            let expr = self.unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Void,
                expr: Box::new(expr),
            });
        }
        if self.match_kind(&TokenKind::Delete) {
            let expr = self.unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Delete,
                expr: Box::new(expr),
            });
        }
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

    fn new_expr(&mut self) -> Result<Expr> {
        let constructor = self.consume_identifier("expected constructor name after 'new'")?;
        self.consume(&TokenKind::LParen, "expected '(' after constructor name")?;
        let args = self.arguments()?;
        self.consume(&TokenKind::RParen, "expected ')' after arguments")?;
        Ok(Expr::New { constructor, args })
    }

    fn call(&mut self) -> Result<Expr> {
        let mut expr = self.primary()?;
        loop {
            if self.match_kind(&TokenKind::Dot) {
                let property = self.consume_identifier("expected property name after '.'")?;
                expr = Expr::Member {
                    object: Box::new(expr),
                    property,
                };
                continue;
            }
            if self.match_kind(&TokenKind::LBracket) {
                let property = self.expression()?;
                self.consume(
                    &TokenKind::RBracket,
                    "expected ']' after property expression",
                )?;
                expr = Expr::ComputedMember {
                    object: Box::new(expr),
                    property: Box::new(property),
                };
                continue;
            }
            if !self.match_kind(&TokenKind::LParen) {
                break;
            }
            let args = if self.check(&TokenKind::RParen) {
                Vec::new()
            } else {
                self.arguments()?
            };
            self.consume(&TokenKind::RParen, "expected ')' after arguments")?;
            expr = Expr::Call {
                callee: Box::new(expr),
                args,
            };
        }
        if self.match_kind(&TokenKind::PlusPlus) {
            return Self::update_expr(UpdateOp::Increment, false, expr, self.previous_offset());
        }
        if self.match_kind(&TokenKind::MinusMinus) {
            return Self::update_expr(UpdateOp::Decrement, false, expr, self.previous_offset());
        }
        Ok(expr)
    }

    fn update_expr(op: UpdateOp, prefix: bool, expr: Expr, offset: usize) -> Result<Expr> {
        if !matches!(
            expr,
            Expr::Identifier(_) | Expr::Member { .. } | Expr::ComputedMember { .. }
        ) {
            return Err(Error::parse("invalid update target", offset));
        }
        Ok(Expr::Update {
            op,
            prefix,
            expr: Box::new(expr),
        })
    }

    fn arguments(&mut self) -> Result<Vec<Expr>> {
        let mut args = Vec::new();
        loop {
            args.push(self.expression()?);
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }
        Ok(args)
    }

    fn primary(&mut self) -> Result<Expr> {
        let token = self
            .advance()
            .ok_or_else(|| Error::parse("expected expression", self.offset()))?;
        let expr = match token.kind {
            TokenKind::Number(value) => Expr::Literal(Value::Number(value)),
            TokenKind::String(value) => Expr::Literal(Value::String(value)),
            TokenKind::True => Expr::Literal(Value::Bool(true)),
            TokenKind::False => Expr::Literal(Value::Bool(false)),
            TokenKind::Null => Expr::Literal(Value::Null),
            TokenKind::Undefined => Expr::Literal(Value::Undefined),
            TokenKind::Identifier(name) => Expr::Identifier(name),
            TokenKind::Function => self.function_expression()?,
            TokenKind::LBrace => self.object_literal()?,
            TokenKind::LBracket => self.array_literal()?,
            TokenKind::LParen => {
                let expr = self.expression()?;
                self.consume(&TokenKind::RParen, "expected ')' after expression")?;
                expr
            }
            _ => return Err(Error::parse("expected expression", token.offset)),
        };
        Ok(expr)
    }

    fn object_literal(&mut self) -> Result<Expr> {
        let mut properties = Vec::new();
        if self.match_kind(&TokenKind::RBrace) {
            return Ok(Expr::Object(properties));
        }

        loop {
            let key = self.object_property_key()?;
            self.consume(&TokenKind::Colon, "expected ':' after object property name")?;
            let value = self.expression()?;
            properties.push(ObjectProperty { key, value });
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
            if self.match_kind(&TokenKind::RBrace) {
                return Ok(Expr::Object(properties));
            }
        }

        self.consume(&TokenKind::RBrace, "expected '}' after object literal")?;
        Ok(Expr::Object(properties))
    }

    fn array_literal(&mut self) -> Result<Expr> {
        let mut elements = Vec::new();
        if self.match_kind(&TokenKind::RBracket) {
            return Ok(Expr::Array(elements));
        }

        loop {
            elements.push(self.expression()?);
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
            if self.match_kind(&TokenKind::RBracket) {
                return Ok(Expr::Array(elements));
            }
        }

        self.consume(&TokenKind::RBracket, "expected ']' after array literal")?;
        Ok(Expr::Array(elements))
    }

    fn object_property_key(&mut self) -> Result<String> {
        let token = self
            .advance()
            .ok_or_else(|| Error::parse("expected object property name", self.offset()))?;
        match token.kind {
            TokenKind::Identifier(name) | TokenKind::String(name) => Ok(name),
            _ => Err(Error::parse("expected object property name", token.offset)),
        }
    }

    fn function_expression(&mut self) -> Result<Expr> {
        self.consume(&TokenKind::LParen, "expected '(' after 'function'")?;
        let params = self.function_parameters()?;
        self.consume(&TokenKind::RParen, "expected ')' after function parameters")?;
        self.consume(&TokenKind::LBrace, "expected '{' before function body")?;
        let body = self.block_statements()?;
        Ok(Expr::Function { params, body })
    }

    fn function_parameters(&mut self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(params);
        }

        loop {
            let name = self.consume_identifier("expected function parameter name")?;
            params.push(name);
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }

        Ok(params)
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
                return Err(Error::parse("expected operator", self.offset()));
            }
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
        self.expression_depth = self
            .expression_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("expression nesting overflowed"))?;
        if self.expression_depth > self.limits.max_expression_depth {
            self.expression_depth = self.expression_depth.saturating_sub(1);
            return Err(Error::limit(format!(
                "expression nesting exceeded {}",
                self.limits.max_expression_depth
            )));
        }
        let result = parse(self);
        self.expression_depth = self.expression_depth.saturating_sub(1);
        result
    }

    fn consume_identifier(&mut self, message: &str) -> Result<String> {
        let token = self
            .advance()
            .ok_or_else(|| Error::parse(message, self.offset()))?;
        match token.kind {
            TokenKind::Identifier(name) => Ok(name),
            _ => Err(Error::parse(message, token.offset)),
        }
    }

    fn consume(&mut self, expected: &TokenKind, message: &str) -> Result<()> {
        if self.check(expected) {
            if self.advance().is_some() {
                Ok(())
            } else {
                Err(Error::parse(message, self.offset()))
            }
        } else {
            Err(Error::parse(message, self.offset()))
        }
    }

    fn consume_optional_semicolon(&mut self) {
        self.match_kind(&TokenKind::Semicolon);
    }

    fn match_kind(&mut self, expected: &TokenKind) -> bool {
        if self.check(expected) {
            self.advance().is_some()
        } else {
            false
        }
    }

    fn check(&self, expected: &TokenKind) -> bool {
        self.peek()
            .is_some_and(|token| token_kind_eq(&token.kind, expected))
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.peek()?.clone();
        if !matches!(token.kind, TokenKind::Eof) {
            self.cursor = self.cursor.saturating_add(1);
        }
        Some(token)
    }

    fn at_end(&self) -> bool {
        self.peek()
            .is_none_or(|token| matches!(token.kind, TokenKind::Eof))
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.cursor)
    }

    fn offset(&self) -> usize {
        self.peek()
            .or_else(|| self.tokens.last())
            .map_or(0, |token| token.offset)
    }

    fn previous_offset(&self) -> usize {
        self.cursor
            .checked_sub(1)
            .and_then(|cursor| self.tokens.get(cursor))
            .map_or_else(|| self.offset(), |token| token.offset)
    }
}

fn token_kind_eq(left: &TokenKind, right: &TokenKind) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}
