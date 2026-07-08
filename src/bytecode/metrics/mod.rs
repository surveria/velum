use crate::{
    binding_metadata::BindingOperand,
    bytecode::{
        BytecodeAssignmentTarget, BytecodeBinding, BytecodeBlock, BytecodeCatch, BytecodeClass,
        BytecodeForInTarget, BytecodeInstruction, BytecodePattern, BytecodePatternKey,
        BytecodeProgram, BytecodeSwitchCase,
    },
};

mod traversal;

use traversal::{count_blocks_2, count_for_blocks, count_switch, count_try};

impl BytecodeProgram {
    pub fn instruction_count(&self) -> usize {
        self.block().instruction_count()
    }

    pub fn binding_operand_count(&self) -> usize {
        self.block().binding_operand_count()
    }

    pub fn property_operand_count(&self) -> usize {
        self.block().property_operand_count()
    }

    pub fn direct_native_call_count(&self) -> usize {
        self.block().direct_native_call_count()
    }

    pub fn array_native_call_count(&self) -> usize {
        self.block().array_native_call_count()
    }

    pub fn numeric_instruction_count(&self) -> usize {
        self.block().numeric_instruction_count()
    }
}

impl BytecodeBlock {
    pub fn instruction_count(&self) -> usize {
        let nested = self
            .instructions()
            .iter()
            .map(BytecodeInstruction::nested_instruction_count)
            .sum::<usize>();
        self.instructions().len().saturating_add(nested)
    }

    pub fn binding_operand_count(&self) -> usize {
        self.instructions()
            .iter()
            .map(BytecodeInstruction::binding_operand_count)
            .sum()
    }

    pub fn property_operand_count(&self) -> usize {
        self.instructions()
            .iter()
            .map(BytecodeInstruction::property_operand_count)
            .sum()
    }

    pub fn direct_native_call_count(&self) -> usize {
        self.instructions()
            .iter()
            .map(BytecodeInstruction::direct_native_call_count)
            .sum()
    }

    pub fn array_native_call_count(&self) -> usize {
        self.instructions()
            .iter()
            .map(BytecodeInstruction::array_native_call_count)
            .sum()
    }

    pub fn numeric_instruction_count(&self) -> usize {
        self.instructions()
            .iter()
            .map(BytecodeInstruction::numeric_instruction_count)
            .sum()
    }
}

impl BytecodeInstruction {
    fn binding_operand_count(&self) -> usize {
        match self {
            Self::LoadBinding(binding)
            | Self::StoreBinding(binding)
            | Self::TypeOfBinding(binding)
            | Self::DeleteBinding(binding) => binding.direct_operand_count(),
            Self::DeclareBinding { name, .. } => name.direct_operand_count(),
            Self::UpdateBinding { name, .. } | Self::CompoundStoreBinding { name, .. } => {
                name.direct_operand_count()
            }
            Self::CallBinding { callee, .. } | Self::CallBindingSpread { callee } => {
                callee.direct_operand_count()
            }
            Self::Construct { constructor, .. } => constructor.direct_operand_count(),
            Self::NullishCoalescing { right } => right.binding_operand_count(),
            Self::LogicalAssignment { target, value, .. } => target
                .binding_operand_count()
                .saturating_add(value.binding_operand_count()),
            Self::While {
                condition, body, ..
            } => count_blocks_2(condition, body, BytecodeBlock::binding_operand_count),
            Self::For {
                init,
                condition,
                update,
                body,
                ..
            } => count_for_blocks(
                init.as_ref(),
                condition.as_ref(),
                update.as_ref(),
                body,
                BytecodeBlock::binding_operand_count,
            ),
            Self::ForIn {
                target,
                object,
                body,
                ..
            }
            | Self::ForOf {
                target,
                object,
                body,
                ..
            } => target
                .binding_operand_count()
                .saturating_add(object.binding_operand_count())
                .saturating_add(body.binding_operand_count()),
            Self::Switch {
                discriminant,
                cases,
            } => count_switch(
                discriminant,
                cases,
                BytecodeBlock::binding_operand_count,
                BytecodeSwitchCase::binding_operand_count,
            ),
            Self::Try {
                body,
                catch,
                finally_body,
            } => count_try(
                body,
                catch.as_ref(),
                finally_body.as_ref(),
                BytecodeBlock::binding_operand_count,
                BytecodeCatch::binding_operand_count,
            ),
            Self::Label { body, .. } => body.binding_operand_count(),
            Self::ScopedBlock(block) => block.binding_operand_count(),
            Self::CreateFunction { bytecode, .. } => bytecode.body().binding_operand_count(),
            Self::CreateClass { class } => class.binding_operand_count(),
            instruction if instruction.is_leaf_instruction() => 0,
            _ => 0,
        }
    }

