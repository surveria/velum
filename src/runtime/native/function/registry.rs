use crate::{
    error::{Error, Result},
    value::{ErrorName, NativeFunctionId},
};

use super::{DateFunctionKind, NativeFunctionKind};

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
const OBJECT_GET_PROTOTYPE_OF_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(54);
const OBJECT_HAS_OWN_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(55);
const OBJECT_KEYS_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(56);
const STRING_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(57);
const ERROR_BASE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(58);
const ERROR_EVAL_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(59);
const ERROR_RANGE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(60);
const ERROR_REFERENCE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(61);
const ERROR_SYNTAX_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(62);
const ERROR_TEST262_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(63);
const ERROR_TYPE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(64);
const ERROR_URI_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(65);
const PROMISE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(66);
const PROMISE_RESOLVE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(67);
const PROMISE_REJECT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(68);
const PROMISE_THEN_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(69);
const PROMISE_CATCH_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(70);
const EVAL_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(71);
const SYMBOL_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(72);
const FUNCTION_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(73);
const ASYNC_FUNCTION_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(74);
const ARRAY_IS_ARRAY_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(75);
const REGEXP_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(76);
const OBJECT_ASSIGN_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(77);
const OBJECT_CREATE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(78);
const OBJECT_DEFINE_PROPERTIES_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(79);
const OBJECT_ENTRIES_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(80);
const OBJECT_GET_OWN_PROPERTY_DESCRIPTORS_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(81);
const OBJECT_IS_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(82);
const OBJECT_SET_PROTOTYPE_OF_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(83);
const OBJECT_VALUES_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(84);
const GLOBAL_DECODE_URI_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(85);
const GLOBAL_DECODE_URI_COMPONENT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(86);
const GLOBAL_ENCODE_URI_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(87);
const GLOBAL_ENCODE_URI_COMPONENT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(88);
const GLOBAL_IS_FINITE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(89);
const GLOBAL_IS_NAN_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(90);
const GLOBAL_PARSE_FLOAT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(91);
const GLOBAL_PARSE_INT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(92);
const NUMBER_IS_FINITE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(93);
const NUMBER_IS_NAN_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(94);
const STRING_PROTOTYPE_CHAR_AT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(95);
const STRING_PROTOTYPE_CHAR_CODE_AT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(96);
const STRING_PROTOTYPE_CONCAT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(97);
const STRING_PROTOTYPE_ENDS_WITH_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(98);
const STRING_PROTOTYPE_INCLUDES_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(99);
const STRING_PROTOTYPE_INDEX_OF_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(100);
const STRING_PROTOTYPE_LAST_INDEX_OF_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(101);
const STRING_PROTOTYPE_REPEAT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(102);
const STRING_PROTOTYPE_SLICE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(103);
const STRING_PROTOTYPE_STARTS_WITH_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(104);
const STRING_PROTOTYPE_SUBSTRING_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(105);
const STRING_PROTOTYPE_TO_LOWER_CASE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(106);
const STRING_PROTOTYPE_TO_UPPER_CASE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(107);
const STRING_PROTOTYPE_TRIM_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(108);
const STRING_PROTOTYPE_TRIM_END_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(109);
const STRING_PROTOTYPE_TRIM_START_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(110);
const ERROR_PROTOTYPE_TO_STRING_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(111);
const ERROR_AGGREGATE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(112);
const STRING_FROM_CHAR_CODE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(113);
const STRING_FROM_CODE_POINT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(114);
const STRING_RAW_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(115);
const STRING_PROTOTYPE_AT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(116);
const STRING_PROTOTYPE_CODE_POINT_AT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(117);
const STRING_PROTOTYPE_PAD_END_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(118);
const STRING_PROTOTYPE_PAD_START_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(119);
const STRING_PROTOTYPE_TO_LOCALE_LOWER_CASE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(120);
const STRING_PROTOTYPE_TO_LOCALE_UPPER_CASE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(121);
const STRING_PROTOTYPE_TO_STRING_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(122);
const STRING_PROTOTYPE_VALUE_OF_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(123);
const MAP_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(124);
const MAP_GET_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(125);
const MAP_SET_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(126);
const MAP_HAS_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(127);
const MAP_DELETE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(128);
const MAP_CLEAR_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(129);
const MAP_FOR_EACH_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(130);
const MAP_SIZE_GETTER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(131);
const MAP_ENTRIES_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(132);
const MAP_KEYS_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(133);
const MAP_VALUES_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(134);
const SET_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(135);
const SET_ADD_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(136);
const SET_HAS_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(137);
const SET_DELETE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(138);
const SET_CLEAR_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(139);
const SET_FOR_EACH_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(140);
const SET_SIZE_GETTER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(141);
const SET_ENTRIES_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(142);
const SET_VALUES_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(143);
const NUMBER_IS_INTEGER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(144);
const NUMBER_IS_SAFE_INTEGER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(145);
const BOOLEAN_PROTOTYPE_TO_STRING_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(146);
const BOOLEAN_PROTOTYPE_VALUE_OF_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(147);
const NUMBER_PROTOTYPE_TO_LOCALE_STRING_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(148);
const NUMBER_PROTOTYPE_TO_STRING_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(149);
const NUMBER_PROTOTYPE_VALUE_OF_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(150);
const SYMBOL_PROTOTYPE_DESCRIPTION_GETTER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(151);
const SYMBOL_PROTOTYPE_TO_STRING_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(152);
const SYMBOL_PROTOTYPE_VALUE_OF_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(153);
const ARRAY_EVERY_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(154);
const ARRAY_FILTER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(155);
const ARRAY_FIND_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(156);
const ARRAY_FIND_INDEX_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(157);
const ARRAY_FOR_EACH_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(158);
const ARRAY_REDUCE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(159);
const ARRAY_REDUCE_RIGHT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(160);
const ARRAY_SOME_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(161);
const ARRAY_MAP_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(162);
const WEAK_MAP_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(163);
const WEAK_MAP_GET_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(164);
const WEAK_MAP_SET_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(165);
const WEAK_MAP_HAS_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(166);
const WEAK_MAP_DELETE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(167);
const WEAK_SET_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(168);
const WEAK_SET_ADD_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(169);
const WEAK_SET_HAS_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(170);
const WEAK_SET_DELETE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(171);
const DATE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(172);
const ARRAY_FLAT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(173);
const ARRAY_FLAT_MAP_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(174);
const OBJECT_FREEZE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(175);
const OBJECT_IS_EXTENSIBLE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(176);
const OBJECT_IS_FROZEN_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(177);
const OBJECT_IS_SEALED_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(178);
const OBJECT_PREVENT_EXTENSIONS_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(179);
const OBJECT_SEAL_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(180);
const MATH_F16ROUND_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(181);
const MATH_SUM_PRECISE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(182);
const REGEXP_EXEC_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(183);
const JSON_IS_RAW_JSON_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(184);
const JSON_RAW_JSON_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(185);
const REGEXP_DOT_ALL_GETTER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(186);
const REGEXP_FLAGS_GETTER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(187);
const REGEXP_GLOBAL_GETTER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(188);
const REGEXP_HAS_INDICES_GETTER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(189);
const REGEXP_IGNORE_CASE_GETTER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(190);
const REGEXP_MULTILINE_GETTER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(191);
const REGEXP_SOURCE_GETTER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(192);
const REGEXP_STICKY_GETTER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(193);
const REGEXP_UNICODE_GETTER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(194);
const REGEXP_UNICODE_SETS_GETTER_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(195);
const REGEXP_TO_STRING_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(196);
const REGEXP_SYMBOL_MATCH_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(197);
const REGEXP_SYMBOL_SEARCH_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(198);
const REGEXP_SYMBOL_REPLACE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(199);
const REGEXP_SYMBOL_SPLIT_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(200);
const REGEXP_SYMBOL_MATCH_ALL_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(201);
const SYMBOL_FOR_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(202);
const SYMBOL_KEY_FOR_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(203);
const NATIVE_FUNCTION_SLOT_COUNT: usize = 204;

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
    if let Some(slot) = array_slot(kind) {
        return Some(slot);
    }
    if let Some(slot) = utility_slot(kind) {
        return Some(slot);
    }
    if let Some(slot) = primitive_prototype_slot(kind) {
        return Some(slot);
    }
    if let Some(slot) = string_static_slot(kind) {
        return Some(slot);
    }
    if let Some(slot) = string_prototype_slot(kind) {
        return Some(slot);
    }
    if let Some(slot) = regexp_slot(kind) {
        return Some(slot);
    }

    match kind {
        NativeFunctionKind::AsyncFunction => Some(ASYNC_FUNCTION_SLOT),
        NativeFunctionKind::Boolean => Some(BOOLEAN_SLOT),
        NativeFunctionKind::Eval => Some(EVAL_SLOT),
        NativeFunctionKind::ErrorConstructor(name) => Some(error_constructor_slot(name)),
        NativeFunctionKind::ErrorPrototypeToString => Some(ERROR_PROTOTYPE_TO_STRING_SLOT),
        NativeFunctionKind::Function => Some(FUNCTION_SLOT),
        NativeFunctionKind::JsonIsRawJson => Some(JSON_IS_RAW_JSON_SLOT),
        NativeFunctionKind::JsonParse => Some(JSON_PARSE_SLOT),
        NativeFunctionKind::JsonRawJson => Some(JSON_RAW_JSON_SLOT),
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
        NativeFunctionKind::MathF16round => Some(MATH_F16ROUND_SLOT),
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
        NativeFunctionKind::MathSumPrecise => Some(MATH_SUM_PRECISE_SLOT),
        NativeFunctionKind::MathTan => Some(MATH_TAN_SLOT),
        NativeFunctionKind::MathTanh => Some(MATH_TANH_SLOT),
        NativeFunctionKind::MathTrunc => Some(MATH_TRUNC_SLOT),
        NativeFunctionKind::Number => Some(NUMBER_SLOT),
        NativeFunctionKind::Object => Some(OBJECT_SLOT),
        NativeFunctionKind::ObjectAssign => Some(OBJECT_ASSIGN_SLOT),
        NativeFunctionKind::ObjectCreate => Some(OBJECT_CREATE_SLOT),
        NativeFunctionKind::ObjectDefineProperties => Some(OBJECT_DEFINE_PROPERTIES_SLOT),
        NativeFunctionKind::ObjectDefineProperty => Some(OBJECT_DEFINE_PROPERTY_SLOT),
        NativeFunctionKind::ObjectEntries => Some(OBJECT_ENTRIES_SLOT),
        NativeFunctionKind::ObjectFreeze => Some(OBJECT_FREEZE_SLOT),
        NativeFunctionKind::ObjectGetPrototypeOf => Some(OBJECT_GET_PROTOTYPE_OF_SLOT),
        NativeFunctionKind::ObjectGetOwnPropertyDescriptor => {
            Some(OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_SLOT)
        }
        NativeFunctionKind::ObjectGetOwnPropertyDescriptors => {
            Some(OBJECT_GET_OWN_PROPERTY_DESCRIPTORS_SLOT)
        }
        NativeFunctionKind::ObjectHasOwn => Some(OBJECT_HAS_OWN_SLOT),
        NativeFunctionKind::ObjectIs => Some(OBJECT_IS_SLOT),
        NativeFunctionKind::ObjectIsExtensible => Some(OBJECT_IS_EXTENSIBLE_SLOT),
        NativeFunctionKind::ObjectIsFrozen => Some(OBJECT_IS_FROZEN_SLOT),
        NativeFunctionKind::ObjectIsSealed => Some(OBJECT_IS_SEALED_SLOT),
        NativeFunctionKind::ObjectKeys => Some(OBJECT_KEYS_SLOT),
        NativeFunctionKind::ObjectPreventExtensions => Some(OBJECT_PREVENT_EXTENSIONS_SLOT),
        NativeFunctionKind::ObjectSetPrototypeOf => Some(OBJECT_SET_PROTOTYPE_OF_SLOT),
        NativeFunctionKind::ObjectSeal => Some(OBJECT_SEAL_SLOT),
        NativeFunctionKind::ObjectValues => Some(OBJECT_VALUES_SLOT),
        NativeFunctionKind::Promise => Some(PROMISE_SLOT),
        NativeFunctionKind::PromiseResolve => Some(PROMISE_RESOLVE_SLOT),
        NativeFunctionKind::PromiseReject => Some(PROMISE_REJECT_SLOT),
        NativeFunctionKind::PromiseThen => Some(PROMISE_THEN_SLOT),
        NativeFunctionKind::PromiseCatch => Some(PROMISE_CATCH_SLOT),
        NativeFunctionKind::String => Some(STRING_SLOT),
        NativeFunctionKind::Symbol => Some(SYMBOL_SLOT),
        _ => collection_slot(kind),
    }
}

