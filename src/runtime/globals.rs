use crate::{
    error::Result,
    runtime::binding::scope::BindingCell,
    runtime::native::{GLOBAL_THIS_NAME, INFINITY_NAME, NAN_NAME},
    runtime::object::{
        DataPropertyDescriptor, DataPropertyUpdate, OBJECT_CONSTRUCTOR_PROPERTY,
        OwnPropertyDescriptor, PropertyConfigurable, PropertyEnumerable, PropertyKey,
        PropertyLookup, PropertyUpdate, PropertyWritable,
    },
    runtime::property::{delete_property, get_property, has_property},
    runtime::{Context, VmStorageKind},
    storage::atom::AtomId,
    storage::string_heap::JsString,
    syntax::DeclKind,
    value::{ObjectId, Value},
};

impl Context {
    pub(crate) fn unresolved_global_property_value(&mut self, name: &str) -> Result<Option<Value>> {
        let Some(global_object) = self.global_object else {
            return Ok(None);
        };
        let lookup = self.property_lookup(name);
        self.global_object_property_value(global_object, lookup)
    }

    pub(crate) fn delete_unresolved_global_property(&mut self, name: &str) -> Result<bool> {
        let Some(global_object) = self.global_object else {
            return Ok(true);
        };
        let object = Value::Object(global_object);
        let lookup = self.property_lookup(name);
        delete_property(&mut self.objects, &object, lookup)
    }

    #[must_use]
    pub fn output(&self) -> &[String] {
        &self.output
    }

    #[must_use]
    pub fn take_output(&mut self) -> Vec<String> {
        self.output_payload_bytes = 0;
        std::mem::take(&mut self.output)
    }

