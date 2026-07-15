fn main() {
    println!("cargo:rerun-if-changed=src/coverage.c");
    cc::Build::new()
        .file("src/coverage.c")
        .warnings(true)
        .extra_warnings(true)
        .compile("velum_fuzzilli_coverage");
}
