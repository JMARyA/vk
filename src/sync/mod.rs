//! One-way sync of Vikunja projects and tasks into a local markdown tree.
//!
//! The sync is a pull: Vikunja is always the source of truth and local notes
//! are treated as a derived backup. To make repeated runs safe it records what
//! it wrote in a manifest ([`MANIFEST_FILE`]) at the root of the output
//! directory, and only ever rewrites or deletes files listed there. Anything
//! else in the output directory is left alone.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use vikunjars::models::{ModelsProject, ModelsTask};

use crate::api::VikunjaAPI;

/// Bookkeeping file recording which notes this tool owns.
pub const MANIFEST_FILE: &str = ".vk-sync.json";

/// Guards against a malformed parent chain looping forever.
const MAX_PROJECT_DEPTH: usize = 32;

// ---------------------------------------------------------------- naming ---

/// Replace characters that are not portable in file names.
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            // Basic cross-platform invalid characters
            match c {
                '/' | '\\' | '?' | '%' | '*' | ':' | '|' | '"' | '<' | '>' => '_',
                _ => c,
            }
        })
        .collect()
}

/// `true` when sanitizing left nothing usable to name a file after.
fn is_unusable(name: &str) -> bool {
    name.trim_matches('_').trim().is_empty()
}

/// Resolve a `/`-separated relative path against `base`.
///
/// Manifest paths are stored with `/` regardless of platform so the manifest
/// stays portable.
fn rel_to_path(base: &Path, rel: &str) -> PathBuf {
    rel.split('/')
        .fold(base.to_path_buf(), |acc, seg| acc.join(seg))
}

// -------------------------------------------------------------- manifest ---

/// A task note written by a previous run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEntry {
    /// Path of the note, relative to the output root.
    pub path: String,
    /// Owning project, so a filtered run knows which entries it may prune.
    pub project: i32,
}

/// Record of the notes written by a previous run.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub version: u32,
    /// project id -> relative path of the project's task directory
    #[serde(default)]
    pub project_dirs: BTreeMap<i32, String>,
    /// project id -> relative path of the project's index note
    #[serde(default)]
    pub project_notes: BTreeMap<i32, String>,
    /// task id -> note record
    #[serde(default)]
    pub tasks: BTreeMap<i32, TaskEntry>,
}

impl Manifest {
    pub const VERSION: u32 = 1;

    fn load(dir: &Path) -> Self {
        let path = dir.join(MANIFEST_FILE);
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return Self::default();
        };

        match serde_json::from_str(&raw) {
            Ok(manifest) => manifest,
            Err(e) => {
                // A corrupt manifest must not drive deletions from garbage, so
                // start clean and leave existing files alone.
                eprintln!("Ignoring unreadable {}: {e}", path.display());
                Self::default()
            }
        }
    }

    fn save(&self, dir: &Path) -> std::io::Result<()> {
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(dir.join(MANIFEST_FILE), raw)
    }
}

/// Whether a manifest entry belonging to `project` must be left untouched.
///
/// An entry is carried forward when its project still exists upstream but is
/// not part of this run (filtered out with `--project`, or archived). Entries
/// whose project has disappeared upstream are not carried, so they get pruned.
fn should_carry(project: i32, existing: &HashSet<i32>, selected: &HashSet<i32>) -> bool {
    existing.contains(&project) && !selected.contains(&project)
}

// ------------------------------------------------------------------ note ---

pub struct TaskNote {
    pub id: i32,
    pub title: String,
    pub description: String,
    pub cleaned_meta: serde_json::Value,
}

impl TaskNote {
    pub fn filename(&self) -> String {
        let sanitized = sanitize_name(&self.title);

        // If sanitizing produced something useless (all underscores or empty),
        // fall back to just the id.
        if is_unusable(&sanitized) {
            format!("{}.md", self.id)
        } else {
            format!("{} {}.md", self.id, sanitized)
        }
    }

    pub fn file_content(&self) -> String {
        let meta = serde_yaml_ng::to_string(&self.cleaned_meta).unwrap_or_default();
        // Fall back to the raw description if it is not convertible HTML.
        let body = htmd::convert(&self.description).unwrap_or_else(|_| self.description.clone());

        format!(
            "---\n{}---\n# {} {}\n\n{}\n",
            meta, self.id, self.title, body
        )
    }
}

