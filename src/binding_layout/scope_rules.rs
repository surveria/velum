use crate::ast::{DeclKind, Stmt};

pub(super) fn for_init_needs_layout_scope(init: Option<&Stmt>) -> bool {
    match init {
        Some(
            Stmt::VarDecl {
                kind: DeclKind::Let | DeclKind::Const,
                ..
            }
            | Stmt::PatternDecl {
                kind: DeclKind::Let | DeclKind::Const,
                ..
            },
        ) => true,
        Some(Stmt::DeclList(statements)) => statements.iter().any(is_lexical_declaration),
        Some(
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
            | Stmt::Expr(_),
        )
        | None => false,
    }
}

const fn is_lexical_declaration(statement: &Stmt) -> bool {
    matches!(
        statement,
        Stmt::VarDecl {
            kind: DeclKind::Let | DeclKind::Const,
            ..
        } | Stmt::PatternDecl {
            kind: DeclKind::Let | DeclKind::Const,
            ..
        }
    )
}
