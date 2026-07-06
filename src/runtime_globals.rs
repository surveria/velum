use crate::{
    atom::AtomId,
    error::Result,
    runtime::Context,
    runtime_object::{OBJECT_CONSTRUCTOR_PROPERTY, PropertyKey, PropertyLookup},
    string_heap::JsString,
    value::{ObjectId, Value},
};

impl Context {
    #[must_use]
    pub fn output(&self) -> &[String] {
        &self.output
    }

    #[must_use]
    pub fn take_output(&mut self) -> Vec<String> {
        std::mem::take(&mut self.output)
    }

    #[must_use]
    pub fn get_global(&self, name: &str) -> Option<Value> {
        let atom = self.atom(name)?;
        self.globals
            .get(atom)
            .or_else(|| self.builtin_globals.get(atom))
            .map(|binding| binding.value())
    }

    #[must_use]
    pub const fn runtime_steps(&self) -> usize {
        self.runtime_steps
    }

    #[must_use]
    pub const fn atom_count(&self) -> usize {
        self.atoms.len()
    }

    #[must_use]
    pub const fn string_count(&self) -> usize {
        self.strings.len()
    }

    #[must_use]
    pub const fn string_bytes(&self) -> usize {
        self.strings.bytes()
    }

    pub(crate) const fn global_binding_count(&self) -> usize {
        self.globals
            .len()
            .saturating_add(self.builtin_globals.len())
    }

    pub(crate) const fn shape_count(&self) -> usize {
        self.objects.shape_count()
    }

    pub(crate) const fn native_function_count(&self) -> usize {
        self.native_functions.len()
    }

    pub(crate) const fn prototype_lookup_version(&self) -> u64 {
        self.objects.prototype_lookup_version()
    }

    pub(crate) fn captured_scope_count(&self) -> usize {
        self.functions.iter().fold(0usize, |count, function| {
            count.saturating_add(function.captures.scope_count())
        })
    }

    pub(crate) fn captured_binding_count(&self) -> usize {
        self.functions.iter().fold(0usize, |count, function| {
            count.saturating_add(function.captures.binding_count())
        })
    }

    pub(crate) fn upvalue_cell_count(&self) -> usize {
        self.functions.iter().fold(0usize, |count, function| {
            count.saturating_add(
                function
                    .upvalues
                    .iter()
                    .filter(|cell| cell.is_some())
                    .count(),
            )
        })
    }

    pub(crate) fn intern_atom(&mut self, name: &str) -> Result<AtomId> {
        self.check_string_len(name)?;
        self.atoms.intern(name)
    }

    pub(crate) fn intern_heap_string(&mut self, text: &str) -> Result<JsString> {
        self.check_string_len(text)?;
        self.strings.intern(text)
    }

    pub(crate) fn heap_string_value(&mut self, text: &str) -> Result<Value> {
        self.intern_heap_string(text).map(Value::HeapString)
    }

    pub(crate) fn atom(&self, name: &str) -> Option<AtomId> {
        self.atoms.get(name)
    }

    pub(crate) fn intern_property_key(&mut self, name: &str) -> Result<PropertyKey> {
        if let Some(key) = self.well_known_properties.lookup(name) {
            return Ok(key);
        }
        let key = self.intern_atom(name).map(PropertyKey::new)?;
        self.well_known_properties.remember(name, key);
        Ok(key)
    }

    pub(crate) fn property_lookup<'a>(&self, name: &'a str) -> PropertyLookup<'a> {
        let key = self.known_property_key(name);
        PropertyLookup::new(name, key)
    }

    pub(crate) fn known_property_key(&self, name: &str) -> Option<PropertyKey> {
        self.well_known_properties
            .lookup(name)
            .or_else(|| self.atom(name).map(PropertyKey::new))
    }

    pub(crate) fn object_constructor_property_key(&mut self) -> Result<PropertyKey> {
        self.intern_property_key(OBJECT_CONSTRUCTOR_PROPERTY)
    }

    pub(crate) fn define_non_enumerable_object_property(
        &mut self,
        id: ObjectId,
        name: &str,
        value: Value,
    ) -> Result<()> {
        let key = self.intern_property_key(name)?;
        self.objects
            .define_non_enumerable(id, key, name, value, self.limits.max_object_properties)
    }
}
