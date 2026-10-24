// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Project folders on disk, and the list of recently opened ones.
//!
//! A project is a directory containing `concat.json`. That file is written the
//! moment the project is created, so a project is a real thing from the start
//! rather than a promise the app keeps only if you remember to save.
//!
//! The recents list lives in the app's config directory, not in any project.
//! It describes this machine's history, and copying a project folder to
//! another machine should not drag one person's recent files along with it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const MANIFEST: &str = "concat.json";
/// The manifest's earlier names, newest first. Projects created before a
/// rename keep theirs forever: the manifest is read and written under the
/// name it has, and only brand-new projects get the current one. A rename
/// must never orphan an edit.
const LEGACY_MANIFESTS: &[&str] = &["wolfcut.json"];

/// The manifest file this project actually uses: the current name, a legacy
/// name if that is what exists, the current name for a fresh folder.
pub fn manifest_path(root: &Path) -> PathBuf {
    let current = root.join(MANIFEST);
    if current.is_file() {
        return current;
    }
    for legacy in LEGACY_MANIFESTS {
        let candidate = root.join(legacy);
        if candidate.is_file() {
            return candidate;
        }
    }
    current
}

/// Whether `root` holds a project: any manifest, current or legacy.
pub fn is_project(root: &Path) -> bool {
    manifest_path(root).is_file()
}

const RECENTS: &str = "recents.json";
/// Long enough to be useful, short enough that the list stays scannable.
const MAX_RECENTS: usize = 12;

/// Everything needed to reopen a project.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    /// The project folder.
    pub path: String,
    /// The name shown in the title bar and the recents list.
    pub name: String,
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// Frame rate as an exact fraction; the decimal is never stored.
    pub rate_num: i64,
    /// Denominator of the frame rate.
    pub rate_den: i64,
    /// Milliseconds since the epoch.
    pub opened_at: u64,
}

/// A snapshot's position in this process's project write order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ticket(u64);

/// Mints the next write ticket. For background saves, mint it when the
/// document snapshot is captured, before dispatching the worker.
pub fn ticket() -> Ticket {
    Ticket(NEXT_TICKET.fetch_add(1, Ordering::Relaxed))
}

static NEXT_TICKET: AtomicU64 = AtomicU64::new(1);

/// Result of an ordered save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Saved {
    /// This snapshot reached the manifest.
    Written,
    /// A later snapshot was already written, so this older one was skipped.
    Superseded,
}

struct Writes {
    writing: Mutex<()>,
    written: AtomicU64,
}

fn writes() -> &'static Mutex<HashMap<PathBuf, Arc<Writes>>> {
    static WRITES: OnceLock<Mutex<HashMap<PathBuf, Arc<Writes>>>> = OnceLock::new();
    WRITES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn project_key(root: &Path) -> PathBuf {
    std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
}

