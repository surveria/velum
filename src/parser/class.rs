use std::rc::Rc;

use crate::{
    ast::{
        ClassConstructor, ClassLiteral, ClassMember, ClassMemberKind, Expr, Expression,
        ObjectPropertyKey, Statement, Stmt,
    },
    error::{Error, Result},
    lexer::TokenKind,
    syntax::StaticName,
};

use super::{Parser, literal::ObjectPropertyName};

/// One parsed class member function: its allocated static function id plus
/// parameters and body statements with pattern prologues applied.
struct ParsedClassFunction {
    id: crate::syntax::StaticFunctionId,
    params: Rc<[crate::ast::FunctionParam]>,
    body: Rc<[Statement]>,
}

const CLASS_STATIC_KEYWORD: &str = "static";
const CLASS_GETTER_KEYWORD: &str = "get";
const CLASS_SETTER_KEYWORD: &str = "set";
const CLASS_CONSTRUCTOR_NAME: &str = "constructor";
const CLASS_PROTOTYPE_NAME: &str = "prototype";
/// Synthesized rest parameter for default derived constructors.
const DERIVED_CONSTRUCTOR_REST_NAME: &str = "%superargs%";

impl Parser {
    /// Parses a class declaration after its consumed `class` keyword.
    pub(super) fn class_declaration(&mut self) -> Result<Stmt> {
        let name = self.consume_binding_identifier("expected class declaration name")?;
        let class = self.class_literal_tail(Some(name.name().clone()))?;
        Ok(Stmt::ClassDecl {
            name,
            class: Box::new(class),
        })
    }

    /// Parses a class expression after its consumed `class` keyword.
    pub(super) fn class_expression(&mut self) -> Result<Expression> {
        let start = self.previous_span();
        let name = if self.next_is_identifier() {
            Some(self.consume_identifier("expected class name")?)
        } else {
            None
        };
        let class = self.class_literal_tail(name)?;
        Ok(self.expression_node(start, Expr::Class(Box::new(class))))
    }

    fn class_literal_tail(&mut self, name: Option<StaticName>) -> Result<ClassLiteral> {
        let previous_strict = self.is_strict_mode();
        self.set_strict_mode(true);
        let heritage = if self.match_kind(&TokenKind::Extends) {
            Some(self.call()?)
        } else {
            None
        };
        let result = self.class_body_literal(name, heritage);
        self.set_strict_mode(previous_strict);
        result
    }

    fn class_body_literal(
        &mut self,
        name: Option<StaticName>,
        heritage: Option<Expression>,
    ) -> Result<ClassLiteral> {
        self.consume(&TokenKind::LBrace, "expected '{' before class body")?;
        let derived = heritage.is_some();
        let mut constructor = None;
        let mut members = Vec::new();
        let mut fields = Vec::new();
        while !self.check(&TokenKind::RBrace) {
            if self.at_end() {
                return Err(self.parse_error("expected '}' after class body"));
            }
            if self.match_kind(&TokenKind::Semicolon) {
                continue;
            }
            self.class_member(&mut constructor, &mut members, &mut fields, derived)?;
        }
        self.consume(&TokenKind::RBrace, "expected '}' after class body")?;
        let constructor = match constructor {
            Some(constructor) => constructor,
            None if derived => self.default_derived_class_constructor()?,
            None => self.default_class_constructor()?,
        };
        Ok(ClassLiteral {
            name,
            heritage,
            constructor,
            members,
            fields,
        })
    }

    fn default_class_constructor(&mut self) -> Result<ClassConstructor> {
        Ok(ClassConstructor {
            id: self.static_function()?,
            params: Vec::new().into(),
            body: Vec::new().into(),
        })
    }

    /// Synthesizes `constructor(...args) { super(...args); }` for derived
    /// classes without an explicit constructor.
    fn default_derived_class_constructor(&mut self) -> Result<ClassConstructor> {
        let span = self.previous_span();
        let rest = self.static_binding_name(DERIVED_CONSTRUCTOR_REST_NAME.to_owned())?;
        let rest_value = Expression::new(Expr::Identifier(rest.clone()), span);
        let spread = Expression::new(Expr::Spread(Box::new(rest_value)), span);
        let forward = Expression::new(Expr::SuperCall { args: vec![spread] }, span);
        Ok(ClassConstructor {
            id: self.static_function()?,
            params: vec![crate::ast::FunctionParam::rest(rest)].into(),
            body: vec![Statement::new(Stmt::Expr(forward), span)].into(),
        })
    }

