use std::{
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, ensure};
use serde_json::json;

const WORKER: &str = "--memory-benchmark-worker";
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

fn configuration(timeout_ms: u64) -> serde_json::Value {
    json!({
        "protocol_version": 1,
        "engine": "velum",
        "scenario": {
            "id": "hello-world", "kind": "hello_world", "vm_count": 1,
            "nodes_per_vm": 0, "bytes_per_node": 0, "rounds": 1
        },
        "timeout_ms": timeout_ms
    })
}

fn spawn(config: &str) -> Result<Child> {
    Command::new(env!("CARGO_BIN_EXE_velum-test-runner"))
        .args([WORKER, config])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn memory worker test process")
}

fn bounded_output(mut child: Child) -> Result<Output> {
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child
                .wait_with_output()
                .context("failed to read memory worker test output");
        }
        if started.elapsed() >= TEST_TIMEOUT {
            child
                .kill()
                .context("failed to kill hung memory worker test")?;
            child
                .wait()
                .context("failed to reap hung memory worker test")?;
            anyhow::bail!("memory worker test exceeded its outer safety deadline");
        }
        thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn rejects_invalid_json_and_unknown_configuration_fields() -> Result<()> {
    let mut extra = configuration(1_000);
    extra
        .as_object_mut()
        .context("test configuration is not an object")?
        .insert("unexpected".to_owned(), json!(true));
    for encoded in ["invalid JSON".to_owned(), serde_json::to_string(&extra)?] {
        let output = bounded_output(spawn(&encoded)?)?;
        ensure!(!output.status.success());
        ensure!(output.stdout.is_empty());
        ensure!(String::from_utf8(output.stderr)?.contains("invalid memory worker configuration"));
    }
    Ok(())
}

#[test]
fn rejects_out_of_bounds_worker_configuration() -> Result<()> {
    let mut config = configuration(1_000);
    config
        .get_mut("scenario")
        .and_then(serde_json::Value::as_object_mut)
        .context("test scenario is not an object")?
        .insert("vm_count".to_owned(), json!(51));
    let output = bounded_output(spawn(&serde_json::to_string(&config)?)?)?;
    ensure!(!output.status.success());
    ensure!(output.stdout.is_empty());
    ensure!(String::from_utf8(output.stderr)?.contains("resource envelope"));
    Ok(())
}

#[test]
fn parent_acknowledgement_eof_stops_the_worker() -> Result<()> {
    let mut child = spawn(&serde_json::to_string(&configuration(1_000))?)?;
    drop(child.stdin.take());
    let output = bounded_output(child)?;
    ensure!(!output.status.success());
    ensure!(String::from_utf8(output.stdout)?.contains("process_baseline"));
    ensure!(String::from_utf8(output.stderr)?.contains("missing memory sampling acknowledgement"));
    Ok(())
}

#[test]
fn independent_deadline_stops_a_worker_waiting_for_its_parent() -> Result<()> {
    let mut child = spawn(&serde_json::to_string(&configuration(20))?)?;
    let input = child
        .stdin
        .take()
        .context("memory worker test stdin missing")?;
    let output = bounded_output(child)?;
    drop(input);
    ensure!(output.status.code() == Some(124));
    ensure!(String::from_utf8(output.stderr)?.contains("independent wall deadline"));
    Ok(())
}
