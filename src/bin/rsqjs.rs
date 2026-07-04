use std::{env, fs, process};

use rs_quickjs::{Runtime, Value};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let source = match args.next().as_deref() {
        Some("-e" | "--eval") => args.next().ok_or("missing source after --eval")?,
        Some(path) => fs::read_to_string(path)?,
        None => {
            eprintln!("usage: rsqjs [-e source] [file.js]");
            process::exit(2);
        }
    };

    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(&source)?;

    for line in context.take_output() {
        println!("{line}");
    }

    if value != Value::Undefined {
        println!("{value}");
    }

    Ok(())
}