impl From<&ModelsTask> for TaskNote {
    fn from(value: &ModelsTask) -> Self {
        let mut cleaned_meta = serde_json::to_value(value)
            .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()));

        // These are rendered into the note body/name, so keep them out of the
        // frontmatter to avoid duplicating them.
        if let Some(obj) = cleaned_meta.as_object_mut() {
            obj.remove("id");
            obj.remove("title");
            obj.remove("description");
        }

        Self {
            id: value.id.unwrap_or_default(),
            title: value.title.clone().unwrap_or_default(),
            description: value.description.clone().unwrap_or_default(),
            cleaned_meta,
        }
    }
}

/// Render a project's index note. `dir_rel` is the project's task directory
/// relative to the output root, so the dataview source matches what is on disk.
fn project_note_content(prj: &ModelsProject, dir_rel: &str) -> String {
    let info = serde_yaml_ng::to_string(prj).unwrap_or_default();
    let name = prj.title.clone().unwrap_or_default();
    let task_view = match prj.id {
        Some(id) => format!("```dataview\nTABLE FROM \"{dir_rel}\" WHERE project_id = {id}\n```\n"),
        None => String::new(),
    };

    format!("---\n{info}---\n# {name}\n\n{task_view}\n")
}

// --------------------------------------------------------------- layout ----

/// Compute each project's directory path relative to the output root,
/// mirroring Vikunja's parent/child hierarchy.
///
/// Sibling projects whose titles sanitize to the same segment get their id
/// appended so they cannot collide on disk.
fn project_paths(projects: &[ModelsProject]) -> HashMap<i32, String> {
    let by_id: HashMap<i32, &ModelsProject> = projects
        .iter()
        .filter_map(|p| p.id.map(|id| (id, p)))
        .collect();

    // Assign a unique segment per (parent, name) group, in id order so the
    // layout is stable between runs.
    let mut ids: Vec<i32> = by_id.keys().copied().collect();
    ids.sort_unstable();

    let mut segments: HashMap<i32, String> = HashMap::new();
    let mut taken: HashSet<(Option<i32>, String)> = HashSet::new();

    for id in &ids {
        let prj = by_id[id];
        let sanitized = sanitize_name(prj.title.as_deref().unwrap_or_default());
        let base = if is_unusable(&sanitized) {
            id.to_string()
        } else {
            sanitized
        };

        let parent = prj.parent_project_id.filter(|p| *p != 0);
        let mut segment = base.clone();
        if !taken.insert((parent, segment.clone())) {
            segment = format!("{base} ({id})");
            taken.insert((parent, segment.clone()));
        }

        segments.insert(*id, segment);
    }

    // Walk parents to build the full relative path.
    let mut paths = HashMap::new();
    for id in &ids {
        let mut chain = Vec::new();
        let mut cursor = Some(*id);
        let mut seen = HashSet::new();

        while let Some(current) = cursor {
            if chain.len() >= MAX_PROJECT_DEPTH || !seen.insert(current) {
                break;
            }
            let Some(seg) = segments.get(&current) else {
                break;
            };
            chain.push(seg.clone());
            cursor = by_id
                .get(&current)
                .and_then(|p| p.parent_project_id)
                .filter(|p| *p != 0);
        }

        chain.reverse();
        paths.insert(*id, chain.join("/"));
    }

    paths
}

/// `true` for pseudo-projects such as Favorites, whose tasks are duplicates of
/// tasks that already live in a real project.
fn is_pseudo_project(prj: &ModelsProject) -> bool {
    // Vikunja exposes these with a non-positive id; the title check is a
    // fallback for instances that do not.
    prj.id.map(|id| id <= 0).unwrap_or(true) || prj.title.as_deref() == Some("Favorites")
}

fn matches_selector(prj: &ModelsProject, selectors: &[String]) -> bool {
    if selectors.is_empty() {
        return true;
    }

    selectors.iter().any(|sel| {
        prj.id.map(|id| id.to_string() == *sel).unwrap_or(false)
            || prj
                .title
                .as_deref()
                .map(|t| t.eq_ignore_ascii_case(sel))
                .unwrap_or(false)
    })
}

// ------------------------------------------------------------------ sync ---

#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub output: PathBuf,
    /// Only sync these projects, matched by id or title. Empty means all.
    pub projects: Vec<String>,
    pub include_done: bool,
    pub include_archived: bool,
    pub dry_run: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SyncStats {
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub removed: usize,
    pub errors: usize,
}