    fn property_operand_count(&self) -> usize {
        match self {
            Self::DeleteStaticProperty { .. }
            | Self::DeleteComputedProperty { .. }
            | Self::UpdateStaticProperty { .. }
            | Self::UpdateArrayIndexProperty { .. }
            | Self::UpdateComputedProperty { .. }
            | Self::Binary {
                property_access: Some(_),
                ..
            }
            | Self::CompoundStaticProperty { .. }
            | Self::CompoundArrayIndexProperty { .. }
            | Self::CompoundComputedProperty { .. }
            | Self::StaticMember { .. }
            | Self::ArrayLength { .. }
            | Self::ArrayIndexMember { .. }
            | Self::ComputedMember { .. }
            | Self::StaticPropertyAssign { .. }
            | Self::ArrayIndexAssign { .. }
            | Self::ComputedPropertyAssign { .. }
            | Self::CallStaticMember { .. }
            | Self::CallComputedMember { .. }
            | Self::CallStaticMemberSpread { .. }
            | Self::CallComputedMemberSpread { .. }
            | Self::SuperMember { .. }
            | Self::CallSuperMember { .. }
            | Self::CallSuperMemberSpread { .. } => 1,
            Self::NullishCoalescing { right } => right.property_operand_count(),
            Self::LogicalAssignment { target, value, .. } => target
                .property_operand_count()
                .saturating_add(value.property_operand_count()),
            Self::While {
                condition, body, ..
            } => count_blocks_2(condition, body, BytecodeBlock::property_operand_count),
            Self::For {
                init,
                condition,
                update,
                body,
                ..
            } => count_for_blocks(
                init.as_ref(),
                condition.as_ref(),
                update.as_ref(),
                body,
                BytecodeBlock::property_operand_count,
            ),
            Self::ForIn {
                target,
                object,
                body,
                ..
            }
            | Self::ForOf {
                target,
                object,
                body,
                ..
            } => target
                .property_operand_count()
                .saturating_add(object.property_operand_count())
                .saturating_add(body.property_operand_count()),
            Self::Switch {
                discriminant,
                cases,
            } => count_switch(
                discriminant,
                cases,
                BytecodeBlock::property_operand_count,
                BytecodeSwitchCase::property_operand_count,
            ),
            Self::Try {
                body,
                catch,
                finally_body,
            } => count_try(
                body,
                catch.as_ref(),
                finally_body.as_ref(),
                BytecodeBlock::property_operand_count,
                BytecodeCatch::property_operand_count,
            ),
            Self::Label { body, .. } => body.property_operand_count(),
            Self::ScopedBlock(block) => block.property_operand_count(),
            Self::CreateFunction { bytecode, .. } => bytecode.body().property_operand_count(),
            Self::CreateClass { class } => class.property_operand_count(),
            instruction if instruction.is_leaf_instruction() => 0,
            _ => 0,
        }
    }

