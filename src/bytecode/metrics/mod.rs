use crate::{
    api::native_call::NativeCallTarget,
    binding_metadata::BindingOperand,
    bytecode::{BytecodeBinding, BytecodeBlock, BytecodeInstruction, BytecodeMetrics},
};

mod program;
mod targets;

impl BytecodeMetrics {
    pub const fn instruction_count(self) -> usize {
        self.instructions
    }

    pub const fn binding_operand_count(self) -> usize {
        self.binding_operands
    }

    pub const fn property_operand_count(self) -> usize {
        self.property_operands
    }

    pub const fn direct_native_call_count(self) -> usize {
        self.direct_native_calls
    }

    pub const fn array_native_call_count(self) -> usize {
        self.array_native_calls
    }

    pub const fn numeric_instruction_count(self) -> usize {
        self.numeric_instructions
    }

    pub const fn linear_peephole_candidate_count(self) -> usize {
        self.linear_peephole_candidates
    }

    pub const fn numeric_array_reduction_role_count(self) -> usize {
        self.numeric_array_reduction_roles
    }

    pub(super) const fn binding_operands(count: usize) -> Self {
        Self {
            binding_operands: count,
            ..Self::empty()
        }
    }

    pub(super) const fn property_operands(count: usize) -> Self {
        Self {
            property_operands: count,
            ..Self::empty()
        }
    }

    const fn linear_template(peephole_candidates: usize, reduction_roles: usize) -> Self {
        Self {
            linear_peephole_candidates: peephole_candidates,
            numeric_array_reduction_roles: reduction_roles,
            ..Self::empty()
        }
    }

    const fn numeric_instruction() -> Self {
        Self {
            numeric_instructions: 1,
            ..Self::empty()
        }
    }

    const fn native_call(target: Option<NativeCallTarget>) -> Self {
        Self {
            direct_native_calls: target.is_some() as usize,
            array_native_calls: match target {
                Some(target) => target.is_array_target() as usize,
                None => 0,
            },
            ..Self::empty()
        }
    }

    pub(super) const fn empty() -> Self {
        Self {
            instructions: 0,
            binding_operands: 0,
            property_operands: 0,
            direct_native_calls: 0,
            array_native_calls: 0,
            numeric_instructions: 0,
            linear_peephole_candidates: 0,
            numeric_array_reduction_roles: 0,
        }
    }

    pub(super) const fn add(&mut self, other: Self) {
        self.instructions = self.instructions.saturating_add(other.instructions);
        self.binding_operands = self.binding_operands.saturating_add(other.binding_operands);
        self.property_operands = self
            .property_operands
            .saturating_add(other.property_operands);
        self.direct_native_calls = self
            .direct_native_calls
            .saturating_add(other.direct_native_calls);
        self.array_native_calls = self
            .array_native_calls
            .saturating_add(other.array_native_calls);
        self.numeric_instructions = self
            .numeric_instructions
            .saturating_add(other.numeric_instructions);
        self.linear_peephole_candidates = self
            .linear_peephole_candidates
            .saturating_add(other.linear_peephole_candidates);
        self.numeric_array_reduction_roles = self
            .numeric_array_reduction_roles
            .saturating_add(other.numeric_array_reduction_roles);
    }

    pub(super) const fn combine(mut self, other: Self) -> Self {
        self.add(other);
        self
    }

    const fn with_instruction(mut self) -> Self {
        self.instructions = self.instructions.saturating_add(1);
        self
    }
}

impl BytecodeBlock {
    pub(super) fn metrics(&self) -> BytecodeMetrics {
        let mut metrics = BytecodeMetrics::linear_template(
            self.linear_template().peephole_candidate_count(),
            usize::from(self.linear_template().reduction_role().is_some()),
        );
        for instruction in self.instructions() {
            metrics.add(instruction.metrics());
        }
        metrics
    }
}

