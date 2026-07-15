use anyhow::{Result, ensure};
use velum_cli::ShellSession;

#[test]
fn preserves_bindings_across_multiline_submissions() -> Result<()> {
    let mut session = ShellSession::new();
    let definition = session.submit(
        "definition.js",
        "const seed = 21;\nfunction double(value) {\n    return value * 2;\n}",
    );
    ensure!(
        definition.succeeded(),
        "definition failed: {:?}",
        definition.errors()
    );

    let call = session.submit("call.js", "double(seed)");
    ensure!(call.succeeded(), "call failed: {:?}", call.errors());
    ensure!(call.value() == Some("42"), "unexpected call value");
    Ok(())
}

#[test]
fn drains_print_output_and_promise_jobs() -> Result<()> {
    let mut session = ShellSession::new();
    let submission = session.submit(
        "jobs.js",
        "Promise.resolve(3).then(value => print('async', value)); print('sync');",
    );
    ensure!(
        submission.succeeded(),
        "submission failed: {:?}",
        submission.errors()
    );
    ensure!(
        submission.output() == ["sync", "async 3"],
        "unexpected print output: {:?}",
        submission.output()
    );
    Ok(())
}

#[test]
fn keeps_output_and_state_after_a_javascript_exception() -> Result<()> {
    let mut session = ShellSession::new();
    let failed = session.submit(
        "failed.js",
        "var survived = 40; print('before throw'); throw new Error('boom');",
    );
    ensure!(!failed.succeeded(), "throw unexpectedly succeeded");
    ensure!(
        failed.output() == ["before throw"],
        "output before throw was lost"
    );

    let recovered = session.submit("recovered.js", "survived + 2");
    ensure!(
        recovered.succeeded(),
        "recovery failed: {:?}",
        recovered.errors()
    );
    ensure!(recovered.value() == Some("42"), "persistent state was lost");
    Ok(())
}

#[test]
fn restarts_runtime_budget_after_a_limit_error() -> Result<()> {
    let mut session = ShellSession::new();
    let limited = session.submit("limited.js", "while (true) {}");
    ensure!(!limited.succeeded(), "infinite loop unexpectedly succeeded");

    let recovered = session.submit("after-limit.js", "6 * 7");
    ensure!(
        recovered.succeeded(),
        "post-limit recovery failed: {:?}",
        recovered.errors()
    );
    ensure!(recovered.value() == Some("42"), "unexpected recovery value");
    Ok(())
}

#[test]
fn reset_discards_previous_bindings() -> Result<()> {
    let mut session = ShellSession::new();
    let definition = session.submit("definition.js", "var camera = 42;");
    ensure!(definition.succeeded(), "definition failed");
    session.reset();

    let lookup = session.submit("lookup.js", "typeof camera");
    ensure!(lookup.succeeded(), "lookup failed: {:?}", lookup.errors());
    ensure!(
        lookup.value() == Some("undefined"),
        "reset retained a binding"
    );
    Ok(())
}