fn write_state(key: &Path) -> Arc<Writes> {
    let mut all = writes()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    Arc::clone(all.entry(key.to_path_buf()).or_insert_with(|| {
        Arc::new(Writes {
            writing: Mutex::new(()),
            written: AtomicU64::new(0),
        })
    }))
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    /// The version of the app that wrote the file, under whichever name the
    /// app had at the time. Informational: nothing reads it back.
    #[serde(alias = "wolfcut")]
    concat: String,
    name: String,
    video: Video,
    /// Everything else in the file is ignored when reading settings for the
    /// recents list. `flatten` keeps it from being dropped on a rewrite.
    #[serde(flatten)]
    #[allow(dead_code)]
    rest: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Video {
    width: u32,
    height: u32,
    rate_num: i64,
    rate_den: i64,
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// Creates the project folder and writes its manifest.
///
/// Refuses to touch a folder that already holds a manifest. Overwriting
/// someone's edit because they reused a name is not a recoverable mistake.
pub fn create(
    location: &str,
    name: &str,
    width: u32,
    height: u32,
    rate_num: i64,
    rate_den: i64,
) -> Result<ProjectInfo, String> {
    let root = Path::new(location).join(folder_name(name));
    let manifest = root.join(MANIFEST);

    // Any name counts as an existing project - a legacy folder must be
    // just as safe from being clobbered as a new one.
    if is_project(&root) {
        return Err(format!(
            "a Concat project already exists at {}",
            root.display()
        ));
    }

    std::fs::create_dir_all(&root)
        .map_err(|error| format!("could not create {}: {error}", root.display()))?;

    // Settings only: a fresh project has no edit yet. The full document -
    // timelines included - is written by the session's save from the first
    // change.
    let document = Manifest {
        concat: env!("CARGO_PKG_VERSION").to_owned(),
        name: name.to_owned(),
        video: Video {
            width,
            height,
            rate_num,
            rate_den,
        },
        rest: serde_json::Map::new(),
    };

    let encoded = serde_json::to_vec_pretty(&document)
        .map_err(|error| format!("could not encode the manifest: {error}"))?;
    std::fs::write(&manifest, encoded)
        .map_err(|error| format!("could not write {}: {error}", manifest.display()))?;

    Ok(ProjectInfo {
        path: root.to_string_lossy().into_owned(),
        name: name.to_owned(),
        width,
        height,
        rate_num,
        rate_den,
        opened_at: now_millis(),
    })
}

/// Writes a captured document in call order. Background callers should mint
/// a ticket at snapshot time and call [`save_ordered`] on their worker.
pub fn save(path: &str, document: &serde_json::Value) -> Result<(), String> {
    save_ordered(path, document, ticket()).map(|_| ())
}

/// Writes a document captured at `ticket`. An older snapshot that arrives
/// after a newer successful write is skipped rather than restoring stale data.
pub fn save_ordered(
    path: &str,
    document: &serde_json::Value,
    ticket: Ticket,
) -> Result<Saved, String> {
    let root = PathBuf::from(path);
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("could not create {}: {error}", root.display()))?;

    let state = write_state(&project_key(&root));
    let _writing = state
        .writing
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if ticket.0 < state.written.load(Ordering::Acquire) {
        return Ok(Saved::Superseded);
    }

    let encoded = serde_json::to_vec_pretty(document)
        .map_err(|error| format!("could not encode the project: {error}"))?;
    write_manifest(&root, &encoded)?;
    state.written.store(ticket.0, Ordering::Release);
    Ok(Saved::Written)
}

/// A per-save temporary; Drop closes and removes it on every failure path.
struct Temporary {
    path: PathBuf,
    file: Option<std::fs::File>,
    renamed: bool,
}

impl Temporary {
    fn open(root: &Path) -> Result<Self, String> {
        let mut attempts = 0;
        loop {
            let nonce = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
            let path = root.join(format!("{MANIFEST}.saving.{}.{nonce}", std::process::id()));
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: Some(file),
                        renamed: false,
                    });
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::AlreadyExists && attempts < 16 =>
                {
                    attempts += 1;
                }
                Err(error) => return Err(format!("could not write {}: {error}", path.display())),
            }
        }
    }

    fn write(&mut self, encoded: &[u8]) -> Result<(), String> {
        let file = self.file.as_mut().expect("new temporary is open");
        std::io::Write::write_all(file, encoded)
            .map_err(|error| format!("could not write {}: {error}", self.path.display()))?;
        file.sync_all()
            .map_err(|error| format!("could not flush {}: {error}", self.path.display()))
    }

    fn commit(&mut self, manifest: &Path) -> Result<(), String> {
        self.file.take();
        let mut wait = std::time::Duration::from_millis(50);
        let mut tries = 0;
        loop {
            match std::fs::rename(&self.path, manifest) {
                Ok(()) => break,
                Err(error) if tries < 5 && transient(&error) => {
                    tries += 1;
                    std::thread::sleep(wait);
                    wait *= 2;
                }
                Err(error) => {
                    return Err(format!("could not replace {}: {error}", manifest.display()));
                }
            }
        }
        self.renamed = true;
        Ok(())
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        self.file.take();
        if !self.renamed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn transient(error: &std::io::Error) -> bool {
    cfg!(windows)
        && (error.kind() == std::io::ErrorKind::PermissionDenied
            || matches!(error.raw_os_error(), Some(5 | 32)))
}

static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

fn write_manifest(root: &Path, encoded: &[u8]) -> Result<(), String> {
    let manifest = manifest_path(root);
    let mut temporary = Temporary::open(root)?;
    temporary.write(encoded)?;
    temporary.commit(&manifest)
}

/// True when a document carries no edit state at all - the settings-only
/// manifest `create` writes, in a project closed before its first save.
/// Such a document is a legitimately empty project. A document that claims
/// edit state the build then cannot load is a different thing: corrupt.
pub fn is_settings_only(document: &serde_json::Value) -> bool {
    document.is_object()
        && ["timelines", "tracks", "clips", "media"]
            .iter()
            .all(|key| document.get(key).is_none())
}

/// Reads the whole project document back: `None` when the folder has no
/// document yet, an error for one that is there and cannot be read or
/// parsed. The two are kept apart because a caller that treats them alike
/// opens a half-written document as an empty project and saves over it
/// (audit 2026-09-23, #1).
pub fn read_document(path: &str) -> Result<Option<serde_json::Value>, String> {
    let manifest = manifest_path(Path::new(path));
    let bytes = match std::fs::read(&manifest) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("could not read {}: {error}", manifest.display())),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| format!("{} is not a Concat project: {error}", manifest.display()))
}

