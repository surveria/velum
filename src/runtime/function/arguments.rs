use crate::{
    error::{Error, Result},
    runtime::Context,
    value::Value,
};

impl Context {
    /// Creates the arguments value from the original passed arguments.
    /// The engine models its elements as a dense array so indexed access,
    /// `length`, iteration, and spread all work, while retaining an explicit
    /// Arguments builtin-class marker. Mapped parameter aliasing and `callee`
    /// are not modeled.
    pub(super) fn create_arguments_object(&mut self, original_args: &[Value]) -> Result<Value> {
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
