use serde::{Deserialize, Serialize};

use crate::events::{CheckRequest, GithubRepository, User};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebhookCommonFields {
    pub action: String,
    pub repository: GithubRepository,
    pub sender: User,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum GithubEvent {
    // https://rust-lang.github.io/rust-clippy/master/index.html#/large_enum_variant
    CheckSuite(Box<CheckSuiteEvent>),
    PullRequest(Box<PullRequestEvent>),
}

impl GithubEvent {
    pub fn into_check_request(self, req_id: String, delivery_id: String) -> CheckRequest {
        match self {
            Self::CheckSuite(e) => e.into_check_request(req_id, delivery_id),
            Self::PullRequest(e) => e.into_check_request(req_id, delivery_id),
        }
    }

    pub fn head_sha(&self) -> &str {
        match self {
            Self::CheckSuite(e) => &e.check_suite.head_sha,
            Self::PullRequest(e) => &e.pull_request.head.sha,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckSuiteEvent {
    #[serde(flatten)]
    pub common: WebhookCommonFields,
    pub check_suite: CheckSuite,
}

impl CheckSuiteEvent {
    fn into_check_request(self, req_id: String, delivery_id: String) -> CheckRequest {
        CheckRequest {
            request_id: req_id,
            delivery_id,
            event_name: "check_suite".to_owned(),
            action: self.common.action,
            repository: self.common.repository,
            head_sha: self.check_suite.head_sha,
            before: self.check_suite.before,
            after: self.check_suite.after,
            // This is current design limitation: if multiple PRs are associated with a check suite, then retying checks
            // for specific PR may not be possible. This is rare case and pushing empty commit will be work-around for
            // that case.
            pull_request_number: self.check_suite.pull_requests.first().map(|pr| pr.number),
            sender: self.common.sender,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PullRequestEvent {
    #[serde(flatten)]
    pub common: WebhookCommonFields,
    /// The pull request number.
    pub number: u64,
    // None for `opened` action.
    pub before: Option<String>,
    // None for `opened` action.
    pub after: Option<String>,
    pub pull_request: PullRequest,
}

impl PullRequestEvent {
    fn into_check_request(self, req_id: String, delivery_id: String) -> CheckRequest {
        // In PR open event, before and after are not available, so insert them from the base and head.
        let before = self
            .before
            .or_else(|| self.pull_request.base.clone().map(|b| b.sha));
        let after = self
            .after
            .or_else(|| Some(self.pull_request.head.sha.clone()));
        CheckRequest {
            request_id: req_id,
            delivery_id,
            event_name: "pull_request".to_owned(),
            action: self.common.action,
            repository: self.common.repository,
            head_sha: self.pull_request.head.sha,
            before,
            after,
            pull_request_number: Some(self.number),
            sender: self.common.sender,
        }
    }
}

// https://docs.github.com/en/webhooks/webhook-events-and-payloads?actionType=requested#check_suite
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckSuite {
    pub id: i64,
    pub head_sha: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub pull_requests: Vec<CheckSuitePullRequest>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckSuitePullRequest {
    pub id: u64,
    pub number: u64,
}

// https://docs.github.com/en/webhooks/webhook-events-and-payloads?actionType=synchronize#pull_request
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PullRequest {
    pub id: i64,
    pub head: Reference,
    pub base: Option<Reference>,
    pub draft: bool,
    pub title: String,
    pub user: User,
    pub url: String,
    pub html_url: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Reference {
    #[serde(rename = "ref")]
    pub ref_: String,
    pub sha: String,
}