/// Reads an existing project's settings.
pub fn open(path: &str) -> Result<ProjectInfo, String> {
    let root = PathBuf::from(path);
    let manifest = manifest_path(&root);

    let bytes = std::fs::read(&manifest)
        .map_err(|error| format!("could not read {}: {error}", manifest.display()))?;
    let document: Manifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("{} is not a Concat project: {error}", manifest.display()))?;

    if document.video.rate_den == 0 {
        return Err(format!("{} has an invalid frame rate", manifest.display()));
    }

    Ok(ProjectInfo {
        path: root.to_string_lossy().into_owned(),
        name: document.name,
        width: document.video.width,
        height: document.video.height,
        rate_num: document.video.rate_num,
        rate_den: document.video.rate_den,
        opened_at: now_millis(),
    })
}

/// Moves a project to the front of the recents list.
pub fn remember(config: &Path, project: &ProjectInfo) -> Result<(), String> {
    let mut entries = read_recents(config);
    entries.retain(|entry| !same_path(&entry.path, &project.path));
    entries.insert(0, project.clone());
    entries.truncate(MAX_RECENTS);
    write_recents(config, &entries)
}

/// The recents list, most recent first, with vanished folders dropped.
///
/// Filtering on read rather than pruning on write means a project on a drive
/// that happens to be unplugged comes back when the drive does.
pub fn list(config: &Path) -> Vec<ProjectInfo> {
    read_recents(config)
        .into_iter()
        .filter(|entry| is_project(Path::new(&entry.path)))
        .collect()
}

/// Removes one project from the list. The folder itself is left alone.
pub fn forget(config: &Path, path: &str) -> Result<(), String> {
    let mut entries = read_recents(config);
    entries.retain(|entry| !same_path(&entry.path, path));
    write_recents(config, &entries)
}

