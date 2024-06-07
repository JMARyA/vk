use std::collections::HashMap;

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
    pub related_tasks: Option<HashMap<String, Vec<Task>>>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRelation {
    pub created: String,
    pub created_by: User,
    pub other_task_id: isize,
    pub task_id: isize,
    pub relation_kind: String,
}

pub enum Relation {
    Unknown,
    Subtask,
    ParentTask,
    Related,
    DuplicateOf,
    Duplicates,
    Blocking,
    Blocked,
    Precedes,
    Follows,
    CopiedFrom,
    CopiedTo,
}

impl Relation {
    pub fn try_parse(val: &str) -> Option<Self> {
        match val {
            "unknown" => Some(Self::Unknown),
            "subtask" | "sub" => Some(Self::Subtask),
            "parenttask" | "parent" => Some(Self::ParentTask),
            "related" => Some(Self::Related),
            "duplicateof" => Some(Self::DuplicateOf),
            "duplicates" => Some(Self::Duplicates),
            "blocking" => Some(Self::Blocking),
            "blocked" => Some(Self::Blocked),
            "precedes" => Some(Self::Precedes),
            "follows" => Some(Self::Follows),
            "copiedfrom" => Some(Self::CopiedFrom),
            "copiedto" => Some(Self::CopiedTo),
            _ => None,
        }
    }

    pub fn repr(&self) -> String {
        match self {
            Relation::Unknown => "Unknown",
            Relation::Subtask => "Subtask",
            Relation::ParentTask => "Parent Task",
            Relation::Related => "Related",
            Relation::DuplicateOf => "Duplicate of",
            Relation::Duplicates => "Duplicates",
            Relation::Blocking => "Blocking",
            Relation::Blocked => "Blocked by",
            Relation::Precedes => "Precedes",
            Relation::Follows => "Follows",
            Relation::CopiedFrom => "Copied from",
            Relation::CopiedTo => "Copied to",
        }
        .to_string()
    }

    pub fn api(&self) -> String {
        match self {
            Relation::Unknown => "unknown",
            Relation::Subtask => "subtask",
            Relation::ParentTask => "parenttask",
            Relation::Related => "related",
            Relation::DuplicateOf => "duplicateof",
            Relation::Duplicates => "duplicates",
            Relation::Blocking => "blocking",
            Relation::Blocked => "blocked",
            Relation::Precedes => "precedes",
            Relation::Follows => "follows",
            Relation::CopiedFrom => "copiedfrom",
            Relation::CopiedTo => "copiedto",
        }
        .to_string()
    }
}
