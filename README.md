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

Or set up the config manually with an API token:
```toml
host  = "https://vikunja.example.com"
token = "your-api-token"
```

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
vk assign user 42    # assign user to task
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

## Configuration

Full config reference with defaults:

```toml
host  = "https://vikunja.example.com"
token = "your-token"

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
