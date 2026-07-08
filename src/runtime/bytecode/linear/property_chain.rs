use crate::{
    bytecode::{
        BytecodeBinding, BytecodeDynamicProperty, BytecodeInstruction, BytecodeNumericBinaryOp,
        BytecodeProperty,
    },
    error::Result,
    runtime::{Context, binding::scope::BindingCell},
    syntax::{BinaryOp, UpdateOp},
    value::Value,
};

use super::BytecodeState;

#[derive(Debug)]
pub(super) struct CompiledPropertyMutation<'a> {
    pub(super) op: PropertyMutation<'a>,
    pub(super) consumed: usize,
}

#[derive(Debug)]
pub(super) enum PropertyMutation<'a> {
    Update {
        target: PropertyTarget<'a>,
        op: UpdateOp,
        prefix: bool,
    },
    Compound {
        target: PropertyTarget<'a>,
        rhs: PropertyNumericRhs<'a>,
        op: BinaryOp,
    },
}

#[derive(Debug)]
pub(super) enum PropertyTarget<'a> {
    Static {
        object: &'a BytecodeBinding,
        object_cell: BindingCell,
        property: &'a BytecodeProperty,
    },
    DynamicArray {
        array: &'a BytecodeBinding,
        array_cell: BindingCell,
        index: &'a BytecodeBinding,
        index_cell: BindingCell,
        index_mask: Option<f64>,
        property: BytecodeDynamicProperty,
    },
}

#[derive(Debug)]
pub(super) enum PropertyNumericRhs<'a> {
    Literal(f64),
    Binding {
        binding: &'a BytecodeBinding,
        cell: BindingCell,
    },
    BindingBitAndLiteral {
        binding: &'a BytecodeBinding,
        cell: BindingCell,
        mask: f64,
    },
    StaticPropertyBitAndLiteral {
        object: &'a BytecodeBinding,
        object_cell: BindingCell,
        property: &'a BytecodeProperty,
        mask: f64,
    },
    DynamicArrayElement {
        array: &'a BytecodeBinding,
        array_cell: BindingCell,
        index: &'a BytecodeBinding,
        index_cell: BindingCell,
        index_mask: Option<f64>,
        property: BytecodeDynamicProperty,
    },
}

