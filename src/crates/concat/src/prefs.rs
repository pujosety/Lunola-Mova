// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! What the window remembers between runs, as one small JSON file in the
//! app's config directory. None of it is project state: the theme, which
//! models are chosen, which languages. A missing or unreadable file is the
//! defaults, never an error.

use crate::dock::SavedWorkspace;
use concat_host::AppDirs;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

const FILE: &str = "settings.json";
static SAVE_SUFFIX: AtomicU64 = AtomicU64::new(0);

/// Remembered preferences.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    /// The dark theme. `None` is the app's default, which is dark.
    pub dark: Option<bool>,
    /// The accent, by the lower-cased name the theme lists it under —
    /// "lime", "sky", ... — or as "#rrggbb" for a colour picked by hand.
    /// `None`, or a name no longer on the list, selects Violet, Lunola's
    /// default. Existing named choices and custom hex colours are preserved.
    pub accent: Option<String>,
    /// The playhead's colour as "#rrggbb", when one was picked by hand in
    /// Settings > Appearance. `None` is the palette's own, which is lime.
    pub playhead: Option<String>,
    /// The chosen transcriber model id, e.g. "base.en".
    pub transcriber_model: Option<String>,
    /// The chosen speech model id.
    pub tts_model: Option<String>,
    /// The chosen Kokoro speaker id.
    pub tts_voice: Option<i32>,
    /// The interface's locale code ("de", "pt-BR", ...); absent is English.
    pub locale: Option<String>,
    /// Package ids starred in the effect libraries, in no order. One list
    /// across all three shelves: a star is a fact about a package, and which
    /// library it happens to be filed in is not part of it.
    #[serde(default)]
    pub favourites: Vec<String>,
    /// The playhead stops at the end of the content instead of going where
    /// it is put. Off by default: a click past the last clip lands there, so
    /// a clip can be dropped at the playhead beyond everything else.
    pub playhead_stops_at_end: bool,
    /// The magnetic timeline: a delete and a trim close the gap they would
    /// leave, on the lane they happen on. Off by default, because a gap is
    /// sometimes the point; ⇧⌫ ripples either way.
    /// https://github.com/jub0t/Concat/issues/106
    pub magnetic: bool,
    /// Trim follow: while a clip's edge is dragged, the playhead rides the
    /// edge, so the monitor shows the exact frame the cut lands on. Off by
    /// default; the tray's button beside the hand turns it on.
    pub trim_follow: bool,
    /// The preview axis: the monitor shows the frame under the pointer as
    /// it crosses the lanes, the playhead staying where it is. Off by
    /// default: a monitor that jumps with the pointer startles whoever has
    /// not asked for it, and the tray's button is where they ask.
    pub preview_axis: bool,
    /// The preview axis plays the sound under the pointer too. Off by
    /// default: a burst of sound on every pass of the pointer is a lot to
    /// ask of a room.
    pub preview_axis_audio: bool,
    /// Where model downloads look first: a `SourcePreference` by name.
    /// Absent is automatic.
    pub download_source: Option<String>,
    /// The base URL a custom download source appends a model's file to.
    pub download_base: Option<String>,
    /// The Concat API on a socket while the window is open.
    #[serde(default)]
    pub server: ServerPrefs,
    /// The look new titles start from and generated captions are set in,
    /// kept from a title by the Text panel's "Use for new titles"; its
    /// words are empty. None starts titles in the bundled face.
    pub title_style: Option<concat_project::model::TextStyle>,
    /// The user's wide and compact workspace trees. Absent in older settings
    /// files, which continue to use the built-in layouts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_layout: Option<SavedWorkspace>,
    /// What the log records, by the name concat_host::logs reads: `info`
    /// unless the Settings sheet was asked for more or less. CONCAT_LOG in
    /// the environment overrides it for one run.
    pub log_level: Option<String>,
}