    fn direct_native_call_count(&self) -> usize {
        match self {
            Self::CallBinding { native, .. }
            | Self::CallStaticMember { native, .. }
            | Self::CallComputedMember { native, .. }
            | Self::Construct { native, .. } => usize::from(native.is_some()),
            Self::NullishCoalescing { right } => right.direct_native_call_count(),
            Self::LogicalAssignment { target, value, .. } => target
                .direct_native_call_count()
                .saturating_add(value.direct_native_call_count()),
            Self::While {
                condition, body, ..
            } => count_blocks_2(condition, body, BytecodeBlock::direct_native_call_count),
            Self::For {
                init,
                condition,
                update,
                body,
                ..
            } => count_for_blocks(
                init.as_ref(),
                condition.as_ref(),
                update.as_ref(),
                body,
                BytecodeBlock::direct_native_call_count,
            ),
            Self::ForIn {
                target,
                object,
                body,
                ..
            }
            | Self::ForOf {
                target,
                object,
                body,
                ..
            } => target
                .direct_native_call_count()
                .saturating_add(object.direct_native_call_count())
                .saturating_add(body.direct_native_call_count()),
            Self::Switch {
                discriminant,
                cases,
            } => count_switch(
                discriminant,
                cases,
                BytecodeBlock::direct_native_call_count,
                BytecodeSwitchCase::direct_native_call_count,
            ),
            Self::Try {
                body,
                catch,
                finally_body,
            } => count_try(
                body,
                catch.as_ref(),
                finally_body.as_ref(),
                BytecodeBlock::direct_native_call_count,
                BytecodeCatch::direct_native_call_count,
            ),
            Self::Label { body, .. } => body.direct_native_call_count(),
            Self::ScopedBlock(block) => block.direct_native_call_count(),
            Self::CreateFunction { bytecode, .. } => bytecode.body().direct_native_call_count(),
            Self::CreateClass { class } => class.direct_native_call_count(),
            instruction if instruction.is_leaf_instruction() => 0,
            _ => 0,
        }
    }

    fn array_native_call_count(&self) -> usize {
        match self {
            Self::CallBinding { native, .. }
            | Self::CallStaticMember { native, .. }
            | Self::CallComputedMember { native, .. }
            | Self::Construct { native, .. } => {
                native.map_or(0, |target| usize::from(target.is_array_target()))
            }
            Self::NullishCoalescing { right } => right.array_native_call_count(),
            Self::LogicalAssignment { target, value, .. } => target
                .array_native_call_count()
                .saturating_add(value.array_native_call_count()),
            Self::While {
                condition, body, ..
            } => count_blocks_2(condition, body, BytecodeBlock::array_native_call_count),
            Self::For {
                init,
                condition,
                update,
                body,
                ..
            } => count_for_blocks(
                init.as_ref(),
                condition.as_ref(),
                update.as_ref(),
                body,
                BytecodeBlock::array_native_call_count,
            ),
            Self::ForIn {
                target,
                object,
                body,
                ..
            }
            | Self::ForOf {
                target,
                object,
                body,
                ..
            } => target
                .array_native_call_count()
                .saturating_add(object.array_native_call_count())
                .saturating_add(body.array_native_call_count()),
            Self::Switch {
                discriminant,
                cases,
            } => count_switch(
                discriminant,
                cases,
                BytecodeBlock::array_native_call_count,
                BytecodeSwitchCase::array_native_call_count,
            ),
            Self::Try {
                body,
                catch,
                finally_body,
            } => count_try(
                body,
                catch.as_ref(),
                finally_body.as_ref(),
                BytecodeBlock::array_native_call_count,
                BytecodeCatch::array_native_call_count,
            ),
            Self::Label { body, .. } => body.array_native_call_count(),
            Self::ScopedBlock(block) => block.array_native_call_count(),
            Self::CreateFunction { bytecode, .. } => bytecode.body().array_native_call_count(),
            Self::CreateClass { class } => class.array_native_call_count(),
            instruction if instruction.is_leaf_instruction() => 0,
            _ => 0,
        }
    }

