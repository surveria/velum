use std::rc::Rc;

use crate::ast::{
    BinaryOp, Expr, ObjectProperty, Program, StaticBinding, StaticBindingId, StaticName, Stmt,
};
use crate::atom::{AtomId, AtomTable};
use crate::binding_layout::BindingLayout;
use crate::compiled_script::CompiledScript;
use crate::error::{Error, Result};
use crate::host::HostFunction;
use crate::runtime_assertions::{
    expected_error_name, is_assert_throws_call, reference_error_undefined, runtime_exception_value,
    thrown_value_matches,
};
use crate::runtime_completion::Completion;
use crate::runtime_limits::RuntimeLimits;
use crate::runtime_numeric::{
    bitwise_and, bitwise_or, bitwise_xor, compare_binary, numeric_binary, shift_left, shift_right,
    shift_right_unsigned,
};
use crate::runtime_object::{
    OBJECT_CONSTRUCTOR_PROPERTY, ObjectHeap, ObjectPropertyInit, PropertyEnumerable,
};
use crate::runtime_property::enumerable_property_keys;
use crate::runtime_scope::{BindingCell, BindingScope};
use crate::string_heap::StringHeap;
use crate::value::{ErrorName, Value};

#[path = "runtime_binding_location.rs"]
mod runtime_binding_location;
#[path = "runtime_declaration.rs"]
mod runtime_declaration;
#[path = "runtime_dynamic_property.rs"]
mod runtime_dynamic_property;
#[path = "runtime_function.rs"]
mod runtime_function;
#[path = "runtime_function_intrinsic.rs"]
mod runtime_function_intrinsic;
#[path = "runtime_function_properties.rs"]
mod runtime_function_properties;
#[path = "runtime_function_upvalues.rs"]
mod runtime_function_upvalues;
#[path = "runtime_globals.rs"]
mod runtime_globals;
#[path = "runtime_native.rs"]
mod runtime_native;
#[path = "runtime_native_registry.rs"]
mod runtime_native_registry;
#[path = "runtime_static_bindings.rs"]
mod runtime_static_bindings;
#[path = "runtime_static_names.rs"]
mod runtime_static_names;
#[path = "runtime_values.rs"]
mod runtime_values;
#[path = "runtime_well_known.rs"]
mod runtime_well_known;

use runtime_native_registry::NativeFunctionRegistry;
pub use runtime_static_bindings::CompiledBindingFrame;
use runtime_static_bindings::StaticBindingCacheHandle;
use runtime_static_names::StaticNameAtomCacheHandle;
use runtime_well_known::{DescriptorPropertyKeys, WellKnownPropertyKeys};

const HOST_PRINT_NAME: &str = "print";
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
    native_functions: Vec<runtime_native::NativeFunction>,
    native_function_registry: NativeFunctionRegistry,
    pub(crate) host_functions: Vec<HostFunction>,
    objects: ObjectHeap,
    this_values: Vec<Value>,
    output: Vec<String>,
    random_state: u64,
    runtime_steps: usize,
}

