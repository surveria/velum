use std::{
    cell::RefCell,
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read as _, Seek as _, Write as _},
    path::PathBuf,
    rc::Rc,
};

use anyhow::{Context as _, bail, ensure};
use velum::{Error, Runtime};

const CONTROL_READ_PATH: &str = "/proc/self/fd/100";
const CONTROL_WRITE_PATH: &str = "/proc/self/fd/101";
const DATA_READ_PATH: &str = "/proc/self/fd/102";
const FUZZOUT_WRITE_PATH: &str = "/proc/self/fd/103";
const HANDSHAKE: [u8; 4] = *b"HELO";
const EXECUTE_COMMAND: [u8; 4] = *b"cexe";
const MAX_SCRIPT_SIZE: usize = 16 << 20;

/// Runs the Velum Fuzzilli target using the inherited REPRL channels.
///
/// # Errors
///
/// Returns an error for invalid arguments, unavailable REPRL channels, an
/// invalid protocol message, or channel I/O failures.
pub fn run() -> anyhow::Result<()> {
    match parse_arguments()? {
        Invocation::Reprl => run_reprl(),
        Invocation::File(path) => run_file(&path),
    }
}

enum Invocation {
    Reprl,
    File(PathBuf),
}

fn parse_arguments() -> anyhow::Result<Invocation> {
    let mut args = env::args().skip(1);
    let Some(mode) = args.next() else {
        bail!("usage: velum-fuzzilli --reprl | --file PATH");
    };
    let invocation = match mode.as_str() {
        "--reprl" => Invocation::Reprl,
        "--file" => Invocation::File(
            args.next()
                .map(PathBuf::from)
                .context("missing JavaScript path after --file")?,
        ),
        _ => bail!("unexpected argument '{mode}'"),
    };
    if let Some(extra) = args.next() {
        bail!("unexpected argument '{extra}'");
    }
    Ok(invocation)
}

fn run_reprl() -> anyhow::Result<()> {
    let mut control_read = File::open(CONTROL_READ_PATH)
        .with_context(|| format!("failed to open REPRL control input '{CONTROL_READ_PATH}'"))?;
    let mut control_write = OpenOptions::new()
        .write(true)
        .open(CONTROL_WRITE_PATH)
        .with_context(|| format!("failed to open REPRL control output '{CONTROL_WRITE_PATH}'"))?;
    let mut data_read = File::open(DATA_READ_PATH)
        .with_context(|| format!("failed to open REPRL data input '{DATA_READ_PATH}'"))?;
    let fuzzout = OpenOptions::new()
        .write(true)
        .open(FUZZOUT_WRITE_PATH)
        .with_context(|| format!("failed to open REPRL fuzz output '{FUZZOUT_WRITE_PATH}'"))?;
    let fuzzout = Rc::new(RefCell::new(fuzzout));
    serve(
        &mut control_read,
        &mut control_write,
        &mut data_read,
        &fuzzout,
    )
}

fn run_file(path: &PathBuf) -> anyhow::Result<()> {
    let script = fs::read(path)
        .with_context(|| format!("failed to read reproducer '{}'", path.display()))?;
    let fuzzout = OpenOptions::new()
        .write(true)
        .open("/dev/stdout")
        .context("failed to open stdout for Fuzzilli output")?;
    let status = execute_for_fuzzing(&script, Rc::new(RefCell::new(fuzzout)))?;
    ensure!(
        status == 0,
        "reproducer completed with JavaScript error status {status}"
    );
    Ok(())
}

fn serve(
    control_read: &mut File,
    control_write: &mut File,
    data_read: &mut File,
    fuzzout: &Rc<RefCell<File>>,
) -> anyhow::Result<()> {
    control_write
        .write_all(&HANDSHAKE)
        .context("failed to send REPRL handshake")?;
    control_write
        .flush()
        .context("failed to flush REPRL handshake")?;

    let mut response = [0_u8; 4];
    control_read
        .read_exact(&mut response)
        .context("failed to read REPRL handshake response")?;
    ensure!(response == HANDSHAKE, "invalid REPRL handshake response");

    loop {
        let mut command = [0_u8; 4];
        match control_read.read_exact(&mut command) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error).context("failed to read REPRL command"),
        }
        ensure!(command == EXECUTE_COMMAND, "invalid REPRL execute command");

        let mut size_bytes = [0_u8; 8];
        control_read
            .read_exact(&mut size_bytes)
            .context("failed to read REPRL script size")?;
        let size = usize::try_from(u64::from_ne_bytes(size_bytes))
            .context("REPRL script size does not fit in memory")?;
        ensure!(
            size <= MAX_SCRIPT_SIZE,
            "REPRL script exceeds the 16 MiB limit"
        );

        data_read
            .seek(io::SeekFrom::Start(0))
            .context("failed to rewind REPRL data input")?;
        let mut script = vec![0_u8; size];
        data_read
            .read_exact(&mut script)
            .context("failed to read REPRL script")?;
        let status = execute_for_fuzzing(&script, Rc::clone(fuzzout))?;
        let encoded_status = u32::from(status)
            .checked_shl(8)
            .context("failed to encode REPRL exit status")?;
        control_write
            .write_all(&encoded_status.to_ne_bytes())
            .context("failed to write REPRL execution status")?;
        control_write
            .flush()
            .context("failed to flush REPRL execution status")?;
    }
}

/// Executes one Fuzzilli-generated script in a fresh Velum runtime.
///
/// A successful evaluation returns status zero. Syntax and JavaScript runtime
/// errors return status one so Fuzzilli can discard semantically invalid
/// mutations without terminating the persistent target process.
///
/// # Errors
///
/// Returns an error when the input is not UTF-8 or the Fuzzilli host callback
/// cannot be installed.
pub fn execute_for_fuzzing(script: &[u8], fuzzout: Rc<RefCell<File>>) -> anyhow::Result<u8> {
    let source = std::str::from_utf8(script).context("Fuzzilli script is not valid UTF-8")?;
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context
        .register_host_function_typed("fuzzilli", move |call| {
            let operation = call.string(0, "operation")?;
            if operation != "FUZZILLI_PRINT" {
                return Ok(());
            }
            let value = call.required_value(1, "value")?;
            let mut output = fuzzout.try_borrow_mut().map_err(|error| {
                Error::runtime(format!("fuzz output is already borrowed: {error}"))
            })?;
            writeln!(output, "{}", value.as_value())
                .map_err(|error| Error::runtime(format!("failed to write fuzz output: {error}")))?;
            output
                .flush()
                .map_err(|error| Error::runtime(format!("failed to flush fuzz output: {error}")))?;
            Ok(())
        })
        .map_err(|error| {
            anyhow::anyhow!("failed to register the Fuzzilli host callback: {error}")
        })?;

    let succeeded = context.eval(source).is_ok();
    drop(context.take_output());
    Ok(u8::from(!succeeded))
}
