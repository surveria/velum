use std::{
    fs,
    path::{Path, StripPrefixError},
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const AST_FRONTEND_SOURCE_DIRS: [&str; 4] = [
    "src/ast",
    "src/parser",
    "src/compiler",
    "src/binding_layout",
];

const FRONTEND_PIPELINE_SOURCE_DIRS: [&str; 5] = [
    "src/lexer",
    "src/parser",
    "src/compiler",
    "src/binding_layout",
    "src/ast",
];

const FRONTEND_BRIDGE_SOURCE_DIRS: [&str; 1] = ["src/compiled_script"];

const PARSER_AST_MODULE: &str = "ast";
const FRONTEND_PIPELINE_MODULES: [&str; 4] = ["lexer", "parser", "compiler", "binding_layout"];

const SLOW_PATH_TERMINOLOGY_SOURCE_DIRS: [&str; 7] = [
    "src/api",
    "src/bytecode",
    "src/compiled_script",
    "src/runtime",
    "src/storage",
    "src/syntax",
    "src/value",
];

const OBSOLETE_AST_EXECUTION_MARKERS: [&str; 4] = [
    "EvalAst",
    "eval_ast",
    "AstInterpreter",
    "RuntimeCallArgs::evaluate",
];

const AST_EXECUTION_TYPE_MARKERS: [&str; 11] = [
    "program: Program",
    ": Program",
    "Rc<[Stmt]>",
    "Vec<Stmt>",
    "Box<Stmt>",
    "Stmt::",
    "Expr::",
    "Box<Expr>",
    "Vec<Expr>",
    ": Expr",
    "FunctionSpec",
];

#[test]
fn only_frontend_and_compiler_layers_import_parser_ast() -> TestResult {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    check_source_dir(repo, &repo.join("src"))?;
    Ok(())
}

#[test]
fn standardized_api_names_do_not_weaken_the_execution_fallback_guard() -> TestResult {
    if line_contains_fallback_terminology("let option = get_option(\"fallback\");") {
        return Err("the standardized fallback property must remain usable".into());
    }
    if !line_contains_fallback_terminology("let fallback = execute_ast();") {
        return Err("execution fallback terminology must remain rejected".into());
    }
    if !line_contains_fallback_terminology("let astFallback = execute();") {
        return Err("camel-case execution fallback terminology must remain rejected".into());
    }
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
    let frontend_pipeline_allowed = path_allows_frontend_pipeline(repo, path)?;
    let slow_path_terminology_required = path_requires_slow_path_terminology(repo, path)?;
    for line in text.lines() {
        if line_imports_parser_ast(line) && !parser_ast_allowed {
            return Err(format!(
                "{} imports parser AST through `{}`; only parser, AST, compiler, and binding-layout layers may traverse parser AST",
                path.display(),
                line.trim()
            )
            .into());
        }
        if line_imports_frontend_pipeline(line) && !frontend_pipeline_allowed {
            return Err(format!(
                "{} imports frontend pipeline module through `{}`; runtime execution must enter compiled bytecode through CompiledScript",
                path.display(),
                line.trim()
            )
            .into());
        }
        if line_contains_ast_execution_type_marker(line) && !parser_ast_allowed {
            return Err(format!(
                "{} contains parser AST execution/storage marker `{}`; non-frontend layers must store bytecode-owned metadata",
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
        if line_contains_fallback_terminology(line) && slow_path_terminology_required {
            return Err(format!(
                "{} uses fallback terminology through `{}`; runtime and public execution layers must call guarded misses slow paths, not AST fallbacks",
                path.display(),
                line.trim()
            )
            .into());
        }
    }
    Ok(())
}

fn path_allows_parser_ast(repo: &Path, path: &Path) -> Result<bool, StripPrefixError> {
    let relative = path.strip_prefix(repo)?;
    Ok(AST_FRONTEND_SOURCE_DIRS
        .iter()
        .any(|allowed| relative.starts_with(allowed)))
}

fn path_allows_frontend_pipeline(repo: &Path, path: &Path) -> Result<bool, StripPrefixError> {
    let relative = path.strip_prefix(repo)?;
    Ok(FRONTEND_PIPELINE_SOURCE_DIRS
        .iter()
        .chain(FRONTEND_BRIDGE_SOURCE_DIRS.iter())
        .any(|allowed| relative.starts_with(allowed)))
}

fn path_requires_slow_path_terminology(repo: &Path, path: &Path) -> Result<bool, StripPrefixError> {
    let relative = path.strip_prefix(repo)?;
    Ok(SLOW_PATH_TERMINOLOGY_SOURCE_DIRS
        .iter()
        .any(|required| relative.starts_with(required)))
}

fn is_rust_file(path: &Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| extension == "rs")
}

fn line_imports_parser_ast(line: &str) -> bool {
    line_imports_module(line, PARSER_AST_MODULE)
}

fn line_imports_frontend_pipeline(line: &str) -> bool {
    FRONTEND_PIPELINE_MODULES
        .iter()
        .any(|module| line_imports_module(line, module))
}

fn line_imports_module(line: &str, module: &str) -> bool {
    let trimmed = line.trim();
    let direct_path = format!("crate::{module}");
    let nested_path = format!("{module}::");
    let grouped_nested_path = format!("{{{module}::");
    let grouped_spaced_nested_path = format!("{{ {module}::");
    let parenthesized_nested_path = format!("({module}::");
    let bare_grouped_module = format!("{{{module}");
    let bare_spaced_grouped_module = format!("{{ {module}");
    let bare_comma_module = format!("{module},");
    let bare_trailing_module = format!("{module}}}");
    line.contains(&direct_path)
        || trimmed.starts_with(&nested_path)
        || line.contains(&format!(" {nested_path}"))
        || line.contains(&grouped_nested_path)
        || line.contains(&grouped_spaced_nested_path)
        || line.contains(&parenthesized_nested_path)
        || line.contains(&bare_grouped_module)
        || line.contains(&bare_spaced_grouped_module)
        || trimmed == bare_comma_module
        || trimmed.ends_with(&bare_trailing_module)
}

fn line_contains_ast_execution_type_marker(line: &str) -> bool {
    AST_EXECUTION_TYPE_MARKERS
        .iter()
        .any(|marker| line.contains(marker))
}

fn line_contains_obsolete_ast_execution_marker(line: &str) -> bool {
    OBSOLETE_AST_EXECUTION_MARKERS
        .iter()
        .any(|marker| line.contains(marker))
}

fn line_contains_fallback_terminology(line: &str) -> bool {
    let execution_text = line.replace("\"fallback\"", "");
    execution_text.contains("fallback") || execution_text.contains("Fallback")
}
