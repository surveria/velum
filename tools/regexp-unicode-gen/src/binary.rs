use crate::{
    CodePointRange, GeneratorError,
    ucd::{collect_property_ranges, complement_ranges, normalize_ranges},
};

pub struct PropertySpec {
    pub canonical: &'static str,
    pub aliases: &'static [&'static str],
}

pub struct GeneratedProperty {
    pub spec: &'static PropertySpec,
    pub ranges: Vec<CodePointRange>,
}

pub const PROPERTY_SPECS: &[PropertySpec] = &[
    PropertySpec {
        canonical: "ASCII",
        aliases: &[],
    },
    PropertySpec {
        canonical: "ASCII_Hex_Digit",
        aliases: &["AHex"],
    },
    PropertySpec {
        canonical: "Alphabetic",
        aliases: &["Alpha"],
    },
    PropertySpec {
        canonical: "Any",
        aliases: &[],
    },
    PropertySpec {
        canonical: "Assigned",
        aliases: &[],
    },
    PropertySpec {
        canonical: "Bidi_Control",
        aliases: &["Bidi_C"],
    },
    PropertySpec {
        canonical: "Bidi_Mirrored",
        aliases: &["Bidi_M"],
    },
    PropertySpec {
        canonical: "Case_Ignorable",
        aliases: &["CI"],
    },
    PropertySpec {
        canonical: "Cased",
        aliases: &[],
    },
    PropertySpec {
        canonical: "Changes_When_Casefolded",
        aliases: &["CWCF"],
    },
    PropertySpec {
        canonical: "Changes_When_Casemapped",
        aliases: &["CWCM"],
    },
    PropertySpec {
        canonical: "Changes_When_Lowercased",
        aliases: &["CWL"],
    },
    PropertySpec {
        canonical: "Changes_When_NFKC_Casefolded",
        aliases: &["CWKCF"],
    },
    PropertySpec {
        canonical: "Changes_When_Titlecased",
        aliases: &["CWT"],
    },
    PropertySpec {
        canonical: "Changes_When_Uppercased",
        aliases: &["CWU"],
    },
    PropertySpec {
        canonical: "Dash",
        aliases: &[],
    },
    PropertySpec {
        canonical: "Default_Ignorable_Code_Point",
        aliases: &["DI"],
    },
    PropertySpec {
        canonical: "Deprecated",
        aliases: &["Dep"],
    },
    PropertySpec {
        canonical: "Diacritic",
        aliases: &["Dia"],
    },
    PropertySpec {
        canonical: "Emoji",
        aliases: &[],
    },
    PropertySpec {
        canonical: "Emoji_Component",
        aliases: &["EComp"],
    },
    PropertySpec {
        canonical: "Emoji_Modifier",
        aliases: &["EMod"],
    },
    PropertySpec {
        canonical: "Emoji_Modifier_Base",
        aliases: &["EBase"],
    },
    PropertySpec {
        canonical: "Emoji_Presentation",
        aliases: &["EPres"],
    },
    PropertySpec {
        canonical: "Extended_Pictographic",
        aliases: &["ExtPict"],
    },
    PropertySpec {
        canonical: "Extender",
        aliases: &["Ext"],
    },
    PropertySpec {
        canonical: "Grapheme_Base",
        aliases: &["Gr_Base"],
    },
    PropertySpec {
        canonical: "Grapheme_Extend",
        aliases: &["Gr_Ext"],
    },
    PropertySpec {
        canonical: "Hex_Digit",
        aliases: &["Hex"],
    },
    PropertySpec {
        canonical: "IDS_Binary_Operator",
        aliases: &["IDSB"],
    },
    PropertySpec {
        canonical: "IDS_Trinary_Operator",
        aliases: &["IDST"],
    },
    PropertySpec {
        canonical: "ID_Continue",
        aliases: &["IDC"],
    },
    PropertySpec {
        canonical: "ID_Start",
        aliases: &["IDS"],
    },
    PropertySpec {
        canonical: "Ideographic",
        aliases: &["Ideo"],
    },
    PropertySpec {
        canonical: "Join_Control",
        aliases: &["Join_C"],
    },
    PropertySpec {
        canonical: "Logical_Order_Exception",
        aliases: &["LOE"],
    },
    PropertySpec {
        canonical: "Lowercase",
        aliases: &["Lower"],
    },
    PropertySpec {
        canonical: "Math",
        aliases: &[],
    },
    PropertySpec {
        canonical: "Noncharacter_Code_Point",
        aliases: &["NChar"],
    },
    PropertySpec {
        canonical: "Pattern_Syntax",
        aliases: &["Pat_Syn"],
    },
    PropertySpec {
        canonical: "Pattern_White_Space",
        aliases: &["Pat_WS"],
    },
    PropertySpec {
        canonical: "Quotation_Mark",
        aliases: &["QMark"],
    },
    PropertySpec {
        canonical: "Radical",
        aliases: &[],
    },
    PropertySpec {
        canonical: "Regional_Indicator",
        aliases: &["RI"],
    },
    PropertySpec {
        canonical: "Sentence_Terminal",
        aliases: &["STerm"],
    },
    PropertySpec {
        canonical: "Soft_Dotted",
        aliases: &["SD"],
    },
    PropertySpec {
        canonical: "Terminal_Punctuation",
        aliases: &["Term"],
    },
    PropertySpec {
        canonical: "Unified_Ideograph",
        aliases: &["UIdeo"],
    },
    PropertySpec {
        canonical: "Uppercase",
        aliases: &["Upper"],
    },
    PropertySpec {
        canonical: "Variation_Selector",
        aliases: &["VS"],
    },
    PropertySpec {
        canonical: "White_Space",
        aliases: &["space"],
    },
    PropertySpec {
        canonical: "XID_Continue",
        aliases: &["XIDC"],
    },
    PropertySpec {
        canonical: "XID_Start",
        aliases: &["XIDS"],
    },
];

pub fn generate(
    sources: &[&str],
    general_categories: &str,
) -> Result<Vec<GeneratedProperty>, GeneratorError> {
    let mut generated = Vec::with_capacity(PROPERTY_SPECS.len());
    for spec in PROPERTY_SPECS {
        let ranges = match spec.canonical {
            "ASCII" => vec![CodePointRange {
                start: 0,
                end: 0x7F,
            }],
            "Any" => vec![CodePointRange {
                start: 0,
                end: 0x10_FFFF,
            }],
            "Assigned" => {
                let mut unassigned = Vec::new();
                collect_property_ranges(general_categories, "Cn", &mut unassigned)?;
                complement_ranges(unassigned)
            }
            property => collect_sources(sources, property)?,
        };
        if ranges.is_empty() {
            return Err(GeneratorError::new(format!(
                "ECMAScript binary property {} has no ranges",
                spec.canonical
            )));
        }
        generated.push(GeneratedProperty { spec, ranges });
    }
    Ok(generated)
}

fn collect_sources(
    sources: &[&str],
    property: &str,
) -> Result<Vec<CodePointRange>, GeneratorError> {
    let mut ranges = Vec::new();
    for source in sources {
        collect_property_ranges(source, property, &mut ranges)?;
    }
    Ok(normalize_ranges(ranges))
}
