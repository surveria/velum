use std::{collections::VecDeque, rc::Rc};

use crate::api::host::HostFunction;
use crate::api::native_call::NativeCallTarget;
use crate::binding_layout::BindingLayout;
use crate::bytecode::{BytecodeBinding, BytecodeCallSite, BytecodeFunction, BytecodeNewTargetMode};
use crate::compiled_script::CompiledScript;
use crate::error::{Error, Result};
use crate::runtime::assertions::reference_error_undefined;
use crate::runtime::binding::scope::{BindingCell, BindingScope};
use crate::runtime::completion::Completion;
use crate::runtime::limits::RuntimeLimits;
use crate::runtime::object::ObjectHeap;
use crate::runtime::property::enumerable_property_keys;
use crate::storage::atom::{AtomId, AtomTable};
use crate::storage::string_heap::StringHeap;
use crate::storage::symbol::SymbolTable;
use crate::syntax::StaticBindingId;
use crate::value::{ErrorName, Value};

pub mod assertions;
pub mod binding;
mod bound;
pub mod bytecode;
pub mod call_args;
pub mod completion;
pub mod engine;
pub mod function;
pub mod globals;
pub mod limits;
pub mod native;
pub mod numeric;
pub mod object;
pub mod promise;
pub mod property;
pub mod values;

pub use binding::static_bindings::CompiledBindingFrame;
use binding::static_bindings::StaticBindingCacheHandle;
use bound::BoundFunction;
use call_args::RuntimeCallArgs;
use native::{NativeFunctionKind, NativeFunctionRegistry};
use promise::{Promise, PromiseId, PromiseJob};
use property::static_names::{CallValueCache, StaticNameAtomCacheHandle};
use property::well_known::{DescriptorPropertyKeys, WellKnownPropertyKeys};

const INITIAL_RANDOM_STATE: u64 = 0x9e37_79b9_7f4a_7c15;
const TEST262_ERROR_NAME: &str = "Test262Error";

#[derive(Debug, Clone)]
pub struct Context {
    limits: RuntimeLimits,
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
    upvalue_frames: Vec<FunctionUpvalues>,
    functions: Vec<Function>,
    native_functions: Vec<native::NativeFunction>,
    native_function_registry: NativeFunctionRegistry,
    bound_functions: Vec<BoundFunction>,
    pub(crate) host_functions: Vec<HostFunction>,
    objects: ObjectHeap,
    promises: Vec<Promise>,
    promise_object_slots: Vec<Option<PromiseId>>,
    promise_jobs: VecDeque<PromiseJob>,
    promise_prototype: Option<crate::value::ObjectId>,
    this_values: Vec<Value>,
    new_target_values: Vec<Value>,
    output: Vec<String>,
    random_state: u64,
    runtime_steps: usize,
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
    bytecode: BytecodeFunction,
    source: Option<Rc<str>>,
    upvalues: FunctionUpvalues,
    static_name_atom_cache: Option<StaticNameAtomCacheHandle>,
    static_binding_cache: Option<StaticBindingCacheHandle>,
    static_binding_layout: Option<BindingLayout>,
    properties: function::FunctionProperties,
    constructable: bool,
    is_async: bool,
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
    pub(crate) const fn set_iterator_symbol(&mut self, symbol: crate::storage::symbol::SymbolId) {
        self.iterator_symbol = Some(symbol);
    }

    pub(crate) const fn iterator_symbol(&self) -> Option<crate::storage::symbol::SymbolId> {
        self.iterator_symbol
    }

    #[must_use]
    pub const fn new(limits: RuntimeLimits) -> Self {
        Self {
            limits,
            atoms: AtomTable::new(),
            strings: StringHeap::new(),
            symbols: SymbolTable::new(),
            well_known_properties: WellKnownPropertyKeys::new(),
            iterator_symbol: None,
            descriptor_property_keys: None,
            static_name_atom_caches: Vec::new(),
            static_binding_caches: Vec::new(),
            static_binding_layouts: Vec::new(),
            globals: BindingScope::new(),
            builtin_globals: BindingScope::new(),
            locals: Vec::new(),
            upvalue_frames: Vec::new(),
            functions: Vec::new(),
            native_functions: Vec::new(),
            native_function_registry: NativeFunctionRegistry::new(),
            bound_functions: Vec::new(),
            host_functions: Vec::new(),
            objects: ObjectHeap::new(),
            promises: Vec::new(),
            promise_object_slots: Vec::new(),
            promise_jobs: VecDeque::new(),
            promise_prototype: None,
            this_values: Vec::new(),
            new_target_values: Vec::new(),
            output: Vec::new(),
            random_state: INITIAL_RANDOM_STATE,
            runtime_steps: 0,
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
    /// Fails when lexing, parsing, evaluation, or configured resource limits fail.
    pub fn eval(&mut self, source: &str) -> Result<Value> {
        let script = self.compile(source)?;
        self.eval_compiled(&script)
    }

    /// # Errors
    /// Fails when lexing, parsing, or configured compile-time resource limits fail.
    pub fn compile(&self, source: &str) -> Result<CompiledScript> {
        CompiledScript::compile(source, self.limits)
    }

    /// # Errors
    /// Fails when the compiled script exceeds this context's limits or evaluation fails.
    pub fn eval_compiled(&mut self, script: &CompiledScript) -> Result<Value> {
        self.eval_compiled_completion(script)?.into_result()
    }

    pub(crate) fn eval_compiled_completion(
        &mut self,
        script: &CompiledScript,
    ) -> Result<Completion> {
        script.ensure_within_limits(self.limits)?;
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
                let completion = context.eval_bytecode_program(script.bytecode())?;
                if matches!(completion, Completion::Normal(_)) {
                    context.drain_promise_jobs()?;
                }
                Ok(completion)
            },
        )
    }

