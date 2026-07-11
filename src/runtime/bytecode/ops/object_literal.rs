use std::rc::Rc;

use crate::{
    bytecode::BytecodeObjectProperty,
    error::{Error, Result},
    runtime::Context,
    runtime::object::{
        OBJECT_CONSTRUCTOR_PROPERTY, ObjectPropertyInit, PropertyEnumerable, PropertyKey,
    },
    syntax::AccessorKind,
    value::Value,
};

impl Context {
    pub(in crate::runtime::bytecode) fn create_bytecode_object_literal(
        &mut self,
        properties: &Rc<[BytecodeObjectProperty]>,
        values: Vec<Value>,
    ) -> Result<Value> {
        if object_literal_stack_value_count(properties)? != values.len() {
            return Err(Error::runtime(
                "bytecode object literal stack arity mismatch",
            ));
        }
        let mut values = values.into_iter();
        let mut dynamic_names = Vec::new();
        let mut entries = Vec::with_capacity(properties.len());
        for property in properties.iter() {
            match property {
                BytecodeObjectProperty::Static(name) => {
                    let value = next_object_literal_stack_value(&mut values)?;
                    let key = self.intern_static_property_key(name)?;
                    entries.push(RuntimeObjectLiteralEntry {
                        key,
                        name: RuntimeObjectLiteralName::Static(name.as_str()),
                        value,
                        accessor: None,
                    });
                }
                BytecodeObjectProperty::StaticAccessor { key: name, kind } => {
                    let value = next_object_literal_stack_value(&mut values)?;
                    self.set_function_name(&value, name.as_str(), Some(*kind))?;
                    let key = self.intern_static_property_key(name)?;
                    entries.push(RuntimeObjectLiteralEntry {
                        key,
                        name: RuntimeObjectLiteralName::Static(name.as_str()),
                        value,
                        accessor: Some(*kind),
                    });
                }
                BytecodeObjectProperty::Spread => {
                    let source = next_object_literal_stack_value(&mut values)?;
                    self.push_spread_literal_entries(&source, &mut dynamic_names, &mut entries)?;
                }
                BytecodeObjectProperty::Computed
                | BytecodeObjectProperty::ComputedInferredName
                | BytecodeObjectProperty::ComputedMethod
                | BytecodeObjectProperty::ComputedAccessor { .. } => {
                    let set_method_name = matches!(
                        property,
                        BytecodeObjectProperty::ComputedInferredName
                            | BytecodeObjectProperty::ComputedMethod
                            | BytecodeObjectProperty::ComputedAccessor { .. }
                    );
                    let accessor = match property {
                        BytecodeObjectProperty::ComputedAccessor { kind } => Some(*kind),
                        _ => None,
                    };
                    let key_value = next_object_literal_stack_value(&mut values)?;
                    let value = next_object_literal_stack_value(&mut values)?;
                    let mut property = self.dynamic_property_key(&key_value)?;
                    let key = self.intern_dynamic_property_key(&mut property)?;
                    if set_method_name {
                        self.set_function_name_from_property(&value, &property, accessor)?;
                    }
                    let name_index = dynamic_names.len();
                    dynamic_names.push(property.name().to_owned());
                    entries.push(RuntimeObjectLiteralEntry {
                        key,
                        name: RuntimeObjectLiteralName::Dynamic(name_index),
                        value,
                        accessor,
                    });
                }
            }
        }
        if values.next().is_some() {
            return Err(Error::runtime(
                "bytecode object literal stack arity mismatch",
            ));
        }
        let mut inits = Vec::with_capacity(entries.len());
        for entry in entries {
            let is_dynamic = entry.name.is_dynamic();
            let name = match entry.name {
                RuntimeObjectLiteralName::Static(name) => name,
                RuntimeObjectLiteralName::Dynamic(index) => dynamic_names
                    .get(index)
                    .map(String::as_str)
                    .ok_or_else(|| Error::runtime("computed object property name disappeared"))?,
            };
            let init = if let Some(kind) = entry.accessor {
                ObjectPropertyInit::new_accessor(entry.key, name, entry.value, kind)
            } else if is_dynamic {
                ObjectPropertyInit::new_data(entry.key, name, entry.value, PropertyEnumerable::Yes)
            } else {
                ObjectPropertyInit::new(entry.key, name, entry.value, PropertyEnumerable::Yes)
            };
            inits.push(init);
        }
        let constructor_key = self.intern_property_key(OBJECT_CONSTRUCTOR_PROPERTY)?;
        self.objects.create(
            inits,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn push_spread_literal_entries(
        &mut self,
        source: &Value,
        dynamic_names: &mut Vec<String>,
        entries: &mut Vec<RuntimeObjectLiteralEntry<'_>>,
    ) -> Result<()> {
        if matches!(source, Value::Undefined | Value::Null) {
            return Ok(());
        }
        for key in self.own_enumerable_keys(source)? {
            let value = self.get_named(source, &key)?;
            let property_key = self.intern_property_key(&key)?;
            let name_index = dynamic_names.len();
            dynamic_names.push(key);
            entries.push(RuntimeObjectLiteralEntry {
                key: property_key,
                name: RuntimeObjectLiteralName::Dynamic(name_index),
                value,
                accessor: None,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum RuntimeObjectLiteralName<'a> {
    Static(&'a str),
    Dynamic(usize),
}

impl RuntimeObjectLiteralName<'_> {
    const fn is_dynamic(self) -> bool {
        matches!(self, Self::Dynamic(_))
    }
}

#[derive(Debug)]
struct RuntimeObjectLiteralEntry<'a> {
    key: PropertyKey,
    name: RuntimeObjectLiteralName<'a>,
    value: Value,
    accessor: Option<AccessorKind>,
}

fn object_literal_stack_value_count(properties: &[BytecodeObjectProperty]) -> Result<usize> {
    let mut count = 0_usize;
    for property in properties {
        count = count
            .checked_add(property.stack_value_count())
            .ok_or_else(|| Error::limit("object literal stack value count overflowed"))?;
    }
    Ok(count)
}

fn next_object_literal_stack_value(values: &mut impl Iterator<Item = Value>) -> Result<Value> {
    values
        .next()
        .ok_or_else(|| Error::runtime("bytecode object literal stack arity mismatch"))
}
