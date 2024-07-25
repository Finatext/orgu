use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckRequest {
    // Request id is unique for each event including re-delivery.
    pub request_id: String,
    // Delivery id has same value for re-delivery.
    pub delivery_id: String,
    /// Name of the event.
    pub event_name: String,
    /// Action of the event.
    pub action: String,
    /// GitHub repository.
    pub repository: GithubRepository,
    /// SHA of the head commit.
    pub head_sha: String,
    /// SHA of the base commit. Always available for pull_request events. Mostly available for check_suite events.
    pub base_sha: Option<String>,
    /// Git reference of the base commit. None for check_suite events.
    pub base_ref: Option<String>,
    /// HEAD SHA of the commit before the push/synchronization.
    pub before: Option<String>,
    /// HEAD SHA of the commit after the push/synchronization. Mostly it is HEAD SHA of the branch.
    pub after: Option<String>,
    /// Pull request number if the event is associated with a pull request. check_suite events can be associated with
    /// multiple PRs and if so, this will be the first PR number.
    pub pull_request_number: Option<u64>,
    /// User who triggered the event.
    pub sender: User,
}

// Add prefix to avoid conflict with actual Git repository.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GithubRepository {
    /// Full name of the repository, e.g. "octocat/hello-world".
    pub full_name: String,
    /// Name of the repository, e.g. "hello-world".
    pub name: String,
    pub private: bool,
    pub owner: User,
    pub custom_properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct User {
    /// Name of the user or organization e.g. "octocat".
    pub login: String,
}
