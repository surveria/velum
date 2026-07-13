use crate::ast::{DeclKind, Statement, Stmt};

pub fn for_init_needs_lexical_scope(init: Option<&Statement>) -> bool {
    let Some(init) = init else {
        return false;
    };
    match init.kind() {
        Stmt::VarDecl {
            kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
            ..
        }
        | Stmt::ImportBinding { .. }
        | Stmt::PatternDecl {
            kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
            ..
        } => true,
        Stmt::DeclList(statements) => statements.iter().any(is_lexical_declaration),
        Stmt::VarDecl {
            kind: DeclKind::Var,
            ..
        }
        | Stmt::PatternDecl {
            kind: DeclKind::Var,
            ..
        }
        | Stmt::ClassDecl { .. }
        | Stmt::FunctionDecl { .. }
        | Stmt::Empty
        | Stmt::Debugger
        | Stmt::Block(_)
        | Stmt::If { .. }
        | Stmt::While { .. }
        | Stmt::DoWhile { .. }
        | Stmt::With { .. }
        | Stmt::Label { .. }
        | Stmt::For { .. }
        | Stmt::ForIn { .. }
        | Stmt::ForOf { .. }
        | Stmt::Switch { .. }
        | Stmt::Try { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Throw(_)
        | Stmt::Return(_)
        | Stmt::Expr(_) => false,
    }
}

pub fn for_init_needs_per_iteration_scope(init: Option<&Statement>) -> bool {
    let Some(init) = init else {
        return false;
    };
    match init.kind() {
        Stmt::VarDecl {
            kind: DeclKind::Let,
            ..
        }
        | Stmt::PatternDecl {
            kind: DeclKind::Let,
            ..
        } => true,
        Stmt::DeclList(statements) => statements.iter().any(is_per_iteration_declaration),
        Stmt::VarDecl { .. }
        | Stmt::PatternDecl { .. }
        | Stmt::ImportBinding { .. }
        | Stmt::ClassDecl { .. }
        | Stmt::FunctionDecl { .. }
        | Stmt::Empty
        | Stmt::Debugger
        | Stmt::Block(_)
        | Stmt::If { .. }
        | Stmt::While { .. }
        | Stmt::DoWhile { .. }
        | Stmt::With { .. }
        | Stmt::Label { .. }
        | Stmt::For { .. }
        | Stmt::ForIn { .. }
        | Stmt::ForOf { .. }
        | Stmt::Switch { .. }
        | Stmt::Try { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Throw(_)
        | Stmt::Return(_)
        | Stmt::Expr(_) => false,
    }
}

const fn is_lexical_declaration(statement: &Statement) -> bool {
    matches!(
        statement.kind(),
        Stmt::VarDecl {
            kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
            ..
        } | Stmt::ImportBinding { .. }
            | Stmt::PatternDecl {
                kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
                ..
            }
    )
}

const fn is_per_iteration_declaration(statement: &Statement) -> bool {
    matches!(
        statement.kind(),
        Stmt::VarDecl {
            kind: DeclKind::Let,
            ..
        } | Stmt::PatternDecl {
            kind: DeclKind::Let,
            ..
        }
    )
}
