use crate::ast::{DeclKind, ForInTarget, Stmt};

pub fn for_can_compile(init: Option<&Stmt>, body: &Stmt) -> bool {
    !for_init_needs_lexical_scope(init) && statement_can_inline_in_loop(body)
}

pub fn block_can_inline(statements: &[Stmt]) -> bool {
    statements.iter().all(statement_can_inline_in_block)
}

fn statement_can_inline_in_loop(statement: &Stmt) -> bool {
    statement_can_inline_in_block(statement)
}

fn statement_can_inline_in_block(statement: &Stmt) -> bool {
    !statement_has_lexical_declaration(statement) && !statement_has_abrupt_completion(statement)
}

fn statement_has_lexical_declaration(statement: &Stmt) -> bool {
    match statement {
        Stmt::Block(statements) | Stmt::DeclList(statements) => {
            statements.iter().any(statement_has_lexical_declaration)
        }
        Stmt::If {
            consequent,
            alternate,
            ..
        } => {
            statement_has_lexical_declaration(consequent)
                || alternate
                    .as_deref()
                    .is_some_and(statement_has_lexical_declaration)
        }
        Stmt::While { body, .. } | Stmt::For { body, .. } => {
            statement_has_lexical_declaration(body)
        }
        Stmt::ForIn { target, body, .. } => {
            matches!(
                target,
                ForInTarget::Binding {
                    kind: DeclKind::Let | DeclKind::Const,
                    ..
                }
            ) || statement_has_lexical_declaration(body)
        }
        Stmt::Switch { cases, .. } => cases.iter().any(|case| {
            case.statements
                .iter()
                .any(statement_has_lexical_declaration)
        }),
        Stmt::Try {
            body,
            catch,
            finally_body,
        } => {
            body.iter().any(statement_has_lexical_declaration)
                || catch
                    .as_ref()
                    .is_some_and(|catch| catch.body.iter().any(statement_has_lexical_declaration))
                || finally_body
                    .as_ref()
                    .is_some_and(|body| body.iter().any(statement_has_lexical_declaration))
        }
        Stmt::VarDecl {
            kind: DeclKind::Let | DeclKind::Const,
            ..
        } => true,
        Stmt::Break
        | Stmt::Continue
        | Stmt::Throw(_)
        | Stmt::Return(_)
        | Stmt::VarDecl {
            kind: DeclKind::Var,
            ..
        }
        | Stmt::Expr(_) => false,
    }
}

fn statement_has_abrupt_completion(statement: &Stmt) -> bool {
    match statement {
        Stmt::Block(statements) | Stmt::DeclList(statements) => {
            statements.iter().any(statement_has_abrupt_completion)
        }
        Stmt::If {
            consequent,
            alternate,
            ..
        } => {
            statement_has_abrupt_completion(consequent)
                || alternate
                    .as_deref()
                    .is_some_and(statement_has_abrupt_completion)
        }
        Stmt::While { .. }
        | Stmt::For { .. }
        | Stmt::ForIn { .. }
        | Stmt::Switch { .. }
        | Stmt::VarDecl { .. }
        | Stmt::Expr(_) => false,
        Stmt::Try {
            body,
            catch,
            finally_body,
        } => {
            body.iter().any(statement_has_abrupt_completion)
                || catch
                    .as_ref()
                    .is_some_and(|catch| catch.body.iter().any(statement_has_abrupt_completion))
                || finally_body
                    .as_ref()
                    .is_some_and(|body| body.iter().any(statement_has_abrupt_completion))
        }
        Stmt::Break | Stmt::Continue | Stmt::Throw(_) | Stmt::Return(_) => true,
    }
}

fn for_init_needs_lexical_scope(init: Option<&Stmt>) -> bool {
    match init {
        Some(Stmt::VarDecl {
            kind: DeclKind::Let | DeclKind::Const,
            ..
        }) => true,
        Some(Stmt::DeclList(statements)) => statements.iter().any(|statement| {
            matches!(
                statement,
                Stmt::VarDecl {
                    kind: DeclKind::Let | DeclKind::Const,
                    ..
                }
            )
        }),
        Some(_) | None => false,
    }
}
