use crate::{
    api::native_call::NativeCallTarget,
    error::Result,
    runtime::{Context, call::RuntimeCallArgs, roots::VmRootKind},
    value::Value,
};

use super::NativeFunctionKind;

impl Context {
    pub(in crate::runtime) fn eval_direct_array_native_call_target(
        &mut self,
        target: NativeCallTarget,
        args: &[Value],
        this_value: &Value,
    ) -> Option<Result<Value>> {
        let boxed_receiver = if direct_array_receiver_requires_to_object(target)
            && !matches!(
                this_value,
                Value::Object(_)
                    | Value::Function(_)
                    | Value::NativeFunction(_)
                    | Value::HostFunction(_)
            ) {
            match self.object_to_object(this_value) {
                Ok(receiver) => Some(receiver),
                Err(error) => return Some(Err(error)),
            }
        } else {
            None
        };
        let _receiver_roots = if let Some(receiver) = boxed_receiver.as_ref() {
            let roots = match self.active_transient_root_scope(VmRootKind::TransientTemporary) {
                Ok(roots) => roots,
                Err(error) => return Some(Err(error)),
            };
            if let Err(error) = roots.add_values(std::iter::once(receiver)) {
                return Some(Err(error));
            }
            Some(roots)
        } else {
            None
        };
        let this_value = boxed_receiver.as_ref().unwrap_or(this_value);
        match target {
            NativeCallTarget::Array => Some(self.eval_direct_array_constructor(args)),
            NativeCallTarget::ArrayConcat => Some(self.eval_direct_array_concat(args, this_value)),
            NativeCallTarget::ArrayEvery => {
                Some(self.eval_direct_array_every(args, this_value, false))
            }
            NativeCallTarget::ArrayFilter => {
                Some(self.eval_direct_array_filter(args, this_value, false))
            }
            NativeCallTarget::ArrayFind => Some(self.eval_direct_array_find(args, this_value)),
            NativeCallTarget::ArrayFindIndex => {
                Some(self.eval_direct_array_find_index(args, this_value))
            }
            NativeCallTarget::ArrayFlat => Some(self.eval_direct_array_flat(args, this_value)),
            NativeCallTarget::ArrayFlatMap => {
                Some(self.eval_direct_array_flat_map(args, this_value))
            }
            NativeCallTarget::ArrayForEach => {
                Some(self.eval_direct_array_for_each(args, this_value, false))
            }
            NativeCallTarget::ArrayIncludes => {
                Some(self.eval_direct_array_includes(args, this_value))
            }
            NativeCallTarget::ArrayIndexOf => {
                Some(self.eval_direct_array_index_of(args, this_value))
            }
            NativeCallTarget::ArrayIsArray => Some(self.eval_direct_array_is_array(args)),
            NativeCallTarget::ArrayJoin => Some(self.eval_direct_array_join(args, this_value)),
            NativeCallTarget::ArrayLastIndexOf => {
                Some(self.eval_direct_array_last_index_of(args, this_value))
            }
            NativeCallTarget::ArrayMap => Some(self.eval_direct_array_map(args, this_value, false)),
            NativeCallTarget::ArrayPop => Some(self.eval_direct_array_pop(args, this_value)),
            NativeCallTarget::ArrayPush => Some(self.eval_direct_array_push(args, this_value)),
            NativeCallTarget::ArrayReduce => Some(self.eval_direct_array_reduce(args, this_value)),
            NativeCallTarget::ArrayReduceRight => {
                Some(self.eval_direct_array_reduce_right(args, this_value))
            }
            NativeCallTarget::ArrayReverse => {
                Some(self.eval_direct_array_reverse(args, this_value))
            }
            NativeCallTarget::ArrayShift => Some(self.eval_direct_array_shift(args, this_value)),
            NativeCallTarget::ArraySlice => Some(self.eval_direct_array_slice(args, this_value)),
            NativeCallTarget::ArraySome => {
                Some(self.eval_direct_array_some(args, this_value, false))
            }
            NativeCallTarget::ArrayUnshift => {
                Some(self.eval_direct_array_unshift(args, this_value))
            }
            _ => None,
        }
    }