impl SyncStats {
    fn merge(&mut self, other: Self) {
        self.created += other.created;
        self.updated += other.updated;
        self.unchanged += other.unchanged;
        self.removed += other.removed;
        self.errors += other.errors;
    }

    /// `true` when the run changed nothing on disk.
    pub fn is_noop(&self) -> bool {
        self.created == 0 && self.updated == 0 && self.removed == 0
    }
}

/// Write `contents` to `path`, skipping the write when it is already current.
fn write_note(path: &Path, contents: &str, dry_run: bool, stats: &mut SyncStats) {
    let existing = std::fs::read_to_string(path).ok();

    if existing.as_deref() == Some(contents) {
        stats.unchanged += 1;
        return;
    }

    let created = existing.is_none();

    if !dry_run {
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("Failed to create {}: {e}", parent.display());
                stats.errors += 1;
                return;
            }
        }

        if let Err(e) = std::fs::write(path, contents) {
            eprintln!("Failed to write {}: {e}", path.display());
            stats.errors += 1;
            return;
        }
    }

    if created {
        stats.created += 1;
    } else {
        stats.updated += 1;
    }
}

fn remove_note(path: &Path, dry_run: bool, stats: &mut SyncStats) {
    if !path.exists() {
        return;
    }

    if !dry_run {
        if let Err(e) = std::fs::remove_file(path) {
            eprintln!("Failed to remove {}: {e}", path.display());
            stats.errors += 1;
            return;
        }
    }

    stats.removed += 1;
}

