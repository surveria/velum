use std::{env, path::Path, time::Duration};

use anyhow::{Context as _, ensure};
use rayon::{ThreadPoolBuilder, prelude::*};

use crate::{
    test262_metadata::{Test262CaseResult, execute_test262_path},
    timing,
};

const TEST_JOBS_ENV: &str = "RSQJS_TEST_JOBS";
const DEFAULT_TEST_JOBS: usize = 4;
const MAX_TEST_JOBS: usize = 32;

#[derive(Debug)]
pub struct Test262PathExecution {
    pub path: String,
    pub result: anyhow::Result<Vec<Test262CaseResult>>,
    pub elapsed: Duration,
}

pub fn execute_paths(
    test262_dir: &Path,
    test_paths: &[String],
) -> anyhow::Result<(Vec<Test262PathExecution>, Duration)> {
    let jobs = configured_jobs()?.min(test_paths.len().max(1));
    let pool = ThreadPoolBuilder::new()
        .num_threads(jobs)
        .thread_name(|index| format!("rsqjs-test262-{index}"))
        .build()
        .context("failed to build the Test262 worker pool")?;
    let wall_timer = timing::RunTimer::start();
    let mut executions = pool.install(|| {
        test_paths
            .par_iter()
            .enumerate()
            .map(|(index, path)| {
                let timed = timing::timed(|| execute_test262_path(test262_dir, path));
                (
                    index,
                    Test262PathExecution {
                        path: path.clone(),
                        result: timed.value,
                        elapsed: timed.elapsed,
                    },
                )
            })
            .collect::<Vec<_>>()
    });
    let wall_elapsed = wall_timer.elapsed();
    executions.sort_by_key(|(index, _)| *index);
    Ok((
        executions
            .into_iter()
            .map(|(_, execution)| execution)
            .collect(),
        wall_elapsed,
    ))
}

fn configured_jobs() -> anyhow::Result<usize> {
    let Some(value) = env::var_os(TEST_JOBS_ENV) else {
        return Ok(DEFAULT_TEST_JOBS);
    };
    let text = value.to_string_lossy();
    let jobs = text
        .parse::<usize>()
        .with_context(|| format!("{TEST_JOBS_ENV} must be a positive integer, got '{text}'"))?;
    ensure!(jobs > 0, "{TEST_JOBS_ENV} must be greater than zero");
    ensure!(
        jobs <= MAX_TEST_JOBS,
        "{TEST_JOBS_ENV} must not exceed {MAX_TEST_JOBS}"
    );
    Ok(jobs)
}
