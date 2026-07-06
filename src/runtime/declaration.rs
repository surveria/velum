use crate::{
    ast::{DeclKind, StaticBinding},
    atom::AtomId,
    bytecode::BytecodeHoistPlan,
    error::{Error, Result},
    runtime::Context,
    runtime::scope::{BindingCell, BindingScope},
    value::Value,
};

use super::static_bindings::CompiledBindingFrame;

impl Context {
    pub(crate) fn hoist_bytecode_var_declarations(
        &mut self,
        plan: &BytecodeHoistPlan,
    ) -> Result<()> {
        for binding in plan.var_declarations() {
            self.hoist_var(binding)?;
        }
        Ok(())
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
        self.locals.iter().try_fold(global_count, |count, scope| {
            count
                .checked_add(scope.len())
                .ok_or_else(|| Error::limit("binding count overflowed"))
        })
    }

    fn active_bindings(&self) -> &BindingScope {
        if let Some(scope) = self.locals.last() {
            return scope;
        }
        &self.globals
    }

    pub(crate) fn active_bindings_mut(&mut self) -> &mut BindingScope {
        if let Some(scope) = self.locals.last_mut() {
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
            .rev()
            .find_map(|scope| scope.get(atom))
            .or_else(|| self.globals.get(atom))
            .or_else(|| self.builtin_globals.get(atom))
    }
}
