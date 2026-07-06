use std::rc::Rc;

use crate::api::host::HostFunction;
use crate::api::native_call::NativeCallTarget;
use crate::ast::StaticBindingId;
use crate::binding_layout::BindingLayout;
use crate::bytecode::{BytecodeBinding, BytecodeFunction};
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
use crate::value::{ErrorName, Value};

pub mod assertions;
pub mod binding;
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
pub mod property;
pub mod values;

pub use binding::static_bindings::CompiledBindingFrame;
use binding::static_bindings::StaticBindingCacheHandle;
use call_args::RuntimeCallArgs;
use native::NativeFunctionRegistry;
use property::static_names::StaticNameAtomCacheHandle;
use property::well_known::{DescriptorPropertyKeys, WellKnownPropertyKeys};

const INITIAL_RANDOM_STATE: u64 = 0x9e37_79b9_7f4a_7c15;
const TEST262_ERROR_NAME: &str = "Test262Error";

#[derive(Debug, Clone)]
pub struct Context {
    limits: RuntimeLimits,
    atoms: AtomTable,
    strings: StringHeap,
    well_known_properties: WellKnownPropertyKeys,
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
    pub(crate) host_functions: Vec<HostFunction>,
    objects: ObjectHeap,
    this_values: Vec<Value>,
    output: Vec<String>,
    random_state: u64,
    runtime_steps: usize,
    call_depth: usize,
    native_call_cache_hits: usize,
    native_call_cache_misses: usize,
    native_call_cache_fallbacks: usize,
}

#[derive(Debug, Clone)]
struct Function {
    param_binding_ids: Rc<[StaticBindingId]>,
    param_atoms: Rc<[AtomId]>,
    bytecode: BytecodeFunction,
    captures: FunctionCaptures,
    upvalues: FunctionUpvalues,
    static_name_atom_cache: Option<StaticNameAtomCacheHandle>,
    static_binding_cache: Option<StaticBindingCacheHandle>,
    static_binding_layout: Option<BindingLayout>,
    properties: function::FunctionProperties,
    constructable: bool,
}

type FunctionUpvalues = Rc<[Option<BindingCell>]>;

#[derive(Debug, Clone)]
struct CapturedFunctionUpvalues {
    cells: FunctionUpvalues,
    needs_legacy_scope_fallback: bool,
}

