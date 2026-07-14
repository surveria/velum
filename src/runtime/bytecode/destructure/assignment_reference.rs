use crate::{
    bytecode::{BytecodeAssignmentTarget, BytecodeDestructureMode, BytecodePattern},
    error::Result,
    runtime::Context,
};

use super::{
    super::ops::{BytecodeAssignmentReference, web_compat_call_assignment_error},
    PatternStep,
};

impl Context {
    pub(super) fn assignment_reference_for_pattern(
        &mut self,
        pattern: &BytecodePattern,
        mode: BytecodeDestructureMode,
    ) -> Result<PatternStep<Option<BytecodeAssignmentReference>>> {
        let step = match pattern {
            BytecodePattern::Assignment(target) => {
                self.eval_resumable_assignment_reference(target)?
            }
            BytecodePattern::Binding(name)
                if matches!(
                    mode,
                    BytecodeDestructureMode::Declaration(crate::syntax::DeclKind::Var)
                ) =>
            {
                if let Some(reference) = self.resolve_with_binding(name)? {
                    PatternStep::Value(BytecodeAssignmentReference::WithBinding {
                        name: name.clone(),
                        reference,
                    })
                } else {
                    let cell = self.get_or_materialize_binding_bytecode(name)?;
                    PatternStep::Value(BytecodeAssignmentReference::Binding {
                        name: name.clone(),
                        cell,
                    })
                }
            }
            BytecodePattern::Binding(_)
            | BytecodePattern::Object { .. }
            | BytecodePattern::Array { .. } => return Ok(PatternStep::Value(None)),
        };
        Ok(match step {
            PatternStep::Value(reference) => PatternStep::Value(Some(reference)),
            PatternStep::Abrupt(completion) => PatternStep::Abrupt(completion),
        })
    }

    fn eval_resumable_assignment_reference(
        &mut self,
        target: &BytecodeAssignmentTarget,
    ) -> Result<PatternStep<BytecodeAssignmentReference>> {
        match target {
            BytecodeAssignmentTarget::Binding(name) => {
                if let Some(reference) = self.resolve_with_binding(name)? {
                    return Ok(PatternStep::Value(
                        BytecodeAssignmentReference::WithBinding {
                            name: name.clone(),
                            reference,
                        },
                    ));
                }
                let cell = self.get_or_materialize_binding_bytecode(name)?;
                Ok(PatternStep::Value(BytecodeAssignmentReference::Binding {
                    name: name.clone(),
                    cell,
                }))
            }
            BytecodeAssignmentTarget::WebCompatCall(target) => {
                self.eval_web_compat_reference(target)
            }
            BytecodeAssignmentTarget::StaticProperty {
                object,
                property,
                strict,
            } => {
                let object = match self.eval_pattern_block(object)? {
                    PatternStep::Value(object) => object,
                    PatternStep::Abrupt(completion) => {
                        return Ok(PatternStep::Abrupt(completion));
                    }
                };
                Ok(PatternStep::Value(
                    BytecodeAssignmentReference::StaticProperty {
                        object,
                        property: property.clone(),
                        strict: *strict,
                    },
                ))
            }
            BytecodeAssignmentTarget::ArrayIndexProperty {
                object,
                property,
                index,
                strict,
            } => {
                let object = match self.eval_pattern_block(object)? {
                    PatternStep::Value(object) => object,
                    PatternStep::Abrupt(completion) => {
                        return Ok(PatternStep::Abrupt(completion));
                    }
                };
                Ok(PatternStep::Value(
                    BytecodeAssignmentReference::ArrayIndexProperty {
                        object,
                        property: property.clone(),
                        index: *index,
                        strict: *strict,
                    },
                ))
            }
            BytecodeAssignmentTarget::ComputedProperty {
                object,
                property,
                operand,
                strict,
            } => {
                let object = match self.eval_pattern_block(object)? {
                    PatternStep::Value(object) => object,
                    PatternStep::Abrupt(completion) => {
                        return Ok(PatternStep::Abrupt(completion));
                    }
                };
                let property_value = match self.eval_pattern_block(property)? {
                    PatternStep::Value(property) => property,
                    PatternStep::Abrupt(completion) => {
                        return Ok(PatternStep::Abrupt(completion));
                    }
                };
                Ok(PatternStep::Value(
                    BytecodeAssignmentReference::ComputedProperty {
                        object,
                        property_value,
                        property: None,
                        access: operand.access(),
                        strict: *strict,
                    },
                ))
            }
            BytecodeAssignmentTarget::PrivateProperty { object, property } => {
                self.eval_resumable_private_reference(object, property)
            }
            BytecodeAssignmentTarget::SuperProperty { property, strict } => {
                self.eval_resumable_super_reference(property, *strict)
            }
        }
    }

    fn eval_web_compat_reference(
        &mut self,
        target: &crate::bytecode::BytecodeBlock,
    ) -> Result<PatternStep<BytecodeAssignmentReference>> {
        match self.eval_pattern_block(target)? {
            PatternStep::Value(_) => Err(web_compat_call_assignment_error()),
            PatternStep::Abrupt(completion) => Ok(PatternStep::Abrupt(completion)),
        }
    }

    fn eval_resumable_private_reference(
        &mut self,
        object: &crate::bytecode::BytecodeBlock,
        property: &crate::bytecode::BytecodePrivateName,
    ) -> Result<PatternStep<BytecodeAssignmentReference>> {
        let object = match self.eval_pattern_block(object)? {
            PatternStep::Value(object) => object,
            PatternStep::Abrupt(completion) => return Ok(PatternStep::Abrupt(completion)),
        };
        let name = self.resolve_private_name(property)?;
        Ok(PatternStep::Value(
            BytecodeAssignmentReference::PrivateProperty { object, name },
        ))
    }

    fn eval_resumable_super_reference(
        &mut self,
        property: &crate::bytecode::BytecodeSuperProperty,
        strict: bool,
    ) -> Result<PatternStep<BytecodeAssignmentReference>> {
        let receiver = self.super_assignment_receiver()?;
        match property {
            crate::bytecode::BytecodeSuperProperty::Static(property) => self
                .finish_static_super_assignment_reference(receiver, property, strict)
                .map(PatternStep::Value),
            crate::bytecode::BytecodeSuperProperty::Computed {
                expression,
                operand,
            } => {
                let property_value = match self.eval_pattern_block(expression)? {
                    PatternStep::Value(value) => value,
                    PatternStep::Abrupt(completion) => {
                        return Ok(PatternStep::Abrupt(completion));
                    }
                };
                self.finish_computed_super_assignment_reference(
                    receiver,
                    property_value,
                    operand.access(),
                    strict,
                )
                .map(PatternStep::Value)
            }
        }
    }
}
