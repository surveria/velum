use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        native::NativeFunctionKind,
        object::{AccessorPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyKey},
    },
    value::{NativeFunctionId, Value},
};

const SPECIES_SYMBOL_DISPLAY: &str = "[Symbol.species]";
const SPECIES_SYMBOL_PROPERTY: &str = "species";

impl Context {
    pub(super) fn install_species_accessor(&mut self, constructor: NativeFunctionId) -> Result<()> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let species = self.get_named(&symbol_constructor, SPECIES_SYMBOL_PROPERTY)?;
        let Value::Symbol(species) = species else {
            return Err(Error::runtime("Symbol.species is not initialized"));
        };
        let getter = self.create_ephemeral_native_function(
            NativeFunctionKind::SpeciesGetter,
            Value::Undefined,
        )?;
        self.define_native_function_accessor_property_key(
            constructor,
            SPECIES_SYMBOL_DISPLAY,
            PropertyKey::symbol(species.id()),
            AccessorPropertyUpdate::new(
                Some(getter),
                None,
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            ),
        )
    }
}
