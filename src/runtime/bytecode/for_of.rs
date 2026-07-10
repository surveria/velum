use crate::{
    bytecode::{BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeForInTarget},
    error::{Error, Result},
    runtime::binding::scope::{BindingCell, BindingScope},
    runtime::control::Completion,
    runtime::object::PropertyKey,
    runtime::property::DynamicPropertyKey,
    runtime::{Context, abstract_operations::to_boolean},
    syntax::{DeclKind, StaticName},
    value::Value,
};

use super::state::{BytecodeState, bytecode_loop_completion};

const ITERATOR_SYMBOL_DISPLAY_NAME: &str = "Symbol(Symbol.iterator)";
const ITERATOR_NEXT_PROPERTY: &str = "next";
const ITERATOR_RETURN_PROPERTY: &str = "return";
const ITERATOR_RESULT_DONE_PROPERTY: &str = "done";
const ITERATOR_RESULT_VALUE_PROPERTY: &str = "value";

/// One iteration source for a `for...of` loop. Arrays and primitive strings
/// use direct engine iteration because the engine does not install
/// `%Array.prototype%[Symbol.iterator]` yet; other objects go through the
/// user-visible iterator protocol.
pub(in crate::runtime) enum ForOfSource {
    /// Live array index iteration: the length is re-read every step so
    /// mutation during iteration behaves like the spec array iterator.
    ArrayIndex { array: Value, index: usize },
    /// Code-point iteration over an immutable string snapshot.
    Chars { chars: std::vec::IntoIter<char> },
    /// User iterator protocol with the `next` method cached at loop entry.
    Protocol {
        iterator: Value,
        next: Value,
        done: bool,
    },
}

