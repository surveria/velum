use crate::bytecode::{
    BytecodeAssignmentTarget, BytecodeCatch, BytecodeClass, BytecodeForInTarget, BytecodeFunction,
    BytecodeFunctionDeclaration, BytecodeFunctionParamTarget, BytecodeHoistPlan, BytecodeMetrics,
    BytecodePattern, BytecodePatternKey, BytecodeSuperProperty, BytecodeSwitchCase,
};

impl BytecodeAssignmentTarget {
    pub(super) fn metrics(&self) -> BytecodeMetrics {
        match self {
            Self::Binding(binding) => {
                BytecodeMetrics::binding_operands(binding.direct_operand_count())
            }
            Self::WebCompatCall(target) => target.metrics(),
            Self::StaticProperty { object, .. }
            | Self::ArrayIndexProperty { object, .. }
            | Self::PrivateProperty { object, .. } => object
                .metrics()
                .combine(BytecodeMetrics::property_operands(1)),
            Self::ComputedProperty {
                object, property, ..
            } => object
                .metrics()
                .combine(property.metrics())
                .combine(BytecodeMetrics::property_operands(1)),
            Self::SuperProperty { property, .. } => property.metrics(),
        }
    }
}

impl BytecodeForInTarget {
    pub(super) fn metrics(&self) -> BytecodeMetrics {
        match self {
            Self::Binding { name, .. } => {
                BytecodeMetrics::binding_operands(name.direct_operand_count())
            }
            Self::PatternBinding { pattern, .. } | Self::PatternAssignment(pattern) => {
                pattern.metrics(true)
            }
            Self::Assignment(target) => target.metrics(),
        }
    }
}

impl BytecodeSwitchCase {
    pub(super) fn metrics(&self) -> BytecodeMetrics {
        let mut metrics = self.body.metrics();
        if let Some(test) = &self.test {
            metrics.add(test.metrics());
        }
        metrics
    }
}

impl BytecodeCatch {
    pub(super) fn metrics(&self) -> BytecodeMetrics {
        let mut metrics = self.body.metrics();
        if let Some(param) = &self.param {
            metrics.add(param.metrics(false));
        }
        for binding in self.param_bindings.iter() {
            metrics.add(BytecodeMetrics::binding_operands(
                binding.direct_operand_count(),
            ));
        }
        metrics
    }
}

impl BytecodeSuperProperty {
    pub(super) fn metrics(&self) -> BytecodeMetrics {
        match self {
            Self::Static(_) => BytecodeMetrics::property_operands(1),
            Self::Computed { expression, .. } => expression
                .metrics()
                .combine(BytecodeMetrics::property_operands(1)),
        }
    }
}

impl BytecodeFunction {
    pub(super) fn metrics(&self) -> BytecodeMetrics {
        let mut metrics = self.body().metrics();
        for param in self.params() {
            if let Some(default) = param.default() {
                metrics.add(default.metrics());
            }
            match param.target() {
                BytecodeFunctionParamTarget::Binding(binding) => metrics.add(
                    BytecodeMetrics::binding_operands(binding.direct_operand_count()),
                ),
                BytecodeFunctionParamTarget::Pattern(pattern) => {
                    metrics.add(pattern.metrics(true));
                }
            }
        }
        metrics.add(self.hoist_plan().metrics());
        metrics
    }
}

impl BytecodeFunctionDeclaration {
    fn metrics(&self) -> BytecodeMetrics {
        BytecodeMetrics::binding_operands(self.name().direct_operand_count())
            .combine(self.bytecode().metrics())
    }
}

impl BytecodeHoistPlan {
    pub(super) fn metrics(&self) -> BytecodeMetrics {
        let mut metrics = BytecodeMetrics::empty();
        for declaration in self.function_declarations() {
            metrics.add(declaration.metrics());
        }
        metrics
    }
}

impl BytecodeClass {
    pub(super) fn metrics(&self) -> BytecodeMetrics {
        let mut metrics = self.constructor.metrics();
        for member in self.members.iter() {
            metrics.add(member.bytecode.metrics());
        }
        for field in self.fields.iter() {
            if let Some(initializer) = &field.initializer {
                metrics.add(initializer.metrics());
            }
        }
        for block in self.static_blocks.iter() {
            metrics.add(block.metrics());
        }
        metrics
    }
}

impl BytecodePattern {
    pub(super) fn metrics(&self, include_binding_operands: bool) -> BytecodeMetrics {
        match self {
            Self::Binding(binding) => {
                if include_binding_operands {
                    BytecodeMetrics::binding_operands(binding.direct_operand_count())
                } else {
                    BytecodeMetrics::empty()
                }
            }
            Self::Assignment(target) => target.metrics(),
            Self::Object { properties, rest } => {
                let mut metrics = BytecodeMetrics::empty();
                for property in properties.iter() {
                    if let BytecodePatternKey::Computed(block) = &property.key {
                        metrics.add(block.metrics());
                    }
                    if let Some(default) = &property.target.default {
                        metrics.add(default.metrics());
                    }
                    metrics.add(property.target.pattern.metrics(include_binding_operands));
                }
                if let Some(rest) = rest {
                    metrics.add(rest.metrics(include_binding_operands));
                }
                metrics
            }
            Self::Array { elements, rest } => {
                let mut metrics = BytecodeMetrics::empty();
                for element in elements.iter().flatten() {
                    if let Some(default) = &element.default {
                        metrics.add(default.metrics());
                    }
                    metrics.add(element.pattern.metrics(include_binding_operands));
                }
                if let Some(rest) = rest {
                    metrics.add(rest.metrics(include_binding_operands));
                }
                metrics
            }
        }
    }
}
