use velum_regexp_unicode_gen::{
    CodePointRange, SourceManifest, all_data_ranges, legacy_reverse_mappings, legacy_uppercase,
    property_ranges, property_value_ranges, simple_case_folding, subtract_ranges,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn manifest_rejects_duplicate_and_parent_paths() -> TestResult {
    let duplicate = SourceManifest::parse(
        "format=1\nunicode=17.0.0\nsha256 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa DerivedCoreProperties.txt https://www.unicode.org/Public/17.0.0/ucd/DerivedCoreProperties.txt\nsha256 bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb DerivedCoreProperties.txt https://www.unicode.org/Public/17.0.0/ucd/DerivedCoreProperties.txt\n",
    );
    if duplicate.is_ok() {
        return Err("duplicate source path was accepted".into());
    }
    let parent = SourceManifest::parse(
        "format=1\nunicode=17.0.0\nsha256 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ../DerivedCoreProperties.txt https://www.unicode.org/Public/17.0.0/ucd/DerivedCoreProperties.txt\n",
    );
    if parent.is_err() {
        return Ok(());
    }
    Err("parent source path was accepted".into())
}

#[test]
fn manifest_rejects_moving_or_non_https_sources() -> TestResult {
    let latest = SourceManifest::parse(
        "format=1\nunicode=17.0.0\nsha256 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa DerivedCoreProperties.txt https://www.unicode.org/Public/latest/ucd/DerivedCoreProperties.txt\n",
    );
    let insecure = SourceManifest::parse(
        "format=1\nunicode=17.0.0\nsha256 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa DerivedCoreProperties.txt http://www.unicode.org/Public/17.0.0/ucd/DerivedCoreProperties.txt\n",
    );
    if latest.is_err() && insecure.is_err() {
        return Ok(());
    }
    Err("a moving or non-HTTPS Unicode source was accepted".into())
}

#[test]
fn property_parser_sorts_merges_and_validates_ranges() -> TestResult {
    let input = "0043 ; ID_Start\n0041..0042 ; ID_Start # comment\n0044 ; Other\n";
    let ranges = property_ranges(input, "ID_Start")?;
    let expected = [(0x41, 0x43)];
    let actual = ranges
        .iter()
        .map(|range| (range.start, range.end))
        .collect::<Vec<_>>();
    if actual == expected {
        return Ok(());
    }
    Err(format!("unexpected merged ranges: {actual:?}").into())
}

#[test]
fn property_parser_rejects_out_of_range_values() -> TestResult {
    let result = property_ranges("110000 ; ID_Start\n", "ID_Start");
    if result.is_err() {
        return Ok(());
    }
    Err("out-of-range code point was accepted".into())
}

#[test]
fn multi_value_parser_and_range_subtraction_preserve_boundaries() -> TestResult {
    let input = "0010..0015 ; Latn Grek\n0020 ; Grek\n";
    let greek = property_value_ranges(input, "Grek")?;
    let all = all_data_ranges(input)?;
    let remainder = subtract_ranges(
        vec![CodePointRange {
            start: 0x000F,
            end: 0x0021,
        }],
        &all,
    );
    if greek
        == [
            CodePointRange {
                start: 0x0010,
                end: 0x0015,
            },
            CodePointRange {
                start: 0x0020,
                end: 0x0020,
            },
        ]
        && remainder
            == [
                CodePointRange {
                    start: 0x000F,
                    end: 0x000F,
                },
                CodePointRange {
                    start: 0x0016,
                    end: 0x001F,
                },
                CodePointRange {
                    start: 0x0021,
                    end: 0x0021,
                },
            ]
    {
        return Ok(());
    }
    Err(format!("unexpected multi-value or subtraction result: {greek:?} {remainder:?}").into())
}

#[test]
fn case_mapping_parsers_keep_only_ecmascript_simple_mappings() -> TestResult {
    let folding = simple_case_folding(
        "0041; C; 0061; # common\n0130; F; 0069 0307; # full\n0130; T; 0069; # Turkic\n1E9E; S; 00DF; # simple\n",
    )?;
    if folding.len() != 2
        || folding
            .first()
            .is_none_or(|mapping| mapping.source != 0x0041)
        || folding
            .get(1)
            .is_none_or(|mapping| mapping.target != 0x00DF)
    {
        return Err(format!("unexpected simple case-folding mappings: {folding:?}").into());
    }

    let unicode_data = [
        "0061",
        "LATIN SMALL LETTER A",
        "Ll",
        "0",
        "L",
        "",
        "",
        "",
        "",
        "N",
        "",
        "",
        "0041",
        "",
        "0041",
    ]
    .join(";");
    let long_s = [
        "017F",
        "LATIN SMALL LETTER LONG S",
        "Ll",
        "0",
        "L",
        "",
        "",
        "",
        "",
        "N",
        "",
        "",
        "0053",
        "",
        "0053",
    ]
    .join(";");
    let uppercase = legacy_uppercase(&format!("{unicode_data}\n{long_s}"))?;
    let reverse = legacy_reverse_mappings(&uppercase);
    if uppercase.len() == 2 && reverse.len() == 1 {
        return Ok(());
    }
    Err(format!("unexpected legacy mappings: {uppercase:?} {reverse:?}").into())
}
