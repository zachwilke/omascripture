//! Persistent study data: bookmarks, highlights, notes, and reading position.

use crate::bible::{self, Position};
use crate::storage;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;

pub const HIGHLIGHT_NAMES: [&str; 5] = ["none", "yellow", "green", "blue", "red"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteDraft {
    pub position: Position,
    pub text: String,
}

#[derive(Debug, Clone)]
struct Persistence {
    path: PathBuf,
    expected: Option<Vec<u8>>,
    blocked: Option<String>,
    preserve_corrupt: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Study {
    #[serde(skip)]
    persistence: Option<Persistence>,
    #[serde(skip)]
    pub load_warning: Option<String>,
    /// Keep fields written by newer versions when opening an existing study.
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub note_draft: Option<NoteDraft>,
    /// Primary translation abbreviation.
    #[serde(default)]
    pub translation: Option<String>,
    /// Parallel translation abbreviation.
    #[serde(default)]
    pub parallel: Option<String>,
    #[serde(default)]
    pub parallel_visible: bool,
    #[serde(default)]
    pub last: Option<Position>,
    #[serde(default)]
    pub bookmarks: Vec<Position>,
    /// key "book:chapter:verse" -> highlight index into HIGHLIGHT_NAMES (1..)
    #[serde(default)]
    pub highlights: BTreeMap<String, u8>,
    /// key "book:chapter:verse" -> note text
    #[serde(default)]
    pub notes: BTreeMap<String, String>,
    /// Active reading plan, if any.
    #[serde(default)]
    pub plan: Option<crate::plans::Progress>,
    /// Preferred commentary pack id for the sidebar.
    #[serde(default)]
    pub commentary: Option<String>,
    /// Sidebar tab shown last (0 = hidden).
    #[serde(default)]
    pub sidebar: u8,
    /// Whether the sidebar was open when the app last closed.
    #[serde(default)]
    pub sidebar_visible: Option<bool>,
    /// Sidebar at start: "" (last state), "hidden" or "shown".
    #[serde(default)]
    pub sidebar_start: String,
    /// Where to open: "" (last position), "votd" or "plan".
    #[serde(default)]
    pub start: String,
    /// Maximum reading column width; 0 = use the whole pane.
    #[serde(default)]
    pub text_width: u16,
    #[serde(default)]
    pub hide_toolbar: bool,
    #[serde(default)]
    pub gui_font_size: f32,
    #[serde(default)]
    pub gui_light: bool,
    /// Empty/omarchy follows the desktop; light/dark explicitly override it.
    #[serde(default)]
    pub gui_theme: String,
}

fn path() -> PathBuf {
    bible::data_dir().join("study.json")
}

impl Study {
    pub fn load() -> Study {
        Self::load_from(path())
    }

    pub(crate) fn load_from(path: PathBuf) -> Study {
        let mut state = Persistence {
            path: path.clone(),
            expected: None,
            blocked: None,
            preserve_corrupt: false,
        };
        let mut warning = None;
        let mut study = match storage::read_optional(&path) {
            Ok(Some(bytes)) => {
                state.expected = Some(bytes.clone());
                match serde_json::from_slice::<Study>(&bytes) {
                    Ok(study) => study,
                    Err(_) => {
                        let recovered = fs::read(path.with_extension("json.bak"))
                            .ok()
                            .and_then(|b| serde_json::from_slice::<Study>(&b).ok());
                        if let Some(study) = recovered {
                            state.preserve_corrupt = true;
                            warning = Some(format!(
                                "Recovered study data from {}. Recent changes may be missing; the damaged file will be preserved.",
                                path.with_extension("json.bak").display()
                            ));
                            study
                        } else {
                            state.blocked = Some(format!(
                                "Cannot read damaged study file {}. Restore it from a backup and reopen the app; the original file is untouched.",
                                path.display()
                            ));
                            Study::default()
                        }
                    }
                }
            }
            Ok(None) => Study::default(),
            Err(e) => {
                state.blocked = Some(format!("Cannot read {}: {e}", path.display()));
                Study::default()
            }
        };
        study.load_warning = warning.or_else(|| state.blocked.clone());
        study.persistence = Some(state);
        study
    }

    pub fn save(&mut self) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self)?;
        let state = self
            .persistence
            .as_mut()
            .ok_or_else(|| io::Error::other("Study storage has not been opened"))?;
        if let Some(error) = &state.blocked {
            return Err(io::Error::other(error.clone()));
        }
        // Unchanged autosaves need no disk I/O.
        if state.expected.as_deref() == Some(bytes.as_slice()) {
            return Ok(());
        }
        let parent = state
            .path
            .parent()
            .ok_or_else(|| io::Error::other("Invalid study path"))?;
        fs::create_dir_all(parent)?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(state.path.with_extension("lock"))?;
        lock.try_lock()
            .map_err(|_| io::Error::other("Another window is saving study data. Try again."))?;
        let current = storage::read_optional(&state.path)?;
        if current != state.expected {
            return Err(io::Error::other(
                "Study data changed in another window. Your changes are still in memory; export them before reopening this window.",
            ));
        }
        if let Some(previous) = &current {
            if state.preserve_corrupt {
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                storage::atomic_write(
                    &state.path.with_extension(format!("json.corrupt.{stamp}")),
                    previous,
                )?;
            } else {
                storage::atomic_write(&state.path.with_extension("json.bak"), previous)?;
            }
        }
        storage::atomic_write(&state.path, &bytes)?;
        state.expected = Some(bytes);
        state.preserve_corrupt = false;
        Ok(())
    }

    /// Rescue in-memory edits when normal saving is blocked or conflicts.
    pub fn export_recovery(&self) -> io::Result<PathBuf> {
        let state = self
            .persistence
            .as_ref()
            .ok_or_else(|| io::Error::other("Study storage has not been opened"))?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = state.path.with_extension(format!("json.recovery.{stamp}"));
        storage::atomic_write(&path, &serde_json::to_vec_pretty(self)?)?;
        Ok(path)
    }

    pub fn is_bookmarked(&self, p: Position) -> bool {
        self.bookmarks.contains(&p)
    }

    /// Returns true when the bookmark was added, false when removed.
    pub fn toggle_bookmark(&mut self, p: Position) -> bool {
        if let Some(i) = self.bookmarks.iter().position(|b| *b == p) {
            self.bookmarks.remove(i);
            false
        } else {
            self.bookmarks.push(p);
            self.bookmarks.sort_by_key(|b| (b.book, b.chapter, b.verse));
            true
        }
    }

    pub fn highlight(&self, p: Position) -> u8 {
        self.highlights.get(&p.key()).copied().unwrap_or(0)
    }

    pub fn cycle_highlight(&mut self, p: Position) -> u8 {
        let next = ((self.highlight(p) as usize + 1) % HIGHLIGHT_NAMES.len()) as u8;
        if next == 0 {
            self.highlights.remove(&p.key());
        } else {
            self.highlights.insert(p.key(), next);
        }
        next
    }

    pub fn note(&self, p: Position) -> Option<&str> {
        self.notes.get(&p.key()).map(|s| s.as_str())
    }

    pub fn set_note(&mut self, p: Position, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            self.notes.remove(&p.key());
        } else {
            self.notes.insert(p.key(), trimmed.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::TestDir;
    const POS: Position = Position {
        book: 43,
        chapter: 3,
        verse: 16,
    };

    #[test]
    fn backups_recover_corruption_and_preserve_the_damaged_original() {
        let dir = TestDir::new();
        let path = dir.0.join("study.json");
        let mut study = Study::load_from(path.clone());
        study.set_note(POS, "first note");
        study.save().unwrap();
        study.set_note(POS, "second note");
        study.save().unwrap();
        assert_eq!(
            Study::load_from(path.clone()).note(POS),
            Some("second note")
        );
        fs::write(&path, b"interrupted data").unwrap();
        let mut recovered = Study::load_from(path.clone());
        assert_eq!(recovered.note(POS), Some("first note"));
        assert!(recovered.load_warning.is_some());
        recovered.save().unwrap();
        assert_eq!(Study::load_from(path).note(POS), Some("first note"));
        let corrupt = fs::read_dir(&dir.0)
            .unwrap()
            .flatten()
            .find(|f| f.file_name().to_string_lossy().contains(".corrupt."))
            .unwrap();
        assert_eq!(fs::read(corrupt.path()).unwrap(), b"interrupted data");
    }

    #[test]
    fn unrecoverable_file_is_never_overwritten_and_edits_can_be_exported() {
        let dir = TestDir::new();
        let path = dir.0.join("study.json");
        fs::write(&path, b"broken").unwrap();
        let mut study = Study::load_from(path.clone());
        study.set_note(POS, "rescue this");
        assert!(study.save().is_err());
        assert_eq!(fs::read(path).unwrap(), b"broken");
        let exported = study.export_recovery().unwrap();
        assert_eq!(Study::load_from(exported).note(POS), Some("rescue this"));
    }

    #[test]
    fn stale_window_cannot_overwrite_newer_notes() {
        let dir = TestDir::new();
        let path = dir.0.join("study.json");
        let mut first = Study::load_from(path.clone());
        let mut second = Study::load_from(path.clone());
        first.set_note(POS, "newer");
        first.save().unwrap();
        second.set_note(POS, "stale");
        assert!(
            second
                .save()
                .unwrap_err()
                .to_string()
                .contains("another window")
        );
        assert_eq!(Study::load_from(path).note(POS), Some("newer"));
        assert_eq!(second.note(POS), Some("stale"));
    }

    #[test]
    fn locked_and_unwritable_storage_return_errors_and_allow_retry() {
        let dir = TestDir::new();
        let path = dir.0.join("study.json");
        let mut study = Study::load_from(path.clone());
        let lock = fs::File::create(path.with_extension("lock")).unwrap();
        lock.lock().unwrap();
        assert!(study.save().is_err());
        drop(lock);
        study.save().unwrap();
        fs::create_dir(path.with_extension("json.bak")).unwrap();
        study.set_note(POS, "pending");
        assert!(study.save().is_err());
        assert_eq!(Study::load_from(path.clone()).note(POS), None);
        fs::remove_dir(path.with_extension("json.bak")).unwrap();
        study.save().unwrap();
        assert_eq!(Study::load_from(path).note(POS), Some("pending"));
    }

    #[test]
    fn round_trip_preserves_unknown_fields_drafts_and_invalid_highlights_do_not_panic() {
        let dir = TestDir::new();
        let path = dir.0.join("study.json");
        fs::write(
            &path,
            br#"{"future_setting":{"enabled":true},"highlights":{"43:3:16":255}}"#,
        )
        .unwrap();
        let mut study = Study::load_from(path.clone());
        study.note_draft = Some(NoteDraft {
            position: POS,
            text: "unfinished".into(),
        });
        study.cycle_highlight(POS);
        study.save().unwrap();
        let saved: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["future_setting"]["enabled"], true);
        assert_eq!(
            Study::load_from(path).note_draft.unwrap().text,
            "unfinished"
        );
    }
}
