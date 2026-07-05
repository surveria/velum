use std::collections::BTreeMap;

use crate::ast::{BinaryOp, DeclKind, Expr, Program, Stmt, UnaryOp};
use crate::error::{Error, Result};
use crate::lexer;
use crate::parser;
use crate::runtime_assertions::{
    error_property, expected_error_name, is_assert_throws_call, reference_error_undefined,
    runtime_exception_value, thrown_value_matches,
};
use crate::runtime_completion::Completion;
use crate::runtime_numeric::{bitwise_and, compare_binary, numeric_binary};
use crate::value::{ErrorName, ErrorObject, FunctionId, Value};

const DEFAULT_MAX_SOURCE_LEN: usize = 65_536;
const DEFAULT_MAX_STATEMENTS: usize = 4_096;
const DEFAULT_MAX_EXPRESSION_DEPTH: usize = 256;
const DEFAULT_MAX_RUNTIME_STEPS: usize = 100_000;
const DEFAULT_MAX_STRING_LEN: usize = 65_536;
const DEFAULT_MAX_BINDINGS: usize = 4_096;
const BOOLEAN_NAME: &str = "Boolean";
const HOST_PRINT_NAME: &str = "print";
const TEST262_ERROR_NAME: &str = "Test262Error";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RuntimeLimits {
    pub max_source_len: usize,
    pub max_statements: usize,
    pub max_expression_depth: usize,
    pub max_runtime_steps: usize,
    pub max_string_len: usize,
    pub max_bindings: usize,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            max_source_len: DEFAULT_MAX_SOURCE_LEN,
            max_statements: DEFAULT_MAX_STATEMENTS,
            max_expression_depth: DEFAULT_MAX_EXPRESSION_DEPTH,
            max_runtime_steps: DEFAULT_MAX_RUNTIME_STEPS,
            max_string_len: DEFAULT_MAX_STRING_LEN,
            max_bindings: DEFAULT_MAX_BINDINGS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Runtime {
    limits: RuntimeLimits,
}

impl Runtime {
    #[must_use]
    pub fn new() -> Self {
        Self {
            limits: RuntimeLimits::default(),
        }
    }

    #[must_use]
    pub const fn with_limits(limits: RuntimeLimits) -> Self {
        Self { limits }
    }

    #[must_use]
    pub const fn limits(&self) -> RuntimeLimits {
        self.limits
    }

    #[must_use]
    pub const fn context(&self) -> Context {
        Context::new(self.limits)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct Context {
    limits: RuntimeLimits,
    globals: BTreeMap<String, Binding>,
    locals: Vec<BTreeMap<String, Binding>>,
    functions: Vec<Function>,
    output: Vec<String>,
    runtime_steps: usize,
}

#[derive(Debug, Clone)]
struct Binding {
    value: Value,
    mutable: bool,
    kind: DeclKind,
}

#[derive(Debug, Clone)]
struct Function {
    params: Vec<String>,
    body: Vec<Stmt>,
}

impl Context {
    #[must_use]
    pub const fn new(limits: RuntimeLimits) -> Self {
        Self {
            limits,
            globals: BTreeMap::new(),
            locals: Vec::new(),
            functions: Vec::new(),
            output: Vec::new(),
            runtime_steps: 0,
        }
    }

    /// Evaluates source text in this context.
    ///
    /// # Errors
    ///
    /// Returns an error when lexing, parsing, evaluation, or configured resource limits fail.
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
    pub fn get_global(&self, name: &str) -> Option<&Value> {
        self.globals.get(name).map(|binding| &binding.value)
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

    fn eval_statement(&mut self, statement: &Stmt) -> Result<Completion> {
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
            Stmt::TryCatch {
                body,
                catch_param,
                catch_body,
            } => self.eval_try_catch(body, catch_param, catch_body),
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

    fn hoist_var_declarations(&mut self, statements: &[Stmt]) -> Result<()> {
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
            Stmt::Throw(_) | Stmt::Return(_) | Stmt::VarDecl { .. } | Stmt::Expr(_) => Ok(()),
        }
    }

    fn hoist_var(&mut self, name: &str) -> Result<()> {
        if let Some(binding) = self.active_bindings().get(name) {
            if binding.kind == DeclKind::Var {
                return Ok(());
            }
            return Err(Error::runtime(format!(
                "'{name}' has already been declared"
            )));
        }

        self.ensure_binding_capacity(name)?;
        self.active_bindings_mut().insert(
            name.to_owned(),
            Binding {
                value: Value::Undefined,
                mutable: true,
                kind: DeclKind::Var,
            },
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

    fn eval_expr(&mut self, expr: &Expr) -> Result<Value> {
        self.step()?;
        match expr {
            Expr::Literal(value) => self.checked_value(value.clone()),
            Expr::Identifier(name) => self
                .get_binding(name)
                .map(|binding| binding.value.clone())
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
            Expr::Member { object, property } => self.eval_member(object, property),
            Expr::Call { callee, args } => self.eval_call(callee, args),
            Expr::Function { params, body } => Ok(self.create_function(params, body)),
            Expr::New { constructor, args } => self.eval_new(constructor, args),
        }
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

    fn eval_block(&mut self, statements: &[Stmt]) -> Result<Completion> {
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
                Completion::Throw(value) => return Ok(Completion::Throw(value)),
                Completion::Return(value) => return Ok(Completion::Return(value)),
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
            Completion::Return(value) => Ok(Completion::Return(value)),
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
            Binding {
                value,
                mutable: true,
                kind: DeclKind::Let,
            },
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
        }
    }

    fn eval_member(&mut self, object: &Expr, property: &str) -> Result<Value> {
        let object = self.eval_expr(object)?;
        match object {
            Value::Error(error) => self.checked_value(error_property(&error, property)),
            value => Err(Error::runtime(format!(
                "member access '{property}' is not supported for {}",
                value.type_name()
            ))),
        }
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
        let scope = self.function_scope(&function.params, args)?;
        self.locals.push(scope);
        let result = self
            .hoist_var_declarations(&function.body)
            .and_then(|()| self.eval_block(&function.body));
        let removed = self.locals.pop();
        if removed.is_none() {
            return Err(Error::runtime("function scope disappeared"));
        }
        result
    }

    fn eval_args(&mut self, args: &[Expr]) -> Result<Vec<Value>> {
        args.iter().map(|arg| self.eval_expr(arg)).collect()
    }

    fn function_scope(
        &self,
        params: &[String],
        args: Vec<Value>,
    ) -> Result<BTreeMap<String, Binding>> {
        let mut scope = BTreeMap::new();
        let mut args = args.into_iter();
        for param in params {
            if !scope.contains_key(param) {
                self.ensure_extra_binding_capacity(scope.len())?;
            }
            let value = args.next().unwrap_or(Value::Undefined);
            self.checked_value(value.clone())?;
            scope.insert(
                param.clone(),
                Binding {
                    value,
                    mutable: true,
                    kind: DeclKind::Var,
                },
            );
        }
        Ok(scope)
    }

    fn define(&mut self, name: &str, value: Value, kind: DeclKind) -> Result<()> {
        self.ensure_binding_capacity(name)?;
        if self.active_bindings().contains_key(name) {
            return Err(Error::runtime(format!(
                "'{name}' has already been declared"
            )));
        }

        self.checked_value(value.clone())?;
        let mutable = kind != DeclKind::Const;
        self.active_bindings_mut().insert(
            name.to_owned(),
            Binding {
                value,
                mutable,
                kind,
            },
        );
        Ok(())
    }

    fn ensure_binding_capacity(&self, name: &str) -> Result<()> {
        if self.active_bindings().contains_key(name) {
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

    fn assign(&mut self, name: &str, value: Value) -> Result<()> {
        self.checked_value(value.clone())?;
        let Some(binding) = self.get_binding_mut(name) else {
            return Err(reference_error_undefined(name));
        };

        if !binding.mutable {
            return Err(Error::runtime(format!("assignment to constant '{name}'")));
        }

        binding.value = value;
        Ok(())
    }

    fn active_bindings(&self) -> &BTreeMap<String, Binding> {
        if let Some(scope) = self.locals.last() {
            return scope;
        }
        &self.globals
    }

    fn active_bindings_mut(&mut self) -> &mut BTreeMap<String, Binding> {
        if let Some(scope) = self.locals.last_mut() {
            return scope;
        }
        &mut self.globals
    }

    fn get_binding(&self, name: &str) -> Option<&Binding> {
        self.locals
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .or_else(|| self.globals.get(name))
    }

    fn get_binding_mut(&mut self, name: &str) -> Option<&mut Binding> {
        if let Some(scope) = self
            .locals
            .iter_mut()
            .rev()
            .find(|scope| scope.contains_key(name))
        {
            return scope.get_mut(name);
        }
        self.globals.get_mut(name)
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
            | Value::Function(_) => {}
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

    fn step(&mut self) -> Result<()> {
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
