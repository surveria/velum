use crate::ast::{BinaryOp, DeclKind, Expr, ObjectProperty, Program, Stmt, UnaryOp};
use crate::error::{Error, Result};
use crate::lexer;
use crate::parser;
use crate::runtime_assertions::{
    expected_error_name, is_assert_throws_call, reference_error_undefined, runtime_exception_value,
    thrown_value_matches,
};
use crate::runtime_completion::Completion;
use crate::runtime_limits::RuntimeLimits;
use crate::runtime_numeric::{bitwise_and, compare_binary, numeric_binary};
use crate::runtime_object::ObjectHeap;
use crate::runtime_property::{get_property, property_key, set_property};
use crate::runtime_scope::{BindingCell, BindingScope};
use crate::value::{ErrorName, ErrorObject, FunctionId, Value};

const BOOLEAN_NAME: &str = "Boolean";
const HOST_PRINT_NAME: &str = "print";
const TEST262_ERROR_NAME: &str = "Test262Error";

#[derive(Debug, Clone)]
pub struct Context {
    limits: RuntimeLimits,
    globals: BindingScope,
    locals: Vec<BindingScope>,
    functions: Vec<Function>,
    objects: ObjectHeap,
    output: Vec<String>,
    runtime_steps: usize,
}

#[derive(Debug, Clone)]
struct Function {
    params: Vec<String>,
    body: Vec<Stmt>,
    captures: Vec<BindingScope>,
}

impl Context {
    #[must_use]
    pub const fn new(limits: RuntimeLimits) -> Self {
        Self {
            limits,
            globals: BindingScope::new(),
            locals: Vec::new(),
            functions: Vec::new(),
            objects: ObjectHeap::new(),
            output: Vec::new(),
            runtime_steps: 0,
        }
    }

    /// # Errors
    /// Fails when lexing, parsing, evaluation, or configured resource limits fail.
    pub fn eval(&mut self, source: &str) -> Result<Value> {
        self.check_source(source)?;
        let tokens = lexer::lex(source)?;
        let program = parser::parse(tokens, self.limits)?;
        self.eval_program(&program)
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
        self.globals.get(name).map(|binding| binding.value())
    }

    #[must_use]
    pub const fn runtime_steps(&self) -> usize {
        self.runtime_steps
    }

    fn check_source(&self, source: &str) -> Result<()> {
        if source.len() > self.limits.max_source_len {
            return Err(Error::limit(format!(
                "source length {} exceeded {}",
                source.len(),
                self.limits.max_source_len
            )));
        }
        Ok(())
    }

    fn eval_program(&mut self, program: &Program) -> Result<Value> {
        self.hoist_var_declarations(&program.statements)?;
        self.eval_block(&program.statements)?.into_result()
    }