    /// Returns the current raw binding value without retaining it.
    ///
    /// Use `get_global_retained` when the result must survive across later
    /// Context calls.
    #[must_use]
    pub fn get_global(&self, name: &str) -> Option<Value> {
        let atom = self.atom(name)?;
        self.globals
            .get(atom)
            .or_else(|| self.builtin_globals.get(atom))
            .and_then(|binding| binding.value(name).ok())
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
    pub const fn symbol_count(&self) -> usize {
        self.symbols.len()
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

    pub(crate) fn upvalue_cell_count(&self) -> usize {
        self.functions.iter().fold(0usize, |count, function| {
            count.saturating_add(function.upvalues.len())
        })
    }

    pub(crate) fn intern_atom(&mut self, name: &str) -> Result<AtomId> {
        self.check_string_len(name)?;
        let reservation = if self.atoms.get(name).is_none() {
            Some(
                self.storage_ledger
                    .reserve_count(crate::runtime::VmStorageKind::CacheEntry, 1)?,
            )
        } else {
            None
        };
        let atom = self.atoms.intern(name)?;
        if let Some(reservation) = reservation {
            reservation.commit()?;
        }
        Ok(atom)
    }

    pub(crate) fn intern_heap_string(&mut self, text: &str) -> Result<JsString> {
        self.check_string_len(text)?;
        let reservation = if self.strings.contains(text) {
            None
        } else {
            Some(
                self.storage_ledger
                    .reserve_count(crate::runtime::VmStorageKind::CacheEntry, 1)?,
            )
        };
        let string = self.strings.intern(text)?;
        if let Some(reservation) = reservation {
            reservation.commit()?;
        }
        Ok(string)
    }

    pub(crate) fn intern_owned_heap_string(&mut self, text: String) -> Result<JsString> {
        self.check_string_len(&text)?;
        let reservation = if self.strings.contains(&text) {
            None
        } else {
            Some(
                self.storage_ledger
                    .reserve_count(crate::runtime::VmStorageKind::CacheEntry, 1)?,
            )
        };
        let string = self.strings.intern_owned(text)?;
        if let Some(reservation) = reservation {
            reservation.commit()?;
        }
        Ok(string)
    }

    pub(crate) fn heap_string_value(&mut self, text: &str) -> Result<Value> {
        self.intern_heap_string(text).map(Value::HeapString)
    }

    pub(crate) fn heap_string_owned_value(&mut self, text: String) -> Result<Value> {
        self.intern_owned_heap_string(text).map(Value::HeapString)
    }

    pub(crate) fn create_symbol_value(&mut self, description: Option<&str>) -> Result<Value> {
        let description = if let Some(description) = description {
            Some(self.intern_heap_string(description)?)
        } else {
            None
        };
        self.symbols.create(description).map(Value::Symbol)
    }

    pub(crate) fn atom(&self, name: &str) -> Option<AtomId> {
        self.atoms.get(name)
    }

    pub(crate) fn intern_property_key(&mut self, name: &str) -> Result<PropertyKey> {
        if let Some(key) = self.well_known_properties.lookup(name) {
            return Ok(key);
        }
        let remember = self.well_known_properties.should_remember(name);
        let key = self.intern_atom(name).map(PropertyKey::new)?;
        let reservation = if remember {
            Some(
                self.storage_ledger
                    .reserve_count(crate::runtime::VmStorageKind::CacheEntry, 1)?,
            )
        } else {
            None
        };
        if let Some(reservation) = reservation {
            reservation.commit()?;
        }
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

    pub(crate) fn global_this_value(&mut self) -> Result<Value> {
        self.global_object_id().map(Value::Object)
    }

    pub(crate) fn global_object_id(&mut self) -> Result<ObjectId> {
        if let Some(id) = self.global_object {
            return Ok(id);
        }

        let constructor_key = self.object_constructor_property_key()?;
        let id = self.objects.create_with_prototype_id(
            None,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        self.storage_ledger
            .grow_count(VmStorageKind::Association, 1)?;
        self.global_object = Some(id);
        let value = Value::Object(id);
        self.define_global_object_data_property(
            id,
            GLOBAL_THIS_NAME,
            value.clone(),
            PropertyWritable::Yes,
            PropertyEnumerable::No,
            PropertyConfigurable::Yes,
        )?;
        self.insert_mutable_global_builtin(GLOBAL_THIS_NAME, value)?;
        Ok(id)
    }

    pub(crate) fn is_global_object_id(&self, id: ObjectId) -> bool {
        matches!(self.global_object, Some(global) if global == id)
    }

    pub(crate) fn global_object_property_value(
        &mut self,
        id: ObjectId,
        lookup: PropertyLookup<'_>,
    ) -> Result<Option<Value>> {
        if !self.is_global_object_id(id) {
            return Ok(None);
        }
        let object = Value::Object(id);
        if has_property(&self.objects, &object, lookup)? {
            let value = get_property(&self.objects, &object, lookup)?;
            return self.runtime_property_value(value).map(Some);
        }
        self.global_binding_property_value(lookup.name())
    }

    pub(crate) fn global_object_has_property(
        &mut self,
        id: ObjectId,
        lookup: PropertyLookup<'_>,
    ) -> Result<Option<bool>> {
        if !self.is_global_object_id(id) {
            return Ok(None);
        }
        let object = Value::Object(id);
        if has_property(&self.objects, &object, lookup)? {
            return Ok(Some(true));
        }
        self.global_binding_property_value(lookup.name())
            .map(|value| Some(value.is_some()))
    }

    pub(crate) fn global_object_property_descriptor(
        &mut self,
        id: ObjectId,
        lookup: PropertyLookup<'_>,
    ) -> Result<Option<OwnPropertyDescriptor>> {
        if !self.is_global_object_id(id) {
            return Ok(None);
        }
        let Some(value) = self.global_binding_property_value(lookup.name())? else {
            return Ok(None);
        };
        let writable = if matches!(lookup.name(), NAN_NAME | INFINITY_NAME) {
            PropertyWritable::No
        } else {
            PropertyWritable::Yes
        };
        let configurable = if matches!(lookup.name(), NAN_NAME | INFINITY_NAME) {
            PropertyConfigurable::No
        } else {
            PropertyConfigurable::Yes
        };
        let descriptor = DataPropertyDescriptor::new(
            value.clone(),
            writable,
            PropertyEnumerable::No,
            configurable,
        );
        self.define_global_object_data_property(
            id,
            lookup.name(),
            value,
            writable,
            PropertyEnumerable::No,
            configurable,
        )?;
        Ok(Some(OwnPropertyDescriptor::Data(descriptor)))
    }

    pub(crate) fn sync_global_object_binding_property(
        &mut self,
        name: &str,
        value: Value,
    ) -> Result<()> {
        if name != GLOBAL_THIS_NAME {
            return Ok(());
        }
        let Some(id) = self.global_object else {
            return Ok(());
        };
        self.define_global_object_data_property(
            id,
            name,
            value,
            PropertyWritable::Yes,
            PropertyEnumerable::No,
            PropertyConfigurable::Yes,
        )
    }

    pub(crate) fn sync_global_object_property_binding(
        &mut self,
        name: &str,
        value: Value,
    ) -> Result<()> {
        if name != GLOBAL_THIS_NAME {
            return Ok(());
        }
        let value = self.runtime_value(value)?;
        if let Some(cell) = self.builtin_global_cell(name) {
            return cell.assign(name, value);
        }
        Ok(())
    }

    fn global_binding_property_value(&mut self, name: &str) -> Result<Option<Value>> {
        if let Some(binding) = self.get_binding(name) {
            return binding.value(name).map(Some);
        }
        self.builtin_value(name)
    }

    fn insert_mutable_global_builtin(&mut self, name: &str, value: Value) -> Result<()> {
        let atom = self.intern_atom(name)?;
        if self.builtin_globals.contains(atom) {
            return Ok(());
        }
        self.ensure_extra_binding_capacity(1)?;
        self.builtin_globals
            .insert(atom, BindingCell::new(value, true, DeclKind::Var))?;
        Ok(())
    }

    fn builtin_global_cell(&self, name: &str) -> Option<BindingCell> {
        let atom = self.atom(name)?;
        self.builtin_globals.get(atom)
    }

    pub(crate) fn define_global_object_data_property(
        &mut self,
        id: ObjectId,
        name: &str,
        value: Value,
        writable: PropertyWritable,
        enumerable: PropertyEnumerable,
        configurable: PropertyConfigurable,
    ) -> Result<()> {
        let key = self.intern_property_key(name)?;
        let update = PropertyUpdate::Data(DataPropertyUpdate::new(
            Some(value),
            Some(writable),
            Some(enumerable),
            Some(configurable),
        ));
        self.objects
            .define_property(id, key, name, update, self.limits.max_object_properties)
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
