use crate::{
    error::{Error, Result},
    runtime::{
        Context, abstract_operations::PreferredType, call::RuntimeCallArgs, object::DateValue,
    },
    value::{ErrorName, ObjectId, Value},
};

mod install;
mod setters;
mod support;

use crate::runtime::native::{DATE_NAME, DateFunctionKind, NativeFunctionKind};

use super::OBJECT_CONSTRUCTOR_PROPERTY;
use support::{
    DateComponent, DateParts, component_value, current_time_value, date_value_to_number,
    format_date_only_string, format_date_time_string, format_iso_string, format_time_only_string,
    format_utc_string, integer_component, integer_component_with_default, make_date_value,
    normalize_component_year, parse_date_string, time_clip,
};

const DATE_RECEIVER_ERROR: &str = "Date method requires a Date receiver";
const DATE_TO_PRIMITIVE_HINT_DEFAULT: &str = "default";
const DATE_TO_PRIMITIVE_HINT_NUMBER: &str = "number";
const DATE_TO_PRIMITIVE_HINT_STRING: &str = "string";
const DATE_TO_PRIMITIVE_INVALID_HINT_ERROR: &str =
    "Date @@toPrimitive hint must be 'default', 'string', or 'number'";
const DATE_YEAR_OFFSET: i64 = 1_900;

