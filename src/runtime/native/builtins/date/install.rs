use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        object::{PropertyEnumerable, PropertyKey},
    },
    value::{NativeFunctionId, ObjectId, Value},
};

use crate::runtime::native::{
    DATE_NOW_NAME, DATE_PARSE_NAME, DATE_UTC_NAME, DateFunctionKind, NativeFunctionKind,
};

const DATE_PROTOTYPE_TO_GMT_STRING_NAME: &str = "toGMTString";
const SYMBOL_TO_PRIMITIVE_PROPERTY: &str = "toPrimitive";

impl Context {
    pub(super) fn install_date_static_methods(
        &mut self,
        constructor: NativeFunctionId,
    ) -> Result<()> {
        self.define_date_static_method(
            constructor,
            DATE_NOW_NAME,
            NativeFunctionKind::Date(DateFunctionKind::Now),
        )?;
        self.define_date_static_method(
            constructor,
            DATE_PARSE_NAME,
            NativeFunctionKind::Date(DateFunctionKind::Parse),
        )?;
        self.define_date_static_method(
            constructor,
            DATE_UTC_NAME,
            NativeFunctionKind::Date(DateFunctionKind::Utc),
        )
    }

    fn define_date_static_method(
        &mut self,
        constructor: NativeFunctionId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let function = self.create_native_function(kind, Value::Undefined)?;
        let key = self.intern_property_key(name)?;
        self.native_function_mut(constructor)?
            .properties_mut()
            .define_builtin(key, function, PropertyEnumerable::No);
        Ok(())
    }

    pub(super) fn install_date_prototype_methods(&mut self, prototype: ObjectId) -> Result<()> {
        for kind in DATE_PROTOTYPE_METHODS {
            self.define_date_prototype_method(prototype, NativeFunctionKind::Date(*kind))?;
        }
        self.define_date_prototype_to_gmt_string_alias(prototype)?;
        self.define_date_prototype_symbol_to_primitive(prototype)?;
        Ok(())
    }

    fn define_date_prototype_method(
        &mut self,
        prototype: ObjectId,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let function = self.create_ephemeral_native_function(kind, Value::Undefined)?;
        self.define_non_enumerable_object_property(prototype, kind.name(), function)
    }

    fn define_date_prototype_to_gmt_string_alias(&mut self, prototype: ObjectId) -> Result<()> {
        let function = self.get_named(
            &Value::Object(prototype),
            DateFunctionKind::PrototypeToUtcString.name(),
        )?;
        self.define_non_enumerable_object_property(
            prototype,
            DATE_PROTOTYPE_TO_GMT_STRING_NAME,
            function,
        )
    }

    fn define_date_prototype_symbol_to_primitive(&mut self, prototype: ObjectId) -> Result<()> {
        let key = self.date_well_known_symbol_property_key(SYMBOL_TO_PRIMITIVE_PROPERTY)?;
        let kind = NativeFunctionKind::Date(DateFunctionKind::PrototypeSymbolToPrimitive);
        let function = self.create_ephemeral_native_function(kind, Value::Undefined)?;
        self.objects.define_non_enumerable(
            prototype,
            key,
            kind.name(),
            function,
            self.limits.max_object_properties,
        )
    }

    fn date_well_known_symbol_property_key(&mut self, property: &str) -> Result<PropertyKey> {
        let constructor = self.symbol_constructor_value()?;
        let value = self.get_named(&constructor, property)?;
        let Value::Symbol(symbol) = value else {
            return Err(Error::runtime("well-known Symbol property is not a symbol"));
        };
        Ok(PropertyKey::symbol(symbol.id()))
    }
}

const DATE_PROTOTYPE_METHODS: &[DateFunctionKind] = &[
    DateFunctionKind::PrototypeGetTime,
    DateFunctionKind::PrototypeValueOf,
    DateFunctionKind::PrototypeGetUtcFullYear,
    DateFunctionKind::PrototypeGetUtcMonth,
    DateFunctionKind::PrototypeGetUtcDate,
    DateFunctionKind::PrototypeGetUtcDay,
    DateFunctionKind::PrototypeGetUtcHours,
    DateFunctionKind::PrototypeGetUtcMinutes,
    DateFunctionKind::PrototypeGetUtcSeconds,
    DateFunctionKind::PrototypeGetUtcMilliseconds,
    DateFunctionKind::PrototypeGetFullYear,
    DateFunctionKind::PrototypeGetYear,
    DateFunctionKind::PrototypeGetMonth,
    DateFunctionKind::PrototypeGetDate,
    DateFunctionKind::PrototypeGetDay,
    DateFunctionKind::PrototypeGetHours,
    DateFunctionKind::PrototypeGetMinutes,
    DateFunctionKind::PrototypeGetSeconds,
    DateFunctionKind::PrototypeGetMilliseconds,
    DateFunctionKind::PrototypeGetTimezoneOffset,
    DateFunctionKind::PrototypeSetFullYear,
    DateFunctionKind::PrototypeSetYear,
    DateFunctionKind::PrototypeSetUtcFullYear,
    DateFunctionKind::PrototypeSetMonth,
    DateFunctionKind::PrototypeSetUtcMonth,
    DateFunctionKind::PrototypeSetDate,
    DateFunctionKind::PrototypeSetUtcDate,
    DateFunctionKind::PrototypeSetHours,
    DateFunctionKind::PrototypeSetUtcHours,
    DateFunctionKind::PrototypeSetMinutes,
    DateFunctionKind::PrototypeSetUtcMinutes,
    DateFunctionKind::PrototypeSetSeconds,
    DateFunctionKind::PrototypeSetUtcSeconds,
    DateFunctionKind::PrototypeSetMilliseconds,
    DateFunctionKind::PrototypeSetUtcMilliseconds,
    DateFunctionKind::PrototypeSetTime,
    DateFunctionKind::PrototypeToIsoString,
    DateFunctionKind::PrototypeToJson,
    DateFunctionKind::PrototypeToLocaleString,
    DateFunctionKind::PrototypeToLocaleDateString,
    DateFunctionKind::PrototypeToLocaleTimeString,
    DateFunctionKind::PrototypeToString,
    DateFunctionKind::PrototypeToUtcString,
    DateFunctionKind::PrototypeToDateString,
    DateFunctionKind::PrototypeToTimeString,
];