pub async fn fetch_local(opts: &SyncOptions, api: &VikunjaAPI) -> SyncStats {
    let mut stats = SyncStats::default();

    let projects = match api.get_all_projects().await {
        Ok(projects) => projects,
        Err(e) => {
            eprintln!("Failed to fetch projects: {e}");
            stats.errors += 1;
            return stats;
        }
    };

    // Paths are computed over every project so a child's path stays stable
    // even when its parent is filtered out of this run.
    let paths = project_paths(&projects);
    let existing_ids: HashSet<i32> = projects.iter().filter_map(|p| p.id).collect();

    let selected: Vec<&ModelsProject> = projects
        .iter()
        .filter(|p| !is_pseudo_project(p))
        .filter(|p| opts.include_archived || !p.is_archived.unwrap_or(false))
        .filter(|p| matches_selector(p, &opts.projects))
        .collect();

    if selected.is_empty() {
        eprintln!("No projects matched.");
        return stats;
    }

    let selected_ids: HashSet<i32> = selected.iter().filter_map(|p| p.id).collect();

    // Ask the server for done tasks explicitly rather than inheriting whatever
    // the project's first view happens to filter on.
    let filter = if opts.include_done {
        "done = true || done = false"
    } else {
        "done = false"
    };

    let tasks = match api.get_all_tasks_filtered(Some(filter)).await {
        Ok(tasks) => tasks,
        Err(e) => {
            eprintln!("Failed to fetch tasks: {e}");
            stats.errors += 1;
            return stats;
        }
    };

    // Re-apply the done rule locally so behaviour is identical even if the
    // server ignores or misparses the filter expression.
    let mut by_project: HashMap<i32, Vec<&ModelsTask>> = HashMap::new();
    for task in &tasks {
        if !opts.include_done && task.done.unwrap_or(false) {
            continue;
        }
        if let Some(pid) = task.project_id {
            by_project.entry(pid).or_default().push(task);
        }
    }

    let manifest = Manifest::load(&opts.output);
    let mut next = Manifest {
        version: Manifest::VERSION,
        ..Default::default()
    };

    println!(
        "Syncing into {}{}",
        opts.output.display(),
        if opts.dry_run { " (dry run)" } else { "" }
    );

    let mut synced_tasks = 0usize;

    for prj in &selected {
        let Some(id) = prj.id else { continue };
        let Some(dir_rel) = paths.get(&id) else {
            continue;
        };

        let mut prj_stats = SyncStats::default();
        let prj_dir = rel_to_path(&opts.output, dir_rel);
        let mut prj_tasks = by_project.remove(&id).unwrap_or_default();
        prj_tasks.sort_by_key(|t| t.id.unwrap_or_default());

        if !opts.dry_run {
            if let Err(e) = std::fs::create_dir_all(&prj_dir) {
                eprintln!("Failed to create {}: {e}", prj_dir.display());
                stats.errors += 1;
                continue;
            }
        }

        for task in &prj_tasks {
            let note: TaskNote = (*task).into();
            let rel = format!("{dir_rel}/{}", note.filename());

            // A retitled task changes its filename; drop the old note so the
            // rename does not leave a duplicate behind.
            if let Some(prev) = manifest.tasks.get(&note.id) {
                if prev.path != rel {
                    remove_note(
                        &rel_to_path(&opts.output, &prev.path),
                        opts.dry_run,
                        &mut prj_stats,
                    );
                }
            }

            write_note(
                &rel_to_path(&opts.output, &rel),
                &note.file_content(),
                opts.dry_run,
                &mut prj_stats,
            );

            next.tasks.insert(
                note.id,
                TaskEntry {
                    path: rel,
                    project: id,
                },
            );
        }

        // Project index note, kept as a sibling of the task directory. Counted
        // separately so the per-project line matches the task count.
        let mut note_stats = SyncStats::default();
        let note_rel = format!("{dir_rel}.md");
        if let Some(prev) = manifest.project_notes.get(&id) {
            if prev != &note_rel {
                remove_note(
                    &rel_to_path(&opts.output, prev),
                    opts.dry_run,
                    &mut note_stats,
                );
            }
        }
        write_note(
            &rel_to_path(&opts.output, &note_rel),
            &project_note_content(prj, dir_rel),
            opts.dry_run,
            &mut note_stats,
        );

        next.project_notes.insert(id, note_rel);
        next.project_dirs.insert(id, dir_rel.clone());

        synced_tasks += prj_tasks.len();
        println!(
            "  {dir_rel}  ({}: +{} ~{} -{})",
            plural(prj_tasks.len(), "task"),
            prj_stats.created,
            prj_stats.updated,
            prj_stats.removed
        );
        stats.merge(prj_stats);
        stats.merge(note_stats);
    }

    // Reconcile against the previous run. Entries owned by projects that still
    // exist but were not synced now are carried forward untouched; the rest
    // are pruned when they are no longer wanted.
    for (tid, entry) in &manifest.tasks {
        if should_carry(entry.project, &existing_ids, &selected_ids) {
            next.tasks.insert(*tid, entry.clone());
            continue;
        }

        if !next.tasks.contains_key(tid) {
            remove_note(
                &rel_to_path(&opts.output, &entry.path),
                opts.dry_run,
                &mut stats,
            );
        }
    }

    for (pid, rel) in &manifest.project_notes {
        if should_carry(*pid, &existing_ids, &selected_ids) {
            next.project_notes.insert(*pid, rel.clone());
            continue;
        }

        if !next.project_notes.contains_key(pid) {
            remove_note(&rel_to_path(&opts.output, rel), opts.dry_run, &mut stats);
        }
    }

    // Directories we created that are no longer in use, deepest first so
    // children are cleared before their parents.
    let mut stale_dirs: Vec<&String> = Vec::new();
    for (pid, rel) in &manifest.project_dirs {
        if should_carry(*pid, &existing_ids, &selected_ids) {
            next.project_dirs.insert(*pid, rel.clone());
            continue;
        }

        if next.project_dirs.get(pid) != Some(rel) {
            stale_dirs.push(rel);
        }
    }
    stale_dirs.sort_by_key(|r| std::cmp::Reverse(r.matches('/').count()));

    if !opts.dry_run {
        for rel in stale_dirs {
            // Fails while the directory still holds files we do not own, which
            // is exactly when it should be kept.
            let _ = std::fs::remove_dir(rel_to_path(&opts.output, rel));
        }

        if let Err(e) = next.save(&opts.output) {
            eprintln!("Failed to write {MANIFEST_FILE}: {e}");
            stats.errors += 1;
        }
    }

    print_summary(&stats, synced_tasks, selected.len(), opts.dry_run);
    stats
}

