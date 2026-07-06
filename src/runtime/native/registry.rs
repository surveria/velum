use crate::{
    error::{Error, Result},
    value::{ErrorName, NativeFunctionId},
};

use super::NativeFunctionKind;

const ARRAY_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(0);
const ARRAY_CONCAT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(1);
const ARRAY_INCLUDES_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(2);
const ARRAY_INDEX_OF_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(3);
const ARRAY_JOIN_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(4);
const ARRAY_LAST_INDEX_OF_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(5);
const ARRAY_POP_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(6);
const ARRAY_PUSH_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(7);
const ARRAY_REVERSE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(8);
const ARRAY_SHIFT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(9);
const ARRAY_SLICE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(10);
const ARRAY_UNSHIFT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(11);
const BOOLEAN_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(12);
const JSON_PARSE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(13);
const JSON_STRINGIFY_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(14);
const MATH_ABS_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(15);
const MATH_ACOS_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(16);
const MATH_ACOSH_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(17);
const MATH_ASIN_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(18);
const MATH_ASINH_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(19);
const MATH_ATAN_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(20);
const MATH_ATAN2_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(21);
const MATH_ATANH_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(22);
const MATH_CBRT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(23);
const MATH_CEIL_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(24);
const MATH_CLZ32_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(25);
const MATH_COS_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(26);
const MATH_COSH_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(27);
const MATH_EXP_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(28);
const MATH_EXPM1_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(29);
const MATH_FLOOR_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(30);
const MATH_FROUND_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(31);
const MATH_HYPOT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(32);
const MATH_IMUL_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(33);
const MATH_LOG_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(34);
const MATH_LOG10_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(35);
const MATH_LOG1P_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(36);
const MATH_LOG2_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(37);
const MATH_MAX_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(38);
const MATH_MIN_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(39);
const MATH_POW_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(40);
const MATH_RANDOM_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(41);
const MATH_ROUND_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(42);
const MATH_SIGN_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(43);
const MATH_SIN_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(44);
const MATH_SINH_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(45);
const MATH_SQRT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(46);
const MATH_TAN_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(47);
const MATH_TANH_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(48);
const MATH_TRUNC_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(49);
const NUMBER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(50);
const OBJECT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(51);
const OBJECT_DEFINE_PROPERTY_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(52);
const OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(53);
const OBJECT_HAS_OWN_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(54);
const OBJECT_KEYS_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(55);
const STRING_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(56);
const ERROR_BASE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(57);
const ERROR_EVAL_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(58);
const ERROR_RANGE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(59);
const ERROR_REFERENCE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(60);
const ERROR_SYNTAX_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(61);
const ERROR_TEST262_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(62);
const ERROR_TYPE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(63);
const ERROR_URI_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(64);
const NATIVE_FUNCTION_SLOT_COUNT: usize = 65;

#[derive(Debug, Clone)]
pub(in crate::runtime) struct NativeFunctionRegistry {
    slots: [Option<NativeFunctionId>; NATIVE_FUNCTION_SLOT_COUNT],
}

impl NativeFunctionRegistry {
    pub(in crate::runtime) const fn new() -> Self {
        Self {
            slots: [None; NATIVE_FUNCTION_SLOT_COUNT],
        }
    }

    pub(in crate::runtime) fn get(&self, kind: NativeFunctionKind) -> Option<NativeFunctionId> {
        self.slots.get(slot(kind).index()).copied().flatten()
    }

