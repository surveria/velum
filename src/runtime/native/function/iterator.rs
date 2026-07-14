use crate::{
    error::Result,
    runtime::{Context, call::RuntimeCallArgs, native::builtins::IteratorConsumer},
    value::Value,
};

use super::{IteratorFunctionKind, NativeFunctionKind};

impl Context {
    pub(in crate::runtime) fn eval_iterator_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        let NativeFunctionKind::Iterator(kind) = kind else {
            return None;
        };
        let result = match kind {
            IteratorFunctionKind::Constructor => Self::eval_iterator_abstract_call(),
            IteratorFunctionKind::Concat => self.eval_iterator_concat(args),
            IteratorFunctionKind::From { .. } => self.eval_iterator_from(args),
            IteratorFunctionKind::Zip => self.eval_iterator_zip(args, false),
            IteratorFunctionKind::ZipKeyed => self.eval_iterator_zip(args, true),
            IteratorFunctionKind::PrototypeMap => {
                self.eval_iterator_prototype_map(args, this_value)
            }
            IteratorFunctionKind::PrototypeFilter => {
                self.eval_iterator_prototype_filter(args, this_value)
            }
            IteratorFunctionKind::PrototypeTake => {
                self.eval_iterator_prototype_take(args, this_value)
            }
            IteratorFunctionKind::PrototypeDrop => {
                self.eval_iterator_prototype_drop(args, this_value)
            }
            IteratorFunctionKind::PrototypeFlatMap => {
                self.eval_iterator_prototype_flat_map(args, this_value)
            }
            IteratorFunctionKind::PrototypeReduce => {
                self.eval_iterator_prototype_reduce(args, this_value)
            }
            IteratorFunctionKind::PrototypeToArray => {
                self.eval_iterator_prototype_to_array(this_value)
            }
            IteratorFunctionKind::PrototypeForEach => {
                self.eval_iterator_consumer(IteratorConsumer::ForEach, args, this_value)
            }
            IteratorFunctionKind::PrototypeSome => {
                self.eval_iterator_consumer(IteratorConsumer::Some, args, this_value)
            }
            IteratorFunctionKind::PrototypeEvery => {
                self.eval_iterator_consumer(IteratorConsumer::Every, args, this_value)
            }
            IteratorFunctionKind::PrototypeFind => {
                self.eval_iterator_consumer(IteratorConsumer::Find, args, this_value)
            }
            IteratorFunctionKind::PrototypeDispose => {
                self.eval_iterator_prototype_dispose(this_value)
            }
            IteratorFunctionKind::PrototypeConstructorGetter
            | IteratorFunctionKind::PrototypeToStringTagGetter => {
                self.eval_iterator_prototype_getter(kind)
            }
            IteratorFunctionKind::PrototypeConstructorSetter
            | IteratorFunctionKind::PrototypeToStringTagSetter => {
                self.eval_iterator_prototype_setter(kind, args, this_value)
            }
            IteratorFunctionKind::HelperNext(id) => self.eval_iterator_helper_next(id),
            IteratorFunctionKind::HelperReturn(id) => self.eval_iterator_helper_return(id),
            IteratorFunctionKind::StaticNext(id) => self.eval_iterator_static_next(id),
            IteratorFunctionKind::StaticReturn(id) => self.eval_iterator_static_return(id),
            IteratorFunctionKind::WrapNext(id) => self.eval_wrapped_iterator_next(id, this_value),
            IteratorFunctionKind::WrapReturn(id) => {
                self.eval_wrapped_iterator_return(id, this_value)
            }
        };
        Some(result)
    }
}
