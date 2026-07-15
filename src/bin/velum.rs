use std::{env, fs, process};

use anyhow::{Context as _, bail};
use velum::{Runtime, Value};

fn main() {
    if let Err(error) = run() {
        // Alternate form prints the full anyhow context chain so CLI smoke
        // runs expose the underlying lexer/parser/runtime error.
        eprintln!("{error:#}");
        process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);
    let source = match args.next().as_deref() {
        Some("-e" | "--eval") => args.next().context("missing source after --eval")?,
        Some(path) => fs::read_to_string(path)
            .with_context(|| format!("failed to read script file '{path}'"))?,
        None => {
            bail!("usage: velum [-e source] [file.js]");
        }
    };

    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context
        .eval(&source)
        .map_err(|error| anyhow::anyhow!("script evaluation failed: {error}"))?;

    for line in context.take_output() {
        println!("{line}");
    }

    if value != Value::Undefined {
        println!("{value}");
    }

    Ok(())
}
