use crate::{
    error::{Error, Result},
    runtime::{
        Context, abstract_operations::IteratorStep, call::RuntimeCallArgs,
        collections::CollectionKind,
    },
    value::Value,
};

use super::{
    COLLECTION_ADD_NAME, COLLECTION_ADDER_ERROR, COLLECTION_SET_NAME, MAP_ENTRY_NOT_OBJECT_ERROR,
    NativeFunctionKind,
};

impl Context {
    pub(in crate::runtime::native) fn construct_collection_object(
        &mut self,
        kind: CollectionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.construct_collection_object_with_prototype(kind, args.as_slice(), None)
    }

    pub(in crate::runtime) fn construct_collection_object_with_new_target(
        &mut self,
        kind: CollectionKind,
        args: &[Value],
        new_target: &Value,
    ) -> Result<Value> {
        let constructor_kind = match kind {
            CollectionKind::Map => NativeFunctionKind::Map,
            CollectionKind::Set => NativeFunctionKind::Set,
            _ => {
                return Err(Error::runtime(
                    "weak collection routed to strong constructor",
                ));
            }
        };
        let prototype = self
            .constructor_instance_semantic_prototype_with_default(new_target, constructor_kind)?;
        self.construct_collection_object_with_prototype(kind, args, Some(prototype))
    }

    fn construct_collection_object_with_prototype(
        &mut self,
        kind: CollectionKind,
        args: &[Value],
        prototype: Option<Value>,
    ) -> Result<Value> {
        let constructor = self.collection_constructor_value(kind)?;
        let Value::NativeFunction(constructor_id) = &constructor else {
            return Err(Error::runtime("collection constructor disappeared"));
        };
        let prototype = if let Some(prototype) = prototype {
            prototype
        } else {
            self.native_function(*constructor_id)?
                .properties()
                .prototype()
        };
        let object = self
            .objects
            .create_with_semantic_prototype(Some(prototype), self.limits.max_objects)?;
        let Value::Object(object_id) = &object else {
            return Err(Error::runtime("collection object creation failed"));
        };
        let collection = self.create_collection(kind)?;
        self.bind_collection_object(*object_id, kind, collection)?;
        let iterable = args.first().cloned().unwrap_or(Value::Undefined);
        if !matches!(iterable, Value::Undefined | Value::Null) {
            self.seed_collection_from_iterable(kind, &object, &iterable)?;
        }
        Ok(object)
    }

    fn seed_collection_from_iterable(
        &mut self,
        kind: CollectionKind,
        object: &Value,
        iterable: &Value,
    ) -> Result<()> {
        let adder_name = match kind {
            CollectionKind::Map => COLLECTION_SET_NAME,
            CollectionKind::Set => COLLECTION_ADD_NAME,
            _ => {
                return Err(Error::runtime(
                    "weak collection routed to Map or Set seeding",
                ));
            }
        };
        let adder = self.get_named(object, adder_name)?;
        if !self.semantic_is_callable(&adder)? {
            return Err(Error::type_error(COLLECTION_ADDER_ERROR));
        }
        let mut source = self.get_iterator(iterable)?;
        loop {
            match self.iterator_step(&mut source)? {
                IteratorStep::Value(item) => {
                    let outcome = self.seed_collection_entry(kind, object, &adder, item);
                    if let Err(error) = outcome {
                        return Err(self.iterator_close_on_error(&mut source, error));
                    }
                }
                IteratorStep::Done => return Ok(()),
                IteratorStep::Abrupt(completion) => {
                    return completion.into_result().map(|_| ());
                }
            }
        }
    }

    fn seed_collection_entry(
        &mut self,
        kind: CollectionKind,
        object: &Value,
        adder: &Value,
        item: Value,
    ) -> Result<()> {
        let args = match kind {
            CollectionKind::Map => {
                if !matches!(item, Value::Object(_)) {
                    return Err(Error::type_error(MAP_ENTRY_NOT_OBJECT_ERROR));
                }
                let key = self.get_named(&item, "0")?;
                let value = self.get_named(&item, "1")?;
                vec![key, value]
            }
            CollectionKind::Set => vec![item],
            _ => {
                return Err(Error::runtime(
                    "weak collection routed to Map or Set seeding",
                ));
            }
        };
        self.call_value(adder, &args, object.clone()).map(|_| ())
    }
}
