use std::{collections::VecDeque, rc::Rc};

use crate::api::host::HostFunction;
use crate::api::native_call::NativeCallTarget;
use crate::binding_metadata::BindingLayout;
use crate::bytecode::{BytecodeBinding, BytecodeCallSite, BytecodeFunction, BytecodeNewTargetMode};
use crate::compiled_script::CompiledScript;
use crate::error::{Error, Result};
use crate::ownership::VmIdentity;
use crate::runtime::binding::scope::{BindingCell, BindingScope};
use crate::runtime::control::{Completion, reference_error_undefined};
use crate::runtime::limits::RuntimeLimits;
use crate::runtime::object::ObjectHeap;
use crate::runtime::property::enumerable_property_keys;
use crate::storage::atom::{AtomId, AtomTable};
use crate::storage::string_heap::StringHeap;
use crate::storage::symbol::SymbolTable;
use crate::syntax::StaticBindingId;
use crate::value::{ErrorName, FunctionId, Value};

mod abstract_operations;
mod accounting;
mod activation;
mod async_trace;
pub mod binding;
pub mod bytecode;
pub mod call;
mod clock;
pub mod collections;
pub mod control;
pub mod engine;
mod execution_storage;
pub mod function;
pub mod globals;
pub mod limits;
pub mod native;
pub mod numeric;
pub mod object;
pub mod promise;
pub mod property;
pub mod retained_values;
mod roots;
mod semantic_object;
mod storage_ledger;
mod trace;
mod transient_roots;
pub mod values;

pub use accounting::{VmStorageKind, VmStorageSnapshot};
pub use async_trace::{VmAsyncEdgeKind, VmAsyncEdgeSnapshot, VmAsyncEdgeStrength};
pub use binding::static_bindings::CompiledBindingFrame;
use binding::static_bindings::StaticBindingCacheHandle;
use bytecode::BytecodeOutcome;
use call::{BoundFunction, RuntimeCallArgs};
use native::{NativeFunctionKind, NativeFunctionRegistry};
use promise::{Promise, PromiseId, PromiseJob};
use property::static_names::{CallValueCache, StaticNameAtomCacheHandle};
use property::well_known::{DescriptorPropertyKeys, WellKnownPropertyKeys};
pub use retained_values::RetainedValue;
use retained_values::RetainedValueRegistry;
pub use roots::{VmRootKind, VmRootSnapshot};
use storage_ledger::VmStorageLedger;
pub use trace::{
    VmCallableEdgeKind, VmCallableEdgeSnapshot, VmObjectEdgeKind, VmObjectEdgeSnapshot,
};
use transient_roots::TransientRootRegistry;

const INITIAL_RANDOM_STATE: u64 = 0x9e37_79b9_7f4a_7c15;
const CONSTRUCTOR_PROTOTYPE_PROPERTY: &str = "prototype";
const TEST262_ERROR_NAME: &str = "Test262Error";