    fn class_member(
        &mut self,
        constructor: &mut Option<ClassConstructor>,
        members: &mut Vec<ClassMember>,
        fields: &mut Vec<crate::ast::ClassField>,
        derived: bool,
    ) -> Result<()> {
        let member_offset = self.offset();
        self.reject_unsupported_class_member()?;
        let is_static = self.match_class_static_prefix();
        if is_static {
            self.reject_unsupported_class_member()?;
        }
        let accessor = self.match_class_accessor_prefix();
        let key = self.object_property_key()?;
        let key_name = Self::class_member_key_name(&key);

        if !self.check(&TokenKind::LParen) {
            if accessor.is_some() {
                return Err(self.parse_error("expected '(' after class accessor name"));
            }
            return self.class_field(key, key_name, is_static, member_offset, fields);
        }

        if let Some(name) = &key_name {
            if !is_static && name.as_str() == CLASS_CONSTRUCTOR_NAME {
                return self.class_constructor_member(
                    accessor,
                    constructor,
                    member_offset,
                    derived,
                );
            }
            if is_static && name.as_str() == CLASS_PROTOTYPE_NAME {
                return Err(Error::parse(
                    "class static member cannot be named 'prototype'",
                    member_offset,
                ));
            }
        }

        let kind = match accessor {
            Some(ClassMemberKind::Getter) => ClassMemberKind::Getter,
            Some(ClassMemberKind::Setter) => ClassMemberKind::Setter,
            Some(ClassMemberKind::Method) | None => ClassMemberKind::Method,
        };
        if is_static && kind != ClassMemberKind::Method {
            return Err(Error::parse(
                "class static accessors are not supported yet",
                member_offset,
            ));
        }
        let function = self.class_member_function(kind, member_offset, false)?;
        members.push(ClassMember {
            key: Self::class_property_key(key),
            kind,
            is_static,
            id: function.id,
            name: key_name,
            params: function.params,
            body: function.body,
        });
        Ok(())
    }

    /// Parses a public field after its key: an optional initializer followed
    /// by an optional semicolon.
    fn class_field(
        &mut self,
        key: ObjectPropertyName,
        key_name: Option<StaticName>,
        is_static: bool,
        member_offset: usize,
        fields: &mut Vec<crate::ast::ClassField>,
    ) -> Result<()> {
        if let Some(name) = &key_name {
            if name.as_str() == CLASS_CONSTRUCTOR_NAME {
                return Err(Error::parse(
                    "class field cannot be named 'constructor'",
                    member_offset,
                ));
            }
            if is_static && name.as_str() == CLASS_PROTOTYPE_NAME {
                return Err(Error::parse(
                    "class static member cannot be named 'prototype'",
                    member_offset,
                ));
            }
        }
        let initializer = if self.match_kind(&TokenKind::Equal) {
            Some(self.expression()?)
        } else {
            None
        };
        if self.check(&TokenKind::Semicolon) && self.advance().is_none() {
            return Err(self.parse_error("expected ';' after class field"));
        }
        fields.push(crate::ast::ClassField {
            key: Self::class_property_key(key),
            is_static,
            name: key_name,
            initializer,
        });
        Ok(())
    }

    fn class_constructor_member(
        &mut self,
        accessor: Option<ClassMemberKind>,
        constructor: &mut Option<ClassConstructor>,
        member_offset: usize,
        derived: bool,
    ) -> Result<()> {
        if accessor.is_some() {
            return Err(Error::parse(
                "class constructor cannot be an accessor",
                member_offset,
            ));
        }
        if constructor.is_some() {
            return Err(Error::parse(
                "class body cannot declare two constructors",
                member_offset,
            ));
        }
        let function =
            self.class_member_function(ClassMemberKind::Method, member_offset, derived)?;
        *constructor = Some(ClassConstructor {
            id: function.id,
            params: function.params,
            body: function.body,
        });
        Ok(())
    }

