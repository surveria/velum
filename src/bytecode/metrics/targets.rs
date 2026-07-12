use crate::bytecode::{BytecodeAssignmentTarget, BytecodeBlock, BytecodeCatch, BytecodeSwitchCase};

impl BytecodeAssignmentTarget {
    pub(super) fn for_each_block(&self, visit: &mut impl FnMut(&BytecodeBlock)) {
        match self {
            Self::Binding(_) => {}
            Self::WebCompatCall(target) => visit(target),
            Self::StaticProperty { object, .. }
            | Self::ArrayIndexProperty { object, .. }
            | Self::PrivateProperty { object, .. } => {
                visit(object);
            }
            Self::ComputedProperty {
                object, property, ..
            } => {
                visit(object);
                visit(property);
            }
        }
    }

    pub(super) fn property_operand_count(&self) -> usize {
        match self {
            Self::Binding(_) => 0,
            Self::WebCompatCall(target) => target.property_operand_count(),
            Self::StaticProperty { object, .. }
            | Self::ArrayIndexProperty { object, .. }
            | Self::PrivateProperty { object, .. } => {
                object.property_operand_count().saturating_add(1)
            }
            Self::ComputedProperty {
                object, property, ..
            } => object
                .property_operand_count()
                .saturating_add(property.property_operand_count()),
        }
    }

    pub(super) fn direct_native_call_count(&self) -> usize {
        match self {
            Self::Binding(_) => 0,
            Self::WebCompatCall(target) => target.direct_native_call_count(),
            Self::StaticProperty { object, .. }
            | Self::ArrayIndexProperty { object, .. }
            | Self::PrivateProperty { object, .. } => object.direct_native_call_count(),
            Self::ComputedProperty {
                object, property, ..
            } => object
                .direct_native_call_count()
                .saturating_add(property.direct_native_call_count()),
        }
    }

    pub(super) fn array_native_call_count(&self) -> usize {
        match self {
            Self::Binding(_) => 0,
            Self::WebCompatCall(target) => target.array_native_call_count(),
            Self::StaticProperty { object, .. }
            | Self::ArrayIndexProperty { object, .. }
            | Self::PrivateProperty { object, .. } => object.array_native_call_count(),
            Self::ComputedProperty {
                object, property, ..
            } => object
                .array_native_call_count()
                .saturating_add(property.array_native_call_count()),
        }
    }

    pub(super) fn numeric_instruction_count(&self) -> usize {
        match self {
            Self::Binding(_) => 0,
            Self::WebCompatCall(target) => target.numeric_instruction_count(),
            Self::StaticProperty { object, .. }
            | Self::ArrayIndexProperty { object, .. }
            | Self::PrivateProperty { object, .. } => object.numeric_instruction_count(),
            Self::ComputedProperty {
                object, property, ..
            } => object
                .numeric_instruction_count()
                .saturating_add(property.numeric_instruction_count()),
        }
    }

    pub(super) fn binding_operand_count(&self) -> usize {
        match self {
            Self::Binding(binding) => binding.direct_operand_count(),
            Self::WebCompatCall(target) => target.binding_operand_count(),
            Self::StaticProperty { object, .. }
            | Self::ArrayIndexProperty { object, .. }
            | Self::PrivateProperty { object, .. } => object.binding_operand_count(),
            Self::ComputedProperty {
                object, property, ..
            } => object
                .binding_operand_count()
                .saturating_add(property.binding_operand_count()),
        }
    }

    pub(super) fn nested_instruction_count(&self) -> usize {
        match self {
            Self::Binding(_) => 0,
            Self::WebCompatCall(target) => target.instruction_count(),
            Self::StaticProperty { object, .. }
            | Self::ArrayIndexProperty { object, .. }
            | Self::PrivateProperty { object, .. } => object.instruction_count(),
            Self::ComputedProperty {
                object, property, ..
            } => object
                .instruction_count()
                .saturating_add(property.instruction_count()),
        }
    }
}

impl BytecodeSwitchCase {
    pub(super) fn property_operand_count(&self) -> usize {
        self.test
            .as_ref()
            .map_or(0, BytecodeBlock::property_operand_count)
            .saturating_add(self.body.property_operand_count())
    }

    pub(super) fn direct_native_call_count(&self) -> usize {
        self.test
            .as_ref()
            .map_or(0, BytecodeBlock::direct_native_call_count)
            .saturating_add(self.body.direct_native_call_count())
    }

    pub(super) fn array_native_call_count(&self) -> usize {
        self.test
            .as_ref()
            .map_or(0, BytecodeBlock::array_native_call_count)
            .saturating_add(self.body.array_native_call_count())
    }

    pub(super) fn numeric_instruction_count(&self) -> usize {
        self.test
            .as_ref()
            .map_or(0, BytecodeBlock::numeric_instruction_count)
            .saturating_add(self.body.numeric_instruction_count())
    }

    pub(super) fn binding_operand_count(&self) -> usize {
        self.test
            .as_ref()
            .map_or(0, BytecodeBlock::binding_operand_count)
            .saturating_add(self.body.binding_operand_count())
    }

    pub(super) fn instruction_count(&self) -> usize {
        self.test
            .as_ref()
            .map_or(0, BytecodeBlock::instruction_count)
            .saturating_add(self.body.instruction_count())
    }
}

impl BytecodeCatch {
    pub(super) fn property_operand_count(&self) -> usize {
        self.param
            .as_ref()
            .map_or(0, |pattern| pattern.property_operand_count())
            .saturating_add(self.body.property_operand_count())
    }

    pub(super) fn direct_native_call_count(&self) -> usize {
        self.param
            .as_ref()
            .map_or(0, |pattern| pattern.direct_native_call_count())
            .saturating_add(self.body.direct_native_call_count())
    }

    pub(super) fn array_native_call_count(&self) -> usize {
        self.param
            .as_ref()
            .map_or(0, |pattern| pattern.array_native_call_count())
            .saturating_add(self.body.array_native_call_count())
    }

    pub(super) fn numeric_instruction_count(&self) -> usize {
        self.param
            .as_ref()
            .map_or(0, |pattern| pattern.numeric_instruction_count())
            .saturating_add(self.body.numeric_instruction_count())
    }

    pub(super) fn binding_operand_count(&self) -> usize {
        let targets = self.param_bindings.iter().fold(0usize, |count, binding| {
            count.saturating_add(binding.direct_operand_count())
        });
        self.param
            .as_ref()
            .map_or(0, |pattern| pattern.binding_operand_count())
            .saturating_add(targets)
            .saturating_add(self.body.binding_operand_count())
    }

    pub(super) fn instruction_count(&self) -> usize {
        self.param
            .as_ref()
            .map_or(0, |pattern| pattern.nested_instruction_count())
            .saturating_add(self.body.instruction_count())
    }
}
