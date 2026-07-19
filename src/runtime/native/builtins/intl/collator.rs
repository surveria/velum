#[cfg(not(feature = "std"))]
use crate::prelude::*;

use core::cmp::Ordering;

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::to_boolean,
        call::RuntimeCallArgs,
        native::IntlFunctionKind,
        object::{
            AccessorPropertyUpdate, CollatorValue, DataPropertyUpdate, IntlValue,
            PropertyConfigurable, PropertyEnumerable, PropertyUpdate, PropertyWritable,
        },
    },
    value::{ErrorName, NativeFunctionId, ObjectId, Value},
};

const COLLATOR_TAG: &str = "Intl.Collator";
const SUPPORTED_LOCALES_OF: &str = "supportedLocalesOf";
const DEFAULT_LOCALE: &str = "en-US";

#[derive(Clone)]
struct CollationKey {
    primary: String,
    accent: String,
    case: String,
    variant: String,
}

impl Context {
    pub(in crate::runtime) fn intl_collator_constructor_value(&mut self) -> Result<Value> {
        let constructor_kind = IntlFunctionKind::CollatorConstructor;
        let native_kind = super::intl_kind(constructor_kind);
        let existed = self.native_function_id(native_kind).is_some();
        let constructor = self.intl_constructor_value(
            constructor_kind,
            COLLATOR_TAG,
            &[("resolvedOptions", IntlFunctionKind::CollatorResolvedOptions)],
        )?;
        if existed {
            return Ok(constructor);
        }
        let Value::NativeFunction(constructor_id) = constructor else {
            return Err(Error::runtime("Intl.Collator constructor is not native"));
        };
        let prototype = self.collator_prototype_id(constructor_id)?;
        self.install_collator_compare_accessor(prototype)?;
        self.install_collator_static_method(constructor_id)?;
        Ok(Value::NativeFunction(constructor_id))
    }

