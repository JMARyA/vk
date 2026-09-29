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
    /// output JSON; accepted anywhere on the command line, or set VK_JSON=1
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
    Sync(SyncCmd),
    Timeline(TimelineCmd),
    Context(ContextCmd),
    Changes(ChangesCmd),
}

/// Everything about one task: fields, description, checklist, relations
/// and comments, as markdown (or JSON)
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "context")]
pub struct ContextCmd {
    /// task ID
    #[argh(positional)]
    pub task_id: i32,
}

/// Tasks created, completed or updated since a point in time
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "changes")]
pub struct ChangesCmd {
    /// how far back: 30m, 12h, 3d, 2w, or a date/time (default: 24h)
    #[argh(option, default = "String::from(\"24h\")")]
    pub since: String,

    /// only tasks in this project
    #[argh(option, short = 'p')]
    pub project: Option<String>,
}

#[derive(FromArgs, PartialEq, Debug)]
/// Sync vikunja tasks into a local markdown folder
#[argh(subcommand, name = "sync")]
pub struct SyncCmd {
    #[argh(option, short = 'o', default = "String::from(\"./local\")")]
    /// directory to write the synced notes into (default: ./local)
    pub output: String,

    #[argh(option, short = 'p')]
    /// only sync these projects, by title or id (repeatable; default: all)
    pub project: Vec<String>,

    #[argh(switch)]
    /// include tasks that are marked done
    pub done: bool,

    #[argh(switch)]
    /// include projects that are archived
    pub archived: bool,

    #[argh(switch, short = 'n')]
    /// report what would change without writing anything
    pub dry_run: bool,
}

#[derive(FromArgs, PartialEq, Debug)]
/// Show tasks on a timeline by their dates
#[argh(subcommand, name = "timeline")]
pub struct TimelineCmd {
    #[argh(option, short = 'p')]
    /// only show tasks from this project
    pub project: Option<String>,

    #[argh(option, short = 'l')]
    /// only show tasks with this label
    pub label: Option<String>,

    #[argh(switch, short = 'd')]
    /// include tasks that are marked done
    pub done: bool,

    #[argh(switch, short = 's')]
    /// only show tasks with a start, end, due date or reminder
    pub scheduled: bool,

    #[argh(option, default = "crate::ui::timeline::Sort::Time")]
    /// row order: time, newest, project, due or activity (default: time)
    pub sort: crate::ui::timeline::Sort,
}

#[derive(FromArgs, PartialEq, Debug)]
/// Show information on task
#[argh(subcommand, name = "info")]
pub struct TaskInfoCmd {
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

    /// new description in markdown; `-` reads it from stdin
    #[argh(option)]
    pub description: Option<String>,

    /// new due date
    #[argh(option)]
    pub due: Option<String>,

    /// new priority
    #[argh(option)]
    pub priority: Option<String>,

    #[argh(option)]
    /// only write if the task's `updated` time is still this (from a
    /// previous read); fails with exit code 5 if it changed
    pub expect_updated: Option<String>,

    #[argh(switch, short = 'n')]
    /// show what would change without changing anything
    pub dry_run: bool,
}

#[derive(FromArgs, PartialEq, Debug)]
/// Delete a task
#[argh(subcommand, name = "rm")]
pub struct TaskRemoveCmd {
    #[argh(positional)]
    /// task id
    pub task_id: i32,

    #[argh(option)]
    /// only write if the task's `updated` time is still this (from a
    /// previous read); fails with exit code 5 if it changed
    pub expect_updated: Option<String>,

    #[argh(switch, short = 'n')]
    /// show what would change without changing anything
    pub dry_run: bool,
}

#[derive(FromArgs, PartialEq, Debug)]
/// Mark task as done
#[argh(subcommand, name = "done")]
pub struct TaskDoneCmd {
    #[argh(switch, short = 'u')]
    /// undo completing the task
    pub undo: bool,

    #[argh(positional)]
    /// task id
    pub task_id: i32,

    #[argh(option)]
    /// only write if the task's `updated` time is still this (from a
    /// previous read); fails with exit code 5 if it changed
    pub expect_updated: Option<String>,

    #[argh(switch, short = 'n')]
    /// show what would change without changing anything
    pub dry_run: bool,
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

    /// task description in markdown; `-` reads it from stdin
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

    #[argh(switch, short = 'n')]
    /// show what would change without changing anything
    pub dry_run: bool,
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

    /// username, or numeric user id
    #[argh(positional)]
    pub user: String,

    /// task ID
    #[argh(positional)]
    pub task_id: i32,

    #[argh(option)]
    /// only write if the task's `updated` time is still this (from a
    /// previous read); fails with exit code 5 if it changed
    pub expect_updated: Option<String>,

    #[argh(switch, short = 'n')]
    /// show what would change without changing anything
    pub dry_run: bool,
}

/// Show task comments
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "comments")]
pub struct TaskCommentsCmd {
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