fn read_recents(config: &Path) -> Vec<ProjectInfo> {
    // A missing or corrupt list is not an error worth showing anyone; it just
    // means there is no history yet.
    std::fs::read(config.join(RECENTS))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn write_recents(config: &Path, entries: &[ProjectInfo]) -> Result<(), String> {
    std::fs::create_dir_all(config)
        .map_err(|error| format!("could not create {}: {error}", config.display()))?;

    let encoded = serde_json::to_vec_pretty(entries)
        .map_err(|error| format!("could not encode the recents list: {error}"))?;
    std::fs::write(config.join(RECENTS), encoded)
        .map_err(|error| format!("could not write the recents list: {error}"))
}

/// Compares paths case-insensitively, because Windows does, and on Windows
/// with either slash, because a path typed or joined with `/` names the same
/// folder as the one the file dialog hands back with `\`. A trailing
/// separator names the same folder too.
fn same_path(left: &str, right: &str) -> bool {
    fn spelled(path: &str) -> String {
        let path = if cfg!(windows) {
            path.replace('/', "\\")
        } else {
            path.to_owned()
        };
        path.trim_end_matches(['/', '\\']).to_owned()
    }
    spelled(left).eq_ignore_ascii_case(&spelled(right))
}

/// Turns a project name into something a filesystem will accept. Also used
/// for template bundle folders and the files inside them.
pub fn folder_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|character| {
            if character.is_control() || r#"<>:"/\|?*"#.contains(character) {
                '-'
            } else {
                character
            }
        })
        .collect();

    // Windows refuses names ending in a dot or a space.
    let trimmed = cleaned.trim_matches(['.', ' ']);
    if trimmed.is_empty() {
        "Untitled project".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Removes all cached artwork and waveforms from a project folder.
///
/// The cache is entirely regenerable, so the whole folder goes, subfolders
/// (`art/`, peaks and all) included. `remove_file` alone would skip them:
/// it refuses directories.
pub fn clear_cache(path: &str) -> Result<usize, String> {
    let cache_dir = Path::new(path).join("cache");
    if !cache_dir.exists() {
        return Ok(0);
    }
    let count = count_files(&cache_dir);
    std::fs::remove_dir_all(&cache_dir)
        .map_err(|error| format!("cannot remove {}: {error}", cache_dir.display()))?;
    Ok(count)
}

/// Recursively counts the files under a directory, for the clear-cache toast.
fn count_files(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() { count_files(&path) } else { 1 }
        })
        .sum()
}
#[cfg(test)]
mod tests {
    use super::*;

    fn save_scratch(label: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "concat-save-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("scratch project folder");
        root
    }

    fn save_document(name: &str) -> serde_json::Value {
        serde_json::json!({ "name": name, "edit": { "revision": name } })
    }

