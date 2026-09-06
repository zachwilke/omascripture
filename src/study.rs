//! Persistent study data: bookmarks, highlights, notes, and reading position.

use crate::bible::{self, Position};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;

pub const HIGHLIGHT_NAMES: [&str; 5] = ["none", "yellow", "green", "blue", "red"];

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Study {
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
}

fn path() -> PathBuf {
    bible::data_dir().join("study.json")
}

impl Study {
    pub fn load() -> Study {
        fs::read_to_string(path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> io::Result<()> {
        fs::create_dir_all(bible::data_dir())?;
        let json = serde_json::to_string_pretty(self)?;
        let tmp = path().with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(tmp, path())
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
        let next = (self.highlight(p) + 1) % HIGHLIGHT_NAMES.len() as u8;
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