impl Preferences {
    /// Which of `names` — the theme's accents, in its order — the remembered
    /// accent is. Missing or unknown choices select Violet by name, not a
    /// hard-coded index. If that palette is absent, retain the first-row
    /// fallback. Never rewrite a saved named or custom choice here.
    pub fn accent_index(&self, names: impl IntoIterator<Item = impl AsRef<str>>) -> usize {
        let wanted = self.accent.as_deref().map(Self::accent_id);
        let mut default = 0;
        let mut chosen = None;
        for (index, name) in names.into_iter().enumerate() {
            let id = Self::accent_id(name.as_ref());
            if id == "violet" {
                default = index;
            }
            if chosen.is_none() && wanted.as_deref() == Some(id.as_str()) {
                chosen = Some(index);
            }
        }
        chosen.unwrap_or(default)
    }

    /// The id an accent is remembered under: its name, lower-cased, so the
    /// file reads `"accent": "sky"` and a hand-typed capital still matches.
    pub fn accent_id(name: &str) -> String {
        name.trim().to_lowercase()
    }

    /// The remembered accent when it is a colour rather than a name: the
    /// hex, for the caller to parse. A hash is what tells the two apart,
    /// and no name on the list starts with one.
    pub fn custom_accent(&self) -> Option<&str> {
        self.accent
            .as_deref()
            .map(str::trim)
            .filter(|text| text.starts_with('#'))
    }
}

/// The Settings sheet's Remote page: whether the API is served while the
/// window is open, where, and behind which token.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ServerPrefs {
    /// Serve while the window is open.
    pub enabled: bool,
    /// The TCP address JSON-RPC lines are served on.
    pub listen: String,
    /// What a connection presents first. Empty means none, which the
    /// server only allows on loopback.
    pub token: String,
}

impl Default for ServerPrefs {
    fn default() -> Self {
        ServerPrefs {
            enabled: false,
            listen: DEFAULT_LISTEN.to_owned(),
            token: String::new(),
        }
    }
}

/// Where the window's server listens unless told otherwise: loopback, on
/// the port `concat-cli serve` uses too.
pub const DEFAULT_LISTEN: &str = "127.0.0.1:7420";

impl Preferences {
    /// Reads the file, or the defaults when there is none.
    pub fn load(dirs: &AppDirs) -> Self {
        let path = dirs.config.join(FILE);
        let Ok(bytes) = std::fs::read(&path) else {
            return Self::default();
        };
        if let Ok(preferences) = serde_json::from_slice(&bytes) {
            return preferences;
        }
        // A single field with the wrong shape should not reset every
        // preference. Keep the original bytes before the next save repairs
        // the settings file.
        let _ = std::fs::copy(&path, path.with_extension("json.unreadable"));
        let preferences = salvage(&bytes);
        log::warn!(
            "settings: {} did not read whole; kept what did",
            path.display()
        );
        preferences
    }

    /// Writes the file. Best effort: a preference that did not stick is
    /// not worth interrupting anyone over. Write a unique staging file and
    /// rename it into place so a crash or overlapping save leaves one whole
    /// document. The config directory is account-specific; on Unix the file
    /// itself is readable by this account alone because it contains a token.
    pub fn save(&self, dirs: &AppDirs) {
        let _ = std::fs::create_dir_all(&dirs.config);
        let Ok(encoded) = serde_json::to_vec_pretty(self) else {
            return;
        };
        let path = dirs.config.join(FILE);
        let staging = loop {
            let suffix = SAVE_SUFFIX.fetch_add(1, Ordering::Relaxed);
            let staging = dirs.config.join(format!(
                "{FILE}.{}.{}.part",
                std::process::id(),
                suffix
            ));
            match write_private(&staging, &encoded) {
                Ok(()) => break staging,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return,
            }
        };
        if std::fs::rename(&staging, &path).is_err() {
            let _ = std::fs::remove_file(&staging);
        }
    }
}

