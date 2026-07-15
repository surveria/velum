use std::{
    env, fs,
    io::{self, IsTerminal as _, Read as _},
};

use anyhow::{Context as _, bail};
use rustyline::{
    Cmd, DefaultEditor, EventHandler, KeyCode, KeyEvent, Modifiers, error::ReadlineError,
};

use crate::{ShellSession, Submission};

const PRIMARY_PROMPT: &str = "> ";
const STDIN_SOURCE_NAME: &str = "<stdin>";
const EVAL_SOURCE_NAME: &str = "<eval>";
const HELP: &str = "\
Usage: velum [OPTIONS] [FILE]\n\
\n\
Run FILE, evaluate source, or start an interactive Velum session.\n\
\n\
Options:\n\
  -e, --eval SOURCE  Evaluate SOURCE and exit\n\
  -h, --help         Show this help\n\
  -V, --version      Show the embedded engine version\n\
\n\
Interactive keys:\n\
  Enter              Execute the current buffer\n\
  Ctrl-J             Insert a newline\n\
  Shift-Enter        Insert a newline when reported distinctly by the terminal\n\
  Ctrl-C             Cancel the current edit\n\
  Ctrl-D             Exit when the buffer is empty\n\
\n\
Interactive commands:\n\
  .help              Show interactive help\n\
  .reset             Replace the JavaScript context\n\
  .gc                Run garbage collection\n\
  .exit              Exit the shell";

#[derive(Debug, Clone, Eq, PartialEq)]
enum Invocation {
    Interactive,
    Eval(String),
    File(String),
    Help,
    Version,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum CommandOutcome {
    NotCommand,
    Continue,
    Exit,
}

/// Runs the console application using process arguments and standard streams.
///
/// # Errors
///
/// Returns an error for invalid arguments, source I/O failures, terminal
/// failures, or failed non-interactive JavaScript evaluation.
pub fn run() -> anyhow::Result<()> {
    match parse_invocation(env::args().skip(1))? {
        Invocation::Interactive if io::stdin().is_terminal() => run_interactive(),
        Invocation::Interactive => run_stdin(),
        Invocation::Eval(source) => run_source(EVAL_SOURCE_NAME, &source),
        Invocation::File(path) => run_file(&path),
        Invocation::Help => {
            println!("{HELP}");
            Ok(())
        }
        Invocation::Version => {
            let build = velum::engine_build_info();
            println!("{} {}", build.package_name, build.version);
            Ok(())
        }
    }
}

fn parse_invocation(mut args: impl Iterator<Item = String>) -> anyhow::Result<Invocation> {
    let invocation = match args.next().as_deref() {
        None => Invocation::Interactive,
        Some("-e" | "--eval") => Invocation::Eval(
            args.next()
                .context("missing JavaScript source after --eval")?,
        ),
        Some("-h" | "--help") => Invocation::Help,
        Some("-V" | "--version") => Invocation::Version,
        Some(path) => Invocation::File(path.to_owned()),
    };
    if let Some(extra) = args.next() {
        bail!("unexpected argument '{extra}'");
    }
    Ok(invocation)
}

fn run_file(path: &str) -> anyhow::Result<()> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read JavaScript file '{path}'"))?;
    run_source(path, &source)
}

fn run_stdin() -> anyhow::Result<()> {
    let mut source = String::new();
    io::stdin()
        .read_to_string(&mut source)
        .context("failed to read JavaScript source from stdin")?;
    run_source(STDIN_SOURCE_NAME, &source)
}

fn run_source(source_name: &str, source: &str) -> anyhow::Result<()> {
    let mut session = ShellSession::new();
    let submission = session.submit(source_name, source);
    render_output(&submission);
    if submission.succeeded() {
        return Ok(());
    }
    bail!(submission.errors().join("\n"));
}

fn run_interactive() -> anyhow::Result<()> {
    let build = velum::engine_build_info();
    println!("Velum {} interactive shell", build.version);
    println!("Enter submits; Ctrl-J inserts a newline; .help lists commands.");

    let mut editor = DefaultEditor::new().context("failed to initialize terminal editor")?;
    configure_multiline_bindings(&mut editor);
    let mut session = ShellSession::new();
    let mut entry = 1_u64;

    loop {
        match editor.readline(PRIMARY_PROMPT) {
            Ok(source) => {
                match handle_command(&source, &mut session) {
                    CommandOutcome::Exit => break,
                    CommandOutcome::Continue => continue,
                    CommandOutcome::NotCommand => {}
                }
                if source.trim().is_empty() {
                    continue;
                }
                editor
                    .add_history_entry(source.as_str())
                    .context("failed to update interactive history")?;
                let source_name = format!("<repl:{entry}>");
                let submission = session.submit(&source_name, &source);
                render_interactive(&submission);
                entry = entry.saturating_add(1);
            }
            Err(ReadlineError::Interrupted) => println!("^C"),
            Err(ReadlineError::Eof) => {
                println!();
                break;
            }
            Err(error) => return Err(error).context("interactive terminal read failed"),
        }
    }
    Ok(())
}

fn configure_multiline_bindings(editor: &mut DefaultEditor) {
    drop(editor.bind_sequence(
        KeyEvent(KeyCode::Char('J'), Modifiers::CTRL),
        EventHandler::Simple(Cmd::Newline),
    ));
    drop(editor.bind_sequence(
        KeyEvent(KeyCode::Enter, Modifiers::SHIFT),
        EventHandler::Simple(Cmd::Newline),
    ));
}

fn handle_command(source: &str, session: &mut ShellSession) -> CommandOutcome {
    match source.trim() {
        ".exit" => CommandOutcome::Exit,
        ".help" => {
            println!("{HELP}");
            CommandOutcome::Continue
        }
        ".reset" => {
            session.reset();
            println!("JavaScript context reset.");
            CommandOutcome::Continue
        }
        ".gc" => {
            match session.collect_garbage() {
                Ok(()) => println!("Garbage collection complete."),
                Err(error) => eprintln!("Garbage collection failed: {error}"),
            }
            CommandOutcome::Continue
        }
        _ => CommandOutcome::NotCommand,
    }
}

fn render_output(submission: &Submission) {
    for line in submission.output() {
        println!("{line}");
    }
    if let Some(value) = submission.value() {
        println!("{value}");
    }
}

fn render_interactive(submission: &Submission) {
    render_output(submission);
    for error in submission.errors() {
        eprintln!("{error}");
    }
}
