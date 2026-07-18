use crate::{
    bytecode::{BytecodeClassMemberKey, BytecodeClassMemberKind},
    error::{Error, Result},
    runtime::object::{
        AccessorPropertyUpdate, DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable,
        PropertyUpdate, PropertyWritable,
    },
    value::Value,
};

pub(super) fn class_element_input_count(
    count: usize,
    decorator_count: usize,
    key: &BytecodeClassMemberKey,
) -> Result<usize> {
    count
        .checked_add(decorator_count)
        .and_then(|count| {
            count.checked_add(usize::from(matches!(key, BytecodeClassMemberKey::Computed)))
        })
        .ok_or_else(|| Error::limit("class evaluation input count overflowed"))
}

pub(super) fn take_class_input_values(
    inputs: &mut alloc::vec::IntoIter<Value>,
    count: usize,
    description: &str,
) -> Result<Vec<Value>> {
    let values = inputs.by_ref().take(count).collect::<Vec<_>>();
    if values.len() != count {
        return Err(Error::runtime(format!("{description} value disappeared")));
    }
    Ok(values)
}

pub(super) fn take_class_computed_key(
    inputs: &mut alloc::vec::IntoIter<Value>,
    key: &BytecodeClassMemberKey,
    missing_message: &str,
) -> Result<Option<Value>> {
    match key {
        BytecodeClassMemberKey::Computed => inputs
            .next()
            .map(Some)
            .ok_or_else(|| Error::runtime(missing_message)),
        BytecodeClassMemberKey::Static(_) | BytecodeClassMemberKey::Private { .. } => Ok(None),
    }
}

pub(super) const fn class_member_decorator_kind(kind: BytecodeClassMemberKind) -> &'static str {
    match kind {
        BytecodeClassMemberKind::Method => "method",
        BytecodeClassMemberKind::Getter => "getter",
        BytecodeClassMemberKind::Setter => "setter",
    }
}

pub(super) const fn class_member_property_update(
    kind: BytecodeClassMemberKind,
    function: Value,
) -> PropertyUpdate {
    match kind {
        BytecodeClassMemberKind::Method => PropertyUpdate::Data(DataPropertyUpdate::new(
            Some(function),
            Some(PropertyWritable::Yes),
            Some(PropertyEnumerable::No),
            Some(PropertyConfigurable::Yes),
        )),
        BytecodeClassMemberKind::Getter => PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
            Some(function),
            None,
            Some(PropertyEnumerable::No),
            Some(PropertyConfigurable::Yes),
        )),
        BytecodeClassMemberKind::Setter => PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
            None,
            Some(function),
            Some(PropertyEnumerable::No),
            Some(PropertyConfigurable::Yes),
        )),
    }
}
