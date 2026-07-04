use std::collections::BTreeMap;

use crate::ast::{BinaryOp, DeclKind, Expr, Program, Stmt, UnaryOp};
use crate::error::{Error, Result};
use crate::lexer;
use crate::parser;
use crate::value::Value;

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
    output: Vec<String>,
    runtime_steps: usize,
}

#[derive(Debug, Clone)]
struct Binding {
    value: Value,
    mutable: bool,
    kind: DeclKind,
}

#[derive(Debug, Clone, PartialEq)]
enum Completion {
    Normal(Value),
    Throw(Value),
}

impl Completion {
    fn into_result(self) -> Result<Value> {
        match self {
            Self::Normal(value) => Ok(value),
            Self::Throw(value) => Err(Error::runtime(format!("uncaught throw: {value}"))),
        }
    }
}

impl Context {
    #[must_use]
    pub const fn new(limits: RuntimeLimits) -> Self {
        Self {
            limits,
            globals: BTreeMap::new(),
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
            Stmt::Throw(_) | Stmt::VarDecl { .. } | Stmt::Expr(_) => Ok(()),
        }
    }

    fn hoist_var(&mut self, name: &str) -> Result<()> {
        if let Some(binding) = self.globals.get(name) {
            if binding.kind == DeclKind::Var {
                return Ok(());
            }
            return Err(Error::runtime(format!(
                "'{name}' has already been declared"
            )));
        }

        self.ensure_binding_capacity(name)?;
        self.globals.insert(
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
                .globals
                .get(name)
                .map(|binding| binding.value.clone())
                .ok_or_else(|| Error::runtime(format!("'{name}' is not defined"))),
            Expr::Unary { op, expr } => {
                let value = self.eval_expr(expr)?;
                Self::eval_unary(*op, &value)
            }
            Expr::Binary { op, left, right } => self.eval_binary(*op, left, right),
            Expr::Assignment { name, expr } => {
                let value = self.eval_expr(expr)?;
                self.assign(name, value.clone())?;
                Ok(value)
            }
            Expr::Call { callee, args } => self.eval_call(callee, args),
            Expr::New { constructor, args } => self.eval_new(constructor, args),
        }
    }

    fn eval_block(&mut self, statements: &[Stmt]) -> Result<Completion> {
        let mut last = Value::Undefined;
        for statement in statements {
            self.step()?;
            match self.eval_statement(statement)? {
                Completion::Normal(value) => last = value,
                Completion::Throw(value) => return Ok(Completion::Throw(value)),
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
        }
    }

    fn eval_catch(
        &mut self,
        catch_param: &str,
        value: Value,
        catch_body: &[Stmt],
    ) -> Result<Completion> {
        let previous = self.globals.remove(catch_param);
        if previous.is_none() {
            self.ensure_binding_capacity(catch_param)?;
        }
        self.checked_value(value.clone())?;
        self.globals.insert(
            catch_param.to_owned(),
            Binding {
                value,
                mutable: true,
                kind: DeclKind::Let,
            },
        );
        let result = self.eval_block(catch_body);
        let removed = self.globals.remove(catch_param);
        if removed.is_none() {
            return Err(Error::runtime("catch binding disappeared"));
        }
        if let Some(previous) = previous {
            self.globals.insert(catch_param.to_owned(), previous);
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
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
                return Err(Error::runtime("logical operator reached eager evaluation"));
            }
        };
        self.checked_value(value)
    }

    fn eval_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<Value> {
        let Expr::Identifier(name) = callee else {
            return Err(Error::runtime("only host function calls are supported"));
        };

        match name.as_str() {
            BOOLEAN_NAME => self.eval_boolean_call(args),
            HOST_PRINT_NAME => {
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
            _ => Err(Error::runtime(format!("'{name}' is not callable"))),
        }
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
            return Ok(Value::String(TEST262_ERROR_NAME.to_owned()));
        };
        let message = self.eval_expr(message)?;
        Ok(Value::String(message.display_for_concat()))
    }

    fn define(&mut self, name: &str, value: Value, kind: DeclKind) -> Result<()> {
        self.ensure_binding_capacity(name)?;
        if self.globals.contains_key(name) {
            return Err(Error::runtime(format!(
                "'{name}' has already been declared"
            )));
        }

        self.checked_value(value.clone())?;
        let mutable = kind != DeclKind::Const;
        self.globals.insert(
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
        if self.globals.len() >= self.limits.max_bindings && !self.globals.contains_key(name) {
            return Err(Error::limit(format!(
                "binding count exceeded {}",
                self.limits.max_bindings
            )));
        }
        Ok(())
    }

    fn assign(&mut self, name: &str, value: Value) -> Result<()> {
        self.checked_value(value.clone())?;
        let Some(binding) = self.globals.get_mut(name) else {
            return Err(Error::runtime(format!("'{name}' is not defined")));
        };

        if !binding.mutable {
            return Err(Error::runtime(format!("assignment to constant '{name}'")));
        }

        binding.value = value;
        Ok(())
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
        if let Value::String(text) = &value {
            self.check_string_len(text)?;
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

fn numeric_binary(
    left: &Value,
    right: &Value,
    op: &str,
    apply: impl FnOnce(f64, f64) -> f64,
) -> Result<Value> {
    let Some(left) = left.as_number() else {
        return Err(Error::runtime(format!("operator '{op}' expects numbers")));
    };
    let Some(right) = right.as_number() else {
        return Err(Error::runtime(format!("operator '{op}' expects numbers")));
    };
    Ok(Value::Number(apply(left, right)))
}

fn compare_binary(
    left: &Value,
    right: &Value,
    op: &str,
    apply: impl FnOnce(f64, f64) -> bool,
) -> Result<Value> {
    let Some(left) = left.as_number() else {
        return Err(Error::runtime(format!("operator '{op}' expects numbers")));
    };
    let Some(right) = right.as_number() else {
        return Err(Error::runtime(format!("operator '{op}' expects numbers")));
    };
    Ok(Value::Bool(apply(left, right)))
}
