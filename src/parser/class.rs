use std::rc::Rc;

use crate::{
    ast::{
        ClassConstructor, ClassElementName, ClassLiteral, ClassMember, ClassMemberKind,
        ClassStaticBlock, Expr, Expression, ObjectPropertyKey, Statement, Stmt,
    },
    error::{Error, Result},
    lexer::TokenKind,
    syntax::{FunctionKind, StaticName},
};

use super::{
    Parser,
    class_private::PrivateElementKind,
    literal::{ObjectPropertyName, is_object_property_name_start},
};

/// One parsed class element key: an ordinary property name or a `#name`
/// private identifier declared by this class body.
enum ClassKeySeed {
    Property(ObjectPropertyName),
    Private(StaticName),
}

/// One parsed class member function and its allocated static function id.
struct ParsedClassFunction {
    id: crate::syntax::StaticFunctionId,
    params: Rc<[crate::ast::FunctionParam]>,
    body: Rc<[Statement]>,
    arguments_binding: Option<crate::syntax::StaticBinding>,
}

const CLASS_STATIC_KEYWORD: &str = "static";
const CLASS_GETTER_KEYWORD: &str = "get";
const CLASS_SETTER_KEYWORD: &str = "set";
const CLASS_AUTO_ACCESSOR_KEYWORD: &str = "accessor";
const CLASS_CONSTRUCTOR_NAME: &str = "constructor";
const CLASS_PROTOTYPE_NAME: &str = "prototype";
/// Synthesized rest parameter for default derived constructors.
const DERIVED_CONSTRUCTOR_REST_NAME: &str = "%superargs%";

impl Parser {
    /// Parses a class declaration after its consumed `class` keyword.
    pub(super) fn class_declaration(&mut self) -> Result<Stmt> {
        self.class_declaration_with_decorators(Vec::new())
    }

    pub(super) fn decorated_class_declaration(&mut self) -> Result<Stmt> {
        self.consume(&TokenKind::At, "expected '@' before class decorator")?;
        let decorators = self.decorator_list_after_first_at()?;
        self.consume(&TokenKind::Class, "expected 'class' after decorators")?;
        self.class_declaration_with_decorators(decorators)
    }

    fn class_declaration_with_decorators(&mut self, decorators: Vec<Expression>) -> Result<Stmt> {
        let name = self.consume_class_binding_identifier("expected class declaration name")?;
        let class = self.class_literal_tail(Some(name.name().clone()), decorators)?;
        Ok(Stmt::ClassDecl {
            name,
            class: Box::new(class),
        })
    }

    /// Parses a class expression after its consumed `class` keyword.
    pub(super) fn class_expression(&mut self) -> Result<Expression> {
        self.class_expression_with_decorators(Vec::new())
    }

    pub(super) fn decorated_class_expression_after_at(&mut self) -> Result<Expression> {
        let decorators = self.decorator_list_after_first_at()?;
        self.consume(&TokenKind::Class, "expected 'class' after decorators")?;
        self.class_expression_with_decorators(decorators)
    }

    fn class_expression_with_decorators(
        &mut self,
        decorators: Vec<Expression>,
    ) -> Result<Expression> {
        let start = self.previous_span();
        let name = if self.next_is_identifier() {
            Some(
                self.consume_class_binding_identifier("expected class name")?
                    .name()
                    .clone(),
            )
        } else {
            None
        };
        let class = self.class_literal_tail(name, decorators)?;
        Ok(self.expression_node(start, Expr::Class(Box::new(class))))
    }

    fn decorator_list_after_first_at(&mut self) -> Result<Vec<Expression>> {
        let mut decorators = vec![self.call()?];
        while self.match_kind(&TokenKind::At) {
            decorators.push(self.call()?);
        }
        Ok(decorators)
    }

    fn class_element_decorators(&mut self) -> Result<Vec<Expression>> {
        if !self.match_kind(&TokenKind::At) {
            return Ok(Vec::new());
        }
        self.decorator_list_after_first_at()
    }

    fn consume_class_binding_identifier(
        &mut self,
        message: &str,
    ) -> Result<crate::syntax::StaticBinding> {
        let previous_strict = self.is_strict_mode();
        self.set_strict_mode(true);
        let result = self.consume_binding_identifier(message);
        self.set_strict_mode(previous_strict);
        result
    }

