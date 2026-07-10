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

/// Result of one pattern-target loop iteration in a `for-in`/`for-of` head.
enum PatternIteration {
    Body(Completion),
    DestructureAbrupt(Completion),
}

impl Context {
    pub(super) fn eval_bytecode_destructure_instruction(
        &mut self,
        state: &mut BytecodeState,
        pattern: &BytecodePattern,
        kind: DeclKind,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let value = state.stack.pop()?;
        match self.destructure_pattern(pattern, kind, value)? {
            DestructureOutcome::Completed => {
                state.last = Value::Undefined;
                state.pc = next;
                Ok(None)
            }
            DestructureOutcome::Abrupt(completion) => Ok(Some(completion)),
        }
    }

    pub(super) fn destructure_pattern(
        &mut self,
        pattern: &BytecodePattern,
        kind: DeclKind,
        value: Value,
    ) -> Result<DestructureOutcome> {
        self.step()?;
        match pattern {
            BytecodePattern::Binding(name) => {
                self.eval_bytecode_declaration(name, kind, Some(value))?;
                Ok(DestructureOutcome::Completed)
            }
            BytecodePattern::Object { properties, rest } => {
                self.destructure_object(properties, rest.as_ref(), kind, &value)
            }
            BytecodePattern::Array { elements, rest } => {
                self.destructure_array(elements, rest.as_deref(), kind, value)
            }
        }
    }

    fn destructure_object(
        &mut self,
        properties: &[BytecodePatternProperty],
        rest: Option<&BytecodeBinding>,
        kind: DeclKind,
        source: &Value,
    ) -> Result<DestructureOutcome> {
        if matches!(source, Value::Undefined | Value::Null) {
            return Err(Error::type_error(format!(
                "cannot destructure '{source}' into an object pattern"
            )));
        }
        let mut consumed = rest.map(|_| Vec::new());
        for property in properties {
            let value = match self.destructure_property_read(source, &property.key)? {
                PatternStep::Value(PatternPropertyRead { name, value }) => {
                    if let Some(consumed) = consumed.as_mut() {
                        consumed.push(name);
                    }
                    value
                }
                PatternStep::Abrupt(completion) => {
                    return Ok(DestructureOutcome::Abrupt(completion));
                }
            };
            let value = match self.apply_pattern_default(value, property.target.default.as_ref())? {
                PatternStep::Value(value) => value,
                PatternStep::Abrupt(completion) => {
                    return Ok(DestructureOutcome::Abrupt(completion));
                }
            };
            match self.destructure_pattern(&property.target.pattern, kind, value)? {
                DestructureOutcome::Completed => {}
                abrupt @ DestructureOutcome::Abrupt(_) => return Ok(abrupt),
            }
        }
        if let Some(rest_binding) = rest {
            let consumed = consumed.unwrap_or_default();
            let rest_value = self.destructure_rest_object(source, &consumed)?;
            self.eval_bytecode_declaration(rest_binding, kind, Some(rest_value))?;
        }
        Ok(DestructureOutcome::Completed)
    }