    pub(in crate::runtime::native) fn construct_intl_collator(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let requested = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let locales = self.intl_locale_list(&requested)?;
        let source = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if matches!(source, Value::Null) {
            return Err(Error::type_error("Intl.Collator options cannot be null"));
        }
        let options = if matches!(source, Value::Undefined) {
            Value::Undefined
        } else {
            self.object_to_object(&source)?
        };
        let usage = self.collator_string_option(&options, "usage", &["sort", "search"], "sort")?;
        let _locale_matcher = self.collator_string_option(
            &options,
            "localeMatcher",
            &["lookup", "best fit"],
            "best fit",
        )?;
        let collation_option = self.collator_optional_string(&options, "collation")?;
        if collation_option
            .as_deref()
            .is_some_and(|value| !is_unicode_type(value))
        {
            return Err(collator_range_error("collation has an invalid value"));
        }
        let numeric_option = self.collator_optional_boolean(&options, "numeric")?;
        let case_first_option = self.collator_optional_string(&options, "caseFirst")?;
        if case_first_option
            .as_deref()
            .is_some_and(|value| !matches!(value, "upper" | "lower" | "false"))
        {
            return Err(collator_range_error("caseFirst has an unsupported value"));
        }
        let sensitivity = self.collator_string_option(
            &options,
            "sensitivity",
            &["base", "accent", "case", "variant"],
            "variant",
        )?;
        let ignore_option = self.collator_optional_boolean(&options, "ignorePunctuation")?;

        let requested_locale = locales
            .into_iter()
            .find(|locale| !locale.eq_ignore_ascii_case("zxx"))
            .unwrap_or_else(|| DEFAULT_LOCALE.to_owned());
        let resolved = resolve_collator_locale(
            &requested_locale,
            collation_option.as_deref(),
            numeric_option,
            case_first_option.as_deref(),
        );
        let ignore_punctuation = ignore_option.unwrap_or_else(|| {
            resolved
                .locale
                .split('-')
                .next()
                .is_some_and(|language| language.eq_ignore_ascii_case("th"))
        });
        let prototype = self.intl_constructor_prototype(IntlFunctionKind::CollatorConstructor)?;
        self.objects.create_intl_object(
            IntlValue::Collator(Box::new(CollatorValue {
                locale: resolved.locale,
                usage,
                sensitivity,
                ignore_punctuation,
                collation: resolved.collation,
                numeric: resolved.numeric,
                case_first: resolved.case_first,
                bound_compare: None,
            })),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(super) fn eval_intl_collator_compare_getter(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let id = self.collator_receiver_id(this_value)?;
        let cached = match self.objects.intl_value(id)? {
            Some(IntlValue::Collator(value)) => value.bound_compare.clone(),
            _ => return Err(Error::type_error("Intl.Collator receiver is invalid")),
        };
        if let Some(cached) = cached {
            return Ok(cached);
        }
        let bound = self.create_ephemeral_native_function(
            super::intl_kind(IntlFunctionKind::CollatorBoundCompare(id)),
            Value::Undefined,
        )?;
        let Some(IntlValue::Collator(value)) = self.objects.intl_value_mut(id)? else {
            return Err(Error::runtime("Intl.Collator receiver disappeared"));
        };
        value.bound_compare = Some(bound.clone());
        Ok(bound)
    }

    pub(in crate::runtime::native) fn eval_intl_collator_compare(
        &mut self,
        args: RuntimeCallArgs<'_>,
        collator: ObjectId,
    ) -> Result<Value> {
        let formatter = match self.objects.intl_value(collator)? {
            Some(IntlValue::Collator(value)) => value.as_ref().clone(),
            _ => return Err(Error::type_error("Intl.Collator receiver is invalid")),
        };
        let left = self.to_string(args.as_slice().first().unwrap_or(&Value::Undefined))?;
        let right = self.to_string(args.as_slice().get(1).unwrap_or(&Value::Undefined))?;
        let ordering = compare_collator_strings(&formatter, &left, &right);
        Ok(Value::Number(match ordering {
            Ordering::Less => -1.0,
            Ordering::Equal => 0.0,
            Ordering::Greater => 1.0,
        }))
    }

    pub(super) fn eval_intl_collator_resolved_options(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let id = self.collator_receiver_id(this_value)?;
        let formatter = match self.objects.intl_value(id)? {
            Some(IntlValue::Collator(value)) => value.as_ref().clone(),
            _ => return Err(Error::type_error("Intl.Collator receiver is invalid")),
        };
        let locale = self.heap_string_value(&formatter.locale)?;
        let usage = self.heap_string_value(&formatter.usage)?;
        let sensitivity = self.heap_string_value(&formatter.sensitivity)?;
        let collation = self.heap_string_value(&formatter.collation)?;
        let case_first = self.heap_string_value(&formatter.case_first)?;
        self.create_intl_data_object(vec![
            ("locale", locale),
            ("usage", usage),
            ("sensitivity", sensitivity),
            (
                "ignorePunctuation",
                Value::Bool(formatter.ignore_punctuation),
            ),
            ("collation", collation),
            ("numeric", Value::Bool(formatter.numeric)),
            ("caseFirst", case_first),
        ])
    }

    pub(super) fn eval_intl_collator_supported_locales(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let requested = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let locales = self.intl_locale_list(&requested)?;
        let source = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if matches!(source, Value::Null) {
            return Err(Error::type_error("Intl locale options cannot be null"));
        }
        let options = if matches!(source, Value::Undefined) {
            Value::Undefined
        } else {
            self.object_to_object(&source)?
        };
        let _matcher = self.collator_string_option(
            &options,
            "localeMatcher",
            &["lookup", "best fit"],
            "best fit",
        )?;
        let mut supported = Vec::new();
        for locale in locales {
            if !locale.eq_ignore_ascii_case("zxx") {
                supported.push(self.heap_string_value(&locale)?);
            }
        }
        self.create_array_from_elements(supported)
    }

    fn collator_receiver_id(&self, this_value: &Value) -> Result<ObjectId> {
        let Value::Object(id) = this_value else {
            return Err(Error::type_error("Intl.Collator receiver is invalid"));
        };
        if !matches!(self.objects.intl_value(*id)?, Some(IntlValue::Collator(_))) {
            return Err(Error::type_error("Intl.Collator receiver is invalid"));
        }
        Ok(*id)
    }

    fn collator_string_option(
        &mut self,
        options: &Value,
        name: &str,
        allowed: &[&str],
        default: &str,
    ) -> Result<String> {
        let Some(value) = self.collator_optional_string(options, name)? else {
            return Ok(default.to_owned());
        };
        if !allowed.contains(&value.as_str()) {
            return Err(collator_range_error(&format!(
                "{name} has an unsupported value"
            )));
        }
        Ok(value)
    }

    fn collator_optional_string(&mut self, options: &Value, name: &str) -> Result<Option<String>> {
        if matches!(options, Value::Undefined) {
            return Ok(None);
        }
        let value = self.get_named(options, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        self.to_string(&value).map(Some)
    }

    fn collator_optional_boolean(&mut self, options: &Value, name: &str) -> Result<Option<bool>> {
        if matches!(options, Value::Undefined) {
            return Ok(None);
        }
        let value = self.get_named(options, name)?;
        (!matches!(value, Value::Undefined))
            .then(|| to_boolean(self, &value))
            .transpose()
    }

    fn collator_prototype_id(&self, constructor: NativeFunctionId) -> Result<ObjectId> {
        match self.native_function(constructor)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("Intl.Collator prototype is not an object")),
        }
    }

    fn install_collator_compare_accessor(&mut self, prototype: ObjectId) -> Result<()> {
        let getter = self.create_native_function(
            super::intl_kind(IntlFunctionKind::CollatorCompareGetter),
            Value::Undefined,
        )?;
        let key = self.intern_property_key("compare")?;
        self.objects.define_property(
            prototype,
            key,
            "compare",
            PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                Some(getter),
                None,
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn install_collator_static_method(&mut self, constructor: NativeFunctionId) -> Result<()> {
        let method = self.create_native_function(
            super::intl_kind(IntlFunctionKind::CollatorSupportedLocalesOf),
            Value::Undefined,
        )?;
        let key = self.intern_property_key(SUPPORTED_LOCALES_OF)?;
        self.define_native_function_property_key(
            constructor,
            SUPPORTED_LOCALES_OF,
            key,
            DataPropertyUpdate::new(
                Some(method),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            ),
        )
    }
}

struct ResolvedCollatorLocale {
    locale: String,
    collation: String,
    numeric: bool,
    case_first: String,
}

fn resolve_collator_locale(
    requested: &str,
    collation_option: Option<&str>,
    numeric_option: Option<bool>,
    case_first_option: Option<&str>,
) -> ResolvedCollatorLocale {
    let (base, keywords) = collator_locale_parts(requested);
    let language = base.split('-').next().unwrap_or(DEFAULT_LOCALE);
    let extension_collation =
        keyword_value(&keywords, "co").filter(|value| supported_collation(language, value));
    let option_collation = collation_option
        .map(str::to_ascii_lowercase)
        .filter(|value| supported_collation(language, value));
    let collation = option_collation
        .clone()
        .or_else(|| extension_collation.map(str::to_owned))
        .unwrap_or_else(|| "default".to_owned());
    let extension_numeric = keyword_value(&keywords, "kn").and_then(|value| match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    });
    let numeric = numeric_option.or(extension_numeric).unwrap_or(false);
    let extension_case_first = keyword_value(&keywords, "kf")
        .filter(|value| matches!(*value, "upper" | "lower" | "false"));
    let case_first = case_first_option
        .map(str::to_owned)
        .or_else(|| extension_case_first.map(str::to_owned))
        .unwrap_or_else(|| "false".to_owned());

    let mut extensions = Vec::new();
    if extension_collation == Some(collation.as_str())
        && option_collation
            .as_deref()
            .is_none_or(|option| option.eq_ignore_ascii_case(&collation))
    {
        extensions.push(format!("co-{collation}"));
    }
    if extension_case_first == Some(case_first.as_str())
        && case_first_option.is_none_or(|option| option == case_first)
    {
        extensions.push(format!("kf-{case_first}"));
    }
    if extension_numeric == Some(numeric) && numeric_option.is_none_or(|option| option == numeric) {
        extensions.push(if numeric {
            "kn".to_owned()
        } else {
            "kn-false".to_owned()
        });
    }
    let locale = if extensions.is_empty() {
        base
    } else {
        format!("{base}-u-{}", extensions.join("-"))
    };
    ResolvedCollatorLocale {
        locale,
        collation,
        numeric,
        case_first,
    }
}

fn collator_locale_parts(locale: &str) -> (String, Vec<(String, String)>) {
    let parts = locale.split('-').collect::<Vec<_>>();
    let unicode_start = parts.iter().position(|part| part.eq_ignore_ascii_case("u"));
    let Some(start) = unicode_start else {
        return (locale.to_owned(), Vec::new());
    };
    if parts
        .iter()
        .take(start)
        .any(|part| part.eq_ignore_ascii_case("x"))
    {
        return (locale.to_owned(), Vec::new());
    }
    let extension_end = parts
        .iter()
        .enumerate()
        .skip(start.saturating_add(1))
        .find_map(|(index, part)| (part.len() == 1).then_some(index))
        .unwrap_or(parts.len());
    let mut base_parts = Vec::new();
    base_parts.extend(parts.iter().take(start).copied());
    base_parts.extend(parts.iter().skip(extension_end).copied());
    let base = base_parts.join("-");
    let mut keywords = Vec::new();
    let mut index = start.saturating_add(1);
    while index < extension_end {
        let Some(part) = parts.get(index) else {
            break;
        };
        if part.len() != 2 {
            index = index.saturating_add(1);
            continue;
        }
        let key = part.to_ascii_lowercase();
        index = index.saturating_add(1);
        let value_start = index;
        while index < extension_end && parts.get(index).is_some_and(|value| value.len() >= 3) {
            index = index.saturating_add(1);
        }
        let value = if value_start == index {
            "true".to_owned()
        } else {
            parts.get(value_start..index).map_or_else(
                || "true".to_owned(),
                |values| values.join("-").to_ascii_lowercase(),
            )
        };
        keywords.push((key, value));
    }
    (base, keywords)
}

fn keyword_value<'a>(keywords: &'a [(String, String)], key: &str) -> Option<&'a str> {
    keywords
        .iter()
        .find_map(|(candidate, value)| (candidate == key).then_some(value.as_str()))
}

