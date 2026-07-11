use super::{
    MATH_ABS_NAME, MATH_ACOS_NAME, MATH_ACOSH_NAME, MATH_ASIN_NAME, MATH_ASINH_NAME,
    MATH_ATAN_NAME, MATH_ATAN2_NAME, MATH_ATANH_NAME, MATH_CBRT_NAME, MATH_CEIL_NAME,
    MATH_CLZ32_NAME, MATH_COS_NAME, MATH_COSH_NAME, MATH_EXP_NAME, MATH_EXPM1_NAME,
    MATH_F16ROUND_NAME, MATH_FLOOR_NAME, MATH_FROUND_NAME, MATH_FUNCTION_LENGTH_ONE,
    MATH_FUNCTION_LENGTH_TWO, MATH_FUNCTION_LENGTH_ZERO, MATH_HYPOT_NAME, MATH_IMUL_NAME,
    MATH_LOG_NAME, MATH_LOG1P_NAME, MATH_LOG2_NAME, MATH_LOG10_NAME, MATH_MAX_NAME, MATH_MIN_NAME,
    MATH_POW_NAME, MATH_RANDOM_NAME, MATH_ROUND_NAME, MATH_SIGN_NAME, MATH_SIN_NAME,
    MATH_SINH_NAME, MATH_SQRT_NAME, MATH_SUM_PRECISE_NAME, MATH_TAN_NAME, MATH_TANH_NAME,
    MATH_TRUNC_NAME, NativeFunctionKind,
};

impl NativeFunctionKind {
    pub(super) const fn math_length(self) -> Option<f64> {
        match self {
            Self::MathRandom => Some(MATH_FUNCTION_LENGTH_ZERO),
            Self::MathAbs
            | Self::MathAcos
            | Self::MathAcosh
            | Self::MathAsin
            | Self::MathAsinh
            | Self::MathAtan
            | Self::MathAtanh
            | Self::MathCbrt
            | Self::MathCeil
            | Self::MathClz32
            | Self::MathCos
            | Self::MathCosh
            | Self::MathExp
            | Self::MathExpm1
            | Self::MathF16round
            | Self::MathFloor
            | Self::MathFround
            | Self::MathLog
            | Self::MathLog10
            | Self::MathLog1p
            | Self::MathLog2
            | Self::MathRound
            | Self::MathSign
            | Self::MathSin
            | Self::MathSinh
            | Self::MathSqrt
            | Self::MathSumPrecise
            | Self::MathTan
            | Self::MathTanh
            | Self::MathTrunc => Some(MATH_FUNCTION_LENGTH_ONE),
            Self::MathAtan2
            | Self::MathHypot
            | Self::MathImul
            | Self::MathMax
            | Self::MathMin
            | Self::MathPow => Some(MATH_FUNCTION_LENGTH_TWO),
            _ => None,
        }
    }

    pub(super) const fn math_name(self) -> Option<&'static str> {
        match self {
            Self::MathAbs => Some(MATH_ABS_NAME),
            Self::MathAcos => Some(MATH_ACOS_NAME),
            Self::MathAcosh => Some(MATH_ACOSH_NAME),
            Self::MathAsin => Some(MATH_ASIN_NAME),
            Self::MathAsinh => Some(MATH_ASINH_NAME),
            Self::MathAtan => Some(MATH_ATAN_NAME),
            Self::MathAtan2 => Some(MATH_ATAN2_NAME),
            Self::MathAtanh => Some(MATH_ATANH_NAME),
            Self::MathCbrt => Some(MATH_CBRT_NAME),
            Self::MathCeil => Some(MATH_CEIL_NAME),
            Self::MathClz32 => Some(MATH_CLZ32_NAME),
            Self::MathCos => Some(MATH_COS_NAME),
            Self::MathCosh => Some(MATH_COSH_NAME),
            Self::MathExp => Some(MATH_EXP_NAME),
            Self::MathExpm1 => Some(MATH_EXPM1_NAME),
            Self::MathF16round => Some(MATH_F16ROUND_NAME),
            Self::MathFloor => Some(MATH_FLOOR_NAME),
            Self::MathFround => Some(MATH_FROUND_NAME),
            Self::MathHypot => Some(MATH_HYPOT_NAME),
            Self::MathImul => Some(MATH_IMUL_NAME),
            Self::MathLog => Some(MATH_LOG_NAME),
            Self::MathLog10 => Some(MATH_LOG10_NAME),
            Self::MathLog1p => Some(MATH_LOG1P_NAME),
            Self::MathLog2 => Some(MATH_LOG2_NAME),
            Self::MathMax => Some(MATH_MAX_NAME),
            Self::MathMin => Some(MATH_MIN_NAME),
            Self::MathPow => Some(MATH_POW_NAME),
            Self::MathRandom => Some(MATH_RANDOM_NAME),
            Self::MathRound => Some(MATH_ROUND_NAME),
            Self::MathSign => Some(MATH_SIGN_NAME),
            Self::MathSin => Some(MATH_SIN_NAME),
            Self::MathSinh => Some(MATH_SINH_NAME),
            Self::MathSqrt => Some(MATH_SQRT_NAME),
            Self::MathSumPrecise => Some(MATH_SUM_PRECISE_NAME),
            Self::MathTan => Some(MATH_TAN_NAME),
            Self::MathTanh => Some(MATH_TANH_NAME),
            Self::MathTrunc => Some(MATH_TRUNC_NAME),
            _ => None,
        }
    }
}