    fn numeric_instruction_count(&self) -> usize {
        match self {
            Self::NumberUnary(_)
            | Self::NumberBinary(_)
            | Self::NumberCompare(_)
            | Self::NumberEquality(_) => 1,
            Self::NullishCoalescing { right } => right.numeric_instruction_count(),
            Self::LogicalAssignment { target, value, .. } => target
                .numeric_instruction_count()
                .saturating_add(value.numeric_instruction_count()),
            Self::While {
                condition, body, ..
            } => count_blocks_2(condition, body, BytecodeBlock::numeric_instruction_count),
            Self::For {
                init,
                condition,
                update,
                body,
                ..
            } => count_for_blocks(
                init.as_ref(),
                condition.as_ref(),
                update.as_ref(),
                body,
                BytecodeBlock::numeric_instruction_count,
            ),
            Self::ForIn {
                target,
                object,
                body,
                ..
            }
            | Self::ForOf {
                target,
                object,
                body,
                ..
            } => target
                .numeric_instruction_count()
                .saturating_add(object.numeric_instruction_count())
                .saturating_add(body.numeric_instruction_count()),
            Self::Switch {
                discriminant,
                cases,
            } => count_switch(
                discriminant,
                cases,
                BytecodeBlock::numeric_instruction_count,
                BytecodeSwitchCase::numeric_instruction_count,
            ),
            Self::Try {
                body,
                catch,
                finally_body,
            } => count_try(
                body,
                catch.as_ref(),
                finally_body.as_ref(),
                BytecodeBlock::numeric_instruction_count,
                BytecodeCatch::numeric_instruction_count,
            ),
            Self::Label { body, .. } => body.numeric_instruction_count(),
            Self::ScopedBlock(block) => block.numeric_instruction_count(),
            Self::CreateFunction { bytecode, .. } => bytecode.body().numeric_instruction_count(),
            Self::CreateClass { class } => class.numeric_instruction_count(),
            instruction if instruction.is_leaf_instruction() => 0,
            _ => 0,
        }
    }

    fn nested_instruction_count(&self) -> usize {
        match self {
            Self::NullishCoalescing { right } => right.instruction_count(),
            Self::LogicalAssignment { target, value, .. } => target
                .nested_instruction_count()
                .saturating_add(value.instruction_count()),
            Self::While {
                condition, body, ..
            } => count_blocks_2(condition, body, BytecodeBlock::instruction_count),
            Self::For {
                init,
                condition,
                update,
                body,
                ..
            } => count_for_blocks(
                init.as_ref(),
                condition.as_ref(),
                update.as_ref(),
                body,
                BytecodeBlock::instruction_count,
            ),
            Self::ForIn {
                object,
                body,
                target,
                ..
            }
            | Self::ForOf {
                object,
                body,
                target,
                ..
            } => object
                .instruction_count()
                .saturating_add(body.instruction_count())
                .saturating_add(target.nested_instruction_count()),
            Self::Switch {
                discriminant,
                cases,
            } => count_switch(
                discriminant,
                cases,
                BytecodeBlock::instruction_count,
                BytecodeSwitchCase::instruction_count,
            ),
            Self::Try {
                body,
                catch,
                finally_body,
            } => count_try(
                body,
                catch.as_ref(),
                finally_body.as_ref(),
                BytecodeBlock::instruction_count,
                BytecodeCatch::instruction_count,
            ),
            Self::Label { body, .. } => body.instruction_count(),
            Self::ScopedBlock(block) => block.instruction_count(),
            Self::CreateFunction { bytecode, .. } => bytecode.body().instruction_count(),
            Self::CreateClass { class } => class.instruction_count(),
            instruction if instruction.is_leaf_instruction() => 0,
            _ => 0,
        }
    }

