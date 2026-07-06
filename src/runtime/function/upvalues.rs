use std::rc::Rc;

use crate::{
    ast::{StaticBinding, StaticFunctionId},
    binding_layout::BindingLayout,
    binding_layout::{BindingOperand, FunctionScopeId},
    error::{Error, Result},
    runtime::Context,
    runtime::binding::scope::BindingCell,
};

impl Context {
    pub(super) fn capture_function_upvalues(
        &self,
        id: StaticFunctionId,
        capture_bindings: &[StaticBinding],
        layout: Option<&BindingLayout>,
    ) -> Result<super::super::CapturedFunctionUpvalues> {
        let Some(layout) = layout else {
            return Ok(super::super::CapturedFunctionUpvalues::new(
                Rc::from(Vec::new().into_boxed_slice()),
                true,
            ));
        };
        let Some(function) = layout.function_for_static_id(id)? else {
            return Ok(super::super::CapturedFunctionUpvalues::new(
                Rc::from(Vec::new().into_boxed_slice()),
                true,
            ));
        };
        let expected_cell_count = layout.upvalue_count_for_function(function)?;
        let mut collector = UpvalueCollector::new(self, layout, function, expected_cell_count);
        collector.collect_bindings(capture_bindings)?;
        Ok(collector.finish())
    }
}

struct UpvalueCollector<'a> {
    context: &'a Context,
    layout: &'a BindingLayout,
    function: FunctionScopeId,
    expected_cell_count: usize,
    cells: Vec<Option<BindingCell>>,
    needs_legacy_scope_fallback: bool,
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
            needs_legacy_scope_fallback: false,
        }
    }

    fn finish(mut self) -> super::super::CapturedFunctionUpvalues {
        self.cells.resize_with(self.expected_cell_count, || None);
        let needs_legacy_scope_fallback =
            self.needs_legacy_scope_fallback || self.cells.iter().any(Option::is_none);
        super::super::CapturedFunctionUpvalues::new(
            Rc::from(self.cells.into_boxed_slice()),
            needs_legacy_scope_fallback,
        )
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
        let Some(declaration) = self.layout.upvalue_declaration(function, slot)? else {
            self.needs_legacy_scope_fallback = true;
            return Ok(());
        };
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
        if cell.is_none() {
            self.needs_legacy_scope_fallback = true;
        }
        *target = cell;
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
