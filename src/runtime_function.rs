use crate::{
    ast::{DeclKind, Expr, Stmt},
    error::{Error, Result},
    runtime::Context,
    runtime_completion::Completion,
    runtime_scope::{BindingCell, BindingScope},
    value::{FunctionId, Value},
};

const FUNCTION_LENGTH_PROPERTY: &str = "length";
const FUNCTION_NAME_PROPERTY: &str = "name";

impl Context {
    pub(crate) fn create_function(
        &mut self,
        name: Option<&str>,
        params: &[String],
        body: &[Stmt],
    ) -> Value {
        let id = FunctionId::new(self.functions.len());
        self.functions.push(super::Function {
            name: name.unwrap_or_default().to_owned(),
            params: params.to_vec(),
            body: body.to_vec(),
            captures: self.locals.clone(),
        });
        Value::Function(id)
    }

    pub(crate) fn eval_function(&mut self, id: FunctionId, args: &[Expr]) -> Result<Value> {
        let value = self
            .eval_function_completion(id, args)?
            .into_function_result()?;
        self.checked_value(value)
    }

    pub(crate) fn eval_function_completion(
        &mut self,
        id: FunctionId,
        args: &[Expr],
    ) -> Result<Completion> {
        let function = self.function(id)?.clone();
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

    pub(crate) fn get_function_property(&self, id: FunctionId, property: &str) -> Result<Value> {
        let function = self.function(id)?;
        let value = match property {
            FUNCTION_LENGTH_PROPERTY => Value::Number(function.length()?),
            FUNCTION_NAME_PROPERTY => Value::String(function.name.clone()),
            _ => Value::Undefined,
        };
        self.checked_value(value)
    }

    pub(crate) fn has_function_property(&self, id: FunctionId, property: &str) -> Result<bool> {
        self.function(id)
            .map(|_| matches!(property, FUNCTION_LENGTH_PROPERTY | FUNCTION_NAME_PROPERTY))
    }

    pub(crate) fn function_enumerable_keys(&self, id: FunctionId) -> Result<Vec<String>> {
        self.function(id).map(|_| Vec::new())
    }

    fn function(&self, id: FunctionId) -> Result<&super::Function> {
        self.functions
            .get(id.index())
            .ok_or_else(|| Error::runtime("function id is not defined"))
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
}

impl super::Function {
    fn length(&self) -> Result<f64> {
        let length = u32::try_from(self.params.len())
            .map_err(|_| Error::limit("function parameter count exceeded supported range"))?;
        Ok(f64::from(length))
    }
}
