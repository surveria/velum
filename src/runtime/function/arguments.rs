#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        object::{
            AccessorPropertyUpdate, DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable,
            PropertyKey, PropertyLookup, PropertyUpdate, PropertyWritable,
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
    pub(super) fn legacy_function_arguments_snapshot(
        &self,
        function: crate::value::FunctionId,
        original_args: &[Value],
        unmapped: bool,
    ) -> Result<Option<crate::runtime::activation::LegacyFunctionArguments>> {
        Ok(self.function_has_legacy_semantics(function)?.then(|| {
            crate::runtime::activation::LegacyFunctionArguments::new(original_args, unmapped)
        }))
    }

    pub(super) fn function_has_legacy_semantics(
        &self,
        function: crate::value::FunctionId,
    ) -> Result<bool> {
        let function = self.function(function)?;
        Ok(!function.bytecode.strict() && function.constructable)
    }

    pub(in crate::runtime) fn function_uses_restricted_prototype(
        &self,
        function: crate::value::FunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        Ok(
            !self.function_has_legacy_semantics(function)?
                && Self::is_restricted_property(property),
        )
    }

    pub(super) fn active_function_has_arguments_binding(&self) -> bool {
        let Some(atom) = self.atom(ARGUMENTS_BINDING_NAME) else {
            return false;
        };
        self.locals
            .iter()
            .skip(self.current_local_frame_start())
            .any(|scope| scope.contains(atom))
    }

    pub(super) fn initialize_legacy_function_arguments(
        &mut self,
        function: crate::value::FunctionId,
        binding: Option<super::FunctionArgumentsBinding>,
        arguments_scope: Option<&crate::runtime::binding::scope::BindingScope>,
        parameter_scope: &crate::runtime::binding::scope::BindingScope,
    ) -> Result<()> {
        let Some(frame_index) = self
            .activation_frames
            .iter()
            .rposition(|frame| frame.function_id() == Some(function))
        else {
            return Err(Error::runtime(
                "legacy arguments activation frame disappeared during setup",
            ));
        };
        let Some(arguments) = self
            .activation_frames
            .get(frame_index)
            .and_then(crate::runtime::activation::ActivationFrame::legacy_arguments)
        else {
            return Ok(());
        };
        let (original_args, _, unmapped) = arguments.materialization_inputs();
        let materialized = match (binding, arguments_scope) {
            (Some(binding), Some(scope)) => {
                let cell = scope.get(binding.atom()).ok_or_else(|| {
                    Error::runtime("legacy arguments binding disappeared during setup")
                })?;
                Some(cell.value(ARGUMENTS_BINDING_NAME)?)
            }
            (None, None) => None,
            (Some(_), None) | (None, Some(_)) => {
                return Err(Error::runtime(
                    "legacy arguments binding scope does not match its metadata",
                ));
            }
        };
        let parameter_map = if materialized.is_none() && !unmapped {
            self.arguments_parameter_map(function, original_args.len(), parameter_scope)?
        } else {
            Vec::new()
        };
        let frame = self
            .activation_frames
            .get_mut(frame_index)
            .ok_or_else(|| Error::runtime("legacy arguments activation frame disappeared"))?;
        let arguments = frame
            .legacy_arguments_mut()
            .ok_or_else(|| Error::runtime("legacy arguments activation state disappeared"))?;
        if let Some(object) = materialized {
            arguments.set_object(object);
        } else {
            arguments.set_parameter_map(parameter_map);
        }
        Ok(())
    }

    pub(super) fn legacy_function_arguments_value(
        &mut self,
        function: crate::value::FunctionId,
    ) -> Result<Value> {
        let Some(frame_index) = self
            .activation_frames
            .iter()
            .rposition(|frame| frame.function_id() == Some(function))
        else {
            return Ok(Value::Null);
        };
        let arguments = self
            .activation_frames
            .get(frame_index)
            .and_then(crate::runtime::activation::ActivationFrame::legacy_arguments)
            .ok_or_else(|| Error::runtime("active legacy function has no arguments snapshot"))?;
        if let Some(object) = arguments.object() {
            return Ok(object.clone());
        }
        let (original_args, parameter_map, unmapped) = arguments.materialization_inputs();
        let object = self.create_arguments_object_with_parameter_map(
            function,
            unmapped,
            &original_args,
            parameter_map,
        )?;
        let frame = self
            .activation_frames
            .get_mut(frame_index)
            .ok_or_else(|| Error::runtime("legacy arguments activation frame disappeared"))?;
        let arguments = frame
            .legacy_arguments_mut()
            .ok_or_else(|| Error::runtime("legacy arguments activation state disappeared"))?;
        arguments.set_object(object.clone());
        Ok(object)
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
        let parameter_map = if unmapped {
            Vec::new()
        } else {
            self.arguments_parameter_map(function, original_args.len(), parameter_scope)?
        };
        self.create_arguments_object_with_parameter_map(
            function,
            unmapped,
            original_args,
            parameter_map,
        )
    }

    pub(super) fn create_arguments_object_with_parameter_map(
        &mut self,
        function: crate::value::FunctionId,
        unmapped: bool,
        original_args: &[Value],
        parameter_map: Vec<Option<crate::runtime::binding::scope::BindingCell>>,
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

    pub(super) fn arguments_parameter_map(
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
