use crate::{
    runtime::function::{FunctionIntrinsicDefaults, FunctionProperties},
    runtime::object::{
        DataPropertyDescriptor, PropertyConfigurable, PropertyEnumerable, PropertyWritable,
    },
    value::Value,
};

use super::{NativeFunctionKind, number_intrinsic_property};

#[derive(Debug, Clone)]
pub(in crate::runtime) struct NativeFunction {
    kind: NativeFunctionKind,
    properties: FunctionProperties,
}

impl NativeFunction {
    pub(in crate::runtime::native) fn new(
        kind: NativeFunctionKind,
        prototype: Value,
        name: Value,
    ) -> Self {
        let prototype_default = DataPropertyDescriptor::new(
            prototype.clone(),
            PropertyWritable::No,
            PropertyEnumerable::No,
            PropertyConfigurable::No,
        );
        let intrinsic_defaults = FunctionIntrinsicDefaults::new(
            Value::Number(kind.length()),
            name,
            Some(prototype_default),
        );
        Self {
            kind,
            properties: FunctionProperties::new(prototype, intrinsic_defaults),
        }
    }

    pub(in crate::runtime) const fn kind(&self) -> NativeFunctionKind {
        self.kind
    }

    pub(in crate::runtime) const fn properties(&self) -> &FunctionProperties {
        &self.properties
    }

    pub(in crate::runtime) const fn properties_mut(&mut self) -> &mut FunctionProperties {
        &mut self.properties
    }

    pub(in crate::runtime) fn intrinsic_property(&self, property: &str) -> Option<Value> {
        match self.kind {
            NativeFunctionKind::Number => number_intrinsic_property(property),
            NativeFunctionKind::Array
            | NativeFunctionKind::ArrayConcat
            | NativeFunctionKind::ArrayIncludes
            | NativeFunctionKind::ArrayIndexOf
            | NativeFunctionKind::ArrayJoin
            | NativeFunctionKind::ArrayLastIndexOf
            | NativeFunctionKind::ArrayPop
            | NativeFunctionKind::ArrayPush
            | NativeFunctionKind::ArrayReverse
            | NativeFunctionKind::ArrayShift
            | NativeFunctionKind::ArraySlice
            | NativeFunctionKind::ArrayUnshift
            | NativeFunctionKind::Boolean
            | NativeFunctionKind::Eval
            | NativeFunctionKind::ErrorConstructor(_)
            | NativeFunctionKind::Function
            | NativeFunctionKind::JsonParse
            | NativeFunctionKind::JsonStringify
            | NativeFunctionKind::MathAbs
            | NativeFunctionKind::MathAcos
            | NativeFunctionKind::MathAcosh
            | NativeFunctionKind::MathAsin
            | NativeFunctionKind::MathAsinh
            | NativeFunctionKind::MathAtan
            | NativeFunctionKind::MathAtan2
            | NativeFunctionKind::MathAtanh
            | NativeFunctionKind::MathCbrt
            | NativeFunctionKind::MathCeil
            | NativeFunctionKind::MathClz32
            | NativeFunctionKind::MathCos
            | NativeFunctionKind::MathCosh
            | NativeFunctionKind::MathExp
            | NativeFunctionKind::MathExpm1
            | NativeFunctionKind::MathFloor
            | NativeFunctionKind::MathFround
            | NativeFunctionKind::MathHypot
            | NativeFunctionKind::MathImul
            | NativeFunctionKind::MathLog
            | NativeFunctionKind::MathLog10
            | NativeFunctionKind::MathLog1p
            | NativeFunctionKind::MathLog2
            | NativeFunctionKind::MathMax
            | NativeFunctionKind::MathMin
            | NativeFunctionKind::MathPow
            | NativeFunctionKind::MathRandom
            | NativeFunctionKind::MathRound
            | NativeFunctionKind::MathSign
            | NativeFunctionKind::MathSin
            | NativeFunctionKind::MathSinh
            | NativeFunctionKind::MathSqrt
            | NativeFunctionKind::MathTan
            | NativeFunctionKind::MathTanh
            | NativeFunctionKind::MathTrunc
            | NativeFunctionKind::Object
            | NativeFunctionKind::ObjectDefineProperty
            | NativeFunctionKind::ObjectGetOwnPropertyDescriptor
            | NativeFunctionKind::ObjectHasOwn
            | NativeFunctionKind::ObjectKeys
            | NativeFunctionKind::Promise
            | NativeFunctionKind::PromiseResolve
            | NativeFunctionKind::PromiseReject
            | NativeFunctionKind::PromiseThen
            | NativeFunctionKind::PromiseCatch
            | NativeFunctionKind::PromiseResolver { .. }
            | NativeFunctionKind::String
            | NativeFunctionKind::Symbol => None,
        }
    }

    pub(in crate::runtime) fn has_intrinsic_property(&self, property: &str) -> bool {
        self.intrinsic_property(property).is_some()
    }
}
