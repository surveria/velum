use crate::runtime::object::DataViewElementKind;

use super::{
    DATA_VIEW_BUFFER_GETTER_SLOT, DATA_VIEW_BYTE_LENGTH_GETTER_SLOT,
    DATA_VIEW_BYTE_OFFSET_GETTER_SLOT, DATA_VIEW_CONSTRUCTOR_SLOT, DATA_VIEW_GET_BIGINT64_SLOT,
    DATA_VIEW_GET_BIGUINT64_SLOT, DATA_VIEW_GET_SLOT_BASE, DATA_VIEW_SET_BIGINT64_SLOT,
    DATA_VIEW_SET_BIGUINT64_SLOT, DATA_VIEW_SET_SLOT_BASE, DataViewFunctionKind,
    NativeFunctionKind, NativeFunctionSlot,
};

pub(super) const fn data_view_slot(kind: NativeFunctionKind) -> Option<NativeFunctionSlot> {
    match kind {
        NativeFunctionKind::DataView(DataViewFunctionKind::Constructor) => {
            Some(DATA_VIEW_CONSTRUCTOR_SLOT)
        }
        NativeFunctionKind::DataView(DataViewFunctionKind::BufferGetter) => {
            Some(DATA_VIEW_BUFFER_GETTER_SLOT)
        }
        NativeFunctionKind::DataView(DataViewFunctionKind::ByteLengthGetter) => {
            Some(DATA_VIEW_BYTE_LENGTH_GETTER_SLOT)
        }
        NativeFunctionKind::DataView(DataViewFunctionKind::ByteOffsetGetter) => {
            Some(DATA_VIEW_BYTE_OFFSET_GETTER_SLOT)
        }
        NativeFunctionKind::DataView(DataViewFunctionKind::Get(DataViewElementKind::BigInt64)) => {
            Some(DATA_VIEW_GET_BIGINT64_SLOT)
        }
        NativeFunctionKind::DataView(DataViewFunctionKind::Get(DataViewElementKind::BigUint64)) => {
            Some(DATA_VIEW_GET_BIGUINT64_SLOT)
        }
        NativeFunctionKind::DataView(DataViewFunctionKind::Set(DataViewElementKind::BigInt64)) => {
            Some(DATA_VIEW_SET_BIGINT64_SLOT)
        }
        NativeFunctionKind::DataView(DataViewFunctionKind::Set(DataViewElementKind::BigUint64)) => {
            Some(DATA_VIEW_SET_BIGUINT64_SLOT)
        }
        NativeFunctionKind::DataView(DataViewFunctionKind::Get(element_kind)) => {
            let Some(index) = DATA_VIEW_GET_SLOT_BASE.checked_add(element_kind.index()) else {
                return None;
            };
            Some(NativeFunctionSlot::new(index))
        }
        NativeFunctionKind::DataView(DataViewFunctionKind::Set(element_kind)) => {
            let Some(index) = DATA_VIEW_SET_SLOT_BASE.checked_add(element_kind.index()) else {
                return None;
            };
            Some(NativeFunctionSlot::new(index))
        }
        _ => None,
    }
}