    /// Parses `( params ) { body }` for one class member and returns its
    /// static function id, parameters, and statements with pattern prologues
    /// applied.
    fn class_member_function(
        &mut self,
        kind: ClassMemberKind,
        member_offset: usize,
        allow_super_call: bool,
    ) -> Result<ParsedClassFunction> {
        self.consume(&TokenKind::LParen, "expected '(' after class member name")?;
        let parameters = self.function_parameters()?;
        self.consume(
            &TokenKind::RParen,
            "expected ')' after class member parameters",
        )?;
        match kind {
            ClassMemberKind::Getter if !parameters.params.is_empty() => {
                return Err(Error::parse(
                    "getter must not declare parameters",
                    member_offset,
                ));
            }
            ClassMemberKind::Setter if parameters.params.len() != 1 => {
                return Err(Error::parse(
                    "setter must declare exactly one parameter",
                    member_offset,
                ));
            }
            ClassMemberKind::Setter
                if parameters.params.first().is_some_and(|param| param.rest) =>
            {
                return Err(Error::parse(
                    "setter parameter cannot be a rest parameter",
                    member_offset,
                ));
            }
            ClassMemberKind::Method | ClassMemberKind::Getter | ClassMemberKind::Setter => {}
        }
        self.consume(&TokenKind::LBrace, "expected '{' before class member body")?;
        let body = self.with_new_target_scope(|parser| {
            parser.with_super_context(true, allow_super_call, |parser| parser.function_body(true))
        })?;
        self.validate_function_parameters(&parameters.params, true, body.contains_use_strict)?;
        let id = self.static_function()?;
        let (params, statements) = parameters.apply_prologue(body.statements);
        Ok(ParsedClassFunction {
            id,
            params: params.into(),
            body: statements.into(),
        })
    }

    /// Consumes a `static` member prefix when it is not itself a member name.
    fn match_class_static_prefix(&mut self) -> bool {
        let is_prefix = self.peek().is_some_and(|token| {
            matches!(&token.kind, TokenKind::Identifier(name) if name == CLASS_STATIC_KEYWORD)
        }) && !self.peek_kind_is(1, &TokenKind::LParen)
            && !self.peek_kind_is(1, &TokenKind::Equal)
            && !self.peek_kind_is(1, &TokenKind::Semicolon)
            && !self.peek_kind_is(1, &TokenKind::RBrace);
        is_prefix && self.advance().is_some()
    }

    /// Consumes a `get`/`set` accessor prefix when it precedes a member name.
    fn match_class_accessor_prefix(&mut self) -> Option<ClassMemberKind> {
        let kind = self.peek().and_then(|token| match &token.kind {
            TokenKind::Identifier(name) if name == CLASS_GETTER_KEYWORD => {
                Some(ClassMemberKind::Getter)
            }
            TokenKind::Identifier(name) if name == CLASS_SETTER_KEYWORD => {
                Some(ClassMemberKind::Setter)
            }
            _ => None,
        })?;
        if self.peek_kind_is(1, &TokenKind::LParen)
            || self.peek_kind_is(1, &TokenKind::Equal)
            || self.peek_kind_is(1, &TokenKind::Semicolon)
            || self.peek_kind_is(1, &TokenKind::RBrace)
        {
            return None;
        }
        if self.advance().is_some() {
            return Some(kind);
        }
        None
    }

    fn reject_unsupported_class_member(&self) -> Result<()> {
        if self.check(&TokenKind::Star) {
            return Err(self.parse_error("class generator methods are not supported yet"));
        }
        if self.check(&TokenKind::Async) && !self.peek_kind_is(1, &TokenKind::LParen) {
            return Err(self.parse_error("class async methods are not supported yet"));
        }
        Ok(())
    }

    fn class_member_key_name(key: &ObjectPropertyName) -> Option<StaticName> {
        match key {
            ObjectPropertyName::Static { key, .. } => Some(key.clone()),
            ObjectPropertyName::Computed(_) => None,
        }
    }

    fn class_property_key(key: ObjectPropertyName) -> ObjectPropertyKey {
        match key {
            ObjectPropertyName::Static { key, .. } => ObjectPropertyKey::Static(key),
            ObjectPropertyName::Computed(expr) => ObjectPropertyKey::Computed(Box::new(expr)),
        }
    }
}
