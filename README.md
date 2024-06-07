# vk
`vk` is a command line todo tool for Vikunja.

## Setup
vk saves it's configuration at `$HOME/.config/vk.toml`.

To log in to your Vikunja Instance you can either use a API Token which you have to create manually or sign in using:
```shell
vk login --username user --password somepass --totp code --host vikunja.example.com
```

## Usage

**Show your current todos:**
```shell
# Just this
vk

# See done tasks as well
vk -d
vk --done

# Show favorites only
vk -f
vk --favorite

# Show tasks from specific project
vk --from myproject

# Show tasks which have a label
vk -l label
vk --label label
```

**Working with tasks:**
```shell
# Create a task
vk new mytask

# Task Detail View
vk info 42 # Tasks are referenced by their ID

# Remove a task
vk rm 42

# Mark as done
vk done 42
vk done -u 42 # You can undo this

# Mark as favorite
vk fav 42
vk fav -u 42 # Undo

# Assign a user to a task
vk assign me 42
vk assign -u me 42 # Undo
```

**Working with projects:**
```shell
# List your projects
vk prj ls

# Create a new project
vk prj add MyPrj --description "My project"

# Remove a project
vk prj rm MyPrj
```

**Working with labels:**
```shell
# Assign a label to a task
vk label mylabel 42
vk label -u mylabel 42 # Undo as well

# List your labels
vk labels ls

# Create a new label
vk labels new mylabel

# Remove a label
vk labels rm mylabel
```

**Working with comments:**
```shell
# Show comments of task
vk comments 42

# Comment on a task
vk comment 42 "my comment"
```

**Relations:**
```shell
# Make Task #42 be a parent task to #7
vk relation 7 parent 42

# Make #42 blocked by #7
vk relation 42 blocked 7
```
