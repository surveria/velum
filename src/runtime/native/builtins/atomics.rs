use std::time::Duration;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::{ATOMICS_NAME, AtomicsFunctionKind, NativeFunctionKind},
        numeric::number_to_uint32,
        object::{
            AtomicWaitOutcome, ByteBuffer, DataPropertyUpdate, PropertyConfigurable,
            PropertyEnumerable, PropertyKey, PropertyUpdate, PropertyWritable,
            TypedArrayContentType, TypedArrayElementKind,
        },
    },
    value::{ErrorName, JsBigInt, ObjectId, Value},
};

const RECEIVER_ERROR: &str = "Atomics requires an integer shared typed array";
const INDEX_ERROR: &str = "Atomics index is out of range";
const MILLISECONDS_PER_SECOND: f64 = 1_000.0;

#[derive(Debug, Clone, Copy)]
enum AtomicUpdate {
    Add,
    And,
    Exchange,
    Or,
    Sub,
    Xor,
}

struct AtomicOperand {
    raw: u64,
    returned: Value,
}

struct AtomicLocation {
    buffer: ByteBuffer,
    offset: usize,
    kind: TypedArrayElementKind,
}

impl Context {
    pub(in crate::runtime) fn atomics_object_value(&mut self) -> Result<Value> {
        if let Some(binding) = self.get_binding(ATOMICS_NAME) {
            return binding.value(ATOMICS_NAME);
        }
        self.object_constructor_value()?;
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype_id(
            None,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        for kind in AtomicsFunctionKind::ALL {
            let method =
                self.create_native_function(NativeFunctionKind::Atomics(kind), Value::Undefined)?;
            self.define_non_enumerable_object_property(object, kind.name(), method)?;
        }
        self.define_atomics_to_string_tag(object)?;
        let value = Value::Object(object);
        self.insert_global_builtin(ATOMICS_NAME, value.clone())?;
        Ok(value)
    }

    pub(in crate::runtime::native) fn eval_atomics_native_function_kind(
        &mut self,
        kind: AtomicsFunctionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        match kind {
            AtomicsFunctionKind::Add => self.eval_atomic_update(args, AtomicUpdate::Add),
            AtomicsFunctionKind::And => self.eval_atomic_update(args, AtomicUpdate::And),
            AtomicsFunctionKind::CompareExchange => self.eval_atomic_compare_exchange(args),
            AtomicsFunctionKind::Exchange => self.eval_atomic_update(args, AtomicUpdate::Exchange),
            AtomicsFunctionKind::IsLockFree => self.eval_atomic_is_lock_free(args),
            AtomicsFunctionKind::Load => self.eval_atomic_load(args),
            AtomicsFunctionKind::Notify => self.eval_atomic_notify(args),
            AtomicsFunctionKind::Or => self.eval_atomic_update(args, AtomicUpdate::Or),
            AtomicsFunctionKind::Pause => self.eval_atomic_pause(args),
            AtomicsFunctionKind::Store => self.eval_atomic_store(args),
            AtomicsFunctionKind::Sub => self.eval_atomic_update(args, AtomicUpdate::Sub),
            AtomicsFunctionKind::Wait => self.eval_atomic_wait(args),
            AtomicsFunctionKind::WaitAsync => self.eval_atomic_wait_async(args),
            AtomicsFunctionKind::Xor => self.eval_atomic_update(args, AtomicUpdate::Xor),
        }
    }

    fn eval_atomic_load(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let location = self.atomic_location(args.as_slice(), false)?;
        let raw = location.buffer.with_exclusive_bytes_mut(|bytes| {
            read_atomic_word(bytes, location.offset, location.kind)
        })?;
        self.atomic_word_value(location.kind, raw)
    }

    fn eval_atomic_store(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let location = self.atomic_location(args.as_slice(), false)?;
        let value = args.as_slice().get(2).unwrap_or(&Value::Undefined);
        let operand = self.atomic_operand(location.kind, value)?;
        location.buffer.with_exclusive_bytes_mut(|bytes| {
            write_atomic_word(bytes, location.offset, location.kind, operand.raw)
        })?;
        Ok(operand.returned)
    }

    fn eval_atomic_update(
        &mut self,
        args: RuntimeCallArgs<'_>,
        update: AtomicUpdate,
    ) -> Result<Value> {
        let location = self.atomic_location(args.as_slice(), false)?;
        let operand = self.atomic_operand(
            location.kind,
            args.as_slice().get(2).unwrap_or(&Value::Undefined),
        )?;
        let mask = atomic_mask(location.kind);
        let old = location.buffer.with_exclusive_bytes_mut(|bytes| {
            let old = read_atomic_word(bytes, location.offset, location.kind)?;
            let next = match update {
                AtomicUpdate::Add => old.wrapping_add(operand.raw),
                AtomicUpdate::And => old & operand.raw,
                AtomicUpdate::Exchange => operand.raw,
                AtomicUpdate::Or => old | operand.raw,
                AtomicUpdate::Sub => old.wrapping_sub(operand.raw),
                AtomicUpdate::Xor => old ^ operand.raw,
            } & mask;
            write_atomic_word(bytes, location.offset, location.kind, next)?;
            Ok(old)
        })?;
        self.atomic_word_value(location.kind, old)
    }

    fn eval_atomic_compare_exchange(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let location = self.atomic_location(args.as_slice(), false)?;
        let expected = self.atomic_operand(
            location.kind,
            args.as_slice().get(2).unwrap_or(&Value::Undefined),
        )?;
        let replacement = self.atomic_operand(
            location.kind,
            args.as_slice().get(3).unwrap_or(&Value::Undefined),
        )?;
        let old = location.buffer.with_exclusive_bytes_mut(|bytes| {
            let old = read_atomic_word(bytes, location.offset, location.kind)?;
            if old == expected.raw {
                write_atomic_word(bytes, location.offset, location.kind, replacement.raw)?;
            }
            Ok(old)
        })?;
        self.atomic_word_value(location.kind, old)
    }

    fn eval_atomic_is_lock_free(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let size =
            self.to_integer_or_infinity(args.as_slice().first().unwrap_or(&Value::Undefined))?;
        Ok(Value::Bool(matches!(size, 1.0 | 2.0 | 4.0 | 8.0)))
    }

    fn eval_atomic_notify(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let location = self.atomic_location(args.as_slice(), true)?;
        if !location.buffer.is_shared() {
            return Ok(Value::Number(0.0));
        }
        let count = self.atomic_notify_count(args.as_slice().get(2))?;
        let notified = location.buffer.notify_at(location.offset, count)?;
        Self::atomic_count_value(notified)
    }

    fn eval_atomic_wait(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let location = self.atomic_location(args.as_slice(), true)?;
        if !location.buffer.is_shared() {
            return Err(Error::type_error("Atomics.wait requires shared storage"));
        }
        let expected = self.atomic_operand(
            location.kind,
            args.as_slice().get(2).unwrap_or(&Value::Undefined),
        )?;
        let current = location.buffer.with_exclusive_bytes_mut(|bytes| {
            read_atomic_word(bytes, location.offset, location.kind)
        })?;
        if current != expected.raw {
            return self.heap_string_value("not-equal");
        }
        let timeout = self.atomic_wait_timeout(args.as_slice().get(3))?;
        let outcome = location.buffer.wait_at(location.offset, timeout)?;
        self.heap_string_value(match outcome {
            AtomicWaitOutcome::Notified => "ok",
            AtomicWaitOutcome::TimedOut => "timed-out",
        })
    }

    fn eval_atomic_wait_async(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let location = self.atomic_location(args.as_slice(), true)?;
        if !location.buffer.is_shared() {
            return Err(Error::type_error(
                "Atomics.waitAsync requires shared storage",
            ));
        }
        let expected = self.atomic_operand(
            location.kind,
            args.as_slice().get(2).unwrap_or(&Value::Undefined),
        )?;
        let current = location.buffer.with_exclusive_bytes_mut(|bytes| {
            read_atomic_word(bytes, location.offset, location.kind)
        })?;
        let timeout = if let Some(value) = args.as_slice().get(3) {
            self.to_number(value)?
        } else {
            f64::INFINITY
        };
        let result = if current != expected.raw {
            self.heap_string_value("not-equal")?
        } else if timeout <= 0.0 {
            self.heap_string_value("timed-out")?
        } else {
            return Err(Error::type_error(
                "asynchronous Atomics waiting requires an embedder agent coordinator",
            ));
        };
        let object = self.create_atomics_result_object()?;
        self.define_enumerable_data_property(object, "async", Value::Bool(false))?;
        self.define_enumerable_data_property(object, "value", result)?;
        Ok(Value::Object(object))
    }

    fn eval_atomic_pause(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        if let Some(value) = args
            .as_slice()
            .first()
            .filter(|value| !matches!(value, Value::Undefined))
        {
            let count = self.to_number(value)?;
            if !count.is_finite() || count < 0.0 || count.fract() != 0.0 {
                return Err(Error::exception(
                    ErrorName::RangeError,
                    "Atomics.pause iteration count is invalid",
                ));
            }
        }
        Ok(Value::Undefined)
    }

    fn atomic_location(&mut self, args: &[Value], waitable: bool) -> Result<AtomicLocation> {
        let Value::Object(id) = args.first().unwrap_or(&Value::Undefined) else {
            return Err(Error::type_error(RECEIVER_ERROR));
        };
        let Some(view) = self.objects.typed_array(*id)? else {
            return Err(Error::type_error(RECEIVER_ERROR));
        };
        if view.is_out_of_bounds() || !is_atomic_element_kind(view.element_kind(), waitable) {
            return Err(Error::type_error(RECEIVER_ERROR));
        }
        let length = view.length();
        let byte_offset = view.byte_offset();
        let index = Self::length_to_usize(self.to_index(args.get(1))?, INDEX_ERROR)?;
        if index >= length {
            return Err(Error::exception(ErrorName::RangeError, INDEX_ERROR));
        }
        let relative = index
            .checked_mul(view.element_kind().bytes_per_element())
            .ok_or_else(|| Error::limit(INDEX_ERROR))?;
        let offset = byte_offset
            .checked_add(relative)
            .ok_or_else(|| Error::limit(INDEX_ERROR))?;
        Ok(AtomicLocation {
            buffer: view.buffer().clone(),
            offset,
            kind: view.element_kind(),
        })
    }

    fn atomic_operand(
        &mut self,
        kind: TypedArrayElementKind,
        value: &Value,
    ) -> Result<AtomicOperand> {
        if kind.content_type() == TypedArrayContentType::BigInt {
            let bigint = self.to_bigint(value)?;
            let Some(raw) = bigint.as_uint_n(64).to_u64() else {
                return Err(Error::runtime("Atomics BigInt conversion overflowed"));
            };
            return Ok(AtomicOperand {
                raw,
                returned: self.bigint_value(bigint)?,
            });
        }
        let integer = self.to_integer_or_infinity(value)?;
        let raw = u64::from(number_to_uint32(integer, "Atomics operand")?) & atomic_mask(kind);
        Ok(AtomicOperand {
            raw,
            returned: Value::Number(integer),
        })
    }

    fn atomic_notify_count(&mut self, value: Option<&Value>) -> Result<usize> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(usize::MAX);
        };
        let count = self.to_integer_or_infinity(value)?;
        if count <= 0.0 {
            return Ok(0);
        }
        if !count.is_finite() || count >= f64::from(u32::MAX) {
            return Ok(usize::MAX);
        }
        Self::finite_nonnegative_integer_to_usize(count, "Atomics notify count is invalid")
    }

    fn atomic_wait_timeout(&mut self, value: Option<&Value>) -> Result<Option<Duration>> {
        let timeout = if let Some(value) = value {
            self.to_number(value)?
        } else {
            f64::INFINITY
        };
        if timeout.is_nan() || timeout == f64::INFINITY {
            return Ok(None);
        }
        let milliseconds = timeout.max(0.0);
        Ok(Duration::try_from_secs_f64(milliseconds / MILLISECONDS_PER_SECOND).ok())
    }

    fn atomic_count_value(value: usize) -> Result<Value> {
        let count = u32::try_from(value)
            .map_err(|_| Error::limit("Atomics notified waiter count exceeded supported range"))?;
        Ok(Value::Number(f64::from(count)))
    }

    fn atomic_word_value(&self, kind: TypedArrayElementKind, raw: u64) -> Result<Value> {
        let number = match kind {
            TypedArrayElementKind::Int8 => {
                let byte = u8::try_from(raw & u64::from(u8::MAX))
                    .map_err(|_| Error::runtime("Atomics Int8 conversion overflowed"))?;
                return Ok(Value::Number(f64::from(i8::from_ne_bytes([byte]))));
            }
            TypedArrayElementKind::Uint8 => f64::from(
                u8::try_from(raw)
                    .map_err(|_| Error::runtime("Atomics Uint8 conversion overflowed"))?,
            ),
            TypedArrayElementKind::Int16 => {
                let word = u16::try_from(raw & u64::from(u16::MAX))
                    .map_err(|_| Error::runtime("Atomics Int16 conversion overflowed"))?;
                return Ok(Value::Number(f64::from(i16::from_ne_bytes(
                    word.to_ne_bytes(),
                ))));
            }
            TypedArrayElementKind::Uint16 => f64::from(
                u16::try_from(raw)
                    .map_err(|_| Error::runtime("Atomics Uint16 conversion overflowed"))?,
            ),
            TypedArrayElementKind::Int32 => {
                let word = u32::try_from(raw)
                    .map_err(|_| Error::runtime("Atomics Int32 conversion overflowed"))?;
                return Ok(Value::Number(f64::from(i32::from_ne_bytes(
                    word.to_ne_bytes(),
                ))));
            }
            TypedArrayElementKind::Uint32 => f64::from(
                u32::try_from(raw)
                    .map_err(|_| Error::runtime("Atomics Uint32 conversion overflowed"))?,
            ),
            TypedArrayElementKind::BigInt64 => {
                let word = i64::from_ne_bytes(raw.to_ne_bytes());
                return self.bigint_value(JsBigInt::from_i64(word));
            }
            TypedArrayElementKind::BigUint64 => {
                return self.bigint_value(JsBigInt::from_u64(raw));
            }
            TypedArrayElementKind::Uint8Clamped
            | TypedArrayElementKind::Float32
            | TypedArrayElementKind::Float64 => return Err(Error::type_error(RECEIVER_ERROR)),
        };
        Ok(Value::Number(number))
    }

    fn define_atomics_to_string_tag(&mut self, object: ObjectId) -> Result<()> {
        let symbol = self.symbol_constructor_value()?;
        let Value::Symbol(tag) = self.get_named(&symbol, "toStringTag")? else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let value = self.heap_string_value(ATOMICS_NAME)?;
        self.objects.define_property(
            object,
            PropertyKey::symbol(tag.id()),
            "[Symbol.toStringTag]",
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn create_atomics_result_object(&mut self) -> Result<ObjectId> {
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_with_prototype_id(
            None,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn define_enumerable_data_property(
        &mut self,
        object: ObjectId,
        name: &str,
        value: Value,
    ) -> Result<()> {
        let key = self.intern_property_key(name)?;
        self.objects.define_property(
            object,
            key,
            name,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::Yes),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }
}

const fn is_atomic_element_kind(kind: TypedArrayElementKind, waitable: bool) -> bool {
    if waitable {
        return matches!(
            kind,
            TypedArrayElementKind::Int32 | TypedArrayElementKind::BigInt64
        );
    }
    matches!(
        kind,
        TypedArrayElementKind::Int8
            | TypedArrayElementKind::Uint8
            | TypedArrayElementKind::Int16
            | TypedArrayElementKind::Uint16
            | TypedArrayElementKind::Int32
            | TypedArrayElementKind::Uint32
            | TypedArrayElementKind::BigInt64
            | TypedArrayElementKind::BigUint64
    )
}

fn atomic_mask(kind: TypedArrayElementKind) -> u64 {
    match kind {
        TypedArrayElementKind::Int8 | TypedArrayElementKind::Uint8 => u64::from(u8::MAX),
        TypedArrayElementKind::Int16 | TypedArrayElementKind::Uint16 => u64::from(u16::MAX),
        TypedArrayElementKind::Int32 | TypedArrayElementKind::Uint32 => u64::from(u32::MAX),
        TypedArrayElementKind::BigInt64 | TypedArrayElementKind::BigUint64 => u64::MAX,
        TypedArrayElementKind::Uint8Clamped
        | TypedArrayElementKind::Float32
        | TypedArrayElementKind::Float64 => 0,
    }
}

fn read_atomic_word(bytes: &[u8], offset: usize, kind: TypedArrayElementKind) -> Result<u64> {
    let size = kind.bytes_per_element();
    let end = offset
        .checked_add(size)
        .ok_or_else(|| Error::limit(INDEX_ERROR))?;
    let source = bytes
        .get(offset..end)
        .ok_or_else(|| Error::exception(ErrorName::RangeError, INDEX_ERROR))?;
    let raw = match size {
        1 => u64::from(
            *source
                .first()
                .ok_or_else(|| Error::exception(ErrorName::RangeError, INDEX_ERROR))?,
        ),
        2 => u64::from(u16::from_ne_bytes(
            source
                .try_into()
                .map_err(|_| Error::runtime("Atomics 16-bit read failed"))?,
        )),
        4 => u64::from(u32::from_ne_bytes(
            source
                .try_into()
                .map_err(|_| Error::runtime("Atomics 32-bit read failed"))?,
        )),
        8 => u64::from_ne_bytes(
            source
                .try_into()
                .map_err(|_| Error::runtime("Atomics 64-bit read failed"))?,
        ),
        _ => return Err(Error::type_error(RECEIVER_ERROR)),
    };
    Ok(raw)
}

fn write_atomic_word(
    bytes: &mut [u8],
    offset: usize,
    kind: TypedArrayElementKind,
    raw: u64,
) -> Result<()> {
    let size = kind.bytes_per_element();
    let end = offset
        .checked_add(size)
        .ok_or_else(|| Error::limit(INDEX_ERROR))?;
    let target = bytes
        .get_mut(offset..end)
        .ok_or_else(|| Error::exception(ErrorName::RangeError, INDEX_ERROR))?;
    let raw_bytes = raw.to_ne_bytes();
    let source = raw_bytes
        .get(..size)
        .ok_or_else(|| Error::runtime("Atomics write width is invalid"))?;
    target.copy_from_slice(source);
    Ok(())
}