fn supported_collation(language: &str, value: &str) -> bool {
    value == "eor" || (language.eq_ignore_ascii_case("de") && value == "phonebk")
}

fn compare_collator_strings(formatter: &CollatorValue, left: &str, right: &str) -> Ordering {
    let left = collator_key(formatter, left);
    let right = collator_key(formatter, right);
    let primary = if formatter.numeric {
        compare_numeric_text(&left.primary, &right.primary)
    } else {
        left.primary.cmp(&right.primary)
    };
    if primary != Ordering::Equal || formatter.sensitivity == "base" {
        return primary;
    }
    match formatter.sensitivity.as_str() {
        "accent" => left.accent.cmp(&right.accent),
        "case" => left.case.cmp(&right.case),
        _ => left
            .accent
            .cmp(&right.accent)
            .then_with(|| compare_case(formatter, &left.case, &right.case))
            .then_with(|| left.variant.cmp(&right.variant)),
    }
}

fn collator_key(formatter: &CollatorValue, input: &str) -> CollationKey {
    let normalized = input.nfc().collect::<String>();
    let filtered = if formatter.ignore_punctuation {
        normalized
            .chars()
            .filter(|character| character.is_alphanumeric() || is_combining_mark(*character))
            .collect::<String>()
    } else {
        normalized
    };
    let phonebook = formatter.collation == "phonebk"
        || (formatter.usage == "search"
            && formatter
                .locale
                .split('-')
                .next()
                .is_some_and(|language| language.eq_ignore_ascii_case("de")));
    let primary_source = if phonebook {
        german_phonebook_text(&filtered)
    } else {
        filtered.clone()
    };
    let primary = strip_marks(&primary_source).to_lowercase();
    let accent = filtered.nfd().flat_map(char::to_lowercase).collect();
    let case = strip_marks(&filtered);
    CollationKey {
        primary,
        accent,
        case,
        variant: filtered,
    }
}

