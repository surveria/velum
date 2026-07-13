use std::{collections::VecDeque, rc::Rc};

use crate::api::host::HostFunction;
use crate::api::native_call::NativeCallTarget;
use crate::binding_metadata::BindingLayout;
use crate::bytecode::{BytecodeBinding, BytecodeCallSite, BytecodeFunction, BytecodeNewTargetMode};
use crate::error::{Error, Result};
use crate::ownership::VmIdentity;
use crate::runtime::binding::scope::{BindingCell, BindingScope};
use crate::runtime::control::{Completion, reference_error_undefined};
use crate::runtime::limits::RuntimeLimits;
use crate::runtime::object::ObjectHeap;
use crate::runtime::property::enumerable_property_keys;
use crate::storage::atom::{AtomId, AtomTable};
use crate::storage::string_heap::StringHeap;
use crate::storage::symbol::{SymbolId, SymbolTable};
use crate::syntax::{FunctionKind, StaticBindingId};
use crate::value::{ErrorName, FunctionId, Value};

mod abstract_operations;
mod accounting;
mod activation;
mod arena;
mod async_disposable_stack;
mod async_operation;
mod async_trace;
pub mod binding;
pub mod bytecode;
pub mod call;
mod clock;
pub(in crate::runtime) mod collection_array_iterator;
mod collection_live_iterator;
mod collection_regexp_iterator;
mod collection_storage;
pub mod collections;
pub mod control;
mod disposable_stack;
mod dynamic_import;
pub mod engine;
mod execution_storage;
pub mod function;
mod gc;
pub(in crate::runtime) mod generator;
pub mod globals;
pub mod limits;
mod module;
pub mod native;
pub mod numeric;
pub mod object;
mod optimizer;
mod private;
pub mod promise;
pub mod property;
mod realm;
mod resource_scope;
pub mod retained_values;
mod roots;
mod script_execution;
mod semantic_object;
mod storage_ledger;
mod trace;
mod transient_roots;
pub mod values;

pub use accounting::{VmStorageKind, VmStorageSnapshot};
use arena::SlotArena;
pub use async_trace::{VmAsyncEdgeKind, VmAsyncEdgeSnapshot, VmAsyncEdgeStrength};
pub use binding::static_bindings::CompiledBindingFrame;
use binding::static_bindings::StaticBindingCacheHandle;
use call::{BoundFunction, RuntimeCallArgs};
pub use gc::{VmGarbageCollectionReport, VmGcKind, VmHeapReachabilitySnapshot};
use native::NativeFunctionKind;
use optimizer::Optimizer;
pub use optimizer::{OptimizationMode, VmOptimizationSnapshot};
use promise::{Promise, PromiseId, PromiseJob};
use property::static_names::{CallValueCache, StaticNameAtomCacheHandle};
use property::well_known::{DescriptorPropertyKeys, WellKnownPropertyKeys};
pub use realm::RealmId;
use realm::{RealmIndex, RealmState};
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
    well_known_symbols: Vec<(&'static str, SymbolId)>,
    well_known_properties: WellKnownPropertyKeys,
    /// VM-local id of the well-known `Symbol.iterator` symbol, cached when the
    /// `Symbol` builtin installs its well-known symbol properties.
    iterator_symbol: Option<crate::storage::symbol::SymbolId>,
    descriptor_property_keys: Option<DescriptorPropertyKeys>,
    static_name_atom_caches: Vec<StaticNameAtomCacheHandle>,
    static_binding_caches: Vec<StaticBindingCacheHandle>,
    static_binding_layouts: Vec<BindingLayout>,
    active_realm: RealmIndex,
    realm: RealmState,
    inactive_realms: Vec<Option<RealmState>>,
    locals: Vec<BindingScope>,
    modules: Vec<module::ModuleRecord>,
    module_evaluation_depth: usize,
    dynamic_module_loader: Option<module::DynamicModuleLoader>,
    active_module_name: Option<String>,
    active_import_meta: Option<Value>,
    activation_frames: Vec<activation::ActivationFrame>,
    functions: SlotArena<Function>,
    native_functions: SlotArena<native::NativeFunction>,
    bound_functions: SlotArena<BoundFunction>,
    pub(crate) host_functions: SlotArena<HostFunction>,
    pub(crate) objects: ObjectHeap,
    collections: SlotArena<collections::CollectionData>,
    collection_object_slots: Vec<Option<(collections::CollectionKind, collections::CollectionId)>>,
    collection_iterators: SlotArena<collections::CollectionIteratorState>,
    generators: SlotArena<generator::GeneratorData>,
    generator_object_slots: Vec<Option<generator::GeneratorId>>,
    promises: SlotArena<Promise>,
    promise_object_slots: Vec<Option<PromiseId>>,
    promise_jobs: VecDeque<PromiseJob>,
    retained_values: RetainedValueRegistry,
    transient_roots: TransientRootRegistry,
    output: Vec<String>,
    output_payload_bytes: usize,
    performance_clock: clock::PerformanceClock,
    random_state: u64,
    runtime_steps: usize,
    agent_can_block: bool,
    optimizer: Optimizer,
    call_depth: usize,
}