#[derive(Debug, Clone)]
struct Function {
    param_binding_ids: Rc<[StaticBindingId]>,
    param_atoms: Rc<[AtomId]>,
    body: Rc<[Stmt]>,
    captures: FunctionCaptures,
    upvalues: FunctionUpvalues,
    static_name_atom_cache: Option<StaticNameAtomCacheHandle>,
    static_binding_cache: Option<StaticBindingCacheHandle>,
    static_binding_layout: Option<BindingLayout>,
    properties: runtime_function_properties::FunctionProperties,
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
        kind: runtime_native::NativeFunctionKind,
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
            |context| context.eval_program(script.program()),
        )
    }

    fn eval_program(&mut self, program: &Program) -> Result<Value> {
        self.hoist_var_declarations(&program.statements)?;
        self.eval_block(&program.statements)?.into_result()
    }

    pub(crate) fn eval_statement(&mut self, statement: &Stmt) -> Result<Completion> {
        match statement {
            Stmt::Block(statements) => self.eval_scoped_block(statements),
            Stmt::DeclList(declarations) => self.eval_declaration_list(declarations),
            Stmt::If {
                condition,
                consequent,
                alternate,
            } => {
                let condition = self.eval_expr(condition)?;
                if condition.is_truthy() {
                    self.eval_statement(consequent)
                } else if let Some(alternate) = alternate {
                    self.eval_statement(alternate)
                } else {
                    Ok(Completion::Normal(Value::Undefined))
                }
            }
            Stmt::While { condition, body } => self.eval_while(condition, body),
            Stmt::For {
                init,
                condition,
                update,
                body,
            } => self.eval_for(init.as_deref(), condition.as_ref(), update.as_ref(), body),
            Stmt::ForIn {
                target,
                object,
                body,
            } => self.eval_for_in(target, object, body),
            Stmt::Switch {
                discriminant,
                cases,
            } => self.eval_switch(discriminant, cases),
            Stmt::Try {
                body,
                catch,
                finally_body,
            } => self.eval_try(body, catch.as_ref(), finally_body.as_deref()),
            Stmt::Break => Ok(Completion::Break),
            Stmt::Continue => Ok(Completion::Continue),
            Stmt::Throw(expr) => {
                let value = self.eval_expr(expr)?;
                Ok(Completion::Throw(value))
            }
            Stmt::Return(expr) => {
                let value = self.eval_optional_init(expr.as_ref())?;
                Ok(Completion::Return(value))
            }
            Stmt::VarDecl { name, kind, init } => self.eval_declaration(name, *kind, init.as_ref()),
            Stmt::Expr(expr) => self.eval_expr(expr).map(Completion::Normal),
        }
    }

    pub(crate) fn eval_expr(&mut self, expr: &Expr) -> Result<Value> {
        self.step()?;
        match expr {
            Expr::Literal(value) => self.literal_value(value),
            Expr::This => self.current_this(),
            Expr::Identifier(name) => self.eval_identifier(name),
            Expr::Parenthesized(expr) => self.eval_expr(expr),
            Expr::Unary { op, expr } => self.eval_unary_expr(*op, expr),
            Expr::Update { op, prefix, expr } => self.eval_update_expr(*op, *prefix, expr),
            Expr::Binary {
                op,
                left,
                right,
                property_access,
            } => self.eval_binary(*op, left, right, *property_access),
            Expr::Conditional {
                condition,
                consequent,
                alternate,
            } => self.eval_conditional(condition, consequent, alternate),
            Expr::Assignment { name, expr } => {
                let value = self.eval_expr(expr)?;
                self.assign_static_or_builtin(name, value.clone())?;
                Ok(value)
            }
            Expr::CompoundAssignment { op, target, expr } => {
                self.eval_compound_assignment(*op, target, expr)
            }
            Expr::PropertyAssignment {
                object,
                property,
                access,
                expr,
            } => self.eval_property_assignment(object, property, *access, expr),
            Expr::ComputedPropertyAssignment {
                object,
                property,
                access,
                expr,
            } => self.eval_computed_property_assignment(object, property, *access, expr),
            Expr::Member {
                object,
                property,
                access,
            } => self.eval_member(object, property, *access),
            Expr::ComputedMember {
                object,
                property,
                access,
            } => self.eval_computed_member(object, property, *access),
            Expr::Call { callee, args } => self.eval_call(callee, args),
            Expr::Function {
                id,
                name,
                params,
                body,
            } => self.create_function(*id, name.as_ref(), params, body),
            Expr::MethodFunction {
                id,
                name,
                params,
                body,
            } => self.create_method_function(*id, name, params, body),
            Expr::Object(properties) => self.eval_object_literal(properties),
            Expr::Array(elements) => self.eval_array_literal(elements),
            Expr::New { constructor, args } => self.eval_new(constructor, args),
        }
    }

    fn eval_object_literal(&mut self, properties: &[ObjectProperty]) -> Result<Value> {
        let mut values = Vec::with_capacity(properties.len());
        for property in properties {
            let value = self.eval_expr(&property.value)?;
            let key = self.intern_static_property_key(&property.key)?;
            values.push(ObjectPropertyInit::new(
                key,
                property.key.as_str(),
                value,
                PropertyEnumerable::Yes,
            ));
        }
        let constructor_key = self.intern_property_key(OBJECT_CONSTRUCTOR_PROPERTY)?;
        self.objects.create(
            values,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn eval_array_literal(&mut self, elements: &[Expr]) -> Result<Value> {
        let mut values = Vec::with_capacity(elements.len());
        for element in elements {
            values.push(self.eval_expr(element)?);
        }
        self.create_array_from_elements(values)
    }

    fn eval_conditional(
        &mut self,
        condition: &Expr,
        consequent: &Expr,
        alternate: &Expr,
    ) -> Result<Value> {
        let condition = self.eval_expr(condition)?;
        if condition.is_truthy() {
            return self.eval_expr(consequent);
        }
        self.eval_expr(alternate)
    }

    pub(crate) fn eval_block(&mut self, statements: &[Stmt]) -> Result<Completion> {
        let mut last = Value::Undefined;
        for statement in statements {
            self.step()?;
            let completion = match self.eval_statement(statement) {
                Ok(completion) => completion,
                Err(error) => {
                    if let Some(value) = runtime_exception_value(&error) {
                        self.checked_value(value.clone())?;
                        return Ok(Completion::Throw(value));
                    }
                    return Err(error);
                }
            };
            match completion {
                Completion::Normal(value) => last = value,
                completion => return Ok(completion),
            }
        }
        Ok(Completion::Normal(last))
    }

    fn eval_binary(
        &mut self,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
        property_access: Option<crate::ast::StaticPropertyAccessId>,
    ) -> Result<Value> {
        if op == BinaryOp::LogicalAnd {
            let left = self.eval_expr(left)?;
            return if left.is_truthy() {
                self.eval_expr(right)
            } else {
                Ok(left)
            };
        }

        if op == BinaryOp::LogicalOr {
            let left = self.eval_expr(left)?;
            return if left.is_truthy() {
                Ok(left)
            } else {
                self.eval_expr(right)
            };
        }

        let left = self.eval_expr(left)?;
        let right = self.eval_expr(right)?;

        let value = match op {
            BinaryOp::Add => self.add(&left, &right)?,
            BinaryOp::Sub => numeric_binary(&left, &right, "-", |left, right| left - right)?,
            BinaryOp::Mul => numeric_binary(&left, &right, "*", |left, right| left * right)?,
            BinaryOp::Div => numeric_binary(&left, &right, "/", |left, right| left / right)?,
            BinaryOp::Rem => numeric_binary(&left, &right, "%", |left, right| left % right)?,
            BinaryOp::Pow => numeric_binary(&left, &right, "**", f64::powf)?,
            BinaryOp::Equal | BinaryOp::StrictEqual => Value::Bool(left == right),
            BinaryOp::NotEqual | BinaryOp::StrictNotEqual => Value::Bool(left != right),
            BinaryOp::Less => compare_binary(&left, &right, "<", |left, right| left < right)?,
            BinaryOp::LessEqual => {
                compare_binary(&left, &right, "<=", |left, right| left <= right)?
            }
            BinaryOp::Greater => compare_binary(&left, &right, ">", |left, right| left > right)?,
            BinaryOp::GreaterEqual => {
                compare_binary(&left, &right, ">=", |left, right| left >= right)?
            }
            BinaryOp::In => self.eval_in(&left, &right, property_access)?,
            BinaryOp::BitAnd => bitwise_and(&left, &right)?,
            BinaryOp::BitOr => bitwise_or(&left, &right)?,
            BinaryOp::BitXor => bitwise_xor(&left, &right)?,
            BinaryOp::ShiftLeft => shift_left(&left, &right)?,
            BinaryOp::ShiftRight => shift_right(&left, &right)?,
            BinaryOp::ShiftRightUnsigned => shift_right_unsigned(&left, &right)?,
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
                return Err(Error::runtime("logical operator reached eager evaluation"));
            }
        };
        self.checked_value(value)
    }

    fn eval_in(
        &self,
        left: &Value,
        right: &Value,
        property_access: Option<crate::ast::StaticPropertyAccessId>,
    ) -> Result<Value> {
        let property = self.dynamic_property_key(left)?;
        if let Some(access) = property_access {
            return self
                .has_cached_dynamic_property_value(right, &property, access)
                .map(Value::Bool);
        }
        self.has_dynamic_property_value(right, &property)
            .map(Value::Bool)
    }

    fn eval_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<Value> {
        if is_assert_throws_call(callee) {
            return self.eval_assert_throws(args);
        }

        if let Expr::Identifier(name) = callee
            && name.as_str() == HOST_PRINT_NAME
        {
            return self.eval_print_call(args);
        }

        if let Expr::Identifier(name) = callee {
            let reference = self.eval_identifier_call_reference(name)?;
            return self.eval_call_reference_result(reference, args);
        }

        if let Some(reference) = self.eval_call_reference(callee)? {
            return self.eval_call_reference_result(reference, args);
        }

        match self.eval_expr(callee)? {
            Value::Function(id) => self.eval_function(id, args),
            Value::NativeFunction(id) => self.eval_native_function(id, args, &Value::Undefined),
            Value::HostFunction(id) => self.eval_host_function(id, args),
            value => Err(Error::runtime(format!("'{value}' is not callable"))),
        }
    }

    fn eval_call_reference_result(
        &mut self,
        reference: CallReference,
        args: &[Expr],
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

    fn eval_identifier_call_reference(&mut self, callee: &StaticBinding) -> Result<CallReference> {
        let Some(binding) = self.get_or_materialize_binding_static(callee)? else {
            return Err(reference_error_undefined(callee));
        };
        let function = binding.value();
        if let Value::NativeFunction(id) = function {
            let kind =
                if let Some(kind) = self.cached_static_binding_native_call_kind(callee, id)? {
                    kind
                } else {
                    let kind = self.native_function(id)?.kind();
                    self.remember_static_binding_native_call_kind(callee, id, kind)?;
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

    fn eval_call_reference(&mut self, callee: &Expr) -> Result<Option<CallReference>> {
        match callee {
            Expr::Member {
                object,
                property,
                access,
            } => {
                let this_value = self.eval_expr(object)?;
                let function = self.get_static_property_value(&this_value, property, *access)?;
                if let Value::NativeFunction(id) = function {
                    let kind =
                        if let Some(kind) = self.cached_static_native_call_kind(*access, id)? {
                            kind
                        } else {
                            let kind = self.native_function(id)?.kind();
                            self.remember_static_native_call_kind(*access, id, kind)?;
                            kind
                        };
                    return Ok(Some(CallReference::Native { kind, this_value }));
                }
                Ok(Some(CallReference::Generic {
                    callee: function,
                    this_value,
                }))
            }
            Expr::ComputedMember {
                object,
                property,
                access,
            } => {
                let this_value = self.eval_expr(object)?;
                let property = self.eval_property_key(property)?;
                let function =
                    self.get_cached_dynamic_property_value(&this_value, &property, *access)?;
                if let Value::NativeFunction(id) = function {
                    let kind =
                        if let Some(kind) = self.cached_static_native_call_kind(*access, id)? {
                            kind
                        } else {
                            let kind = self.native_function(id)?.kind();
                            self.remember_static_native_call_kind(*access, id, kind)?;
                            kind
                        };
                    return Ok(Some(CallReference::Native { kind, this_value }));
                }
                Ok(Some(CallReference::Generic {
                    callee: function,
                    this_value,
                }))
            }
            Expr::Parenthesized(expr) => self.eval_call_reference(expr),
            _ => Ok(None),
        }
    }

    fn eval_assert_throws(&mut self, args: &[Expr]) -> Result<Value> {
        let mut args = args.iter();
        let Some(expected) = args.next() else {
            return Err(Error::runtime("assert.throws requires an expected error"));
        };
        let Some(callback) = args.next() else {
            return Err(Error::runtime("assert.throws requires a callback"));
        };
        let message = args.next();
        if args.next().is_some() {
            return Err(Error::runtime(
                "assert.throws supports at most three arguments",
            ));
        }
        let expected_name = expected_error_name(expected)?;
        let callback = self.eval_expr(callback)?;
        if let Some(message) = message {
            self.eval_expr(message)?;
        }
        let Value::Function(id) = callback else {
            return Err(Error::runtime("assert.throws callback must be a function"));
        };

        match self.eval_function_completion(id, &[])? {
            Completion::Throw(value) if thrown_value_matches(&value, expected_name) => {
                Ok(Value::Undefined)
            }
            Completion::Throw(value) => Err(Error::runtime(format!(
                "assert.throws expected {expected_name}, got {value}"
            ))),
            Completion::Normal(_) | Completion::Return(_) => Err(Error::runtime(format!(
                "assert.throws expected {expected_name}, but no exception was thrown"
            ))),
            completion @ (Completion::Break | Completion::Continue) => {
                completion.into_function_result()
            }
        }
    }

    fn eval_member(
        &mut self,
        object: &Expr,
        property: &StaticName,
        access: crate::ast::StaticPropertyAccessId,
    ) -> Result<Value> {
        let object = self.eval_expr(object)?;
        self.get_static_property_value(&object, property, access)
    }

    fn eval_computed_member(
        &mut self,
        object: &Expr,
        property: &Expr,
        access: crate::ast::StaticPropertyAccessId,
    ) -> Result<Value> {
        let object = self.eval_expr(object)?;
        let property = self.eval_property_key(property)?;
        self.get_cached_dynamic_property_value(&object, &property, access)
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

    fn eval_print_call(&mut self, args: &[Expr]) -> Result<Value> {
        let values = args
            .iter()
            .map(|arg| self.eval_expr(arg))
            .collect::<Result<Vec<_>>>()?;
        let line = values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        self.check_string_len(&line)?;
        self.output.push(line);
        Ok(Value::Undefined)
    }

    fn eval_new(&mut self, constructor: &StaticBinding, args: &[Expr]) -> Result<Value> {
        if constructor.as_str() != TEST262_ERROR_NAME {
            return self.eval_function_constructor(constructor, args);
        }
        self.eval_error_constructor(ErrorName::Test262Error, args)
    }

    fn eval_function_constructor(
        &mut self,
        constructor: &StaticBinding,
        args: &[Expr],
    ) -> Result<Value> {
        let value = self
            .constructor_binding_static(constructor)?
            .ok_or_else(|| reference_error_undefined(constructor))?;
        let Value::Function(id) = value else {
            if let Value::NativeFunction(id) = value {
                return self.construct_native_function(id, args);
            }
            return Err(Error::runtime(format!(
                "'{constructor}' is not a constructor"
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

    fn eval_identifier(&mut self, name: &StaticBinding) -> Result<Value> {
        if let Some(binding) = self.get_binding_static(name)? {
            return self.runtime_value(binding.value());
        }
        self.builtin_value(name.name())?
            .ok_or_else(|| reference_error_undefined(name))
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
