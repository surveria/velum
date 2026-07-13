use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBlock, BytecodeDestructureMode, BytecodePattern,
        BytecodePatternKey, BytecodePatternProperty, BytecodePatternTarget,
    },
    error::{Error, Result},
    runtime::binding::scope::BindingScope,
    runtime::control::Completion,
    runtime::object::{OBJECT_CONSTRUCTOR_PROPERTY, ObjectPropertyInit, PropertyEnumerable},
    runtime::{
        Context,
        abstract_operations::{IteratorSource, IteratorStep},
        control::runtime_exception_value,
        property::DynamicPropertyKey,
    },
    syntax::{DeclKind, StaticName},
    value::Value,
};

use super::{
    control_continuation::{BytecodeControlRecord, BytecodeLoopPhase},
    destructure_continuation::{DestructureContinuation, DestructureTask, ObjectPropertyPhase},
    ops::BytecodeAssignmentReference,
    state::{BytecodeState, bytecode_loop_completion},
};

mod assignment_reference;

/// Result of walking one destructuring pattern against a source value.
pub(in crate::runtime) enum DestructureOutcome {
    Completed,
    /// An abrupt completion raised by user code that must propagate outward.
    Abrupt(Completion),
}

/// Result of one pattern sub-step: either a produced value or an abrupt
/// completion raised by user code that must propagate outward.
enum PatternStep<T> {
    Value(T),
    Abrupt(Completion),
}

impl Context {
    pub(super) fn eval_bytecode_destructure_instruction(
        &mut self,
        state: &mut BytecodeState,
        pattern: &BytecodePattern,
        mode: BytecodeDestructureMode,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let value = if state.has_destructure_continuation() {
            None
        } else {
            Some(state.stack.peek()?.clone())
        };
        match self.eval_resumable_destructure(state, pattern, mode, value)? {
            DestructureOutcome::Completed => {
                if let BytecodeDestructureMode::Declaration(_) = mode {
                    state.stack.pop()?;
                }
                state.pc = next;
                Ok(None)
            }
            DestructureOutcome::Abrupt(completion) if completion.suspends_execution() => {
                Ok(Some(completion))
            }
            DestructureOutcome::Abrupt(completion) => {
                state.stack.pop()?;
                Ok(Some(completion))
            }
        }
    }

    pub(in crate::runtime) fn eval_resumable_destructure(
        &mut self,
        state: &mut BytecodeState,
        pattern: &BytecodePattern,
        mode: BytecodeDestructureMode,
        value: Option<Value>,
    ) -> Result<DestructureOutcome> {
        let mut continuation = if let Some(continuation) = state.take_destructure_continuation() {
            continuation
        } else {
            let value = value.ok_or_else(|| {
                Error::runtime("bytecode destructuring value disappeared before start")
            })?;
            DestructureContinuation::new(pattern.clone(), mode, value)
        };
        loop {
            let Some(task) = continuation.tasks.pop() else {
                return Ok(DestructureOutcome::Completed);
            };
            let abrupt = match self.run_destructure_task(&mut continuation, task) {
                Ok(abrupt) => abrupt,
                Err(error) => Some(self.destructure_error_completion(error)?),
            };
            let Some(completion) = abrupt else {
                continue;
            };
            if completion.suspends_execution() {
                state.store_destructure_continuation(continuation)?;
                return Ok(DestructureOutcome::Abrupt(completion));
            }
            let completion = self.close_destructure_iterators(&mut continuation, completion)?;
            return Ok(DestructureOutcome::Abrupt(completion));
        }
    }

    fn destructure_error_completion(&mut self, error: Error) -> Result<Completion> {
        let Some(value) = runtime_exception_value(self, &error)? else {
            return Err(error);
        };
        self.checked_value(value.clone())?;
        Ok(Completion::Throw(value))
    }