#[derive(Debug, Clone)]
struct Function {
    realm: RealmIndex,
    script_or_module_name: Option<String>,
    self_binding: Option<function::FunctionSelfBinding>,
    arguments_binding: Option<function::FunctionArgumentsBinding>,
    param_binding_ids: Rc<[StaticBindingId]>,
    param_atoms: Rc<[AtomId]>,
    param_frames: Rc<[Option<CompiledBindingFrame>]>,
    bytecode: BytecodeFunction,
    fast_path: Option<Rc<function::FunctionFastPath>>,
    source: Option<Rc<str>>,
    upvalues: FunctionUpvalues,
    with_environments: Rc<[Value]>,
    static_name_atom_cache: Option<StaticNameAtomCacheHandle>,
    static_binding_cache: Option<StaticBindingCacheHandle>,
    static_binding_layout: Option<BindingLayout>,
    properties: function::FunctionProperties,
    constructable: bool,
    kind: FunctionKind,
    class_constructor: bool,
    super_binding: Option<Rc<function::FunctionSuperBinding>>,
    static_parent: Option<Value>,
    class_fields: Option<Rc<[function::ResolvedClassField]>>,
    class_private_slots: Option<Rc<[private::PrivateSlot]>>,
    private_environment: Option<Rc<private::PrivateEnvironment>>,
    private_slots: Vec<private::PrivateSlot>,
    params_remembered: std::cell::Cell<bool>,
    scope_template: Option<Rc<function::FunctionScopeTemplate>>,
    lexical_this: Option<Value>,
    new_target: FunctionNewTarget,
}

type FunctionUpvalues = Rc<[BindingCell]>;
type FunctionActivationEnvironment = (FunctionUpvalues, Vec<Value>);

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
        id: crate::value::NativeFunctionId,
        kind: native::NativeFunctionKind,
        this_value: Value,
    },
    DirectNative {
        id: crate::value::NativeFunctionId,
        target: NativeCallTarget,
        this_value: Value,
        strict: bool,
    },
}

impl CallReference {
    fn with_this(self, this_value: Value) -> Self {
        match self {
            Self::Function { id, .. } => Self::Function { id, this_value },
            Self::Generic { callee, .. } => Self::Generic { callee, this_value },
            Self::Native { id, kind, .. } => Self::Native {
                id,
                kind,
                this_value,
            },
            Self::DirectNative {
                id, target, strict, ..
            } => Self::DirectNative {
                id,
                target,
                this_value,
                strict,
            },
        }
    }

    fn into_tail_call(self, arguments: Vec<Value>) -> control::TailCall {
        let (callee, this_value) = match self {
            Self::Function { id, this_value } => (Value::Function(id), this_value),
            Self::Generic { callee, this_value } => (callee, this_value),
            Self::Native { id, this_value, .. } | Self::DirectNative { id, this_value, .. } => {
                (Value::NativeFunction(id), this_value)
            }
        };
        control::TailCall::new(callee, arguments, this_value)
    }
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

    /// Configures whether this VM's current agent may block in `Atomics.wait`.
    pub const fn set_agent_can_block(&mut self, can_block: bool) {
        self.agent_can_block = can_block;
    }

    pub(crate) const fn agent_can_block(&self) -> bool {
        self.agent_can_block
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
        Self::with_optimization(limits, OptimizationMode::Enabled)
    }

