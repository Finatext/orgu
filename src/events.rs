use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckRequest {
    // Request id is unique for each event including re-delivery.
    pub request_id: String,
    // Delivery id has same value for re-delivery.
    pub delivery_id: String,
    pub event_name: String,
    pub action: String,
    pub repository: GithubRepository,
    pub head_sha: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub pull_request_number: Option<u64>,
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