impl BytecodeInstruction {
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive instruction match prevents structural metric drift"
    )]
    fn metrics(&self) -> BytecodeMetrics {
        let metrics = match self {
            Self::LoadBinding(binding)
            | Self::StoreBinding(binding)
            | Self::ResolveBinding(binding)
            | Self::StoreResolvedBinding(binding)
            | Self::TypeOfBinding(binding)
            | Self::DeleteBinding(binding) => {
                BytecodeMetrics::binding_operands(binding.direct_operand_count())
            }
            Self::HoistLexicalBinding { name, .. }
            | Self::DeclareBinding { name, .. }
            | Self::UpdateBinding { name, .. }
            | Self::CompoundStoreBinding { name, .. } => {
                BytecodeMetrics::binding_operands(name.direct_operand_count())
            }
            Self::CallBindingSpread { callee, native, .. }
            | Self::CallBinding { callee, native, .. } => {
                BytecodeMetrics::binding_operands(callee.direct_operand_count())
                    .combine(BytecodeMetrics::native_call(*native))
            }
            Self::Construct {
                constructor,
                native,
                ..
            } => BytecodeMetrics::binding_operands(constructor.direct_operand_count())
                .combine(BytecodeMetrics::native_call(*native)),
            Self::ConstructValue { native, .. } => BytecodeMetrics::native_call(*native),
            Self::DeleteStaticProperty { .. }
            | Self::DeleteComputedProperty { .. }
            | Self::UpdateStaticProperty { .. }
            | Self::UpdateArrayIndexProperty { .. }
            | Self::UpdateComputedProperty { .. }
            | Self::InStaticProperty { .. }
            | Self::CompoundStaticProperty { .. }
            | Self::CompoundArrayIndexProperty { .. }
            | Self::CompoundComputedProperty { .. }
            | Self::StaticMember { .. }
            | Self::OptionalStaticMember { .. }
            | Self::ArrayLength { .. }
            | Self::ArrayIndexMember { .. }
            | Self::ComputedMember { .. }
            | Self::StaticPropertyAssign { .. }
            | Self::ArrayIndexAssign { .. }
            | Self::ComputedPropertyAssign { .. }
            | Self::CallStaticMemberSpread { .. }
            | Self::CallComputedMemberSpread { .. }
            | Self::SuperMember { .. }
            | Self::CallSuperMember { .. }
            | Self::CallSuperMemberSpread { .. }
            | Self::CallComputedSuperMember { .. }
            | Self::CallComputedSuperMemberSpread { .. } => BytecodeMetrics::property_operands(1),
            Self::Binary {
                property_access, ..
            } => BytecodeMetrics::property_operands(usize::from(property_access.is_some())),
            Self::CallStaticMember { native, .. } | Self::CallComputedMember { native, .. } => {
                BytecodeMetrics::property_operands(1).combine(BytecodeMetrics::native_call(*native))
            }
            Self::NumberUnary(_)
            | Self::NumberBinary(_)
            | Self::NumberCompare(_)
            | Self::NumberEquality(_) => BytecodeMetrics::numeric_instruction(),
            Self::NullishCoalescing { right } => right.metrics(),
            Self::DynamicImport {
                specifier, options, ..
            } => {
                let mut metrics = specifier.metrics();
                if let Some(options) = options {
                    metrics.add(options.metrics());
                }
                metrics
            }
            Self::LogicalAssignment { target, value, .. } => {
                target.metrics().combine(value.metrics())
            }
            Self::WebCompatCallAssignment { target } => target.metrics(),
            Self::CreateFunction { bytecode, .. } => bytecode.metrics(),
            Self::While {
                condition, body, ..
            }
            | Self::DoWhile {
                condition, body, ..
            } => condition.metrics().combine(body.metrics()),
            Self::With { body: block } | Self::Label { body: block, .. } => block.metrics(),
            Self::ScopedBlock {
                block,
                var_hoist_plan,
                ..
            } => {
                let mut metrics = block.metrics();
                if let Some(plan) = var_hoist_plan {
                    metrics.add(plan.metrics());
                }
                metrics
            }
            Self::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                let mut metrics = body.metrics();
                for block in [init.as_ref(), condition.as_ref(), update.as_ref()]
                    .into_iter()
                    .flatten()
                {
                    metrics.add(block.metrics());
                }
                metrics
            }
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
                .metrics()
                .combine(object.metrics())
                .combine(body.metrics()),
            Self::DestructurePattern { pattern, .. } => pattern.metrics(true),
            Self::CreateClass { class } => class.metrics(),
            Self::ComputedSuperMember { expression, .. } => expression
                .metrics()
                .combine(BytecodeMetrics::property_operands(1)),
            Self::SuperPropertyAssign {
                property, value, ..
            }
            | Self::CompoundSuperProperty {
                property, value, ..
            } => property.metrics().combine(value.metrics()),
            Self::UpdateSuperProperty { property, .. } => property.metrics(),
            Self::Switch {
                discriminant,
                cases,
                scope_init,
                ..
            } => {
                let mut metrics = discriminant.metrics();
                if let Some(scope_init) = scope_init {
                    metrics.add(scope_init.metrics());
                }
                for case in cases.iter() {
                    metrics.add(case.metrics());
                }
                metrics
            }
            Self::Try {
                body,
                catch,
                finally_body,
                ..
            } => {
                let mut metrics = body.metrics();
                if let Some(catch) = catch {
                    metrics.add(catch.metrics());
                }
                if let Some(finally_body) = finally_body {
                    metrics.add(finally_body.metrics());
                }
                metrics
            }
            Self::BeginPrivateEnvironment { .. }
            | Self::PushLiteral(_)
            | Self::PushString(_)
            | Self::TemplateConcat { .. }
            | Self::GetTemplateObject { .. }
            | Self::StringConcat { .. }
            | Self::StringConcatStatic { .. }
            | Self::CollectSpreadArgs { .. }
            | Self::CallValueSpread
            | Self::ConstructValueSpread
            | Self::ArrayLiteralSpread { .. }
            | Self::CreateRegExp { .. }
            | Self::PushUndefined
            | Self::LoadThis
            | Self::ImportMeta
            | Self::LoadNewTarget
            | Self::StoreAnnexBVar(_)
            | Self::StoreLast
            | Self::Pop
            | Self::Duplicate
            | Self::Unary(_)
            | Self::Await
            | Self::GeneratorStart
            | Self::Yield { .. }
            | Self::TypeOfValue
            | Self::ToPropertyKey
            | Self::DeleteSuperProperty
            | Self::DeleteValue
            | Self::PrivateMember { .. }
            | Self::PrivateAssign { .. }
            | Self::CompoundPrivateProperty { .. }
            | Self::UpdatePrivateProperty { .. }
            | Self::CallPrivateMember { .. }
            | Self::CallPrivateMemberSpread { .. }
            | Self::PrivateIn { .. }
            | Self::CallValue { .. }
            | Self::CallValueWithReceiver { .. }
            | Self::CallValueWithReceiverSpread
            | Self::ArrayLiteral { .. }
            | Self::ObjectLiteral { .. }
            | Self::CallSuper { .. }
            | Self::CallSuperSpread
            | Self::Jump(_)
            | Self::JumpIfFalse(_)
            | Self::JumpIfFalseKeep(_)
            | Self::JumpIfTrueKeep(_)
            | Self::JumpIfNullishKeep(_)
            | Self::TailCallBinding { .. }
            | Self::TailCallValue { .. }
            | Self::Complete(_) => BytecodeMetrics::empty(),
        };
        metrics.with_instruction()
    }
}

impl BytecodeBinding {
    pub(super) const fn direct_operand_count(&self) -> usize {
        !matches!(self.operand(), BindingOperand::Unresolved) as usize
    }
}