    fn class_literal_tail(
        &mut self,
        name: Option<StaticName>,
        decorators: Vec<Expression>,
    ) -> Result<ClassLiteral> {
        let previous_strict = self.is_strict_mode();
        self.set_strict_mode(true);
        let inner_name_binding = name
            .clone()
            .map(|name| self.static_binding(name))
            .transpose()?;
        let heritage = if self.match_kind(&TokenKind::Extends) {
            Some(self.call()?)
        } else {
            None
        };
        self.push_class_private_scope();
        let result = self
            .class_body_literal(name, inner_name_binding, heritage, decorators)
            .and_then(|class| self.pop_class_private_scope().map(|()| class));
        self.set_strict_mode(previous_strict);
        result
    }

    fn class_body_literal(
        &mut self,
        name: Option<StaticName>,
        inner_name_binding: Option<crate::syntax::StaticBinding>,
        heritage: Option<Expression>,
        decorators: Vec<Expression>,
    ) -> Result<ClassLiteral> {
        self.consume(&TokenKind::LBrace, "expected '{' before class body")?;
        let derived = heritage.is_some();
        let mut constructor = None;
        let mut members = Vec::new();
        let mut fields = Vec::new();
        let mut static_blocks = Vec::new();
        while !self.check(&TokenKind::RBrace) {
            if self.at_end() {
                return Err(self.parse_error("expected '}' after class body"));
            }
            if self.match_kind(&TokenKind::Semicolon) {
                continue;
            }
            self.class_member(
                &mut constructor,
                &mut members,
                &mut fields,
                &mut static_blocks,
                derived,
            )?;
        }
        self.consume(&TokenKind::RBrace, "expected '}' after class body")?;
        let constructor = match constructor {
            Some(constructor) => constructor,
            None if derived => self.default_derived_class_constructor()?,
            None => self.default_class_constructor()?,
        };
        Ok(ClassLiteral {
            decorators,
            name,
            inner_name_binding,
            heritage,
            constructor,
            members,
            fields,
            static_blocks,
        })
    }

    fn default_class_constructor(&mut self) -> Result<ClassConstructor> {
        Ok(ClassConstructor {
            id: self.static_function()?,
            default_derived: false,
            arguments_binding: None,
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
            default_derived: true,
            arguments_binding: None,
            params: vec![crate::ast::FunctionParam::rest(rest)].into(),
            body: vec![Statement::new(Stmt::Expr(forward), span)].into(),
        })
    }

