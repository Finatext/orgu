use std::ffi::OsStr;

use crate::events::CheckRequest;

pub type JobEnv = Vec<Entry>;

#[derive(Debug, Clone)]
pub struct Entry {
    pub key: String,
    pub value: String,
    pub secret: bool,
}

pub fn build_job_env(req: &CheckRequest, token: &str, job_name: &str) -> JobEnv {
    let mut vars = vec![
        env("GITHUB_TOKEN", token, true),
        // Reviewdog env vars. https://github.com/reviewdog/reviewdog?tab=readme-ov-file#jenkins-with-github-pull-request-builder-plugin
        env("REVIEWDOG_GITHUB_API_TOKEN", token, true),
        env("REVIEWDOG_SKIP_DOGHOUSE", "true", false),
        env("JOB_NAME", job_name, false),
        env("CI_COMMIT", req.head_sha.clone(), false),
        env("CI_REPO_OWNER", req.repository.owner.login.clone(), false),
        env("CI_REPO_NAME", req.repository.name.clone(), false),
        env(
            "CI_PULL_REQUEST",
            req.pull_request_number
                .map(|n| n.to_string())
                .unwrap_or_default(),
            false,
        ),
        // Other useful env vars.
        env("CI_DELIVERY_ID", req.delivery_id.clone(), false),
        env("CI_REQUEST_ID", req.request_id.clone(), false),
        env("CI_EVENT_NAME", req.event_name.clone(), false),
        env("CI_EVENT_ACTION", req.action.clone(), false),
        env("CI_HEAD", req.head_sha.clone(), false),
        env(
            "CI_HEAD_REF",
            req.pull_request_head_ref.clone().unwrap_or_default(),
            false,
        ),
        env("CI_BASE", req.base_sha.clone().unwrap_or_default(), false),
        env(
            "CI_BASE_REF",
            req.base_ref.clone().unwrap_or_default(),
            false,
        ),
        env("CI_BEFORE", req.before.clone().unwrap_or_default(), false),
        env("CI_AFTER", req.after.clone().unwrap_or_default(), false),
    ];

    // Job can refer custom properties as env vars with `CUSTOM_PROP_` prefix with upcased key.
    // e.g. `CUSTOM_PROP_TEAM=t-ferris`.
    for (k, v) in req.repository.custom_properties.iter() {
        let upcased = k.to_uppercase();
        vars.push(env(format!("CUSTOM_PROP_{upcased}"), v, false));
    }
    vars
}

fn env<K, V>(key: K, value: V, secret: bool) -> Entry
where
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    Entry {
        key: key.as_ref().to_string_lossy().to_string(),
        value: value.as_ref().to_string_lossy().to_string(),
        secret,
    }
}
