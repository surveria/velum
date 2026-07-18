use std::{env, path::PathBuf, process::ExitCode};

use velum_regexp_unicode_gen::{GenerationConfig, generate};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("velum-regexp-unicode-gen: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    let [input_directory, source_manifest, output_directory] = arguments.as_slice() else {
        return Err(
            "usage: velum-regexp-unicode-gen <input-directory> <source-manifest> <output-directory>"
                .into(),
        );
    };
    let config = GenerationConfig::new(
        PathBuf::from(input_directory),
        PathBuf::from(source_manifest),
        PathBuf::from(output_directory),
    );
    let summary = generate(&config)?;
    println!(
        "generated Unicode {}: {} binary properties, {} general categories, {} scripts, {} ID_Start ranges, {} ID_Continue ranges, {} bytes",
        summary.unicode_version,
        summary.binary_properties,
        summary.general_categories,
        summary.scripts,
        summary.id_start_ranges,
        summary.id_continue_ranges,
        summary.output_bytes
    );
    Ok(())
}