/// Keeps each top-level value that still deserializes over the defaults;
/// one bad value costs itself rather than every setting.
fn salvage(bytes: &[u8]) -> Preferences {
    let Ok(serde_json::Value::Object(found)) = serde_json::from_slice(bytes) else {
        return Preferences::default();
    };
    let Ok(serde_json::Value::Object(mut kept)) = serde_json::to_value(Preferences::default())
    else {
        return Preferences::default();
    };
    for (key, value) in found {
        let before = kept.insert(key.clone(), value);
        if serde_json::from_value::<Preferences>(serde_json::Value::Object(kept.clone())).is_err() {
            match before {
                Some(before) => {
                    kept.insert(key, before);
                }
                None => {
                    kept.remove(&key);
                }
            }
        }
    }
    serde_json::from_value(serde_json::Value::Object(kept)).unwrap_or_default()
}

/// Writes a new staging file, private on Unix, and flushes it before rename.
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAMES: [&str; 3] = ["Lime", "Sky", "Violet"];

    /// Nothing remembered selects the named Lunola default.
    #[test]
    fn no_accent_is_violet() {
        assert_eq!(Preferences::default().accent_index(NAMES), 2);
        assert_eq!(Preferences::default().accent_index(["Violet", "Lime"]), 0);
        assert_eq!(Preferences::default().accent_index(["Lime", "Sky"]), 0);
    }

    /// A remembered name finds its row whatever its case or spacing, since
    /// the file is plain JSON somebody may have edited.
    #[test]
    fn accent_matches_by_name_not_by_case() {
        for spelled in ["violet", "Violet", "VIOLET", "  violet\n"] {
            let prefs = Preferences {
                accent: Some(spelled.to_owned()),
                ..Preferences::default()
            };
            assert_eq!(prefs.accent_index(NAMES), 2, "{spelled:?}");
        }
    }

    /// Rebranding changes the default, not a user's explicit Lime choice.
    #[test]
    fn remembered_lime_is_preserved() {
        let prefs = Preferences {
            accent: Some("lime".to_owned()),
            ..Preferences::default()
        };
        assert_eq!(prefs.accent_index(NAMES), 0);
        assert_eq!(prefs.accent_index(["Violet", "Lime", "Sky"]), 1);
    }

    /// A name the list no longer has falls back to the default rather than
    /// to a position, and an empty list cannot be indexed into at all.
    #[test]
    fn unknown_or_empty_accent_falls_back() {
        let prefs = Preferences {
            accent: Some("chartreuse".to_owned()),
            ..Preferences::default()
        };
        assert_eq!(prefs.accent_index(NAMES), 2);
        assert_eq!(prefs.accent_index(Vec::<String>::new()), 0);
        let blank = Preferences {
            accent: Some(String::new()),
            ..Preferences::default()
        };
        assert_eq!(blank.accent_index(NAMES), 2);
    }

    /// The id is what gets written and what gets matched, so the two have
    /// to agree on one spelling.
    #[test]
    fn accent_id_round_trips_through_the_file() {
        let prefs = Preferences {
            accent: Some(Preferences::accent_id("Sky")),
            ..Preferences::default()
        };
        let text = serde_json::to_string(&prefs).unwrap();
        assert!(text.contains("\"accent\":\"sky\""), "{text}");
        let back: Preferences = serde_json::from_str(&text).unwrap();
        assert_eq!(back.accent_index(NAMES), 1);
    }

    /// A hex is a picked colour and a word is not, whatever else is around
    /// it; and a picked colour is no name, so the index falls back to the
    /// default until the caller has parsed it.
    #[test]
    fn a_hash_means_a_picked_colour() {
        let picked = Preferences {
            accent: Some("  #FF00AA ".to_owned()),
            ..Preferences::default()
        };
        assert_eq!(picked.custom_accent(), Some("#FF00AA"));
        assert_eq!(picked.accent_index(NAMES), 2);
        let named = Preferences {
            accent: Some("sky".to_owned()),
            ..Preferences::default()
        };
        assert_eq!(named.custom_accent(), None);
        assert_eq!(Preferences::default().custom_accent(), None);
        let blank = Preferences {
            accent: Some(String::new()),
            ..Preferences::default()
        };
        assert_eq!(blank.custom_accent(), None);
    }

    /// A file from before the accent existed still loads, and reads as the
    /// default.
    #[test]
    fn older_file_without_an_accent_is_the_default() {
        let back: Preferences = serde_json::from_str(r#"{"dark": false}"#).unwrap();
        assert_eq!(back.dark, Some(false));
        assert_eq!(back.accent, None);
        assert_eq!(back.accent_index(NAMES), 2);
    }

    /// A picked playhead colour is written as its hex and read back as
    /// one, and a file from before it existed reads as the palette's own.
    #[test]
    fn a_picked_playhead_round_trips_and_an_older_file_has_none() {
        let prefs = Preferences {
            playhead: Some("#ff453a".to_owned()),
            ..Preferences::default()
        };
        let text = serde_json::to_string(&prefs).unwrap();
        assert!(text.contains("\"playhead\":\"#ff453a\""), "{text}");
        let back: Preferences = serde_json::from_str(&text).unwrap();
        assert_eq!(back.playhead.as_deref(), Some("#ff453a"));
        let older: Preferences = serde_json::from_str(r#"{"dark": true}"#).unwrap();
        assert_eq!(older.playhead, None);
    }

    #[test]
    fn workspace_layout_roundtrips_through_settings_json() {
        let root =
            std::env::temp_dir().join(format!("lunola-workspace-prefs-{}", std::process::id()));
        let dirs = AppDirs {
            config: root.join("config"),
            data: root.join("data"),
        };
        let expected =
            SavedWorkspace::from_docks(&crate::dock::default_dock(), &crate::dock::compact_dock());
        let prefs = Preferences {
            workspace_layout: Some(expected.clone()),
            ..Preferences::default()
        };
        prefs.save(&dirs);
        let loaded = Preferences::load(&dirs);
        assert_eq!(loaded.workspace_layout, Some(expected));
        let _ = std::fs::remove_dir_all(root);
    }

    /// One malformed field must not reset good settings, and the original
    /// bytes must be preserved before the next save repairs the file.
    #[test]
    fn malformed_settings_keep_good_values_and_quarantine_original() {
        let root =
            std::env::temp_dir().join(format!("lunola-settings-salvage-{}", std::process::id()));
        let dirs = AppDirs {
            config: root.join("config"),
            data: root.join("data"),
        };
        std::fs::create_dir_all(&dirs.config).expect("a config folder");
        let original = br#"{"magnetic": true, "dark": "very", "locale": "id"}"#;
        let path = dirs.config.join(FILE);
        std::fs::write(&path, original).expect("a settings file");

        let prefs = Preferences::load(&dirs);
        assert!(prefs.magnetic, "a malformed dark value must not erase this");
        assert_eq!(prefs.dark, None, "only the malformed field returns to default");
        assert_eq!(prefs.locale.as_deref(), Some("id"));
        assert_eq!(
            std::fs::read(dirs.config.join("settings.json.unreadable")).expect("quarantine"),
            original
        );

        prefs.save(&dirs);
        let saved = std::fs::read(&path).expect("repaired settings");
        let valid: Preferences = serde_json::from_slice(&saved).expect("complete saved JSON");
        assert!(valid.magnetic);
        assert_eq!(valid.locale.as_deref(), Some("id"));
        let loaded = Preferences::load(&dirs);
        assert!(loaded.magnetic);
        assert_eq!(loaded.locale.as_deref(), Some("id"));
        let part_files = std::fs::read_dir(&dirs.config)
            .expect("listed")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".part"))
            .count();
        assert_eq!(part_files, 0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn magnetic_preference_is_off_by_default_and_persists_when_enabled() {
        assert!(!Preferences::default().magnetic);
        let root =
            std::env::temp_dir().join(format!("lunola-magnetic-prefs-{}", std::process::id()));
        let dirs = AppDirs {
            config: root.join("config"),
            data: root.join("data"),
        };
        let prefs = Preferences {
            magnetic: true,
            ..Preferences::default()
        };
        prefs.save(&dirs);
        assert!(Preferences::load(&dirs).magnetic);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_settings_without_workspace_layout_remain_readable() {
        let legacy: Preferences = serde_json::from_str(r#"{"dark": false}"#).unwrap();
        assert_eq!(legacy.dark, Some(false));
        assert!(legacy.workspace_layout.is_none());
    }
}
