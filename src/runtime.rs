use std::collections::BTreeMap;

use crate::ast::{BinaryOp, Expr, Program, Stmt, UnaryOp};
use crate::error::{Error, Result};
use crate::lexer;
use crate::parser;
use crate::value::Value;

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
            max_source_len: 64 * 1024,
            max_statements: 4_096,
            max_expression_depth: 256,
            max_runtime_steps: 100_000,
            max_string_len: 64 * 1024,
            max_bindings: 4_096,
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
    pub fn with_limits(limits: RuntimeLimits) -> Self {
        Self { limits }
    }

    #[must_use]
    pub fn limits(&self) -> RuntimeLimits {
        self.limits
    }

    #[must_use]
    pub fn context(&self) -> Context {
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
}

impl Context {
    #[must_use]
    pub fn new(limits: RuntimeLimits) -> Self {
        Self {
            limits,
            globals: BTreeMap::new(),
            output: Vec::new(),
            runtime_steps: 0,
        }
    }

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
    pub fn runtime_steps(&self) -> usize {
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
        let mut last = Value::Undefined;
        for statement in &program.statements {
            self.step()?;
            last = self.eval_statement(statement)?;
        }
        Ok(last)
    }

    fn eval_statement(&mut self, statement: &Stmt) -> Result<Value> {
        match statement {
            Stmt::VarDecl {
                name,
                mutable,
                init,
            } => {
                let value = self.eval_expr(init)?;
                self.define(name, value, *mutable)?;
                Ok(Value::Undefined)
            }
            Stmt::Expr(expr) => self.eval_expr(expr),
        }
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
        }
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
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr => unreachable!("handled before eager eval"),
        };
        self.checked_value(value)
    }

    fn eval_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<Value> {
        let Expr::Identifier(name) = callee else {
            return Err(Error::runtime("only host function calls are supported"));
        };

        match name.as_str() {
            "print" => {
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

    fn define(&mut self, name: &str, value: Value, mutable: bool) -> Result<()> {
        if self.globals.len() >= self.limits.max_bindings && !self.globals.contains_key(name) {
            return Err(Error::limit(format!(
                "binding count exceeded {}",
                self.limits.max_bindings
            )));
        }

        if self.globals.contains_key(name) {
            return Err(Error::runtime(format!(
                "'{name}' has already been declared"
            )));
        }

        self.checked_value(value.clone())?;
        self.globals
            .insert(name.to_owned(), Binding { value, mutable });
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
        self.runtime_steps += 1;
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