    fn destructure_property_read(
        &mut self,
        source: &Value,
        key: &BytecodePatternKey,
    ) -> Result<PatternStep<PatternPropertyRead>> {
        match key {
            BytecodePatternKey::Static(name) => {
                let value = self.get_named(source, name.as_str())?;
                Ok(PatternStep::Value(PatternPropertyRead {
                    name: name.as_str().to_owned(),
                    value,
                }))
            }
            BytecodePatternKey::Computed(block) => {
                let key_value = match self.eval_pattern_block(block)? {
                    PatternStep::Value(value) => value,
                    PatternStep::Abrupt(completion) => {
                        return Ok(PatternStep::Abrupt(completion));
                    }
                };
                if matches!(key_value, Value::Symbol(_)) {
                    let key = self.dynamic_property_key(&key_value)?;
                    let value = self.get(source, key.lookup())?;
                    return Ok(PatternStep::Value(PatternPropertyRead {
                        name: key.name().to_owned(),
                        value,
                    }));
                }
                let name = self.dynamic_property_key(&key_value)?.name().to_owned();
                let value = self.get_named(source, &name)?;
                Ok(PatternStep::Value(PatternPropertyRead { name, value }))
            }
        }
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

    fn destructure_array(
        &mut self,
        elements: &[Option<BytecodePatternTarget>],
        rest: Option<&BytecodePattern>,
        kind: DeclKind,
        value: Value,
    ) -> Result<DestructureOutcome> {
        let mut source = self.get_iterator(value)?;
        let mut exhausted = false;
        for element in elements {
            self.step()?;
            let step_value = if exhausted {
                Value::Undefined
            } else {
                match self.iterator_step(&mut source)? {
                    IteratorStep::Value(value) => value,
                    IteratorStep::Done => {
                        exhausted = true;
                        Value::Undefined
                    }
                    // A throw from next() propagates without IteratorClose.
                    IteratorStep::Abrupt(completion) => {
                        return Ok(DestructureOutcome::Abrupt(completion));
                    }
                }
            };
            let Some(target) = element else {
                continue;
            };
            let step_value =
                match self.apply_pattern_default(step_value, target.default.as_ref())? {
                    PatternStep::Value(value) => value,
                    PatternStep::Abrupt(completion) => {
                        let completion = self.iterator_close(&mut source, completion)?;
                        return Ok(DestructureOutcome::Abrupt(completion));
                    }
                };
            match self.destructure_pattern(&target.pattern, kind, step_value)? {
                DestructureOutcome::Completed => {}
                DestructureOutcome::Abrupt(completion) => {
                    let completion = self.iterator_close(&mut source, completion)?;
                    return Ok(DestructureOutcome::Abrupt(completion));
                }
            }
        }
        if let Some(rest) = rest {
            let mut items = Vec::new();
            while !exhausted {
                self.step()?;
                match self.iterator_step(&mut source)? {
                    IteratorStep::Value(value) => items.push(value),
                    IteratorStep::Done => exhausted = true,
                    IteratorStep::Abrupt(completion) => {
                        return Ok(DestructureOutcome::Abrupt(completion));
                    }
                }
            }
            let rest_value = self.create_array_from_elements(items)?;
            match self.destructure_pattern(rest, kind, rest_value)? {
                DestructureOutcome::Completed => {}
                abrupt @ DestructureOutcome::Abrupt(_) => return Ok(abrupt),
            }
        } else if !exhausted {
            // The pattern finished before the iterator: IteratorClose.
            let completion =
                self.iterator_close(&mut source, Completion::Normal(Value::Undefined))?;
            if !matches!(completion, Completion::Normal(_)) {
                return Ok(DestructureOutcome::Abrupt(completion));
            }
        }
        Ok(DestructureOutcome::Completed)
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
            completion @ Completion::Throw(_) => Ok(PatternStep::Abrupt(completion)),
            completion @ (Completion::Return(_)
            | Completion::Break { .. }
            | Completion::Continue(_)) => completion.into_result().map(PatternStep::Value),
            completion @ Completion::Suspended(_) => Ok(PatternStep::Abrupt(completion)),
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
            *control.for_of_state_mut()?.0 = BytecodeLoopPhase::Initialize;
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
            *control.for_of_state_mut()?.0 = BytecodeLoopPhase::Body;
            let iteration = self.run_bytecode_control_action_result(&control, |context| {
                context.eval_pattern_iteration(pattern, kind, value, body)
            });
            let iteration = match iteration {
                Ok(iteration) => iteration,
                Err(error) => return self.close_for_of_error(handle, control, error),
            };
            match iteration {
                PatternIteration::Body(completion) => {
                    let (_, last) = control.for_of_state_mut()?;
                    if let Some(completion) = bytecode_loop_completion(last, completion, labels) {
                        return self.close_for_of_completion(handle, control, completion);
                    }
                }
                PatternIteration::DestructureAbrupt(completion) => {
                    return self.close_for_of_completion(handle, control, completion);
                }
            }
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
            let (phase, keys, _) = control.for_in_state_mut()?;
            *phase = BytecodeLoopPhase::Initialize;
            let Some(key) = keys.next() else {
                break;
            };
            *control.for_in_state_mut()?.0 = BytecodeLoopPhase::Body;
            let iteration = self.run_bytecode_control_action(handle, &control, |context| {
                context.step()?;
                let value = context.heap_string_value(&key)?;
                context.eval_pattern_iteration(pattern, kind, value, body)
            })?;
            match iteration {
                PatternIteration::Body(completion) => {
                    let (_, _, last) = control.for_in_state_mut()?;
                    if let Some(completion) = bytecode_loop_completion(last, completion, labels) {
                        return self.finish_bytecode_control_result(handle, Ok(completion));
                    }
                }
                PatternIteration::DestructureAbrupt(completion) => {
                    return self.finish_bytecode_control_result(handle, Ok(completion));
                }
            }
        }
        let (_, _, last) = control.for_in_state_mut()?;
        let completion = Completion::Normal(std::mem::replace(last, Value::Undefined));
        self.finish_bytecode_control_result(handle, Ok(completion))
    }

    /// Destructures one loop value and runs the loop body, giving lexical
    /// kinds a fresh per-iteration scope for their pattern bindings.
    fn eval_pattern_iteration(
        &mut self,
        pattern: &BytecodePattern,
        kind: DeclKind,
        value: Value,
        body: &BytecodeBlock,
    ) -> Result<PatternIteration> {
        let lexical = matches!(kind, DeclKind::Let | DeclKind::Const);
        if lexical {
            self.push_lexical_scope_with(BindingScope::new())?;
        }
        let iteration = self.destructured_body_completion(pattern, kind, value, body);
        if lexical && self.pop_lexical_scope()?.is_none() {
            return Err(Error::runtime("bytecode pattern loop scope disappeared"));
        }
        iteration
    }

    fn destructured_body_completion(
        &mut self,
        pattern: &BytecodePattern,
        kind: DeclKind,
        value: Value,
        body: &BytecodeBlock,
    ) -> Result<PatternIteration> {
        match self.destructure_pattern(pattern, kind, value)? {
            DestructureOutcome::Completed => {
                Ok(PatternIteration::Body(self.eval_bytecode_block(body)?))
            }
            DestructureOutcome::Abrupt(completion) => {
                Ok(PatternIteration::DestructureAbrupt(completion))
            }
        }
    }
}

/// A property read produced by an object pattern key: the consumed key name
/// plus the value it resolved to.
struct PatternPropertyRead {
    name: String,
    value: Value,
}
