use crate::runtime::native::ArrayBufferFunctionKind;

use super::{ARRAY_BUFFER_METHOD_SLOT_BASE, NativeFunctionKind, NativeFunctionSlot};

const ARRAY_BUFFER_IMMUTABLE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(644);
const ARRAY_BUFFER_SLICE_TO_IMMUTABLE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(645);
const ARRAY_BUFFER_TRANSFER_TO_IMMUTABLE_SLOT: NativeFunctionSlot = NativeFunctionSlot::new(646);

pub(super) const fn array_buffer_slot(kind: NativeFunctionKind) -> Option<NativeFunctionSlot> {
    let NativeFunctionKind::ArrayBufferPrototype(method) = kind else {
        return None;
    };
    match method {
        ArrayBufferFunctionKind::ImmutableGetter => Some(ARRAY_BUFFER_IMMUTABLE_SLOT),
        ArrayBufferFunctionKind::SliceToImmutable => Some(ARRAY_BUFFER_SLICE_TO_IMMUTABLE_SLOT),
        ArrayBufferFunctionKind::TransferToImmutable => {
            Some(ARRAY_BUFFER_TRANSFER_TO_IMMUTABLE_SLOT)
        }
        _ => {
            let Some(index) = ARRAY_BUFFER_METHOD_SLOT_BASE.checked_add(method.index()) else {
                return None;
            };
            Some(NativeFunctionSlot::new(index))
        }
    }
}
