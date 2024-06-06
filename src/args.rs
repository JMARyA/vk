use clap::{arg, command};

pub fn get_args() -> clap::ArgMatches {
    command!()
        .about("CLI Tool for Vikunja")
        .arg(arg!(-d --done "Show done tasks too").required(false))
        .arg(arg!(-f --favorite "Show only favorites").required(false))
        .arg(arg!(--from <project> "Show only tasks from project").required(false))
        .arg(arg!(-l --label <label> "Show only tasks with label").required(false))
        .subcommand(
            command!()
                .name("info")
                .about("Show information on task")
                .arg(arg!([task_id] "Task ID").required(true)),
        )
        .subcommand(
            command!()
                .name("prj")
                .about("Commands about projects")
                .subcommand(command!().name("ls").about("List projects")),
            // todo : prj add <project>
            // todo : prj rm <project>
        )
        .subcommand(
            command!()
                .name("new")
                .about("Create a new task")
                .arg(arg!([title] "Task title").required(true))
                .arg(
                    arg!(-p --project <project> "Project to add task to")
                        .required(false)
                        .default_value("Inbox"),
                ),
        )
        .subcommand(
            command!()
                .name("label")
                .about("Add a label to a task")
                .arg(arg!(-u --undo "Remove label from task").required(false))
                .arg(arg!([label] "Label").required(true))
                .arg(arg!([task_id] "Task ID").required(true)),
        )
        .subcommand(
            command!()
                .name("labels")
                .about("Manage labels")
                .subcommand(command!().name("ls").about("List all labels"))
                .subcommand(
                    command!()
                        .name("new")
                        .about("Create a new label")
                        .arg(
                            arg!(-c --color <color> "HEX Color Code for the label").required(false),
                        )
                        .arg(
                            arg!(-d --description <description> "Description for the label")
                                .required(false),
                        )
                        .arg(arg!(<title> "Label title").required(true)),
                )
                .subcommand(
                    command!()
                        .name("rm")
                        .about("Remove a label")
                        .arg(arg!(<title> "Label title").required(true)),
                ),
        )
        .subcommand(
            command!()
                .name("done")
                .about("Mark task as done")
                .arg(arg!([task_id] "Task ID").required(true)),
        )
        .get_matches()
}
