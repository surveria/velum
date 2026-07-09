use crate::{
    bytecode::{
        BytecodeBinding, BytecodeBlock, BytecodeCompletion, BytecodeInstruction,
        BytecodeNumericBinaryOp,
    },
    error::Result,
    runtime::{Context, binding::scope::BindingCell, numeric::number_to_i32},
    value::Value,
};

#[derive(Debug)]
pub(super) struct BytecodeForSwitchFastPath<'a> {
    pub(super) discriminant: &'a BytecodeBinding,
    discriminant_cell: BindingCell,
    mask_i32: i32,
    pub(super) target: &'a BytecodeBinding,
    pub(super) target_cell: BindingCell,
    array: &'a BytecodeBinding,
    array_cell: BindingCell,
    cases: Vec<BytecodeForSwitchCase>,
    default_index: Option<usize>,
}

#[derive(Debug)]
struct BytecodeForSwitchCase {
    test: Option<f64>,
    array_index: usize,
}

#[derive(Debug)]
struct ParsedSwitchCase<'a> {
    target: &'a BytecodeBinding,
    array: &'a BytecodeBinding,
    array_index: usize,
    breaks: bool,
}

impl Context {
    pub(super) fn compile_bytecode_for_switch_fast_path<'a>(
        &mut self,
        index: &'a BytecodeBinding,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeForSwitchFastPath<'a>>> {
        let [
            BytecodeInstruction::Switch {
                discriminant,
                cases,
                scoped: false,
            },
        ] = body.instructions()
        else {
            return Ok(None);
        };
        let [
            BytecodeInstruction::LoadBinding(discriminant_binding),
            BytecodeInstruction::PushLiteral(Value::Number(mask)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::StoreLast,
        ] = discriminant.instructions()
        else {
            return Ok(None);
        };
        if !super::for_loop::same_bytecode_binding(index, discriminant_binding) {
            return Ok(None);
        }
        let Ok(mask_i32) = number_to_i32(*mask, "&") else {
            return Ok(None);
        };
        let mut switch_cases = Vec::with_capacity(cases.len());
        let mut default_index = None;
        let mut target = None;
        let mut array = None;
        for (case_index, case) in cases.iter().enumerate() {
            let test = if let Some(test) = &case.test {
                let Some(value) = super::bytecode_numeric_switch_case_test(test) else {
                    return Ok(None);
                };
                Some(value)
            } else {
                default_index = Some(case_index);
                None
            };
            let Some(parsed) = Self::compile_bytecode_for_switch_case_fast_path(&case.body)? else {
                return Ok(None);
            };
            if case_index + 1 < cases.len() && !parsed.breaks {
                return Ok(None);
            }
            match target {
                Some(existing)
                    if !super::for_loop::same_bytecode_binding(existing, parsed.target) =>
                {
                    return Ok(None);
                }
                Some(_) => {}
                None => target = Some(parsed.target),
            }
            match array {
                Some(existing)
                    if !super::for_loop::same_bytecode_binding(existing, parsed.array) =>
                {
                    return Ok(None);
                }
                Some(_) => {}
                None => array = Some(parsed.array),
            }
            switch_cases.push(BytecodeForSwitchCase {
                test,
                array_index: parsed.array_index,
            });
        }
        let (Some(target), Some(array)) = (target, array) else {
            return Ok(None);
        };
        let Some(discriminant_cell) = self.get_binding_bytecode(discriminant_binding)? else {
            return Ok(None);
        };
        let Some(target_cell) = self.get_or_materialize_binding_bytecode(target)? else {
            return Ok(None);
        };
        let Some(array_cell) = self.get_binding_bytecode(array)? else {
            return Ok(None);
        };
        Ok(Some(BytecodeForSwitchFastPath {
            discriminant: discriminant_binding,
            discriminant_cell,
            mask_i32,
            target,
            target_cell,
            array,
            array_cell,
            cases: switch_cases,
            default_index,
        }))
    }

    fn compile_bytecode_for_switch_case_fast_path(
        body: &BytecodeBlock,
    ) -> Result<Option<ParsedSwitchCase<'_>>> {
        match body.instructions() {
            [
                BytecodeInstruction::LoadBinding(target_read),
                BytecodeInstruction::LoadBinding(array),
                BytecodeInstruction::ArrayIndexMember { index, .. },
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
                BytecodeInstruction::StoreBinding(target_write),
                BytecodeInstruction::StoreLast,
                BytecodeInstruction::Complete(BytecodeCompletion::Break(None)),
            ] if super::for_loop::same_bytecode_binding(target_read, target_write) => {
                Ok(Some(ParsedSwitchCase {
                    target: target_write,
                    array,
                    array_index: index.index()?,
                    breaks: true,
                }))
            }
            [
                BytecodeInstruction::LoadBinding(target_read),
                BytecodeInstruction::LoadBinding(array),
                BytecodeInstruction::ArrayIndexMember { index, .. },
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
                BytecodeInstruction::StoreBinding(target_write),
                BytecodeInstruction::StoreLast,
            ] if super::for_loop::same_bytecode_binding(target_read, target_write) => {
                Ok(Some(ParsedSwitchCase {
                    target: target_write,
                    array,
                    array_index: index.index()?,
                    breaks: false,
                }))
            }
            _ => Ok(None),
        }
    }

