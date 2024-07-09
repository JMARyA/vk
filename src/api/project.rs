use serde::{Deserialize, Serialize};

use super::User;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: isize,
    pub title: String,
    pub description: String,
    pub identifier: String,
    pub hex_color: String,
    pub parent_project_id: isize,
    pub default_bucket_id: Option<usize>,
    pub done_bucket_id: Option<usize>,
    pub owner: Option<User>,
    pub is_archived: bool,
    pub background_information: Option<String>,
    pub background_blur_hash: String,
    pub is_favorite: bool,
    pub position: f64,
    pub created: String,
    pub updated: String,
}