    fn class_member(
        &mut self,
        constructor: &mut Option<ClassConstructor>,
        members: &mut Vec<ClassMember>,
        fields: &mut Vec<crate::ast::ClassField>,
        static_blocks: &mut Vec<ClassStaticBlock>,
        derived: bool,
    ) -> Result<()> {
        let member_offset = self.offset();
        let decorators = self.class_element_decorators()?;
        if self.class_static_block_start() {
            if !decorators.is_empty() {
                return Err(self.parse_error("class static blocks cannot be decorated"));
            }
            return self.class_static_block(member_offset, static_blocks);
        }
        let is_static = self.match_class_static_prefix();
        let function_kind = self.match_class_method_prefix();
        let is_auto_accessor =
            function_kind == FunctionKind::Ordinary && self.match_class_auto_accessor_prefix();
        let accessor = (function_kind == FunctionKind::Ordinary)
            .then(|| self.match_class_accessor_prefix())
            .flatten();
        let key = self.class_element_key()?;
        let key_name = Self::class_member_key_name(&key);

        if !self.check(&TokenKind::LParen) {
            if function_kind != FunctionKind::Ordinary {
                return Err(self.parse_error("expected '(' after class method name"));
            }
            if accessor.is_some() {
                return Err(self.parse_error("expected '(' after class accessor name"));
            }
            return self.class_field(
                key,
                key_name,
                is_static,
                is_auto_accessor,
                decorators,
                member_offset,
                fields,
            );
        }
        if is_auto_accessor {
            return Err(self.parse_error("auto-accessor class elements cannot be methods"));
        }

        if matches!(&key, ClassKeySeed::Property(_))
            && let Some(name) = &key_name
        {
            if !is_static && name.as_str() == CLASS_CONSTRUCTOR_NAME {
                if !decorators.is_empty() {
                    return Err(self.parse_error("class constructors cannot be decorated"));
                }
                return self.class_constructor_member(
                    accessor,
                    function_kind,
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
        if let ClassKeySeed::Private(name) = &key {
            let private_kind = match kind {
                ClassMemberKind::Getter => PrivateElementKind::Getter,
                ClassMemberKind::Setter => PrivateElementKind::Setter,
                ClassMemberKind::Method => PrivateElementKind::Method,
            };
            let name = name.clone();
            self.declare_private_name(&name, private_kind, is_static, member_offset)?;
        }
        let function = self.class_member_function(kind, function_kind, member_offset, false)?;
        members.push(ClassMember {
            source_order: member_offset,
            key: Self::class_element_name(key),
            kind,
            function_kind,
            is_static,
            id: function.id,
            arguments_binding: function.arguments_binding,
            name: key_name,
            params: function.params,
            body: function.body,
            decorators,
        });
        Ok(())
    }

    /// Parses one class element key: a `#name` private identifier or an
    /// ordinary object property name.
    fn class_element_key(&mut self) -> Result<ClassKeySeed> {
        if !matches!(self.peek_kind(0), Some(TokenKind::PrivateName(_))) {
            return Ok(ClassKeySeed::Property(self.object_property_key()?));
        }
        let token = self.advance_token("expected class element name")?;
        let token_span = token.span;
        let TokenKind::PrivateName(text) = token.kind else {
            return Err(Error::parse_at("expected class element name", token_span));
        };
        Ok(ClassKeySeed::Private(self.static_name_shared(text)?))
    }

    fn class_static_block_start(&mut self) -> bool {
        self.peek()
            .is_some_and(|token| token.is_unescaped_identifier_named(CLASS_STATIC_KEYWORD))
            && self.peek_kind_is(1, &TokenKind::LBrace)
    }

    fn class_static_block(
        &mut self,
        source_order: usize,
        static_blocks: &mut Vec<ClassStaticBlock>,
    ) -> Result<()> {
        let static_token = self.advance_token("expected 'static' before class static block")?;
        self.consume(&TokenKind::LBrace, "expected '{' after 'static'")?;
        let mut body = self.with_new_target_scope(|parser| {
            parser.with_restricted_class_arguments(|parser| {
                parser.with_isolated_control_context(|parser| {
                    parser.with_super_context(true, false, |parser| {
                        parser.with_await_context(false, true, |parser| {
                            parser.with_yield_expression(false, Self::block_statements)
                        })
                    })
                })
            })
        })?;
        if body.is_empty() {
            body.push(crate::ast::Statement::new(
                crate::ast::Stmt::Empty,
                static_token.span,
            ));
        }
        if body
            .iter()
            .any(|statement| matches!(statement.kind(), crate::ast::Stmt::Return(_)))
        {
            return Err(Error::parse_at(
                "return is not allowed in a class static block",
                static_token.span,
            ));
        }
        self.validate_static_block_declarations(&body)?;
        static_blocks.push(ClassStaticBlock {
            source_order,
            body: body.into(),
        });
        Ok(())
    }

    /// Parses a field after its key: an optional initializer followed by an
    /// optional semicolon. Private field names are declared into the class
    /// private scope.
    fn class_field(
        &mut self,
        key: ClassKeySeed,
        key_name: Option<StaticName>,
        is_static: bool,
        is_auto_accessor: bool,
        decorators: Vec<Expression>,
        member_offset: usize,
        fields: &mut Vec<crate::ast::ClassField>,
    ) -> Result<()> {
        if let ClassKeySeed::Private(name) = &key {
            let name = name.clone();
            self.declare_private_name(&name, PrivateElementKind::Field, is_static, member_offset)?;
        } else if let Some(name) = &key_name {
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
            Some(self.with_new_target_scope(|parser| {
                parser.with_restricted_class_arguments(|parser| {
                    parser.with_super_context(true, false, Self::assignment_expression)
                })
            })?)
        } else {
            None
        };
        self.consume_statement_terminator("expected statement terminator after class field")?;
        let key = Self::class_element_name(key);
        let auto_accessor = if is_auto_accessor && matches!(key, ClassElementName::Property(_)) {
            Some(self.build_public_auto_accessor(is_static, member_offset)?)
        } else {
            None
        };
        fields.push(crate::ast::ClassField {
            source_order: member_offset,
            key,
            is_static,
            auto_accessor,
            name: key_name,
            initializer,
            decorators,
        });
        Ok(())
    }

    fn class_constructor_member(
        &mut self,
        accessor: Option<ClassMemberKind>,
        function_kind: FunctionKind,
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
        if function_kind != FunctionKind::Ordinary {
            return Err(Error::parse(
                "class constructor must be an ordinary method",
                member_offset,
            ));
        }
        if constructor.is_some() {
            return Err(Error::parse(
                "class body cannot declare two constructors",
                member_offset,
            ));
        }
        let function = self.class_member_function(
            ClassMemberKind::Method,
            FunctionKind::Ordinary,
            member_offset,
            derived,
        )?;
        *constructor = Some(ClassConstructor {
            id: function.id,
            default_derived: false,
            arguments_binding: function.arguments_binding,
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
        function_kind: FunctionKind,
        member_offset: usize,
        allow_super_call: bool,
    ) -> Result<ParsedClassFunction> {
        self.consume(&TokenKind::LParen, "expected '(' after class member name")?;
        let ((parameters, body), uses_arguments) =
            self.with_function_arguments_context(|parser| {
                parser.with_new_target_scope(|parser| {
                    parser.with_super_context(true, allow_super_call, |parser| {
                        let parameters = parser.with_await_context(
                            false,
                            function_kind.is_async(),
                            |parser| {
                                parser.with_yield_expression(false, |parser| {
                                    parser.with_yield_identifier_reserved(
                                        function_kind.is_generator(),
                                        Self::function_parameters,
                                    )
                                })
                            },
                        )?;
                        parser.reject_duplicate_parameters(&parameters.bound_names)?;
                        parser.consume(
                            &TokenKind::RParen,
                            "expected ')' after class member parameters",
                        )?;
                        Self::validate_class_member_parameters(kind, &parameters, member_offset)?;
                        parser
                            .consume(&TokenKind::LBrace, "expected '{' before class member body")?;
                        let body = parser.with_await_context(
                            function_kind.is_async(),
                            function_kind.is_async(),
                            |parser| {
                                parser
                                    .with_yield_expression(function_kind.is_generator(), |parser| {
                                        parser.function_body(true)
                                    })
                            },
                        )?;
                        Ok((parameters, body))
                    })
                })
            })?;
        self.validate_function_parameters(
            &parameters.bound_names,
            parameters.is_simple,
            true,
            body.contains_use_strict,
        )?;
        self.validate_function_parameter_lexicals(&parameters.params, &body.statements)?;
        let id = self.static_function()?;
        let arguments_binding = if uses_arguments {
            Some(self.implicit_arguments_binding()?)
        } else {
            None
        };
        let params = parameters.into_params();
        let statements = body.statements;
        Ok(ParsedClassFunction {
            id,
            params: params.into(),
            body: statements.into(),
            arguments_binding,
        })
    }

    fn validate_class_member_parameters(
        kind: ClassMemberKind,
        parameters: &crate::parser::function::ParsedParameters,
        member_offset: usize,
    ) -> Result<()> {
        match kind {
            ClassMemberKind::Getter if !parameters.params.is_empty() => Err(Error::parse(
                "getter must not declare parameters",
                member_offset,
            )),
            ClassMemberKind::Setter if parameters.params.len() != 1 => Err(Error::parse(
                "setter must declare exactly one parameter",
                member_offset,
            )),
            ClassMemberKind::Setter
                if parameters.params.first().is_some_and(|param| param.rest) =>
            {
                Err(Error::parse(
                    "setter parameter cannot be a rest parameter",
                    member_offset,
                ))
            }
            ClassMemberKind::Method | ClassMemberKind::Getter | ClassMemberKind::Setter => Ok(()),
        }
    }

    /// Consumes a `static` member prefix when it is not itself a member name.
    fn match_class_static_prefix(&mut self) -> bool {
        let is_prefix = self
            .peek()
            .is_some_and(|token| token.is_unescaped_identifier_named(CLASS_STATIC_KEYWORD))
            && !self.peek_kind_is(1, &TokenKind::LParen)
            && !self.peek_kind_is(1, &TokenKind::Equal)
            && !self.peek_kind_is(1, &TokenKind::Semicolon)
            && !self.peek_kind_is(1, &TokenKind::RBrace);
        is_prefix && self.advance().is_some()
    }

    /// Consumes a `get`/`set` accessor prefix when it precedes a member name.
    fn match_class_accessor_prefix(&mut self) -> Option<ClassMemberKind> {
        let kind = self.peek().and_then(|token| {
            if token.is_unescaped_identifier_named(CLASS_GETTER_KEYWORD) {
                Some(ClassMemberKind::Getter)
            } else if token.is_unescaped_identifier_named(CLASS_SETTER_KEYWORD) {
                Some(ClassMemberKind::Setter)
            } else {
                None
            }
        })?;
        if self.peek_kind_is(1, &TokenKind::LParen)
            || self.peek_kind_is(1, &TokenKind::Equal)
            || self.peek_kind_is(1, &TokenKind::Semicolon)
            || self.peek_kind_is(1, &TokenKind::RBrace)
            || (self.peek_kind_is(1, &TokenKind::Star) && self.peek_has_line_terminator_before(1))
        {
            return None;
        }
        if self.advance().is_some() {
            return Some(kind);
        }
        None
    }

    /// Consumes the decorators auto-accessor prefix while lowering the
    /// element through the existing field representation.
    fn match_class_auto_accessor_prefix(&mut self) -> bool {
        let is_prefix = self
            .peek()
            .is_some_and(|token| token.is_unescaped_identifier_named(CLASS_AUTO_ACCESSOR_KEYWORD))
            && !self.peek_has_line_terminator_before(1)
            && !self.peek_kind_is(1, &TokenKind::LParen)
            && !self.peek_kind_is(1, &TokenKind::Equal)
            && !self.peek_kind_is(1, &TokenKind::Semicolon)
            && !self.peek_kind_is(1, &TokenKind::RBrace);
        is_prefix && self.advance().is_some()
    }

    fn match_class_method_prefix(&mut self) -> FunctionKind {
        if self.match_kind(&TokenKind::Star) {
            return FunctionKind::Generator;
        }
        if !self.class_async_method_start() || !self.match_kind(&TokenKind::Async) {
            return FunctionKind::Ordinary;
        }
        if self.match_kind(&TokenKind::Star) {
            FunctionKind::AsyncGenerator
        } else {
            FunctionKind::Async
        }
    }

    fn class_async_method_start(&mut self) -> bool {
        if !self.peek_kind_is(0, &TokenKind::Async) || self.peek_has_line_terminator_before(1) {
            return false;
        }
        if self.peek_kind_is(1, &TokenKind::Star) {
            return self.peek_kind(2).is_some_and(class_element_name_start);
        }
        self.peek_kind(1).is_some_and(class_element_name_start)
    }

    fn class_member_key_name(key: &ClassKeySeed) -> Option<StaticName> {
        match key {
            ClassKeySeed::Property(ObjectPropertyName::Static { key, .. })
            | ClassKeySeed::Private(key) => Some(key.clone()),
            ClassKeySeed::Property(ObjectPropertyName::Computed(_)) => None,
        }
    }

    fn class_element_name(key: ClassKeySeed) -> ClassElementName {
        match key {
            ClassKeySeed::Property(property) => {
                ClassElementName::Property(Self::class_property_key(property))
            }
            ClassKeySeed::Private(name) => ClassElementName::Private(name),
        }
    }

    fn class_property_key(key: ObjectPropertyName) -> ObjectPropertyKey {
        match key {
            ObjectPropertyName::Static { key, .. } => ObjectPropertyKey::Static(key),
            ObjectPropertyName::Computed(expr) => ObjectPropertyKey::Computed(Box::new(expr)),
        }
    }
}

const fn class_element_name_start(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::PrivateName(_)) || is_object_property_name_start(kind)
}
