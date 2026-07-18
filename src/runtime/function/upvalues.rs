use alloc::rc::Rc;

use crate::{
    binding_metadata::BindingLayout,
    binding_metadata::{BindingOperand, FunctionScopeId, UpvalueSlot},
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
        let mut collector = UpvalueCollector::new(self, layout, id, function, expected_cell_count);
        collector.collect_bindings(capture_bindings)?;
        collector.finish(capture_bindings)
    }
}

struct UpvalueCollector<'a> {
    context: &'a Context,
    layout: &'a BindingLayout,
    static_function: StaticFunctionId,
    function: FunctionScopeId,
    expected_cell_count: usize,
    cells: Vec<Option<BindingCell>>,
}

impl<'a> UpvalueCollector<'a> {
    const fn new(
        context: &'a Context,
        layout: &'a BindingLayout,
        static_function: StaticFunctionId,
        function: FunctionScopeId,
        expected_cell_count: usize,
    ) -> Self {
        Self {
            context,
            layout,
            static_function,
            function,
            expected_cell_count,
            cells: Vec::new(),
        }
    }

    fn finish(
        mut self,
        bindings: &[StaticBinding],
    ) -> Result<super::super::CapturedFunctionUpvalues> {
        self.cells.resize_with(self.expected_cell_count, || None);
        let mut cells = Vec::with_capacity(self.cells.len());
        for (index, cell) in self.cells.into_iter().enumerate() {
            let Some(cell) = cell else {
                let slot = UpvalueSlot::from_index(index)?;
                let declaration = self.layout.upvalue_declaration(self.function, slot)?;
                let mut candidate = None;
                if let Some(declaration) = declaration {
                    for binding in bindings {
                        let Some(BindingOperand::Upvalue { function, slot }) =
                            self.layout.operand_for_binding_id(binding.id())?
                        else {
                            continue;
                        };
                        if self.layout.upvalue_declaration(function, slot)? == Some(declaration) {
                            candidate = Some(binding);
                            break;
                        }
                    }
                }
                let name = candidate.map_or("<transitive>", StaticBinding::as_str);
                let operand = candidate
                    .map(|binding| self.layout.operand_for_binding_id(binding.id()))
                    .transpose()?
                    .flatten();
                return Err(Error::runtime(format!(
                    "compiled function {:?} upvalue layout did not resolve captured cell {index} ('{name}', operand {operand:?})",
                    self.static_function,
                )));
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
        if self.cells.len() < required_len {
            self.cells.resize_with(required_len, || None);
        }
        Ok(())
    }
}
