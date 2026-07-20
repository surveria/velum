#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::rc::Rc;

use crate::{
    error::{Error, Result},
    syntax::{BinaryOp, DeclKind, UpdateOp},
    value::Value,
};

use super::{BytecodeBinding, BytecodeInstruction, BytecodeNumericBinaryOp};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BytecodeLinearTemplate {
    entries: Rc<[BytecodeLinearTemplateEntry]>,
    peepholes: Rc<[BytecodeLinearPeepholeKind]>,
    uses_with_environment: bool,
    reduction_role: Option<BytecodeNumericArrayReductionRole>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct BytecodeLinearTemplateEntry {
    peephole_start: u32,
    peephole_count: u8,
    single_op: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeLinearPeepholeKind {
    CompareBindingNumber,
    DeclareVarFromBindingNumberBinary,
    StoreBindingFromBindingNumberBinary,
    NumericBindingChain,
    NumericCompoundChain,
    NumericCompoundBinding,
    PropertyMutation,
    UpdateBindingStoreLast,
    AddArrayElementToBindingWithMask,
    InStaticPropertyBinding,
    InArrayIndexMaskBinding,
    AddArrayElementToBinding,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeNumericArrayReductionRole {
    Condition,
    Update,
    Body,
}

impl BytecodeLinearTemplate {
    pub(crate) fn compile(instructions: &[BytecodeInstruction]) -> Result<Self> {
        let uses_with_environment = instructions.iter().any(instruction_uses_with_environment);
        let reduction_role = recognize_numeric_array_reduction_role(instructions);
        if uses_with_environment {
            return Ok(Self {
                entries: Rc::from([]),
                peepholes: Rc::from([]),
                uses_with_environment,
                reduction_role,
            });
        }

        let mut entries = Vec::with_capacity(instructions.len());
        let mut peepholes = Vec::new();
        for index in 0..instructions.len() {
            let peephole_start = peepholes.len();
            recognize_peepholes(instructions, index, &mut peepholes)?;
            let peephole_count = peepholes
                .len()
                .checked_sub(peephole_start)
                .ok_or_else(|| Error::runtime("linear template candidate count underflowed"))?;
            let instruction = instructions
                .get(index)
                .ok_or_else(|| Error::runtime("linear template instruction is not defined"))?;
            entries.push(BytecodeLinearTemplateEntry {
                peephole_start: u32::try_from(peephole_start)
                    .map_err(|_| Error::limit("linear template candidate offset overflowed"))?,
                peephole_count: u8::try_from(peephole_count)
                    .map_err(|_| Error::limit("linear template candidate count overflowed"))?,
                single_op: instruction_is_linear_op(instruction),
            });
        }
        Ok(Self {
            entries: Rc::from(entries.into_boxed_slice()),
            peepholes: Rc::from(peepholes.into_boxed_slice()),
            uses_with_environment,
            reduction_role,
        })
    }

    pub(crate) const fn uses_with_environment(&self) -> bool {
        self.uses_with_environment
    }

    pub(crate) fn peepholes_at(&self, index: usize) -> Result<&[BytecodeLinearPeepholeKind]> {
        let entry = self
            .entries
            .get(index)
            .ok_or_else(|| Error::runtime("linear template entry is not defined"))?;
        let start = usize::try_from(entry.peephole_start)
            .map_err(|_| Error::runtime("linear template candidate offset is not supported"))?;
        let end = start
            .checked_add(usize::from(entry.peephole_count))
            .ok_or_else(|| Error::runtime("linear template candidate range overflowed"))?;
        self.peepholes
            .get(start..end)
            .ok_or_else(|| Error::runtime("linear template candidate range is not defined"))
    }

    pub(crate) fn instruction_is_linear(&self, index: usize) -> Result<bool> {
        self.entries
            .get(index)
            .map(|entry| entry.single_op)
            .ok_or_else(|| Error::runtime("linear template entry is not defined"))
    }

    pub(crate) const fn reduction_role(&self) -> Option<BytecodeNumericArrayReductionRole> {
        self.reduction_role
    }

    pub(crate) fn peephole_candidate_count(&self) -> usize {
        self.peepholes.len()
    }
}

fn recognize_peepholes(
    instructions: &[BytecodeInstruction],
    index: usize,
    candidates: &mut Vec<BytecodeLinearPeepholeKind>,
) -> Result<()> {
    if recognize_compare_binding_number(instructions, index) {
        candidates.push(BytecodeLinearPeepholeKind::CompareBindingNumber);
    }
    if recognize_declare_var_from_binding_number_binary(instructions, index) {
        candidates.push(BytecodeLinearPeepholeKind::DeclareVarFromBindingNumberBinary);
    }
    if recognize_store_binding_from_binding_number_binary(instructions, index) {
        candidates.push(BytecodeLinearPeepholeKind::StoreBindingFromBindingNumberBinary);
    }
    if recognize_numeric_binding_chain(instructions, index)? {
        candidates.push(BytecodeLinearPeepholeKind::NumericBindingChain);
    }
    if recognize_numeric_compound_chain(instructions, index)? {
        candidates.push(BytecodeLinearPeepholeKind::NumericCompoundChain);
    }
    if recognize_numeric_compound_binding(instructions, index).is_some() {
        candidates.push(BytecodeLinearPeepholeKind::NumericCompoundBinding);
    }
    if recognize_property_mutation(instructions, index) {
        candidates.push(BytecodeLinearPeepholeKind::PropertyMutation);
    }
    if recognize_update_binding_store_last(instructions, index) {
        candidates.push(BytecodeLinearPeepholeKind::UpdateBindingStoreLast);
    }
    if recognize_add_array_element(instructions, index, true) {
        candidates.push(BytecodeLinearPeepholeKind::AddArrayElementToBindingWithMask);
    }
    if recognize_in_static_property_binding(instructions, index) {
        candidates.push(BytecodeLinearPeepholeKind::InStaticPropertyBinding);
    }
    if recognize_in_array_index_mask_binding(instructions, index) {
        candidates.push(BytecodeLinearPeepholeKind::InArrayIndexMaskBinding);
    }
    if recognize_add_array_element(instructions, index, false) {
        candidates.push(BytecodeLinearPeepholeKind::AddArrayElementToBinding);
    }
    Ok(())
}

fn recognize_compare_binding_number(instructions: &[BytecodeInstruction], index: usize) -> bool {
    matches!(
        instruction_window(instructions, index, 4),
        Some([
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::PushLiteral(Value::Number(_)),
            BytecodeInstruction::NumberCompare(_),
            BytecodeInstruction::StoreLast,
        ])
    )
}

fn recognize_declare_var_from_binding_number_binary(
    instructions: &[BytecodeInstruction],
    index: usize,
) -> bool {
    matches!(
        instruction_window(instructions, index, 4),
        Some([
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::PushLiteral(Value::Number(_)),
            BytecodeInstruction::NumberBinary(_),
            BytecodeInstruction::DeclareBinding {
                kind: DeclKind::Var,
                has_init: true,
                ..
            },
        ])
    )
}

fn recognize_store_binding_from_binding_number_binary(
    instructions: &[BytecodeInstruction],
    index: usize,
) -> bool {
    matches!(
        instruction_window(instructions, index, 5),
        Some([
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::PushLiteral(Value::Number(_)),
            BytecodeInstruction::NumberBinary(_),
            BytecodeInstruction::StoreBinding(_),
            BytecodeInstruction::StoreLast,
        ])
    )
}

fn recognize_numeric_binding_chain(
    instructions: &[BytecodeInstruction],
    index: usize,
) -> Result<bool> {
    if !matches!(
        instructions.get(index),
        Some(BytecodeInstruction::LoadBinding(_))
    ) {
        return Ok(false);
    }
    let mut cursor = checked_add(index, 1, "numeric binding template index overflowed")?;
    let mut term_count = 0_usize;
    loop {
        let consumed = if matches!(
            instruction_window(instructions, cursor, 2),
            Some([
                BytecodeInstruction::PushLiteral(Value::Number(_)),
                BytecodeInstruction::NumberBinary(_),
            ])
        ) {
            2
        } else if matches!(
            instruction_window(instructions, cursor, 3),
            Some([
                BytecodeInstruction::LoadBinding(_),
                BytecodeInstruction::StaticMember { .. },
                BytecodeInstruction::NumberBinary(_),
            ])
        ) {
            3
        } else if matches!(
            instruction_window(instructions, cursor, 4),
            Some([
                BytecodeInstruction::LoadBinding(_),
                BytecodeInstruction::PushLiteral(Value::Number(_)),
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
                BytecodeInstruction::NumberBinary(_),
            ])
        ) {
            4
        } else {
            break;
        };
        term_count = term_count
            .checked_add(1)
            .ok_or_else(|| Error::limit("numeric binding template term count overflowed"))?;
        cursor = checked_add(
            cursor,
            consumed,
            "numeric binding template index overflowed",
        )?;
    }
    Ok(term_count > 0
        && matches!(
            instruction_window(instructions, cursor, 2),
            Some([
                BytecodeInstruction::StoreBinding(_),
                BytecodeInstruction::StoreLast,
            ])
        ))
}

fn recognize_numeric_compound_chain(
    instructions: &[BytecodeInstruction],
    index: usize,
) -> Result<bool> {
    let Some((first_consumed, target)) = recognize_numeric_compound_binding(instructions, index)
    else {
        return Ok(false);
    };
    let mut cursor = checked_add(
        index,
        first_consumed,
        "numeric compound template index overflowed",
    )?;
    let mut term_count = 1_usize;
    while let Some((consumed, next_target)) =
        recognize_numeric_compound_binding(instructions, cursor)
    {
        if !same_bytecode_binding(target, next_target) {
            break;
        }
        cursor = checked_add(
            cursor,
            consumed,
            "numeric compound template index overflowed",
        )?;
        term_count = term_count
            .checked_add(1)
            .ok_or_else(|| Error::limit("numeric compound template term count overflowed"))?;
    }
    Ok(term_count >= 2)
}

fn recognize_numeric_compound_binding(
    instructions: &[BytecodeInstruction],
    index: usize,
) -> Option<(usize, &BytecodeBinding)> {
    if let Some(
        [
            BytecodeInstruction::PushLiteral(Value::Number(_)),
            BytecodeInstruction::CompoundStoreBinding { name, op },
            BytecodeInstruction::StoreLast,
        ],
    ) = instruction_window(instructions, index, 3)
        && BytecodeNumericBinaryOp::from_binary(*op).is_some()
    {
        return Some((3, name));
    }
    if let Some(
        [
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::CompoundStoreBinding { name, op },
            BytecodeInstruction::StoreLast,
        ],
    ) = instruction_window(instructions, index, 3)
        && BytecodeNumericBinaryOp::from_binary(*op).is_some()
    {
        return Some((3, name));
    }
    if let Some(
        [
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::StaticMember { .. },
            BytecodeInstruction::CompoundStoreBinding { name, op },
            BytecodeInstruction::StoreLast,
        ],
    ) = instruction_window(instructions, index, 4)
        && BytecodeNumericBinaryOp::from_binary(*op).is_some()
    {
        return Some((4, name));
    }
    if let Some(
        [
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::PushLiteral(Value::Number(_)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::CompoundStoreBinding { name, op },
            BytecodeInstruction::StoreLast,
        ],
    ) = instruction_window(instructions, index, 5)
        && BytecodeNumericBinaryOp::from_binary(*op).is_some()
    {
        return Some((5, name));
    }
    None
}

fn recognize_property_mutation(instructions: &[BytecodeInstruction], index: usize) -> bool {
    recognize_dynamic_array_update_with_mask(instructions, index)
        || recognize_dynamic_array_update(instructions, index)
        || recognize_static_property_update(instructions, index)
        || recognize_dynamic_array_compound_static_property(instructions, index)
        || recognize_dynamic_array_compound_literal_with_mask(instructions, index)
        || recognize_dynamic_array_compound_literal(instructions, index)
        || recognize_static_property_compound_array(instructions, index)
        || recognize_static_property_compound_bitand(instructions, index)
        || recognize_static_property_compound_binding(instructions, index)
        || recognize_static_property_compound_literal(instructions, index)
}

fn recognize_dynamic_array_update_with_mask(
    instructions: &[BytecodeInstruction],
    index: usize,
) -> bool {
    matches!(
        instruction_window(instructions, index, 6),
        Some([
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::PushLiteral(Value::Number(_)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::UpdateComputedProperty { strict: false, .. },
            BytecodeInstruction::StoreLast,
        ])
    )
}

fn recognize_dynamic_array_update(instructions: &[BytecodeInstruction], index: usize) -> bool {
    matches!(
        instruction_window(instructions, index, 4),
        Some([
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::UpdateComputedProperty { strict: false, .. },
            BytecodeInstruction::StoreLast,
        ])
    )
}

fn recognize_static_property_update(instructions: &[BytecodeInstruction], index: usize) -> bool {
    matches!(
        instruction_window(instructions, index, 3),
        Some([
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::UpdateStaticProperty { strict: false, .. },
            BytecodeInstruction::StoreLast,
        ])
    )
}

fn recognize_dynamic_array_compound_static_property(
    instructions: &[BytecodeInstruction],
    index: usize,
) -> bool {
    matches!(
        instruction_window(instructions, index, 10),
        Some([
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::PushLiteral(Value::Number(_)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::StaticMember { .. },
            BytecodeInstruction::PushLiteral(Value::Number(_)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::CompoundComputedProperty { strict: false, .. },
            BytecodeInstruction::StoreLast,
        ])
    )
}

fn recognize_dynamic_array_compound_literal_with_mask(
    instructions: &[BytecodeInstruction],
    index: usize,
) -> bool {
    matches!(
        instruction_window(instructions, index, 7),
        Some([
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::PushLiteral(Value::Number(_)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::PushLiteral(Value::Number(_)),
            BytecodeInstruction::CompoundComputedProperty { strict: false, .. },
            BytecodeInstruction::StoreLast,
        ])
    )
}

fn recognize_dynamic_array_compound_literal(
    instructions: &[BytecodeInstruction],
    index: usize,
) -> bool {
    matches!(
        instruction_window(instructions, index, 5),
        Some([
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::PushLiteral(Value::Number(_)),
            BytecodeInstruction::CompoundComputedProperty { strict: false, .. },
            BytecodeInstruction::StoreLast,
        ])
    )
}

fn recognize_static_property_compound_array(
    instructions: &[BytecodeInstruction],
    index: usize,
) -> bool {
    matches!(
        instruction_window(instructions, index, 8),
        Some([
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::PushLiteral(Value::Number(_)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::ComputedMember { .. },
            BytecodeInstruction::CompoundStaticProperty { strict: false, .. },
            BytecodeInstruction::StoreLast,
        ])
    )
}

fn recognize_static_property_compound_bitand(
    instructions: &[BytecodeInstruction],
    index: usize,
) -> bool {
    matches!(
        instruction_window(instructions, index, 6),
        Some([
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::PushLiteral(Value::Number(_)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::CompoundStaticProperty { strict: false, .. },
            BytecodeInstruction::StoreLast,
        ])
    )
}

fn recognize_static_property_compound_binding(
    instructions: &[BytecodeInstruction],
    index: usize,
) -> bool {
    matches!(
        instruction_window(instructions, index, 4),
        Some([
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::CompoundStaticProperty { strict: false, .. },
            BytecodeInstruction::StoreLast,
        ])
    )
}

fn recognize_static_property_compound_literal(
    instructions: &[BytecodeInstruction],
    index: usize,
) -> bool {
    matches!(
        instruction_window(instructions, index, 4),
        Some([
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::PushLiteral(Value::Number(_)),
            BytecodeInstruction::CompoundStaticProperty { strict: false, .. },
            BytecodeInstruction::StoreLast,
        ])
    )
}

fn recognize_update_binding_store_last(instructions: &[BytecodeInstruction], index: usize) -> bool {
    matches!(
        instruction_window(instructions, index, 2),
        Some([
            BytecodeInstruction::UpdateBinding { .. },
            BytecodeInstruction::StoreLast,
        ])
    )
}

fn recognize_add_array_element(
    instructions: &[BytecodeInstruction],
    index: usize,
    masked: bool,
) -> bool {
    let bindings = if masked {
        let Some(
            [
                BytecodeInstruction::LoadBinding(target_read),
                BytecodeInstruction::LoadBinding(array),
                BytecodeInstruction::LoadBinding(index_binding),
                BytecodeInstruction::PushLiteral(Value::Number(_)),
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
                BytecodeInstruction::ComputedMember { .. },
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
                BytecodeInstruction::StoreBinding(target_write),
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 9)
        else {
            return false;
        };
        (target_read, target_write, array, index_binding)
    } else {
        let Some(
            [
                BytecodeInstruction::LoadBinding(target_read),
                BytecodeInstruction::LoadBinding(array),
                BytecodeInstruction::LoadBinding(index_binding),
                BytecodeInstruction::ComputedMember { .. },
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
                BytecodeInstruction::StoreBinding(target_write),
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 7)
        else {
            return false;
        };
        (target_read, target_write, array, index_binding)
    };
    same_bytecode_binding(bindings.0, bindings.1)
}

fn recognize_in_static_property_binding(
    instructions: &[BytecodeInstruction],
    index: usize,
) -> bool {
    matches!(
        instruction_window(instructions, index, 2),
        Some([
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::InStaticProperty { .. },
        ])
    )
}

fn recognize_in_array_index_mask_binding(
    instructions: &[BytecodeInstruction],
    index: usize,
) -> bool {
    matches!(
        instruction_window(instructions, index, 5),
        Some([
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::PushLiteral(Value::Number(_)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::Binary {
                op: BinaryOp::In,
                property_access: Some(_),
            },
        ])
    )
}

const fn instruction_is_linear_op(instruction: &BytecodeInstruction) -> bool {
    matches!(
        instruction,
        BytecodeInstruction::PushLiteral(_)
            | BytecodeInstruction::PushUndefined
            | BytecodeInstruction::LoadBinding(_)
            | BytecodeInstruction::StoreBinding(_)
            | BytecodeInstruction::DeclareBinding {
                kind: DeclKind::Var,
                ..
            }
            | BytecodeInstruction::StoreLast
            | BytecodeInstruction::Pop
            | BytecodeInstruction::UpdateBinding { .. }
            | BytecodeInstruction::CompoundStoreBinding { .. }
            | BytecodeInstruction::NumberBinary(_)
            | BytecodeInstruction::NumberCompare(_)
            | BytecodeInstruction::NumberEquality(_)
            | BytecodeInstruction::ArrayLength { .. }
            | BytecodeInstruction::ArrayIndexMember { .. }
            | BytecodeInstruction::ComputedMember { .. }
    )
}

fn recognize_numeric_array_reduction_role(
    instructions: &[BytecodeInstruction],
) -> Option<BytecodeNumericArrayReductionRole> {
    if matches!(
        instructions,
        [
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::LoadBinding(_),
            BytecodeInstruction::ArrayLength { .. },
            BytecodeInstruction::NumberCompare(super::BytecodeNumericCompareOp::Less),
            BytecodeInstruction::StoreLast,
        ]
    ) {
        return Some(BytecodeNumericArrayReductionRole::Condition);
    }
    if recognize_numeric_reduction_update(instructions) {
        return Some(BytecodeNumericArrayReductionRole::Update);
    }
    let [
        BytecodeInstruction::LoadBinding(target_read),
        BytecodeInstruction::LoadBinding(_),
        BytecodeInstruction::LoadBinding(index),
        BytecodeInstruction::ComputedMember { .. },
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::StoreBinding(target_write),
        BytecodeInstruction::StoreLast,
    ] = instructions
    else {
        return None;
    };
    if same_bytecode_binding(target_read, target_write)
        && !same_bytecode_binding(index, target_write)
    {
        return Some(BytecodeNumericArrayReductionRole::Body);
    }
    None
}

fn recognize_numeric_reduction_update(instructions: &[BytecodeInstruction]) -> bool {
    match instructions {
        [
            BytecodeInstruction::LoadBinding(read),
            BytecodeInstruction::PushLiteral(Value::Number(step)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
            BytecodeInstruction::StoreBinding(write),
            BytecodeInstruction::StoreLast,
        ] => step.to_bits() == 1.0f64.to_bits() && same_bytecode_binding(read, write),
        [
            BytecodeInstruction::UpdateBinding {
                op: UpdateOp::Increment,
                ..
            },
            BytecodeInstruction::StoreLast,
        ] => true,
        _ => false,
    }
}

const fn instruction_uses_with_environment(instruction: &BytecodeInstruction) -> bool {
    match instruction {
        BytecodeInstruction::LoadBinding(binding)
        | BytecodeInstruction::StoreBinding(binding)
        | BytecodeInstruction::ResolveBinding(binding)
        | BytecodeInstruction::StoreResolvedBinding(binding)
        | BytecodeInstruction::TypeOfBinding(binding)
        | BytecodeInstruction::DeleteBinding(binding) => binding.with_environment_count() > 0,
        BytecodeInstruction::DeclareBinding { name, .. }
        | BytecodeInstruction::UpdateBinding { name, .. }
        | BytecodeInstruction::CompoundStoreBinding { name, .. } => {
            name.with_environment_count() > 0
        }
        BytecodeInstruction::CallBinding { callee, .. }
        | BytecodeInstruction::CallBindingSpread { callee, .. }
        | BytecodeInstruction::Construct {
            constructor: callee,
            ..
        } => callee.with_environment_count() > 0,
        _ => false,
    }
}

fn instruction_window(
    instructions: &[BytecodeInstruction],
    start: usize,
    len: usize,
) -> Option<&[BytecodeInstruction]> {
    let end = start.checked_add(len)?;
    instructions.get(start..end)
}

fn same_bytecode_binding(left: &BytecodeBinding, right: &BytecodeBinding) -> bool {
    left.operand() == right.operand() && left.name().as_str() == right.name().as_str()
}

fn checked_add(value: usize, additional: usize, message: &'static str) -> Result<usize> {
    value
        .checked_add(additional)
        .ok_or_else(|| Error::runtime(message))
}