    const fn is_leaf_instruction(&self) -> bool {
        matches!(
            self,
            Self::DeleteBinding(_)
                | Self::DeleteStaticProperty { .. }
                | Self::DeleteComputedProperty { .. }
                | Self::DeleteValue
                | Self::UpdateBinding { .. }
                | Self::UpdateStaticProperty { .. }
                | Self::UpdateArrayIndexProperty { .. }
                | Self::UpdateComputedProperty { .. }
                | Self::CompoundStoreBinding { .. }
                | Self::CompoundStaticProperty { .. }
                | Self::CompoundArrayIndexProperty { .. }
                | Self::CompoundComputedProperty { .. }
                | Self::CallBinding { .. }
                | Self::CallValue { .. }
                | Self::CallStaticMember { .. }
                | Self::CallComputedMember { .. }
                | Self::Print { .. }
                | Self::AssertThrows { .. }
                | Self::Construct { .. }
                | Self::ConstructValue { .. }
                | Self::PushLiteral(_)
                | Self::PushString(_)
                | Self::TemplateConcat { .. }
                | Self::CollectSpreadArgs { .. }
                | Self::CallBindingSpread { .. }
                | Self::CallValueSpread
                | Self::CallStaticMemberSpread { .. }
                | Self::CallComputedMemberSpread { .. }
                | Self::ConstructValueSpread
                | Self::ArrayLiteralSpread { .. }
                | Self::CallSuper { .. }
                | Self::CallSuperSpread
                | Self::SuperMember { .. }
                | Self::CallSuperMember { .. }
                | Self::CallSuperMemberSpread { .. }
                | Self::CreateRegExp { .. }
                | Self::PushUndefined
                | Self::LoadThis
                | Self::LoadNewTarget
                | Self::LoadBinding(_)
                | Self::StoreBinding(_)
                | Self::DeclareBinding { .. }
                | Self::StoreLast
                | Self::Pop
                | Self::Unary(_)
                | Self::NumberUnary(_)
                | Self::TypeOfBinding(_)
                | Self::TypeOfValue
                | Self::Binary { .. }
                | Self::NumberBinary(_)
                | Self::NumberCompare(_)
                | Self::NumberEquality(_)
                | Self::StaticMember { .. }
                | Self::ArrayLength { .. }
                | Self::ArrayIndexMember { .. }
                | Self::ComputedMember { .. }
                | Self::StaticPropertyAssign { .. }
                | Self::ArrayIndexAssign { .. }
                | Self::ComputedPropertyAssign { .. }
                | Self::ArrayLiteral { .. }
                | Self::ObjectLiteral { .. }
                | Self::Jump(_)
                | Self::JumpIfFalse(_)
                | Self::JumpIfFalseKeep(_)
                | Self::JumpIfTrueKeep(_)
                | Self::Complete(_)
        )
    }
}

impl BytecodeBinding {
    const fn direct_operand_count(&self) -> usize {
        !matches!(self.operand(), BindingOperand::Unresolved) as usize
    }
}

impl BytecodeForInTarget {
    fn property_operand_count(&self) -> usize {
        match self {
            Self::Binding { .. } => 0,
            Self::PatternBinding { pattern, .. } => pattern.property_operand_count(),
            Self::Assignment(target) => target.property_operand_count(),
        }
    }

    fn direct_native_call_count(&self) -> usize {
        match self {
            Self::Binding { .. } => 0,
            Self::PatternBinding { pattern, .. } => pattern.direct_native_call_count(),
            Self::Assignment(target) => target.direct_native_call_count(),
        }
    }

    fn array_native_call_count(&self) -> usize {
        match self {
            Self::Binding { .. } => 0,
            Self::PatternBinding { pattern, .. } => pattern.array_native_call_count(),
            Self::Assignment(target) => target.array_native_call_count(),
        }
    }

    fn numeric_instruction_count(&self) -> usize {
        match self {
            Self::Binding { .. } => 0,
            Self::PatternBinding { pattern, .. } => pattern.numeric_instruction_count(),
            Self::Assignment(target) => target.numeric_instruction_count(),
        }
    }

