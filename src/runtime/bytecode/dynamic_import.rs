use crate::{
    bytecode::{BytecodeAddress, BytecodeBlock},
    error::{Error, Result},
    runtime::{Context, control::Completion},
    syntax::ImportPhase,
    value::Value,
};

use super::{
    control_continuation::{
        BytecodeControlRecord, BytecodeControlStateSlot, BytecodeDynamicImportPhase,
    },
    state::BytecodeState,
};

impl Context {
    pub(super) fn eval_bytecode_dynamic_import_instruction(
        &mut self,
        state: &mut BytecodeState,
        phase: ImportPhase,
        specifier: &BytecodeBlock,
        options: Option<&BytecodeBlock>,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let handle = self.push_bytecode_control(BytecodeControlRecord::dynamic_import())?;
        let mut control = self.checkout_bytecode_control(handle)?;
        let current_phase = *control.dynamic_import_mut()?.0;
        if current_phase == BytecodeDynamicImportPhase::Specifier {
            let completion = self.run_bytecode_control_segment(
                handle,
                &mut control,
                BytecodeControlStateSlot::ImportSpecifier,
                |context, child| context.eval_bytecode_block_with_state(specifier, child),
            )?;
            if completion.suspends_execution() {
                self.park_bytecode_control(handle, control)?;
                return Ok(Some(completion));
            }
            let value = match completion {
                Completion::Normal(value) => value,
                completion => {
                    self.finish_bytecode_control(handle)?;
                    return Ok(Some(completion));
                }
            };
            let specifier = match self
                .run_bytecode_control_action(handle, &control, |context| context.to_string(&value))
            {
                Ok(specifier) => specifier,
                Err(error) => {
                    return self.finish_dynamic_import_error(state, next, &error);
                }
            };
            let (current_phase, stored_specifier) = control.dynamic_import_mut()?;
            *stored_specifier = Some(specifier);
            *current_phase = BytecodeDynamicImportPhase::Options;
        }

        let options = if let Some(options) = options {
            let completion = self.run_bytecode_control_segment(
                handle,
                &mut control,
                BytecodeControlStateSlot::ImportOptions,
                |context, child| context.eval_bytecode_block_with_state(options, child),
            )?;
            if completion.suspends_execution() {
                self.park_bytecode_control(handle, control)?;
                return Ok(Some(completion));
            }
            match completion {
                Completion::Normal(value) => value,
                completion => {
                    self.finish_bytecode_control(handle)?;
                    return Ok(Some(completion));
                }
            }
        } else {
            Value::Undefined
        };
        let specifier = control
            .dynamic_import_mut()?
            .1
            .take()
            .ok_or_else(|| Error::runtime("dynamic import specifier disappeared"))?;
        let promise = match self.run_bytecode_control_action(handle, &control, |context| {
            context.enqueue_dynamic_import(phase, specifier, &options)
        }) {
            Ok(promise) => {
                self.finish_bytecode_control(handle)?;
                promise
            }
            Err(error) => return self.finish_dynamic_import_error(state, next, &error),
        };
        state.stack.push(promise);
        state.pc = next;
        Ok(None)
    }

    fn finish_dynamic_import_rejection(
        &mut self,
        state: &mut BytecodeState,
        next: BytecodeAddress,
        reason: Value,
    ) -> Result<Option<Completion>> {
        let promise = self.create_rejected_promise(reason)?;
        state.stack.push(promise);
        state.pc = next;
        Ok(None)
    }

    fn finish_dynamic_import_error(
        &mut self,
        state: &mut BytecodeState,
        next: BytecodeAddress,
        error: &Error,
    ) -> Result<Option<Completion>> {
        let reason = self.dynamic_import_error_value(error)?;
        self.finish_dynamic_import_rejection(state, next, reason)
    }
}
