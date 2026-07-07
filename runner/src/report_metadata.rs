use std::env;

const TIMESTAMP_ENV: &str = "RSQJS_REPORT_TIMESTAMP";
const COMMIT_ENV: &str = "RSQJS_REPORT_COMMIT_SHA";
const TREE_ENV: &str = "RSQJS_REPORT_TREE_SHA";
const EVENT_ENV: &str = "RSQJS_REPORT_EVENT_NAME";
const RUN_ID_ENV: &str = "RSQJS_REPORT_RUN_ID";
const RUN_ATTEMPT_ENV: &str = "RSQJS_REPORT_RUN_ATTEMPT";
const REPOSITORY_ENV: &str = "RSQJS_REPORT_REPOSITORY";
const WORKFLOW_ENV: &str = "RSQJS_REPORT_WORKFLOW";
const PR_NUMBER_ENV: &str = "RSQJS_REPORT_PR_NUMBER";
const TASK_ENV: &str = "RSQJS_REPORT_TASK";

#[derive(Debug, Clone, Default)]
pub struct RunMetadata {
    timestamp: String,
    commit: String,
    tree: String,
    event: String,
    run_id: String,
    run_attempt: String,
    repository: String,
    workflow: String,
    pull_request: String,
    task: String,
}

impl RunMetadata {
    pub fn from_env() -> Self {
        Self {
            timestamp: env::var(TIMESTAMP_ENV).unwrap_or_default(),
            commit: env::var(COMMIT_ENV).unwrap_or_default(),
            tree: env::var(TREE_ENV).unwrap_or_default(),
            event: env::var(EVENT_ENV).unwrap_or_default(),
            run_id: env::var(RUN_ID_ENV).unwrap_or_default(),
            run_attempt: env::var(RUN_ATTEMPT_ENV).unwrap_or_default(),
            repository: env::var(REPOSITORY_ENV).unwrap_or_default(),
            workflow: env::var(WORKFLOW_ENV).unwrap_or_default(),
            pull_request: env::var(PR_NUMBER_ENV).unwrap_or_default(),
            task: env::var(TASK_ENV).unwrap_or_default(),
        }
    }

    const fn has_values(&self) -> bool {
        !self.timestamp.is_empty()
            || !self.commit.is_empty()
            || !self.tree.is_empty()
            || !self.event.is_empty()
            || !self.run_id.is_empty()
            || !self.workflow.is_empty()
            || !self.pull_request.is_empty()
            || !self.task.is_empty()
    }
}

pub fn render_section(metadata: &RunMetadata) -> Vec<String> {
    if !metadata.has_values() {
        return Vec::new();
    }
    let mut lines = vec!["## Run Metadata".to_owned(), String::new()];
    push_metadata_line(&mut lines, "Generated at", &metadata.timestamp);
    push_metadata_line(&mut lines, "Event", &metadata.event);
    push_metadata_line(&mut lines, "Workflow", &metadata.workflow);
    push_metadata_line(&mut lines, "Task", &metadata.task);
    if !metadata.pull_request.is_empty() {
        lines.push(format!("- Pull request: #{}", metadata.pull_request));
    }
    push_metadata_line(&mut lines, "Tested commit", &metadata.commit);
    push_metadata_line(&mut lines, "Tested tree", &metadata.tree);
    if !metadata.repository.is_empty() && !metadata.run_id.is_empty() {
        lines.push(format!("- Workflow run: {}", workflow_run_url(metadata)));
    }
    lines.push(String::new());
    lines
}

fn workflow_run_url(metadata: &RunMetadata) -> String {
    let mut url = format!(
        "https://github.com/{}/actions/runs/{}",
        metadata.repository, metadata.run_id
    );
    if !metadata.run_attempt.is_empty() {
        url.push_str("/attempts/");
        url.push_str(&metadata.run_attempt);
    }
    url
}

fn push_metadata_line(lines: &mut Vec<String>, label: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    lines.push(format!("- {label}: `{value}`"));
}