const fn regexp_slot(kind: NativeFunctionKind) -> Option<NativeFunctionSlot> {
    match kind {
        NativeFunctionKind::RegExp => Some(REGEXP_SLOT),
        NativeFunctionKind::RegExpPrototypeDotAllGetter => Some(REGEXP_DOT_ALL_GETTER_SLOT),
        NativeFunctionKind::RegExpPrototypeExec => Some(REGEXP_EXEC_SLOT),
        NativeFunctionKind::RegExpPrototypeFlagsGetter => Some(REGEXP_FLAGS_GETTER_SLOT),
        NativeFunctionKind::RegExpPrototypeGlobalGetter => Some(REGEXP_GLOBAL_GETTER_SLOT),
        NativeFunctionKind::RegExpPrototypeHasIndicesGetter => Some(REGEXP_HAS_INDICES_GETTER_SLOT),
        NativeFunctionKind::RegExpPrototypeIgnoreCaseGetter => Some(REGEXP_IGNORE_CASE_GETTER_SLOT),
        NativeFunctionKind::RegExpPrototypeMultilineGetter => Some(REGEXP_MULTILINE_GETTER_SLOT),
        NativeFunctionKind::RegExpPrototypeSourceGetter => Some(REGEXP_SOURCE_GETTER_SLOT),
        NativeFunctionKind::RegExpPrototypeStickyGetter => Some(REGEXP_STICKY_GETTER_SLOT),
        NativeFunctionKind::RegExpPrototypeToString => Some(REGEXP_TO_STRING_SLOT),
        NativeFunctionKind::RegExpPrototypeUnicodeGetter => Some(REGEXP_UNICODE_GETTER_SLOT),
        NativeFunctionKind::RegExpPrototypeUnicodeSetsGetter => {
            Some(REGEXP_UNICODE_SETS_GETTER_SLOT)
        }
        NativeFunctionKind::RegExpPrototypeSymbolMatch => Some(REGEXP_SYMBOL_MATCH_SLOT),
        NativeFunctionKind::RegExpPrototypeSymbolMatchAll => Some(REGEXP_SYMBOL_MATCH_ALL_SLOT),
        NativeFunctionKind::RegExpPrototypeSymbolReplace => Some(REGEXP_SYMBOL_REPLACE_SLOT),
        NativeFunctionKind::RegExpPrototypeSymbolSearch => Some(REGEXP_SYMBOL_SEARCH_SLOT),
        NativeFunctionKind::RegExpPrototypeSymbolSplit => Some(REGEXP_SYMBOL_SPLIT_SLOT),
        _ => None,
    }
}

