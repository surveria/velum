use std::{env, error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let output_directory = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?);
    println!("cargo:rerun-if-changed=src/coverage.c");
    cc::Build::new()
        .file("src/coverage.c")
        .warnings(true)
        .extra_warnings(true)
        .cargo_metadata(false)
        .compile("velum_differential_fuzz_coverage");
    println!(
        "cargo:rustc-link-search=native={}",
        output_directory.display()
    );
    println!("cargo:rustc-link-lib=static:+whole-archive=velum_differential_fuzz_coverage");
    Ok(())
}