    fn run_destructure_task(
        &mut self,
        continuation: &mut DestructureContinuation,
        task: DestructureTask,
    ) -> Result<Option<Completion>> {
        match task {
            DestructureTask::Pattern { pattern, value } => {
                self.run_destructure_pattern_task(continuation, pattern, value)
            }
            DestructureTask::Object {
                properties,
                rest,
                source,
                next,
                consumed,
            } => self.run_destructure_object_task(
                continuation,
                properties,
                rest,
                source,
                next,
                consumed,
            ),
            DestructureTask::ObjectProperty {
                key,
                target,
                source,
                phase,
            } => self.run_destructure_property_task(continuation, key, target, source, phase),
            DestructureTask::Array {
                elements,
                rest,
                source,
                next,
                exhausted,
            } => self.run_destructure_array_task(
                continuation,
                elements,
                rest,
                source,
                next,
                exhausted,
            ),
            DestructureTask::ArrayElement {
                target,
                value,
                reference,
            } => self.run_destructure_element_task(continuation, target, value, reference),
        }
    }

    fn run_destructure_pattern_task(
        &mut self,
        continuation: &mut DestructureContinuation,
        pattern: BytecodePattern,
        value: Value,
    ) -> Result<Option<Completion>> {
        self.step()?;
        match pattern {
            BytecodePattern::Binding(name) => {
                self.initialize_bytecode_pattern_binding(&name, continuation.mode, value)?;
            }
            BytecodePattern::Assignment(target) => {
                if continuation.mode != BytecodeDestructureMode::Assignment {
                    return Err(Error::runtime(
                        "assignment pattern leaf used by declaration destructuring",
                    ));
                }
                self.assign_bytecode_target(&target, value)?;
            }
            BytecodePattern::Object { properties, rest } => {
                if matches!(value, Value::Undefined | Value::Null) {
                    return Err(Error::type_error(format!(
                        "cannot destructure '{value}' into an object pattern"
                    )));
                }
                continuation.tasks.push(DestructureTask::Object {
                    properties,
                    rest,
                    source: value,
                    next: 0,
                    consumed: Vec::new(),
                });
            }
            BytecodePattern::Array { elements, rest } => {
                let source = self.get_iterator(&value)?;
                continuation.tasks.push(DestructureTask::Array {
                    elements,
                    rest,
                    source,
                    next: 0,
                    exhausted: false,
                });
            }
        }
        Ok(None)
    }

    #[allow(clippy::too_many_arguments)]
    fn run_destructure_object_task(
        &mut self,
        continuation: &mut DestructureContinuation,
        properties: std::rc::Rc<[BytecodePatternProperty]>,
        rest: Option<std::rc::Rc<BytecodePattern>>,
        source: Value,
        next: usize,
        consumed: Vec<String>,
    ) -> Result<Option<Completion>> {
        if let Some(property) = properties.get(next).cloned() {
            let next = next
                .checked_add(1)
                .ok_or_else(|| Error::limit("destructuring property cursor overflowed"))?;
            continuation.tasks.push(DestructureTask::Object {
                properties,
                rest,
                source: source.clone(),
                next,
                consumed,
            });
            continuation.tasks.push(DestructureTask::ObjectProperty {
                key: property.key,
                target: property.target,
                source,
                phase: ObjectPropertyPhase::Read,
            });
            return Ok(None);
        }
        if let Some(rest_pattern) = rest.as_ref() {
            let reference = match self.assignment_reference_for_pattern(rest_pattern)? {
                PatternStep::Value(reference) => reference,
                PatternStep::Abrupt(completion) => {
                    continuation.tasks.push(DestructureTask::Object {
                        properties,
                        rest,
                        source,
                        next,
                        consumed,
                    });
                    return Ok(Some(completion));
                }
            };
            let rest_value = self.destructure_rest_object(&source, &consumed)?;
            if let Some(reference) = reference {
                reference.set(self, rest_value)?;
            } else {
                continuation.tasks.push(DestructureTask::Pattern {
                    pattern: rest_pattern.as_ref().clone(),
                    value: rest_value,
                });
            }
        }
        Ok(None)
    }

