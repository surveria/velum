use crate::ast::{Expr, ForInTarget, Stmt, SwitchCase};

pub(super) fn statements_contain_nested_function(statements: &[Stmt]) -> bool {
    statements.iter().any(statement_contains_nested_function)
}

fn statement_contains_nested_function(statement: &Stmt) -> bool {
    match statement {
        Stmt::Block(statements) | Stmt::DeclList(statements) => {
            statements_contain_nested_function(statements)
        }
        Stmt::If {
            condition,
            consequent,
            alternate,
        } => {
            expr_contains_nested_function(condition)
                || statement_contains_nested_function(consequent)
                || alternate
                    .as_deref()
                    .is_some_and(statement_contains_nested_function)
        }
        Stmt::While { condition, body } => {
            expr_contains_nested_function(condition) || statement_contains_nested_function(body)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
        } => {
            init.as_deref()
                .is_some_and(statement_contains_nested_function)
                || condition
                    .as_ref()
                    .is_some_and(expr_contains_nested_function)
                || update.as_ref().is_some_and(expr_contains_nested_function)
                || statement_contains_nested_function(body)
        }
        Stmt::ForIn {
            target,
            object,
            body,
        } => {
            for_in_target_contains_nested_function(target)
                || expr_contains_nested_function(object)
                || statement_contains_nested_function(body)
        }
        Stmt::Switch {
            discriminant,
            cases,
        } => {
            expr_contains_nested_function(discriminant)
                || cases.iter().any(switch_case_contains_nested_function)
        }
        Stmt::Try {
            body,
            catch,
            finally_body,
        } => {
            statements_contain_nested_function(body)
                || catch
                    .as_ref()
                    .is_some_and(|catch| statements_contain_nested_function(&catch.body))
                || finally_body
                    .as_deref()
                    .is_some_and(statements_contain_nested_function)
        }
        Stmt::Throw(expr) | Stmt::Return(Some(expr)) | Stmt::Expr(expr) => {
            expr_contains_nested_function(expr)
        }
        Stmt::VarDecl { init, .. } => init.as_ref().is_some_and(expr_contains_nested_function),
        Stmt::Break | Stmt::Continue | Stmt::Return(None) => false,
    }
}

fn for_in_target_contains_nested_function(target: &ForInTarget) -> bool {
    match target {
        ForInTarget::Binding { .. } => false,
        ForInTarget::Assignment(expr) => expr_contains_nested_function(expr),
    }
}

fn switch_case_contains_nested_function(case: &SwitchCase) -> bool {
    case.test
        .as_ref()
        .is_some_and(expr_contains_nested_function)
        || statements_contain_nested_function(&case.statements)
}

fn expr_contains_nested_function(expr: &Expr) -> bool {
    match expr {
        Expr::Function { .. } | Expr::MethodFunction { .. } => true,
        Expr::Literal(_) | Expr::This | Expr::Identifier(_) => false,
        Expr::Parenthesized(expr) | Expr::Unary { expr, .. } | Expr::Update { expr, .. } => {
            expr_contains_nested_function(expr)
        }
        Expr::Binary { left, right, .. } => {
            expr_contains_nested_function(left) || expr_contains_nested_function(right)
        }
        Expr::Conditional {
            condition,
            consequent,
            alternate,
        } => {
            expr_contains_nested_function(condition)
                || expr_contains_nested_function(consequent)
                || expr_contains_nested_function(alternate)
        }
        Expr::Assignment { expr, .. } => expr_contains_nested_function(expr),
        Expr::CompoundAssignment { target, expr, .. } => {
            expr_contains_nested_function(target) || expr_contains_nested_function(expr)
        }
        Expr::PropertyAssignment { object, expr, .. } => {
            expr_contains_nested_function(object) || expr_contains_nested_function(expr)
        }
        Expr::ComputedPropertyAssignment {
            object,
            property,
            expr,
        } => {
            expr_contains_nested_function(object)
                || expr_contains_nested_function(property)
                || expr_contains_nested_function(expr)
        }
        Expr::Member { object, .. } => expr_contains_nested_function(object),
        Expr::ComputedMember { object, property } => {
            expr_contains_nested_function(object) || expr_contains_nested_function(property)
        }
        Expr::Call { callee, args } => {
            expr_contains_nested_function(callee) || args.iter().any(expr_contains_nested_function)
        }
        Expr::New { args, .. } => args.iter().any(expr_contains_nested_function),
        Expr::Object(properties) => properties
            .iter()
            .any(|property| expr_contains_nested_function(&property.value)),
        Expr::Array(elements) => elements.iter().any(expr_contains_nested_function),
    }
}