    pub(crate) fn eval_call_value(
        &mut self,
        callee: Value,
        args: &[Value],
        this_value: Value,
    ) -> Result<Value> {
        match callee {
            Value::Function(id) => {
                self.eval_function_with_this(id, RuntimeCallArgs::values(args), this_value)
            }
            Value::NativeFunction(id) => {
                let kind = self.native_function(id)?.kind();
                self.eval_direct_or_generic_native_function_kind(kind, args, &this_value)
            }
            Value::HostFunction(id) => self.eval_host_function(id, RuntimeCallArgs::values(args)),
            value => Err(Error::type_error(format!("'{value}' is not callable"))),
        }
    }

    pub(crate) fn eval_call_completion(
        &mut self,
        callee: Value,
        args: &[Value],
        this_value: Value,
    ) -> Result<Completion> {
        match callee {
            Value::Function(id) => self.eval_function_call_completion_with_this(
                id,
                RuntimeCallArgs::values(args),
                this_value,
            ),
            Value::NativeFunction(id) => {
                let kind = self.native_function(id)?.kind();
                self.eval_direct_or_generic_native_function_kind(kind, args, &this_value)
                    .map(Completion::Normal)
            }
            Value::HostFunction(id) => self
                .eval_host_function(id, RuntimeCallArgs::values(args))
                .map(Completion::Normal),
            value => Err(Error::type_error(format!("'{value}' is not callable"))),
        }
    }

    pub(crate) fn eval_cached_call_completion(
        &mut self,
        site: BytecodeCallSite,
        callee: Value,
        args: &[Value],
        this_value: Value,
    ) -> Result<Completion> {
        let site = site.site();
        if let Some(cache) = self.cached_call_value(site)? {
            if cache.matches_callee(&callee) {
                self.record_call_value_cache_hit();
                return self.eval_call_completion_cache(cache, args, this_value);
            }
            self.record_call_value_cache_slow_path();
        } else {
            self.record_call_value_cache_miss();
        }

        let Some(cache) = self.cacheable_call_value(&callee)? else {
            return self.eval_call_completion(callee, args, this_value);
        };
        self.remember_call_value(site, cache)?;
        self.eval_call_completion_cache(cache, args, this_value)
    }

    fn cacheable_call_value(&self, callee: &Value) -> Result<Option<CallValueCache>> {
        let native_kind = if let Value::NativeFunction(id) = callee {
            Some(self.native_function(*id)?.kind())
        } else {
            None
        };
        Ok(CallValueCache::from_callee(callee, native_kind))
    }

    fn eval_call_completion_cache(
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
            CallReference::DirectNative { target, this_value } => self
                .eval_direct_native_call_target(target, args, &this_value)
                .map(Completion::Normal),
            CallReference::Native { kind, this_value } => self
                .eval_direct_or_generic_native_function_kind(kind, args, &this_value)
                .map(Completion::Normal),
            CallReference::Generic { callee, this_value } => {
                self.eval_call_completion(callee, args, this_value)
            }
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
        let line = args
            .as_slice()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        self.check_string_len(&line)?;
        self.output.push(line);
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

    pub(crate) fn eval_new_value(&mut self, constructor: Value, args: &[Value]) -> Result<Value> {
        match constructor {
            Value::Function(id) => {
                self.eval_function_constructor_value(id, RuntimeCallArgs::values(args))
            }
            Value::NativeFunction(id) => {
                self.construct_native_function(id, RuntimeCallArgs::values(args))
            }
            value => Err(Error::type_error(format!("'{value}' is not a constructor"))),
        }
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
        let Value::Function(id) = value else {
            if let Value::NativeFunction(id) = value {
                if let Some(target) = native
                    && let Some(kind) = self.direct_native_call_kind(id, target)
                {
                    if kind == NativeFunctionKind::Function {
                        return self.eval_direct_function_constructor(args);
                    }
                    return self
                        .construct_native_function_kind(kind, RuntimeCallArgs::values(args));
                }
                return self.construct_native_function(id, RuntimeCallArgs::values(args));
            }
            return Err(Error::type_error(format!(
                "'{}' is not a constructor",
                constructor.name()
            )));
        };
        self.eval_function_constructor_value(id, RuntimeCallArgs::values(args))
    }

    fn eval_function_constructor_value(
        &mut self,
        id: crate::value::FunctionId,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let prototype = self.function_constructor_prototype(id)?;
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype(
            prototype,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        match self.eval_function_completion_with_this_and_new_target(
            id,
            args,
            object.clone(),
            Value::Function(id),
        )? {
            Completion::Return(value) if Self::constructor_return_is_object(&value) => Ok(value),
            Completion::Normal(_) | Completion::Return(_) => Ok(object),
            Completion::Throw(value) => Err(Error::runtime(format!("uncaught throw: {value}"))),
            Completion::Break { .. } => Err(Error::runtime("break statement outside loop")),
            Completion::Continue(_) => Err(Error::runtime("continue statement outside loop")),
        }
    }

    const fn constructor_return_is_object(value: &Value) -> bool {
        matches!(
            value,
            Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
                | Value::Object(_)
                | Value::Error(_)
        )
    }

    pub(crate) fn push_lexical_scope(&mut self) {
        self.locals.push(BindingScope::new());
    }

    pub(crate) fn push_lexical_scope_with(&mut self, scope: BindingScope) {
        self.locals.push(scope);
    }

    pub(crate) fn pop_lexical_scope(&mut self) -> Option<BindingScope> {
        self.locals.pop()
    }
}
