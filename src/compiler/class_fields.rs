#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    ast::ClassAutoAccessor,
    bytecode::{BytecodeBlock, BytecodeClassAutoAccessor, BytecodeClassField, BytecodeFunction},
    error::{Error, Result},
    syntax::StaticName,
};

use super::{BytecodeCompiler, FunctionCompileMode};

impl BytecodeCompiler<'_> {
    /// Lowers logical class fields in source order. Auto-accessor getter and
    /// setter functions share the field's one resolved public key.
    pub(super) fn compile_class_fields(
        &self,
        class: &crate::ast::ClassLiteral,
        private_names: &[StaticName],
    ) -> Result<Vec<BytecodeClassField>> {
        let mut fields = Vec::with_capacity(class.fields.len());
        for field in &class.fields {
            let key = Self::lower_class_element_key(&field.key, private_names)?;
            let infer_name_from_computed_key = field.name.is_none()
                && field
                    .initializer
                    .as_ref()
                    .is_some_and(Self::is_anonymous_function_definition);
            let auto_accessor = field
                .auto_accessor
                .as_ref()
                .map(|accessor| self.compile_auto_accessor(accessor, private_names))
                .transpose()?;
            fields.push(BytecodeClassField {
                key,
                decorator_count: field.decorators.len(),
                is_static: field.is_static,
                auto_accessor,
                name: field.name.clone(),
                infer_name_from_computed_key,
                initializer: field
                    .initializer
                    .as_ref()
                    .map(|initializer| {
                        field.name.as_ref().map_or_else(
                            || BytecodeBlock::compile_expression(initializer, self.layout),
                            |name| {
                                BytecodeBlock::compile_expression_with_inferred_name(
                                    initializer,
                                    name,
                                    self.layout,
                                )
                            },
                        )
                    })
                    .transpose()?,
            });
        }
        Ok(fields)
    }

    fn compile_auto_accessor(
        &self,
        accessor: &ClassAutoAccessor,
        private_names: &[StaticName],
    ) -> Result<BytecodeClassAutoAccessor> {
        let backing_name_index = private_names
            .iter()
            .position(|candidate| candidate.as_str() == accessor.backing_name.as_str())
            .ok_or_else(|| Error::runtime("auto-accessor backing name disappeared"))?;
        let backing_name_index = u32::try_from(backing_name_index)
            .map_err(|_| Error::limit("auto-accessor backing name index overflowed"))?;
        Ok(BytecodeClassAutoAccessor {
            backing_name_index,
            getter_id: accessor.getter.id,
            getter: BytecodeFunction::compile(
                None,
                None,
                &accessor.getter.params,
                &accessor.getter.body,
                FunctionCompileMode::new(crate::syntax::FunctionKind::Ordinary, true),
                self.layout,
                None,
            )?,
            setter_id: accessor.setter.id,
            setter: BytecodeFunction::compile(
                None,
                None,
                &accessor.setter.params,
                &accessor.setter.body,
                FunctionCompileMode::new(crate::syntax::FunctionKind::Ordinary, true),
                self.layout,
                None,
            )?,
        })
    }
}