const fn collection_slot(kind: NativeFunctionKind) -> Option<NativeFunctionSlot> {
    match kind {
        NativeFunctionKind::Date(DateFunctionKind::Constructor) => Some(DATE_SLOT),
        NativeFunctionKind::Map => Some(MAP_SLOT),
        NativeFunctionKind::MapGet => Some(MAP_GET_SLOT),
        NativeFunctionKind::MapSet => Some(MAP_SET_SLOT),
        NativeFunctionKind::MapHas => Some(MAP_HAS_SLOT),
        NativeFunctionKind::MapDelete => Some(MAP_DELETE_SLOT),
        NativeFunctionKind::MapClear => Some(MAP_CLEAR_SLOT),
        NativeFunctionKind::MapForEach => Some(MAP_FOR_EACH_SLOT),
        NativeFunctionKind::MapSizeGetter => Some(MAP_SIZE_GETTER_SLOT),
        NativeFunctionKind::MapEntries => Some(MAP_ENTRIES_SLOT),
        NativeFunctionKind::MapKeys => Some(MAP_KEYS_SLOT),
        NativeFunctionKind::MapValues => Some(MAP_VALUES_SLOT),
        NativeFunctionKind::Set => Some(SET_SLOT),
        NativeFunctionKind::SetAdd => Some(SET_ADD_SLOT),
        NativeFunctionKind::SetHas => Some(SET_HAS_SLOT),
        NativeFunctionKind::SetDelete => Some(SET_DELETE_SLOT),
        NativeFunctionKind::SetClear => Some(SET_CLEAR_SLOT),
        NativeFunctionKind::SetForEach => Some(SET_FOR_EACH_SLOT),
        NativeFunctionKind::SetSizeGetter => Some(SET_SIZE_GETTER_SLOT),
        NativeFunctionKind::SetEntries => Some(SET_ENTRIES_SLOT),
        NativeFunctionKind::SetValues => Some(SET_VALUES_SLOT),
        NativeFunctionKind::WeakMap => Some(WEAK_MAP_SLOT),
        NativeFunctionKind::WeakMapGet => Some(WEAK_MAP_GET_SLOT),
        NativeFunctionKind::WeakMapSet => Some(WEAK_MAP_SET_SLOT),
        NativeFunctionKind::WeakMapHas => Some(WEAK_MAP_HAS_SLOT),
        NativeFunctionKind::WeakMapDelete => Some(WEAK_MAP_DELETE_SLOT),
        NativeFunctionKind::WeakSet => Some(WEAK_SET_SLOT),
        NativeFunctionKind::WeakSetAdd => Some(WEAK_SET_ADD_SLOT),
        NativeFunctionKind::WeakSetHas => Some(WEAK_SET_HAS_SLOT),
        NativeFunctionKind::WeakSetDelete => Some(WEAK_SET_DELETE_SLOT),
        _ => None,
    }
}