    /// comment text in markdown; `-` reads it from stdin, omitted opens
    /// $EDITOR
    #[argh(positional)]
    pub comment: Option<String>,

    #[argh(switch, short = 'n')]
    /// show what would change without changing anything
    pub dry_run: bool,
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

    #[argh(option)]
    /// only write if the task's `updated` time is still this (from a
    /// previous read); fails with exit code 5 if it changed
    pub expect_updated: Option<String>,

    #[argh(switch, short = 'n')]
    /// show what would change without changing anything
    pub dry_run: bool,
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

    #[argh(option)]
    /// only write if the task's `updated` time is still this (from a
    /// previous read); fails with exit code 5 if it changed
    pub expect_updated: Option<String>,

    #[argh(switch, short = 'n')]
    /// show what would change without changing anything
    pub dry_run: bool,
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

    #[argh(option)]
    /// only write if the task's `updated` time is still this (from a
    /// previous read); fails with exit code 5 if it changed
    pub expect_updated: Option<String>,

    #[argh(switch, short = 'n')]
    /// show what would change without changing anything
    pub dry_run: bool,
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
pub struct ProjectListCmd {}

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

    #[argh(switch, short = 'n')]
    /// show what would change without changing anything
    pub dry_run: bool,
}

/// Remove a project

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "rm")]
pub struct ProjectRemoveCmd {
    /// project
    #[argh(positional)]
    pub project: String,

    #[argh(switch, short = 'n')]
    /// show what would change without changing anything
    pub dry_run: bool,
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
pub struct LabelListCmd {}

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

    #[argh(switch, short = 'n')]
    /// show what would change without changing anything
    pub dry_run: bool,
}

/// Remove a label
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "rm")]
pub struct LabelRemoveCmd {
    /// label title
    #[argh(positional)]
    pub title: String,

    #[argh(switch, short = 'n')]
    /// show what would change without changing anything
    pub dry_run: bool,
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

/// Stands in for a bare `-` argument ("read from stdin"). argh rejects `-`
/// as a positional because it looks like a flag, so it is swapped for this
/// before parsing.
pub const STDIN_ARG: &str = "\u{1}stdin";

/// Split `--json`/`-j` out of the arguments. argh only accepts a flag where
/// it is declared, but JSON output applies to every command, so it is taken
/// from any position. Arguments after `--` are left alone.
fn take_json_flag(args: Vec<String>) -> (Vec<String>, bool) {
    let mut json = false;
    let mut rest = Vec::with_capacity(args.len());
    let mut literal = false;

    for arg in args {
        if !literal && (arg == "--json" || arg == "-j") {
            json = true;
            continue;
        }
        if !literal && arg == "-" {
            rest.push(STDIN_ARG.to_string());
            continue;
        }
        if arg == "--" {
            literal = true;
        }
        rest.push(arg);
    }

    (rest, json)
}

fn env_json() -> bool {
    std::env::var("VK_JSON").is_ok_and(|v| !v.is_empty() && v != "0" && v != "false")
}

/// Parse the command line. Returns the arguments and whether JSON output was
/// requested. Bad arguments exit with the usage code (2).
pub fn get_args() -> (VkCLI, bool) {
    let mut argv: Vec<String> = std::env::args().collect();
    let program = if argv.is_empty() {
        String::from("vk")
    } else {
        argv.remove(0)
    };
    let (rest, json_flag) = take_json_flag(argv);
    let json = json_flag || env_json();

    let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
    match VkCLI::from_args(&[&program], &rest) {
        Ok(cli) => (cli, json),
        Err(argh::EarlyExit { output, status }) => match status {
            Ok(()) => {
                println!("{output}");
                std::process::exit(0);
            }
            Err(()) => {
                let err = crate::error::VkError::usage(output.trim_end());
                err.report(json);
                std::process::exit(err.kind.exit_code());
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_json_flag_is_taken_from_any_position() {
        assert_eq!(
            take_json_flag(args(&["info", "42", "--json"])),
            (args(&["info", "42"]), true)
        );
        assert_eq!(
            take_json_flag(args(&["-j", "done", "7"])),
            (args(&["done", "7"]), true)
        );
        assert_eq!(
            take_json_flag(args(&["done", "7"])),
            (args(&["done", "7"]), false)
        );
    }

    #[test]
    fn test_bare_dash_becomes_stdin_marker() {
        assert_eq!(
            take_json_flag(args(&["comment", "42", "-"])),
            (args(&["comment", "42", STDIN_ARG]), false)
        );
        assert_eq!(
            take_json_flag(args(&["comment", "42", "--", "-"])),
            (args(&["comment", "42", "--", "-"]), false)
        );
    }

    #[test]
    fn test_json_flag_after_double_dash_is_literal() {
        assert_eq!(
            take_json_flag(args(&["comment", "42", "--", "--json"])),
            (args(&["comment", "42", "--", "--json"]), false)
        );
    }
}