    fn run_destructure_property_task(
        &mut self,
        continuation: &mut DestructureContinuation,
        key: BytecodePatternKey,
        target: BytecodePatternTarget,
        source: Value,
        phase: ObjectPropertyPhase,
    ) -> Result<Option<Completion>> {
        match phase {
            ObjectPropertyPhase::Read => {
                let resolved_key = match self.resolve_resumable_pattern_property_key(&key)? {
                    PatternStep::Value(key) => key,
                    PatternStep::Abrupt(completion) => {
                        continuation.tasks.push(DestructureTask::ObjectProperty {
                            key,
                            target,
                            source,
                            phase: ObjectPropertyPhase::Read,
                        });
                        return Ok(Some(completion));
                    }
                };
                Self::record_destructure_consumed_name(
                    continuation,
                    resolved_key.name().to_owned(),
                )?;
                let reference = match self.assignment_reference_for_pattern(&target.pattern)? {
                    PatternStep::Value(reference) => reference,
                    PatternStep::Abrupt(completion) => {
                        continuation.tasks.push(DestructureTask::ObjectProperty {
                            key,
                            target,
                            source,
                            phase: ObjectPropertyPhase::Read,
                        });
                        return Ok(Some(completion));
                    }
                };
                let value = self.get(&source, resolved_key.lookup())?;
                continuation.tasks.push(DestructureTask::ObjectProperty {
                    key,
                    target,
                    source,
                    phase: ObjectPropertyPhase::Default { value, reference },
                });
            }
            ObjectPropertyPhase::Default { value, reference } => {
                let value =
                    match self.apply_pattern_default(value.clone(), target.default.as_ref())? {
                        PatternStep::Value(value) => value,
                        PatternStep::Abrupt(completion) => {
                            continuation.tasks.push(DestructureTask::ObjectProperty {
                                key,
                                target,
                                source,
                                phase: ObjectPropertyPhase::Default { value, reference },
                            });
                            return Ok(Some(completion));
                        }
                    };
                if let Some(reference) = reference {
                    reference.set(self, value)?;
                } else {
                    continuation.tasks.push(DestructureTask::Pattern {
                        pattern: target.pattern,
                        value,
                    });
                }
            }
        }
        Ok(None)
    }

    fn resolve_resumable_pattern_property_key(
        &mut self,
        key: &BytecodePatternKey,
    ) -> Result<PatternStep<DynamicPropertyKey>> {
        let key_value = match key {
            BytecodePatternKey::Static(name) => {
                return Ok(PatternStep::Value(DynamicPropertyKey::new(
                    name.as_str().to_owned(),
                    self.known_property_key(name.as_str()),
                )));
            }
            BytecodePatternKey::Computed(block) => match self.eval_pattern_block(block)? {
                PatternStep::Value(value) => value,
                PatternStep::Abrupt(completion) => {
                    return Ok(PatternStep::Abrupt(completion));
                }
            },
        };
        self.dynamic_property_key(&key_value)
            .map(PatternStep::Value)
    }