const fn array_slot(kind: NativeFunctionKind) -> Option<NativeFunctionSlot> {
    match kind {
        NativeFunctionKind::Array => Some(ARRAY_SLOT),
        NativeFunctionKind::ArrayConcat => Some(ARRAY_CONCAT_SLOT),
        NativeFunctionKind::ArrayEvery => Some(ARRAY_EVERY_SLOT),
        NativeFunctionKind::ArrayFilter => Some(ARRAY_FILTER_SLOT),
        NativeFunctionKind::ArrayFind => Some(ARRAY_FIND_SLOT),
        NativeFunctionKind::ArrayFindIndex => Some(ARRAY_FIND_INDEX_SLOT),
        NativeFunctionKind::ArrayFlat => Some(ARRAY_FLAT_SLOT),
        NativeFunctionKind::ArrayFlatMap => Some(ARRAY_FLAT_MAP_SLOT),
        NativeFunctionKind::ArrayForEach => Some(ARRAY_FOR_EACH_SLOT),
        NativeFunctionKind::ArrayIncludes => Some(ARRAY_INCLUDES_SLOT),
        NativeFunctionKind::ArrayIndexOf => Some(ARRAY_INDEX_OF_SLOT),
        NativeFunctionKind::ArrayIsArray => Some(ARRAY_IS_ARRAY_SLOT),
        NativeFunctionKind::ArrayJoin => Some(ARRAY_JOIN_SLOT),
        NativeFunctionKind::ArrayLastIndexOf => Some(ARRAY_LAST_INDEX_OF_SLOT),
        NativeFunctionKind::ArrayMap => Some(ARRAY_MAP_SLOT),
        NativeFunctionKind::ArrayPop => Some(ARRAY_POP_SLOT),
        NativeFunctionKind::ArrayPush => Some(ARRAY_PUSH_SLOT),
        NativeFunctionKind::ArrayReduce => Some(ARRAY_REDUCE_SLOT),
        NativeFunctionKind::ArrayReduceRight => Some(ARRAY_REDUCE_RIGHT_SLOT),
        NativeFunctionKind::ArrayReverse => Some(ARRAY_REVERSE_SLOT),
        NativeFunctionKind::ArrayShift => Some(ARRAY_SHIFT_SLOT),
        NativeFunctionKind::ArraySlice => Some(ARRAY_SLICE_SLOT),
        NativeFunctionKind::ArraySome => Some(ARRAY_SOME_SLOT),
        NativeFunctionKind::ArrayUnshift => Some(ARRAY_UNSHIFT_SLOT),
        _ => None,
    }
}

