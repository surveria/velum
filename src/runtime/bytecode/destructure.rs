use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodePattern, BytecodePatternKey,
        BytecodePatternProperty, BytecodePatternTarget,
    },
    error::{Error, Result},
    runtime::binding::scope::BindingScope,
    runtime::control::Completion,
    runtime::object::{OBJECT_CONSTRUCTOR_PROPERTY, ObjectPropertyInit, PropertyEnumerable},
    runtime::{
        Context,
        abstract_operations::{IteratorSource, IteratorStep},
    },
    syntax::{DeclKind, StaticName},
    value::Value,
};

use super::{
    control_continuation::{BytecodeControlRecord, BytecodeLoopPhase},
    destructure_continuation::{DestructureContinuation, DestructureTask, ObjectPropertyPhase},
    state::{BytecodeState, bytecode_loop_completion},
};

/// Result of walking one destructuring pattern against a source value.
pub(super) enum DestructureOutcome {
    Completed,
    /// An abrupt completion raised by user code (defaults, computed keys, or
    /// iterator protocol calls) that must propagate out of the statement.
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
        kind: DeclKind,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let value = if state.has_destructure_continuation() {
            None
        } else {
            Some(state.stack.peek()?.clone())
        };
        match self.eval_resumable_destructure(state, pattern, kind, value)? {
            DestructureOutcome::Completed => {
                state.stack.pop()?;
                state.last = Value::Undefined;
                state.pc = next;
                Ok(None)
            }
            DestructureOutcome::Abrupt(completion @ Completion::Suspended(_)) => {
                Ok(Some(completion))
            }
            DestructureOutcome::Abrupt(completion) => {
                state.stack.pop()?;
                Ok(Some(completion))
            }
        }
    }

    fn eval_resumable_destructure(
        &mut self,
        state: &mut BytecodeState,
        pattern: &BytecodePattern,
        kind: DeclKind,
        value: Option<Value>,
    ) -> Result<DestructureOutcome> {
        let mut continuation = if let Some(continuation) = state.take_destructure_continuation() {
            continuation
        } else {
            let value = value.ok_or_else(|| {
                Error::runtime("bytecode destructuring value disappeared before start")
            })?;
            DestructureContinuation::new(pattern.clone(), kind, value)
        };
        loop {
            let Some(task) = continuation.tasks.pop() else {
                return Ok(DestructureOutcome::Completed);
            };
            let abrupt = self.run_destructure_task(&mut continuation, task)?;
            let Some(completion) = abrupt else {
                continue;
            };
            if matches!(completion, Completion::Suspended(_)) {
                state.store_destructure_continuation(continuation)?;
                return Ok(DestructureOutcome::Abrupt(completion));
            }
            let completion = self.close_destructure_iterators(&mut continuation, completion)?;
            return Ok(DestructureOutcome::Abrupt(completion));
        }
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
            DestructureTask::ArrayElement { target, value } => {
                self.run_destructure_element_task(continuation, target, value)
            }
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
                self.eval_bytecode_declaration(&name, continuation.kind, Some(value))?;
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
                let source = self.get_iterator(value)?;
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
        rest: Option<BytecodeBinding>,
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
        if let Some(rest_binding) = rest {
            let rest_value = self.destructure_rest_object(&source, &consumed)?;
            self.eval_bytecode_declaration(&rest_binding, continuation.kind, Some(rest_value))?;
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
                let (name, value) = match self.read_resumable_pattern_property(&source, &key)? {
                    PatternStep::Value(read) => (read.name, read.value),
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
                Self::record_destructure_consumed_name(continuation, name)?;
                continuation.tasks.push(DestructureTask::ObjectProperty {
                    key,
                    target,
                    source,
                    phase: ObjectPropertyPhase::Default { value },
                });
            }
            ObjectPropertyPhase::Default { value } => {
                let value =
                    match self.apply_pattern_default(value.clone(), target.default.as_ref())? {
                        PatternStep::Value(value) => value,
                        PatternStep::Abrupt(completion) => {
                            continuation.tasks.push(DestructureTask::ObjectProperty {
                                key,
                                target,
                                source,
                                phase: ObjectPropertyPhase::Default { value },
                            });
                            return Ok(Some(completion));
                        }
                    };
                continuation.tasks.push(DestructureTask::Pattern {
                    pattern: target.pattern,
                    value,
                });
            }
        }
        Ok(None)
    }

    fn read_resumable_pattern_property(
        &mut self,
        source: &Value,
        key: &BytecodePatternKey,
    ) -> Result<PatternStep<PatternPropertyRead>> {
        let key_value = match key {
            BytecodePatternKey::Static(name) => {
                let value = self.get_named(source, name.as_str())?;
                return Ok(PatternStep::Value(PatternPropertyRead {
                    name: name.as_str().to_owned(),
                    value,
                }));
            }
            BytecodePatternKey::Computed(block) => match self.eval_pattern_block(block)? {
                PatternStep::Value(value) => value,
                PatternStep::Abrupt(completion) => {
                    return Ok(PatternStep::Abrupt(completion));
                }
            },
        };
        let key = self.dynamic_property_key(&key_value)?;
        let name = key.name().to_owned();
        let value = self.get(source, key.lookup())?;
        Ok(PatternStep::Value(PatternPropertyRead { name, value }))
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
                continuation
                    .tasks
                    .push(DestructureTask::ArrayElement { target, value });
            }
            return Ok(None);
        }
        if let Some(rest_pattern) = rest {
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
            continuation.tasks.push(DestructureTask::Pattern {
                pattern: rest_pattern.as_ref().clone(),
                value: rest_value,
            });
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
    ) -> Result<Option<Completion>> {
        let resolved = match self.apply_pattern_default(value.clone(), target.default.as_ref())? {
            PatternStep::Value(value) => value,
            PatternStep::Abrupt(completion) => {
                continuation
                    .tasks
                    .push(DestructureTask::ArrayElement { target, value });
                return Ok(Some(completion));
            }
        };
        continuation.tasks.push(DestructureTask::Pattern {
            pattern: target.pattern,
            value: resolved,
        });
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
            completion @ (Completion::Throw(_) | Completion::Suspended(_)) => {
                Ok(PatternStep::Abrupt(completion))
            }
            completion @ (Completion::Return(_)
            | Completion::Break { .. }
            | Completion::Continue(_)) => completion.into_result().map(PatternStep::Value),
        }
    }

    pub(super) fn eval_for_of_pattern_loop(
        &mut self,
        source: IteratorSource,
        pattern: &BytecodePattern,
        kind: DeclKind,
        body: &BytecodeBlock,
        labels: Option<&[StaticName]>,
    ) -> Result<Completion> {
        let handle = self.push_bytecode_control(BytecodeControlRecord::for_of(source))?;
        let mut control = self.checkout_bytecode_control(handle)?;
        loop {
            let phase = *control.for_of_state_mut()?.0;
            let value = if phase == BytecodeLoopPhase::Initialize {
                let step =
                    self.run_bytecode_iterator_action(handle, &mut control, |context, source| {
                        context.step()?;
                        context.iterator_step(source)
                    })?;
                let value = match step {
                    IteratorStep::Value(value) => value,
                    IteratorStep::Done => break,
                    IteratorStep::Abrupt(completion) => {
                        return Self::finish_for_of_control(self, handle, completion);
                    }
                };
                if matches!(kind, DeclKind::Let | DeclKind::Const) {
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
                        context.eval_resumable_destructure(state, pattern, kind, value)
                    },
                );
                let destructure = match destructure {
                    Ok(destructure) => destructure,
                    Err(error) => {
                        self.pop_pattern_iteration_scope(kind)?;
                        return self.close_for_of_error(handle, control, error);
                    }
                };
                match destructure {
                    DestructureOutcome::Completed => {
                        *control.for_of_state_mut()?.0 = BytecodeLoopPhase::Body;
                    }
                    DestructureOutcome::Abrupt(completion @ Completion::Suspended(_)) => {
                        self.park_bytecode_control(handle, control)?;
                        return Ok(completion);
                    }
                    DestructureOutcome::Abrupt(completion) => {
                        self.pop_pattern_iteration_scope(kind)?;
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
                .is_ok_and(|completion| matches!(completion, Completion::Suspended(_)))
            {
                let completion = body_result?;
                self.park_bytecode_control(handle, control)?;
                return Ok(completion);
            }
            self.pop_pattern_iteration_scope(kind)?;
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
        kind: DeclKind,
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
                if matches!(kind, DeclKind::Let | DeclKind::Const) {
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
                        context.eval_resumable_destructure(state, pattern, kind, value)
                    },
                )?;
                match destructure {
                    DestructureOutcome::Completed => {
                        *control.for_in_state_mut()?.0 = BytecodeLoopPhase::Body;
                    }
                    DestructureOutcome::Abrupt(completion @ Completion::Suspended(_)) => {
                        self.park_bytecode_control(handle, control)?;
                        return Ok(completion);
                    }
                    DestructureOutcome::Abrupt(completion) => {
                        self.pop_pattern_iteration_scope(kind)?;
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
                .is_ok_and(|completion| matches!(completion, Completion::Suspended(_)))
            {
                let completion = body_result?;
                self.park_bytecode_control(handle, control)?;
                return Ok(completion);
            }
            self.pop_pattern_iteration_scope(kind)?;
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

    fn pop_pattern_iteration_scope(&mut self, kind: DeclKind) -> Result<()> {
        if matches!(kind, DeclKind::Let | DeclKind::Const) && self.pop_lexical_scope()?.is_none() {
            return Err(Error::runtime("bytecode pattern loop scope disappeared"));
        }
        Ok(())
    }
}

/// A property read produced by an object pattern key: the consumed key name
/// plus the value it resolved to.
struct PatternPropertyRead {
    name: String,
    value: Value,
}
