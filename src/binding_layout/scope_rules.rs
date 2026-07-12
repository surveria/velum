use crate::ast::{DeclKind, Statement, Stmt};

pub(super) fn for_init_needs_layout_scope(init: Option<&Statement>) -> bool {
    let Some(init) = init else {
        return false;
    };
    match init.kind() {
        Stmt::VarDecl {
            kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
            ..
        }
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
        } | Stmt::PatternDecl {
            kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
            ..
        }
    )
}