    fn save_temporary_count(root: &Path) -> usize {
        std::fs::read_dir(root)
            .expect("reads scratch folder")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&format!("{MANIFEST}.saving"))
            })
            .count()
    }

    #[test]
    fn a_late_snapshot_never_lands_after_a_newer_one() {
        let root = save_scratch("late-snapshot");
        let path = root.to_string_lossy().into_owned();
        let older = save_document("older");
        let newer = save_document("newer");
        let older_ticket = ticket();
        let newer_ticket = ticket();

        assert_eq!(
            save_ordered(&path, &newer, newer_ticket).expect("newer write"),
            Saved::Written
        );
        assert_eq!(
            save_ordered(&path, &older, older_ticket).expect("older write is skipped"),
            Saved::Superseded
        );
        assert_eq!(read_document(&path).expect("reads"), Some(newer));
        assert_eq!(save_temporary_count(&root), 0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_snapshots_leave_the_highest_ticket_on_disk() {
        let root = save_scratch("concurrent");
        let path = root.to_string_lossy().into_owned();
        let documents: Vec<_> = (0..8)
            .map(|index| save_document(&format!("doc-{index}")))
            .collect();
        let tickets: Vec<_> = (0..documents.len()).map(|_| ticket()).collect();

        std::thread::scope(|scope| {
            for (document, ticket) in documents.iter().zip(&tickets) {
                let path = path.clone();
                scope.spawn(move || {
                    save_ordered(&path, document, *ticket).expect("write succeeds");
                });
            }
        });

        assert_eq!(
            read_document(&path).expect("reads"),
            Some(documents.last().expect("documents").clone())
        );
        assert_eq!(save_temporary_count(&root), 0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_failed_ordered_save_preserves_the_manifest_and_cleans_its_temp() {
        let root = save_scratch("failed");
        let path = root.to_string_lossy().into_owned();
        let manifest = root.join(MANIFEST);
        std::fs::create_dir(&manifest).expect("block manifest replacement");
        let sentinel = manifest.join("keep");
        std::fs::write(&sentinel, b"keep").expect("sentinel");

        assert!(save_ordered(&path, &save_document("new"), ticket()).is_err());
        assert!(sentinel.exists(), "failed save leaves prior data intact");
        assert_eq!(save_temporary_count(&root), 0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sanitises_names_windows_would_reject() {
        assert_eq!(folder_name("My: Project?"), "My- Project-");
        assert_eq!(folder_name("  trailing.  "), "trailing");
        assert_eq!(folder_name("..."), "Untitled project");
        assert_eq!(folder_name(""), "Untitled project");
    }

    #[test]
    fn a_legacy_manifest_keeps_its_name() {
        for legacy in LEGACY_MANIFESTS {
            let scratch = std::env::temp_dir().join(format!("concat-legacy-{legacy}-test"));
            let _ = std::fs::remove_dir_all(&scratch);
            std::fs::create_dir_all(&scratch).expect("scratch dir");
            // A fresh folder resolves to the current name...
            assert!(manifest_path(&scratch).ends_with(MANIFEST));
            assert!(!is_project(&scratch));
            // ...a pre-rename project keeps the manifest it has, forever...
            std::fs::write(scratch.join(legacy), b"{}").expect("writes");
            assert!(manifest_path(&scratch).ends_with(legacy));
            assert!(is_project(&scratch));
            // ...and the current name wins only where it actually exists.
            std::fs::write(scratch.join(MANIFEST), b"{}").expect("writes");
            assert!(manifest_path(&scratch).ends_with(MANIFEST));
            let _ = std::fs::remove_dir_all(&scratch);
        }
    }

    #[test]
    fn a_manifest_under_any_of_its_names_opens() {
        let scratch = std::env::temp_dir().join("concat-manifest-key-test");
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).expect("scratch dir");
        for key in ["concat", "wolfcut"] {
            let document = serde_json::json!({
                key: "0.2.0",
                "name": "Old",
                "video": { "width": 1920, "height": 1080, "rateNum": 30, "rateDen": 1 },
            });
            std::fs::write(
                scratch.join(MANIFEST),
                serde_json::to_vec(&document).unwrap(),
            )
            .expect("writes");
            let info = open(&scratch.to_string_lossy()).expect("opens");
            assert_eq!((info.width, info.height, info.rate_num), (1920, 1080, 30));
        }
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn a_fresh_manifest_is_settings_only_and_a_saved_edit_is_not() {
        // Exactly what `create` writes: settings, no edit state.
        let fresh = serde_json::json!({
            "concat": "0.2.0",
            "name": "Untitled project",
            "video": { "width": 1080, "height": 1920, "rateNum": 30, "rateDen": 1 },
        });
        assert!(is_settings_only(&fresh));
        // Any edit state present means the document must load or fail loudly.
        assert!(!is_settings_only(&serde_json::json!({ "tracks": [] })));
        assert!(!is_settings_only(
            &serde_json::json!({ "timelines": "garbage" })
        ));
        assert!(!is_settings_only(&serde_json::json!("nonsense")));
    }

    #[test]
    fn paths_compare_case_insensitively() {
        assert!(same_path("D:\\Work\\Film", "d:\\work\\film"));
        assert!(!same_path("D:\\Work\\Film", "D:\\Work\\Other"));
        assert!(same_path("/work/film/", "/work/film"));
    }

    #[cfg(windows)]
    #[test]
    fn either_slash_names_the_same_folder_on_windows() {
        assert!(same_path(
            "C:\\Temp\\.tmp1\\Round trip",
            "C:\\Temp\\.tmp1/Round trip"
        ));
        assert!(same_path("C:/Work/Film/", "c:\\work\\film"));
    }
}