    pub(super) fn bytecode_for_switch_fast_path_ready(
        &self,
        fast_path: &BytecodeForSwitchFastPath<'_>,
    ) -> Result<bool> {
        if !matches!(
            fast_path
                .discriminant_cell
                .value(fast_path.discriminant.name())?,
            Value::Number(_)
        ) || !matches!(
            fast_path.target_cell.value(fast_path.target.name())?,
            Value::Number(_)
        ) {
            return Ok(false);
        }
        let Some(values) = self.switch_loop_numeric_array_values(fast_path)? else {
            return Ok(false);
        };
        for case in &fast_path.cases {
            if values.get(case.array_index).is_none() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn eval_bytecode_for_switch_fast_path(
        &self,
        fast_path: &BytecodeForSwitchFastPath<'_>,
        array_values: Option<&[f64]>,
    ) -> Result<Value> {
        let Some(array_values) = array_values else {
            return Ok(Value::Undefined);
        };
        let Some(case_index) = Self::bytecode_for_switch_case_index(fast_path)? else {
            return Ok(Value::Undefined);
        };
        let Some(case) = fast_path.cases.get(case_index) else {
            return Ok(Value::Undefined);
        };
        let Some(element) = array_values.get(case.array_index).copied() else {
            return Ok(Value::Undefined);
        };
        let Value::Number(total) = fast_path.target_cell.value(fast_path.target.name())? else {
            return Ok(Value::Undefined);
        };
        let value = self.checked_value(Value::Number(total + element))?;
        fast_path
            .target_cell
            .assign(fast_path.target.name(), value.clone())?;
        Ok(value)
    }

    pub(super) fn bytecode_for_switch_element(
        fast_path: &BytecodeForSwitchFastPath<'_>,
        array_values: &[f64],
        discriminant: f64,
    ) -> Result<Option<f64>> {
        let Some(case_index) =
            Self::bytecode_for_switch_case_index_for_number(fast_path, discriminant)?
        else {
            return Ok(None);
        };
        let Some(case) = fast_path.cases.get(case_index) else {
            return Ok(None);
        };
        Ok(array_values.get(case.array_index).copied())
    }

    fn bytecode_for_switch_case_index(
        fast_path: &BytecodeForSwitchFastPath<'_>,
    ) -> Result<Option<usize>> {
        let Value::Number(discriminant) = fast_path
            .discriminant_cell
            .value(fast_path.discriminant.name())?
        else {
            return Ok(None);
        };
        Self::bytecode_for_switch_case_index_for_number(fast_path, discriminant)
    }

    fn bytecode_for_switch_case_index_for_number(
        fast_path: &BytecodeForSwitchFastPath<'_>,
        discriminant: f64,
    ) -> Result<Option<usize>> {
        let masked = f64::from(number_to_i32(discriminant, "&")? & fast_path.mask_i32);
        for (index, case) in fast_path.cases.iter().enumerate() {
            let Some(test) = case.test else {
                continue;
            };
            if super::bytecode_switch_number_equal(test, masked) {
                return Ok(Some(index));
            }
        }
        Ok(fast_path.default_index)
    }

    pub(super) fn switch_loop_numeric_array_values(
        &self,
        fast_path: &BytecodeForSwitchFastPath<'_>,
    ) -> Result<Option<Vec<f64>>> {
        let Value::Object(id) = fast_path.array_cell.value(fast_path.array.name())? else {
            return Ok(None);
        };
        let Some(values) = self.objects.packed_array_values_if_array(id)? else {
            return Ok(None);
        };
        let mut numbers = Vec::with_capacity(values.len());
        for value in values {
            let Value::Number(number) = value else {
                return Ok(None);
            };
            numbers.push(number);
        }
        Ok(Some(numbers))
    }
}
