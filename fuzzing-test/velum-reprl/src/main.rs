use std::process;

fn main() {
    if let Err(error) = velum_fuzzilli_target::run() {
        eprintln!("{error:#}");
        process::exit(1);
    }
}
