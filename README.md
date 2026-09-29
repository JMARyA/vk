# vk

A fast, opinionated terminal interface for [Vikunja](https://vikunja.io). Built for people who live in the terminal.

```
  ◆  Fix authentication bug           [Hydra]      ★   2d ago
  ◆  Update README                    [vk]              just now
  ◆  Design CLI vision                [vk]          ★   just now
  ◆  vikunja-rs patches               [Sidequest]       overdue 2d
```

## Install

```shell
cargo install --git https://git.hydrar.de/jmarya/vk
```

## Setup

vk stores its config at `~/.config/vk.toml`.

Login with your credentials:
```shell
vk login --host vikunja.example.com --username user --password pass
vk login --host vikunja.example.com --username user --password pass --totp 123456
```

Or with an API token. API tokens cannot tell vk which user they belong to, so
pass your user id too (needed for `vk claim` and `vk --mine`):
```shell
vk login --host vikunja.example.com --token tk_... --user-id 1
```

Both check the credentials before saving them. A password login records your
user id by itself.

## Usage

```shell
vk                   # your tasks
vk -d                # include done tasks
vk -f                # favorites only
vk --from myproject  # tasks from a specific project
vk -l mylabel        # tasks with a specific label
vk stats             # dashboard with stats overview
```

**Tasks:**
```shell
vk new "fix the bug"                         # create in default project
vk new "fix the bug" --project myproject     # create in specific project
vk new "fix the bug" --due 2024-12-31        # with due date
vk new "fix the bug" --label urgent          # with label
vk new "fix the bug" --priority 4           # with priority

vk info 42           # full task detail
vk context 42        # everything about a task as markdown (see below)
vk edit 42           # edit a task
vk done 42           # mark as done
vk done -u 42        # undo
vk fav 42            # mark as favorite
vk fav -u 42         # undo
vk rm 42             # delete
```

**Comments:**
```shell
vk comments 42       # show comments
vk comment 42 "text" # post a comment
```

**Relations:**
```shell
vk relation 7 parent 42    # make #42 a parent of #7
vk relation 42 blocked 7   # mark #42 as blocked by #7
vk relation 42 sub 7       # make #7 a subtask of #42
vk relation --delete 42 blocked 7
```

**Assignments:**
```shell
vk assign user 42    # assign user to task (username or numeric id)
vk assign -u user 42 # unassign
```

**Labels:**
```shell
vk label urgent 42   # add label to task
vk label -u urgent 42

vk labels ls
vk labels new urgent --color ff0000
vk labels rm urgent
```

**Projects:**
```shell
vk prj ls
vk prj add "My Project" --description "..." --color 8800ff
vk prj add "Sub Project" --parent "My Project"
vk prj rm "My Project"
```

**Timeline:**
```shell
vk timeline              # every task, laid out by its dates
vk timeline -s           # only tasks with a start, end, due date or reminder
vk timeline -p myproject # one project
vk timeline -l mylabel -d
vk timeline --sort newest  # time (default), newest, project, due, activity
```

Rows take their project's colour (or a stable one from a built-in palette when
the project has none). A darker `▄` lifeline runs from when a task was created
until it was done (or today), lighter at its last update. On top of it,
`start`–`end` is drawn as a taller `▆` bar in full colour, `◆` marks the due date (red when overdue), `◇` its projected repeats,
`◷` reminders and `✓` when it was done. Keys: `j`/`k` select, `h`/`l` scroll, `H`/`L` page,
`+`/`-` zoom (6 hours, day, week, month per column), `s` cycle sort, `t` today, `c` center on
the selected task, `enter` show its details, `q` quit.

## Scripts and agents

vk is built to be driven by scripts and AI agents as well as by hand.

**Read a task in one call:**
```shell
vk context 42              # fields, description, checklist, relations, comments as markdown
vk changes --since 12h     # tasks created, done or updated since then (30m, 3d, 2w, or a date)
vk changes -p myproject --since 2026-09-28
```

**JSON everywhere.** Add `--json` (or `-j`) anywhere on the command line, or set
`VK_JSON=1`. Commands that change something print the result: `vk new --json`
returns the created task with its id, `vk rm --json` returns
`{"deleted": {"task": 42}}`.

**Errors and exit codes.** Failures go to stderr, as
`{"error": {"kind": "...", "message": "..."}}` in JSON mode.

| Exit | Kind        | Meaning                                              |
|------|-------------|------------------------------------------------------|
| 0    |             | success                                              |
| 1    | `other`     | anything else                                        |
| 2    | `usage`     | bad arguments or input, or a command that needs a terminal |
| 3    | `not_found` | no such task, project, label or user                 |
| 4    | `auth`      | not logged in, or the token lacks permission         |
| 5    | `conflict`  | the task changed since you read it, or someone else claimed it |
| 6    | `api`       | the server rejected the request                      |
| 7    | `network`   | the server could not be reached                      |

**Don't overwrite other people's changes.** Every command that changes a task
takes `--expect-updated <time>`: pass the task's `updated` value from when you
read it, and vk refuses with exit 5 if the task has changed since.
```shell
vk edit 42 --title "New title" --expect-updated 2026-09-29T10:00:00+02:00
```

**Preview before writing.** `--dry-run` (`-n`) on every command that changes
something shows what would happen without doing it:
```shell
$ vk edit 42 --title "New title" --priority 3 -n
Would edit task #42:
  priority: 0 → 3
  title: Old title → New title
```

**Long text from stdin.** `-` reads a description or comment from stdin:
```shell
generate-notes | vk comment 42 -
vk edit 42 --description - < notes.md
```

**Claiming tasks.** When several agents (or you and an agent) share a board:
```shell
vk claim 42          # assign yourself; exit 5 if someone else has it
vk claim 42 --force  # claim it alongside whoever has it
vk unclaim 42        # release it
vk --mine            # what you have claimed
```
Claiming a task you already hold, or releasing one you don't, succeeds without
changing anything, so retries are safe. If two agents claim the same task at
the same moment, the one with the lower user id keeps it.

vk needs to know which user it acts as. `vk login` saves it for password
logins; API tokens need `--user-id` at login (or `VK_USER_ID`, e.g. one per
agent). Your id is `created_by.id` on a task you created:
`vk info <id> --json`.

## Configuration

Full config reference with defaults:

```toml
host  = "https://vikunja.example.com"
token = "your-token"
user_id = 1                   # who you are, for `vk claim`; needed with API tokens

# display
bullet          = "◆"        # task bullet — any string works
show_id         = false       # show task ID in the list
show_age        = true        # show relative timestamp
show_labels     = false       # show label chips in the task list
date_format     = "relative"  # "relative" | "absolute" | "hidden"

# behaviour
default_view    = "tasks"     # what `vk` shows with no args: "tasks" | "stats"
default_project = "Inbox"     # project used by `vk new` when --project is omitted
page_size       = 25          # tasks shown in list view
sort_by         = "created"   # "created" | "due" | "priority" | "updated"
order           = "desc"      # "asc" | "desc"

# vk stats
stats_logo      = true        # show ASCII logo in stats view
```