impl Context {
    pub(in crate::runtime::native) fn date_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) =
            self.native_function_id(NativeFunctionKind::Date(DateFunctionKind::Constructor))
        {
            return Ok(Value::NativeFunction(id));
        }

        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype_id = self.date_prototype_id_with_constructor(constructor.clone())?;
        let prototype = Value::Object(prototype_id);
        let kind = NativeFunctionKind::Date(DateFunctionKind::Constructor);
        let name = self.native_function_name_value(kind)?;
        self.push_native_function_with_id(id, kind, prototype, name)?;
        self.install_date_static_methods(id)?;
        self.install_date_prototype_methods(prototype_id)?;
        self.insert_global_builtin(DATE_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime::native) fn eval_date_constructor(
        &mut self,
        _args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_date_constructor(&[])
    }

    pub(in crate::runtime::native) fn eval_direct_date_constructor(
        &mut self,
        _args: &[Value],
    ) -> Result<Value> {
        let now = current_time_value()?;
        let text = Self::format_date_string_value(now)?;
        self.heap_string_value(&text)
    }

    pub(in crate::runtime::native) fn construct_date_object(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let value = self.date_value_from_constructor_args(args.as_slice())?;
        let prototype = self.date_constructor_prototype()?;
        self.objects
            .create_date_object(value, prototype, self.limits.max_objects)
    }

    pub(in crate::runtime) fn eval_date_native_function_kind(
        &mut self,
        kind: DateFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if let Some(result) = self.eval_date_constructor_or_static_kind(kind, args) {
            return result;
        }
        if let Some(result) = self.eval_date_getter_kind(kind, this_value) {
            return result;
        }
        if let Some(result) = self.eval_date_setter_kind(kind, args, this_value) {
            return result;
        }
        if let Some(result) = self.eval_date_string_kind(kind, args, this_value) {
            return result;
        }
        Err(Error::runtime("Date native function kind was not handled"))
    }

    fn eval_date_constructor_or_static_kind(
        &mut self,
        kind: DateFunctionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Option<Result<Value>> {
        match kind {
            DateFunctionKind::Constructor => Some(self.eval_date_constructor(args)),
            DateFunctionKind::Now => Some(Self::eval_date_now(args)),
            DateFunctionKind::Parse => Some(self.eval_date_parse(args)),
            DateFunctionKind::Utc => Some(self.eval_date_utc(args)),
            _ => None,
        }
    }

    fn eval_date_getter_kind(
        &self,
        kind: DateFunctionKind,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            DateFunctionKind::PrototypeGetDate => {
                Some(self.eval_date_prototype_get_date(this_value))
            }
            DateFunctionKind::PrototypeGetDay => Some(self.eval_date_prototype_get_day(this_value)),
            DateFunctionKind::PrototypeGetFullYear => {
                Some(self.eval_date_prototype_get_full_year(this_value))
            }
            DateFunctionKind::PrototypeGetYear => {
                Some(self.eval_date_prototype_get_year(this_value))
            }
            DateFunctionKind::PrototypeGetHours => {
                Some(self.eval_date_prototype_get_hours(this_value))
            }
            DateFunctionKind::PrototypeGetMilliseconds => {
                Some(self.eval_date_prototype_get_milliseconds(this_value))
            }
            DateFunctionKind::PrototypeGetMinutes => {
                Some(self.eval_date_prototype_get_minutes(this_value))
            }
            DateFunctionKind::PrototypeGetMonth => {
                Some(self.eval_date_prototype_get_month(this_value))
            }
            DateFunctionKind::PrototypeGetSeconds => {
                Some(self.eval_date_prototype_get_seconds(this_value))
            }
            DateFunctionKind::PrototypeGetTime => {
                Some(self.eval_date_prototype_get_time(this_value))
            }
            DateFunctionKind::PrototypeGetTimezoneOffset => {
                Some(self.eval_date_prototype_get_timezone_offset(this_value))
            }
            DateFunctionKind::PrototypeGetUtcDate => {
                Some(self.eval_date_prototype_get_utc_date(this_value))
            }
            DateFunctionKind::PrototypeGetUtcDay => {
                Some(self.eval_date_prototype_get_utc_day(this_value))
            }
            DateFunctionKind::PrototypeGetUtcFullYear => {
                Some(self.eval_date_prototype_get_utc_full_year(this_value))
            }
            DateFunctionKind::PrototypeGetUtcHours => {
                Some(self.eval_date_prototype_get_utc_hours(this_value))
            }
            DateFunctionKind::PrototypeGetUtcMilliseconds => {
                Some(self.eval_date_prototype_get_utc_milliseconds(this_value))
            }
            DateFunctionKind::PrototypeGetUtcMinutes => {
                Some(self.eval_date_prototype_get_utc_minutes(this_value))
            }
            DateFunctionKind::PrototypeGetUtcMonth => {
                Some(self.eval_date_prototype_get_utc_month(this_value))
            }
            DateFunctionKind::PrototypeGetUtcSeconds => {
                Some(self.eval_date_prototype_get_utc_seconds(this_value))
            }
            DateFunctionKind::PrototypeValueOf => {
                Some(self.eval_date_prototype_value_of(this_value))
            }
            _ => None,
        }
    }

    fn eval_date_setter_kind(
        &mut self,
        kind: DateFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            DateFunctionKind::PrototypeSetDate | DateFunctionKind::PrototypeSetUtcDate => {
                Some(self.eval_date_prototype_set_date(args, this_value))
            }
            DateFunctionKind::PrototypeSetFullYear | DateFunctionKind::PrototypeSetUtcFullYear => {
                Some(self.eval_date_prototype_set_full_year(args, this_value))
            }
            DateFunctionKind::PrototypeSetYear => {
                Some(self.eval_date_prototype_set_year(args, this_value))
            }
            DateFunctionKind::PrototypeSetHours | DateFunctionKind::PrototypeSetUtcHours => {
                Some(self.eval_date_prototype_set_hours(args, this_value))
            }
            DateFunctionKind::PrototypeSetMilliseconds
            | DateFunctionKind::PrototypeSetUtcMilliseconds => {
                Some(self.eval_date_prototype_set_milliseconds(args, this_value))
            }
            DateFunctionKind::PrototypeSetMinutes | DateFunctionKind::PrototypeSetUtcMinutes => {
                Some(self.eval_date_prototype_set_minutes(args, this_value))
            }
            DateFunctionKind::PrototypeSetMonth | DateFunctionKind::PrototypeSetUtcMonth => {
                Some(self.eval_date_prototype_set_month(args, this_value))
            }
            DateFunctionKind::PrototypeSetSeconds | DateFunctionKind::PrototypeSetUtcSeconds => {
                Some(self.eval_date_prototype_set_seconds(args, this_value))
            }
            DateFunctionKind::PrototypeSetTime => {
                Some(self.eval_date_prototype_set_time(args, this_value))
            }
            _ => None,
        }
    }

    fn eval_date_string_kind(
        &mut self,
        kind: DateFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            DateFunctionKind::PrototypeSymbolToPrimitive => {
                Some(self.eval_date_prototype_symbol_to_primitive(args, this_value))
            }
            DateFunctionKind::PrototypeToDateString => {
                Some(self.eval_date_prototype_to_date_string(this_value))
            }
            DateFunctionKind::PrototypeToIsoString => {
                Some(self.eval_date_prototype_to_iso_string(this_value))
            }
            DateFunctionKind::PrototypeToJson => Some(self.eval_date_prototype_to_json(this_value)),
            DateFunctionKind::PrototypeToLocaleDateString => {
                Some(self.eval_date_prototype_to_locale_date_string(this_value))
            }
            DateFunctionKind::PrototypeToLocaleString => {
                Some(self.eval_date_prototype_to_locale_string(this_value))
            }
            DateFunctionKind::PrototypeToLocaleTimeString => {
                Some(self.eval_date_prototype_to_locale_time_string(this_value))
            }
            DateFunctionKind::PrototypeToString => {
                Some(self.eval_date_prototype_to_string(this_value))
            }
            DateFunctionKind::PrototypeToTimeString => {
                Some(self.eval_date_prototype_to_time_string(this_value))
            }
            DateFunctionKind::PrototypeToUtcString => {
                Some(self.eval_date_prototype_to_utc_string(this_value))
            }
            _ => None,
        }
    }

    pub(in crate::runtime::native) fn eval_date_now(_args: RuntimeCallArgs<'_>) -> Result<Value> {
        date_value_to_number(current_time_value()?).map(Value::Number)
    }

    pub(in crate::runtime::native) fn eval_date_parse(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let text = match args.as_slice().first() {
            Some(value) => self.to_string(value)?,
            None => self.to_string(&Value::Undefined)?,
        };
        let value = parse_date_string(&text)?;
        date_value_to_number(value).map(Value::Number)
    }

    pub(in crate::runtime::native) fn eval_date_utc(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let value = self.date_value_from_components(args.as_slice())?;
        date_value_to_number(value).map(Value::Number)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_time(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.date_this_number(this_value).map(Value::Number)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_value_of(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.date_this_number(this_value).map(Value::Number)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_utc_full_year(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.date_component(this_value, DateComponent::FullYear)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_utc_month(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.date_component(this_value, DateComponent::Month)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_utc_date(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.date_component(this_value, DateComponent::Date)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_utc_day(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.date_component(this_value, DateComponent::Day)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_utc_hours(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.date_component(this_value, DateComponent::Hours)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_utc_minutes(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.date_component(this_value, DateComponent::Minutes)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_utc_seconds(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.date_component(this_value, DateComponent::Seconds)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_utc_milliseconds(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.date_component(this_value, DateComponent::Milliseconds)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_full_year(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_get_utc_full_year(this_value)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_year(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        let value = self.date_this_value(this_value)?;
        let Some(ms) = value.millis() else {
            return Ok(Value::Number(f64::NAN));
        };
        let parts = DateParts::from_millis(ms)?;
        let year = parts
            .year
            .checked_sub(DATE_YEAR_OFFSET)
            .ok_or_else(|| Error::runtime("Date getYear result overflowed"))?;
        support::integer_to_number(year).map(Value::Number)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_month(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_get_utc_month(this_value)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_date(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_get_utc_date(this_value)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_day(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_get_utc_day(this_value)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_hours(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_get_utc_hours(this_value)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_minutes(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_get_utc_minutes(this_value)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_seconds(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_get_utc_seconds(this_value)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_milliseconds(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_get_utc_milliseconds(this_value)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_get_timezone_offset(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        let value = self.date_this_value(this_value)?;
        if value.millis().is_none() {
            return Ok(Value::Number(f64::NAN));
        }
        Ok(Value::Number(0.0))
    }

    pub(in crate::runtime::native) fn eval_date_prototype_set_time(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let (id, _) = self.date_this_object_value(this_value)?;
        let value = match args.as_slice().first() {
            Some(value) => self.to_number(value)?,
            None => f64::NAN,
        };
        let date = time_clip(value)?;
        self.objects.set_date_value(id, date)?;
        date_value_to_number(date).map(Value::Number)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_symbol_to_primitive(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let Some(hint) = args.as_slice().first() else {
            return Err(Error::type_error(DATE_TO_PRIMITIVE_INVALID_HINT_ERROR));
        };
        let hint = Self::date_to_primitive_hint(hint)?;
        match hint {
            DATE_TO_PRIMITIVE_HINT_DEFAULT | DATE_TO_PRIMITIVE_HINT_STRING => {
                self.ordinary_to_primitive(this_value, PreferredType::String)
            }
            DATE_TO_PRIMITIVE_HINT_NUMBER => {
                self.ordinary_to_primitive(this_value, PreferredType::Number)
            }
            _ => Err(Error::type_error(DATE_TO_PRIMITIVE_INVALID_HINT_ERROR)),
        }
    }

    pub(in crate::runtime::native) fn eval_date_prototype_to_iso_string(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let value = self.date_this_value(this_value)?;
        let Some(ms) = value.millis() else {
            return Err(Error::exception(
                ErrorName::RangeError,
                "invalid Date cannot be formatted as ISO string",
            ));
        };
        let text = format_iso_string(ms)?;
        self.heap_string_value(&text)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_to_json(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let value = self.date_this_value(this_value)?;
        if value.millis().is_none() {
            return Ok(Value::Null);
        }
        self.eval_date_prototype_to_iso_string(this_value)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_to_locale_string(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_to_string(this_value)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_to_locale_date_string(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_to_date_string(this_value)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_to_locale_time_string(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_to_time_string(this_value)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_to_string(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let value = self.date_this_value(this_value)?;
        let text = Self::format_date_string_value(value)?;
        self.heap_string_value(&text)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_to_utc_string(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let value = self.date_this_value(this_value)?;
        let text = format_utc_string(value)?;
        self.heap_string_value(&text)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_to_date_string(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let value = self.date_this_value(this_value)?;
        let text = format_date_only_string(value)?;
        self.heap_string_value(&text)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_to_time_string(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let value = self.date_this_value(this_value)?;
        let text = format_time_only_string(value)?;
        self.heap_string_value(&text)
    }

    fn date_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<ObjectId> {
        let constructor_key = self.object_constructor_property_key()?;
        let object_prototype = self.objects.object_prototype_id(
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(prototype) = self.objects.create_date_object(
            DateValue::Invalid,
            object_prototype,
            self.limits.max_objects,
        )?
        else {
            return Err(Error::runtime("Date prototype is not an object"));
        };
        self.define_non_enumerable_object_property(
            prototype,
            OBJECT_CONSTRUCTOR_PROPERTY,
            constructor,
        )?;
        Ok(prototype)
    }

    fn date_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.date_constructor_value()? else {
            return Err(Error::runtime("Date constructor value is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("Date prototype is not an object")),
        }
    }

    fn date_to_primitive_hint(value: &Value) -> Result<&str> {
        let Some(hint) = value.string_text() else {
            return Err(Error::type_error(DATE_TO_PRIMITIVE_INVALID_HINT_ERROR));
        };
        match hint {
            DATE_TO_PRIMITIVE_HINT_DEFAULT
            | DATE_TO_PRIMITIVE_HINT_NUMBER
            | DATE_TO_PRIMITIVE_HINT_STRING => Ok(hint),
            _ => Err(Error::type_error(DATE_TO_PRIMITIVE_INVALID_HINT_ERROR)),
        }
    }

    fn date_value_from_constructor_args(&mut self, args: &[Value]) -> Result<DateValue> {
        match args.len() {
            0 => current_time_value(),
            1 => self.date_value_from_single_argument(args.first()),
            _ => self.date_value_from_components(args),
        }
    }

    fn date_value_from_single_argument(&mut self, value: Option<&Value>) -> Result<DateValue> {
        let Some(value) = value else {
            return current_time_value();
        };
        // A Date argument copies its time value directly.
        if let Value::Object(id) = value
            && let Some(existing) = self.objects.date_value(*id)?
        {
            return Ok(existing);
        }
        let primitive = self.to_primitive(value, PreferredType::Default)?;
        if let Some(text) = primitive.string_text() {
            return parse_date_string(text);
        }
        time_clip(self.to_number(&primitive)?)
    }

    fn date_value_from_components(&mut self, args: &[Value]) -> Result<DateValue> {
        let Some(year) = integer_component(self, args.first())? else {
            return Ok(DateValue::Invalid);
        };
        let Some(month) = integer_component(self, args.get(1))? else {
            return Ok(DateValue::Invalid);
        };
        let Some(date) = integer_component_with_default(self, args.get(2), 1)? else {
            return Ok(DateValue::Invalid);
        };
        let Some(hour) = integer_component_with_default(self, args.get(3), 0)? else {
            return Ok(DateValue::Invalid);
        };
        let Some(minute) = integer_component_with_default(self, args.get(4), 0)? else {
            return Ok(DateValue::Invalid);
        };
        let Some(second) = integer_component_with_default(self, args.get(5), 0)? else {
            return Ok(DateValue::Invalid);
        };
        let Some(millisecond) = integer_component_with_default(self, args.get(6), 0)? else {
            return Ok(DateValue::Invalid);
        };
        Ok(make_date_value(
            normalize_component_year(year),
            month,
            date,
            hour,
            minute,
            second,
            millisecond,
        ))
    }

    fn date_this_value(&self, this_value: &Value) -> Result<DateValue> {
        self.date_this_object_value(this_value)
            .map(|(_, value)| value)
    }

    fn date_this_object_value(&self, this_value: &Value) -> Result<(ObjectId, DateValue)> {
        let Value::Object(id) = this_value else {
            return Err(Error::type_error(DATE_RECEIVER_ERROR));
        };
        let value = self
            .objects
            .date_value(*id)?
            .ok_or_else(|| Error::type_error(DATE_RECEIVER_ERROR))?;
        Ok((*id, value))
    }

    fn date_this_number(&self, this_value: &Value) -> Result<f64> {
        date_value_to_number(self.date_this_value(this_value)?)
    }

    fn date_component(&self, this_value: &Value, component: DateComponent) -> Result<Value> {
        let value = self.date_this_value(this_value)?;
        let Some(ms) = value.millis() else {
            return Ok(Value::Number(f64::NAN));
        };
        let parts = DateParts::from_millis(ms)?;
        component_value(parts, component).map(Value::Number)
    }

    fn format_date_string_value(value: DateValue) -> Result<String> {
        format_date_time_string(value)
    }
}