#[derive(Debug)]
pub struct Context {
    identity: VmIdentity,
    limits: RuntimeLimits,
    storage_ledger: VmStorageLedger,
    atoms: AtomTable,
    strings: StringHeap,
    symbols: SymbolTable,
    well_known_properties: WellKnownPropertyKeys,
    /// VM-local id of the well-known `Symbol.iterator` symbol, cached when the
    /// `Symbol` builtin installs its well-known symbol properties.
    iterator_symbol: Option<crate::storage::symbol::SymbolId>,
    descriptor_property_keys: Option<DescriptorPropertyKeys>,
    static_name_atom_caches: Vec<StaticNameAtomCacheHandle>,
    static_binding_caches: Vec<StaticBindingCacheHandle>,
    static_binding_layouts: Vec<BindingLayout>,
    globals: BindingScope,
    builtin_globals: BindingScope,
    locals: Vec<BindingScope>,
    activation_frames: Vec<activation::ActivationFrame>,
    functions: Vec<Function>,
    native_functions: Vec<native::NativeFunction>,
    native_function_registry: NativeFunctionRegistry,
    bound_functions: Vec<BoundFunction>,
    pub(crate) host_functions: Vec<HostFunction>,
    objects: ObjectHeap,
    global_object: Option<crate::value::ObjectId>,
    collections: Vec<collections::CollectionData>,
    collection_object_slots: Vec<Option<(collections::CollectionKind, collections::CollectionId)>>,
    collection_iterators: Vec<collections::CollectionIteratorState>,
    promises: Vec<Promise>,
    promise_object_slots: Vec<Option<PromiseId>>,
    promise_jobs: VecDeque<PromiseJob>,
    promise_prototype: Option<crate::value::ObjectId>,
    retained_values: RetainedValueRegistry,
    transient_roots: TransientRootRegistry,
    output: Vec<String>,
    output_payload_bytes: usize,
    performance_clock: clock::PerformanceClock,
    random_state: u64,
    runtime_steps: usize,
    bytecode_linear_segment_runs: usize,
    bytecode_linear_direct_runs: usize,
    call_depth: usize,
    native_call_cache_hits: usize,
    native_call_cache_misses: usize,
    native_call_cache_slow_paths: usize,
    call_value_cache_hits: usize,
    call_value_cache_misses: usize,
    call_value_cache_slow_paths: usize,
}

#[derive(Debug, Clone)]
struct Function {
    param_binding_ids: Rc<[StaticBindingId]>,
    param_atoms: Rc<[AtomId]>,
    param_frames: Rc<[Option<CompiledBindingFrame>]>,
    bytecode: BytecodeFunction,
    fast_path: Option<Rc<function::FunctionFastPath>>,
    source: Option<Rc<str>>,
    upvalues: FunctionUpvalues,
    static_name_atom_cache: Option<StaticNameAtomCacheHandle>,
    static_binding_cache: Option<StaticBindingCacheHandle>,
    static_binding_layout: Option<BindingLayout>,
    properties: function::FunctionProperties,
    constructable: bool,
    is_async: bool,
    class_constructor: bool,
    super_binding: Option<Rc<function::FunctionSuperBinding>>,
    static_parent: Option<Value>,
    class_fields: Option<Rc<[function::ResolvedClassField]>>,
    params_remembered: std::cell::Cell<bool>,
    scope_template: Option<Rc<function::FunctionScopeTemplate>>,
    new_target: FunctionNewTarget,
}

type FunctionUpvalues = Rc<[BindingCell]>;

#[derive(Debug, Clone)]
enum FunctionNewTarget {
    Own,
    Lexical(Value),
}

impl FunctionNewTarget {
    fn from_mode(mode: BytecodeNewTargetMode, lexical_value: Value) -> Self {
        match mode {
            BytecodeNewTargetMode::Own => Self::Own,
            BytecodeNewTargetMode::Lexical => Self::Lexical(lexical_value),
        }
    }
}

#[derive(Debug, Clone)]
struct CapturedFunctionUpvalues {
    cells: FunctionUpvalues,
}

impl CapturedFunctionUpvalues {
    const fn new(cells: FunctionUpvalues) -> Self {
        Self { cells }
    }
}

enum CallReference {
    Function {
        id: FunctionId,
        this_value: Value,
    },
    Generic {
        callee: Value,
        this_value: Value,
    },
    Native {
        kind: native::NativeFunctionKind,
        this_value: Value,
    },
    DirectNative {
        target: NativeCallTarget,
        this_value: Value,
    },
}

#[derive(Debug, Clone, Copy)]
struct FunctionArity(usize);

impl FunctionArity {
    const fn new(value: usize) -> Self {
        Self(value)
    }

    const fn as_usize(self) -> usize {
        self.0
    }
}

impl Context {
    /// Returns the opaque identity of this VM-owned storage generation.
    #[must_use]
    pub const fn identity(&self) -> &VmIdentity {
        &self.identity
    }

