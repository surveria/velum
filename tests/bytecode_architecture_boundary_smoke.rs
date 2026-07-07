use std::{fs, path::Path};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const BYTECODE_ONLY_SOURCE_DIRS: [&str; 5] = [
    "src/api",
    "src/compiled_script",
    "src/runtime",
    "src/storage",
    "src/value",
];

#[test]
fn runtime_and_embedding_layers_do_not_import_parser_ast() -> TestResult {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative_dir in BYTECODE_ONLY_SOURCE_DIRS {
        check_source_dir(&repo.join(relative_dir))?;
    }
    Ok(())
}

fn check_source_dir(dir: &Path) -> TestResult {
    let entries = fs::read_dir(dir)
        .map_err(|error| format!("failed to read source dir {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read source dir entry under {}: {error}",
                dir.display()
            )
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
        if file_type.is_dir() {
            check_source_dir(&path)?;
        } else if is_rust_file(&path) {
            check_source_file(&path)?;
        }
    }
    Ok(())
}

fn check_source_file(path: &Path) -> TestResult {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read source file {}: {error}", path.display()))?;
    for line in text.lines() {
        if line_imports_parser_ast(line) {
            return Err(format!(
                "{} imports parser AST through `{}`; runtime and embedding layers must execute bytecode-owned metadata",
                path.display(),
                line.trim()
            )
            .into());
        }
    }
    Ok(())
}

fn is_rust_file(path: &Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| extension == "rs")
}

fn line_imports_parser_ast(line: &str) -> bool {
    let trimmed = line.trim_start();
    line.contains("crate::ast")
        || trimmed.starts_with("ast::")
        || line.contains(" ast::")
        || line.contains("{ast::")
        || line.contains("(ast::")
}
