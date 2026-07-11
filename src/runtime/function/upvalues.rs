use std::rc::Rc;

use crate::{
    binding_metadata::BindingLayout,
    binding_metadata::{BindingOperand, FunctionScopeId},
    error::{Error, Result},
    runtime::Context,
    runtime::binding::scope::BindingCell,
    syntax::{StaticBinding, StaticFunctionId},
};

impl Context {
    pub(super) fn capture_function_upvalues(
        &self,
        id: StaticFunctionId,
        capture_bindings: &[StaticBinding],
        layout: Option<&BindingLayout>,
    ) -> Result<super::super::CapturedFunctionUpvalues> {
        let layout = layout.ok_or_else(|| {
            Error::runtime("compiled function cannot capture upvalues without a binding layout")
        })?;
        let function = layout.function_for_static_id(id)?.ok_or_else(|| {
            Error::runtime("compiled function is missing from the binding layout")
        })?;
        let expected_cell_count = layout.upvalue_count_for_function(function)?;
        let mut collector = UpvalueCollector::new(self, layout, function, expected_cell_count);
        collector.collect_bindings(capture_bindings)?;
        collector.finish()
    }
}

struct UpvalueCollector<'a> {
    context: &'a Context,
    layout: &'a BindingLayout,
    function: FunctionScopeId,
    expected_cell_count: usize,
    cells: Vec<Option<BindingCell>>,
}

impl<'a> UpvalueCollector<'a> {
    const fn new(
        context: &'a Context,
        layout: &'a BindingLayout,
        function: FunctionScopeId,
        expected_cell_count: usize,
    ) -> Self {
        Self {
            context,
            layout,
            function,
            expected_cell_count,
            cells: Vec::new(),
        }
    }

    fn finish(mut self) -> Result<super::super::CapturedFunctionUpvalues> {
        self.cells.resize_with(self.expected_cell_count, || None);
        let mut cells = Vec::with_capacity(self.cells.len());
        for cell in self.cells {
            let Some(cell) = cell else {
                return Err(Error::runtime(
                    "compiled function upvalue layout did not resolve every captured cell",
                ));
            };
            cells.push(cell);
        }
        Ok(super::super::CapturedFunctionUpvalues::new(Rc::from(
            cells.into_boxed_slice(),
        )))
    }

    fn collect_bindings(&mut self, bindings: &[StaticBinding]) -> Result<()> {
        for binding in bindings {
            self.capture_binding(binding)?;
        }
        Ok(())
    }

    fn capture_binding(&mut self, binding: &StaticBinding) -> Result<()> {
        let Some(BindingOperand::Upvalue { function, slot }) =
            self.layout.operand_for_binding_id(binding.id())?
        else {
            return Ok(());
        };
        let declaration = self
            .layout
            .upvalue_declaration(function, slot)?
            .ok_or_else(|| Error::runtime("compiled upvalue declaration is not defined"))?;
        let Some(current_slot) = self
            .layout
            .upvalue_slot_for_declaration(self.function, declaration)?
        else {
            return Ok(());
        };
        let index = current_slot.index()?;
        self.ensure_cell_slot(index)?;
        let Some(target) = self.cells.get_mut(index) else {
            return Err(Error::runtime("upvalue cell slot is not defined"));
        };
        if target.is_some() {
            return Ok(());
        }
        let cell = self.context.resolve_runtime_static_declaration(
            self.layout,
            self.function,
            declaration,
            binding,
        )?;
        let cell = cell.ok_or_else(|| {
            Error::runtime(format!(
                "compiled upvalue declaration '{}' did not resolve to a runtime cell",
                binding.as_str()
            ))
        })?;
        *target = Some(cell);
        Ok(())
    }

    fn ensure_cell_slot(&mut self, index: usize) -> Result<()> {
        let required_len = index
            .checked_add(1)
            .ok_or_else(|| Error::limit("upvalue cell slot count overflowed"))?;
        self.cells.resize_with(required_len, || None);
        Ok(())
    }
}