    pub(in crate::runtime) fn insert(
        &mut self,
        kind: NativeFunctionKind,
        id: NativeFunctionId,
    ) -> Result<()> {
        let slot = slot(kind);
        let entry = self
            .slots
            .get_mut(slot.index())
            .ok_or_else(|| Error::runtime("native function registry slot is not defined"))?;
        if let Some(existing) = *entry {
            if existing == id {
                return Ok(());
            }
            return Err(Error::runtime("native function kind is already registered"));
        }
        *entry = Some(id);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct NativeFunctionSlot(usize);

impl NativeFunctionSlot {
    const fn new(index: usize) -> Self {
        Self(index)
    }

    const fn index(self) -> usize {
        self.0
    }
}

const fn slot(kind: NativeFunctionKind) -> NativeFunctionSlot {
    match kind {
        NativeFunctionKind::Array => ARRAY_SLOT,
        NativeFunctionKind::ArrayConcat => ARRAY_CONCAT_SLOT,
        NativeFunctionKind::ArrayIncludes => ARRAY_INCLUDES_SLOT,
        NativeFunctionKind::ArrayIndexOf => ARRAY_INDEX_OF_SLOT,
        NativeFunctionKind::ArrayJoin => ARRAY_JOIN_SLOT,
        NativeFunctionKind::ArrayLastIndexOf => ARRAY_LAST_INDEX_OF_SLOT,
        NativeFunctionKind::ArrayPop => ARRAY_POP_SLOT,
        NativeFunctionKind::ArrayPush => ARRAY_PUSH_SLOT,
        NativeFunctionKind::ArrayReverse => ARRAY_REVERSE_SLOT,
        NativeFunctionKind::ArrayShift => ARRAY_SHIFT_SLOT,
        NativeFunctionKind::ArraySlice => ARRAY_SLICE_SLOT,
        NativeFunctionKind::ArrayUnshift => ARRAY_UNSHIFT_SLOT,
        NativeFunctionKind::Boolean => BOOLEAN_SLOT,
        NativeFunctionKind::ErrorConstructor(name) => error_constructor_slot(name),
        NativeFunctionKind::JsonParse => JSON_PARSE_SLOT,
        NativeFunctionKind::JsonStringify => JSON_STRINGIFY_SLOT,
        NativeFunctionKind::MathAbs => MATH_ABS_SLOT,
        NativeFunctionKind::MathAcos => MATH_ACOS_SLOT,
        NativeFunctionKind::MathAcosh => MATH_ACOSH_SLOT,
        NativeFunctionKind::MathAsin => MATH_ASIN_SLOT,
        NativeFunctionKind::MathAsinh => MATH_ASINH_SLOT,
        NativeFunctionKind::MathAtan => MATH_ATAN_SLOT,
        NativeFunctionKind::MathAtan2 => MATH_ATAN2_SLOT,
        NativeFunctionKind::MathAtanh => MATH_ATANH_SLOT,
        NativeFunctionKind::MathCbrt => MATH_CBRT_SLOT,
        NativeFunctionKind::MathCeil => MATH_CEIL_SLOT,
        NativeFunctionKind::MathClz32 => MATH_CLZ32_SLOT,
        NativeFunctionKind::MathCos => MATH_COS_SLOT,
        NativeFunctionKind::MathCosh => MATH_COSH_SLOT,
        NativeFunctionKind::MathExp => MATH_EXP_SLOT,
        NativeFunctionKind::MathExpm1 => MATH_EXPM1_SLOT,
        NativeFunctionKind::MathFloor => MATH_FLOOR_SLOT,
        NativeFunctionKind::MathFround => MATH_FROUND_SLOT,
        NativeFunctionKind::MathHypot => MATH_HYPOT_SLOT,
        NativeFunctionKind::MathImul => MATH_IMUL_SLOT,
        NativeFunctionKind::MathLog => MATH_LOG_SLOT,
        NativeFunctionKind::MathLog10 => MATH_LOG10_SLOT,
        NativeFunctionKind::MathLog1p => MATH_LOG1P_SLOT,
        NativeFunctionKind::MathLog2 => MATH_LOG2_SLOT,
        NativeFunctionKind::MathMax => MATH_MAX_SLOT,
        NativeFunctionKind::MathMin => MATH_MIN_SLOT,
        NativeFunctionKind::MathPow => MATH_POW_SLOT,
        NativeFunctionKind::MathRandom => MATH_RANDOM_SLOT,
        NativeFunctionKind::MathRound => MATH_ROUND_SLOT,
        NativeFunctionKind::MathSign => MATH_SIGN_SLOT,
        NativeFunctionKind::MathSin => MATH_SIN_SLOT,
        NativeFunctionKind::MathSinh => MATH_SINH_SLOT,
        NativeFunctionKind::MathSqrt => MATH_SQRT_SLOT,
        NativeFunctionKind::MathTan => MATH_TAN_SLOT,
        NativeFunctionKind::MathTanh => MATH_TANH_SLOT,
        NativeFunctionKind::MathTrunc => MATH_TRUNC_SLOT,
        NativeFunctionKind::Number => NUMBER_SLOT,
        NativeFunctionKind::Object => OBJECT_SLOT,
        NativeFunctionKind::ObjectDefineProperty => OBJECT_DEFINE_PROPERTY_SLOT,
        NativeFunctionKind::ObjectGetOwnPropertyDescriptor => {
            OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_SLOT
        }
        NativeFunctionKind::ObjectHasOwn => OBJECT_HAS_OWN_SLOT,
        NativeFunctionKind::ObjectKeys => OBJECT_KEYS_SLOT,
        NativeFunctionKind::String => STRING_SLOT,
    }
}

const fn error_constructor_slot(name: ErrorName) -> NativeFunctionSlot {
    match name {
        ErrorName::Base => ERROR_BASE_SLOT,
        ErrorName::EvalError => ERROR_EVAL_SLOT,
        ErrorName::RangeError => ERROR_RANGE_SLOT,
        ErrorName::ReferenceError => ERROR_REFERENCE_SLOT,
        ErrorName::SyntaxError => ERROR_SYNTAX_SLOT,
        ErrorName::Test262Error => ERROR_TEST262_SLOT,
        ErrorName::TypeError => ERROR_TYPE_SLOT,
        ErrorName::UriError => ERROR_URI_SLOT,
    }
}
