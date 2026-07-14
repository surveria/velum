use std::rc::Rc;

use crate::{
    ast::{Expr, ObjectProperty, ObjectPropertyKey, ObjectPropertyKind},
    bytecode::{BytecodeInstruction, BytecodeObjectProperty},
    error::Result,
    syntax::AccessorKind,
};

use super::BytecodeCompiler;

impl BytecodeCompiler<'_> {
    pub(super) fn compile_object_literal(&mut self, properties: &[ObjectProperty]) -> Result<()> {
        let mut operands = Vec::with_capacity(properties.len());
        for property in properties {
            if property.kind == ObjectPropertyKind::Spread {
                self.compile_expr(&property.value)?;
                operands.push(BytecodeObjectProperty::Spread);
                continue;
            }
            let accessor = match property.kind {
                ObjectPropertyKind::Init | ObjectPropertyKind::Spread => None,
                ObjectPropertyKind::Get => Some(AccessorKind::Getter),
                ObjectPropertyKind::Set => Some(AccessorKind::Setter),
            };
            match &property.key {
                ObjectPropertyKey::Static(key) => {
                    let operand = accessor.map_or_else(
                        || {
                            if matches!(property.value.kind(), Expr::MethodFunction { .. }) {
                                BytecodeObjectProperty::StaticMethod(key.clone())
                            } else {
                                BytecodeObjectProperty::Static(key.clone())
                            }
                        },
                        |kind| BytecodeObjectProperty::StaticAccessor {
                            key: key.clone(),
                            kind,
                        },
                    );
                    operands.push(operand);
                }
                ObjectPropertyKey::Computed(expr) => {
                    self.compile_expr(expr)?;
                    self.emit(BytecodeInstruction::ToPropertyKey);
                    let property = match accessor {
                        Some(kind) => BytecodeObjectProperty::ComputedAccessor { kind },
                        None if matches!(property.value.kind(), Expr::MethodFunction { .. }) => {
                            BytecodeObjectProperty::ComputedMethod
                        }
                        None if Self::is_anonymous_function_definition(&property.value) => {
                            BytecodeObjectProperty::ComputedInferredName
                        }
                        None => BytecodeObjectProperty::Computed,
                    };
                    operands.push(property);
                }
            }
            if property.kind == ObjectPropertyKind::Init
                && let ObjectPropertyKey::Static(key) = &property.key
            {
                self.compile_expr_with_inferred_name(&property.value, key)?;
            } else {
                self.compile_expr(&property.value)?;
            }
        }
        self.emit(BytecodeInstruction::ObjectLiteral {
            properties: Rc::from(operands.into_boxed_slice()),
        });
        Ok(())
    }
}