    fn record_destructure_consumed_name(
        continuation: &mut DestructureContinuation,
        name: String,
    ) -> Result<()> {
        for task in continuation.tasks.iter_mut().rev() {
            if let DestructureTask::Object { consumed, .. } = task {
                consumed.push(name);
                return Ok(());
            }
        }
        Err(Error::runtime(
            "destructuring object property lost its parent task",
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn run_destructure_array_task(
        &mut self,
        continuation: &mut DestructureContinuation,
        elements: std::rc::Rc<[Option<BytecodePatternTarget>]>,
        rest: Option<std::rc::Rc<BytecodePattern>>,
        mut source: IteratorSource,
        next: usize,
        mut exhausted: bool,
    ) -> Result<Option<Completion>> {
        if let Some(element) = elements.get(next).cloned() {
            self.step()?;
            let reference = if let Some(target) = element.as_ref() {
                match self.assignment_reference_for_pattern(&target.pattern)? {
                    PatternStep::Value(reference) => reference,
                    PatternStep::Abrupt(completion) => {
                        continuation.tasks.push(DestructureTask::Array {
                            elements,
                            rest,
                            source,
                            next,
                            exhausted,
                        });
                        return Ok(Some(completion));
                    }
                }
            } else {
                None
            };
            let value = if exhausted {
                Value::Undefined
            } else {
                match self.iterator_step(&mut source)? {
                    IteratorStep::Value(value) => value,
                    IteratorStep::Done => {
                        exhausted = true;
                        Value::Undefined
                    }
                    IteratorStep::Abrupt(completion) => return Ok(Some(completion)),
                }
            };
            let next = next
                .checked_add(1)
                .ok_or_else(|| Error::limit("destructuring element cursor overflowed"))?;
            continuation.tasks.push(DestructureTask::Array {
                elements,
                rest,
                source,
                next,
                exhausted,
            });
            if let Some(target) = element {
                continuation.tasks.push(DestructureTask::ArrayElement {
                    target,
                    value,
                    reference,
                });
            }
            return Ok(None);
        }
        if let Some(rest_pattern) = rest.as_ref() {
            let reference = match self.assignment_reference_for_pattern(rest_pattern)? {
                PatternStep::Value(reference) => reference,
                PatternStep::Abrupt(completion) => {
                    continuation.tasks.push(DestructureTask::Array {
                        elements,
                        rest,
                        source,
                        next,
                        exhausted,
                    });
                    return Ok(Some(completion));
                }
            };
            let mut items = Vec::new();
            while !exhausted {
                self.step()?;
                match self.iterator_step(&mut source)? {
                    IteratorStep::Value(value) => items.push(value),
                    IteratorStep::Done => exhausted = true,
                    IteratorStep::Abrupt(completion) => return Ok(Some(completion)),
                }
            }
            let rest_value = self.create_array_from_elements(items)?;
            if let Some(reference) = reference {
                reference.set(self, rest_value)?;
            } else {
                continuation.tasks.push(DestructureTask::Pattern {
                    pattern: rest_pattern.as_ref().clone(),
                    value: rest_value,
                });
            }
            return Ok(None);
        }
        if !exhausted {
            let completion =
                self.iterator_close(&mut source, Completion::Normal(Value::Undefined))?;
            if !matches!(completion, Completion::Normal(_)) {
                return Ok(Some(completion));
            }
        }
        Ok(None)
    }

    fn run_destructure_element_task(
        &mut self,
        continuation: &mut DestructureContinuation,
        target: BytecodePatternTarget,
        value: Value,
        reference: Option<BytecodeAssignmentReference>,
    ) -> Result<Option<Completion>> {
        let resolved = match self.apply_pattern_default(value.clone(), target.default.as_ref())? {
            PatternStep::Value(value) => value,
            PatternStep::Abrupt(completion) => {
                continuation.tasks.push(DestructureTask::ArrayElement {
                    target,
                    value,
                    reference,
                });
                return Ok(Some(completion));
            }
        };
        if let Some(reference) = reference {
            reference.set(self, resolved)?;
        } else {
            continuation.tasks.push(DestructureTask::Pattern {
                pattern: target.pattern,
                value: resolved,
            });
        }
        Ok(None)
    }

    fn close_destructure_iterators(
        &mut self,
        continuation: &mut DestructureContinuation,
        mut completion: Completion,
    ) -> Result<Completion> {
        for task in continuation.tasks.iter_mut().rev() {
            if let DestructureTask::Array {
                source, exhausted, ..
            } = task
                && !*exhausted
            {
                completion = self.iterator_close(source, completion)?;
                *exhausted = true;
            }
        }
        Ok(completion)
    }

    fn destructure_rest_object(&mut self, source: &Value, consumed: &[String]) -> Result<Value> {
        let keys = match source {
            Value::Bool(_) | Value::Number(_) | Value::Symbol(_) => Vec::new(),
            _ => self.own_enumerable_keys(source)?,
        };
        let mut entries = Vec::new();
        for key in keys {
            if consumed.iter().any(|used| used == &key) {
                continue;
            }
            let value = self.get_named(source, &key)?;
            let property_key = self.intern_property_key(&key)?;
            entries.push((property_key, key, value));
        }
        let inits = entries
            .iter()
            .map(|(key, name, value)| {
                ObjectPropertyInit::new_data(
                    *key,
                    name.as_str(),
                    value.clone(),
                    PropertyEnumerable::Yes,
                )
            })
            .collect::<Vec<_>>();
        let constructor_key = self.intern_property_key(OBJECT_CONSTRUCTOR_PROPERTY)?;
        self.objects.create(
            inits,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn apply_pattern_default(
        &mut self,
        value: Value,
        default: Option<&BytecodeBlock>,
    ) -> Result<PatternStep<Value>> {
        if !matches!(value, Value::Undefined) {
            return Ok(PatternStep::Value(value));
        }
        let Some(default) = default else {
            return Ok(PatternStep::Value(value));
        };
        self.eval_pattern_block(default)
    }

    fn eval_pattern_block(&mut self, block: &BytecodeBlock) -> Result<PatternStep<Value>> {
        match self.eval_bytecode_block(block)? {
            Completion::Normal(value) => Ok(PatternStep::Value(value)),
            completion @ (Completion::Throw(_)
            | Completion::Return(_)
            | Completion::ReturnDirect(_)
            | Completion::Break { .. }
            | Completion::Continue { .. }
            | Completion::Suspended(_)
            | Completion::GeneratorStart
            | Completion::Yielded(_)
            | Completion::YieldedIteratorResult(_)) => Ok(PatternStep::Abrupt(completion)),
        }
    }

    pub(super) fn eval_for_of_pattern_loop(
        &mut self,
        iterator: Option<super::super::abstract_operations::ForOfIterator>,
        pattern: &BytecodePattern,
        mode: BytecodeDestructureMode,
        body: &BytecodeBlock,
        labels: Option<&[StaticName]>,
    ) -> Result<Completion> {
        let handle = self.push_bytecode_control(BytecodeControlRecord::for_of(iterator))?;
        let mut control = self.checkout_bytecode_control(handle)?;
        if *control.for_of_state_mut()?.0 == BytecodeLoopPhase::Close {
            return self.resume_for_of_close(handle, control);
        }
        loop {
            let phase = *control.for_of_state_mut()?.0;
            let value = if phase == BytecodeLoopPhase::Initialize {
                let value = match self.next_for_of_value(handle, &mut control)? {
                    super::for_of::ForOfNext::Value(value) => value,
                    super::for_of::ForOfNext::Done => break,
                    super::for_of::ForOfNext::Abrupt(completion) => {
                        return Self::finish_for_of_control(self, handle, completion);
                    }
                    super::for_of::ForOfNext::Await(awaited) => {
                        self.park_bytecode_control(handle, control)?;
                        return Ok(Completion::Suspended(awaited));
                    }
                };
                if matches!(
                    mode,
                    BytecodeDestructureMode::Declaration(
                        DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing
                    )
                ) {
                    self.push_lexical_scope_with(BindingScope::new())?;
                }
                *control.for_of_state_mut()?.0 = BytecodeLoopPhase::Destructure;
                Some(value)
            } else {
                None
            };
            if *control.for_of_state_mut()?.0 == BytecodeLoopPhase::Destructure {
                let destructure = self.run_bytecode_control_segment_result(
                    &mut control,
                    super::control_continuation::BytecodeControlStateSlot::Body,
                    |context, state| {
                        context.eval_resumable_destructure(state, pattern, mode, value)
                    },
                );
                let destructure = match destructure {
                    Ok(destructure) => destructure,
                    Err(error) => {
                        self.pop_pattern_iteration_scope(mode)?;
                        return self.close_for_of_error(handle, control, error);
                    }
                };
                match destructure {
                    DestructureOutcome::Completed => {
                        *control.for_of_state_mut()?.0 = BytecodeLoopPhase::Body;
                    }
                    DestructureOutcome::Abrupt(completion) if completion.suspends_execution() => {
                        self.park_bytecode_control(handle, control)?;
                        return Ok(completion);
                    }
                    DestructureOutcome::Abrupt(completion) => {
                        self.pop_pattern_iteration_scope(mode)?;
                        return self.close_for_of_completion(handle, control, completion);
                    }
                }
            }
            let body_result = self.run_bytecode_control_segment_result(
                &mut control,
                super::control_continuation::BytecodeControlStateSlot::Body,
                |context, state| context.eval_bytecode_block_with_state(body, state),
            );
            if body_result
                .as_ref()
                .is_ok_and(Completion::suspends_execution)
            {
                let completion = body_result?;
                self.park_bytecode_control(handle, control)?;
                return Ok(completion);
            }
            self.pop_pattern_iteration_scope(mode)?;
            let completion = match body_result {
                Ok(completion) => completion,
                Err(error) => return self.close_for_of_error(handle, control, error),
            };
            let (_, last) = control.for_of_state_mut()?;
            if let Some(completion) = bytecode_loop_completion(last, completion, labels) {
                return self.close_for_of_completion(handle, control, completion);
            }
            *control.for_of_state_mut()?.0 = BytecodeLoopPhase::Initialize;
        }
        let (_, last) = control.for_of_state_mut()?;
        let completion = Completion::Normal(std::mem::replace(last, Value::Undefined));
        Self::finish_for_of_control(self, handle, completion)
    }

    pub(super) fn eval_for_in_pattern_loop(
        &mut self,
        keys: Vec<String>,
        pattern: &BytecodePattern,
        mode: BytecodeDestructureMode,
        body: &BytecodeBlock,
        labels: Option<&[StaticName]>,
    ) -> Result<Completion> {
        let handle = self.push_bytecode_control(BytecodeControlRecord::for_in(keys))?;
        let mut control = self.checkout_bytecode_control(handle)?;
        loop {
            let phase = *control.for_in_state_mut()?.0;
            let value = if phase == BytecodeLoopPhase::Initialize {
                let key = {
                    let (_, keys, _) = control.for_in_state_mut()?;
                    let Some(key) = keys.next() else {
                        break;
                    };
                    key
                };
                self.step()?;
                let value = self.heap_string_value(&key)?;
                if matches!(
                    mode,
                    BytecodeDestructureMode::Declaration(
                        DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing
                    )
                ) {
                    self.push_lexical_scope_with(BindingScope::new())?;
                }
                *control.for_in_state_mut()?.0 = BytecodeLoopPhase::Destructure;
                Some(value)
            } else {
                None
            };
            if *control.for_in_state_mut()?.0 == BytecodeLoopPhase::Destructure {
                let destructure = self.run_bytecode_control_segment(
                    handle,
                    &mut control,
                    super::control_continuation::BytecodeControlStateSlot::Body,
                    |context, state| {
                        context.eval_resumable_destructure(state, pattern, mode, value)
                    },
                )?;
                match destructure {
                    DestructureOutcome::Completed => {
                        *control.for_in_state_mut()?.0 = BytecodeLoopPhase::Body;
                    }
                    DestructureOutcome::Abrupt(completion) if completion.suspends_execution() => {
                        self.park_bytecode_control(handle, control)?;
                        return Ok(completion);
                    }
                    DestructureOutcome::Abrupt(completion) => {
                        self.pop_pattern_iteration_scope(mode)?;
                        return self.finish_bytecode_control_result(handle, Ok(completion));
                    }
                }
            }
            let body_result = self.run_bytecode_control_segment(
                handle,
                &mut control,
                super::control_continuation::BytecodeControlStateSlot::Body,
                |context, state| context.eval_bytecode_block_with_state(body, state),
            );
            if body_result
                .as_ref()
                .is_ok_and(Completion::suspends_execution)
            {
                let completion = body_result?;
                self.park_bytecode_control(handle, control)?;
                return Ok(completion);
            }
            self.pop_pattern_iteration_scope(mode)?;
            let completion = body_result?;
            let (_, _, last) = control.for_in_state_mut()?;
            if let Some(completion) = bytecode_loop_completion(last, completion, labels) {
                return self.finish_bytecode_control_result(handle, Ok(completion));
            }
            *control.for_in_state_mut()?.0 = BytecodeLoopPhase::Initialize;
        }
        let (_, _, last) = control.for_in_state_mut()?;
        let completion = Completion::Normal(std::mem::replace(last, Value::Undefined));
        self.finish_bytecode_control_result(handle, Ok(completion))
    }

    fn pop_pattern_iteration_scope(&mut self, mode: BytecodeDestructureMode) -> Result<()> {
        if matches!(
            mode,
            BytecodeDestructureMode::Declaration(
                DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing
            )
        ) && self.pop_lexical_scope()?.is_none()
        {
            return Err(Error::runtime("bytecode pattern loop scope disappeared"));
        }
        Ok(())
    }
}