    pub(crate) fn current_local_frame_start(&self) -> usize {
        self.activation_frames
            .iter()
            .rev()
            .find_map(activation::ActivationFrame::local_base)
            .unwrap_or(0)
    }

    pub(crate) fn visible_local_scope_count(&self) -> usize {
        self.locals
            .len()
            .saturating_sub(self.current_local_frame_start())
    }

    pub(crate) fn has_visible_local_scope(&self) -> bool {
        self.visible_local_scope_count() > 0
    }

    pub(crate) fn set_iterator_symbol(
        &mut self,
        symbol: crate::storage::symbol::SymbolId,
    ) -> Result<()> {
        if self.iterator_symbol.is_none() {
            self.storage_ledger
                .grow_count(VmStorageKind::Association, 1)?;
        }
        self.iterator_symbol = Some(symbol);
        Ok(())
    }

    pub(crate) const fn iterator_symbol(&self) -> Option<crate::storage::symbol::SymbolId> {
        self.iterator_symbol
    }

    #[must_use]
    pub fn new(limits: RuntimeLimits) -> Self {
        Self::with_performance_clock(limits, clock::PerformanceClock::system())
    }

    /// Creates a context whose VM-local `performance.now()` uses `read` as its
    /// monotonic source. The first reading becomes this context's zero point.
    /// Later source regressions are clamped so JavaScript observes a
    /// non-decreasing value.
    #[must_use]
    pub fn with_monotonic_clock<F>(limits: RuntimeLimits, read: F) -> Self
    where
        F: Fn() -> std::time::Duration + 'static,
    {
        Self::with_performance_clock(limits, clock::PerformanceClock::from_reader(read))
    }

    fn with_performance_clock(
        limits: RuntimeLimits,
        performance_clock: clock::PerformanceClock,
    ) -> Self {
        let identity = VmIdentity::new();
        let storage_limits = limits.storage.clone();
        let storage_ledger = VmStorageLedger::new(storage_limits.clone());
        Self {
            identity: identity.clone(),
            limits,
            storage_ledger: storage_ledger.clone(),
            atoms: AtomTable::new(
                storage_limits.max_count(VmStorageKind::Atom),
                storage_limits.max_payload_bytes(VmStorageKind::Atom),
            ),
            strings: StringHeap::new(
                identity.clone(),
                storage_limits.max_count(VmStorageKind::HeapString),
                storage_limits.max_payload_bytes(VmStorageKind::HeapString),
            ),
            symbols: SymbolTable::new(
                identity.clone(),
                storage_limits.max_count(VmStorageKind::Symbol),
            ),
            well_known_properties: WellKnownPropertyKeys::new(),
            iterator_symbol: None,
            descriptor_property_keys: None,
            static_name_atom_caches: Vec::new(),
            static_binding_caches: Vec::new(),
            static_binding_layouts: Vec::new(),
            globals: BindingScope::new_active(storage_ledger.clone()),
            builtin_globals: BindingScope::new_active(storage_ledger.clone()),
            locals: Vec::new(),
            activation_frames: Vec::new(),
            functions: Vec::new(),
            native_functions: Vec::new(),
            native_function_registry: NativeFunctionRegistry::new(),
            bound_functions: Vec::new(),
            host_functions: Vec::new(),
            objects: ObjectHeap::new(storage_limits, storage_ledger.clone()),
            global_object: None,
            collections: Vec::new(),
            collection_object_slots: Vec::new(),
            collection_iterators: Vec::new(),
            promises: Vec::new(),
            promise_object_slots: Vec::new(),
            promise_jobs: VecDeque::new(),
            promise_prototype: None,
            retained_values: RetainedValueRegistry::new(identity, storage_ledger.clone()),
            transient_roots: TransientRootRegistry::new(storage_ledger),
            output: Vec::new(),
            output_payload_bytes: 0,
            performance_clock,
            random_state: INITIAL_RANDOM_STATE,
            runtime_steps: 0,
            bytecode_linear_segment_runs: 0,
            bytecode_linear_direct_runs: 0,
            call_depth: 0,
            native_call_cache_hits: 0,
            native_call_cache_misses: 0,
            native_call_cache_slow_paths: 0,
            call_value_cache_hits: 0,
            call_value_cache_misses: 0,
            call_value_cache_slow_paths: 0,
        }
    }

