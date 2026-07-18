use std::collections::BTreeSet;

use crate::{
    CodePointRange, GeneratorError,
    ucd::{
        all_data_ranges, collect_property_ranges, complement_ranges, normalize_ranges,
        property_value_ranges, subtract_ranges,
    },
};

#[derive(Debug, Clone)]
pub struct ValueSpec {
    pub short: String,
    pub long: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GeneratedValue {
    pub spec: ValueSpec,
    pub ranges: Vec<CodePointRange>,
}

pub fn generate_general_categories(
    aliases: &str,
    categories: &str,
) -> Result<Vec<GeneratedValue>, GeneratorError> {
    let specs = parse_value_aliases(aliases, "gc")?;
    let mut generated = Vec::with_capacity(specs.len());
    for spec in specs {
        let ranges = general_category_ranges(categories, &spec.short)?;
        if ranges.is_empty() {
            return Err(GeneratorError::new(format!(
                "general category {} has no ranges",
                spec.short
            )));
        }
        generated.push(GeneratedValue { spec, ranges });
    }
    Ok(generated)
}

pub fn generate_scripts(
    aliases: &str,
    scripts: &str,
    extensions: &str,
) -> Result<(Vec<GeneratedValue>, Vec<GeneratedValue>), GeneratorError> {
    let specs = parse_value_aliases(aliases, "sc")?;
    let explicit_scripts = all_data_ranges(scripts)?;
    let extension_overrides = all_data_ranges(extensions)?;
    let mut script_values = Vec::with_capacity(specs.len());
    let mut extension_values = Vec::with_capacity(specs.len());
    for spec in specs {
        let script_ranges = if spec.short == "Zzzz" {
            complement_ranges(explicit_scripts.clone())
        } else {
            property_value_ranges(scripts, &spec.long)?
        };
        let mut extension_ranges = subtract_ranges(script_ranges.clone(), &extension_overrides);
        extension_ranges.extend(property_value_ranges(extensions, &spec.short)?);
        extension_ranges = normalize_ranges(extension_ranges);
        script_values.push(GeneratedValue {
            spec: spec.clone(),
            ranges: script_ranges,
        });
        extension_values.push(GeneratedValue {
            spec,
            ranges: extension_ranges,
        });
    }
    Ok((script_values, extension_values))
}

fn general_category_ranges(
    contents: &str,
    short: &str,
) -> Result<Vec<CodePointRange>, GeneratorError> {
    let leaf_categories = [
        "Cc", "Cf", "Cn", "Co", "Cs", "Ll", "Lm", "Lo", "Lt", "Lu", "Mc", "Me", "Mn", "Nd", "Nl",
        "No", "Pc", "Pd", "Pe", "Pf", "Pi", "Po", "Ps", "Sc", "Sk", "Sm", "So", "Zl", "Zp", "Zs",
    ];
    let mut ranges = Vec::new();
    match short {
        "LC" => {
            for category in ["Ll", "Lt", "Lu"] {
                collect_property_ranges(contents, category, &mut ranges)?;
            }
        }
        value if value.len() == 1 => {
            for category in leaf_categories {
                if category.starts_with(value) {
                    collect_property_ranges(contents, category, &mut ranges)?;
                }
            }
        }
        value => collect_property_ranges(contents, value, &mut ranges)?,
    }
    Ok(normalize_ranges(ranges))
}

fn parse_value_aliases(
    contents: &str,
    requested_property: &str,
) -> Result<Vec<ValueSpec>, GeneratorError> {
    let mut specs = Vec::new();
    let mut all_aliases = BTreeSet::new();
    for (line_index, raw_line) in contents.lines().enumerate() {
        let line_number = line_index
            .checked_add(1)
            .ok_or_else(|| GeneratorError::new("property alias line number overflowed"))?;
        let data = raw_line.split('#').next().unwrap_or_default().trim();
        if data.is_empty() {
            continue;
        }
        let fields = data.split(';').map(str::trim).collect::<Vec<_>>();
        if fields.first().copied() != Some(requested_property) {
            continue;
        }
        let short = fields
            .get(1)
            .copied()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                GeneratorError::new(format!(
                    "property alias line {line_number}: missing short value"
                ))
            })?;
        let long = fields
            .get(2)
            .copied()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                GeneratorError::new(format!(
                    "property alias line {line_number}: missing long value"
                ))
            })?;
        let mut aliases = Vec::new();
        for alias in core::iter::once(short)
            .chain(core::iter::once(long))
            .chain(fields.iter().skip(3).copied())
        {
            if alias.is_empty() || aliases.iter().any(|existing| existing == alias) {
                continue;
            }
            if !all_aliases.insert(alias.to_owned()) {
                return Err(GeneratorError::new(format!(
                    "duplicate {requested_property} value alias {alias}"
                )));
            }
            aliases.push(alias.to_owned());
        }
        specs.push(ValueSpec {
            short: short.to_owned(),
            long: long.to_owned(),
            aliases,
        });
    }
    if specs.is_empty() {
        return Err(GeneratorError::new(format!(
            "property value aliases contain no {requested_property} values"
        )));
    }
    Ok(specs)
}
