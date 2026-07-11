use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::binding::scope::{BindingCell, BindingScope},
    syntax::DeclKind,
    value::Value,
};

const ARGUMENTS_BINDING_NAME: &str = "arguments";

impl Context {
    /// Builds a one-binding scope holding the `arguments` object. The caller
    /// pushes it *below* the function scope so parameters, hoisted vars, and
    /// lexical bindings named `arguments` shadow it through normal scope
    /// ordering without disturbing compiled slot layouts.
    pub(super) fn arguments_wrapper_scope(
        &mut self,
        original_args: &[Value],
    ) -> Result<BindingScope> {
        let atom = self.intern_atom(ARGUMENTS_BINDING_NAME)?;
        let value = self.create_arguments_object(original_args)?;
        self.ensure_extra_binding_capacity(0)?;
        let mut scope = BindingScope::new();
        scope.insert(atom, BindingCell::new(value, true, DeclKind::Var))?;
        Ok(scope)
    }

    /// Creates the arguments value from the original passed arguments.
    /// The engine models its elements as a dense array so indexed access,
    /// `length`, iteration, and spread all work, while retaining an explicit
    /// Arguments builtin-class marker. Mapped parameter aliasing and `callee`
    /// are not modeled.
    fn create_arguments_object(&mut self, original_args: &[Value]) -> Result<Value> {
        let value = self.create_array_from_elements(original_args.to_vec())?;
        let Value::Object(id) = &value else {
            return Err(Error::runtime(
                "arguments object allocation did not return an object",
            ));
        };
        self.objects.mark_arguments_object(*id)?;
        Ok(value)
    }
}
