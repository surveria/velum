use velum_regexp_unicode_gen::{SourceManifest, property_ranges};

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