    fn binding_operand_count(&self) -> usize {
        match self {
            Self::Binding { name, .. } => name.direct_operand_count(),
            Self::PatternBinding { pattern, .. } => pattern.binding_operand_count(),
            Self::Assignment(target) => target.binding_operand_count(),
        }
    }

    fn nested_instruction_count(&self) -> usize {
        match self {
            Self::Binding { .. } => 0,
            Self::PatternBinding { pattern, .. } => pattern.nested_instruction_count(),
            Self::Assignment(target) => target.nested_instruction_count(),
        }
    }
}

impl BytecodeClass {
    fn sum_bodies(&self, count: fn(&BytecodeBlock) -> usize) -> usize {
        let mut total = count(self.constructor.body());
        for member in self.members.iter() {
            total = total.saturating_add(count(member.bytecode.body()));
        }
        total
    }

    fn binding_operand_count(&self) -> usize {
        self.sum_bodies(BytecodeBlock::binding_operand_count)
    }

    fn property_operand_count(&self) -> usize {
        self.sum_bodies(BytecodeBlock::property_operand_count)
    }

    fn direct_native_call_count(&self) -> usize {
        self.sum_bodies(BytecodeBlock::direct_native_call_count)
    }

    fn array_native_call_count(&self) -> usize {
        self.sum_bodies(BytecodeBlock::array_native_call_count)
    }

    fn numeric_instruction_count(&self) -> usize {
        self.sum_bodies(BytecodeBlock::numeric_instruction_count)
    }

    fn instruction_count(&self) -> usize {
        self.sum_bodies(BytecodeBlock::instruction_count)
    }
}

impl BytecodePattern {
    fn for_each_block(&self, count: &mut impl FnMut(&BytecodeBlock)) {
        match self {
            Self::Binding(_) => {}
            Self::Object { properties, .. } => {
                for property in properties.iter() {
                    if let BytecodePatternKey::Computed(block) = &property.key {
                        count(block);
                    }
                    if let Some(default) = &property.target.default {
                        count(default);
                    }
                    property.target.pattern.for_each_block(count);
                }
            }
            Self::Array { elements, rest } => {
                for element in elements.iter().flatten() {
                    if let Some(default) = &element.default {
                        count(default);
                    }
                    element.pattern.for_each_block(count);
                }
                if let Some(rest) = rest {
                    rest.for_each_block(count);
                }
            }
        }
    }

    fn sum_blocks(&self, count: fn(&BytecodeBlock) -> usize) -> usize {
        let mut total = 0usize;
        self.for_each_block(&mut |block| total = total.saturating_add(count(block)));
        total
    }

    fn property_operand_count(&self) -> usize {
        self.sum_blocks(BytecodeBlock::property_operand_count)
    }

    fn direct_native_call_count(&self) -> usize {
        self.sum_blocks(BytecodeBlock::direct_native_call_count)
    }

    fn array_native_call_count(&self) -> usize {
        self.sum_blocks(BytecodeBlock::array_native_call_count)
    }

    fn numeric_instruction_count(&self) -> usize {
        self.sum_blocks(BytecodeBlock::numeric_instruction_count)
    }

    fn binding_operand_count(&self) -> usize {
        self.sum_blocks(BytecodeBlock::binding_operand_count)
    }

    fn nested_instruction_count(&self) -> usize {
        self.sum_blocks(BytecodeBlock::instruction_count)
    }
}

impl BytecodeAssignmentTarget {
    fn property_operand_count(&self) -> usize {
        match self {
            Self::Binding(_) => 0,
            Self::StaticProperty { object, .. } | Self::ArrayIndexProperty { object, .. } => {
                object.property_operand_count().saturating_add(1)
            }
            Self::ComputedProperty {
                object, property, ..
            } => object
                .property_operand_count()
                .saturating_add(property.property_operand_count()),
        }
    }

