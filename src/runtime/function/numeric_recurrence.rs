use alloc::boxed::Box;

use crate::{
    bytecode::BytecodeNumericBinaryOp,
    error::Result,
    runtime::{Context, bytecode::apply_number_binary},
};

use super::fast_path::{FastValueSource, FunctionFastPath, FunctionFastPathKind};

#[derive(Debug)]
pub(in crate::runtime) struct NumericUnaryFunctionFastPath {
    expression: NumericUnaryExpression,
    step_count: usize,
}

#[derive(Debug)]
enum NumericUnaryExpression {
    Param,
    Literal(f64),
    Binary {
        op: BytecodeNumericBinaryOp,
        left: Box<Self>,
        right: Box<Self>,
    },
}

impl NumericUnaryFunctionFastPath {
    fn compile(fast_path: &FunctionFastPath) -> Option<Self> {
        let FunctionFastPathKind::ReturnNumberBinary { op, left, right } = &fast_path.kind else {
            return None;
        };
        let expression = NumericUnaryExpression::Binary {
            op: *op,
            left: Box::new(NumericUnaryExpression::compile(left)?),
            right: Box::new(NumericUnaryExpression::compile(right)?),
        };
        expression.uses_param().then_some(Self {
            expression,
            step_count: fast_path.step_count,
        })
    }

    pub(in crate::runtime) const fn step_count(&self) -> usize {
        self.step_count
    }

    pub(in crate::runtime) fn evaluate(&self, input: f64) -> Result<f64> {
        self.expression.evaluate(input)
    }
}

impl NumericUnaryExpression {
    fn compile(source: &FastValueSource) -> Option<Self> {
        match source {
            FastValueSource::Param(0) => Some(Self::Param),
            FastValueSource::Literal(crate::value::Value::Number(value)) => {
                Some(Self::Literal(*value))
            }
            FastValueSource::NumberBinary { op, left, right } => Some(Self::Binary {
                op: *op,
                left: Box::new(Self::compile(left)?),
                right: Box::new(Self::compile(right)?),
            }),
            FastValueSource::Param(_)
            | FastValueSource::Binding(_)
            | FastValueSource::Literal(_) => None,
        }
    }

    const fn uses_param(&self) -> bool {
        match self {
            Self::Param => true,
            Self::Literal(_) => false,
            Self::Binary { left, right, .. } => left.uses_param() || right.uses_param(),
        }
    }

    fn evaluate(&self, input: f64) -> Result<f64> {
        match self {
            Self::Param => Ok(input),
            Self::Literal(value) => Ok(*value),
            Self::Binary { op, left, right } => {
                apply_number_binary(*op, left.evaluate(input)?, right.evaluate(input)?)
            }
        }
    }
}

impl Context {
    pub(in crate::runtime) fn bind_numeric_unary_function_fast_path(
        &self,
        function: crate::value::FunctionId,
    ) -> Result<Option<NumericUnaryFunctionFastPath>> {
        let record = self.function(function)?;
        if record.realm != self.active_realm_index()
            || !record.upvalues.is_empty()
            || !record.dynamic_environments.is_empty()
        {
            return Ok(None);
        }
        Ok(record
            .fast_path
            .as_deref()
            .and_then(NumericUnaryFunctionFastPath::compile))
    }
}
