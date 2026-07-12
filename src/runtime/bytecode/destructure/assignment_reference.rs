use crate::{
    bytecode::{BytecodeAssignmentTarget, BytecodePattern},
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
    ) -> Result<PatternStep<Option<BytecodeAssignmentReference>>> {
        let BytecodePattern::Assignment(target) = pattern else {
            return Ok(PatternStep::Value(None));
        };
        self.eval_resumable_assignment_reference(target)
            .map(|step| match step {
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
                let property = self.dynamic_property_key(&property_value)?;
                Ok(PatternStep::Value(
                    BytecodeAssignmentReference::ComputedProperty {
                        object,
                        property_value,
                        property,
                        access: operand.access(),
                        strict: *strict,
                    },
                ))
            }
            BytecodeAssignmentTarget::PrivateProperty { object, property } => {
                self.eval_resumable_private_reference(object, property)
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
}