/// Outcome of advancing a `for...of` source by one element.
pub(in crate::runtime) enum ForOfStep {
    Value(Value),
    Done,
    /// An abrupt completion thrown by user iterator code.
    Abrupt(Completion),
}

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
        let mut source = self.for_of_source(iterable)?;
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

    pub(in crate::runtime) fn for_of_source(&mut self, iterable: Value) -> Result<ForOfSource> {
        match &iterable {
            Value::String(text) => Ok(chars_source(text)),
            Value::HeapString(text) => Ok(chars_source(text.as_str())),
            Value::Object(id) => {
                if let Some(source) = self.protocol_source(&iterable)? {
                    return Ok(source);
                }
                if self.objects.array_len_if_array(*id)?.is_some() {
                    return Ok(ForOfSource::ArrayIndex {
                        array: iterable,
                        index: 0,
                    });
                }
                if let Some(text) = self.string_object_primitive_value(*id)? {
                    return Ok(chars_source(text));
                }
                Err(not_iterable_error(&iterable))
            }
            Value::Function(_) | Value::NativeFunction(_) | Value::HostFunction(_) => {
                if let Some(source) = self.protocol_source(&iterable)? {
                    return Ok(source);
                }
                Err(not_iterable_error(&iterable))
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::Symbol(_)
            | Value::Error(_) => Err(not_iterable_error(&iterable)),
        }
    }

    /// Builds a protocol source when the value exposes a callable
    /// `Symbol.iterator` method, mirroring `GetIterator`.
    fn protocol_source(&mut self, iterable: &Value) -> Result<Option<ForOfSource>> {
        let Some(symbol) = self.iterator_symbol() else {
            return Ok(None);
        };
        let key = DynamicPropertyKey::new(
            ITERATOR_SYMBOL_DISPLAY_NAME.to_owned(),
            Some(PropertyKey::symbol(symbol)),
        );
        let method = self.get_property_value_with_lookup(iterable, key.lookup())?;
        if matches!(method, Value::Undefined | Value::Null) {
            return Ok(None);
        }
        let iterator = match self.eval_call_completion(&method, &[], iterable.clone())? {
            Completion::Normal(value) => value,
            completion => return completion.into_result().map(|_| None),
        };
        if !matches!(
            iterator,
            Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
        ) {
            return Err(Error::type_error(format!(
                "iterator '{iterator}' is not an object"
            )));
        }
        let next = self.get_property_value(&iterator, ITERATOR_NEXT_PROPERTY)?;
        Ok(Some(ForOfSource::Protocol {
            iterator,
            next,
            done: false,
        }))
    }

    pub(in crate::runtime) fn for_of_step(
        &mut self,
        source: &mut ForOfSource,
    ) -> Result<ForOfStep> {
        match source {
            ForOfSource::ArrayIndex { array, index } => {
                let Value::Object(id) = array else {
                    return Err(Error::runtime("for-of array source is not an object"));
                };
                let Some(len) = self.objects.array_len_if_array(*id)? else {
                    return Ok(ForOfStep::Done);
                };
                if *index >= len {
                    return Ok(ForOfStep::Done);
                }
                let key = index.to_string();
                *index = index
                    .checked_add(1)
                    .ok_or_else(|| Error::runtime("for-of array index overflowed"))?;
                let array = array.clone();
                Ok(ForOfStep::Value(self.get_property_value(&array, &key)?))
            }
            ForOfSource::Chars { chars } => match chars.next() {
                Some(ch) => Ok(ForOfStep::Value(self.heap_string_char_value(ch)?)),
                None => Ok(ForOfStep::Done),
            },
            ForOfSource::Protocol {
                iterator,
                next,
                done,
            } => {
                if *done {
                    return Ok(ForOfStep::Done);
                }
                let next = next.clone();
                let iterator = iterator.clone();
                let result = match self.eval_call_completion(&next, &[], iterator)? {
                    Completion::Normal(value) => value,
                    Completion::Throw(value) => {
                        // A throw from next() ends iteration without close.
                        set_protocol_done(source);
                        return Ok(ForOfStep::Abrupt(Completion::Throw(value)));
                    }
                    completion => {
                        return completion.into_result().map(ForOfStep::Value);
                    }
                };
                if !matches!(
                    result,
                    Value::Object(_)
                        | Value::Function(_)
                        | Value::NativeFunction(_)
                        | Value::HostFunction(_)
                ) {
                    return Err(Error::type_error(format!(
                        "iterator result '{result}' is not an object"
                    )));
                }
                if to_boolean(&self.get_property_value(&result, ITERATOR_RESULT_DONE_PROPERTY)?) {
                    set_protocol_done(source);
                    return Ok(ForOfStep::Done);
                }
                Ok(ForOfStep::Value(self.get_property_value(
                    &result,
                    ITERATOR_RESULT_VALUE_PROPERTY,
                )?))
            }
        }
    }

    /// Calls the iterator's `return` method when the loop ends abruptly
    /// before exhaustion, mirroring `IteratorClose`. Errors raised by the
    /// close call are intentionally dropped so the original completion wins.
    pub(in crate::runtime) fn close_for_of_source(&mut self, source: &ForOfSource) {
        let ForOfSource::Protocol {
            iterator,
            done: false,
            ..
        } = source
        else {
            return;
        };
        let iterator = iterator.clone();
        let return_method = match self.get_property_value(&iterator, ITERATOR_RETURN_PROPERTY) {
            Ok(method) => method,
            Err(error) => {
                drop(error);
                return;
            }
        };
        if matches!(return_method, Value::Undefined | Value::Null) {
            return;
        }
        if let Err(error) = self.eval_call_completion(&return_method, &[], iterator) {
            drop(error);
        }
    }

    fn eval_for_of_lexical_binding(
        &mut self,
        name: &BytecodeBinding,
        kind: DeclKind,
        source: &mut ForOfSource,
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
            let value = match self.for_of_step(source)? {
                ForOfStep::Value(value) => value,
                ForOfStep::Done => break,
                ForOfStep::Abrupt(completion) => return Ok(completion),
            };
            let inserted = scope.insert_or_replace_at_optional_slot(
                atom,
                BindingCell::new(value, mutable, kind),
                frame.map(crate::runtime::CompiledBindingFrame::slot),
            )?;
            if let Some(frame) = frame {
                Self::mark_binding_scope_frame_slot(&mut scope, frame, inserted)?;
            }
            self.push_lexical_scope_with(scope);
            self.remember_active_static_binding(name.name(), atom)?;
            let completion = self.eval_bytecode_block(body);
            let Some(removed_scope) = self.pop_lexical_scope() else {
                return Err(Error::runtime("bytecode for-of lexical scope disappeared"));
            };
            scope = removed_scope;
            if let Some(completion) = bytecode_loop_completion(&mut last, completion?, labels) {
                self.close_for_of_source(source);
                return Ok(completion);
            }
        }
        Ok(Completion::Normal(last))
    }

    fn eval_for_of_assignment_loop(
        &mut self,
        source: &mut ForOfSource,
        body: &BytecodeBlock,
        labels: Option<&[StaticName]>,
        mut assign: impl FnMut(&mut Self, Value) -> Result<()>,
    ) -> Result<Completion> {
        let mut last = Value::Undefined;
        loop {
            self.step()?;
            let value = match self.for_of_step(source)? {
                ForOfStep::Value(value) => value,
                ForOfStep::Done => break,
                ForOfStep::Abrupt(completion) => return Ok(completion),
            };
            assign(self, value)?;
            if let Some(completion) =
                bytecode_loop_completion(&mut last, self.eval_bytecode_block(body)?, labels)
            {
                self.close_for_of_source(source);
                return Ok(completion);
            }
        }
        Ok(Completion::Normal(last))
    }
}

fn chars_source(text: &str) -> ForOfSource {
    ForOfSource::Chars {
        chars: text.chars().collect::<Vec<_>>().into_iter(),
    }
}

fn not_iterable_error(value: &Value) -> Error {
    Error::type_error(format!("'{value}' is not iterable"))
}

const fn set_protocol_done(source: &mut ForOfSource) {
    if let ForOfSource::Protocol { done, .. } = source {
        *done = true;
    }
}
