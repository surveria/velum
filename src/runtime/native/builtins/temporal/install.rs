use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        native::TemporalFunctionKind,
        object::{
            AccessorPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyUpdate,
        },
    },
    value::{NativeFunctionId, ObjectId, Value},
};

use super::temporal_kind;

const DURATION_TAG: &str = "Temporal.Duration";

const DURATION_ACCESSORS: &[(&str, TemporalFunctionKind)] = &[
    ("years", TemporalFunctionKind::PrototypeYears),
    ("months", TemporalFunctionKind::PrototypeMonths),
    ("weeks", TemporalFunctionKind::PrototypeWeeks),
    ("days", TemporalFunctionKind::PrototypeDays),
    ("hours", TemporalFunctionKind::PrototypeHours),
    ("minutes", TemporalFunctionKind::PrototypeMinutes),
    ("seconds", TemporalFunctionKind::PrototypeSeconds),
    ("milliseconds", TemporalFunctionKind::PrototypeMilliseconds),
    ("microseconds", TemporalFunctionKind::PrototypeMicroseconds),
    ("nanoseconds", TemporalFunctionKind::PrototypeNanoseconds),
    ("sign", TemporalFunctionKind::PrototypeSign),
    ("blank", TemporalFunctionKind::PrototypeBlank),
];

const DURATION_METHODS: &[(&str, TemporalFunctionKind)] = &[
    ("with", TemporalFunctionKind::PrototypeWith),
    ("negated", TemporalFunctionKind::PrototypeNegated),
    ("abs", TemporalFunctionKind::PrototypeAbs),
    ("add", TemporalFunctionKind::PrototypeAdd),
    ("subtract", TemporalFunctionKind::PrototypeSubtract),
    ("round", TemporalFunctionKind::PrototypeRound),
    ("total", TemporalFunctionKind::PrototypeTotal),
    ("toString", TemporalFunctionKind::PrototypeToString),
    ("toJSON", TemporalFunctionKind::PrototypeToJson),
    (
        "toLocaleString",
        TemporalFunctionKind::PrototypeToLocaleString,
    ),
    ("valueOf", TemporalFunctionKind::PrototypeValueOf),
];

impl Context {
    pub(super) fn temporal_duration_constructor_value(&mut self) -> Result<Value> {
        let kind = temporal_kind(TemporalFunctionKind::Constructor);
        if let Some(id) = self.native_function_id(kind) {
            return Ok(Value::NativeFunction(id));
        }

        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype = self.temporal_duration_prototype(constructor.clone())?;
        let name = self.native_function_name_value(kind)?;
        self.push_native_function_with_id(id, kind, Value::Object(prototype), name)?;
        self.install_duration_static_methods(id)?;
        self.install_duration_prototype(prototype)?;
        Ok(constructor)
    }

    fn temporal_duration_prototype(&mut self, constructor: Value) -> Result<ObjectId> {
        let constructor_key = self.object_constructor_property_key()?;
        let object_prototype = self.objects.object_prototype_id(
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let prototype = self.objects.create_with_prototype_id(
            Some(object_prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        self.define_non_enumerable_object_property(prototype, "constructor", constructor)?;
        Ok(prototype)
    }

    fn install_duration_static_methods(&mut self, constructor: NativeFunctionId) -> Result<()> {
        for (name, kind) in [
            ("from", TemporalFunctionKind::From),
            ("compare", TemporalFunctionKind::Compare),
        ] {
            let function = self.create_native_function(temporal_kind(kind), Value::Undefined)?;
            let key = self.intern_property_key(name)?;
            self.native_function_mut(constructor)?
                .properties_mut()
                .define_builtin(key, function, PropertyEnumerable::No)?;
        }
        Ok(())
    }

    fn install_duration_prototype(&mut self, prototype: ObjectId) -> Result<()> {
        for (name, kind) in DURATION_ACCESSORS {
            let getter = self.create_native_function(temporal_kind(*kind), Value::Undefined)?;
            let key = self.intern_property_key(name)?;
            self.objects.define_property(
                prototype,
                key,
                name,
                PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                    Some(getter),
                    None,
                    Some(PropertyEnumerable::No),
                    Some(PropertyConfigurable::Yes),
                )),
                self.limits.max_object_properties,
            )?;
        }
        for (name, kind) in DURATION_METHODS {
            let method = self.create_native_function(temporal_kind(*kind), Value::Undefined)?;
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        self.define_temporal_to_string_tag(prototype, DURATION_TAG)
    }

    pub(super) fn temporal_duration_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.temporal_duration_constructor_value()? else {
            return Err(Error::runtime(
                "Temporal.Duration constructor is not native",
            ));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime(
                "Temporal.Duration prototype is not an object",
            )),
        }
    }
}