    fn direct_native_call_count(&self) -> usize {
        match self {
            Self::Binding(_) => 0,
            Self::StaticProperty { object, .. } | Self::ArrayIndexProperty { object, .. } => {
                object.direct_native_call_count()
            }
            Self::ComputedProperty {
                object, property, ..
            } => object
                .direct_native_call_count()
                .saturating_add(property.direct_native_call_count()),
        }
    }

    fn array_native_call_count(&self) -> usize {
        match self {
            Self::Binding(_) => 0,
            Self::StaticProperty { object, .. } | Self::ArrayIndexProperty { object, .. } => {
                object.array_native_call_count()
            }
            Self::ComputedProperty {
                object, property, ..
            } => object
                .array_native_call_count()
                .saturating_add(property.array_native_call_count()),
        }
    }

    fn numeric_instruction_count(&self) -> usize {
        match self {
            Self::Binding(_) => 0,
            Self::StaticProperty { object, .. } | Self::ArrayIndexProperty { object, .. } => {
                object.numeric_instruction_count()
            }
            Self::ComputedProperty {
                object, property, ..
            } => object
                .numeric_instruction_count()
                .saturating_add(property.numeric_instruction_count()),
        }
    }

    fn binding_operand_count(&self) -> usize {
        match self {
            Self::Binding(binding) => binding.direct_operand_count(),
            Self::StaticProperty { object, .. } | Self::ArrayIndexProperty { object, .. } => {
                object.binding_operand_count()
            }
            Self::ComputedProperty {
                object, property, ..
            } => object
                .binding_operand_count()
                .saturating_add(property.binding_operand_count()),
        }
    }

    fn nested_instruction_count(&self) -> usize {
        match self {
            Self::Binding(_) => 0,
            Self::StaticProperty { object, .. } | Self::ArrayIndexProperty { object, .. } => {
                object.instruction_count()
            }
            Self::ComputedProperty {
                object, property, ..
            } => object
                .instruction_count()
                .saturating_add(property.instruction_count()),
        }
    }
}

impl BytecodeSwitchCase {
    fn property_operand_count(&self) -> usize {
        self.test
            .as_ref()
            .map_or(0, BytecodeBlock::property_operand_count)
            .saturating_add(self.body.property_operand_count())
    }

    fn direct_native_call_count(&self) -> usize {
        self.test
            .as_ref()
            .map_or(0, BytecodeBlock::direct_native_call_count)
            .saturating_add(self.body.direct_native_call_count())
    }

    fn array_native_call_count(&self) -> usize {
        self.test
            .as_ref()
            .map_or(0, BytecodeBlock::array_native_call_count)
            .saturating_add(self.body.array_native_call_count())
    }

    fn numeric_instruction_count(&self) -> usize {
        self.test
            .as_ref()
            .map_or(0, BytecodeBlock::numeric_instruction_count)
            .saturating_add(self.body.numeric_instruction_count())
    }

    fn binding_operand_count(&self) -> usize {
        self.test
            .as_ref()
            .map_or(0, BytecodeBlock::binding_operand_count)
            .saturating_add(self.body.binding_operand_count())
    }

    fn instruction_count(&self) -> usize {
        self.test
            .as_ref()
            .map_or(0, BytecodeBlock::instruction_count)
            .saturating_add(self.body.instruction_count())
    }
}

impl BytecodeCatch {
    fn property_operand_count(&self) -> usize {
        self.body.property_operand_count()
    }

    fn direct_native_call_count(&self) -> usize {
        self.body.direct_native_call_count()
    }

    fn array_native_call_count(&self) -> usize {
        self.body.array_native_call_count()
    }

    fn numeric_instruction_count(&self) -> usize {
        self.body.numeric_instruction_count()
    }

    fn binding_operand_count(&self) -> usize {
        self.param
            .as_ref()
            .map_or(0, BytecodeBinding::direct_operand_count)
            .saturating_add(self.body.binding_operand_count())
    }

    fn instruction_count(&self) -> usize {
        self.body.instruction_count()
    }
}
