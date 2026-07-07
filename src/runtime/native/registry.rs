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
const PROMISE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(65);
const PROMISE_RESOLVE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(66);
const PROMISE_REJECT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(67);
const PROMISE_THEN_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(68);
const PROMISE_CATCH_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(69);
const EVAL_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(70);
const SYMBOL_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(71);
const NATIVE_FUNCTION_SLOT_COUNT: usize = 72;

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
        self.slots.get(slot(kind)?.index()).copied().flatten()
    }

    pub(in crate::runtime) fn insert(
        &mut self,
        kind: NativeFunctionKind,
        id: NativeFunctionId,
    ) -> Result<()> {
        let Some(slot) = slot(kind) else {
            return Ok(());
        };
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

const fn slot(kind: NativeFunctionKind) -> Option<NativeFunctionSlot> {
    match kind {
        NativeFunctionKind::Array => Some(ARRAY_SLOT),
        NativeFunctionKind::ArrayConcat => Some(ARRAY_CONCAT_SLOT),
        NativeFunctionKind::ArrayIncludes => Some(ARRAY_INCLUDES_SLOT),
        NativeFunctionKind::ArrayIndexOf => Some(ARRAY_INDEX_OF_SLOT),
        NativeFunctionKind::ArrayJoin => Some(ARRAY_JOIN_SLOT),
        NativeFunctionKind::ArrayLastIndexOf => Some(ARRAY_LAST_INDEX_OF_SLOT),
        NativeFunctionKind::ArrayPop => Some(ARRAY_POP_SLOT),
        NativeFunctionKind::ArrayPush => Some(ARRAY_PUSH_SLOT),
        NativeFunctionKind::ArrayReverse => Some(ARRAY_REVERSE_SLOT),
        NativeFunctionKind::ArrayShift => Some(ARRAY_SHIFT_SLOT),
        NativeFunctionKind::ArraySlice => Some(ARRAY_SLICE_SLOT),
        NativeFunctionKind::ArrayUnshift => Some(ARRAY_UNSHIFT_SLOT),
        NativeFunctionKind::Boolean => Some(BOOLEAN_SLOT),
        NativeFunctionKind::Eval => Some(EVAL_SLOT),
        NativeFunctionKind::ErrorConstructor(name) => Some(error_constructor_slot(name)),
        NativeFunctionKind::JsonParse => Some(JSON_PARSE_SLOT),
        NativeFunctionKind::JsonStringify => Some(JSON_STRINGIFY_SLOT),
        NativeFunctionKind::MathAbs => Some(MATH_ABS_SLOT),
        NativeFunctionKind::MathAcos => Some(MATH_ACOS_SLOT),
        NativeFunctionKind::MathAcosh => Some(MATH_ACOSH_SLOT),
        NativeFunctionKind::MathAsin => Some(MATH_ASIN_SLOT),
        NativeFunctionKind::MathAsinh => Some(MATH_ASINH_SLOT),
        NativeFunctionKind::MathAtan => Some(MATH_ATAN_SLOT),
        NativeFunctionKind::MathAtan2 => Some(MATH_ATAN2_SLOT),
        NativeFunctionKind::MathAtanh => Some(MATH_ATANH_SLOT),
        NativeFunctionKind::MathCbrt => Some(MATH_CBRT_SLOT),
        NativeFunctionKind::MathCeil => Some(MATH_CEIL_SLOT),
        NativeFunctionKind::MathClz32 => Some(MATH_CLZ32_SLOT),
        NativeFunctionKind::MathCos => Some(MATH_COS_SLOT),
        NativeFunctionKind::MathCosh => Some(MATH_COSH_SLOT),
        NativeFunctionKind::MathExp => Some(MATH_EXP_SLOT),
        NativeFunctionKind::MathExpm1 => Some(MATH_EXPM1_SLOT),
        NativeFunctionKind::MathFloor => Some(MATH_FLOOR_SLOT),
        NativeFunctionKind::MathFround => Some(MATH_FROUND_SLOT),
        NativeFunctionKind::MathHypot => Some(MATH_HYPOT_SLOT),
        NativeFunctionKind::MathImul => Some(MATH_IMUL_SLOT),
        NativeFunctionKind::MathLog => Some(MATH_LOG_SLOT),
        NativeFunctionKind::MathLog10 => Some(MATH_LOG10_SLOT),
        NativeFunctionKind::MathLog1p => Some(MATH_LOG1P_SLOT),
        NativeFunctionKind::MathLog2 => Some(MATH_LOG2_SLOT),
        NativeFunctionKind::MathMax => Some(MATH_MAX_SLOT),
        NativeFunctionKind::MathMin => Some(MATH_MIN_SLOT),
        NativeFunctionKind::MathPow => Some(MATH_POW_SLOT),
        NativeFunctionKind::MathRandom => Some(MATH_RANDOM_SLOT),
        NativeFunctionKind::MathRound => Some(MATH_ROUND_SLOT),
        NativeFunctionKind::MathSign => Some(MATH_SIGN_SLOT),
        NativeFunctionKind::MathSin => Some(MATH_SIN_SLOT),
        NativeFunctionKind::MathSinh => Some(MATH_SINH_SLOT),
        NativeFunctionKind::MathSqrt => Some(MATH_SQRT_SLOT),
        NativeFunctionKind::MathTan => Some(MATH_TAN_SLOT),
        NativeFunctionKind::MathTanh => Some(MATH_TANH_SLOT),
        NativeFunctionKind::MathTrunc => Some(MATH_TRUNC_SLOT),
        NativeFunctionKind::Number => Some(NUMBER_SLOT),
        NativeFunctionKind::Object => Some(OBJECT_SLOT),
        NativeFunctionKind::ObjectDefineProperty => Some(OBJECT_DEFINE_PROPERTY_SLOT),
        NativeFunctionKind::ObjectGetOwnPropertyDescriptor => {
            Some(OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_SLOT)
        }
        NativeFunctionKind::ObjectHasOwn => Some(OBJECT_HAS_OWN_SLOT),
        NativeFunctionKind::ObjectKeys => Some(OBJECT_KEYS_SLOT),
        NativeFunctionKind::Promise => Some(PROMISE_SLOT),
        NativeFunctionKind::PromiseResolve => Some(PROMISE_RESOLVE_SLOT),
        NativeFunctionKind::PromiseReject => Some(PROMISE_REJECT_SLOT),
        NativeFunctionKind::PromiseThen => Some(PROMISE_THEN_SLOT),
        NativeFunctionKind::PromiseCatch => Some(PROMISE_CATCH_SLOT),
        NativeFunctionKind::PromiseResolver { .. } => None,
        NativeFunctionKind::String => Some(STRING_SLOT),
        NativeFunctionKind::Symbol => Some(SYMBOL_SLOT),
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