    /// # Errors
    /// Fails when lexing, parsing, evaluation, or configured resource limits
    /// fail. An uncaught JavaScript value is returned as
    /// [`Error::JavaScript`](crate::Error::JavaScript).
    ///
    /// The returned raw value is not a durable root. Use `eval_owned` for a
    /// portable primitive or `eval_retained` across later Context calls.
    pub fn eval(&mut self, source: &str) -> Result<Value> {
        let script = self.compile(source)?;
        self.eval_compiled(&script)
    }

    /// # Errors
    /// Fails when lexing, parsing, or configured compile-time resource limits fail.
    pub fn compile(&self, source: &str) -> Result<CompiledScript> {
        CompiledScript::compile(source, self.limits.clone())
    }

    /// Compiles source with a stable embedder-provided diagnostic name.
    ///
    /// # Errors
    /// Fails when the source name exceeds configured string limits, or when
    /// lexing, parsing, or configured compile-time resource limits fail.
    pub fn compile_named(&self, source_name: &str, source: &str) -> Result<CompiledScript> {
        CompiledScript::compile_named(source_name, source, self.limits.clone())
    }

    /// # Errors
    /// Fails when the compiled script exceeds this context's limits or evaluation fails.
    pub fn eval_compiled(&mut self, script: &CompiledScript) -> Result<Value> {
        let outcome = self.eval_compiled_outcome(script)?;
        let span = outcome.span();
        let completion = outcome.completion();
        let Completion::Throw(value) = completion else {
            let result = completion.into_result();
            return if let Some(span) = span {
                result.map_err(|error| error.with_runtime_span(span))
            } else {
                result
            };
        };
        let metadata = if let Value::Object(id) = &value {
            self.objects.error_metadata(*id)?.cloned()
        } else {
            None
        };
        Err(Error::javascript_with_metadata(
            self.identity.clone(),
            value,
            metadata,
            span,
        ))
    }

    pub(crate) fn eval_compiled_completion(
        &mut self,
        script: &CompiledScript,
    ) -> Result<Completion> {
        self.eval_compiled_outcome(script)
            .map(BytecodeOutcome::completion)
    }

    fn eval_compiled_outcome(&mut self, script: &CompiledScript) -> Result<BytecodeOutcome> {
        script.ensure_within_limits(&self.limits)?;
        let static_name_cache = StaticNameAtomCacheHandle::new(
            script.usage().static_name_count(),
            script.usage().static_property_access_count(),
            script.usage().static_call_site_count(),
        );
        let binding_cache = StaticBindingCacheHandle::new(script.binding_layout().operand_count());
        self.with_static_name_caches(
            static_name_cache,
            binding_cache,
            script.binding_layout().clone(),
            |context| {
                context.hoist_bytecode_declarations(script.bytecode().hoist_plan())?;
                let outcome = context.eval_bytecode_program(script.bytecode())?;
                if outcome.is_normal() {
                    context.drain_promise_jobs()?;
                }
                Ok(outcome)
            },
        )
    }

    pub(crate) fn eval_cached_call_completion(
        &mut self,
        site: BytecodeCallSite,
        callee: &Value,
        args: &[Value],
        this_value: Value,
    ) -> Result<Completion> {
        let site = site.site();
        if let Some(cache) = self.cached_call_value(site)? {
            if cache.matches_callee(callee) {
                self.record_call_value_cache_hit();
                return self.call_cache(cache, args, this_value);
            }
            self.record_call_value_cache_slow_path();
        } else {
            self.record_call_value_cache_miss();
        }

        let Some(cache) = self.cacheable_call_value(callee)? else {
            return self.call(callee, args, this_value);
        };
        self.remember_call_value(site, cache)?;
        self.call_cache(cache, args, this_value)
    }

