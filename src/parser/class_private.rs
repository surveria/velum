use crate::{
    error::{Error, Result},
    source::SourceSpan,
    syntax::StaticName,
};

use super::Parser;

/// The reserved private name that no class body may declare or reference.
const PRIVATE_CONSTRUCTOR_NAME: &str = "#constructor";

/// Kind of one declared private class element used for duplicate checks: a
/// getter and a setter with matching placement may share one private name.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) enum PrivateElementKind {
    Field,
    Method,
    Getter,
    Setter,
}

/// One `#name` declaration collected while parsing a class body.
#[derive(Debug)]
struct PrivateDeclaration {
    name: StaticName,
    kind: PrivateElementKind,
    is_static: bool,
}

/// One `#name` reference that must resolve to a declaration in the same or
/// an enclosing class body once that body finishes parsing.
#[derive(Debug)]
struct PrivateUse {
    name: StaticName,
    span: SourceSpan,
}

/// Private-name environment for one class body under parsing. References may
/// legally appear before their declaration, so uses are validated when the
/// scope closes and unresolved names bubble to the enclosing class scope.
#[derive(Debug, Default)]
pub(super) struct ClassPrivateScope {
    declared: Vec<PrivateDeclaration>,
    used: Vec<PrivateUse>,
}

impl ClassPrivateScope {
    pub(super) fn external(names: &[StaticName]) -> Self {
        Self {
            declared: names
                .iter()
                .map(|name| PrivateDeclaration {
                    name: name.clone(),
                    kind: PrivateElementKind::Field,
                    is_static: false,
                })
                .collect(),
            used: Vec::new(),
        }
    }

    fn declares(&self, name: &str) -> bool {
        self.declared
            .iter()
            .any(|declaration| declaration.name.as_str() == name)
    }
}

fn undeclared_private_name_error(name: &StaticName, span: SourceSpan) -> Error {
    let name = name.as_str();
    Error::parse_at(
        format!("private name '{name}' must be declared in an enclosing class"),
        span,
    )
}

impl Parser {
    /// Opens the private-name scope of one class body. The scope covers the
    /// class heritage expression and every member, so `#name` references in
    /// `extends` clauses resolve against the class's own declarations.
    pub(super) fn push_class_private_scope(&mut self) {
        self.class_private_scopes.push(ClassPrivateScope::default());
    }

    /// Validates and registers one private element declaration in the
    /// innermost class scope.
    pub(super) fn declare_private_name(
        &mut self,
        name: &StaticName,
        kind: PrivateElementKind,
        is_static: bool,
        offset: usize,
    ) -> Result<()> {
        if name.as_str() == PRIVATE_CONSTRUCTOR_NAME {
            return Err(Error::parse(
                "class private name cannot be '#constructor'",
                offset,
            ));
        }
        let Some(scope) = self.class_private_scopes.last_mut() else {
            let name = name.as_str();
            return Err(Error::parse(
                format!("private name '{name}' is only valid inside a class body"),
                offset,
            ));
        };
        let duplicate = scope
            .declared
            .iter()
            .filter(|existing| existing.name.as_str() == name.as_str())
            .any(|existing| {
                existing.is_static != is_static
                    || !matches!(
                        (existing.kind, kind),
                        (PrivateElementKind::Getter, PrivateElementKind::Setter)
                            | (PrivateElementKind::Setter, PrivateElementKind::Getter)
                    )
            });
        if duplicate {
            let name = name.as_str();
            return Err(Error::parse(
                format!("duplicate private name '{name}'"),
                offset,
            ));
        }
        scope.declared.push(PrivateDeclaration {
            name: name.clone(),
            kind,
            is_static,
        });
        Ok(())
    }

    /// Records one `#name` reference for end-of-class resolution. References
    /// outside any class body are immediate syntax errors.
    pub(super) fn record_private_name_use(
        &mut self,
        name: &StaticName,
        span: SourceSpan,
    ) -> Result<()> {
        let Some(scope) = self.class_private_scopes.last_mut() else {
            return Err(undeclared_private_name_error(name, span));
        };
        scope.used.push(PrivateUse {
            name: name.clone(),
            span,
        });
        Ok(())
    }

    /// Closes the innermost class scope: every collected reference must match
    /// a declaration here or bubble up to an enclosing class body. References
    /// that escape the outermost class are syntax errors.
    pub(super) fn pop_class_private_scope(&mut self) -> Result<()> {
        let Some(scope) = self.class_private_scopes.pop() else {
            return Err(self.parse_error("class private scope underflow"));
        };
        for use_record in &scope.used {
            if scope.declares(use_record.name.as_str()) {
                continue;
            }
            if let Some(parent) = self.class_private_scopes.last_mut() {
                parent.used.push(PrivateUse {
                    name: use_record.name.clone(),
                    span: use_record.span,
                });
            } else {
                return Err(undeclared_private_name_error(
                    &use_record.name,
                    use_record.span,
                ));
            }
        }
        Ok(())
    }
}
