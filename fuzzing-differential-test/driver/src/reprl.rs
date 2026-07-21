use std::{
    fs::{File, OpenOptions},
    io::{self, Read as _, Seek as _, Write as _},
};

use anyhow::{Context as _, ensure};

use crate::artifacts::{ArtifactRecorder, TargetConfig};

const CONTROL_READ_PATH: &str = "/proc/self/fd/100";
const CONTROL_WRITE_PATH: &str = "/proc/self/fd/101";
const DATA_READ_PATH: &str = "/proc/self/fd/102";
const FUZZOUT_WRITE_PATH: &str = "/proc/self/fd/103";
const HANDSHAKE: [u8; 4] = *b"HELO";
const EXECUTE_COMMAND: [u8; 4] = *b"exec";
const MAX_SCRIPT_SIZE: usize = 16 << 20;

/// Runs the differential target over inherited REPRL channels.
///
/// # Errors
///
/// Returns an error for invalid REPRL channels, invalid protocol messages, or
/// per-case artifact write failures.
pub fn run_reprl(args: impl IntoIterator<Item = String>) -> anyhow::Result<()> {
    let config = TargetConfig::from_args_or_env(args)?;
    let mut recorder = ArtifactRecorder::new(config)?;
    let mut control_read = File::open(CONTROL_READ_PATH)
        .with_context(|| format!("failed to open REPRL control input '{CONTROL_READ_PATH}'"))?;
    let mut control_write = OpenOptions::new()
        .write(true)
        .open(CONTROL_WRITE_PATH)
        .with_context(|| format!("failed to open REPRL control output '{CONTROL_WRITE_PATH}'"))?;
    let mut data_read = File::open(DATA_READ_PATH)
        .with_context(|| format!("failed to open REPRL data input '{DATA_READ_PATH}'"))?;
    let _fuzzout = OpenOptions::new()
        .write(true)
        .open(FUZZOUT_WRITE_PATH)
        .with_context(|| format!("failed to open REPRL fuzz output '{FUZZOUT_WRITE_PATH}'"))?;
    serve(
        &mut control_read,
        &mut control_write,
        &mut data_read,
        &mut recorder,
    )
}

fn serve(
    control_read: &mut File,
    control_write: &mut File,
    data_read: &mut File,
    recorder: &mut ArtifactRecorder,
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
        let status = recorder.record(&script)?;
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