fn german_phonebook_text(input: &str) -> String {
    input
        .chars()
        .flat_map(|character| match character {
            'ä' => "ae".chars().collect::<Vec<_>>(),
            'Ä' => "AE".chars().collect::<Vec<_>>(),
            'ö' => "oe".chars().collect::<Vec<_>>(),
            'Ö' => "OE".chars().collect::<Vec<_>>(),
            'ü' => "ue".chars().collect::<Vec<_>>(),
            'Ü' => "UE".chars().collect::<Vec<_>>(),
            'ß' => "ss".chars().collect::<Vec<_>>(),
            value => vec![value],
        })
        .collect()
}

fn strip_marks(input: &str) -> String {
    input
        .nfd()
        .filter(|value| !is_combining_mark(*value))
        .collect()
}

fn compare_case(formatter: &CollatorValue, left: &str, right: &str) -> Ordering {
    match formatter.case_first.as_str() {
        "upper" => case_rank(left).cmp(&case_rank(right)),
        "lower" => case_rank(left).cmp(&case_rank(right)).reverse(),
        _ => left.cmp(right).reverse(),
    }
}

fn case_rank(input: &str) -> u8 {
    input
        .chars()
        .find(|value| value.is_alphabetic())
        .map_or(0, |value| u8::from(value.is_lowercase()))
}

