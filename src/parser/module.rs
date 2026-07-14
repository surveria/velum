use std::collections::BTreeSet;

use crate::{
    ast::{DeclKind, Expr, Statement, Stmt},
    error::{Error, Result},
    lexer::TokenKind,
    syntax::ImportPhase,
};

use super::{Parser, property_name::keyword_property_name};

const AS_KEYWORD: &str = "as";
const DEFER_KEYWORD: &str = "defer";
const FROM_KEYWORD: &str = "from";

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ModuleSyntax {
    pub(crate) requests: Vec<ModuleRequestEntry>,
    pub(crate) imports: Vec<ModuleImportEntry>,
    pub(crate) exports: Vec<ModuleExportEntry>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ModuleImportEntry {
    pub(crate) request: ModuleRequestEntry,
    pub(crate) import_name: ModuleImportName,
    pub(crate) local_name: String,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct ModuleRequestEntry {
    pub(crate) specifier: String,
    pub(crate) phase: ImportPhase,
    pub(crate) attributes: Box<[(String, String)]>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ModuleImportName {
    Name(String),
    Namespace,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ModuleExportEntry {
    Local {
        export_name: String,
        local_name: String,
    },
    Indirect {
        export_name: String,
        import_name: String,
        request: String,
    },
    Namespace {
        export_name: String,
        request: String,
    },
    Star {
        request: String,
    },
}

impl Parser {
    pub(super) fn parse_module_declaration(
        &mut self,
        module: &mut ModuleSyntax,
        statements: &mut Vec<Statement>,
    ) -> Result<bool> {
        if self.check(&TokenKind::Import)
            && !matches!(self.peek_kind(1), Some(TokenKind::LParen | TokenKind::Dot))
        {
            self.consume(&TokenKind::Import, "expected 'import'")?;
            self.module_import_declaration(module, statements)?;
            return Ok(true);
        }
        if self.match_kind(&TokenKind::Export) {
            self.module_export_declaration(module, statements)?;
            return Ok(true);
        }
        Ok(false)
    }

    fn module_import_declaration(
        &mut self,
        module: &mut ModuleSyntax,
        statements: &mut Vec<Statement>,
    ) -> Result<()> {
        if matches!(self.peek_kind(0), Some(TokenKind::String(_))) {
            let specifier = self.module_specifier()?;
            let request =
                self.module_request_after_specifier(specifier, ImportPhase::Evaluation)?;
            Self::remember_module_request(module, request);
            self.consume_statement_terminator("expected terminator after import declaration")?;
            return Ok(());
        }

        let phase = if self
            .peek()
            .is_some_and(|token| token.is_unescaped_identifier_named(DEFER_KEYWORD))
            && matches!(self.peek_kind(1), Some(TokenKind::Star))
        {
            self.advance();
            ImportPhase::Defer
        } else {
            ImportPhase::Evaluation
        };
        let mut pending = Vec::new();
        if !self.check(&TokenKind::Star) && !self.check(&TokenKind::LBrace) {
            let binding = self.consume_binding_identifier("expected default import binding")?;
            pending.push(("default".to_owned(), binding));
            if self.match_kind(&TokenKind::Comma) {
                self.module_import_tail(&mut pending)?;
            }
        } else {
            self.module_import_tail(&mut pending)?;
        }
        self.consume_contextual_module_word(FROM_KEYWORD)?;
        let specifier = self.module_specifier()?;
        let request = self.module_request_after_specifier(specifier, phase)?;
        Self::remember_module_request(module, request.clone());
        for (import_name, binding) in pending {
            let local_name = binding.name().as_str().to_owned();
            module.imports.push(ModuleImportEntry {
                request: request.clone(),
                import_name: if import_name == "*" {
                    ModuleImportName::Namespace
                } else {
                    ModuleImportName::Name(import_name)
                },
                local_name,
            });
            statements.push(
                self.statement_node(self.previous_span(), Stmt::ImportBinding { name: binding }),
            );
        }
        self.consume_statement_terminator("expected terminator after import declaration")
    }

    fn module_import_tail(
        &mut self,
        pending: &mut Vec<(String, crate::syntax::StaticBinding)>,
    ) -> Result<()> {
        if self.match_kind(&TokenKind::Star) {
            self.consume_contextual_module_word(AS_KEYWORD)?;
            let binding = self.consume_binding_identifier("expected namespace import binding")?;
            pending.push(("*".to_owned(), binding));
            return Ok(());
        }
        self.consume(&TokenKind::LBrace, "expected '{' before named imports")?;
        while !self.check(&TokenKind::RBrace) {
            let import_name = self.module_identifier_name(true)?;
            let local_name = if self.match_contextual_module_word(AS_KEYWORD) {
                self.consume_binding_identifier("expected local import binding")?
            } else {
                let name = self.static_name(import_name.clone())?;
                if self.is_strict_mode() {
                    self.validate_function_name_in_strict_code(&name)?;
                }
                self.static_binding(name)?
            };
            pending.push((import_name, local_name));
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }
        self.consume(&TokenKind::RBrace, "expected '}' after named imports")
    }

    fn module_export_declaration(
        &mut self,
        module: &mut ModuleSyntax,
        statements: &mut Vec<Statement>,
    ) -> Result<()> {
        if self.match_kind(&TokenKind::Star) {
            if self.match_contextual_module_word(AS_KEYWORD) {
                let export_name = self.module_identifier_name(true)?;
                self.consume_contextual_module_word(FROM_KEYWORD)?;
                let request = self.module_specifier()?;
                let module_request =
                    self.module_request_after_specifier(request.clone(), ImportPhase::Evaluation)?;
                Self::remember_module_request(module, module_request);
                module.exports.push(ModuleExportEntry::Namespace {
                    export_name,
                    request,
                });
            } else {
                self.consume_contextual_module_word(FROM_KEYWORD)?;
                let request = self.module_specifier()?;
                let module_request =
                    self.module_request_after_specifier(request.clone(), ImportPhase::Evaluation)?;
                Self::remember_module_request(module, module_request);
                module.exports.push(ModuleExportEntry::Star { request });
            }
            return self
                .consume_statement_terminator("expected terminator after export declaration");
        }
        if self.match_kind(&TokenKind::LBrace) {
            return self.module_named_exports(module);
        }
        self.validate_default_export_keyword()?;
        if self.match_kind(&TokenKind::Default) {
            return self.module_default_export(module, statements);
        }

        let start = self.current_span();
        let statement = self.module_exported_statement()?;
        let mut names = Vec::new();
        Self::module_statement_bound_names(&statement, &mut names)?;
        if names.is_empty() {
            return Err(self.parse_error("export declaration does not declare a binding"));
        }
        for name in names {
            module.exports.push(ModuleExportEntry::Local {
                export_name: name.clone(),
                local_name: name,
            });
        }
        statements.push(self.statement_node(start, statement));
        Ok(())
    }

    fn module_default_export(
        &mut self,
        module: &mut ModuleSyntax,
        statements: &mut Vec<Statement>,
    ) -> Result<()> {
        let start = self.current_span();
        let async_function = self.check(&TokenKind::Async)
            && self.peek_kind_is_no_line_terminator(1, &TokenKind::Function);
        let declaration_like =
            self.check(&TokenKind::Function) || self.check(&TokenKind::Class) || async_function;
        let expression = self.assignment_expression()?;
        if declaration_like && !matches!(expression.kind(), Expr::Function { .. } | Expr::Class(_))
        {
            return Err(self.parse_error("default function or class export must be a declaration"));
        }
        let expression_span = expression.span();
        let (local_name, default_expression) = match expression.into_kind() {
            Expr::Function {
                id,
                name: Some(name),
                arguments_binding,
                params,
                body,
                kind,
                strict,
            } if declaration_like => {
                let local_name = name.name().as_str().to_owned();
                statements.push(self.statement_node(
                    start,
                    Stmt::FunctionDecl {
                        name,
                        block_scoped: false,
                        annex_b_var_binding: None,
                        arguments_binding,
                        id,
                        params,
                        body,
                        kind,
                        strict,
                    },
                ));
                (local_name, None)
            }
            Expr::Class(class) if declaration_like && class.name.is_some() => {
                let name = class
                    .name
                    .clone()
                    .ok_or_else(|| self.parse_error("default class name disappeared"))?;
                let local_name = name.as_str().to_owned();
                let binding = self.static_binding(name)?;
                statements.push(self.statement_node(
                    start,
                    Stmt::ClassDecl {
                        name: binding,
                        class,
                    },
                ));
                (local_name, None)
            }
            kind => (
                "default".to_owned(),
                Some(crate::ast::Expression::new(kind, expression_span)),
            ),
        };
        if let Some(expression) = default_expression {
            let binding = self.static_binding_name(local_name.clone())?;
            statements.push(self.statement_node(
                start,
                Stmt::VarDecl {
                    name: binding,
                    kind: DeclKind::Const,
                    init: Some(expression),
                },
            ));
        }
        module.exports.push(ModuleExportEntry::Local {
            export_name: "default".to_owned(),
            local_name,
        });
        if declaration_like {
            self.consume_optional_semicolon();
            return Ok(());
        }
        self.consume_statement_terminator("expected terminator after default export expression")
    }

    fn validate_default_export_keyword(&mut self) -> Result<()> {
        if self
            .peek()
            .is_some_and(|token| token.kind == TokenKind::Default && token.identifier_escaped)
        {
            return Err(self.parse_error("escaped 'default' is not an export keyword"));
        }
        Ok(())
    }

    fn module_named_exports(&mut self, module: &mut ModuleSyntax) -> Result<()> {
        let mut names = Vec::new();
        while !self.check(&TokenKind::RBrace) {
            let local_is_string = matches!(self.peek_kind(0), Some(TokenKind::String(_)));
            let local_name = self.module_identifier_name(true)?;
            let export_name = if self.match_contextual_module_word(AS_KEYWORD) {
                self.module_identifier_name(true)?
            } else {
                local_name.clone()
            };
            names.push((local_name, export_name, local_is_string));
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }
        self.consume(&TokenKind::RBrace, "expected '}' after named exports")?;
        if self.match_contextual_module_word(FROM_KEYWORD) {
            let request = self.module_specifier()?;
            let module_request =
                self.module_request_after_specifier(request.clone(), ImportPhase::Evaluation)?;
            Self::remember_module_request(module, module_request);
            for (import_name, export_name, _) in names {
                module.exports.push(ModuleExportEntry::Indirect {
                    export_name,
                    import_name,
                    request: request.clone(),
                });
            }
        } else {
            for (local_name, export_name, local_is_string) in names {
                if local_is_string {
                    return Err(
                        self.parse_error("string export names require an explicit module source")
                    );
                }
                module.exports.push(ModuleExportEntry::Local {
                    export_name,
                    local_name,
                });
            }
        }
        self.consume_statement_terminator("expected terminator after export declaration")
    }

    fn module_exported_statement(&mut self) -> Result<Stmt> {
        if self.match_kind(&TokenKind::Var) {
            return self.var_decl(DeclKind::Var);
        }
        if self.match_kind(&TokenKind::Let) {
            return self.var_decl(DeclKind::Let);
        }
        if self.match_kind(&TokenKind::Const) {
            return self.var_decl(DeclKind::Const);
        }
        if self.match_kind(&TokenKind::Class) {
            return self.class_declaration();
        }
        if self.check(&TokenKind::Async)
            && self.peek_kind_is_no_line_terminator(1, &TokenKind::Function)
        {
            self.consume(&TokenKind::Async, "expected 'async'")?;
            self.consume(&TokenKind::Function, "expected 'function'")?;
            let kind = if self.match_kind(&TokenKind::Star) {
                crate::syntax::FunctionKind::AsyncGenerator
            } else {
                crate::syntax::FunctionKind::Async
            };
            return self.function_declaration(kind);
        }
        if self.match_kind(&TokenKind::Function) {
            let kind = if self.match_kind(&TokenKind::Star) {
                crate::syntax::FunctionKind::Generator
            } else {
                crate::syntax::FunctionKind::Ordinary
            };
            return self.function_declaration(kind);
        }
        Err(self.parse_error("expected declaration after 'export'"))
    }

    fn module_statement_bound_names(statement: &Stmt, names: &mut Vec<String>) -> Result<()> {
        match statement {
            Stmt::DeclList(declarations) => {
                for declaration in declarations {
                    Self::module_statement_bound_names(declaration.kind(), names)?;
                }
            }
            Stmt::VarDecl { name, .. }
            | Stmt::ImportBinding { name }
            | Stmt::FunctionDecl { name, .. }
            | Stmt::ClassDecl { name, .. } => names.push(name.name().as_str().to_owned()),
            Stmt::PatternDecl { pattern, .. } => Self::collect_pattern_names(pattern, names)?,
            _ => {}
        }
        Ok(())
    }

    fn module_specifier(&mut self) -> Result<String> {
        let token = self.advance_token("expected module specifier")?;
        let span = token.span;
        let TokenKind::String(value) = token.kind else {
            return Err(Error::parse_at("expected module specifier string", span));
        };
        String::from_utf16(&value.cooked)
            .map_err(|_| Error::parse_at("module specifier contains a lone surrogate", span))
    }

    fn module_request_after_specifier(
        &mut self,
        specifier: String,
        phase: ImportPhase,
    ) -> Result<ModuleRequestEntry> {
        let attributes = if self.match_kind(&TokenKind::With) {
            self.module_import_attributes()?
        } else {
            Box::new([])
        };
        Ok(ModuleRequestEntry {
            specifier,
            phase,
            attributes,
        })
    }

    fn module_import_attributes(&mut self) -> Result<Box<[(String, String)]>> {
        self.consume(&TokenKind::LBrace, "expected '{' after import attributes")?;
        let mut attributes = Vec::new();
        let mut names = BTreeSet::new();
        while !self.check(&TokenKind::RBrace) {
            let name = self.module_identifier_name(true)?;
            if !names.insert(name.clone()) {
                return Err(self.parse_error("duplicate import attribute"));
            }
            self.consume(
                &TokenKind::Colon,
                "expected ':' after import attribute name",
            )?;
            let token = self.advance_token("expected import attribute value")?;
            let span = token.span;
            let TokenKind::String(value) = token.kind else {
                return Err(Error::parse_at(
                    "import attribute value must be a string literal",
                    span,
                ));
            };
            let value = String::from_utf16(&value.cooked).map_err(|_| {
                Error::parse_at("import attribute value contains a lone surrogate", span)
            })?;
            attributes.push((name, value));
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }
        self.consume(&TokenKind::RBrace, "expected '}' after import attributes")?;
        attributes.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(attributes.into_boxed_slice())
    }

    fn module_identifier_name(&mut self, allow_string: bool) -> Result<String> {
        let token = self.advance_token("expected module export name")?;
        let span = token.span;
        match token.kind {
            TokenKind::Identifier(name) => Ok(name.as_ref().to_owned()),
            TokenKind::String(value) if allow_string => String::from_utf16(&value.cooked)
                .map_err(|_| Error::parse_at("module name contains a lone surrogate", span)),
            kind => keyword_property_name(&kind)
                .map(str::to_owned)
                .ok_or_else(|| Error::parse_at("expected module export name", span)),
        }
    }

    fn consume_contextual_module_word(&mut self, expected: &str) -> Result<()> {
        if self.match_contextual_module_word(expected) {
            return Ok(());
        }
        Err(self.parse_error(format!("expected '{expected}'")))
    }

    fn match_contextual_module_word(&mut self, expected: &str) -> bool {
        if !self
            .peek()
            .is_some_and(|token| token.is_unescaped_identifier_named(expected))
        {
            return false;
        }
        self.advance().is_some()
    }

    fn remember_module_request(module: &mut ModuleSyntax, request: ModuleRequestEntry) {
        if !module.requests.iter().any(|known| known == &request) {
            module.requests.push(request);
        }
    }

    pub(super) fn validate_module_syntax(
        module: &ModuleSyntax,
        statements: &[Statement],
    ) -> Result<()> {
        Self::validate_module_statement_list(statements)?;
        let mut exported = BTreeSet::new();
        let mut declared = BTreeSet::new();
        for statement in statements {
            let mut names = Vec::new();
            Self::module_statement_bound_names(statement.kind(), &mut names)?;
            declared.extend(names);
        }
        for entry in &module.exports {
            let name = match entry {
                ModuleExportEntry::Local { export_name, .. }
                | ModuleExportEntry::Indirect { export_name, .. }
                | ModuleExportEntry::Namespace { export_name, .. } => Some(export_name),
                ModuleExportEntry::Star { .. } => None,
            };
            if let Some(name) = name
                && !exported.insert(name)
            {
                return Err(Error::parse("duplicate module export", 0));
            }
            if let ModuleExportEntry::Local { local_name, .. } = entry
                && !declared.contains(local_name)
            {
                return Err(Error::parse(
                    format!("module export '{local_name}' is not declared"),
                    0,
                ));
            }
        }
        Ok(())
    }

    fn validate_module_statement_list(statements: &[Statement]) -> Result<()> {
        for statement in statements {
            match statement.kind() {
                Stmt::Return(_) => {
                    return Err(Error::parse_at(
                        "return statement is not allowed in module code",
                        statement.span(),
                    ));
                }
                Stmt::Block(statements) | Stmt::DeclList(statements) => {
                    Self::validate_module_statement_list(statements)?;
                }
                Stmt::If {
                    consequent,
                    alternate,
                    ..
                } => {
                    Self::validate_module_statement_list(std::slice::from_ref(consequent))?;
                    if let Some(alternate) = alternate {
                        Self::validate_module_statement_list(std::slice::from_ref(alternate))?;
                    }
                }
                Stmt::While { body, .. }
                | Stmt::DoWhile { body, .. }
                | Stmt::With { body, .. }
                | Stmt::Label { body, .. }
                | Stmt::ForIn { body, .. }
                | Stmt::ForOf { body, .. } => {
                    Self::validate_module_statement_list(std::slice::from_ref(body))?;
                }
                Stmt::For { init, body, .. } => {
                    if let Some(init) = init {
                        Self::validate_module_statement_list(std::slice::from_ref(init))?;
                    }
                    Self::validate_module_statement_list(std::slice::from_ref(body))?;
                }
                Stmt::Switch { cases, .. } => {
                    for case in cases {
                        Self::validate_module_statement_list(&case.statements)?;
                    }
                }
                Stmt::Try {
                    body,
                    catch,
                    finally_body,
                } => {
                    Self::validate_module_statement_list(body)?;
                    if let Some(catch) = catch {
                        Self::validate_module_statement_list(&catch.body)?;
                    }
                    if let Some(finally_body) = finally_body {
                        Self::validate_module_statement_list(finally_body)?;
                    }
                }
                Stmt::Empty
                | Stmt::Debugger
                | Stmt::Break(_)
                | Stmt::Continue(_)
                | Stmt::Throw(_)
                | Stmt::FunctionDecl { .. }
                | Stmt::ImportBinding { .. }
                | Stmt::VarDecl { .. }
                | Stmt::PatternDecl { .. }
                | Stmt::ClassDecl { .. }
                | Stmt::Expr(_) => {}
            }
        }
        Ok(())
    }
}
