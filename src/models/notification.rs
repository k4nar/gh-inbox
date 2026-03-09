use serde::{Deserialize, Serialize};

/// A GitHub notification, parsed from the REST API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub reason: String,
    pub unread: bool,
    pub updated_at: String,
    pub subject: Subject,
    pub repository: Repository,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    pub title: String,
    pub url: Option<String>,
    #[serde(rename = "type")]
    pub subject_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub full_name: String,
}
