use std::rc::Rc;

use crate::ast::{BinaryOp, Expr, ObjectProperty, Program, Stmt};
use crate::atom::{AtomId, AtomTable};
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
use crate::runtime_object::ObjectHeap;
use crate::runtime_property::{
    delete_property, enumerable_property_keys, get_property, has_property, property_key,
    set_property,
};
use crate::runtime_scope::BindingScope;
use crate::value::{ErrorName, Value};

#[path = "runtime_declaration.rs"]
mod runtime_declaration;
#[path = "runtime_function.rs"]
mod runtime_function;
#[path = "runtime_native.rs"]
mod runtime_native;

const BOOLEAN_NAME: &str = "Boolean";
const HOST_PRINT_NAME: &str = "print";
const TEST262_ERROR_NAME: &str = "Test262Error";

#[derive(Debug, Clone)]
pub struct Context {
    limits: RuntimeLimits,
    atoms: AtomTable,
    globals: BindingScope,
    locals: Vec<BindingScope>,
    functions: Vec<Function>,
    native_functions: Vec<runtime_native::NativeFunction>,
    pub(crate) host_functions: Vec<HostFunction>,
    objects: ObjectHeap,
    this_values: Vec<Value>,
    output: Vec<String>,
    runtime_steps: usize,
}

#[derive(Debug, Clone)]
struct Function {
    name: String,
    params: Rc<[String]>,
    body: Rc<[Stmt]>,
    captures: Vec<BindingScope>,
    properties: runtime_function::FunctionProperties,
    constructable: bool,
}

impl Context {
    #[must_use]
    pub const fn new(limits: RuntimeLimits) -> Self {
        Self {
            limits,
            atoms: AtomTable::new(),
            globals: BindingScope::new(),
            locals: Vec::new(),
            functions: Vec::new(),
            native_functions: Vec::new(),
            host_functions: Vec::new(),
            objects: ObjectHeap::new(),
            this_values: Vec::new(),
            output: Vec::new(),
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
        self.eval_program(script.program())
    }

    #[must_use]
    pub fn output(&self) -> &[String] {
        &self.output
    }

    #[must_use]
    pub fn take_output(&mut self) -> Vec<String> {
        std::mem::take(&mut self.output)
    }

    #[must_use]
    pub fn get_global(&self, name: &str) -> Option<Value> {
        let atom = self.atom(name)?;
        self.globals.get(atom).map(|binding| binding.value())
    }

    #[must_use]
    pub const fn runtime_steps(&self) -> usize {
        self.runtime_steps
    }

    #[must_use]
    pub const fn atom_count(&self) -> usize {
        self.atoms.len()
    }

    pub(crate) const fn global_binding_count(&self) -> usize {
        self.globals.len()
    }

    pub(crate) fn intern_atom(&mut self, name: &str) -> Result<AtomId> {
        self.check_string_len(name)?;
        self.atoms.intern(name)
    }

    pub(crate) fn atom(&self, name: &str) -> Option<AtomId> {
        self.atoms.get(name)
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
            Expr::Literal(value) => self.checked_value(value.clone()),
            Expr::This => self.current_this(),
            Expr::Identifier(name) => self.eval_identifier(name),
            Expr::Parenthesized(expr) => self.eval_expr(expr),
            Expr::Unary { op, expr } => self.eval_unary_expr(*op, expr),
            Expr::Update { op, prefix, expr } => self.eval_update_expr(*op, *prefix, expr),
            Expr::Binary { op, left, right } => self.eval_binary(*op, left, right),
            Expr::Conditional {
                condition,
                consequent,
                alternate,
            } => self.eval_conditional(condition, consequent, alternate),
            Expr::Assignment { name, expr } => {
                let value = self.eval_expr(expr)?;
                self.assign(name, value.clone())?;
                Ok(value)
            }
            Expr::CompoundAssignment { op, target, expr } => {
                self.eval_compound_assignment(*op, target, expr)
            }
            Expr::PropertyAssignment {
                object,
                property,
                expr,
            } => self.eval_property_assignment(object, property, expr),
            Expr::ComputedPropertyAssignment {
                object,
                property,
                expr,
            } => self.eval_computed_property_assignment(object, property, expr),
            Expr::Member { object, property } => self.eval_member(object, property),
            Expr::ComputedMember { object, property } => {
                self.eval_computed_member(object, property)
            }
            Expr::Call { callee, args } => self.eval_call(callee, args),
            Expr::Function { name, params, body } => {
                self.create_function(name.as_deref(), params, body)
            }
            Expr::MethodFunction { name, params, body } => {
                self.create_method_function(name, params, body)
            }
            Expr::Object(properties) => self.eval_object_literal(properties),
            Expr::Array(elements) => self.eval_array_literal(elements),
            Expr::New { constructor, args } => self.eval_new(constructor, args),
        }
    }

