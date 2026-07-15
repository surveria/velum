use std::process;

fn main() {
    if let Err(error) = velum_cli::run() {
        eprintln!("{error:#}");
        process::exit(1);
    }
}
