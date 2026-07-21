use std::process;

use velum_differential_fuzz::reprl;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(mode) = args.next() else {
        anyhow::bail!("usage: velum-diff-target --reprl");
    };
    if mode != "--reprl" {
        anyhow::bail!("unexpected argument '{mode}'");
    }
    reprl::run_reprl(args)
}
