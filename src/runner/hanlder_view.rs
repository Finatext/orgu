use std::process::Output;

use humantime::Duration;
use octorust::types::{
    ChecksCreateRequest, ChecksCreateRequestConclusion, ChecksCreateRequestOutput,
    ChecksUpdateRequest, ChecksUpdateRequestOutput, JobStatus,
};
use tokio::process::Command;

use crate::{events::CheckRequest, runner::job_env::JobEnv};

#[derive(Debug, Clone)]
pub struct CreateInput {
    pub req: CheckRequest,
    pub name: String,
    pub command: Vec<String>,
}

impl From<CreateInput> for ChecksCreateRequest {
    fn from(v: CreateInput) -> Self {
        Self {
            name: v.name,
            head_sha: v.req.head_sha.clone(),
            status: Some(JobStatus::InProgress),
            conclusion: None,
            output: Some(ChecksCreateRequestOutput {
                title: "Runner is running job".to_owned(),
                summary: with_debug_info(
                    format!("Running command:\n```\n{}\n```", v.command.join(" ")),
                    &v.req,
                ),
                text: "".to_owned(),
                annotations: Vec::new(),
                images: Vec::new(),
            }),
            actions: Vec::new(),
            started_at: None,
            completed_at: None,
            details_url: String::new(),
            external_id: String::new(),
        }
    }
}

impl CreateInput {
    pub fn into_update_input(self, check_run_id: i64, wrap_stdout: bool) -> UpdateInputBase {
        UpdateInputBase {
            req: self.req,
            name: self.name,
            check_run_id,
            wrap_stdout,
            job_env: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpdateInputBase {
    pub check_run_id: i64,
    pub req: CheckRequest,
    pub name: String,
    pub wrap_stdout: bool,
    pub job_env: Option<JobEnv>,
}

impl UpdateInputBase {
    pub fn owner(&self) -> &str {
        &self.req.repository.owner.login
    }

    pub fn repo(&self) -> &str {
        &self.req.repository.name
    }

    pub fn into_checkout_timed_out(self, duration: Duration) -> ChecksUpdateRequest {
        let mut input = default_checks_update_request(&self);
        input.conclusion = Some(ChecksCreateRequestConclusion::TimedOut);
        input.output = input.output.map(|mut o| {
            "Checkout repository timed out".clone_into(&mut o.title);
            let summary = format!(
              "Runner tried to checkout repository but timed out ({duration}): owner={}, repo={}, sha={}",
              self.req.repository.owner.login,
              self.req.repository.name,
              self.req.head_sha,
            );
            o.summary = with_debug_info(summary, &self.req);
            o
        });
        input
    }

    pub fn into_command_timed_out(self, duration: Duration, cmd: Command) -> ChecksUpdateRequest {
        let mut input = default_checks_update_request(&self);
        input.conclusion = Some(ChecksCreateRequestConclusion::TimedOut);
        input.output = input.output.map(|mut o| {
            "Running job timed out".clone_into(&mut o.title);
            let summary = format!(
                "Job execution has timed out on the runner ({duration}): `{}`",
                fmt_cmd(&cmd)
            );
            o.summary = with_debug_info(summary, &self.req);
            o
        });
        input
    }

    pub fn into_command_succeeded(self, cmd: Command, out: &Output) -> ChecksUpdateRequest {
        let mut input = default_checks_update_request(&self);
        input.conclusion = Some(ChecksCreateRequestConclusion::Success);
        input.output = input.output.map(|mut o| {
            "Runner executed job successfully".clone_into(&mut o.title);
            o.summary =
                with_debug_info(format!("Command succeeded: `{}`", fmt_cmd(&cmd)), &self.req);
            o.text = self.to_text(out);
            o
        });
        input
    }

    pub fn into_command_failed(self, cmd: Command, out: &Output) -> ChecksUpdateRequest {
        let mut input = default_checks_update_request(&self);
        input.conclusion = Some(ChecksCreateRequestConclusion::Failure);
        input.output = input.output.map(|mut o| {
            "Runner ran job but it failed".clone_into(&mut o.title);
            o.summary = with_debug_info(
                format!("Command failed with {}: `{}`", out.status, fmt_cmd(&cmd)),
                &self.req,
            );
            o.text = self.to_text(out);
            o
        });
        input
    }

    pub fn into_event_handle_failed(self, error: &anyhow::Error) -> ChecksUpdateRequest {
        let mut input = default_checks_update_request(&self);
        input.conclusion = Some(ChecksCreateRequestConclusion::Failure);
        input.output = input.output.map(|mut o| {
            "Runner failed to handle event".clone_into(&mut o.title);
            o.summary = with_debug_info(
                "Event handling failed, contact operation team.".to_owned(),
                &self.req,
            );
            // Use Debug trait here to include ancestor errors.
            o.text = format!("Error:\n\n```\n{error:?}\n```");
            o
        });
        input
    }

    fn to_text(&self, out: &Output) -> String {
        let stdout = cut_text_length(&out.stdout);
        let stderr = cut_text_length(&out.stderr);
        let outs = if self.wrap_stdout {
            format!(
                "## stdout\n```\n{stdout}\n```\n## stderr\n```\n{stderr}\n```"
            )
        } else {
            format!("## stdout\n{stdout}\n## stderr\n{stderr}")
        };

        match &self.job_env {
            None => outs,
            Some(e) => {
                format!("## env\n```\n{}\n```\n{}", job_env_text(e), outs)
            }
        }
    }
}

fn job_env_text(job_env: &JobEnv) -> String {
    job_env
        .iter()
        .map(|e| {
            if e.secret {
                format!("{}: {}", e.key, "*".repeat(e.value.len()))
            } else {
                format!("{}: {}", e.key, e.value)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn fmt_cmd(cmd: &Command) -> String {
    let c = cmd.as_std();
    let mut s = vec![c.get_program()];
    s.extend(c.get_args());
    s.into_iter()
        .map(|s| s.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

fn default_checks_update_request(base: &UpdateInputBase) -> ChecksUpdateRequest {
    ChecksUpdateRequest {
        name: base.name.clone(),
        status: Some(JobStatus::Completed),
        conclusion: Default::default(),
        output: Some(ChecksUpdateRequestOutput {
            title: Default::default(),
            summary: Default::default(),
            text: Default::default(),
            annotations: Default::default(),
            images: Default::default(),
        }),
        actions: Default::default(),
        completed_at: Default::default(),
        started_at: Default::default(),
        details_url: Default::default(),
        external_id: Default::default(),
    }
}

fn with_debug_info(original: String, req: &CheckRequest) -> String {
    format!(
        "{original}\n\nDelivery ID (not unique for re-delivery): `{}`\nRequest ID (unique for re-delivery): `{}`",
        req.delivery_id, req.request_id,
    )
}

// GitHub API has a limit of 65535 characters for text fields. So cut the text if it's too long.
// https://docs.github.com/en/rest/checks/runs?apiVersion=2022-11-28#create-a-check-run
const MAX_TEXT_LENGTH: usize = 30_000;
fn cut_text_length(v: &[u8]) -> String {
    let s = String::from_utf8_lossy(v);
    if s.chars().count() > MAX_TEXT_LENGTH {
        format!("{}...", s.chars().take(MAX_TEXT_LENGTH).collect::<String>())
    } else {
        s.to_string()
    }
}
