use crate::{
    ast::{CatchClause, DeclKind, Expr, ForInTarget, Stmt, SwitchCase},
    error::{Error, Result},
    lexer::TokenKind,
};

use super::Parser;

impl Parser {
    pub(super) fn statement(&mut self) -> Result<Stmt> {
        self.with_statement_depth(Self::statement_inner)
    }

    fn statement_inner(&mut self) -> Result<Stmt> {
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
        if self.check(&TokenKind::Async)
            && self.peek_kind_is_no_line_terminator(1, &TokenKind::Function)
        {
            self.consume(&TokenKind::Async, "expected 'async' before async function")?;
            self.consume(&TokenKind::Function, "expected 'function' after 'async'")?;
            return self.function_declaration(true);
        }
        if self.match_kind(&TokenKind::Function) {
            return self.function_declaration(false);
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
        self.consume_statement_terminator("expected statement terminator")?;
        Ok(Stmt::Expr(expr))
    }

    fn with_statement_depth(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<Stmt>,
    ) -> Result<Stmt> {
        self.statement_depth = self
            .statement_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("statement nesting overflowed"))?;
        if self.statement_depth > self.limits.max_expression_depth {
            self.statement_depth = self.statement_depth.saturating_sub(1);
            return Err(Error::limit(format!(
                "statement nesting exceeded {}",
                self.limits.max_expression_depth
            )));
        }
        let result = parse(self);
        self.statement_depth = self.statement_depth.saturating_sub(1);
        result
    }

    pub(super) fn block_statements(&mut self) -> Result<Vec<Stmt>> {
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

    fn block(&mut self) -> Result<Stmt> {
        Ok(Stmt::Block(self.block_statements()?))
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
        let cursor = self.cursor;
        let expression_depth = self.expression_depth;
        let static_names = self.static_names.clone();
        let static_bindings = self.static_bindings.clone();
        let static_functions = self.static_functions.clone();
        if let Some((target, object)) = self.for_in_header()? {
            self.consume(&TokenKind::RParen, "expected ')' after for-in expression")?;
            let body = Box::new(self.statement()?);
            return Ok(Stmt::ForIn {
                target,
                object,
                body,
            });
        }
        self.cursor = cursor;
        self.expression_depth = expression_depth;
        self.static_names = static_names;
        self.static_bindings = static_bindings;
        self.static_functions = static_functions;

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

    fn for_in_header(&mut self) -> Result<Option<(ForInTarget, Expr)>> {
        if self.match_kind(&TokenKind::Let) {
            return self.for_in_binding_header(DeclKind::Let);
        }
        if self.match_kind(&TokenKind::Const) {
            return self.for_in_binding_header(DeclKind::Const);
        }
        if self.match_kind(&TokenKind::Var) {
            return self.for_in_binding_header(DeclKind::Var);
        }

        if !self.for_in_assignment_target_start() {
            return Ok(None);
        }
        let target = self.call()?;
        if !self.match_kind(&TokenKind::In) {
            return Ok(None);
        }
        let Some(target) = Self::assignment_target(target) else {
            return Err(Error::parse(
                "invalid for-in assignment target",
                self.offset(),
            ));
        };
        let object = self.expression()?;
        Ok(Some((ForInTarget::Assignment(target), object)))
    }

    fn for_in_binding_header(&mut self, kind: DeclKind) -> Result<Option<(ForInTarget, Expr)>> {
        let name = self.consume_binding_identifier("expected for-in binding name")?;
        if !self.match_kind(&TokenKind::In) {
            return Ok(None);
        }
        let object = self.expression()?;
        Ok(Some((ForInTarget::Binding { name, kind }, object)))
    }

    fn for_in_assignment_target_start(&self) -> bool {
        self.peek().is_some_and(|token| {
            matches!(
                &token.kind,
                TokenKind::Identifier(_) | TokenKind::Async | TokenKind::LParen
            )
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
        if self.match_kind(&TokenKind::LBrace) {
            let body = self.block_statements()?;
            return Ok(CatchClause { param: None, body });
        }
        self.consume(&TokenKind::LParen, "expected '(' or '{' after 'catch'")?;
        let param = self.consume_binding_identifier("expected catch binding name")?;
        self.consume(&TokenKind::RParen, "expected ')' after catch binding")?;
        self.consume(&TokenKind::LBrace, "expected '{' after catch binding")?;
        let body = self.block_statements()?;
        Ok(CatchClause {
            param: Some(param),
            body,
        })
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

    fn function_declaration(&mut self, is_async: bool) -> Result<Stmt> {
        let name = self.consume_binding_identifier("expected function declaration name")?;
        self.consume(&TokenKind::LParen, "expected '(' after function name")?;
        let params = self.function_parameters()?.into();
        self.consume(&TokenKind::RParen, "expected ')' after function parameters")?;
        self.consume(&TokenKind::LBrace, "expected '{' before function body")?;
        let body = self.with_new_target_scope(Self::block_statements)?.into();
        let id = self.static_function()?;
        Ok(Stmt::FunctionDecl {
            name,
            id,
            params,
            body,
            is_async,
        })
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
            let name = self.consume_binding_identifier("expected binding name")?;
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
}