const fn utility_slot(kind: NativeFunctionKind) -> Option<NativeFunctionSlot> {
    match kind {
        NativeFunctionKind::GlobalDecodeUri => Some(GLOBAL_DECODE_URI_SLOT),
        NativeFunctionKind::GlobalDecodeUriComponent => Some(GLOBAL_DECODE_URI_COMPONENT_SLOT),
        NativeFunctionKind::GlobalEncodeUri => Some(GLOBAL_ENCODE_URI_SLOT),
        NativeFunctionKind::GlobalEncodeUriComponent => Some(GLOBAL_ENCODE_URI_COMPONENT_SLOT),
        NativeFunctionKind::GlobalIsFinite => Some(GLOBAL_IS_FINITE_SLOT),
        NativeFunctionKind::GlobalIsNan => Some(GLOBAL_IS_NAN_SLOT),
        NativeFunctionKind::GlobalParseFloat => Some(GLOBAL_PARSE_FLOAT_SLOT),
        NativeFunctionKind::GlobalParseInt => Some(GLOBAL_PARSE_INT_SLOT),
        NativeFunctionKind::NumberIsFinite => Some(NUMBER_IS_FINITE_SLOT),
        NativeFunctionKind::NumberIsInteger => Some(NUMBER_IS_INTEGER_SLOT),
        NativeFunctionKind::NumberIsNan => Some(NUMBER_IS_NAN_SLOT),
        NativeFunctionKind::NumberIsSafeInteger => Some(NUMBER_IS_SAFE_INTEGER_SLOT),
        _ => None,
    }
}

