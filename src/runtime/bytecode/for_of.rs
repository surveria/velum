use crate::{
    bytecode::{BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeForInTarget},
    error::{Error, Result},
    runtime::Context,
    runtime::abstract_operations::{IteratorSource, IteratorStep},
    runtime::binding::scope::{BindingCell, BindingScope},
    runtime::control::Completion,
    syntax::{DeclKind, StaticName},
    value::Value,
};

use super::state::{BytecodeState, bytecode_loop_completion};

impl Context {
    pub(super) fn eval_bytecode_for_of(
        &mut self,
        state: &mut BytecodeState,
        labels: Option<&[StaticName]>,
        target: &BytecodeForInTarget,
        object: &BytecodeBlock,
        body: &BytecodeBlock,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let iterable = self.eval_bytecode_expression(object)?;
        let mut source = self.get_iterator(iterable)?;
        let completion = match target {
            BytecodeForInTarget::Binding {
                name,
                kind: kind @ (DeclKind::Let | DeclKind::Const),
            } => self.eval_for_of_lexical_binding(name, *kind, &mut source, body, labels)?,
            BytecodeForInTarget::Binding {
                name,
                kind: DeclKind::Var,
            } => {
                self.eval_for_of_assignment_loop(&mut source, body, labels, |context, value| {
                    context.assign_bytecode(name, value)
                })?
            }
            BytecodeForInTarget::PatternBinding { pattern, kind } => {
                self.eval_for_of_pattern_loop(&mut source, pattern, *kind, body, labels)?
            }
            BytecodeForInTarget::Assignment(target) => {
                self.eval_for_of_assignment_loop(&mut source, body, labels, |context, value| {
                    context.assign_bytecode_target(target, value)
                })?
            }
        };
        Ok(Self::store_or_return_completion(state, completion, next))
    }

    fn eval_for_of_lexical_binding(
        &mut self,
        name: &BytecodeBinding,
        kind: DeclKind,
        source: &mut IteratorSource,
        body: &BytecodeBlock,
        labels: Option<&[StaticName]>,
    ) -> Result<Completion> {
        let mut last = Value::Undefined;
        self.ensure_extra_binding_capacity(0)?;
        let atom = self.intern_static_name_atom(name.name().name())?;
        let frame = self.compiled_local_binding_frame(name.name())?;
        let mutable = kind != DeclKind::Const;
        let mut scope = BindingScope::new();
        loop {
            self.step()?;
            let value = match self.iterator_step(source)? {
                IteratorStep::Value(value) => value,
                IteratorStep::Done => break,
                IteratorStep::Abrupt(completion) => return Ok(completion),
            };
            let inserted = match scope.insert_or_replace_at_optional_slot(
                atom,
                BindingCell::new(value, mutable, kind),
                frame.map(crate::runtime::CompiledBindingFrame::slot),
            ) {
                Ok(inserted) => inserted,
                Err(error) => return Err(self.iterator_close_on_error(source, error)),
            };
            if let Some(frame) = frame
                && let Err(error) = Self::mark_binding_scope_frame_slot(&mut scope, frame, inserted)
            {
                return Err(self.iterator_close_on_error(source, error));
            }
            self.push_lexical_scope_with(scope);
            if let Err(error) = self.remember_active_static_binding(name.name(), atom) {
                if self.pop_lexical_scope().is_none() {
                    return Err(Error::runtime(
                        "bytecode for-of lexical scope disappeared after binding failure",
                    ));
                }
                return Err(self.iterator_close_on_error(source, error));
            }
            let completion = self.eval_bytecode_block(body);
            let Some(removed_scope) = self.pop_lexical_scope() else {
                let error = Error::runtime("bytecode for-of lexical scope disappeared");
                return Err(self.iterator_close_on_error(source, error));
            };
            scope = removed_scope;
            let completion = match completion {
                Ok(completion) => completion,
                Err(error) => return Err(self.iterator_close_on_error(source, error)),
            };
            if let Some(completion) = bytecode_loop_completion(&mut last, completion, labels) {
                return self.iterator_close(source, completion);
            }
        }
        Ok(Completion::Normal(last))
    }

    fn eval_for_of_assignment_loop(
        &mut self,
        source: &mut IteratorSource,
        body: &BytecodeBlock,
        labels: Option<&[StaticName]>,
        mut assign: impl FnMut(&mut Self, Value) -> Result<()>,
    ) -> Result<Completion> {
        let mut last = Value::Undefined;
        loop {
            self.step()?;
            let value = match self.iterator_step(source)? {
                IteratorStep::Value(value) => value,
                IteratorStep::Done => break,
                IteratorStep::Abrupt(completion) => return Ok(completion),
            };
            if let Err(error) = assign(self, value) {
                return Err(self.iterator_close_on_error(source, error));
            }
            let completion = match self.eval_bytecode_block(body) {
                Ok(completion) => completion,
                Err(error) => return Err(self.iterator_close_on_error(source, error)),
            };
            if let Some(completion) = bytecode_loop_completion(&mut last, completion, labels) {
                return self.iterator_close(source, completion);
            }
        }
        Ok(Completion::Normal(last))
    }
}