/// `"1 task"` / `"2 tasks"`.
fn plural(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("{n} {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

fn print_summary(stats: &SyncStats, tasks: usize, projects: usize, dry_run: bool) {
    let verb = if dry_run { "Would sync" } else { "Synced" };
    let scope = format!(
        "{} across {}",
        plural(tasks, "task"),
        plural(projects, "project")
    );

    if stats.is_noop() {
        println!("{verb} {scope}: already up to date.");
    } else {
        // Note counts cover task notes plus one index note per project, so
        // they will not match the task count exactly.
        println!(
            "{verb} {scope}: {} created, {} updated, {} removed, {} unchanged.",
            stats.created, stats.updated, stats.removed, stats.unchanged
        );
    }

    if stats.errors > 0 {
        eprintln!("{} failed.", plural(stats.errors, "operation"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(id: i32, title: &str) -> TaskNote {
        TaskNote {
            id,
            title: title.to_string(),
            description: String::new(),
            cleaned_meta: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    fn project(id: i32, title: &str, parent: Option<i32>) -> ModelsProject {
        ModelsProject {
            id: Some(id),
            title: Some(title.to_string()),
            parent_project_id: parent,
            ..Default::default()
        }
    }

    fn ids(v: &[i32]) -> HashSet<i32> {
        v.iter().copied().collect()
    }

    // ------------------------------------------------------------ naming --

    #[test]
    fn test_sanitize_replaces_path_separators() {
        assert_eq!(sanitize_name("a/b\\c"), "a_b_c");
        assert_eq!(sanitize_name("why? 50%: yes"), "why_ 50__ yes");
    }

    #[test]
    fn test_sanitize_keeps_ordinary_titles() {
        assert_eq!(sanitize_name("Buy milk"), "Buy milk");
    }

    #[test]
    fn test_filename_prefixes_id() {
        assert_eq!(note(7, "Buy milk").filename(), "7 Buy milk.md");
    }

    #[test]
    fn test_filename_sanitizes_title() {
        assert_eq!(note(7, "feat/sync").filename(), "7 feat_sync.md");
    }

    #[test]
    fn test_filename_falls_back_to_id_when_unusable() {
        // A title made only of separators sanitizes down to nothing useful.
        assert_eq!(note(42, "///").filename(), "42.md");
        assert_eq!(note(42, "").filename(), "42.md");
        assert_eq!(note(42, "   ").filename(), "42.md");
    }

    #[test]
    fn test_rel_to_path_joins_segments() {
        assert_eq!(
            rel_to_path(Path::new("/tmp/out"), "Work/Sub"),
            Path::new("/tmp/out/Work/Sub")
        );
    }

    // -------------------------------------------------------------- note --

    #[test]
    fn test_file_content_has_frontmatter_and_heading() {
        let content = note(3, "Task").file_content();
        assert!(content.starts_with("---\n"));
        assert!(content.contains("\n---\n# 3 Task\n"));
    }

    #[test]
    fn test_file_content_converts_html_description() {
        let mut n = note(1, "T");
        n.description = "<p>hello <strong>world</strong></p>".to_string();
        assert!(n.file_content().contains("hello **world**"));
    }

    #[test]
    fn test_task_note_from_model_omits_body_fields_from_meta() {
        let task = ModelsTask {
            id: Some(5),
            title: Some("Title".to_string()),
            description: Some("<p>d</p>".to_string()),
            ..Default::default()
        };
        let n: TaskNote = (&task).into();

        assert_eq!(n.id, 5);
        assert_eq!(n.title, "Title");
        assert_eq!(n.description, "<p>d</p>");

        let meta = n.cleaned_meta.as_object().unwrap();
        assert!(!meta.contains_key("id"));
        assert!(!meta.contains_key("title"));
        assert!(!meta.contains_key("description"));
    }

    #[test]
    fn test_task_note_from_model_tolerates_missing_fields() {
        // Vikunja can return tasks with no title/description set.
        let n: TaskNote = (&ModelsTask::default()).into();
        assert_eq!(n.id, 0);
        assert_eq!(n.title, "");
        assert_eq!(n.description, "");
        // Must still produce a usable filename rather than panicking.
        assert_eq!(n.filename(), "0.md");
    }

    #[test]
    fn test_project_note_references_its_directory() {
        let content = project_note_content(&project(4, "Work", None), "Work");
        assert!(content.contains("TABLE FROM \"Work\" WHERE project_id = 4"));
    }

    // ------------------------------------------------------------ layout --

    #[test]
    fn test_project_paths_mirror_hierarchy() {
        let projects = vec![
            project(1, "Work", None),
            project(2, "Subproject", Some(1)),
            project(3, "Deep", Some(2)),
        ];
        let paths = project_paths(&projects);

        assert_eq!(paths[&1], "Work");
        assert_eq!(paths[&2], "Work/Subproject");
        assert_eq!(paths[&3], "Work/Subproject/Deep");
    }

    #[test]
    fn test_project_paths_sanitize_segments() {
        assert_eq!(project_paths(&[project(1, "A/B", None)])[&1], "A_B");
    }

    #[test]
    fn test_project_paths_disambiguate_colliding_siblings() {
        // Two top-level projects with the same title must not share a folder.
        let paths = project_paths(&[project(1, "Inbox", None), project(2, "Inbox", None)]);

        assert_eq!(paths[&1], "Inbox");
        assert_eq!(paths[&2], "Inbox (2)");
    }

    #[test]
    fn test_project_paths_allow_same_name_under_different_parents() {
        let projects = vec![
            project(1, "Work", None),
            project(2, "Home", None),
            project(3, "Notes", Some(1)),
            project(4, "Notes", Some(2)),
        ];
        let paths = project_paths(&projects);

        assert_eq!(paths[&3], "Work/Notes");
        assert_eq!(paths[&4], "Home/Notes");
    }

    #[test]
    fn test_project_paths_survive_parent_cycle() {
        // A malformed parent chain must not hang the sync.
        let paths = project_paths(&[project(1, "A", Some(2)), project(2, "B", Some(1))]);

        assert!(paths.contains_key(&1));
        assert!(paths.contains_key(&2));
    }

    #[test]
    fn test_project_paths_treat_zero_parent_as_root() {
        assert_eq!(project_paths(&[project(1, "Work", Some(0))])[&1], "Work");
    }

    // ----------------------------------------------------------- filters --

    #[test]
    fn test_pseudo_projects_are_detected() {
        assert!(is_pseudo_project(&project(-1, "Favorites", None)));
        assert!(is_pseudo_project(&project(0, "Anything", None)));
        // Non-English instances still expose it with a non-positive id.
        assert!(is_pseudo_project(&project(-1, "Favoriten", None)));
        assert!(!is_pseudo_project(&project(3, "Work", None)));
    }

    #[test]
    fn test_selector_matches_id_and_title() {
        let prj = project(7, "Work", None);
        assert!(matches_selector(&prj, &[]));
        assert!(matches_selector(&prj, &["7".to_string()]));
        assert!(matches_selector(&prj, &["work".to_string()]));
        assert!(!matches_selector(&prj, &["other".to_string()]));
    }

    // --------------------------------------------------------- reconcile --

    #[test]
    fn test_carry_forward_protects_filtered_out_projects() {
        // Project 2 still exists but is not part of this run, so its notes
        // must survive rather than look like orphans.
        assert!(should_carry(2, &ids(&[1, 2]), &ids(&[1])));
    }

    #[test]
    fn test_selected_projects_are_reconciled() {
        // Project 1 is being synced, so its stale entries may be pruned.
        assert!(!should_carry(1, &ids(&[1, 2]), &ids(&[1])));
    }

    #[test]
    fn test_deleted_projects_are_not_carried() {
        // Project 9 is gone upstream; its notes should be cleaned up.
        assert!(!should_carry(9, &ids(&[1, 2]), &ids(&[1])));
    }

    #[test]
    fn test_manifest_round_trips() {
        let dir = std::env::temp_dir().join(format!("vk-sync-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut m = Manifest {
            version: Manifest::VERSION,
            ..Default::default()
        };
        m.project_dirs.insert(1, "Work".into());
        m.project_notes.insert(1, "Work.md".into());
        m.tasks.insert(
            5,
            TaskEntry {
                path: "Work/5 Task.md".into(),
                project: 1,
            },
        );
        m.save(&dir).unwrap();

        let loaded = Manifest::load(&dir);
        assert_eq!(loaded.version, Manifest::VERSION);
        assert_eq!(loaded.project_dirs[&1], "Work");
        assert_eq!(loaded.tasks[&5].project, 1);
        assert_eq!(loaded.tasks[&5].path, "Work/5 Task.md");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_missing_manifest_is_empty_not_fatal() {
        let dir = std::env::temp_dir().join("vk-sync-test-does-not-exist");
        assert!(Manifest::load(&dir).tasks.is_empty());
    }

    // ------------------------------------------------------------- stats --

    #[test]
    fn test_plural_agreement() {
        assert_eq!(plural(0, "task"), "0 tasks");
        assert_eq!(plural(1, "task"), "1 task");
        assert_eq!(plural(2, "task"), "2 tasks");
    }

    #[test]
    fn test_stats_noop_detection() {
        let mut s = SyncStats {
            unchanged: 5,
            ..Default::default()
        };
        assert!(s.is_noop());

        s.created = 1;
        assert!(!s.is_noop());
    }
}
