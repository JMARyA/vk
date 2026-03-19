use argh::FromArgs;

#[derive(FromArgs, PartialEq, Debug)]
/// CLI Tool for Vikunja
pub struct VkCLI {
    #[argh(switch, short = 'd')]
    /// show done tasks too
    pub done: bool,

    #[argh(switch, short = 'f')]
    /// show only favorites
    pub favorite: bool,

    #[argh(option)]
    /// show only tasks from this project
    pub from: Option<String>,

    #[argh(option, short = 'l')]
    /// show only tasks with label
    pub label: Option<String>,

    #[argh(switch, short = 'j')]
    /// output as json
    pub json: bool,

    #[argh(subcommand)]
    pub cmd: Option<VkCommands>,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
pub enum VkCommands {
    TaskInfo(TaskInfoCmd),
    TaskEdit(TaskEditCmd),
    TaskRemove(TaskRemoveCmd),
    Stats(StatsCmd),
    TaskDone(TaskDoneCmd),
    TaskNew(TaskNewCmd),
    TaskAssign(TaskAssignCmd),
    TaskComments(TaskCommentsCmd),
    TaskComment(TaskCommentCmd),
    TaskRelation(TaskRelationCmd),
    TaskLabel(TaskLabelCmd),
    TaskFav(TaskFavCmd),
    TaskCheck(TaskCheckCmd),
    Login(LoginCmd),
    ProjectCmds(ProjectCmds),
    Labels(LabelCmds),
}

#[derive(FromArgs, PartialEq, Debug)]
/// Show information on task
#[argh(subcommand, name = "info")]
pub struct TaskInfoCmd {
    #[argh(switch, short = 'j')]
    /// output in json
    pub json: bool,

    #[argh(positional)]
    /// task id
    pub task_id: i32,
}

/// Edit a task
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "edit")]
pub struct TaskEditCmd {
    /// task ID
    #[argh(positional)]
    pub task_id: i32,

    /// new title
    #[argh(option)]
    pub title: Option<String>,

    /// new description
    #[argh(option)]
    pub description: Option<String>,

    /// new due date
    #[argh(option)]
    pub due: Option<String>,

    /// new priority
    #[argh(option)]
    pub priority: Option<String>,
}

#[derive(FromArgs, PartialEq, Debug)]
/// Delete a task
#[argh(subcommand, name = "rm")]
pub struct TaskRemoveCmd {
    #[argh(positional)]
    /// task id
    pub task_id: i32,
}

#[derive(FromArgs, PartialEq, Debug)]
/// Mark task as done
#[argh(subcommand, name = "done")]
pub struct TaskDoneCmd {
    #[argh(switch, short = 'u')]
    /// ndo completing the task
    pub undo: bool,

    #[argh(positional)]
    /// task id
    pub task_id: i32,
}

/// Create a new task
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "new")]
pub struct TaskNewCmd {
    /// task title
    #[argh(positional)]
    pub title: String,

    /// project to add task to
    #[argh(option, default = "String::from(\"Inbox\")")]
    pub project: String,

    /// task description
    #[argh(option)]
    pub description: Option<String>,

    /// task due
    #[argh(option)]
    pub due: Option<String>,

    /// task label
    #[argh(option)]
    pub label: Option<String>,

    /// task priority
    #[argh(option)]
    pub priority: Option<String>,

    /// mark task as favorite
    #[argh(switch)]
    pub favorite: bool,
}

/// Get a JWT Token for authentication
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "login")]
pub struct LoginCmd {
    /// username
    #[argh(option)]
    pub username: String,

    /// password
    #[argh(option)]
    pub password: String,

    /// vikunja host
    #[argh(option)]
    pub host: String,

    /// TOTP code
    #[argh(option)]
    pub totp: Option<String>,
}

/// Assign a user to a task
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "assign")]
pub struct TaskAssignCmd {
    /// remove user from task
    #[argh(switch)]
    pub undo: bool,

    /// user
    #[argh(positional)]
    pub user: String,

    /// task ID
    #[argh(positional)]
    pub task_id: i32,
}

/// Show task comments
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "comments")]
pub struct TaskCommentsCmd {
    #[argh(switch, short = 'j')]
    /// output as json
    pub json: bool,

    /// task ID
    #[argh(positional)]
    pub task_id: i32,
}

/// Comment on a task
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "comment")]
pub struct TaskCommentCmd {
    /// task ID
    #[argh(positional)]
    pub task_id: i32,

    /// comment text (opens $EDITOR if omitted)
    #[argh(positional)]
    pub comment: Option<String>,
}

/// Set task relations
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "relation")]
pub struct TaskRelationCmd {
    /// delete the relation
    #[argh(switch)]
    pub delete: bool,

    /// task ID
    #[argh(positional)]
    pub task_id: i32,

    /// relation
    #[argh(positional)]
    pub relation: String,

    /// other task ID
    #[argh(positional)]
    pub second_task_id: i32,
}

/// Favorite a task
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "fav")]
pub struct TaskFavCmd {
    /// remove favorite from task
    #[argh(switch)]
    pub undo: bool,

    /// task ID
    #[argh(positional)]
    pub task_id: i32,
}

/// Add a label to a task
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "label")]
pub struct TaskLabelCmd {
    /// remove label from task
    #[argh(switch)]
    pub undo: bool,

    /// label
    #[argh(positional)]
    pub label: String,

    /// task ID
    #[argh(positional)]
    pub task_id: i32,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "prj")]
/// Commands for projects
pub struct ProjectCmds {
    #[argh(subcommand)]
    pub cmd: ProjectCommands,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
pub enum ProjectCommands {
    List(ProjectListCmd),
    Add(ProjectAddCmd),
    Remove(ProjectRemoveCmd),
}

/// List projects
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "ls")]
pub struct ProjectListCmd {
    #[argh(switch, short = 'j')]
    /// output as json
    pub json: bool,
}

/// Create a new project
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "add")]
pub struct ProjectAddCmd {
    /// HEX color code for the project
    #[argh(option)]
    pub color: Option<String>,

    /// project description
    #[argh(option)]
    pub description: Option<String>,

    /// parent project
    #[argh(option)]
    pub parent: Option<String>,

    /// project title
    #[argh(positional)]
    pub title: String,
}

/// Remove a project
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "rm")]
pub struct ProjectRemoveCmd {
    /// project
    #[argh(positional)]
    pub project: String,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "labels")]
/// Commands for labels
pub struct LabelCmds {
    #[argh(subcommand)]
    pub cmd: LabelCommands,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
pub enum LabelCommands {
    List(LabelListCmd),
    New(LabelNewCmd),
    Remove(LabelRemoveCmd),
}

/// List all labels
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "ls")]
pub struct LabelListCmd {
    #[argh(switch, short = 'j')]
    /// output as json
    pub json: bool,
}

/// Create a new label
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "new")]
pub struct LabelNewCmd {
    /// HEX color code for the label
    #[argh(option)]
    pub color: Option<String>,

    /// description for the label
    #[argh(option)]
    pub description: Option<String>,

    /// label title
    #[argh(positional)]
    pub title: String,
}

/// Remove a label
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "rm")]
pub struct LabelRemoveCmd {
    /// label title
    #[argh(positional)]
    pub title: String,
}

/// Interactively toggle subtasks of a task
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "check")]
pub struct TaskCheckCmd {
    /// task ID
    #[argh(positional)]
    pub task_id: i32,
}

/// Show stats dashboard
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "stats")]
pub struct StatsCmd {}

pub fn get_args() -> VkCLI {
    argh::from_env()
}