impl Context {
    pub(super) fn compile_property_mutation<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<CompiledPropertyMutation<'a>>> {
        if let Some(op) = self.compile_dynamic_array_update_with_mask(instructions, index)? {
            return Ok(Some(CompiledPropertyMutation { op, consumed: 6 }));
        }
        if let Some(op) = self.compile_dynamic_array_update(instructions, index)? {
            return Ok(Some(CompiledPropertyMutation { op, consumed: 4 }));
        }
        if let Some(op) = self.compile_static_property_update(instructions, index)? {
            return Ok(Some(CompiledPropertyMutation { op, consumed: 3 }));
        }
        if let Some(op) =
            self.compile_dynamic_array_compound_static_property_bitand(instructions, index)?
        {
            return Ok(Some(CompiledPropertyMutation { op, consumed: 10 }));
        }
        if let Some(op) =
            self.compile_dynamic_array_compound_literal_with_mask(instructions, index)?
        {
            return Ok(Some(CompiledPropertyMutation { op, consumed: 7 }));
        }
        if let Some(op) = self.compile_dynamic_array_compound_literal(instructions, index)? {
            return Ok(Some(CompiledPropertyMutation { op, consumed: 5 }));
        }
        if let Some(op) = self.compile_static_property_compound_array_rhs(instructions, index)? {
            return Ok(Some(CompiledPropertyMutation { op, consumed: 8 }));
        }
        if let Some(op) = self.compile_static_property_compound_bitand_rhs(instructions, index)? {
            return Ok(Some(CompiledPropertyMutation { op, consumed: 6 }));
        }
        if let Some(op) = self.compile_static_property_compound_binding_rhs(instructions, index)? {
            return Ok(Some(CompiledPropertyMutation { op, consumed: 4 }));
        }
        self.compile_static_property_compound_literal(instructions, index)
            .map(|op| op.map(|op| CompiledPropertyMutation { op, consumed: 4 }))
    }

    fn compile_static_property_update<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<PropertyMutation<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(object),
                BytecodeInstruction::UpdateStaticProperty {
                    property,
                    op,
                    prefix,
                },
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 3)
        else {
            return Ok(None);
        };
        self.compile_static_target(object, property).map(|target| {
            target.map(|target| PropertyMutation::Update {
                target,
                op: *op,
                prefix: *prefix,
            })
        })
    }

    fn compile_dynamic_array_update_with_mask<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<PropertyMutation<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(array),
                BytecodeInstruction::LoadBinding(index_binding),
                BytecodeInstruction::PushLiteral(Value::Number(mask)),
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
                BytecodeInstruction::UpdateComputedProperty {
                    property,
                    op,
                    prefix,
                },
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 6)
        else {
            return Ok(None);
        };
        self.compile_dynamic_array_target(array, index_binding, Some(*mask), *property)
            .map(|target| {
                target.map(|target| PropertyMutation::Update {
                    target,
                    op: *op,
                    prefix: *prefix,
                })
            })
    }

    fn compile_dynamic_array_update<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<PropertyMutation<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(array),
                BytecodeInstruction::LoadBinding(index_binding),
                BytecodeInstruction::UpdateComputedProperty {
                    property,
                    op,
                    prefix,
                },
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 4)
        else {
            return Ok(None);
        };
        self.compile_dynamic_array_target(array, index_binding, None, *property)
            .map(|target| {
                target.map(|target| PropertyMutation::Update {
                    target,
                    op: *op,
                    prefix: *prefix,
                })
            })
    }

    fn compile_static_property_compound_literal<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<PropertyMutation<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(object),
                BytecodeInstruction::PushLiteral(Value::Number(right)),
                BytecodeInstruction::CompoundStaticProperty { property, op },
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 4)
        else {
            return Ok(None);
        };
        self.compile_static_compound(object, property, PropertyNumericRhs::Literal(*right), *op)
    }

    fn compile_static_property_compound_binding_rhs<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<PropertyMutation<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(object),
                BytecodeInstruction::LoadBinding(binding),
                BytecodeInstruction::CompoundStaticProperty { property, op },
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 4)
        else {
            return Ok(None);
        };
        let Some(cell) = self.get_binding_bytecode(binding)? else {
            return Ok(None);
        };
        self.compile_static_compound(
            object,
            property,
            PropertyNumericRhs::Binding { binding, cell },
            *op,
        )
    }

    fn compile_static_property_compound_bitand_rhs<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<PropertyMutation<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(object),
                BytecodeInstruction::LoadBinding(binding),
                BytecodeInstruction::PushLiteral(Value::Number(mask)),
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
                BytecodeInstruction::CompoundStaticProperty { property, op },
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 6)
        else {
            return Ok(None);
        };
        let Some(cell) = self.get_binding_bytecode(binding)? else {
            return Ok(None);
        };
        self.compile_static_compound(
            object,
            property,
            PropertyNumericRhs::BindingBitAndLiteral {
                binding,
                cell,
                mask: *mask,
            },
            *op,
        )
    }

    fn compile_static_property_compound_array_rhs<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<PropertyMutation<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(object),
                BytecodeInstruction::LoadBinding(array),
                BytecodeInstruction::LoadBinding(index_binding),
                BytecodeInstruction::PushLiteral(Value::Number(mask)),
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
                BytecodeInstruction::ComputedMember {
                    property: rhs_property,
                },
                BytecodeInstruction::CompoundStaticProperty { property, op },
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 8)
        else {
            return Ok(None);
        };
        let Some(rhs) =
            self.compile_dynamic_array_rhs(array, index_binding, Some(*mask), *rhs_property)?
        else {
            return Ok(None);
        };
        self.compile_static_compound(object, property, rhs, *op)
    }

    fn compile_dynamic_array_compound_literal_with_mask<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<PropertyMutation<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(array),
                BytecodeInstruction::LoadBinding(index_binding),
                BytecodeInstruction::PushLiteral(Value::Number(mask)),
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
                BytecodeInstruction::PushLiteral(Value::Number(right)),
                BytecodeInstruction::CompoundComputedProperty { property, op },
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 7)
        else {
            return Ok(None);
        };
        self.compile_dynamic_array_compound(
            array,
            index_binding,
            Some(*mask),
            *property,
            PropertyNumericRhs::Literal(*right),
            *op,
        )
    }

    fn compile_dynamic_array_compound_literal<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<PropertyMutation<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(array),
                BytecodeInstruction::LoadBinding(index_binding),
                BytecodeInstruction::PushLiteral(Value::Number(right)),
                BytecodeInstruction::CompoundComputedProperty { property, op },
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 5)
        else {
            return Ok(None);
        };
        self.compile_dynamic_array_compound(
            array,
            index_binding,
            None,
            *property,
            PropertyNumericRhs::Literal(*right),
            *op,
        )
    }

    fn compile_dynamic_array_compound_static_property_bitand<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<PropertyMutation<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(array),
                BytecodeInstruction::LoadBinding(index_binding),
                BytecodeInstruction::PushLiteral(Value::Number(index_mask)),
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
                BytecodeInstruction::LoadBinding(object),
                BytecodeInstruction::StaticMember {
                    property: rhs_property,
                },
                BytecodeInstruction::PushLiteral(Value::Number(rhs_mask)),
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
                BytecodeInstruction::CompoundComputedProperty { property, op },
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 10)
        else {
            return Ok(None);
        };
        let Some(object_cell) = self.get_binding_bytecode(object)? else {
            return Ok(None);
        };
        self.compile_dynamic_array_compound(
            array,
            index_binding,
            Some(*index_mask),
            *property,
            PropertyNumericRhs::StaticPropertyBitAndLiteral {
                object,
                object_cell,
                property: rhs_property,
                mask: *rhs_mask,
            },
            *op,
        )
    }

    fn compile_static_compound<'a>(
        &self,
        object: &'a BytecodeBinding,
        property: &'a BytecodeProperty,
        rhs: PropertyNumericRhs<'a>,
        op: BinaryOp,
    ) -> Result<Option<PropertyMutation<'a>>> {
        self.compile_static_target(object, property)
            .map(|target| target.map(|target| PropertyMutation::Compound { target, rhs, op }))
    }

    fn compile_dynamic_array_compound<'a>(
        &self,
        array: &'a BytecodeBinding,
        index: &'a BytecodeBinding,
        index_mask: Option<f64>,
        property: BytecodeDynamicProperty,
        rhs: PropertyNumericRhs<'a>,
        op: BinaryOp,
    ) -> Result<Option<PropertyMutation<'a>>> {
        self.compile_dynamic_array_target(array, index, index_mask, property)
            .map(|target| target.map(|target| PropertyMutation::Compound { target, rhs, op }))
    }

    fn compile_static_target<'a>(
        &self,
        object: &'a BytecodeBinding,
        property: &'a BytecodeProperty,
    ) -> Result<Option<PropertyTarget<'a>>> {
        let Some(object_cell) = self.get_binding_bytecode(object)? else {
            return Ok(None);
        };
        Ok(Some(PropertyTarget::Static {
            object,
            object_cell,
            property,
        }))
    }

    fn compile_dynamic_array_target<'a>(
        &self,
        array: &'a BytecodeBinding,
        index: &'a BytecodeBinding,
        index_mask: Option<f64>,
        property: BytecodeDynamicProperty,
    ) -> Result<Option<PropertyTarget<'a>>> {
        let Some(array_cell) = self.get_binding_bytecode(array)? else {
            return Ok(None);
        };
        let Some(index_cell) = self.get_binding_bytecode(index)? else {
            return Ok(None);
        };
        Ok(Some(PropertyTarget::DynamicArray {
            array,
            array_cell,
            index,
            index_cell,
            index_mask,
            property,
        }))
    }

    fn compile_dynamic_array_rhs<'a>(
        &self,
        array: &'a BytecodeBinding,
        index: &'a BytecodeBinding,
        index_mask: Option<f64>,
        property: BytecodeDynamicProperty,
    ) -> Result<Option<PropertyNumericRhs<'a>>> {
        let Some(array_cell) = self.get_binding_bytecode(array)? else {
            return Ok(None);
        };
        let Some(index_cell) = self.get_binding_bytecode(index)? else {
            return Ok(None);
        };
        Ok(Some(PropertyNumericRhs::DynamicArrayElement {
            array,
            array_cell,
            index,
            index_cell,
            index_mask,
            property,
        }))
    }

    pub(super) fn eval_property_mutation(
        &mut self,
        state: &mut BytecodeState,
        mutation: &PropertyMutation<'_>,
    ) -> Result<()> {
        state.last = match mutation {
            PropertyMutation::Update { target, op, prefix } => {
                self.eval_property_update(target, *op, *prefix)?
            }
            PropertyMutation::Compound { target, rhs, op } => {
                let rhs = self.eval_property_numeric_rhs(rhs)?;
                self.eval_property_compound(target, *op, &rhs)?
            }
        };
        Ok(())
    }

    fn eval_property_update(
        &mut self,
        target: &PropertyTarget<'_>,
        op: UpdateOp,
        prefix: bool,
    ) -> Result<Value> {
        match target {
            PropertyTarget::Static {
                object,
                object_cell,
                property,
            } => {
                let object = self.runtime_value(object_cell.value(object.name())?)?;
                self.eval_bytecode_update_static_property(
                    &object,
                    property.name(),
                    property.access(),
                    op,
                    prefix,
                )
            }
            PropertyTarget::DynamicArray {
                array,
                array_cell,
                index,
                index_cell,
                index_mask,
                property,
            } => {
                let object = self.runtime_value(array_cell.value(array.name())?)?;
                let index = self.eval_property_index_value(index, index_cell, *index_mask)?;
                if let Some(value) = self.eval_dynamic_array_index_update(
                    &object,
                    &index,
                    property.access(),
                    op,
                    prefix,
                )? {
                    return Ok(value);
                }
                let key = self.dynamic_property_key(&index)?;
                self.eval_bytecode_update_dynamic_property(
                    &object,
                    key,
                    property.access(),
                    op,
                    prefix,
                )
            }
        }
    }

    fn eval_property_compound(
        &mut self,
        target: &PropertyTarget<'_>,
        op: BinaryOp,
        rhs: &Value,
    ) -> Result<Value> {
        match target {
            PropertyTarget::Static {
                object,
                object_cell,
                property,
            } => {
                let object = self.runtime_value(object_cell.value(object.name())?)?;
                self.eval_bytecode_static_compound_assignment(
                    op,
                    &object,
                    property.name(),
                    property.access(),
                    rhs,
                )
            }
            PropertyTarget::DynamicArray {
                array,
                array_cell,
                index,
                index_cell,
                index_mask,
                property,
            } => {
                let object = self.runtime_value(array_cell.value(array.name())?)?;
                let index = self.eval_property_index_value(index, index_cell, *index_mask)?;
                if let Some(value) = self.eval_dynamic_array_index_compound_assignment(
                    op,
                    &object,
                    &index,
                    property.access(),
                    rhs,
                )? {
                    return Ok(value);
                }
                let key = self.dynamic_property_key(&index)?;
                self.eval_bytecode_dynamic_compound_assignment(
                    op,
                    &object,
                    key,
                    property.access(),
                    rhs,
                )
            }
        }
    }

    fn eval_property_numeric_rhs(&mut self, rhs: &PropertyNumericRhs<'_>) -> Result<Value> {
        match rhs {
            PropertyNumericRhs::Literal(value) => Ok(Value::Number(*value)),
            PropertyNumericRhs::Binding { binding, cell } => {
                self.runtime_value(cell.value(binding.name())?)
            }
            PropertyNumericRhs::BindingBitAndLiteral {
                binding,
                cell,
                mask,
            } => {
                let value = self.runtime_value(cell.value(binding.name())?)?;
                self.eval_bytecode_number_binary(
                    BytecodeNumericBinaryOp::BitAnd,
                    &value,
                    &Value::Number(*mask),
                )
            }
            PropertyNumericRhs::StaticPropertyBitAndLiteral {
                object,
                object_cell,
                property,
                mask,
            } => {
                let object = self.runtime_value(object_cell.value(object.name())?)?;
                let value =
                    self.get_static_property_value(&object, property.name(), property.access())?;
                self.eval_bytecode_number_binary(
                    BytecodeNumericBinaryOp::BitAnd,
                    &value,
                    &Value::Number(*mask),
                )
            }
            PropertyNumericRhs::DynamicArrayElement {
                array,
                array_cell,
                index,
                index_cell,
                index_mask,
                property,
            } => {
                let object = self.runtime_value(array_cell.value(array.name())?)?;
                let index = self.eval_property_index_value(index, index_cell, *index_mask)?;
                if let Some(value) = self.eval_dynamic_array_index_member(&object, &index)? {
                    return Ok(value);
                }
                let key = self.dynamic_property_key(&index)?;
                self.get_cached_dynamic_property_value(&object, &key, property.access())
            }
        }
    }

    fn eval_property_index_value(
        &mut self,
        index: &BytecodeBinding,
        index_cell: &BindingCell,
        index_mask: Option<f64>,
    ) -> Result<Value> {
        let value = self.runtime_value(index_cell.value(index.name())?)?;
        if let Some(mask) = index_mask {
            return self.eval_bytecode_number_binary(
                BytecodeNumericBinaryOp::BitAnd,
                &value,
                &Value::Number(mask),
            );
        }
        Ok(value)
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
