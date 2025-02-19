use serde::{Deserialize, Serialize};

use crate::events::{CheckRequest, GithubRepository, User};

// GitHub webhooks send a zero SHA value in some cases, such as when creating a draft PR. For non-draft PRs, GitHub
// webhooks send a null SHA value. Although this behavior has been reported as a bug, GitHub has stated that it is
// expected behavior. This inconsistency increases the complexity of handling events, so orgu addresses this
// inconsistency. The zero SHA value is treated as a null SHA value, and thus, the zero SHA value is replaced with
// the base SHA value.
const ZERO_SHA_VALUE: &str = "0000000000000000000000000000000000000000";

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
    // Current design limitation: if multiple PRs are associated with a check suite, re-running checks
    // for the specific PR may not be possible. This is rare case and pushing empty commit will be work-around for
    // that case.
    fn into_check_request(self, req_id: String, delivery_id: String) -> CheckRequest {
        let first_pr = self.check_suite.pull_requests.first();
        let before = self.before();

        CheckRequest {
            request_id: req_id,
            delivery_id,
            event_name: "check_suite".to_owned(),
            action: self.common.action,
            repository: self.common.repository,
            head_sha: self.check_suite.head_sha,
            base_sha: self.check_suite.before.clone(),
            base_ref: first_pr.map(|pr| pr.base.ref_.clone()),
            before,
            after: self.check_suite.after,
            pull_request_number: first_pr.map(|pr| pr.number),
            pull_request_head_ref: first_pr.map(|pr| pr.head.ref_.clone()),
            sender: self.common.sender,
        }
    }

    // If top level before is broken, then try to get it from the first PR.
    fn before(&self) -> Option<String> {
        self.check_suite
            .before
            .clone()
            .filter(|s| s != ZERO_SHA_VALUE)
            .or_else(|| {
                self.check_suite
                    .pull_requests
                    .first()
                    .map(|pr| pr.base.sha.clone())
            })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PullRequestEvent {
    #[serde(flatten)]
    pub common: WebhookCommonFields,
    /// The pull request number.
    pub number: u64,
    // Base sha for `pull_requst.opened` events.
    // Can be None for `check_suite` events.
    pub before: Option<String>,
    // Head sha for `pull_requst.opened` event.
    // Can be None for `check_suite` events.
    pub after: Option<String>,
    pub pull_request: PullRequest,
}

impl PullRequestEvent {
    // In PR open event, before and after are not available, so insert them from the base and head.
    fn before(&self) -> Option<String> {
        let before = self.before.clone().filter(|s| s != ZERO_SHA_VALUE);
        before.or_else(|| Some(self.pull_request.base.sha.clone()))
    }

    fn after(&self) -> Option<String> {
        self.after
            .clone()
            .or_else(|| Some(self.pull_request.head.sha.clone()))
    }

    fn into_check_request(self, req_id: String, delivery_id: String) -> CheckRequest {
        let before = self.before();
        let after = self.after();
        CheckRequest {
            request_id: req_id,
            delivery_id,
            event_name: "pull_request".to_owned(),
            action: self.common.action,
            repository: self.common.repository,
            head_sha: self.pull_request.head.sha,
            base_sha: Some(self.pull_request.base.sha),
            base_ref: Some(self.pull_request.base.ref_.clone()),
            before,
            after,
            pull_request_number: Some(self.number),
            pull_request_head_ref: Some(self.pull_request.head.ref_.clone()),
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
    pub head: Reference,
    pub base: Reference,
}

// https://docs.github.com/en/webhooks/webhook-events-and-payloads?actionType=synchronize#pull_request
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PullRequest {
    pub id: i64,
    pub head: Reference,
    pub base: Reference,
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

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn pull_request_before_zero_value() {
        let pr = PullRequestEvent {
            before: Some("0000000000000000000000000000000000000000".to_owned()),
            pull_request: PullRequest {
                base: Reference {
                    sha: "base_sha".to_owned(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(pr.before(), Some("base_sha".to_owned()));
    }

    #[test]
    fn pull_request_before_null_value() {
        let pr = PullRequestEvent {
            before: None,
            pull_request: PullRequest {
                base: Reference {
                    sha: "base_sha".to_owned(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(pr.before(), Some("base_sha".to_owned()));
    }

    #[test]
    fn pull_request_before_ok() {
        let pr = PullRequestEvent {
            before: Some("before_sha".to_owned()),
            ..Default::default()
        };
        assert_eq!(pr.before(), Some("before_sha".to_owned()));
    }
}