fn compare_numeric_text(left: &str, right: &str) -> Ordering {
    let mut left_chars = left.chars().peekable();
    let mut right_chars = right.chars().peekable();
    loop {
        match (left_chars.peek().copied(), right_chars.peek().copied()) {
            (Some(left_char), Some(right_char))
                if left_char.is_ascii_digit() && right_char.is_ascii_digit() =>
            {
                let left_digits = take_ascii_digits(&mut left_chars);
                let right_digits = take_ascii_digits(&mut right_chars);
                let ordering = compare_digit_runs(&left_digits, &right_digits);
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            (Some(left_char), Some(right_char)) => {
                left_chars.next();
                right_chars.next();
                let ordering = left_char.cmp(&right_char);
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
        }
    }
}

fn take_ascii_digits<I>(chars: &mut core::iter::Peekable<I>) -> String
where
    I: Iterator<Item = char>,
{
    let mut digits = String::new();
    while chars.peek().is_some_and(char::is_ascii_digit) {
        if let Some(value) = chars.next() {
            digits.push(value);
        }
    }
    digits
}

fn compare_digit_runs(left: &str, right: &str) -> Ordering {
    let left_trimmed = left.trim_start_matches('0');
    let right_trimmed = right.trim_start_matches('0');
    let left_significant = if left_trimmed.is_empty() {
        "0"
    } else {
        left_trimmed
    };
    let right_significant = if right_trimmed.is_empty() {
        "0"
    } else {
        right_trimmed
    };
    left_significant
        .len()
        .cmp(&right_significant.len())
        .then_with(|| left_significant.cmp(right_significant))
}

fn is_unicode_type(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|part| {
            (3..=8).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

fn collator_range_error(message: &str) -> Error {
    Error::exception(ErrorName::RangeError, message)
}
