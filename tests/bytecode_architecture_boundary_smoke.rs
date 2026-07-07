use std::{fs, path::Path};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const AST_FRONTEND_SOURCE_DIRS: [&str; 4] = [
    "src/ast",
    "src/parser",
    "src/compiler",
    "src/binding_layout",
];

const OBSOLETE_AST_EXECUTION_MARKERS: [&str; 4] = [
    "EvalAst",
    "eval_ast",
    "AstInterpreter",
    "RuntimeCallArgs::evaluate",
];

#[test]
fn only_frontend_and_compiler_layers_import_parser_ast() -> TestResult {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    check_source_dir(repo, &repo.join("src"))?;
    Ok(())
}

fn check_source_dir(repo: &Path, dir: &Path) -> TestResult {
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
            check_source_dir(repo, &path)?;
        } else if is_rust_file(&path) {
            check_source_file(repo, &path)?;
        }
    }
    Ok(())
}

fn check_source_file(repo: &Path, path: &Path) -> TestResult {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read source file {}: {error}", path.display()))?;
    let parser_ast_allowed = path_allows_parser_ast(repo, path)?;
    for line in text.lines() {
        if line_imports_parser_ast(line) && !parser_ast_allowed {
            return Err(format!(
                "{} imports parser AST through `{}`; only parser, AST, compiler, and binding-layout layers may traverse parser AST",
                path.display(),
                line.trim()
            )
            .into());
        }
        if line_contains_obsolete_ast_execution_marker(line) {
            return Err(format!(
                "{} contains obsolete AST execution marker `{}`; runtime execution must stay bytecode-owned",
                path.display(),
                line.trim()
            )
            .into());
        }
    }
    Ok(())
}

fn path_allows_parser_ast(repo: &Path, path: &Path) -> Result<bool, std::path::StripPrefixError> {
    let relative = path.strip_prefix(repo)?;
    Ok(AST_FRONTEND_SOURCE_DIRS
        .iter()
        .any(|allowed| relative.starts_with(allowed)))
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

fn line_contains_obsolete_ast_execution_marker(line: &str) -> bool {
    OBSOLETE_AST_EXECUTION_MARKERS
        .iter()
        .any(|marker| line.contains(marker))
}
