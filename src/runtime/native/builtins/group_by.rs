use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{IteratorSource, IteratorStep, same_value},
        call::RuntimeCallArgs,
        collections::canonicalize_keyed_collection_key,
        object::{
            DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyKey,
            PropertyUpdate, PropertyWritable,
        },
        property::DynamicPropertyKey,
        roots::VmRootKind,
        transient_roots::TransientRootScope,
    },
    value::Value,
};

const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
const GROUP_BY_STORAGE_ERROR: &str = "groupBy temporary group storage exhausted";
pub(super) const OBJECT_GROUP_BY_NAME: &str = "groupBy";

#[derive(Clone, Copy)]
pub(super) enum GroupByKeyCoercion {
    Collection,
    Property,
}

pub(super) enum GroupByKey {
    Collection(Value),
    Property { name: String, key: PropertyKey },
}

pub(super) struct KeyedGroup {
    pub(super) key: GroupByKey,
    pub(super) values: Vec<Value>,
}

impl GroupByKey {
    fn matches(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Collection(left), Self::Collection(right)) => same_value(left, right),
            (Self::Property { key: left, .. }, Self::Property { key: right, .. }) => left == right,
            (Self::Collection(_), Self::Property { .. })
            | (Self::Property { .. }, Self::Collection(_)) => false,
        }
    }
}

impl Context {
    pub(in crate::runtime::native) fn eval_object_group_by(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let roots = self.group_by_root_scope()?;
        let groups = self.collect_keyed_groups(
            args,
            GroupByKeyCoercion::Property,
            "Object.groupBy",
            &roots,
        )?;
        let result = self
            .objects
            .create_with_exact_prototype(None, self.limits.max_objects)?;
        roots.add_values(std::iter::once(&result))?;
        for group in groups {
            let GroupByKey::Property { name, key } = group.key else {
                return Err(Error::runtime("Object.groupBy produced a collection key"));
            };
            let values = self.create_array_from_elements(group.values)?;
            roots.add_values(std::iter::once(&values))?;
            let mut property = DynamicPropertyKey::new(name, Some(key));
            let defined = self.semantic_define_own_property_update(
                &result,
                &mut property,
                PropertyUpdate::Data(DataPropertyUpdate::new(
                    Some(values),
                    Some(PropertyWritable::Yes),
                    Some(PropertyEnumerable::Yes),
                    Some(PropertyConfigurable::Yes),
                )),
            )?;
            if !defined {
                return Err(Error::type_error(
                    "Object.groupBy could not define result property",
                ));
            }
        }
        Ok(result)
    }

    pub(super) fn collect_keyed_groups(
        &mut self,
        args: RuntimeCallArgs<'_>,
        coercion: GroupByKeyCoercion,
        builtin_name: &str,
        roots: &TransientRootScope,
    ) -> Result<Vec<KeyedGroup>> {
        let items = Self::argument_or_undefined(args.as_slice().first());
        let callback = Self::argument_or_undefined(args.as_slice().get(1));
        if !self.semantic_is_callable(&callback)? {
            return Err(Error::type_error(format!(
                "{builtin_name} callback must be callable"
            )));
        }
        roots.add_values([&items, &callback])?;
        let mut iterator = self.get_iterator(&items)?;
        let mut groups = Vec::new();
        let mut index = 0.0;
        loop {
            if index >= MAX_SAFE_INTEGER {
                let error = Error::type_error(format!(
                    "{builtin_name} iteration index exceeded safe integer range"
                ));
                return Err(self.iterator_close_on_error(&mut iterator, error));
            }
            let value = match self.iterator_step(&mut iterator)? {
                IteratorStep::Value(value) => value,
                IteratorStep::Done => break,
                IteratorStep::Abrupt(completion) => {
                    return completion.into_result().map(|_| groups);
                }
            };
            roots.add_values(std::iter::once(&value))?;
            let callback_args = [value.clone(), Value::Number(index)];
            let raw_key = match self.call_value(&callback, &callback_args, Value::Undefined) {
                Ok(key) => key,
                Err(error) => return Err(self.iterator_close_on_error(&mut iterator, error)),
            };
            roots.add_values(std::iter::once(&raw_key))?;
            let key = match self.group_by_key(raw_key, coercion) {
                Ok(key) => key,
                Err(error) => return Err(self.iterator_close_on_error(&mut iterator, error)),
            };
            if let GroupByKey::Collection(key) = &key {
                roots.add_values(std::iter::once(key))?;
            }
            self.add_value_to_keyed_group(&mut groups, key, value, &mut iterator)?;
            index += 1.0;
        }
        Ok(groups)
    }

    fn group_by_key(&mut self, value: Value, coercion: GroupByKeyCoercion) -> Result<GroupByKey> {
        match coercion {
            GroupByKeyCoercion::Collection => Ok(GroupByKey::Collection(
                canonicalize_keyed_collection_key(value),
            )),
            GroupByKeyCoercion::Property => {
                let mut property = self.to_property_key(&value)?;
                let key = self.intern_dynamic_property_key(&mut property)?;
                Ok(GroupByKey::Property {
                    name: property.name().to_owned(),
                    key,
                })
            }
        }
    }

    fn add_value_to_keyed_group(
        &mut self,
        groups: &mut Vec<KeyedGroup>,
        key: GroupByKey,
        value: Value,
        iterator: &mut IteratorSource,
    ) -> Result<()> {
        if let Some(group) = groups.iter_mut().find(|group| group.key.matches(&key)) {
            if let Err(error) = group.values.try_reserve(1) {
                let error = Error::limit(format!("{GROUP_BY_STORAGE_ERROR}: {error}"));
                return Err(self.iterator_close_on_error(iterator, error));
            }
            group.values.push(value);
            return Ok(());
        }
        if let Err(error) = groups.try_reserve(1) {
            let error = Error::limit(format!("{GROUP_BY_STORAGE_ERROR}: {error}"));
            return Err(self.iterator_close_on_error(iterator, error));
        }
        groups.push(KeyedGroup {
            key,
            values: vec![value],
        });
        Ok(())
    }

    pub(super) fn group_by_root_scope(&self) -> Result<TransientRootScope> {
        self.active_transient_root_scope(VmRootKind::TransientTemporary)
    }
}