const fn primitive_prototype_slot(kind: NativeFunctionKind) -> Option<NativeFunctionSlot> {
    match kind {
        NativeFunctionKind::BooleanPrototypeToString => Some(BOOLEAN_PROTOTYPE_TO_STRING_SLOT),
        NativeFunctionKind::BooleanPrototypeValueOf => Some(BOOLEAN_PROTOTYPE_VALUE_OF_SLOT),
        NativeFunctionKind::NumberPrototypeToLocaleString => {
            Some(NUMBER_PROTOTYPE_TO_LOCALE_STRING_SLOT)
        }
        NativeFunctionKind::NumberPrototypeToString => Some(NUMBER_PROTOTYPE_TO_STRING_SLOT),
        NativeFunctionKind::NumberPrototypeValueOf => Some(NUMBER_PROTOTYPE_VALUE_OF_SLOT),
        NativeFunctionKind::SymbolPrototypeDescriptionGetter => {
            Some(SYMBOL_PROTOTYPE_DESCRIPTION_GETTER_SLOT)
        }
        NativeFunctionKind::SymbolPrototypeToString => Some(SYMBOL_PROTOTYPE_TO_STRING_SLOT),
        NativeFunctionKind::SymbolPrototypeValueOf => Some(SYMBOL_PROTOTYPE_VALUE_OF_SLOT),
        NativeFunctionKind::SymbolFor => Some(SYMBOL_FOR_SLOT),
        NativeFunctionKind::SymbolKeyFor => Some(SYMBOL_KEY_FOR_SLOT),
        _ => None,
    }
}

