use crate::{
    ast::{ClassElementName, Expression, ObjectPropertyKey},
    error::Result,
};

use super::BytecodeCompiler;

struct ClassElementInput<'a> {
    source_order: usize,
    decorators: &'a [Expression],
    key: &'a ClassElementName,
}

impl BytecodeCompiler<'_> {
    pub(super) fn compile_class_element_inputs(
        &mut self,
        class: &crate::ast::ClassLiteral,
    ) -> Result<()> {
        let mut elements = class
            .members
            .iter()
            .map(|member| ClassElementInput {
                source_order: member.source_order,
                decorators: &member.decorators,
                key: &member.key,
            })
            .chain(class.fields.iter().map(|field| ClassElementInput {
                source_order: field.source_order,
                decorators: &field.decorators,
                key: &field.key,
            }))
            .collect::<Vec<_>>();
        elements.sort_by_key(|element| element.source_order);
        for element in elements {
            for decorator in element.decorators {
                self.compile_expr(decorator)?;
            }
            if let ClassElementName::Property(ObjectPropertyKey::Computed(key)) = element.key {
                self.compile_expr(key)?;
            }
        }
        Ok(())
    }
}
