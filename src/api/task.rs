use serde::{Deserialize, Serialize};

use super::{Label, User};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: isize,
    pub title: String,
    pub description: String,
    pub done: bool,
    pub done_at: String,
    pub due_date: String,
    pub reminders: Option<String>,
    pub project_id: isize,
    pub repeat_after: usize,
    pub repeat_mode: usize,
    pub priority: usize,
    pub start_date: String,
    pub end_date: String,
    pub assignees: Option<Vec<User>>,
    pub labels: Option<Vec<Label>>,
    pub hex_color: String,
    pub percent_done: f64,
    pub identifier: String,
    pub index: usize,
    // pub related_tasks
    // pub attachments
    pub cover_image_attachment_id: usize,
    pub is_favorite: bool,
    pub created: String,
    pub updated: String,
    pub bucket_id: usize,
    pub position: f64,
    pub kanban_position: f64,
    pub created_by: Option<User>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub author: User,
    pub comment: String,
    pub created: String,
    pub id: isize,
    pub updated: String,
}