    fn eval_object_literal(&mut self, properties: &[ObjectProperty]) -> Result<Value> {
        let mut values = Vec::with_capacity(properties.len());
        for property in properties {
            let value = self.eval_expr(&property.value)?;
            values.push((property.key.clone(), value));
        }
        self.objects.create(
            values,
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

    fn eval_binary(&mut self, op: BinaryOp, left: &Expr, right: &Expr) -> Result<Value> {
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
            BinaryOp::In => self.eval_in(&left, &right)?,
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

    fn eval_in(&self, left: &Value, right: &Value) -> Result<Value> {
        let property = property_key(left);
        self.check_string_len(&property)?;
        self.has_property_value(right, &property).map(Value::Bool)
    }

    fn eval_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<Value> {
        if is_assert_throws_call(callee) {
            return self.eval_assert_throws(args);
        }

        if let Expr::Identifier(name) = callee {
            match name.as_str() {
                BOOLEAN_NAME => return self.eval_boolean_call(args),
                HOST_PRINT_NAME => return self.eval_print_call(args),
                _ => {}
            }
        }

        if let Some((callee, this_value)) = self.eval_call_reference(callee)? {
            return match callee {
                Value::Function(id) => self.eval_function_with_this(id, args, this_value),
                Value::NativeFunction(id) => self.eval_native_function(id, args, &this_value),
                Value::HostFunction(id) => self.eval_host_function(id, args),
                value => Err(Error::runtime(format!("'{value}' is not callable"))),
            };
        }

        match self.eval_expr(callee)? {
            Value::Function(id) => self.eval_function(id, args),
            Value::NativeFunction(id) => self.eval_native_function(id, args, &Value::Undefined),
            Value::HostFunction(id) => self.eval_host_function(id, args),
            value => Err(Error::runtime(format!("'{value}' is not callable"))),
        }
    }

    fn eval_call_reference(&mut self, callee: &Expr) -> Result<Option<(Value, Value)>> {
        match callee {
            Expr::Member { object, property } => {
                let this_value = self.eval_expr(object)?;
                let function = self.get_property_value(&this_value, property)?;
                Ok(Some((function, this_value)))
            }
            Expr::ComputedMember { object, property } => {
                let this_value = self.eval_expr(object)?;
                let property = self.eval_property_key(property)?;
                let function = self.get_property_value(&this_value, &property)?;
                Ok(Some((function, this_value)))
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

    fn eval_member(&mut self, object: &Expr, property: &str) -> Result<Value> {
        let object = self.eval_expr(object)?;
        self.get_property_value(&object, property)
    }

    fn eval_computed_member(&mut self, object: &Expr, property: &Expr) -> Result<Value> {
        let object = self.eval_expr(object)?;
        let property = self.eval_property_key(property)?;
        self.get_property_value(&object, &property)
    }

    pub(crate) fn eval_property_key(&mut self, property: &Expr) -> Result<String> {
        let value = self.eval_expr(property)?;
        let key = property_key(&value);
        self.check_string_len(&key)?;
        Ok(key)
    }

    pub(crate) fn get_property_value(&self, object: &Value, property: &str) -> Result<Value> {
        if let Value::Function(id) = object {
            return self.get_function_property(*id, property);
        }
        if let Value::NativeFunction(id) = object {
            return self.get_native_function_property(*id, property);
        }
        self.checked_value(get_property(&self.objects, object, property)?)
    }

    pub(crate) fn set_property_value(
        &mut self,
        object: &Value,
        property: String,
        value: Value,
    ) -> Result<()> {
        self.checked_value(value.clone())?;
        if let Value::Function(id) = object {
            return self.set_function_property(*id, property, value);
        }
        if let Value::NativeFunction(id) = object {
            return self.set_native_function_property(*id, property, value);
        }
        set_property(
            &mut self.objects,
            object,
            property,
            value,
            self.limits.max_object_properties,
        )
    }

    pub(crate) fn delete_property_value(
        &mut self,
        object: &Value,
        property: &str,
    ) -> Result<Value> {
        if let Value::Function(id) = object {
            return self
                .delete_function_property(*id, property)
                .map(Value::Bool);
        }
        if let Value::NativeFunction(id) = object {
            return self
                .delete_native_function_property(*id, property)
                .map(Value::Bool);
        }
        delete_property(&mut self.objects, object, property).map(Value::Bool)
    }

    fn has_property_value(&self, object: &Value, property: &str) -> Result<bool> {
        match object {
            Value::Function(id) => self.has_function_property(*id, property),
            Value::NativeFunction(id) => self.has_native_function_property(*id, property),
            _ => has_property(&self.objects, object, property),
        }
    }

    pub(crate) fn enumerable_keys(&self, object: &Value) -> Result<Vec<String>> {
        if let Value::Function(id) = object {
            return self.function_enumerable_keys(*id);
        }
        if let Value::NativeFunction(id) = object {
            return self.native_function_enumerable_keys(*id);
        }
        enumerable_property_keys(&self.objects, object)
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

    fn eval_boolean_call(&mut self, args: &[Expr]) -> Result<Value> {
        let Some(arg) = args.first() else {
            return Ok(Value::Bool(false));
        };
        let value = self.eval_expr(arg)?;
        Ok(Value::Bool(value.is_truthy()))
    }

    fn eval_new(&mut self, constructor: &str, args: &[Expr]) -> Result<Value> {
        if constructor != TEST262_ERROR_NAME {
            return self.eval_function_constructor(constructor, args);
        }
        self.eval_error_constructor(ErrorName::Test262Error, args)
    }

    fn eval_function_constructor(&mut self, constructor: &str, args: &[Expr]) -> Result<Value> {
        let value = self
            .constructor_binding(constructor)?
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
        let object = self.objects.create_with_prototype(
            prototype,
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

    fn eval_identifier(&mut self, name: &str) -> Result<Value> {
        if let Some(binding) = self.get_binding(name) {
            return self.checked_value(binding.value());
        }
        self.builtin_value(name)?
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

    pub(crate) fn add(&self, left: &Value, right: &Value) -> Result<Value> {
        match (left, right) {
            (Value::Number(left), Value::Number(right)) => Ok(Value::Number(left + right)),
            (Value::String(_), _) | (_, Value::String(_)) => {
                let value = left.display_for_concat() + &right.display_for_concat();
                self.check_string_len(&value)?;
                Ok(Value::String(value))
            }
            _ => Err(Error::runtime("operator '+' expects numbers or strings")),
        }
    }

    pub(crate) fn checked_value(&self, value: Value) -> Result<Value> {
        match &value {
            Value::String(text) => self.check_string_len(text)?,
            Value::Error(error) => self.check_string_len(error.message())?,
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Object(_) => {}
        }
        Ok(value)
    }

    pub(crate) fn current_this(&self) -> Result<Value> {
        self.checked_value(self.this_values.last().cloned().unwrap_or(Value::Undefined))
    }

    pub(crate) fn check_string_len(&self, text: &str) -> Result<()> {
        if text.len() > self.limits.max_string_len {
            return Err(Error::limit(format!(
                "string length {} exceeded {}",
                text.len(),
                self.limits.max_string_len
            )));
        }
        Ok(())
    }

    pub(crate) fn step(&mut self) -> Result<()> {
        self.runtime_steps = self
            .runtime_steps
            .checked_add(1)
            .ok_or_else(|| Error::limit("runtime steps overflowed"))?;
        if self.runtime_steps > self.limits.max_runtime_steps {
            return Err(Error::limit(format!(
                "runtime steps exceeded {}",
                self.limits.max_runtime_steps
            )));
        }
        Ok(())
    }
}