    fn cacheable_call_value(&self, callee: &Value) -> Result<Option<CallValueCache>> {
        let native_kind = if let Value::NativeFunction(id) = callee {
            Some(self.native_function(*id)?.kind())
        } else {
            None
        };
        Ok(CallValueCache::from_callee(callee, native_kind))
    }

    fn call_cache(
        &mut self,
        cache: CallValueCache,
        args: &[Value],
        this_value: Value,
    ) -> Result<Completion> {
        match cache {
            CallValueCache::Function(id) => self.eval_function_call_completion_with_this(
                id,
                RuntimeCallArgs::values(args),
                this_value,
            ),
            CallValueCache::NativeFunction { kind, .. } => self
                .eval_direct_or_generic_native_function_kind(kind, args, &this_value)
                .map(Completion::Normal),
            CallValueCache::HostFunction(id) => self
                .eval_host_function(id, RuntimeCallArgs::values(args))
                .map(Completion::Normal),
        }
    }

    pub(crate) fn eval_bytecode_identifier_call_completion(
        &mut self,
        callee: &BytecodeBinding,
        native: Option<NativeCallTarget>,
        args: &[Value],
    ) -> Result<Completion> {
        let reference = self.eval_bytecode_identifier_call_reference(callee, native)?;
        self.eval_call_reference_completion(reference, args)
    }

    fn eval_call_reference_completion(
        &mut self,
        reference: CallReference,
        args: &[Value],
    ) -> Result<Completion> {
        match reference {
            CallReference::Function { id, this_value } => self
                .eval_function_call_completion_with_this(
                    id,
                    RuntimeCallArgs::values(args),
                    this_value,
                ),
            CallReference::DirectNative { target, this_value } => self
                .eval_direct_native_call_target(target, args, &this_value)
                .map(Completion::Normal),
            CallReference::Native { kind, this_value } => self
                .eval_direct_or_generic_native_function_kind(kind, args, &this_value)
                .map(Completion::Normal),
            CallReference::Generic { callee, this_value } => self.call(&callee, args, this_value),
        }
    }

    fn eval_bytecode_identifier_call_reference(
        &mut self,
        callee: &BytecodeBinding,
        native: Option<NativeCallTarget>,
    ) -> Result<CallReference> {
        if let Some(function) = self.unresolved_direct_builtin_callable(callee)? {
            return self.call_reference_from_value(callee, native, function);
        }
        let Some(binding) = self.get_or_materialize_binding_bytecode(callee)? else {
            return Err(reference_error_undefined(callee.name()));
        };
        let function = binding.value(callee.name())?;
        self.call_reference_from_value(callee, native, function)
    }

    fn call_reference_from_value(
        &self,
        callee: &BytecodeBinding,
        native: Option<NativeCallTarget>,
        function: Value,
    ) -> Result<CallReference> {
        if let Value::Function(id) = function {
            return Ok(CallReference::Function {
                id,
                this_value: Value::Undefined,
            });
        }
        if let Value::NativeFunction(id) = function {
            if let Some(target) = native
                && self.direct_native_call_kind(id, target).is_some()
            {
                return Ok(CallReference::DirectNative {
                    target,
                    this_value: Value::Undefined,
                });
            }
            if let Some(kind) = self.cached_static_binding_native_call_kind(callee.name(), id)? {
                return Ok(CallReference::Native {
                    kind,
                    this_value: Value::Undefined,
                });
            }
            let kind = self.native_function(id)?.kind();
            self.remember_static_binding_native_call_kind(callee.name(), id, kind)?;
            return Ok(CallReference::Native {
                kind,
                this_value: Value::Undefined,
            });
        }
        Ok(CallReference::Generic {
            callee: function,
            this_value: Value::Undefined,
        })
    }

    pub(crate) fn enumerable_keys(&self, object: &Value) -> Result<Vec<String>> {
        if let Value::Function(id) = object {
            return self.function_enumerable_keys(*id);
        }
        if let Value::NativeFunction(id) = object {
            return self.native_function_enumerable_keys(*id);
        }
        enumerable_property_keys(&self.objects, &self.atoms, object)
    }

