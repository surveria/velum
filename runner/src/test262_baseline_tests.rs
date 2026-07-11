use std::collections::BTreeSet;

use super::comparison_report;
use crate::{STATUS_FAILED, STATUS_PASSED};

type TestResult = anyhow::Result<()>;

#[test]
fn accepts_new_passes_but_rejects_regressions() -> TestResult {
    let expected = BTreeSet::from([String::from("kept#strict"), String::from("lost#strict")]);
    let current = BTreeSet::from([String::from("kept#strict"), String::from("new#strict")]);
    let report = comparison_report(&expected, &current);
    if report.stats.passed != 2 || report.stats.failed != 1 || report.stats.total != 3 {
        anyhow::bail!("baseline comparison counts do not separate improvements from regressions");
    }
    let lost = report
        .rows
        .iter()
        .find(|row| row.case == "lost#strict")
        .ok_or_else(|| anyhow::anyhow!("missing regression row"))?;
    let added = report
        .rows
        .iter()
        .find(|row| row.case == "new#strict")
        .ok_or_else(|| anyhow::anyhow!("missing improvement row"))?;
    if lost.status != STATUS_FAILED || added.status != STATUS_PASSED {
        anyhow::bail!("baseline comparison assigned the wrong row status");
    }
    Ok(())
}
