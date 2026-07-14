mod address;
mod block;
mod call_site;
mod completion;
mod fast_path;
mod function;
mod function_mode;
mod hoist;
mod linear_template;
mod metrics;
mod numeric;
mod private;
mod super_property;
mod template;
mod types;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct BytecodeMetrics {
    instructions: usize,
    binding_operands: usize,
    property_operands: usize,
    direct_native_calls: usize,
    array_native_calls: usize,
    numeric_instructions: usize,
    linear_peephole_candidates: usize,
    numeric_array_reduction_roles: usize,
}

pub use address::BytecodeAddress;
pub use block::BytecodeBlock;
pub use call_site::BytecodeCallSite;
pub use completion::BytecodeCompletion;
pub use fast_path::BytecodeDirectThrow;
pub use function::{
    BytecodeFunction, BytecodeFunctionInit, BytecodeFunctionParam, BytecodeFunctionParamTarget,
};
pub use function_mode::BytecodeNewTargetMode;
pub use hoist::BytecodeHoistPlan;
pub use linear_template::{
    BytecodeLinearPeepholeKind, BytecodeLinearTemplate, BytecodeNumericArrayReductionRole,
};
pub use numeric::{
    BytecodeNumericBinaryOp, BytecodeNumericCompareOp, BytecodeNumericEqualityOp,
    BytecodeNumericUnaryOp,
};
pub use private::{BytecodeClassMemberKey, BytecodePrivateName};
pub use super_property::BytecodeSuperProperty;
pub use template::BytecodeTemplateElement;
pub use types::{
    BytecodeArrayIndex, BytecodeAssignmentTarget, BytecodeBinding, BytecodeCatch, BytecodeClass,
    BytecodeClassField, BytecodeClassMember, BytecodeClassMemberKind, BytecodeClassStaticElement,
    BytecodeDestructureMode, BytecodeDynamicProperty, BytecodeForInTarget,
    BytecodeFunctionDeclaration, BytecodeInstruction, BytecodeObjectProperty, BytecodePattern,
    BytecodePatternKey, BytecodePatternProperty, BytecodePatternTarget, BytecodePreparedNativeCall,
    BytecodeProgram, BytecodeProperty, BytecodeSwitchCase,
};