    pub(crate) fn eval_print_call(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let mut values = Vec::with_capacity(args.as_slice().len());
        for value in args.as_slice() {
            values.push(self.to_string(value)?);
        }
        let line = values.join(" ");
        self.check_string_len(&line)?;
        let projected_count = self
            .output
            .len()
            .checked_add(1)
            .ok_or_else(|| Error::limit("output entry count overflowed"))?;
        let projected_payload_bytes = self
            .output_payload_bytes
            .checked_add(line.len())
            .ok_or_else(|| Error::limit("output payload bytes overflowed"))?;
        self.ensure_storage_totals(
            VmStorageKind::OutputEntry,
            projected_count,
            projected_payload_bytes,
        )?;
        self.output.push(line);
        self.output_payload_bytes = projected_payload_bytes;
        Ok(Value::Undefined)
    }

    pub(crate) fn eval_bytecode_new_value(
        &mut self,
        constructor: &BytecodeBinding,
        native: Option<NativeCallTarget>,
        args: &[Value],
    ) -> Result<Value> {
        if constructor.name().as_str() != TEST262_ERROR_NAME {
            return self.eval_bytecode_function_constructor(constructor, native, args);
        }
        if self.get_binding_bytecode(constructor)?.is_some() {
            return self.eval_bytecode_function_constructor(constructor, native, args);
        }
        self.eval_error_constructor(ErrorName::Test262Error, RuntimeCallArgs::values(args))
    }

    pub(crate) fn eval_new_value(&mut self, constructor: &Value, args: &[Value]) -> Result<Value> {
        self.semantic_construct(constructor, args, constructor.clone())
    }

    fn eval_bytecode_function_constructor(
        &mut self,
        constructor: &BytecodeBinding,
        native: Option<NativeCallTarget>,
        args: &[Value],
    ) -> Result<Value> {
        let value = self
            .constructor_binding_bytecode(constructor)?
            .ok_or_else(|| reference_error_undefined(constructor.name()))?;
        if let Value::NativeFunction(id) = value
            && let Some(target) = native
            && let Some(kind) = self.direct_native_call_kind(id, target)
        {
            if kind == NativeFunctionKind::Function {
                return self.eval_direct_function_constructor(args);
            }
            return self.construct_native_function_kind(kind, RuntimeCallArgs::values(args));
        }
        self.semantic_construct(&value, args, value.clone())
    }

    pub(in crate::runtime) fn eval_function_constructor_value(
        &mut self,
        id: crate::value::FunctionId,
        args: RuntimeCallArgs<'_>,
        new_target: Value,
    ) -> Result<Value> {
        let prototype = self.constructor_instance_prototype(&new_target)?;
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype(
            prototype,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        if !self.is_derived_class_constructor(id) {
            self.initialize_class_fields(id, &object)?;
        }
        match self.eval_function_completion_with_this_and_new_target(
            id,
            args,
            object.clone(),
            new_target,
        )? {
            Completion::Return(value) => {
                if self.semantic_object_ref(&value)?.is_some() {
                    return Ok(value);
                }
                Ok(object)
            }
            Completion::Normal(_) => Ok(object),
            Completion::Throw(value) => Err(Error::javascript(value)),
            Completion::Break { .. } => Err(Error::runtime("break statement outside loop")),
            Completion::Continue(_) => Err(Error::runtime("continue statement outside loop")),
        }
    }

    fn constructor_instance_prototype(
        &mut self,
        new_target: &Value,
    ) -> Result<Option<crate::value::ObjectId>> {
        let prototype = self.get_named(new_target, CONSTRUCTOR_PROTOTYPE_PROPERTY)?;
        let Value::Object(id) = prototype else {
            return Ok(None);
        };
        self.objects.validate_id(id)?;
        Ok(Some(id))
    }
}