impl CapturedFunctionUpvalues {
    const fn new(cells: FunctionUpvalues, needs_legacy_scope_fallback: bool) -> Self {
        Self {
            cells,
            needs_legacy_scope_fallback,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct FunctionCaptures {
    scopes: Vec<BindingScope>,
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
}

impl FunctionCaptures {
    fn from_current_locals(
        locals: &[BindingScope],
        has_compiled_layout: bool,
        upvalues: &FunctionUpvalues,
        needs_legacy_scope_fallback: bool,
    ) -> Self {
        if has_compiled_layout
            && !needs_legacy_scope_fallback
            && upvalues.iter().all(Option::is_some)
        {
            return Self::default();
        }
        Self {
            scopes: locals.to_vec(),
        }
    }

    fn call_locals(&self) -> Vec<BindingScope> {
        self.scopes.clone()
    }

    const fn scope_count(&self) -> usize {
        self.scopes.len()
    }

    fn binding_count(&self) -> usize {
        self.scopes
            .iter()
            .fold(0usize, |count, scope| count.saturating_add(scope.len()))
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
    #[must_use]
    pub const fn new(limits: RuntimeLimits) -> Self {
        Self {
            limits,
            atoms: AtomTable::new(),
            strings: StringHeap::new(),
            well_known_properties: WellKnownPropertyKeys::new(),
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
            host_functions: Vec::new(),
            objects: ObjectHeap::new(),
            this_values: Vec::new(),
            output: Vec::new(),
            random_state: INITIAL_RANDOM_STATE,
            runtime_steps: 0,
            call_depth: 0,
            native_call_cache_hits: 0,
            native_call_cache_misses: 0,
            native_call_cache_fallbacks: 0,
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
        script.ensure_within_limits(self.limits)?;
        let static_name_cache = StaticNameAtomCacheHandle::new(
            script.usage().static_name_count(),
            script.usage().static_property_access_count(),
        );
        let binding_cache = StaticBindingCacheHandle::new(script.binding_layout().operand_count());
        self.with_static_name_caches(
            static_name_cache,
            binding_cache,
            script.binding_layout().clone(),
            |context| {
                context.hoist_bytecode_declarations(script.bytecode().hoist_plan())?;
                context
                    .eval_bytecode_program(script.bytecode())
                    .and_then(Completion::into_result)
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
                self.eval_native_function(id, RuntimeCallArgs::values(args), &this_value)
            }
            Value::HostFunction(id) => self.eval_host_function(id, RuntimeCallArgs::values(args)),
            value => Err(Error::runtime(format!("'{value}' is not callable"))),
        }
    }

    pub(crate) fn eval_bytecode_identifier_call_value(
        &mut self,
        callee: &BytecodeBinding,
        native: Option<NativeCallTarget>,
        args: &[Value],
    ) -> Result<Value> {
        let reference = self.eval_bytecode_identifier_call_reference(callee, native)?;
        self.eval_call_reference_result(reference, RuntimeCallArgs::values(args))
    }

    fn eval_call_reference_result(
        &mut self,
        reference: CallReference,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        match reference {
            CallReference::Native { kind, this_value } => {
                self.eval_native_function_kind(kind, args, &this_value)
            }
            CallReference::Generic { callee, this_value } => match callee {
                Value::Function(id) => self.eval_function_with_this(id, args, this_value),
                Value::NativeFunction(id) => self.eval_native_function(id, args, &this_value),
                Value::HostFunction(id) => self.eval_host_function(id, args),
                value => Err(Error::runtime(format!("'{value}' is not callable"))),
            },
        }
    }

    fn eval_bytecode_identifier_call_reference(
        &mut self,
        callee: &BytecodeBinding,
        native: Option<NativeCallTarget>,
    ) -> Result<CallReference> {
        let Some(binding) = self.get_or_materialize_binding_bytecode(callee)? else {
            return Err(reference_error_undefined(callee.name()));
        };
        let function = binding.value();
        if let Value::NativeFunction(id) = function {
            let kind = if let Some(target) = native
                && let Some(kind) = self.direct_native_call_kind(id, target)
            {
                kind
            } else if let Some(kind) =
                self.cached_static_binding_native_call_kind(callee.name(), id)?
            {
                kind
            } else {
                let kind = self.native_function(id)?.kind();
                self.remember_static_binding_native_call_kind(callee.name(), id, kind)?;
                kind
            };
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
        args: &[Value],
    ) -> Result<Value> {
        if constructor.name().as_str() != TEST262_ERROR_NAME {
            return self
                .eval_bytecode_function_constructor(constructor, RuntimeCallArgs::values(args));
        }
        self.eval_error_constructor(ErrorName::Test262Error, RuntimeCallArgs::values(args))
    }

    fn eval_bytecode_function_constructor(
        &mut self,
        constructor: &BytecodeBinding,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let value = self
            .constructor_binding_bytecode(constructor)?
            .ok_or_else(|| reference_error_undefined(constructor.name()))?;
        let Value::Function(id) = value else {
            if let Value::NativeFunction(id) = value {
                return self.construct_native_function(id, args);
            }
            return Err(Error::runtime(format!(
                "'{}' is not a constructor",
                constructor.name()
            )));
        };
        let prototype = self.function_constructor_prototype(id)?;
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype(
            prototype,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        match self.eval_function_completion_with_this(id, args, object.clone())? {
            Completion::Return(value) if Self::constructor_return_is_object(&value) => Ok(value),
            Completion::Normal(_) | Completion::Return(_) => Ok(object),
            Completion::Throw(value) => Err(Error::runtime(format!("uncaught throw: {value}"))),
            Completion::Break => Err(Error::runtime("break statement outside loop")),
            Completion::Continue => Err(Error::runtime("continue statement outside loop")),
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
