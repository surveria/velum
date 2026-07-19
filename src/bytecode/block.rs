#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::rc::Rc;

use crate::{
    SourceId, SourceSpan,
    error::{Error, Result},
};

use super::{BytecodeAddress, BytecodeInstruction, BytecodeLinearTemplate};

/// One instruction and its canonical source range.
pub struct BytecodeStep<'a> {
    instruction: &'a BytecodeInstruction,
    span: SourceSpan,
}

impl<'a> BytecodeStep<'a> {
    pub(crate) const fn new(instruction: &'a BytecodeInstruction, span: SourceSpan) -> Self {
        Self { instruction, span }
    }

    pub(crate) const fn instruction(&self) -> &'a BytecodeInstruction {
        self.instruction
    }

    pub(crate) const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// Executable instructions plus an instruction-aligned source-range table.
#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeBlock {
    instructions: Rc<[BytecodeInstruction]>,
    spans: Rc<[SourceSpan]>,
    linear_template: BytecodeLinearTemplate,
}

impl BytecodeBlock {
    pub(crate) fn from_parts(
        instructions: Vec<BytecodeInstruction>,
        spans: Vec<SourceSpan>,
    ) -> Result<Self> {
        if instructions.len() != spans.len() {
            return Err(Error::runtime(
                "bytecode instruction and source span counts differ",
            ));
        }
        if spans
            .iter()
            .any(|span| span.source_id() == SourceId::UNKNOWN)
        {
            return Err(Error::runtime(
                "bytecode instruction has an unknown source identity",
            ));
        }
        if let Some(first) = spans.first()
            && spans
                .iter()
                .any(|span| span.source_id() != first.source_id())
        {
            return Err(Error::runtime(
                "bytecode block spans multiple source identities",
            ));
        }
        let linear_template = BytecodeLinearTemplate::compile(&instructions)?;
        Ok(Self {
            instructions: Rc::from(instructions.into_boxed_slice()),
            spans: Rc::from(spans.into_boxed_slice()),
            linear_template,
        })
    }

    pub(crate) fn step(&self, address: BytecodeAddress) -> Result<Option<BytecodeStep<'_>>> {
        let index = address.index();
        if index == self.instructions.len() {
            return Ok(None);
        }
        if index > self.instructions.len() {
            return Err(Error::runtime(
                "bytecode instruction pointer escaped program",
            ));
        }
        let instruction = self
            .instructions
            .get(index)
            .ok_or_else(|| Error::runtime("bytecode instruction is not available"))?;
        let span = self
            .spans
            .get(index)
            .copied()
            .ok_or_else(|| Error::runtime("bytecode source span is not available"))?;
        Ok(Some(BytecodeStep::new(instruction, span)))
    }

    pub(crate) fn source_span(&self, address: BytecodeAddress) -> Result<Option<SourceSpan>> {
        self.step(address).map(|step| step.map(|step| step.span()))
    }

    pub(crate) fn instructions(&self) -> &[BytecodeInstruction] {
        &self.instructions
    }

    pub(crate) const fn linear_template(&self) -> &BytecodeLinearTemplate {
        &self.linear_template
    }
}
