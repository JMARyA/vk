use clap::{arg, command};

pub fn get_args() -> clap::ArgMatches {
    command!()
        .about("CLI Tool for Vikunja")
        .arg(arg!(-d --done "Show done tasks too").required(false))
        .arg(arg!(-f --favorite "Show only favorites").required(false))
        .arg(arg!(--from <project> "Show only tasks from project").required(false))
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
        )
        .get_matches()
}
