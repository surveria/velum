use std::rc::Rc;

use crate::{error::Result, syntax::StaticBinding};

use super::{BytecodeBinding, BytecodeBlock, BytecodeHoistPlan, BytecodePattern};

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeFunction {
    self_binding: Option<StaticBinding>,
    arguments_binding: Option<StaticBinding>,
    params: Rc<[BytecodeFunctionParam]>,
    body: BytecodeBlock,
    hoist_plan: BytecodeHoistPlan,
    capture_bindings: Rc<[StaticBinding]>,
    uses_arguments: bool,
    strict: bool,
    pub(crate) simple_parameters: bool,
}

pub struct BytecodeFunctionInit {
    pub self_binding: Option<StaticBinding>,
    pub arguments_binding: Option<StaticBinding>,
    pub params: Rc<[BytecodeFunctionParam]>,
    pub body: BytecodeBlock,
    pub hoist_plan: BytecodeHoistPlan,
    pub capture_bindings: Rc<[StaticBinding]>,
    pub uses_arguments: bool,
    pub strict: bool,
    pub simple_parameters: bool,
}

impl BytecodeFunction {
    pub(crate) fn new(init: BytecodeFunctionInit) -> Self {
        Self {
            self_binding: init.self_binding,
            arguments_binding: init.arguments_binding,
            params: init.params,
            body: init.body,
            hoist_plan: init.hoist_plan,
            capture_bindings: init.capture_bindings,
            uses_arguments: init.uses_arguments,
            strict: init.strict,
            simple_parameters: init.simple_parameters,
        }
    }

    pub const fn self_binding(&self) -> Option<&StaticBinding> {
        self.self_binding.as_ref()
    }

    pub const fn arguments_binding(&self) -> Option<&StaticBinding> {
        self.arguments_binding.as_ref()
    }

    pub const fn uses_arguments(&self) -> bool {
        self.uses_arguments
    }

    pub const fn strict(&self) -> bool {
        self.strict
    }

    pub fn params(&self) -> &[BytecodeFunctionParam] {
        &self.params
    }

    pub fn requires_parameter_initialization(&self) -> bool {
        self.params
            .iter()
            .any(BytecodeFunctionParam::requires_runtime_initialization)
    }

    pub fn has_rest_parameter(&self) -> bool {
        self.params.last().is_some_and(BytecodeFunctionParam::rest)
    }

    pub(crate) fn has_unique_parameter_names(&self) -> bool {
        for (index, parameter) in self.params.iter().enumerate() {
            let Some(binding) = parameter.binding() else {
                return false;
            };
            if self
                .params
                .iter()
                .skip(index.saturating_add(1))
                .filter_map(BytecodeFunctionParam::binding)
                .any(|other| other.name().name() == binding.name().name())
            {
                return false;
            }
        }
        true
    }

    pub const fn body(&self) -> &BytecodeBlock {
        &self.body
    }

    pub const fn hoist_plan(&self) -> &BytecodeHoistPlan {
        &self.hoist_plan
    }

    pub fn capture_bindings(&self) -> &[StaticBinding] {
        &self.capture_bindings
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeFunctionParam {
    target: BytecodeFunctionParamTarget,
    default: Option<BytecodeBlock>,
    rest: bool,
}

impl BytecodeFunctionParam {
    pub(crate) const fn new(
        target: BytecodeFunctionParamTarget,
        default: Option<BytecodeBlock>,
        rest: bool,
    ) -> Self {
        Self {
            target,
            default,
            rest,
        }
    }

    const fn requires_runtime_initialization(&self) -> bool {
        self.has_default() || matches!(self.target(), BytecodeFunctionParamTarget::Pattern(_))
    }

    pub const fn target(&self) -> &BytecodeFunctionParamTarget {
        &self.target
    }

    pub const fn binding(&self) -> Option<&BytecodeBinding> {
        match &self.target {
            BytecodeFunctionParamTarget::Binding(binding) => Some(binding),
            BytecodeFunctionParamTarget::Pattern(_) => None,
        }
    }

    pub const fn default(&self) -> Option<&BytecodeBlock> {
        self.default.as_ref()
    }

    pub const fn has_default(&self) -> bool {
        self.default.is_some()
    }

    pub const fn rest(&self) -> bool {
        self.rest
    }

    pub(crate) fn for_each_binding(
        &self,
        visit: &mut impl FnMut(&BytecodeBinding) -> Result<()>,
    ) -> Result<()> {
        match &self.target {
            BytecodeFunctionParamTarget::Binding(binding) => visit(binding),
            BytecodeFunctionParamTarget::Pattern(pattern) => pattern.for_each_binding(visit),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeFunctionParamTarget {
    Binding(BytecodeBinding),
    Pattern(Rc<BytecodePattern>),
}
