use crate::{
    bytecode::{BytecodeBinding, BytecodeHoistPlan, BytecodeNewTargetMode},
    error::{Error, Result},
    runtime::Context,
    runtime::binding::scope::{BindingCell, BindingScope},
    runtime::function::BytecodeFunctionInit,
    runtime::object::{PropertyConfigurable, PropertyEnumerable, PropertyWritable},
    storage::atom::AtomId,
    syntax::{DeclKind, StaticBinding},
    value::Value,
};

use super::static_bindings::CompiledBindingFrame;

impl Context {
    pub(crate) fn assign_bytecode_or_create_sloppy_global(
        &mut self,
        binding: &BytecodeBinding,
        value: Value,
    ) -> Result<()> {
        if let Some(cell) = self.get_or_materialize_binding_bytecode(binding)? {
            return self.assign_bytecode_cell(binding, &cell, value);
        }
        if binding.strict_write() {
            return Err(crate::runtime::control::reference_error_undefined(
                binding.name(),
            ));
        }

        let value = self.checked_value(value)?;
        let atom = self.intern_static_name_atom(binding.name().name())?;
        self.ensure_binding_capacity_for_atom(atom)?;
        self.globals
            .insert(atom, BindingCell::new(value.clone(), true, DeclKind::Var))?;
        self.remember_active_static_binding(binding.name(), atom)?;
        if let Some(global_object) = self.global_object {
            self.define_global_object_data_property(
                global_object,
                binding.name().name(),
                value,
                PropertyWritable::Yes,
                PropertyEnumerable::Yes,
                PropertyConfigurable::Yes,
            )?;
        }
        Ok(())
    }

    pub(crate) fn hoist_bytecode_declarations(&mut self, plan: &BytecodeHoistPlan) -> Result<()> {
        for binding in plan.var_declarations() {
            self.hoist_var(binding)?;
        }
        for declaration in plan.function_declarations() {
            self.hoist_function(declaration)?;
        }
        Ok(())
    }

    fn hoist_function(
        &mut self,
        declaration: &crate::bytecode::BytecodeFunctionDeclaration,
    ) -> Result<()> {
        self.hoist_var(declaration.name().name())?;
        let function = self.create_bytecode_function(&BytecodeFunctionInit {
            static_function_id: declaration.id(),
            name: Some(declaration.function_name()),
            bytecode: declaration.bytecode(),
            constructable: !declaration.is_async(),
            is_async: declaration.is_async(),
            class_constructor: false,
            prototype_parent: None,
            new_target_mode: BytecodeNewTargetMode::Own,
        })?;
        self.assign_bytecode(declaration.name(), function)
    }

    fn hoist_var(&mut self, name: &StaticBinding) -> Result<()> {
        let atom = self.intern_static_name_atom(name.name())?;
        if let Some(binding) = self.active_bindings().get(atom) {
            if binding.kind() == DeclKind::Var {
                self.remember_active_static_binding(name, atom)?;
                return Ok(());
            }
            return Err(Error::runtime(format!(
                "'{name}' has already been declared"
            )));
        }

        self.ensure_binding_capacity_for_atom(atom)?;
        let frame = self.compiled_active_binding_frame(name)?;
        let inserted = self
            .active_bindings_mut()
            .insert_or_replace_at_optional_slot(
                atom,
                BindingCell::new(Value::Undefined, true, DeclKind::Var),
                frame.map(CompiledBindingFrame::slot),
            )?;
        self.mark_active_binding_frame_slot(frame, inserted)?;
        self.remember_active_static_binding(name, atom)?;
        Ok(())
    }

    pub(crate) fn define(&mut self, name: &str, value: Value, kind: DeclKind) -> Result<()> {
        let atom = self.intern_atom(name)?;
        self.define_atom(atom, name, value, kind, None)
    }

    pub(crate) fn define_static(
        &mut self,
        name: &StaticBinding,
        value: Value,
        kind: DeclKind,
    ) -> Result<()> {
        let atom = self.intern_static_name_atom(name.name())?;
        let frame = self.compiled_active_binding_frame(name)?;
        self.define_atom(atom, name, value, kind, frame)?;
        self.remember_active_static_binding(name, atom)
    }

    fn define_atom(
        &mut self,
        atom: AtomId,
        name: &str,
        value: Value,
        kind: DeclKind,
        frame: Option<CompiledBindingFrame>,
    ) -> Result<()> {
        if self.active_bindings().contains(atom) {
            return Err(Error::runtime(format!(
                "'{name}' has already been declared"
            )));
        }
        self.ensure_binding_capacity_for_atom(atom)?;

        let value = self.runtime_value(value)?;
        let mutable = kind != DeclKind::Const;
        let inserted = self
            .active_bindings_mut()
            .insert_or_replace_at_optional_slot(
                atom,
                BindingCell::new(value, mutable, kind),
                frame.map(CompiledBindingFrame::slot),
            )?;
        self.mark_active_binding_frame_slot(frame, inserted)?;
        Ok(())
    }

    pub(crate) fn ensure_binding_capacity_static(
        &mut self,
        name: &StaticBinding,
    ) -> Result<AtomId> {
        let atom = self.intern_static_name_atom(name.name())?;
        self.ensure_binding_capacity_for_atom(atom)?;
        Ok(atom)
    }

    fn ensure_binding_capacity_for_atom(&self, atom: AtomId) -> Result<()> {
        if self.active_bindings().contains(atom) {
            return Ok(());
        }
        if self.binding_count()? >= self.limits.max_bindings {
            return Err(Error::limit(format!(
                "binding count exceeded {}",
                self.limits.max_bindings
            )));
        }
        Ok(())
    }

    pub(crate) fn ensure_extra_binding_capacity(&self, extra_bindings: usize) -> Result<()> {
        let projected = self
            .binding_count()?
            .checked_add(extra_bindings)
            .ok_or_else(|| Error::limit("binding count overflowed"))?;
        if projected >= self.limits.max_bindings {
            return Err(Error::limit(format!(
                "binding count exceeded {}",
                self.limits.max_bindings
            )));
        }
        Ok(())
    }

    fn binding_count(&self) -> Result<usize> {
        let global_count = self
            .globals
            .len()
            .checked_add(self.builtin_globals.len())
            .ok_or_else(|| Error::limit("binding count overflowed"))?;
        self.locals
            .iter()
            .skip(self.current_local_frame_start())
            .try_fold(global_count, |count, scope| {
                count
                    .checked_add(scope.len())
                    .ok_or_else(|| Error::limit("binding count overflowed"))
            })
    }

    fn active_bindings(&self) -> &BindingScope {
        if self.has_visible_local_scope()
            && let Some(scope) = self.locals.last()
        {
            return scope;
        }
        &self.globals
    }

    pub(crate) fn active_bindings_mut(&mut self) -> &mut BindingScope {
        if self.has_visible_local_scope()
            && let Some(scope) = self.locals.last_mut()
        {
            return scope;
        }
        &mut self.globals
    }

    pub(crate) fn get_binding(&self, name: &str) -> Option<BindingCell> {
        let atom = self.atom(name)?;
        self.get_binding_by_atom(atom)
    }

    pub(crate) fn get_binding_by_atom(&self, atom: AtomId) -> Option<BindingCell> {
        self.locals
            .iter()
            .skip(self.current_local_frame_start())
            .rev()
            .find_map(|scope| scope.get(atom))
            .or_else(|| self.globals.get(atom))
            .or_else(|| self.builtin_globals.get(atom))
    }
}