    /// Creates a context with an explicit optional-optimization policy.
    #[must_use]
    pub fn with_optimization(limits: RuntimeLimits, mode: OptimizationMode) -> Self {
        Self::with_performance_clock(limits, mode, clock::PerformanceClock::system())
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
        Self::with_optimization_and_monotonic_clock(limits, OptimizationMode::Enabled, read)
    }

    /// Creates a configured context with an embedder-provided monotonic clock.
    #[must_use]
    pub fn with_optimization_and_monotonic_clock<F>(
        limits: RuntimeLimits,
        mode: OptimizationMode,
        read: F,
    ) -> Self
    where
        F: Fn() -> std::time::Duration + 'static,
    {
        Self::with_performance_clock(limits, mode, clock::PerformanceClock::from_reader(read))
    }

    /// Returns the optimizer-owned policy and profiling counters.
    #[must_use]
    pub const fn optimization_snapshot(&self) -> VmOptimizationSnapshot {
        self.optimizer.snapshot()
    }

    pub(in crate::runtime) const fn optional_optimizations_enabled(&self) -> bool {
        self.optimizer.optional_paths_enabled() && self.inactive_realms.len() == 1
    }

    fn with_performance_clock(
        limits: RuntimeLimits,
        optimization_mode: OptimizationMode,
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
            well_known_symbols: Vec::new(),
            well_known_properties: WellKnownPropertyKeys::new(),
            iterator_symbol: None,
            descriptor_property_keys: None,
            static_name_atom_caches: Vec::new(),
            static_binding_caches: Vec::new(),
            static_binding_layouts: Vec::new(),
            active_realm: RealmIndex::ROOT,
            realm: RealmState::new(storage_ledger.clone()),
            inactive_realms: vec![None],
            locals: Vec::new(),
            modules: Vec::new(),
            module_evaluation_depth: 0,
            dynamic_module_loader: None,
            active_module_name: None,
            active_import_meta: None,
            activation_frames: Vec::new(),
            functions: SlotArena::new(),
            native_functions: SlotArena::new(),
            bound_functions: SlotArena::new(),
            host_functions: SlotArena::new(),
            objects: ObjectHeap::new(storage_limits, storage_ledger.clone()),
            collections: SlotArena::new(),
            collection_object_slots: Vec::new(),
            collection_iterators: SlotArena::new(),
            generators: SlotArena::new(),
            generator_object_slots: Vec::new(),
            promises: SlotArena::new(),
            promise_object_slots: Vec::new(),
            promise_jobs: VecDeque::new(),
            retained_values: RetainedValueRegistry::new(identity, storage_ledger.clone()),
            transient_roots: TransientRootRegistry::new(storage_ledger),
            output: Vec::new(),
            output_payload_bytes: 0,
            performance_clock,
            random_state: INITIAL_RANDOM_STATE,
            runtime_steps: 0,
            agent_can_block: false,
            optimizer: Optimizer::new(optimization_mode),
            call_depth: 0,
        }
    }

    pub(crate) fn eval_cached_call_completion(
        &mut self,
        site: BytecodeCallSite,
        callee: &Value,
        args: &[Value],
        this_value: Value,
    ) -> Result<Completion> {
        if !self.optional_optimizations_enabled() {
            return self.call(callee, args, this_value);
        }
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
            CallValueCache::NativeFunction { function, kind } => self
                .eval_native_function_in_realm(function, kind, args, &this_value)
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
        strict: bool,
        args: &[Value],
    ) -> Result<Completion> {
        let reference = self.eval_bytecode_identifier_call_reference(callee, native, strict)?;
        self.eval_call_reference_completion(reference, args)
    }

    pub(crate) fn eval_bytecode_identifier_tail_call(
        &mut self,
        callee: &BytecodeBinding,
        native: Option<NativeCallTarget>,
        strict: bool,
        args: &[Value],
    ) -> Result<control::TailCall> {
        self.eval_bytecode_identifier_call_reference(callee, native, strict)
            .map(|reference| reference.into_tail_call(args.to_vec()))
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
            CallReference::DirectNative {
                id,
                target,
                this_value,
                strict,
            } => {
                let realm = self.native_function(id)?.realm();
                let value = self.with_realm(realm, |context| {
                    if target == NativeCallTarget::Eval {
                        context
                            .eval_eval_function_with_strict(RuntimeCallArgs::values(args), strict)
                    } else {
                        context.eval_direct_native_call_target(target, args, &this_value)
                    }
                })?;
                Ok(Completion::Normal(value))
            }
            CallReference::Native {
                id,
                kind,
                this_value,
            } => self
                .eval_native_function_in_realm(id, kind, args, &this_value)
                .map(Completion::Normal),
            CallReference::Generic { callee, this_value } => self.call(&callee, args, this_value),
        }
    }

    fn eval_bytecode_identifier_call_reference(
        &mut self,
        callee: &BytecodeBinding,
        native: Option<NativeCallTarget>,
        strict: bool,
    ) -> Result<CallReference> {
        if let Some(reference) = self.resolve_with_binding(callee)? {
            let function = reference.get(self, callee)?;
            let this_value = reference.object().clone();
            return self
                .call_reference_from_value(callee, native, strict, function)
                .map(|reference| reference.with_this(this_value));
        }
        if let Some(function) = self.unresolved_direct_builtin_callable(callee)? {
            return self.call_reference_from_value(callee, native, strict, function);
        }
        let function = if let Some(binding) = self.get_or_materialize_binding_bytecode(callee)? {
            binding.value(callee.name())?
        } else {
            self.unresolved_global_property_value(callee.name().name())?
                .ok_or_else(|| reference_error_undefined(callee.name()))?
        };
        self.call_reference_from_value(callee, native, strict, function)
    }

    fn call_reference_from_value(
        &self,
        callee: &BytecodeBinding,
        native: Option<NativeCallTarget>,
        strict: bool,
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
                && (self.direct_native_call_kind(id, target).is_some()
                    || (target == NativeCallTarget::Eval
                        && self.native_function(id)?.kind() == NativeFunctionKind::Eval))
            {
                return Ok(CallReference::DirectNative {
                    id,
                    target,
                    this_value: Value::Undefined,
                    strict,
                });
            }
            if let Some(kind) = self.cached_static_binding_native_call_kind(callee.name(), id)? {
                return Ok(CallReference::Native {
                    id,
                    kind,
                    this_value: Value::Undefined,
                });
            }
            let kind = self.native_function(id)?.kind();
            self.remember_static_binding_native_call_kind(callee.name(), id, kind)?;
            return Ok(CallReference::Native {
                id,
                kind,
                this_value: Value::Undefined,
            });
        }
        Ok(CallReference::Generic {
            callee: function,
            this_value: Value::Undefined,
        })
    }

    pub(crate) fn enumerable_keys(&mut self, object: &Value) -> Result<Vec<String>> {
        if matches!(
            object,
            Value::Function(_) | Value::NativeFunction(_) | Value::HostFunction(_)
        ) || matches!(object, Value::Object(id) if self.objects.is_proxy(*id))
        {
            return self.semantic_enumerable_property_keys(object);
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
        let realm = self.function(id)?.realm;
        self.with_realm(realm, |context| {
            context.eval_function_constructor_value_in_active_realm(id, args, new_target)
        })
    }

    fn eval_function_constructor_value_in_active_realm(
        &mut self,
        id: crate::value::FunctionId,
        args: RuntimeCallArgs<'_>,
        new_target: Value,
    ) -> Result<Value> {
        let prototype = self
            .constructor_instance_prototype_with_default(&new_target, NativeFunctionKind::Object)?;
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype(
            Some(prototype),
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
            Completion::Return(value) | Completion::ReturnDirect(value) => {
                if self.semantic_object_ref(&value)?.is_some() {
                    return Ok(value);
                }
                Ok(object)
            }
            Completion::Normal(_) => Ok(object),
            Completion::Throw(value) => Err(Error::javascript(value)),
            Completion::TailCall(_) => Err(Error::runtime("tail call escaped constructor")),
            Completion::Break { .. } => Err(Error::runtime("break statement outside loop")),
            Completion::Continue { .. } => Err(Error::runtime("continue statement outside loop")),
            completion @ (Completion::Suspended(_)
            | Completion::GeneratorStart
            | Completion::Yielded(_)
            | Completion::YieldedIteratorResult(_)) => {
                completion.into_function_result().map(|_| object)
            }
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
