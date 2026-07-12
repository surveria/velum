use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use rs_quickjs::{Error, ModuleLoader, ModuleSource};

#[derive(Clone)]
pub struct Test262ModuleLoader {
    test262_dir: PathBuf,
}

impl Test262ModuleLoader {
    pub fn new(test262_dir: &Path) -> Self {
        Self {
            test262_dir: test262_dir.to_path_buf(),
        }
    }

    fn resolve(referrer: &str, request: &str) -> rs_quickjs::Result<PathBuf> {
        let request_path = Path::new(request);
        let unresolved = if request_path.is_absolute() {
            return Err(Error::runtime(
                "absolute Test262 module request is not allowed",
            ));
        } else if request.starts_with("./") || request.starts_with("../") {
            Path::new(referrer)
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(request_path)
        } else {
            request_path.to_path_buf()
        };
        normalize_relative_module_path(&unresolved)
    }
}

impl ModuleLoader for Test262ModuleLoader {
    fn load(&mut self, referrer: &str, request: &str) -> rs_quickjs::Result<ModuleSource> {
        let relative = Self::resolve(referrer, request)?;
        let source = fs::read_to_string(self.test262_dir.join(&relative)).map_err(|error| {
            Error::runtime(format!(
                "failed to load Test262 module '{}' from '{referrer}': {error}",
                relative.display()
            ))
        })?;
        let specifier = relative_module_specifier(&relative)?;
        Ok(ModuleSource::new(specifier, source))
    }
}

fn normalize_relative_module_path(path: &Path) -> rs_quickjs::Result<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(Error::runtime(
                        "Test262 module request escaped the corpus root",
                    ));
                }
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(Error::runtime(
                    "Test262 module request must remain relative",
                ));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(Error::runtime("Test262 module request resolved to no file"));
    }
    Ok(normalized)
}

fn relative_module_specifier(path: &Path) -> rs_quickjs::Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            return Err(Error::runtime("canonical module path is not normalized"));
        };
        let part = part
            .to_str()
            .ok_or_else(|| Error::runtime("Test262 module path is not valid UTF-8"))?;
        parts.push(part);
    }
    Ok(parts.join("/"))
}
