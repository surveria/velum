//! Replace only the timing executable in an owned launcher copy.

use std::{fs, os::unix::fs::PermissionsExt as _, path::Path};

type Fallible<T> = Result<T, Box<dyn std::error::Error>>;
const MOCK_TIME: &str = include_str!("mock-time.sh");
const EXECUTABLE_CHECK: &str = "[[ -x /usr/bin/time ]]";
const TIMED_INVOCATION: &str = "setsid /usr/bin/time -f ";

pub fn launcher(source: &str, root: &Path) -> Fallible<String> {
    if source.matches(EXECUTABLE_CHECK).count() != 1
        || source.matches(TIMED_INVOCATION).count() != 1
    {
        return Err("production timer anchors changed; review the fixture substitution".into());
    }
    let timer = root.join("mock-time");
    fs::write(&timer, MOCK_TIME)?;
    fs::set_permissions(&timer, fs::Permissions::from_mode(0o755))?;
    let path = timer.to_str().ok_or("fixture timer path is not UTF-8")?;
    let quoted = format!("'{}'", path.replace('\'', "'\\''"));
    Ok(source
        .replace(EXECUTABLE_CHECK, &format!("[[ -x {quoted} ]]"))
        .replace(TIMED_INVOCATION, &format!("setsid {quoted} -f ")))
}
