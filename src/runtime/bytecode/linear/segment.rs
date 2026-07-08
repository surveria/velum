use crate::{
    bytecode::{BytecodeAddress, BytecodeBlock},
    error::{Error, Result},
    runtime::{Context, control::Completion, control::runtime_exception_value},
    value::Value,
};

use super::{BytecodeLinearOp, BytecodeState};

#[derive(Debug)]
pub(in crate::runtime::bytecode) struct BytecodeLinearPlan<'a> {
    segments: Vec<BytecodeLinearSegment<'a>>,
    entry_by_pc: Vec<Option<usize>>,
}

#[derive(Debug)]
struct BytecodeLinearSegment<'a> {
    start: usize,
    end: usize,
    ops: Vec<BytecodeLinearOp<'a>>,
}

impl Context {
    pub(in crate::runtime::bytecode) fn compile_bytecode_linear_plan<'a>(
        &mut self,
        block: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeLinearPlan<'a>>> {
        let instructions = block.instructions();
        let mut builder = BytecodeLinearPlanBuilder::new(instructions.len());
        let mut ops = Vec::new();
        let mut segment_start = None;
        let mut index = 0;

        while index < instructions.len() {
            if let Some((op, consumed)) =
                self.compile_bytecode_linear_peephole(instructions, index)?
            {
                ensure_positive_consumed(consumed)?;
                if segment_start.is_none() {
                    segment_start = Some(index);
                }
                ops.push(op);
                index = checked_segment_end(index, consumed)?;
                continue;
            }

            let Some(instruction) = instructions.get(index) else {
                return Err(Error::runtime("bytecode instruction index is not defined"));
            };
            if let Some(op) = self.compile_bytecode_linear_op(instruction)? {
                if segment_start.is_none() {
                    segment_start = Some(index);
                }
                ops.push(op);
                index = checked_segment_end(index, 1)?;
                continue;
            }

            builder.flush(segment_start.take(), index, &mut ops)?;
            index = checked_segment_end(index, 1)?;
        }

        builder.flush(segment_start, instructions.len(), &mut ops)?;
        builder.finish()
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_block_with_linear_plan(
        &mut self,
        block: &BytecodeBlock,
        plan: Option<&BytecodeLinearPlan<'_>>,
        state: &mut BytecodeState,
    ) -> Result<Completion> {
        let Some(plan) = plan else {
            return self.eval_bytecode_block_with_state(block, state);
        };
        if let Some(segment) = plan.single_full_block_segment(block) {
            return self.eval_bytecode_linear_full_block(segment, state);
        }
        self.eval_bytecode_segmented_plan(block, plan, state)
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_expression_with_plan(
        &mut self,
        block: &BytecodeBlock,
        plan: Option<&BytecodeLinearPlan<'_>>,
        state: &mut BytecodeState,
    ) -> Result<Value> {
        if let Some(value) = self.eval_bytecode_linear_direct_expression(block, plan)? {
            return Ok(value);
        }
        self.eval_bytecode_block_with_linear_plan(block, plan, state)?
            .into_result()
    }

    fn eval_bytecode_segmented_plan(
        &mut self,
        block: &BytecodeBlock,
        plan: &BytecodeLinearPlan<'_>,
        state: &mut BytecodeState,
    ) -> Result<Completion> {
        state.reset();
        while let Some(instruction) = block.instruction(state.pc)? {
            if let Some(segment) = plan.segment_at(state.pc.index()) {
                if let Some(completion) = self.eval_bytecode_linear_segment(segment, state)? {
                    return Ok(completion);
                }
                continue;
            }

            self.step()?;
            let completion = match self.eval_bytecode_instruction(state, instruction) {
                Ok(completion) => completion,
                Err(error) => {
                    if let Some(value) = runtime_exception_value(&error) {
                        self.checked_value(value.clone())?;
                        Some(Completion::Throw(value))
                    } else {
                        return Err(error);
                    }
                }
            };
            if let Some(completion) = completion {
                return Ok(completion);
            }
        }
        Ok(Completion::Normal(state.last.clone()))
    }

    fn eval_bytecode_linear_full_block(
        &mut self,
        segment: &BytecodeLinearSegment<'_>,
        state: &mut BytecodeState,
    ) -> Result<Completion> {
        state.reset();
        if let Some(completion) = self.eval_bytecode_linear_segment(segment, state)? {
            return Ok(completion);
        }
        Ok(Completion::Normal(state.last.clone()))
    }

    fn eval_bytecode_linear_segment(
        &mut self,
        segment: &BytecodeLinearSegment<'_>,
        state: &mut BytecodeState,
    ) -> Result<Option<Completion>> {
        self.record_bytecode_linear_segment_run()?;
        for op in &segment.ops {
            self.step()?;
            if let Err(error) = self.eval_bytecode_linear_op(state, op) {
                if let Some(value) = runtime_exception_value(&error) {
                    self.checked_value(value.clone())?;
                    return Ok(Some(Completion::Throw(value)));
                }
                return Err(error);
            }
        }
        state.pc = BytecodeAddress::new(segment.end);
        Ok(None)
    }
}

impl<'a> BytecodeLinearPlan<'a> {
    fn new(block_len: usize, segments: Vec<BytecodeLinearSegment<'a>>) -> Result<Option<Self>> {
        if segments.is_empty() {
            return Ok(None);
        }

        let mut entry_by_pc = vec![None; block_len];
        for (segment_index, segment) in segments.iter().enumerate() {
            let Some(entry) = entry_by_pc.get_mut(segment.start) else {
                return Err(Error::runtime(
                    "bytecode linear segment start escaped block",
                ));
            };
            if entry.is_some() {
                return Err(Error::runtime("bytecode linear segment start duplicated"));
            }
            *entry = Some(segment_index);
        }

        Ok(Some(Self {
            segments,
            entry_by_pc,
        }))
    }

