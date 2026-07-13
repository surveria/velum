use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        object::{
            AccessorPropertyUpdate, DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable,
            PropertyKey, PropertyUpdate, PropertyWritable,
        },
    },
    value::Value,
};

const ARGUMENTS_BINDING_NAME: &str = "arguments";
const ARGUMENTS_ITERATOR_DISPLAY: &str = "[Symbol.iterator]";
const ARGUMENTS_ARRAY_VALUES_NAME: &str = "values";
const ARGUMENTS_LENGTH_PROPERTY: &str = "length";
const ARGUMENTS_CALLEE_PROPERTY: &str = "callee";

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
    /// Indexed values and `length` are ordinary own properties. Sloppy simple
    /// parameter lists additionally retain the spec parameter-map cells.
    pub(super) fn create_arguments_object(
        &mut self,
        function: crate::value::FunctionId,
        unmapped: bool,
        original_args: &[Value],
        parameter_scope: &crate::runtime::binding::scope::BindingScope,
    ) -> Result<Value> {
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
        let parameter_map = if unmapped {
            Vec::new()
        } else {
            self.arguments_parameter_map(function, original_args.len(), parameter_scope)?
        };
        self.objects.mark_arguments_object(*id, parameter_map)?;
        self.install_arguments_values(*id, original_args)?;
        self.install_arguments_length(*id, original_args.len())?;
        self.install_arguments_iterator(*id)?;
        if unmapped {
            self.install_arguments_restricted_callee(*id)?;
        } else {
            self.install_arguments_callee(*id, function)?;
        }
        Ok(value)
    }

    fn arguments_parameter_map(
        &self,
        function: crate::value::FunctionId,
        argument_count: usize,
        parameter_scope: &crate::runtime::binding::scope::BindingScope,
    ) -> Result<Vec<Option<crate::runtime::binding::scope::BindingCell>>> {
        let parameter_atoms = self.function(function)?.param_atoms.clone();
        let mut mapped = vec![None; argument_count];
        let mut seen = Vec::new();
        for (index, atom) in parameter_atoms
            .iter()
            .copied()
            .take(argument_count)
            .enumerate()
            .rev()
        {
            if seen.contains(&atom) {
                continue;
            }
            seen.push(atom);
            let cell = parameter_scope
                .get(atom)
                .ok_or_else(|| Error::runtime("mapped argument parameter binding disappeared"))?;
            let Some(slot) = mapped.get_mut(index) else {
                return Err(Error::runtime("mapped argument index disappeared"));
            };
            *slot = Some(cell);
        }
        Ok(mapped)
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
        self.array_constructor_value()?;
        let array_prototype = self.objects.existing_array_prototype_id()?;
        let iterator =
            self.get_named(&Value::Object(array_prototype), ARGUMENTS_ARRAY_VALUES_NAME)?;
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

    fn install_arguments_restricted_callee(&mut self, id: crate::value::ObjectId) -> Result<()> {
        let thrower = self.realm_throw_type_error()?;
        let key = self.intern_property_key(ARGUMENTS_CALLEE_PROPERTY)?;
        self.objects.define_property(
            id,
            key,
            ARGUMENTS_CALLEE_PROPERTY,
            PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                Some(thrower.clone()),
                Some(thrower),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::No),
            )),
            self.limits.max_object_properties,
        )
    }

    fn install_arguments_callee(
        &mut self,
        id: crate::value::ObjectId,
        function: crate::value::FunctionId,
    ) -> Result<()> {
        let key = self.intern_property_key(ARGUMENTS_CALLEE_PROPERTY)?;
        self.objects.define_property(
            id,
            key,
            ARGUMENTS_CALLEE_PROPERTY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(Value::Function(function)),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }
}
