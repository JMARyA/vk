use vikunjars::models::ModelsRelationKind;

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
    pub fn model_rel(&self) -> ModelsRelationKind {
        match self {
            Relation::Unknown => ModelsRelationKind::RelationKindUnknown,
            Relation::Subtask => ModelsRelationKind::RelationKindSubtask,
            Relation::ParentTask => ModelsRelationKind::RelationKindParenttask,
            Relation::Related => ModelsRelationKind::RelationKindRelated,
            Relation::DuplicateOf => ModelsRelationKind::RelationKindDuplicateOf,
            Relation::Duplicates => ModelsRelationKind::RelationKindDuplicates,
            Relation::Blocking => ModelsRelationKind::RelationKindBlocking,
            Relation::Blocked => ModelsRelationKind::RelationKindBlocked,
            Relation::Precedes => ModelsRelationKind::RelationKindPreceeds,
            Relation::Follows => ModelsRelationKind::RelationKindFollows,
            Relation::CopiedFrom => ModelsRelationKind::RelationKindCopiedFrom,
            Relation::CopiedTo => ModelsRelationKind::RelationKindCopiedTo,
        }
    }

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
            Self::Unknown => "Unknown",
            Self::Subtask => "Subtask",
            Self::ParentTask => "Parent Task",
            Self::Related => "Related",
            Self::DuplicateOf => "Duplicate of",
            Self::Duplicates => "Duplicates",
            Self::Blocking => "Blocking",
            Self::Blocked => "Blocked by",
            Self::Precedes => "Precedes",
            Self::Follows => "Follows",
            Self::CopiedFrom => "Copied from",
            Self::CopiedTo => "Copied to",
        }
        .to_string()
    }

    pub fn api(&self) -> String {
        match self {
            Self::Unknown => "unknown",
            Self::Subtask => "subtask",
            Self::ParentTask => "parenttask",
            Self::Related => "related",
            Self::DuplicateOf => "duplicateof",
            Self::Duplicates => "duplicates",
            Self::Blocking => "blocking",
            Self::Blocked => "blocked",
            Self::Precedes => "precedes",
            Self::Follows => "follows",
            Self::CopiedFrom => "copiedfrom",
            Self::CopiedTo => "copiedto",
        }
        .to_string()
    }
}
