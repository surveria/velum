# Velum CLI

`velum-cli` is a small console application that embeds the Velum ECMAScript
engine. Terminal dependencies stay in this nested package and do not become
dependencies of the embeddable engine crate.

## Build

From the repository root:

```sh
cargo build --release --manifest-path cli/Cargo.toml
./cli/target/release/velum
```

The package builds natively on supported Rust platforms. The interactive line
editor supports Unix terminals, including macOS, and Windows consoles.

## Usage

Start a persistent interactive session:

```sh
./cli/target/release/velum
```

Run a JavaScript file or source string:

```sh
./cli/target/release/velum script.js
./cli/target/release/velum --eval 'print("hello"); 40 + 2'
```

In interactive mode, Enter executes the entire edit buffer and Ctrl-J inserts
a newline. Shift-Enter also inserts a newline when the terminal reports it as
a distinct modified key; some terminals encode it identically to Enter.

The same JavaScript context is reused until `.reset` or process exit, so global
bindings, functions, and objects remain available to later submissions.

Available shell commands are `.help`, `.reset`, `.gc`, and `.exit`. Ctrl-C exits
the shell immediately, while Ctrl-D exits when the edit buffer is empty.

Velum provides ECMAScript rather than Node.js or browser APIs. Use `print(...)`
for output. `console.log`, filesystem APIs, networking, timers, and package
loading are intentionally outside this initial shell.