    pub(in crate::runtime) fn eval_array_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        let boxed_receiver = if kind.array_receiver_requires_to_object()
            && !matches!(
                this_value,
                Value::Object(_)
                    | Value::Function(_)
                    | Value::NativeFunction(_)
                    | Value::HostFunction(_)
            ) {
            match self.object_to_object(this_value) {
                Ok(receiver) => Some(receiver),
                Err(error) => return Some(Err(error)),
            }
        } else {
            None
        };
        let _receiver_roots = if let Some(receiver) = boxed_receiver.as_ref() {
            let roots = match self.active_transient_root_scope(VmRootKind::TransientTemporary) {
                Ok(roots) => roots,
                Err(error) => return Some(Err(error)),
            };
            if let Err(error) = roots.add_values(std::iter::once(receiver)) {
                return Some(Err(error));
            }
            Some(roots)
        } else {
            None
        };
        let this_value = boxed_receiver.as_ref().unwrap_or(this_value);
        match kind {
            NativeFunctionKind::Array => Some(self.eval_array_constructor(args)),
            NativeFunctionKind::ArrayConcat => Some(self.eval_array_concat(args, this_value)),
            NativeFunctionKind::ArrayEvery => Some(self.eval_array_every(args, this_value)),
            NativeFunctionKind::ArrayFilter => Some(self.eval_array_filter(args, this_value)),
            NativeFunctionKind::ArrayFind => Some(self.eval_array_find(args, this_value)),
            NativeFunctionKind::ArrayFindIndex => {
                Some(self.eval_array_find_index(args, this_value))
            }
            NativeFunctionKind::ArrayFlat => Some(self.eval_array_flat(args, this_value)),
            NativeFunctionKind::ArrayFlatMap => Some(self.eval_array_flat_map(args, this_value)),
            NativeFunctionKind::ArrayForEach => Some(self.eval_array_for_each(args, this_value)),
            NativeFunctionKind::ArrayFrom => Some(self.eval_array_from(args, this_value)),
            NativeFunctionKind::ArrayFromAsync => {
                Some(self.eval_array_from_async(args, this_value))
            }
            NativeFunctionKind::ArrayIncludes => Some(self.eval_array_includes(args, this_value)),
            NativeFunctionKind::ArrayIndexOf => Some(self.eval_array_index_of(args, this_value)),
            NativeFunctionKind::ArrayIsArray => Some(self.eval_array_is_array(args)),
            NativeFunctionKind::ArrayJoin => Some(self.eval_array_join(args, this_value)),
            NativeFunctionKind::ArrayToString => Some(self.eval_array_to_string(args, this_value)),
            NativeFunctionKind::ArrayLastIndexOf => {
                Some(self.eval_array_last_index_of(args, this_value))
            }
            NativeFunctionKind::ArrayMap => Some(self.eval_array_map(args, this_value)),
            NativeFunctionKind::ArrayPop => Some(self.eval_array_pop(args, this_value)),
            NativeFunctionKind::ArrayPush => Some(self.eval_array_push(args, this_value)),
            NativeFunctionKind::ArrayReduce => Some(self.eval_array_reduce(args, this_value)),
            NativeFunctionKind::ArrayReduceRight => {
                Some(self.eval_array_reduce_right(args, this_value))
            }
            NativeFunctionKind::ArrayReverse => Some(self.eval_array_reverse(args, this_value)),
            NativeFunctionKind::ArrayShift => Some(self.eval_array_shift(args, this_value)),
            NativeFunctionKind::ArraySlice => Some(self.eval_array_slice(args, this_value)),
            NativeFunctionKind::ArraySome => Some(self.eval_array_some(args, this_value)),
            NativeFunctionKind::ArrayUnshift => Some(self.eval_array_unshift(args, this_value)),
            NativeFunctionKind::ArraySort => Some(self.eval_array_sort(args, this_value)),
            NativeFunctionKind::ArraySplice => Some(self.eval_array_splice(args, this_value)),
            NativeFunctionKind::ArrayFill => Some(self.eval_array_fill(args, this_value)),
            NativeFunctionKind::ArrayCopyWithin => {
                Some(self.eval_array_copy_within(args, this_value))
            }
            NativeFunctionKind::ArrayAt => Some(self.eval_array_at(args, this_value)),
            NativeFunctionKind::ArrayFindLast => Some(self.eval_array_find_last(args, this_value)),
            NativeFunctionKind::ArrayFindLastIndex => {
                Some(self.eval_array_find_last_index(args, this_value))
            }
            NativeFunctionKind::ArrayToSorted => Some(self.eval_array_to_sorted(args, this_value)),
            NativeFunctionKind::ArrayToReversed => {
                Some(self.eval_array_to_reversed(args, this_value))
            }
            NativeFunctionKind::ArrayToSpliced => {
                Some(self.eval_array_to_spliced(args, this_value))
            }
            NativeFunctionKind::ArrayWith => Some(self.eval_array_with(args, this_value)),
            NativeFunctionKind::ArrayEntries => Some(self.eval_array_entries(this_value)),
            NativeFunctionKind::ArrayKeys => Some(self.eval_array_keys(this_value)),
            NativeFunctionKind::ArrayValues => Some(self.eval_array_values(this_value)),
            NativeFunctionKind::ArrayIteratorNext => {
                Some(self.eval_array_iterator_next(this_value))
            }
            _ => None,
        }
    }
}

const fn direct_array_receiver_requires_to_object(target: NativeCallTarget) -> bool {
    matches!(
        target,
        NativeCallTarget::ArrayConcat
            | NativeCallTarget::ArrayEvery
            | NativeCallTarget::ArrayFilter
            | NativeCallTarget::ArrayFind
            | NativeCallTarget::ArrayFindIndex
            | NativeCallTarget::ArrayFlat
            | NativeCallTarget::ArrayFlatMap
            | NativeCallTarget::ArrayForEach
            | NativeCallTarget::ArrayIncludes
            | NativeCallTarget::ArrayIndexOf
            | NativeCallTarget::ArrayJoin
            | NativeCallTarget::ArrayLastIndexOf
            | NativeCallTarget::ArrayMap
            | NativeCallTarget::ArrayPop
            | NativeCallTarget::ArrayPush
            | NativeCallTarget::ArrayReduce
            | NativeCallTarget::ArrayReduceRight
            | NativeCallTarget::ArrayReverse
            | NativeCallTarget::ArrayShift
            | NativeCallTarget::ArraySlice
            | NativeCallTarget::ArraySome
            | NativeCallTarget::ArrayUnshift
    )
}