    pub(crate) fn eval_statement(&mut self, statement: &Stmt) -> Result<Completion> {
        match statement {
            Stmt::Block(statements) => self.eval_block(statements),
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
            Stmt::Switch {
                discriminant,
                cases,
            } => self.eval_switch(discriminant, cases),
            Stmt::TryCatch {
                body,
                catch_param,
                catch_body,
            } => self.eval_try_catch(body, catch_param, catch_body),
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

    pub(crate) fn hoist_var_declarations(&mut self, statements: &[Stmt]) -> Result<()> {
        for statement in statements {
            self.hoist_statement_vars(statement)?;
        }
        Ok(())
    }

    fn hoist_statement_vars(&mut self, statement: &Stmt) -> Result<()> {
        match statement {
            Stmt::Block(statements) => self.hoist_var_declarations(statements),
            Stmt::If {
                consequent,
                alternate,
                ..
            } => {
                self.hoist_statement_vars(consequent)?;
                if let Some(alternate) = alternate {
                    self.hoist_statement_vars(alternate)?;
                }
                Ok(())
            }
            Stmt::While { body, .. } => self.hoist_statement_vars(body),
            Stmt::For { init, body, .. } => {
                if let Some(init) = init {
                    self.hoist_statement_vars(init)?;
                }
                self.hoist_statement_vars(body)
            }
            Stmt::Switch { cases, .. } => self.hoist_switch_vars(cases),
            Stmt::TryCatch {
                body, catch_body, ..
            } => {
                self.hoist_var_declarations(body)?;
                self.hoist_var_declarations(catch_body)
            }
            Stmt::VarDecl {
                name,
                kind: DeclKind::Var,
                ..
            } => self.hoist_var(name),
            Stmt::Break
            | Stmt::Continue
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::VarDecl { .. }
            | Stmt::Expr(_) => Ok(()),
        }
    }

    fn hoist_var(&mut self, name: &str) -> Result<()> {
        if let Some(binding) = self.active_bindings().get(name) {
            if binding.kind() == DeclKind::Var {
                return Ok(());
            }
            return Err(Error::runtime(format!(
                "'{name}' has already been declared"
            )));
        }

        self.ensure_binding_capacity(name)?;
        self.active_bindings_mut().insert(
            name.to_owned(),
            BindingCell::new(Value::Undefined, true, DeclKind::Var),
        );
        Ok(())
    }

    fn eval_declaration(
        &mut self,
        name: &str,
        kind: DeclKind,
        init: Option<&Expr>,
    ) -> Result<Completion> {
        match kind {
            DeclKind::Var => {
                if let Some(init) = init {
                    let value = self.eval_expr(init)?;
                    self.assign(name, value)?;
                }
            }
            DeclKind::Let => {
                let value = self.eval_optional_init(init)?;
                self.define(name, value, DeclKind::Let)?;
            }
            DeclKind::Const => {
                let Some(init) = init else {
                    return Err(Error::runtime("const declaration requires an initializer"));
                };
                let value = self.eval_expr(init)?;
                self.define(name, value, DeclKind::Const)?;
            }
        }
        Ok(Completion::Normal(Value::Undefined))
    }

    fn eval_optional_init(&mut self, init: Option<&Expr>) -> Result<Value> {
        if let Some(init) = init {
            return self.eval_expr(init);
        }
        Ok(Value::Undefined)
    }

    pub(crate) fn eval_expr(&mut self, expr: &Expr) -> Result<Value> {
        self.step()?;
        match expr {
            Expr::Literal(value) => self.checked_value(value.clone()),
            Expr::Identifier(name) => self
                .get_binding(name)
                .map(|binding| binding.value())
                .ok_or_else(|| reference_error_undefined(name)),
            Expr::Unary { op, expr } => {
                let value = self.eval_expr(expr)?;
                Self::eval_unary(*op, &value)
            }
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
            Expr::Function { params, body } => Ok(self.create_function(params, body)),
            Expr::Object(properties) => self.eval_object_literal(properties),
            Expr::Array(elements) => self.eval_array_literal(elements),
            Expr::New { constructor, args } => self.eval_new(constructor, args),
        }
    }

    fn eval_object_literal(&mut self, properties: &[ObjectProperty]) -> Result<Value> {
        let mut values = Vec::new();
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
        let mut values = Vec::new();
        for element in elements {
            values.push(self.eval_expr(element)?);
        }
        self.objects.create_array(
            values,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
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

    fn eval_try_catch(
        &mut self,
        body: &[Stmt],
        catch_param: &str,
        catch_body: &[Stmt],
    ) -> Result<Completion> {
        match self.eval_block(body)? {
            Completion::Normal(value) => Ok(Completion::Normal(value)),
            Completion::Throw(value) => self.eval_catch(catch_param, value, catch_body),
            completion => Ok(completion),
        }
    }

    fn eval_catch(
        &mut self,
        catch_param: &str,
        value: Value,
        catch_body: &[Stmt],
    ) -> Result<Completion> {
        let previous = self.active_bindings_mut().remove(catch_param);
        if previous.is_none() {
            self.ensure_binding_capacity(catch_param)?;
        }
        self.checked_value(value.clone())?;
        self.active_bindings_mut().insert(
            catch_param.to_owned(),
            BindingCell::new(value, true, DeclKind::Let),
        );
        let result = self.eval_block(catch_body);
        let removed = self.active_bindings_mut().remove(catch_param);
        if removed.is_none() {
            return Err(Error::runtime("catch binding disappeared"));
        }
        if let Some(previous) = previous {
            self.active_bindings_mut()
                .insert(catch_param.to_owned(), previous);
        }
        result
    }

    fn eval_unary(op: UnaryOp, value: &Value) -> Result<Value> {
        match op {
            UnaryOp::Not => Ok(Value::Bool(!value.is_truthy())),
            UnaryOp::Negate => value
                .as_number()
                .map(|value| Value::Number(-value))
                .ok_or_else(|| Error::runtime("unary '-' expects a number")),
            UnaryOp::Plus => value
                .as_number()
                .map(Value::Number)
                .ok_or_else(|| Error::runtime("unary '+' expects a number")),
        }
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
            BinaryOp::BitAnd => bitwise_and(&left, &right)?,
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
                return Err(Error::runtime("logical operator reached eager evaluation"));
            }
        };
        self.checked_value(value)
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

        match self.eval_expr(callee)? {
            Value::Function(id) => self.eval_function(id, args),
            value => Err(Error::runtime(format!("'{value}' is not callable"))),
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
        if args.next().is_some() {
            return Err(Error::runtime(
                "assert.throws supports exactly two arguments",
            ));
        }

        let expected_name = expected_error_name(expected)?;
        let callback = self.eval_expr(callback)?;
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
        self.checked_value(get_property(&self.objects, &object, property)?)
    }

    fn eval_computed_member(&mut self, object: &Expr, property: &Expr) -> Result<Value> {
        let object = self.eval_expr(object)?;
        let property = self.eval_property_key(property)?;
        self.checked_value(get_property(&self.objects, &object, &property)?)
    }

    fn eval_property_assignment(
        &mut self,
        object: &Expr,
        property: &str,
        expr: &Expr,
    ) -> Result<Value> {
        let object = self.eval_expr(object)?;
        let value = self.eval_expr(expr)?;
        self.checked_value(value.clone())?;
        set_property(
            &mut self.objects,
            &object,
            property.to_owned(),
            value.clone(),
            self.limits.max_object_properties,
        )?;
        Ok(value)
    }

    fn eval_computed_property_assignment(
        &mut self,
        object: &Expr,
        property: &Expr,
        expr: &Expr,
    ) -> Result<Value> {
        let object = self.eval_expr(object)?;
        let property = self.eval_property_key(property)?;
        let value = self.eval_expr(expr)?;
        self.checked_value(value.clone())?;
        set_property(
            &mut self.objects,
            &object,
            property,
            value.clone(),
            self.limits.max_object_properties,
        )?;
        Ok(value)
    }

    fn eval_property_key(&mut self, property: &Expr) -> Result<String> {
        let value = self.eval_expr(property)?;
        let key = property_key(&value);
        self.check_string_len(&key)?;
        Ok(key)
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
            return Err(Error::runtime(format!(
                "constructor '{constructor}' is not supported"
            )));
        }
        let Some(message) = args.first() else {
            return Ok(Value::Error(ErrorObject::new(ErrorName::Test262Error, "")));
        };
        let message = self.eval_expr(message)?;
        Ok(Value::Error(ErrorObject::new(
            ErrorName::Test262Error,
            message.display_for_concat(),
        )))
    }

    fn create_function(&mut self, params: &[String], body: &[Stmt]) -> Value {
        let id = FunctionId::new(self.functions.len());
        self.functions.push(Function {
            params: params.to_vec(),
            body: body.to_vec(),
            captures: self.locals.clone(),
        });
        Value::Function(id)
    }

    fn eval_function(&mut self, id: FunctionId, args: &[Expr]) -> Result<Value> {
        let value = self
            .eval_function_completion(id, args)?
            .into_function_result()?;
        self.checked_value(value)
    }

    fn eval_function_completion(&mut self, id: FunctionId, args: &[Expr]) -> Result<Completion> {
        let function = self
            .functions
            .get(id.index())
            .cloned()
            .ok_or_else(|| Error::runtime("function id is not defined"))?;
        let args = self.eval_args(args)?;
        let caller_locals = std::mem::replace(&mut self.locals, function.captures);
        let scope = match self.function_scope(&function.params, args) {
            Ok(scope) => scope,
            Err(error) => {
                self.locals = caller_locals;
                return Err(error);
            }
        };
        self.locals.push(scope);
        let result = self
            .hoist_var_declarations(&function.body)
            .and_then(|()| self.eval_block(&function.body));
        let removed = self.locals.pop();
        self.locals = caller_locals;
        if removed.is_none() {
            return Err(Error::runtime("function scope disappeared"));
        }
        result
    }

    fn eval_args(&mut self, args: &[Expr]) -> Result<Vec<Value>> {
        args.iter().map(|arg| self.eval_expr(arg)).collect()
    }

    fn function_scope(&self, params: &[String], args: Vec<Value>) -> Result<BindingScope> {
        let mut scope = BindingScope::new();
        let mut args = args.into_iter();
        for param in params {
            if !scope.contains(param) {
                self.ensure_extra_binding_capacity(scope.len())?;
            }
            let value = args.next().unwrap_or(Value::Undefined);
            self.checked_value(value.clone())?;
            scope.insert(param.clone(), BindingCell::new(value, true, DeclKind::Var));
        }
        Ok(scope)
    }

    fn define(&mut self, name: &str, value: Value, kind: DeclKind) -> Result<()> {
        self.ensure_binding_capacity(name)?;
        if self.active_bindings().contains(name) {
            return Err(Error::runtime(format!(
                "'{name}' has already been declared"
            )));
        }

        self.checked_value(value.clone())?;
        let mutable = kind != DeclKind::Const;
        self.active_bindings_mut()
            .insert(name.to_owned(), BindingCell::new(value, mutable, kind));
        Ok(())
    }

    fn ensure_binding_capacity(&self, name: &str) -> Result<()> {
        if self.active_bindings().contains(name) {
            return Ok(());
        }
        if self.binding_count()? >= self.limits.max_bindings {
            return Err(Error::limit(format!(
                "binding count exceeded {}",
                self.limits.max_bindings
            )));
        }
        Ok(())
    }

    fn ensure_extra_binding_capacity(&self, extra_bindings: usize) -> Result<()> {
        let projected = self
            .binding_count()?
            .checked_add(extra_bindings)
            .ok_or_else(|| Error::limit("binding count overflowed"))?;
        if projected >= self.limits.max_bindings {
            return Err(Error::limit(format!(
                "binding count exceeded {}",
                self.limits.max_bindings
            )));
        }
        Ok(())
    }

    fn binding_count(&self) -> Result<usize> {
        self.locals
            .iter()
            .try_fold(self.globals.len(), |count, scope| {
                count
                    .checked_add(scope.len())
                    .ok_or_else(|| Error::limit("binding count overflowed"))
            })
    }

    fn assign(&self, name: &str, value: Value) -> Result<()> {
        self.checked_value(value.clone())?;
        let Some(binding) = self.get_binding(name) else {
            return Err(reference_error_undefined(name));
        };
        binding.assign(name, value)
    }

    fn active_bindings(&self) -> &BindingScope {
        if let Some(scope) = self.locals.last() {
            return scope;
        }
        &self.globals
    }

    fn active_bindings_mut(&mut self) -> &mut BindingScope {
        if let Some(scope) = self.locals.last_mut() {
            return scope;
        }
        &mut self.globals
    }

    fn get_binding(&self, name: &str) -> Option<BindingCell> {
        self.locals
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .or_else(|| self.globals.get(name))
    }

    fn add(&self, left: &Value, right: &Value) -> Result<Value> {
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

    fn checked_value(&self, value: Value) -> Result<Value> {
        match &value {
            Value::String(text) => self.check_string_len(text)?,
            Value::Error(error) => self.check_string_len(error.message())?,
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::Function(_)
            | Value::Object(_) => {}
        }
        Ok(value)
    }

    fn check_string_len(&self, text: &str) -> Result<()> {
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
