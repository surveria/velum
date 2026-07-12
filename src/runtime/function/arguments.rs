use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        native::NativeFunctionKind,
        object::{
            DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyKey,
            PropertyUpdate, PropertyWritable,
        },
    },
    value::Value,
};

const ARGUMENTS_BINDING_NAME: &str = "arguments";
const ARGUMENTS_ITERATOR_DISPLAY: &str = "[Symbol.iterator]";
const ARGUMENTS_LENGTH_PROPERTY: &str = "length";

impl Context {
    pub(super) fn active_function_has_arguments_binding(&self) -> bool {
        let Some(atom) = self.atom(ARGUMENTS_BINDING_NAME) else {
            return false;
        };
        self.locals
            .iter()
            .skip(self.current_local_frame_start())
            .any(|scope| scope.contains(atom))
    }

    /// Creates the arguments value from the original passed arguments.
    /// Indexed values and `length` are ordinary own properties. The explicit
    /// Arguments builtin-class marker only supplies the internal brand. Mapped
    /// parameter aliasing and `callee` are not modeled.
    pub(super) fn create_arguments_object(&mut self, original_args: &[Value]) -> Result<Value> {
        let constructor_key = self.object_constructor_property_key()?;
        let value = self.objects.create_with_prototype(
            None,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(id) = &value else {
            return Err(Error::runtime(
                "arguments object allocation did not return an object",
            ));
        };
        self.objects.mark_arguments_object(*id)?;
        self.install_arguments_values(*id, original_args)?;
        self.install_arguments_length(*id, original_args.len())?;
        self.install_arguments_iterator(*id)?;
        Ok(value)
    }

    fn install_arguments_values(
        &mut self,
        id: crate::value::ObjectId,
        original_args: &[Value],
    ) -> Result<()> {
        for (index, value) in original_args.iter().enumerate() {
            let name = index.to_string();
            let key = self.intern_property_key(&name)?;
            self.objects.define_property(
                id,
                key,
                &name,
                PropertyUpdate::Data(DataPropertyUpdate::new(
                    Some(value.clone()),
                    Some(PropertyWritable::Yes),
                    Some(PropertyEnumerable::Yes),
                    Some(PropertyConfigurable::Yes),
                )),
                self.limits.max_object_properties,
            )?;
        }
        Ok(())
    }

    fn install_arguments_length(
        &mut self,
        id: crate::value::ObjectId,
        length: usize,
    ) -> Result<()> {
        let key = self.intern_property_key(ARGUMENTS_LENGTH_PROPERTY)?;
        let length = u32::try_from(length)
            .map_err(|_| Error::limit("arguments length exceeded the supported range"))?;
        self.objects.define_property(
            id,
            key,
            ARGUMENTS_LENGTH_PROPERTY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(Value::Number(f64::from(length))),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn install_arguments_iterator(&mut self, id: crate::value::ObjectId) -> Result<()> {
        self.symbol_constructor_value()?;
        let Some(symbol) = self.iterator_symbol() else {
            return Err(Error::runtime("Symbol.iterator is not initialized"));
        };
        let iterator =
            self.create_native_function(NativeFunctionKind::ArrayValues, Value::Undefined)?;
        self.objects.define_property(
            id,
            PropertyKey::symbol(symbol),
            ARGUMENTS_ITERATOR_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(iterator),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }
}