const fn string_static_slot(kind: NativeFunctionKind) -> Option<NativeFunctionSlot> {
    match kind {
        NativeFunctionKind::StringFromCharCode => Some(STRING_FROM_CHAR_CODE_SLOT),
        NativeFunctionKind::StringFromCodePoint => Some(STRING_FROM_CODE_POINT_SLOT),
        NativeFunctionKind::StringRaw => Some(STRING_RAW_SLOT),
        _ => None,
    }
}

const fn string_prototype_slot(kind: NativeFunctionKind) -> Option<NativeFunctionSlot> {
    match kind {
        NativeFunctionKind::StringPrototypeAt => Some(STRING_PROTOTYPE_AT_SLOT),
        NativeFunctionKind::StringPrototypeCharAt => Some(STRING_PROTOTYPE_CHAR_AT_SLOT),
        NativeFunctionKind::StringPrototypeCharCodeAt => Some(STRING_PROTOTYPE_CHAR_CODE_AT_SLOT),
        NativeFunctionKind::StringPrototypeCodePointAt => Some(STRING_PROTOTYPE_CODE_POINT_AT_SLOT),
        NativeFunctionKind::StringPrototypeConcat => Some(STRING_PROTOTYPE_CONCAT_SLOT),
        NativeFunctionKind::StringPrototypeEndsWith => Some(STRING_PROTOTYPE_ENDS_WITH_SLOT),
        NativeFunctionKind::StringPrototypeIncludes => Some(STRING_PROTOTYPE_INCLUDES_SLOT),
        NativeFunctionKind::StringPrototypeIndexOf => Some(STRING_PROTOTYPE_INDEX_OF_SLOT),
        NativeFunctionKind::StringPrototypeLastIndexOf => Some(STRING_PROTOTYPE_LAST_INDEX_OF_SLOT),
        NativeFunctionKind::StringPrototypePadEnd => Some(STRING_PROTOTYPE_PAD_END_SLOT),
        NativeFunctionKind::StringPrototypePadStart => Some(STRING_PROTOTYPE_PAD_START_SLOT),
        NativeFunctionKind::StringPrototypeRepeat => Some(STRING_PROTOTYPE_REPEAT_SLOT),
        NativeFunctionKind::StringPrototypeSlice => Some(STRING_PROTOTYPE_SLICE_SLOT),
        NativeFunctionKind::StringPrototypeStartsWith => Some(STRING_PROTOTYPE_STARTS_WITH_SLOT),
        NativeFunctionKind::StringPrototypeSubstring => Some(STRING_PROTOTYPE_SUBSTRING_SLOT),
        NativeFunctionKind::StringPrototypeToLocaleLowerCase => {
            Some(STRING_PROTOTYPE_TO_LOCALE_LOWER_CASE_SLOT)
        }
        NativeFunctionKind::StringPrototypeToLocaleUpperCase => {
            Some(STRING_PROTOTYPE_TO_LOCALE_UPPER_CASE_SLOT)
        }
        NativeFunctionKind::StringPrototypeToLowerCase => Some(STRING_PROTOTYPE_TO_LOWER_CASE_SLOT),
        NativeFunctionKind::StringPrototypeToString => Some(STRING_PROTOTYPE_TO_STRING_SLOT),
        NativeFunctionKind::StringPrototypeToUpperCase => Some(STRING_PROTOTYPE_TO_UPPER_CASE_SLOT),
        NativeFunctionKind::StringPrototypeTrim => Some(STRING_PROTOTYPE_TRIM_SLOT),
        NativeFunctionKind::StringPrototypeTrimEnd => Some(STRING_PROTOTYPE_TRIM_END_SLOT),
        NativeFunctionKind::StringPrototypeTrimStart => Some(STRING_PROTOTYPE_TRIM_START_SLOT),
        NativeFunctionKind::StringPrototypeValueOf => Some(STRING_PROTOTYPE_VALUE_OF_SLOT),
        _ => None,
    }
}

const fn error_constructor_slot(name: ErrorName) -> NativeFunctionSlot {
    match name {
        ErrorName::AggregateError => ERROR_AGGREGATE_SLOT,
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
