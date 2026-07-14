use crate::ast::{Expr, Expression, ObjectPropertyKey};

const DIRECT_EVAL_BINDING: &str = "eval";

pub(super) fn expression_contains_direct_eval(expression: &Expression) -> bool {
    match expression.kind() {
        Expr::Literal(_)
        | Expr::StringLiteral { .. }
        | Expr::TemplateObject { .. }
        | Expr::RegExpLiteral { .. }
        | Expr::This
        | Expr::ImportMeta
        | Expr::NewTarget
        | Expr::Identifier(_)
        | Expr::SuperMember { .. }
        | Expr::ArrayHole
        | Expr::Function { .. }
        | Expr::ArrowFunction { .. }
        | Expr::MethodFunction { .. } => false,
        Expr::Class(_) | Expr::DestructuringAssignment { .. } => true,
        Expr::Spread(expression)
        | Expr::Parenthesized(expression)
        | Expr::OptionalChain(expression)
        | Expr::Unary {
            expr: expression, ..
        }
        | Expr::Await(expression)
        | Expr::Update {
            expr: expression, ..
        } => expression_contains_direct_eval(expression),
        Expr::SuperCall { args } | Expr::Array(args) => expressions_contain_direct_eval(args),
        Expr::SuperComputedMember { property, .. } => expression_contains_direct_eval(property),
        Expr::TemplateLiteral { expressions, .. } | Expr::Sequence(expressions) => {
            expressions_contain_direct_eval(expressions)
        }
        Expr::Yield { expr, .. } => expr.as_deref().is_some_and(expression_contains_direct_eval),
        Expr::Binary { left, right, .. } => {
            expression_contains_direct_eval(left) || expression_contains_direct_eval(right)
        }
        Expr::Conditional {
            condition,
            consequent,
            alternate,
        } => {
            expression_contains_direct_eval(condition)
                || expression_contains_direct_eval(consequent)
                || expression_contains_direct_eval(alternate)
        }
        assignment @ (Expr::Assignment { .. }
        | Expr::CompoundAssignment { .. }
        | Expr::WebCompatCallAssignment { .. }
        | Expr::PropertyAssignment { .. }
        | Expr::ComputedPropertyAssignment { .. }
        | Expr::SuperPropertyAssignment { .. }
        | Expr::SuperComputedPropertyAssignment { .. }
        | Expr::PrivateAssignment { .. }) => assignment_contains_direct_eval(assignment),
        Expr::Member { object, .. }
        | Expr::OptionalMember { object, .. }
        | Expr::PrivateMember { object, .. }
        | Expr::OptionalPrivateMember { object, .. }
        | Expr::PrivateIn { object, .. } => expression_contains_direct_eval(object),
        Expr::ComputedMember {
            object, property, ..
        }
        | Expr::OptionalComputedMember {
            object, property, ..
        } => expression_contains_direct_eval(object) || expression_contains_direct_eval(property),
        Expr::Call { callee, args, .. } => {
            direct_eval_callee(callee)
                || expression_contains_direct_eval(callee)
                || expressions_contain_direct_eval(args)
        }
        Expr::OptionalCall { callee, args, .. }
        | Expr::New {
            constructor: callee,
            args,
        } => expression_contains_direct_eval(callee) || expressions_contain_direct_eval(args),
        Expr::DynamicImport {
            specifier, options, ..
        } => {
            expression_contains_direct_eval(specifier)
                || options
                    .as_deref()
                    .is_some_and(expression_contains_direct_eval)
        }
        Expr::Object(properties) => properties.iter().any(|property| {
            let computed_key_has_eval = match &property.key {
                ObjectPropertyKey::Static(_) => false,
                ObjectPropertyKey::Computed(key) => expression_contains_direct_eval(key),
            };
            computed_key_has_eval || expression_contains_direct_eval(&property.value)
        }),
    }
}

fn expressions_contain_direct_eval(expressions: &[Expression]) -> bool {
    expressions.iter().any(expression_contains_direct_eval)
}

fn assignment_contains_direct_eval(assignment: &Expr) -> bool {
    match assignment {
        Expr::Assignment { expr, .. } | Expr::SuperPropertyAssignment { expr, .. } => {
            expression_contains_direct_eval(expr)
        }
        Expr::CompoundAssignment { target, expr, .. }
        | Expr::PropertyAssignment {
            object: target,
            expr,
            ..
        }
        | Expr::PrivateAssignment {
            object: target,
            expr,
            ..
        } => expression_contains_direct_eval(target) || expression_contains_direct_eval(expr),
        Expr::WebCompatCallAssignment { target, discarded } => {
            expression_contains_direct_eval(target)
                || discarded
                    .as_deref()
                    .is_some_and(expression_contains_direct_eval)
        }
        Expr::ComputedPropertyAssignment {
            object,
            property,
            expr,
            ..
        } => {
            expression_contains_direct_eval(object)
                || expression_contains_direct_eval(property)
                || expression_contains_direct_eval(expr)
        }
        Expr::SuperComputedPropertyAssignment { property, expr, .. } => {
            expression_contains_direct_eval(property) || expression_contains_direct_eval(expr)
        }
        _ => false,
    }
}

pub(super) fn direct_eval_callee(expression: &Expression) -> bool {
    match expression.kind() {
        Expr::Identifier(binding) => binding.as_str() == DIRECT_EVAL_BINDING,
        Expr::Parenthesized(expression) => direct_eval_callee(expression),
        _ => false,
    }
}
