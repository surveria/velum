use crate::{
    ast::{Expr, FunctionParam, ObjectProperty, ObjectPropertyKey, StaticName, UnaryOp, UpdateOp},
    error::{Error, Result},
    lexer::TokenKind,
    value::Value,
};

use super::{Parser, SUPER_IDENTIFIER_NAME};

const THIS_PROPERTY_NAME: &str = "this";
const NEW_TARGET_PROPERTY_NAME: &str = "target";
const IMPORT_BINDING_NAME: &str = "import";

enum ObjectPropertyName {
    Static {
        key: StaticName,
        shorthand_name: Option<StaticName>,
    },
    Computed(Expr),
}

#[derive(Debug, Clone, Copy)]
enum ArrowParameters {
    Single,
    Parenthesized,
}

#[derive(Debug, Clone, Copy)]
struct ArrowSignature {
    is_async: bool,
    parameters: ArrowParameters,
}

impl Parser {
    pub(super) fn expression(&mut self) -> Result<Expr> {
        self.with_expression_depth(Self::assignment)
    }

    pub(super) fn unary(&mut self) -> Result<Expr> {
        if self.match_kind(&TokenKind::Await) {
            let expr = self.unary()?;
            return Ok(Expr::Await(Box::new(expr)));
        }
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

    pub(super) fn call(&mut self) -> Result<Expr> {
        let expr = self.primary()?;
        self.call_suffix(expr)
    }

    fn call_suffix(&mut self, mut expr: Expr) -> Result<Expr> {
        loop {
            if self.match_kind(&TokenKind::Dot) {
                let property = self.consume_property_name("expected property name after '.'")?;
                let access = self.static_property_access()?;
                expr = Expr::Member {
                    object: Box::new(expr),
                    property,
                    access,
                };
                continue;
            }
            if self.match_kind(&TokenKind::LBracket) {
                let property = self.expression()?;
                self.consume(
                    &TokenKind::RBracket,
                    "expected ']' after property expression",
                )?;
                if let Some(property) = self.static_computed_property_key(&property)? {
                    let access = self.static_property_access()?;
                    expr = Expr::Member {
                        object: Box::new(expr),
                        property,
                        access,
                    };
                    continue;
                }
                let access = self.static_property_access()?;
                expr = Expr::ComputedMember {
                    object: Box::new(expr),
                    property: Box::new(property),
                    access,
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
            let site = self.static_call_site()?;
            expr = Expr::Call {
                callee: Box::new(expr),
                site,
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

    pub(super) fn assignment_target(expr: Expr) -> Option<Expr> {
        match expr {
            Expr::Identifier(_) | Expr::Member { .. } | Expr::ComputedMember { .. } => Some(expr),
            Expr::Parenthesized(expr) => Self::assignment_target(*expr),
            _ => None,
        }
    }

    fn new_expr(&mut self) -> Result<Expr> {
        let new_offset = self.previous_offset();
        if self.match_kind(&TokenKind::Dot) {
            let expr = self.new_target_expr(new_offset)?;
            return self.call_suffix(expr);
        }
        let constructor = self.primary()?;
        let constructor = self.member_suffix(constructor)?;
        if Self::constructor_starts_with_import(&constructor) {
            return Err(Error::parse(
                "import call cannot be used as a constructor",
                new_offset,
            ));
        }
        if !self.match_kind(&TokenKind::LParen) {
            return Err(Error::parse(
                "expected '(' after constructor expression",
                self.offset(),
            ));
        }
        let args = if self.check(&TokenKind::RParen) {
            Vec::new()
        } else {
            self.arguments()?
        };
        self.consume(&TokenKind::RParen, "expected ')' after arguments")?;
        let expr = Expr::New {
            constructor: Box::new(constructor),
            args,
        };
        self.call_suffix(expr)
    }

    fn constructor_starts_with_import(expr: &Expr) -> bool {
        match expr {
            Expr::Identifier(name) => name.as_str() == IMPORT_BINDING_NAME,
            Expr::Member { object, .. } | Expr::ComputedMember { object, .. } => {
                Self::constructor_starts_with_import(object)
            }
            Expr::Parenthesized(expr) => Self::constructor_starts_with_import(expr),
            _ => false,
        }
    }

    fn member_suffix(&mut self, mut expr: Expr) -> Result<Expr> {
        loop {
            if self.match_kind(&TokenKind::Dot) {
                let property = self.consume_property_name("expected property name after '.'")?;
                let access = self.static_property_access()?;
                expr = Expr::Member {
                    object: Box::new(expr),
                    property,
                    access,
                };
                continue;
            }
            if !self.match_kind(&TokenKind::LBracket) {
                break;
            }
            let property = self.expression()?;
            self.consume(
                &TokenKind::RBracket,
                "expected ']' after property expression",
            )?;
            if let Some(property) = self.static_computed_property_key(&property)? {
                let access = self.static_property_access()?;
                expr = Expr::Member {
                    object: Box::new(expr),
                    property,
                    access,
                };
                continue;
            }
            let access = self.static_property_access()?;
            expr = Expr::ComputedMember {
                object: Box::new(expr),
                property: Box::new(property),
                access,
            };
        }
        Ok(expr)
    }

    fn new_target_expr(&mut self, new_offset: usize) -> Result<Expr> {
        let token = self
            .advance()
            .ok_or_else(|| Error::parse("expected 'target' after 'new.'", self.offset()))?;
        let TokenKind::Identifier(name) = token.kind else {
            return Err(Error::parse("expected 'target' after 'new.'", token.offset));
        };
        if name != NEW_TARGET_PROPERTY_NAME {
            return Err(Error::parse("expected 'target' after 'new.'", token.offset));
        }
        if !self.allows_new_target() {
            return Err(Error::parse(
                "new.target is only valid inside functions",
                new_offset,
            ));
        }
        Ok(Expr::NewTarget)
    }

    fn update_expr(op: UpdateOp, prefix: bool, expr: Expr, offset: usize) -> Result<Expr> {
        let expr = Self::assignment_target(expr)
            .ok_or_else(|| Error::parse("invalid update target", offset))?;
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
            TokenKind::String(value) => Expr::StringLiteral(self.static_string(value)?),
            TokenKind::RegExp { pattern, flags } => Expr::RegExpLiteral {
                pattern: self.static_string(pattern)?,
                flags: self.static_string(flags)?,
            },
            TokenKind::True => Expr::Literal(Value::Bool(true)),
            TokenKind::False => Expr::Literal(Value::Bool(false)),
            TokenKind::Null => Expr::Literal(Value::Null),
            TokenKind::Undefined => Expr::Literal(Value::Undefined),
            TokenKind::This => Expr::This,
            TokenKind::Identifier(name) if name == SUPER_IDENTIFIER_NAME => {
                return Err(Error::parse(
                    "super is only valid inside class methods",
                    token.offset,
                ));
            }
            TokenKind::Identifier(name) => Expr::Identifier(self.static_binding_name(name)?),
            TokenKind::Function => self.function_expression(false)?,
            TokenKind::Async => {
                if self.peek_kind_is_no_line_terminator(0, &TokenKind::Function) {
                    self.consume(&TokenKind::Function, "expected 'function' after 'async'")?;
                    self.function_expression(true)?
                } else {
                    Expr::Identifier(self.contextual_async_binding(token.offset)?)
                }
            }
            TokenKind::LBrace => self.object_literal()?,
            TokenKind::LBracket => self.array_literal()?,
            TokenKind::LParen => {
                let expr = self.expression()?;
                self.consume(&TokenKind::RParen, "expected ')' after expression")?;
                Expr::Parenthesized(Box::new(expr))
            }
            _ => return Err(Error::parse("expected expression", token.offset)),
        };
        Ok(expr)
    }

    pub(super) fn arrow_function(&mut self) -> Result<Option<Expr>> {
        let Some(signature) = self.arrow_signature() else {
            return Ok(None);
        };
        if signature.is_async {
            self.consume(
                &TokenKind::Async,
                "expected 'async' before async arrow function",
            )?;
        }
        let params = match signature.parameters {
            ArrowParameters::Single => vec![FunctionParam::new(
                self.consume_binding_identifier("expected arrow function parameter")?,
                None,
            )],
            ArrowParameters::Parenthesized => {
                self.consume(&TokenKind::LParen, "expected '(' before arrow parameters")?;
                let params = self.function_parameters()?;
                self.consume(&TokenKind::RParen, "expected ')' after arrow parameters")?;
                params
            }
        };
        self.consume(&TokenKind::Arrow, "expected '=>' after arrow parameters")?;
        let body = self.arrow_body()?;
        let id = self.static_function()?;
        Ok(Some(Expr::ArrowFunction {
            id,
            params: params.into(),
            body,
            is_async: signature.is_async,
        }))
    }

    fn arrow_body(&mut self) -> Result<std::rc::Rc<[crate::ast::Stmt]>> {
        if self.match_kind(&TokenKind::LBrace) {
            return Ok(self.block_statements()?.into());
        }
        let value = self.assignment()?;
        Ok(std::rc::Rc::from(
            vec![crate::ast::Stmt::Return(Some(value))].into_boxed_slice(),
        ))
    }

    fn arrow_signature(&self) -> Option<ArrowSignature> {
        match self.peek_kind(0)? {
            TokenKind::Identifier(_) | TokenKind::Async
                if self.peek_kind_is_no_line_terminator(1, &TokenKind::Arrow) =>
            {
                Some(ArrowSignature {
                    is_async: false,
                    parameters: ArrowParameters::Single,
                })
            }
            TokenKind::LParen if self.parenthesized_arrow_end(0).is_some() => {
                Some(ArrowSignature {
                    is_async: false,
                    parameters: ArrowParameters::Parenthesized,
                })
            }
            TokenKind::Async => self.async_arrow_signature(),
            _ => None,
        }
    }

    fn async_arrow_signature(&self) -> Option<ArrowSignature> {
        match self.peek_kind(1)? {
            _ if !self.peek_has_line_terminator_before(1)
                && self.peek_is_identifier_name(1)
                && self.peek_kind_is_no_line_terminator(2, &TokenKind::Arrow) =>
            {
                Some(ArrowSignature {
                    is_async: true,
                    parameters: ArrowParameters::Single,
                })
            }
            TokenKind::LParen
                if !self.peek_has_line_terminator_before(1)
                    && self.parenthesized_arrow_end(1).is_some() =>
            {
                Some(ArrowSignature {
                    is_async: true,
                    parameters: ArrowParameters::Parenthesized,
                })
            }
            _ => None,
        }
    }

    fn parenthesized_arrow_end(&self, lparen_offset: usize) -> Option<usize> {
        if !self.peek_kind_is(lparen_offset, &TokenKind::LParen) {
            return None;
        }
        let mut offset = lparen_offset;
        let mut depth = 0usize;
        loop {
            let kind = self.peek_kind(offset)?;
            match kind {
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                    depth = depth.checked_add(1)?;
                }
                TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        if !self.peek_kind_is(offset, &TokenKind::RParen) {
                            return None;
                        }
                        let arrow = offset.checked_add(1)?;
                        return self
                            .peek_kind_is_no_line_terminator(arrow, &TokenKind::Arrow)
                            .then_some(arrow);
                    }
                }
                TokenKind::Eof => return None,
                _ => {}
            }
            offset = offset.checked_add(1)?;
        }
    }
    fn object_literal(&mut self) -> Result<Expr> {
        let mut properties = Vec::new();
        if self.match_kind(&TokenKind::RBrace) {
            return Ok(Expr::Object(properties));
        }

        loop {
            properties.push(self.object_literal_property()?);
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

    fn object_literal_property(&mut self) -> Result<ObjectProperty> {
        if self.async_object_method_start() {
            self.consume(
                &TokenKind::Async,
                "expected 'async' before async object method",
            )?;
            let name = self.object_property_key()?;
            return self.object_method_property(name, true);
        }
        let name = self.object_property_key()?;
        if self.match_kind(&TokenKind::Colon) {
            let value = self.expression()?;
            return Ok(ObjectProperty {
                key: name.into_key(),
                value,
            });
        }
        if self.match_kind(&TokenKind::LParen) {
            return self.object_method_property_after_lparen(name, false);
        }
        if let ObjectPropertyName::Static {
            key,
            shorthand_name: Some(binding),
        } = name
        {
            let binding = self.static_binding(binding)?;
            return Ok(ObjectProperty {
                key: ObjectPropertyKey::Static(key),
                value: Expr::Identifier(binding),
            });
        }
        Err(Error::parse(
            "expected ':' after object property name",
            self.offset(),
        ))
    }

    fn async_object_method_start(&self) -> bool {
        self.peek_kind_is(0, &TokenKind::Async)
            && !self.peek_has_line_terminator_before(1)
            && self.peek_kind(1).is_some_and(is_object_property_name_start)
    }

    fn object_method_property(
        &mut self,
        name: ObjectPropertyName,
        is_async: bool,
    ) -> Result<ObjectProperty> {
        self.consume(&TokenKind::LParen, "expected '(' after object method name")?;
        self.object_method_property_after_lparen(name, is_async)
    }

    fn object_method_property_after_lparen(
        &mut self,
        name: ObjectPropertyName,
        is_async: bool,
    ) -> Result<ObjectProperty> {
        let params = self.function_parameters()?.into();
        self.consume(&TokenKind::RParen, "expected ')' after method parameters")?;
        self.consume(&TokenKind::LBrace, "expected '{' before method body")?;
        let body = self.with_new_target_scope(Self::block_statements)?.into();
        let id = self.static_function()?;
        let key = name.into_key();
        let name = match &key {
            ObjectPropertyKey::Static(name) => Some(name.clone()),
            ObjectPropertyKey::Computed(_) => None,
        };
        let value = Expr::MethodFunction {
            id,
            name,
            params,
            body,
            is_async,
        };
        Ok(ObjectProperty { key, value })
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

    fn object_property_key(&mut self) -> Result<ObjectPropertyName> {
        if self.match_kind(&TokenKind::LBracket) {
            let expr = self.expression()?;
            self.consume(
                &TokenKind::RBracket,
                "expected ']' after computed object property name",
            )?;
            return Ok(ObjectPropertyName::Computed(expr));
        }
        let token = self
            .advance()
            .ok_or_else(|| Error::parse("expected object property name", self.offset()))?;
        match token.kind {
            TokenKind::Identifier(name) => {
                let name = self.static_name(name)?;
                Ok(ObjectPropertyName::Static {
                    key: name.clone(),
                    shorthand_name: Some(name),
                })
            }
            TokenKind::String(name) => Ok(ObjectPropertyName::Static {
                key: self.static_name(name)?,
                shorthand_name: None,
            }),
            TokenKind::Number(value) => Ok(ObjectPropertyName::Static {
                key: self.static_name(Value::Number(value).to_string())?,
                shorthand_name: None,
            }),
            kind => {
                let Some(name) = keyword_property_name(&kind) else {
                    return Err(Error::parse("expected object property name", token.offset));
                };
                self.keyword_property_name(name)
            }
        }
    }

    fn keyword_property_name(&mut self, name: &str) -> Result<ObjectPropertyName> {
        Ok(ObjectPropertyName::Static {
            key: self.borrowed_static_name(name)?,
            shorthand_name: None,
        })
    }

    fn function_expression(&mut self, is_async: bool) -> Result<Expr> {
        let name = if self.next_is_identifier() {
            Some(self.consume_identifier("expected function name")?)
        } else {
            None
        };
        self.consume(&TokenKind::LParen, "expected '(' after 'function'")?;
        let params = self.function_parameters()?.into();
        self.consume(&TokenKind::RParen, "expected ')' after function parameters")?;
        self.consume(&TokenKind::LBrace, "expected '{' before function body")?;
        let body = self.with_new_target_scope(Self::block_statements)?.into();
        let id = self.static_function()?;
        Ok(Expr::Function {
            id,
            name,
            params,
            body,
            is_async,
        })
    }

    pub(super) fn function_parameters(&mut self) -> Result<Vec<FunctionParam>> {
        let mut params = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(params);
        }

        loop {
            if self.check(&TokenKind::RParen) {
                break;
            }
            let name = self.consume_binding_identifier("expected function parameter name")?;
            let default = if self.match_kind(&TokenKind::Equal) {
                Some(self.assignment()?)
            } else {
                None
            };
            params.push(FunctionParam::new(name, default));
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }

        Ok(params)
    }

    fn with_expression_depth(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<Expr>,
    ) -> Result<Expr> {
        self.expression_depth = self
            .expression_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("expression nesting overflowed"))?;
        self.max_expression_depth = self.max_expression_depth.max(self.expression_depth);
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

    fn consume_property_name(&mut self, message: &str) -> Result<StaticName> {
        let token = self
            .advance()
            .ok_or_else(|| Error::parse(message, self.offset()))?;
        match token.kind {
            TokenKind::Identifier(name) => self.static_name(name),
            kind => {
                let Some(name) = keyword_property_name(&kind) else {
                    return Err(Error::parse(message, token.offset));
                };
                self.borrowed_static_name(name)
            }
        }
    }

    fn static_computed_property_key(&mut self, property: &Expr) -> Result<Option<StaticName>> {
        match property {
            Expr::StringLiteral(value) => self.borrowed_static_name(value.as_str()).map(Some),
            Expr::Literal(
                value @ (Value::Undefined | Value::Null | Value::Bool(_) | Value::Number(_)),
            ) => self.static_name(value.to_string()).map(Some),
            _ => Ok(None),
        }
    }
}

impl ObjectPropertyName {
    fn into_key(self) -> ObjectPropertyKey {
        match self {
            Self::Static { key, .. } => ObjectPropertyKey::Static(key),
            Self::Computed(expr) => ObjectPropertyKey::Computed(Box::new(expr)),
        }
    }
}

const fn keyword_property_name(kind: &TokenKind) -> Option<&'static str> {
    match kind {
        TokenKind::This => Some(THIS_PROPERTY_NAME),
        TokenKind::Let => Some("let"),
        TokenKind::Const => Some("const"),
        TokenKind::Var => Some("var"),
        TokenKind::If => Some("if"),
        TokenKind::Else => Some("else"),
        TokenKind::Do => Some("do"),
        TokenKind::While => Some("while"),
        TokenKind::For => Some("for"),
        TokenKind::Switch => Some("switch"),
        TokenKind::Case => Some("case"),
        TokenKind::Default => Some("default"),
        TokenKind::Break => Some("break"),
        TokenKind::Continue => Some("continue"),
        TokenKind::Try => Some("try"),
        TokenKind::Catch => Some("catch"),
        TokenKind::Finally => Some("finally"),
        TokenKind::Throw => Some("throw"),
        TokenKind::Return => Some("return"),
        TokenKind::Function => Some("function"),
        TokenKind::Async => Some("async"),
        TokenKind::Await => Some("await"),
        TokenKind::New => Some("new"),
        TokenKind::In => Some("in"),
        TokenKind::InstanceOf => Some("instanceof"),
        TokenKind::Typeof => Some("typeof"),
        TokenKind::Void => Some("void"),
        TokenKind::Delete => Some("delete"),
        TokenKind::True => Some("true"),
        TokenKind::False => Some("false"),
        TokenKind::Null => Some("null"),
        TokenKind::Undefined => Some("undefined"),
        _ => None,
    }
}

const fn is_object_property_name_start(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Identifier(_)
            | TokenKind::String(_)
            | TokenKind::Number(_)
            | TokenKind::LBracket
    ) || keyword_property_name(kind).is_some()
}