    fn segment_at(&self, pc: usize) -> Option<&BytecodeLinearSegment<'a>> {
        let segment_index = self.entry_by_pc.get(pc).copied().flatten()?;
        self.segments.get(segment_index)
    }

    pub(super) fn single_full_block_op(
        &self,
        block: &BytecodeBlock,
    ) -> Option<&BytecodeLinearOp<'a>> {
        let segment = self.single_full_block_segment(block)?;
        if segment.ops.len() == 1 {
            return segment.ops.first();
        }
        None
    }

    fn single_full_block_segment(
        &self,
        block: &BytecodeBlock,
    ) -> Option<&BytecodeLinearSegment<'a>> {
        let segment = self.segments.first()?;
        if self.segments.len() == 1
            && segment.start == 0
            && segment.end == block.instructions().len()
        {
            return Some(segment);
        }
        None
    }
}

struct BytecodeLinearPlanBuilder<'a> {
    block_len: usize,
    segments: Vec<BytecodeLinearSegment<'a>>,
}

impl<'a> BytecodeLinearPlanBuilder<'a> {
    const fn new(block_len: usize) -> Self {
        Self {
            block_len,
            segments: Vec::new(),
        }
    }

    fn flush(
        &mut self,
        start: Option<usize>,
        end: usize,
        ops: &mut Vec<BytecodeLinearOp<'a>>,
    ) -> Result<()> {
        let Some(start) = start else {
            ensure_empty_ops(ops)?;
            return Ok(());
        };
        let instruction_count = end
            .checked_sub(start)
            .ok_or_else(|| Error::runtime("bytecode linear segment end escaped start"))?;
        if keep_segment(start, end, self.block_len, instruction_count, ops.len()) {
            let segment_ops = std::mem::take(ops);
            self.segments.push(BytecodeLinearSegment {
                start,
                end,
                ops: segment_ops,
            });
            return Ok(());
        }
        ops.clear();
        Ok(())
    }

    fn finish(self) -> Result<Option<BytecodeLinearPlan<'a>>> {
        BytecodeLinearPlan::new(self.block_len, self.segments)
    }
}

const fn keep_segment(
    start: usize,
    end: usize,
    block_len: usize,
    instruction_count: usize,
    op_count: usize,
) -> bool {
    if start == 0 && end == block_len {
        return op_count > 0;
    }
    instruction_count >= 2 || instruction_count > op_count
}

fn ensure_empty_ops(ops: &[BytecodeLinearOp<'_>]) -> Result<()> {
    if ops.is_empty() {
        return Ok(());
    }
    Err(Error::runtime(
        "bytecode linear segment has ops without a start",
    ))
}

fn ensure_positive_consumed(consumed: usize) -> Result<()> {
    if consumed == 0 {
        return Err(Error::runtime("bytecode peephole consumed no instructions"));
    }
    Ok(())
}

fn checked_segment_end(index: usize, consumed: usize) -> Result<usize> {
    index
        .checked_add(consumed)
        .ok_or_else(|| Error::runtime("bytecode linear segment index overflowed"))
}
