//! Application state and key handling.

use crate::providers;

use crate::bible::{self, Loc, Position, SearchHit, Translation, TranslationInfo};
use crate::harmony;
use crate::menu::{self, MENUS};
use crate::plans;
use crate::resources::{self, LexEntry, Store, Word};
use crate::study::{HIGHLIGHT_NAMES, Study};
use crate::votd;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

pub const SEARCH_LIMIT: usize = 2000;
pub const OCCURRENCE_LIMIT: usize = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Read,
    Books,
    Chapters,
    Translations,
    Search,
    SearchResults,
    GoTo,
    Note,
    Bookmarks,
    Help,
    Resources,
    Harmony,
    Plans,
    DictSearch,
    DictEntry,
    WordStudy,
    Occurrences,
    Menu,
    Settings,
}

/// A region of the screen the mouse can scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollArea {
    Main,
    Parallel,
    Side,
    List,
    Text,
    Chapters,
}

/// What a click on a recorded region does.
#[derive(Debug, Clone)]
pub enum Action {
    /// Press a key in the current mode.
    Key(KeyCode, KeyModifiers),
    /// Select a verse in the reading pane.
    Verse(Position),
    /// Click on empty reading-pane space: focus the text.
    FocusMain,
    /// Click on sidebar body: focus the sidebar.
    FocusSide,
    Tab(u8),
    /// Sidebar item: select and open.
    SideItem(usize),
    /// List item: select; a second click (or double-click) opens it.
    Select(usize),
    /// List item: select and open at once.
    Open(usize),
    Chapter(usize),
    Menu(usize),
    MenuItem(usize),
    /// Settings row: select it and move its value forward or back.
    Adjust(usize, i8),
    Scroll(ScrollArea),
    /// Click outside a popup: same as Esc.
    Dismiss,
    /// Swallow the click (popup body).
    None,
}

/// A clickable region recorded while drawing.
#[derive(Debug, Clone)]
pub struct Hit {
    pub rect: Rect,
    pub action: Action,
}

/// Which slot a translation picker selection applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Primary,
    Parallel,
}

/// Messages from background worker threads.
pub enum Msg {
    Chapter {
        id: String,
        slot: Slot,
        book: usize,
        chapter: usize,
        result: Result<providers::Passage, String>,
    },
    Catalog(Result<Vec<TranslationInfo>, String>),
    Loaded {
        generation: u64,
        abbr: String,
        slot: Slot,
        result: Result<Translation, String>,
    },
    Installed {
        id: String,
        result: Result<(), String>,
    },
}

#[derive(Default)]
pub struct Pane {
    pub scroll: usize,
}

/// Sidebar tabs. 0 = hidden.
pub const TAB_REFS: u8 = 1;
pub const TAB_WORDS: u8 = 2;
pub const TAB_COMMENTARY: u8 = 3;
pub const TAB_NOTES: u8 = 4;
pub const TAB_NAMES: [&str; 5] = ["", "Cross-refs", "Interlinear", "Commentary", "Notes"];

#[derive(Debug, Clone)]
pub struct RefItem {
    pub pos: Position,
    pub label: String,
    pub text: String,
    pub source: &'static str,
}

#[derive(Clone)]
pub enum SideContent {
    Empty(String),
    Refs(Vec<RefItem>),
    Words(Vec<Word>),
    Text { title: String, body: String },
}

pub struct SideCache {
    pub key: (Position, u8, String),
    pub content: SideContent,
}

pub struct WordStudy {
    pub strong: String,
    pub word: Option<Word>,
    pub entry: Option<LexEntry>,
    pub occurrences: Option<(usize, Vec<(Position, String)>)>,
}

pub struct HarmonyItem {
    pub title: &'static str,
    pub range: harmony::Range,
    pub current: bool,
}

pub struct App {
    pub onboarding: bool,
    pub mode: Mode,
    pub translation: Option<Translation>,
    pub parallel: Option<Translation>,
    pub parallel_visible: bool,
    pub loc: Loc,
    pub study: Study,
    pub main_pane: Pane,
    pub side_pane: Pane,
    pub history: Vec<Position>,
    pub status: Option<(String, Instant)>,
    pub busy: Option<String>,
    pub should_quit: bool,
    pub save_error: Option<String>,
    last_checkpoint: Instant,
    load_generation: u64,
    load_pending: [Option<u64>; 2],
    install_pending: bool,
    catalog_pending: bool,

    // pickers
    pub filter: String,
    pub list_index: usize,
    pub catalog: Vec<TranslationInfo>,
    pub installed: Vec<TranslationInfo>,
    pub picker_slot: Slot,
    pub chapter_pick: usize,
    pub book_pick: usize,
    pub number_buffer: String,

    // search
    pub search_query: String,
    pub search_hits: Vec<SearchHit>,
    pub search_index: usize,

    // note editor
    pub note_buffer: String,

    // scrolling for text popups (help, dictionary entry, word study)
    pub text_scroll: u16,

    /// Reference passed on the command line, applied once a translation loads.
    pub pending_reference: Option<String>,
    /// Column count of the last drawn chapter grid (for j/k navigation).
    pub last_grid_cols: usize,

    // study sidebar
    pub sidebar: u8,
    pub focus_side: bool,
    pub side_index: usize,
    pub side_scroll: usize,
    pub side_cache: Option<SideCache>,
    pub resources: Store,

    // study popups
    pub harmony_items: Vec<HarmonyItem>,
    pub dict_results: Vec<(String, String, String)>, // (source id, source name, key)
    pub dict_entry: Option<(String, String)>,        // (title, body)
    pub word_study: Option<WordStudy>,
    pub plan_day: Option<usize>,
    pub install_all: bool,

    // mouse
    pub hits: Vec<Hit>,
    pub menu: usize,
    /// Mode to return to after the translation picker (settings page).
    pub return_to: Option<Mode>,
    first_load: bool,
    chapter_pending: [bool; 2],

    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

fn worker<T>(work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).unwrap_or_else(|_| {
        Err("Background operation failed unexpectedly. Please try again.".into())
    })
}

impl App {
    pub fn new() -> App {
        let (tx, rx) = mpsc::channel();
        let study = Study::load();
        App {
            onboarding: false,
            mode: Mode::Read,
            translation: None,
            parallel: None,
            parallel_visible: false,
            loc: Loc {
                book: 0,
                chapter: 0,
                verse: 0,
            },
            sidebar: match study.sidebar_start.as_str() {
                "hidden" => 0,
                "shown" => study.sidebar.clamp(1, 4),
                _ if study.sidebar_visible.unwrap_or(study.sidebar != 0) => study.sidebar.min(4),
                _ => 0,
            },
            study,
            main_pane: Pane::default(),
            side_pane: Pane::default(),
            history: Vec::new(),
            status: None,
            busy: None,
            should_quit: false,
            save_error: None,
            last_checkpoint: Instant::now(),
            load_generation: 0,
            load_pending: [None; 2],
            install_pending: false,
            catalog_pending: false,
            filter: String::new(),
            list_index: 0,
            catalog: bible::cached_catalog(),
            installed: bible::installed(),
            picker_slot: Slot::Primary,
            chapter_pick: 0,
            book_pick: 0,
            number_buffer: String::new(),
            search_query: String::new(),
            search_hits: Vec::new(),
            search_index: 0,
            note_buffer: String::new(),
            text_scroll: 0,
            pending_reference: None,
            last_grid_cols: 10,
            focus_side: false,
            side_index: 0,
            side_scroll: 0,
            side_cache: None,
            resources: Store::default(),
            harmony_items: Vec::new(),
            dict_results: Vec::new(),
            dict_entry: None,
            word_study: None,
            plan_day: None,
            install_all: false,
            hits: Vec::new(),
            menu: 0,
            return_to: None,
            first_load: true,
            chapter_pending: [false; 2],
            tx,
            rx,
        }
    }

    /// Restore state from disk and honour command-line overrides.
    pub fn start_gui(&mut self, translation_override: Option<String>, reference: Option<String>) {
        if translation_override.is_none()
            && !self.study.onboarding_complete.unwrap_or(self.study.translation.is_some()) {
            self.onboarding = true;
            self.study.onboarding_complete = Some(false);
            self.pending_reference = reference;
        } else {
            self.start(translation_override, reference);
        }
    }

    pub fn choose_initial_translation(&mut self, id: &str) {
        self.status = None;
        self.mode = Mode::Read;
        if bible::is_installed(id) || providers::is_online(id) {
            self.busy = Some(format!("Opening {}…", id.to_uppercase()));
            self.spawn_load(id.into(), Slot::Primary);
        } else {
            self.spawn_download(id.into(), Slot::Primary);
        }
    }

    pub fn finish_onboarding(&mut self) -> bool {
        let Some(t) = &self.translation else { return false };
        if t.online.as_ref().is_some_and(|online| online.error.is_some()
            || online.loaded != Some((self.loc.book, self.loc.chapter))) {
            return false;
        }
        self.study.onboarding_complete = Some(true);
        if self.save() {
            self.onboarding = false;
            true
        } else {
            self.study.onboarding_complete = Some(false);
            false
        }
    }

    /// Restore state from disk and honour command-line overrides.
    pub fn start(&mut self, translation_override: Option<String>, reference: Option<String>) {
        let abbr = translation_override
            .or_else(|| self.study.translation.clone())
            .or_else(|| self.installed.first().map(|i| i.abbreviation.clone()));
        self.parallel_visible = self.study.parallel_visible;
        match abbr {
            Some(a) if bible::is_installed(&a) || providers::is_online(&a) => {
                self.busy = Some(format!("Loading {}…", a.to_uppercase()));
                self.spawn_load(a, Slot::Primary);
            }
            Some(a) => {
                self.set_status(format!(
                    "Translation '{a}' is not installed. Pick one to download."
                ));
                self.open_translations(Slot::Primary);
            }
            None => {
                self.set_status("Welcome! Choose a Bible to start reading.".to_string());
                self.open_translations(Slot::Primary);
            }
        }
        if let Some(p) = self.study.parallel.clone()
            && (bible::is_installed(&p) || providers::is_online(&p))
        {
            self.spawn_load(p, Slot::Parallel);
        }
        self.pending_reference = reference;
        if self.catalog.is_empty() {
            self.refresh_catalog();
        }
    }

    // ---------------------------------------------------------------------
    // Background work
    // ---------------------------------------------------------------------

    pub fn refresh_catalog(&mut self) {
        if self.catalog_pending {
            return;
        }
        self.catalog_pending = true;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let _ = tx.send(Msg::Catalog(worker(bible::fetch_catalog)));
        });
    }

    fn begin_load(&mut self, slot: Slot) -> u64 {
        self.load_generation += 1;
        self.load_pending[if slot == Slot::Primary { 0 } else { 1 }] = Some(self.load_generation);
        self.load_generation
    }

    pub fn cancel_parallel_load(&mut self) {
        self.load_pending[1] = None;
        self.clear_finished_busy();
    }

    fn clear_finished_busy(&mut self) {
        if self.load_pending.iter().all(Option::is_none) && !self.install_pending {
            self.busy = None;
        }
    }

    fn spawn_load(&mut self, abbr: String, slot: Slot) {
        let generation = self.begin_load(slot);
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = worker(|| {
                if providers::is_online(&abbr) {
                    providers::open(&abbr)
                } else {
                    bible::load(&abbr).map_err(|e| e.to_string())
                }
            });
            let _ = tx.send(Msg::Loaded {
                generation,
                abbr,
                slot,
                result,
            });
        });
    }

    fn spawn_download(&mut self, abbr: String, slot: Slot) {
        let generation = self.begin_load(slot);
        self.busy = Some(format!("Downloading {}…", abbr.to_uppercase()));
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = worker(|| bible::download(&abbr));
            let _ = tx.send(Msg::Loaded {
                generation,
                abbr,
                slot,
                result,
            });
        });
    }

    fn spawn_install(&mut self, id: String) {
        self.install_pending = true;
        let name = resources::pack(&id).map(|p| p.name).unwrap_or("resource");
        self.busy = Some(format!("Downloading {name}…"));
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = worker(|| resources::install(&id));
            let _ = tx.send(Msg::Installed { id, result });
        });
    }

    /// Drain worker messages. Returns true if anything changed.
    pub fn background_work_pending(&self) -> bool {
        self.catalog_pending || self.install_pending
            || self.load_pending.iter().any(Option::is_some)
            || self.chapter_pending.iter().any(|pending| *pending)
    }

    pub fn poll_messages(&mut self) -> bool {
        let mut changed = false;
        while let Ok(msg) = self.rx.try_recv() {
            changed = true;
            match msg {
                Msg::Chapter {
                    id,
                    slot,
                    book,
                    chapter,
                    result,
                } => {
                    self.chapter_pending[if slot == Slot::Primary { 0 } else { 1 }] = false;
                    self.accept_chapter(&id, slot, book, chapter, result);
                }
                Msg::Catalog(Ok(list)) => {
                    self.catalog_pending = false;
                    self.catalog = list;
                    if self.mode == Mode::Translations {
                        self.set_status(format!("{} translations available", self.catalog.len()));
                    }
                }
                Msg::Catalog(Err(e)) => {
                    self.catalog_pending = false;
                    if self.catalog.is_empty() {
                        self.set_status(format!("Could not fetch translation list: {e}"));
                    }
                }
                Msg::Loaded {
                    generation,
                    abbr,
                    slot,
                    result,
                } => {
                    let index = if slot == Slot::Primary { 0 } else { 1 };
                    if self.load_pending[index] != Some(generation) {
                        continue;
                    }
                    self.load_pending[index] = None;
                    self.clear_finished_busy();
                    match result {
                        Ok(t) => self.install_translation(t, slot),
                        Err(e) => self.set_status(format!("Failed to load {abbr}: {e}")),
                    }
                }
                Msg::Installed { id, result } => {
                    self.install_pending = false;
                    self.clear_finished_busy();
                    match result {
                        Ok(()) => {
                            let name = resources::pack(&id).map(|p| p.name).unwrap_or("resource");
                            self.set_status(format!("Installed {name}"));
                            self.resources.reset();
                            self.side_cache = None;
                            let is_commentary = resources::pack(&id)
                                .map(|p| p.kind == resources::Kind::Commentary && id != "tsk")
                                .unwrap_or(false);
                            if self.study.commentary.is_none() && is_commentary {
                                self.study.commentary = Some(id.clone());
                                self.save();
                            }
                            if self.install_all {
                                match resources::PACKS
                                    .iter()
                                    .find(|p| !resources::is_installed(p.id))
                                {
                                    Some(p) => self.spawn_install(p.id.to_string()),
                                    None => {
                                        self.install_all = false;
                                        self.set_status("All study resources installed".into());
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            self.install_all = false;
                            self.set_status(format!("Install of {id} failed: {e}"));
                        }
                    }
                }
            }
        }
        self.ensure_online_chapters();
        if self.last_checkpoint.elapsed() >= Duration::from_secs(3) {
            self.last_checkpoint = Instant::now();
            if self.translation.is_some() {
                self.save();
            }
        }
        changed
    }

    fn online_target(&self, slot: Slot) -> Option<(String, usize, usize)> {
        let primary = self.translation.as_ref()?;
        let (t, loc) = if slot == Slot::Primary {
            (primary, self.loc)
        } else {
            if !self.parallel_visible {
                return None;
            }
            let t = self.parallel.as_ref()?;
            (t, t.loc_from_position(primary.position_from_loc(self.loc))?)
        };
        t.online.as_ref()?;
        Some((t.abbreviation.clone(), loc.book, loc.chapter))
    }

    fn ensure_online_chapters(&mut self) {
        for (i, slot) in [Slot::Primary, Slot::Parallel].into_iter().enumerate() {
            if self.chapter_pending[i] {
                continue;
            }
            let Some((id, book, chapter)) = self.online_target(slot) else {
                continue;
            };
            let t = if slot == Slot::Primary {
                self.translation.as_mut()
            } else {
                self.parallel.as_mut()
            }
            .unwrap();
            let online = t.online.as_mut().unwrap();
            if online.loaded == Some((book, chapter)) {
                continue;
            }
            // Evict the last chapter before fetching another. No growing cache.
            if let Some((b, c)) = online.loaded.take() {
                t.books[b].chapters[c] = providers::placeholder(b, c);
            }
            online.error = None;
            online.notice.clear();
            self.chapter_pending[i] = true;
            let tx = self.tx.clone();
            thread::spawn(move || {
                let result = worker(|| providers::fetch(&id, book, chapter));
                let _ = tx.send(Msg::Chapter {
                    id,
                    slot,
                    book,
                    chapter,
                    result,
                });
            });
        }
    }

    fn accept_chapter(
        &mut self,
        id: &str,
        slot: Slot,
        book: usize,
        chapter: usize,
        result: Result<providers::Passage, String>,
    ) {
        // The user may have navigated or switched translations during the request.
        if self.online_target(slot) != Some((id.to_string(), book, chapter)) {
            return;
        }
        let selected = self
            .translation
            .as_ref()
            .map(|t| t.position_from_loc(self.loc));
        let t = if slot == Slot::Primary {
            self.translation.as_mut()
        } else {
            self.parallel.as_mut()
        }
        .unwrap();
        let online = t.online.as_mut().unwrap();
        online.loaded = Some((book, chapter));
        match result {
            Ok(passage) => {
                online.notice = passage.notice;
                online.error = None;
                t.books[book].chapters[chapter].verses = passage.verses;
                if slot == Slot::Primary
                    && let Some(loc) = selected.and_then(|p| t.loc_from_position(p))
                {
                    self.loc = loc;
                }
                self.side_cache = None;
            }
            Err(e) => {
                online.error = Some(e);
            }
        }
    }

    fn install_translation(&mut self, t: Translation, slot: Slot) {
        self.installed = bible::installed();
        match slot {
            Slot::Primary => {
                let previous = self
                    .translation
                    .as_ref()
                    .map(|old| old.position_from_loc(self.loc));
                let name = t.translation.clone();
                let abbr = t.abbreviation.clone();
                self.search_hits.clear();
                self.search_index = 0;
                self.translation = Some(t);
                self.study.translation = Some(abbr);
                let target = previous.or(self.study.last);
                let loc = target
                    .and_then(|p| self.translation.as_ref().unwrap().loc_from_position(p))
                    .unwrap_or(Loc {
                        book: 0,
                        chapter: 0,
                        verse: 0,
                    });
                self.loc = loc;
                if self.mode == Mode::Translations {
                    self.finish_translation_picker();
                }
                let first = std::mem::replace(&mut self.first_load, false);
                let had_reference = self.pending_reference.is_some();
                if let Some(r) = self.pending_reference.take() {
                    self.goto_reference(&r);
                } else if first {
                    match self.study.start.as_str() {
                        "votd" => self.goto_votd(),
                        "plan" => self.goto_plan_today(),
                        _ => {}
                    }
                }
                if first
                    && !had_reference
                    && let Some(draft) = self.study.note_draft.clone()
                    && let Some(loc) = self
                        .translation
                        .as_ref()
                        .and_then(|t| t.loc_from_position(draft.position))
                {
                    self.loc = loc;
                    self.note_buffer = draft.text;
                    self.mode = Mode::Note;
                }
                self.set_status(format!("Reading {name}"));
                self.save();
            }
            Slot::Parallel => {
                let abbr = t.abbreviation.clone();
                let restoring = self.parallel.is_none()
                    && self.study.parallel.as_deref() == Some(&abbr)
                    && !(self.mode == Mode::Translations && self.picker_slot == Slot::Parallel);
                self.parallel = Some(t);
                self.study.parallel = Some(abbr);
                self.parallel_visible = if restoring {
                    self.study.parallel_visible
                } else {
                    true
                };
                self.study.parallel_visible = self.parallel_visible;
                if self.mode == Mode::Translations {
                    self.finish_translation_picker();
                }
                self.save();
            }
        }
    }

    // ---------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------

    pub fn set_status(&mut self, s: String) {
        self.status = Some((s, Instant::now()));
    }

    pub fn status_text(&self) -> Option<&str> {
        if let Some(error) = &self.save_error {
            return Some(error);
        }
        if let Some(warning) = &self.study.load_warning {
            return Some(warning);
        }
        match &self.status {
            Some((s, at)) if at.elapsed() < Duration::from_secs(6) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn position(&self) -> Option<Position> {
        self.translation
            .as_ref()
            .map(|t| t.position_from_loc(self.loc))
    }

    pub fn save(&mut self) -> bool {
        if let Some(position) = self.position() {
            self.study.last = Some(position);
            if self.mode == Mode::Note {
                self.study.note_draft = Some(crate::study::NoteDraft {
                    position,
                    text: self.note_buffer.clone(),
                });
            }
        }
        self.study.parallel_visible = self.parallel_visible;
        if self.sidebar != 0 {
            self.study.sidebar = self.sidebar;
        }
        self.study.sidebar_visible = Some(self.sidebar != 0);
        match self.study.save() {
            Ok(()) => {
                self.save_error = None;
                true
            }
            Err(error) => {
                self.save_error = Some(format!("Changes are not saved: {error}"));
                false
            }
        }
    }

    fn push_history(&mut self) {
        if let Some(position) = self.position() {
            self.history.push(position);
        }
        if self.history.len() > 100 {
            self.history.remove(0);
        }
    }

    fn jump(&mut self, loc: Loc) {
        self.push_history();
        self.loc = loc;
        self.mode = Mode::Read;
        self.focus_side = false;
    }

    /// Jump to a Position (book/chapter/verse numbers) if the translation has it.
    pub fn jump_position(&mut self, p: Position) -> bool {
        let Some(t) = &self.translation else {
            return false;
        };
        match t.loc_from_position(p) {
            Some(loc) => {
                self.jump(loc);
                true
            }
            None => {
                self.set_status("That passage is not in the current translation".into());
                false
            }
        }
    }

    pub fn goto_reference(&mut self, input: &str) -> bool {
        let Some(t) = &self.translation else {
            return false;
        };
        match t.parse_reference(input) {
            Some(loc) => {
                self.jump(loc);
                true
            }
            None => {
                self.set_status(format!("No such reference: {input}"));
                false
            }
        }
    }

    fn verse_count(&self) -> usize {
        self.translation
            .as_ref()
            .map(|t| {
                t.books[self.loc.book].chapters[self.loc.chapter]
                    .verses
                    .len()
            })
            .unwrap_or(0)
    }

    fn move_verse(&mut self, delta: isize) {
        let n = self.verse_count();
        if n == 0 {
            return;
        }
        let target = self.loc.verse as isize + delta;
        if target < 0 {
            if self.loc.verse == 0 {
                if self.prev_chapter() {
                    let n = self.verse_count();
                    self.loc.verse = n.saturating_sub(1);
                }
            } else {
                self.loc.verse = 0;
            }
        } else if target as usize >= n {
            if self.loc.verse + 1 >= n {
                self.next_chapter();
            } else {
                self.loc.verse = n - 1;
            }
        } else {
            self.loc.verse = target as usize;
        }
    }

    fn next_chapter(&mut self) -> bool {
        let Some(t) = &self.translation else {
            return false;
        };
        let b = &t.books[self.loc.book];
        if self.loc.chapter + 1 < b.chapters.len() {
            self.loc.chapter += 1;
        } else if self.loc.book + 1 < t.books.len() {
            self.loc.book += 1;
            self.loc.chapter = 0;
        } else {
            return false;
        }
        self.loc.verse = 0;
        true
    }

    fn prev_chapter(&mut self) -> bool {
        let Some(t) = &self.translation else {
            return false;
        };
        if self.loc.chapter > 0 {
            self.loc.chapter -= 1;
        } else if self.loc.book > 0 {
            self.loc.book -= 1;
            self.loc.chapter = t.books[self.loc.book].chapters.len() - 1;
        } else {
            return false;
        }
        self.loc.verse = 0;
        true
    }

    fn move_book(&mut self, delta: isize) {
        let Some(t) = &self.translation else { return };
        let n = t.books.len() as isize;
        let target = (self.loc.book as isize + delta).clamp(0, n - 1) as usize;
        if target != self.loc.book {
            self.push_history();
            self.loc = Loc {
                book: target,
                chapter: 0,
                verse: 0,
            };
        }
    }

    fn finish_translation_picker(&mut self) {
        self.mode = self.return_to.take().unwrap_or(Mode::Read);
        if self.mode == Mode::Settings {
            self.list_index = if self.picker_slot == Slot::Primary {
                0
            } else {
                1
            };
        }
    }

    pub fn open_translations(&mut self, slot: Slot) {
        self.return_to = None;
        self.mode = Mode::Translations;
        self.picker_slot = slot;
        self.filter.clear();
        self.list_index = 0;
        self.installed = bible::installed();
    }

    /// Translations visible in the picker: installed first, then catalog entries.
    pub fn translation_rows(&self) -> Vec<(TranslationInfo, bool)> {
        let f = self.filter.to_lowercase();
        let mut rows: Vec<(TranslationInfo, bool)> =
            self.installed.iter().cloned().map(|i| (i, true)).collect();
        for c in providers::catalog() {
            rows.push((c, false));
        }
        for c in &self.catalog {
            if providers::is_online(&c.abbreviation) {
                continue;
            }
            if !self
                .installed
                .iter()
                .any(|i| i.abbreviation == c.abbreviation)
            {
                rows.push((c.clone(), false));
            }
        }
        if f.is_empty() {
            return rows;
        }
        rows.into_iter()
            .filter(|(i, _)| {
                i.abbreviation.to_lowercase().contains(&f)
                    || i.translation.to_lowercase().contains(&f)
                    || i.language.to_lowercase().contains(&f)
                    || i.lang.to_lowercase().contains(&f)
            })
            .collect()
    }

    pub fn book_rows(&self) -> Vec<(usize, String)> {
        let Some(t) = &self.translation else {
            return Vec::new();
        };
        let f = self.filter.to_lowercase();
        t.books
            .iter()
            .enumerate()
            .filter(|(_, b)| f.is_empty() || b.name.to_lowercase().contains(&f))
            .map(|(i, b)| (i, b.name.clone()))
            .collect()
    }

    pub fn bookmark_rows(&self) -> Vec<(Position, String, String)> {
        let Some(t) = &self.translation else {
            return Vec::new();
        };
        self.study
            .bookmarks
            .iter()
            .map(|p| {
                let (reference, text) = match t.loc_from_position(*p) {
                    Some(l) => (t.reference(l), t.verse_text(l).unwrap_or("").to_string()),
                    None => (p.key(), String::new()),
                };
                (*p, reference, text)
            })
            .collect()
    }

    /// Human reference for a Position in the current translation.
    pub fn label(&self, p: Position) -> String {
        match self
            .translation
            .as_ref()
            .and_then(|t| t.loc_from_position(p).map(|l| t.reference(l)))
        {
            Some(s) => s,
            None => p.key(),
        }
    }

    pub fn text_at(&self, p: Position) -> String {
        self.translation
            .as_ref()
            .and_then(|t| {
                t.loc_from_position(p)
                    .and_then(|l| t.verse_text(l).map(|s| s.to_string()))
            })
            .unwrap_or_default()
    }

    fn copy_to_clipboard(text: &str) -> Result<(), String> {
        let mut child = Command::new("wl-copy")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("wl-copy: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        child.wait().map_err(|e| e.to_string())?;
        Ok(())
    }

    // ---------------------------------------------------------------------
    // Study sidebar
    // ---------------------------------------------------------------------

    pub fn commentary_id(&self) -> Option<String> {
        let ids = Store::commentary_ids();
        match &self.study.commentary {
            Some(c) if ids.contains(&c.as_str()) => Some(c.clone()),
            _ => ids.first().map(|s| s.to_string()),
        }
    }

    /// Sidebar content for the current verse, recomputed when the key changes.
    pub fn side_content(&mut self) -> &SideContent {
        let pos = self.position().unwrap_or(Position {
            book: 1,
            chapter: 1,
            verse: 1,
        });
        let key = (pos, self.sidebar, self.commentary_id().unwrap_or_default());
        if self
            .side_cache
            .as_ref()
            .map(|c| c.key != key)
            .unwrap_or(true)
        {
            let content = self.compute_side(pos);
            self.side_cache = Some(SideCache { key, content });
            self.side_index = 0;
            self.side_scroll = 0;
        }
        &self.side_cache.as_ref().unwrap().content
    }

    fn compute_side(&mut self, pos: Position) -> SideContent {
        match self.sidebar {
            TAB_REFS => {
                let mut items: Vec<RefItem> = Vec::new();
                if let Some(xr) = self.resources.crossrefs() {
                    let refs: Vec<_> = xr.get(pos).iter().take(60).cloned().collect();
                    for r in refs {
                        let mut label = self.label(r.to);
                        if let Some(e) = r.to_end {
                            if e.chapter == r.to.chapter {
                                label.push_str(&format!("-{}", e.verse));
                            } else {
                                label.push_str(&format!("-{}:{}", e.chapter, e.verse));
                            }
                        }
                        items.push(RefItem {
                            pos: r.to,
                            label,
                            text: self.text_at(r.to),
                            source: "openbible",
                        });
                    }
                }
                let tsk_raw = self.resources.commentary("tsk").and_then(|t| t.raw(pos));
                if let Some(raw) = tsk_raw {
                    for (p, _) in parse_tsk(&raw, pos) {
                        if !items.iter().any(|i| i.pos == p) {
                            let (label, text) = (self.label(p), self.text_at(p));
                            items.push(RefItem {
                                pos: p,
                                label,
                                text,
                                source: "tsk",
                            });
                        }
                    }
                }
                if items.is_empty() {
                    if !resources::is_installed("crossrefs") && !resources::is_installed("tsk") {
                        SideContent::Empty("No cross-reference pack installed.\n\nOpen Resources to install OpenBible cross-references or the Treasury of Scripture Knowledge.".into())
                    } else {
                        SideContent::Empty("No cross-references for this verse.".into())
                    }
                } else {
                    SideContent::Refs(items)
                }
            }
            TAB_WORDS => {
                let nt = pos.book >= 40;
                let id = if nt {
                    "interlinear-nt"
                } else {
                    "interlinear-ot"
                };
                if !resources::is_installed(id) {
                    return SideContent::Empty(format!(
                        "The {} interlinear pack is not installed.\n\nOpen Resources to install it.",
                        if nt { "Greek NT" } else { "Hebrew OT" }
                    ));
                }
                let words = self.resources.interlinear.verse(pos);
                if words.is_empty() {
                    SideContent::Empty("No tagged words for this verse.".into())
                } else {
                    SideContent::Words(words)
                }
            }
            TAB_COMMENTARY => {
                let Some(id) = self.commentary_id() else {
                    return SideContent::Empty("No commentary installed.\n\nOpen Resources to install Matthew Henry, Barnes, Clarke, JFB, Calvin, Wesley or the Geneva notes.".into());
                };
                let name = resources::pack(&id).map(|p| p.name).unwrap_or("Commentary");
                match self.resources.commentary(&id) {
                    Some(c) => match c.lookup(pos) {
                        Some((text, from)) => {
                            let title = if from == pos.verse {
                                name.to_string()
                            } else {
                                format!("{name} (on verse {from})")
                            };
                            SideContent::Text { title, body: text }
                        }
                        None => SideContent::Text {
                            title: name.into(),
                            body: "No comment on this passage.".into(),
                        },
                    },
                    None => SideContent::Empty(format!("Could not open {name}.")),
                }
            }
            TAB_NOTES => {
                if !resources::is_installed("uw-notes") {
                    return SideContent::Empty("unfoldingWord Translation Notes are not installed.\n\nOpen Resources to install them.".into());
                }
                let notes = self.resources.notes.verse(pos);
                if notes.is_empty() {
                    let intro = self.resources.notes.chapter_intro(pos);
                    return match intro {
                        Some(n) if pos.verse == 1 => SideContent::Text {
                            title: "Chapter introduction".into(),
                            body: n.text,
                        },
                        _ => SideContent::Text {
                            title: "unfoldingWord notes".into(),
                            body: "No notes on this verse.".into(),
                        },
                    };
                }
                let mut body = String::new();
                for n in notes {
                    if !n.quote.is_empty() {
                        body.push_str(&format!("« {} »\n", n.quote));
                    }
                    body.push_str(&n.text);
                    body.push_str("\n\n");
                }
                SideContent::Text {
                    title: "unfoldingWord notes".into(),
                    body: body.trim_end().to_string(),
                }
            }
            _ => SideContent::Empty(String::new()),
        }
    }

    fn side_len(&mut self) -> usize {
        match self.side_content() {
            SideContent::Refs(v) => v.len(),
            SideContent::Words(v) => v.len(),
            _ => 0,
        }
    }

    pub fn show_tab(&mut self, tab: u8) {
        self.sidebar = tab;
        self.study.sidebar = tab;
        self.side_index = 0;
        self.side_scroll = 0;
    }

    pub fn open_word_study(&mut self, word: Word) {
        let strong = word.strong.clone();
        let entry = self
            .resources
            .lexicon(&strong)
            .and_then(|l| l.get(&strong))
            .cloned();
        self.word_study = Some(WordStudy {
            strong,
            word: Some(word),
            entry,
            occurrences: None,
        });
        self.text_scroll = 0;
        self.mode = Mode::WordStudy;
    }

    fn side_enter(&mut self) {
        let idx = self.side_index;
        enum Act {
            Jump(Position),
            Word(Word),
            None,
        }
        let act = match self.side_content() {
            SideContent::Refs(v) => v.get(idx).map(|r| Act::Jump(r.pos)).unwrap_or(Act::None),
            SideContent::Words(v) => v
                .get(idx)
                .map(|w| Act::Word(w.clone()))
                .unwrap_or(Act::None),
            _ => Act::None,
        };
        match act {
            Act::Jump(p) => {
                self.jump_position(p);
            }
            Act::Word(w) => self.open_word_study(w),
            Act::None => {}
        }
    }

    fn cycle_commentary(&mut self) {
        let ids = Store::commentary_ids();
        if ids.is_empty() {
            self.set_status("No commentaries installed. Open Resources to add a commentary.".into());
            return;
        }
        let cur = self.commentary_id().unwrap_or_default();
        let i = ids
            .iter()
            .position(|s| *s == cur)
            .map(|i| (i + 1) % ids.len())
            .unwrap_or(0);
        self.study.commentary = Some(ids[i].to_string());
        self.save();
        self.show_tab(TAB_COMMENTARY);
        let name = resources::pack(ids[i]).map(|p| p.name).unwrap_or(ids[i]);
        self.set_status(format!("Commentary: {name}"));
    }

    fn open_harmony(&mut self) {
        let Some(pos) = self.position() else { return };
        self.harmony_items.clear();
        for (per, ranges) in harmony::find(pos) {
            for r in ranges {
                self.harmony_items.push(HarmonyItem {
                    title: per.title,
                    range: r,
                    current: r.contains(pos),
                });
            }
        }
        self.list_index = self
            .harmony_items
            .iter()
            .position(|i| !i.current)
            .unwrap_or(0);
        self.mode = Mode::Harmony;
    }

    pub fn dict_search(&mut self) {
        let q = self.filter.clone();
        let mut out = Vec::new();
        if q.trim().is_empty() {
            self.dict_results = out;
            return;
        }
        for id in Store::dictionary_ids() {
            let name = resources::pack(id).map(|p| p.name).unwrap_or(id);
            if let Some(d) = self.resources.dictionary(id) {
                for k in d.search(&q, 25) {
                    out.push((id.to_string(), name.to_string(), k));
                }
            }
        }
        if let Some(w) = self.resources.words() {
            for (slug, title) in w.search(&q, 25) {
                out.push((
                    "uw-words".into(),
                    "unfoldingWord".into(),
                    format!("{title} [{slug}]"),
                ));
            }
        }
        self.dict_results = out;
        self.list_index = 0;
    }

    fn open_dict_entry(&mut self) {
        let Some((id, name, key)) = self.dict_results.get(self.list_index).cloned() else {
            return;
        };
        let body = if id == "uw-words" {
            let slug = key.rsplit('[').next().unwrap_or("").trim_end_matches(']');
            self.resources
                .words()
                .and_then(|w| w.get(slug))
                .map(|w| format!("{}\n\n{}", w.title, clean_md(&w.body)))
        } else {
            self.resources.dictionary(&id).and_then(|d| d.entry(&key))
        };
        self.dict_entry = Some((
            format!("{name}: {key}"),
            body.unwrap_or_else(|| "Entry not found.".into()),
        ));
        self.text_scroll = 0;
        self.mode = Mode::DictEntry;
    }

    pub fn goto_votd(&mut self) {
        let Some(t) = &self.translation else { return };
        if let Some((loc, r)) = votd::today(t) {
            self.jump(loc);
            self.set_status(format!("Verse of the day: {r}"));
        }
    }

    /// Open the first chapter of today's reading-plan entry.
    pub fn goto_plan_today(&mut self) {
        let Some(p) = &self.study.plan else { return };
        let day = p.current_day().min(p.days().saturating_sub(1));
        if let Some((b, c)) = plans::readings(&p.id, day).first().copied() {
            self.jump_position(Position {
                book: b,
                chapter: c,
                verse: 1,
            });
        }
    }

    // ---------------------------------------------------------------------
    // Key handling
    // ---------------------------------------------------------------------

    pub fn handle_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        match self.mode {
            Mode::Read => {
                if self.focus_side && self.sidebar != 0 {
                    self.key_side(key)
                } else {
                    self.key_read(key)
                }
            }
            Mode::Books => self.key_books(key),
            Mode::Chapters => self.key_chapters(key),
            Mode::Translations => self.key_translations(key),
            Mode::Search => self.key_search_input(key),
            Mode::SearchResults => self.key_search_results(key),
            Mode::GoTo => self.key_goto(key),
            Mode::Note => self.key_note(key),
            Mode::Bookmarks => self.key_bookmarks(key),
            Mode::Help => self.key_scroll_popup(key),
            Mode::Resources => self.key_resources(key),
            Mode::Harmony => self.key_harmony(key),
            Mode::Plans => self.key_plans(key),
            Mode::DictSearch => self.key_dict_search(key),
            Mode::DictEntry => self.key_scroll_popup(key),
            Mode::WordStudy => self.key_word_study(key),
            Mode::Occurrences => self.key_occurrences(key),
            Mode::Menu => self.key_menu(key),
            Mode::Settings => self.key_settings(key),
        }
    }

    // ---------------------------------------------------------------------
    // Mouse
    // ---------------------------------------------------------------------

    pub fn handle_mouse(&mut self, m: MouseEvent) {
        let (x, y) = (m.column, m.row);
        let inside = |r: Rect| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height;
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let action = self
                    .hits
                    .iter()
                    .rev()
                    .find(|h| !matches!(h.action, Action::Scroll(_)) && inside(h.rect))
                    .map(|h| h.action.clone());
                if let Some(a) = action {
                    self.run_action(a);
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if self.mode != Mode::Read {
                    self.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
                } else if self.focus_side {
                    self.focus_side = false;
                }
            }
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                let down = m.kind == MouseEventKind::ScrollDown;
                let area = self.hits.iter().rev().find_map(|h| match h.action {
                    Action::Scroll(a) if inside(h.rect) => Some(a),
                    _ => None,
                });
                if let Some(a) = area {
                    self.scroll_area(a, down);
                }
            }
            _ => {}
        }
    }

    fn press(&mut self, code: KeyCode) {
        self.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn run_action(&mut self, action: Action) {
        match action {
            Action::Key(code, mods) => self.handle_key(KeyEvent::new(code, mods)),
            Action::Verse(p) => {
                self.focus_side = false;
                if let Some(loc) = self
                    .translation
                    .as_ref()
                    .and_then(|t| t.loc_from_position(p))
                {
                    self.loc = loc;
                }
            }
            Action::FocusMain => self.focus_side = false,
            Action::FocusSide => {
                if self.sidebar != 0 {
                    self.focus_side = true;
                }
            }
            Action::Tab(t) => self.show_tab(t),
            Action::SideItem(i) => {
                self.focus_side = true;
                self.side_index = i;
                self.side_enter();
            }
            Action::Select(i) => {
                // First click selects; clicking the selected item opens it.
                if self.list_index == i {
                    self.press(KeyCode::Enter);
                } else {
                    self.list_index = i;
                }
            }
            Action::Open(i) => {
                self.list_index = i;
                self.press(KeyCode::Enter);
            }
            Action::Chapter(i) => {
                self.chapter_pick = i;
                self.press(KeyCode::Enter);
            }
            Action::Menu(i) => {
                if self.mode == Mode::Menu && self.menu == i {
                    self.mode = Mode::Read;
                } else if matches!(self.mode, Mode::Read | Mode::Menu) {
                    self.mode = Mode::Menu;
                    self.menu = i;
                    self.list_index = 0;
                }
            }
            Action::MenuItem(i) => {
                self.list_index = i;
                self.press(KeyCode::Enter);
            }
            Action::Adjust(i, dir) => {
                self.list_index = i;
                self.press(if dir < 0 {
                    KeyCode::Left
                } else {
                    KeyCode::Right
                });
            }
            Action::Scroll(_) | Action::None => {}
            Action::Dismiss => self.press(KeyCode::Esc),
        }
    }

    fn scroll_area(&mut self, area: ScrollArea, down: bool) {
        match area {
            ScrollArea::Main | ScrollArea::Parallel => {
                if self.mode == Mode::Read {
                    self.move_verse(if down { 1 } else { -1 });
                }
            }
            ScrollArea::Side => {
                if self.mode != Mode::Read {
                    return;
                }
                let is_text = matches!(
                    self.side_content(),
                    SideContent::Text { .. } | SideContent::Empty(_)
                );
                if is_text {
                    self.side_scroll = if down {
                        self.side_scroll + 3
                    } else {
                        self.side_scroll.saturating_sub(3)
                    };
                } else {
                    let n = self.side_len();
                    self.focus_side = true;
                    self.side_index = if down {
                        (self.side_index + 1).min(n.saturating_sub(1))
                    } else {
                        self.side_index.saturating_sub(1)
                    };
                }
            }
            ScrollArea::List | ScrollArea::Chapters => {
                self.press(if down { KeyCode::Down } else { KeyCode::Up })
            }
            ScrollArea::Text => {
                self.text_scroll = if down {
                    self.text_scroll.saturating_add(3)
                } else {
                    self.text_scroll.saturating_sub(3)
                }
            }
        }
    }

    fn key_menu(&mut self, key: KeyEvent) {
        let n = MENUS.len();
        let items = MENUS[self.menu].items;
        match key.code {
            KeyCode::Esc | KeyCode::F(10) => self.mode = Mode::Read,
            KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => {
                self.menu = (self.menu + n - 1) % n;
                self.list_index = 0;
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
                self.menu = (self.menu + 1) % n;
                self.list_index = 0;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.list_index = (self.list_index + 1) % items.len()
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.list_index = (self.list_index + items.len() - 1) % items.len()
            }
            KeyCode::Enter => {
                let code = items[self.list_index.min(items.len() - 1)].code;
                self.mode = Mode::Read;
                self.press(code);
            }
            _ => {
                self.mode = Mode::Read;
                self.handle_key(key);
            }
        }
    }

    /// Footer buttons for the current mode.
    pub fn footer_buttons(&self) -> Vec<menu::Button> {
        menu::footer(self)
    }

    fn key_read(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if self.translation.is_none() {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                KeyCode::Char('t') | KeyCode::Enter => self.open_translations(Slot::Primary),
                KeyCode::Char('?') => self.mode = Mode::Help,
                KeyCode::Char('R') => {
                    self.mode = Mode::Resources;
                    self.list_index = 0;
                }
                KeyCode::Char(',') | KeyCode::Char('S') => self.open_settings(),
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::F(5) => {
                let selected = self
                    .translation
                    .as_ref()
                    .map(|t| t.position_from_loc(self.loc));
                for t in [&mut self.translation, &mut self.parallel]
                    .into_iter()
                    .flatten()
                {
                    if let Some(o) = &mut t.online {
                        if let Some((b, c)) = o.loaded.take() {
                            t.books[b].chapters[c] = providers::placeholder(b, c);
                        }
                        o.error = None;
                    }
                }
                if let Some(loc) =
                    selected.and_then(|p| self.translation.as_ref()?.loc_from_position(p))
                {
                    self.loc = loc;
                }
            }
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => self.move_verse(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_verse(-1),
            KeyCode::Char('d') if ctrl => self.move_verse(5),
            KeyCode::Char('u') if ctrl => self.move_verse(-5),
            KeyCode::Char('J') | KeyCode::PageDown | KeyCode::Char(' ') => self.move_verse(10),
            KeyCode::Char('K') | KeyCode::PageUp => self.move_verse(-10),
            KeyCode::Char('g') | KeyCode::Home => self.loc.verse = 0,
            KeyCode::Char('G') | KeyCode::End => {
                self.loc.verse = self.verse_count().saturating_sub(1)
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.next_chapter();
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.prev_chapter();
            }
            KeyCode::Char(']') | KeyCode::Char('L') => self.move_book(1),
            KeyCode::Char('[') | KeyCode::Char('H') => self.move_book(-1),
            KeyCode::Char('b') => {
                self.mode = Mode::Books;
                self.filter.clear();
                self.book_pick = self.loc.book;
                self.list_index = self.loc.book;
            }
            KeyCode::Char('c') => {
                self.mode = Mode::Chapters;
                self.book_pick = self.loc.book;
                self.chapter_pick = self.loc.chapter;
                self.number_buffer.clear();
            }
            KeyCode::Char('t') => self.open_translations(Slot::Primary),
            KeyCode::Char('T') => self.open_translations(Slot::Parallel),
            KeyCode::Char('p') => {
                if self.parallel.is_some() {
                    self.parallel_visible = !self.parallel_visible;
                    self.study.parallel_visible = self.parallel_visible;
                } else {
                    self.set_status(
                        "No parallel Bible selected. Open Settings and choose a Parallel Bible.".to_string(),
                    );
                    self.open_translations(Slot::Parallel);
                }
            }
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
            }
            KeyCode::Char('n') => self.step_search(1),
            KeyCode::Char('N') => self.step_search(-1),
            KeyCode::Char(':') | KeyCode::Char('o') => {
                self.mode = Mode::GoTo;
                self.filter.clear();
            }
            KeyCode::Char('m') => {
                if let Some(p) = self.position() {
                    let added = self.study.toggle_bookmark(p);
                    self.save();
                    self.set_status(if added {
                        "Bookmarked".into()
                    } else {
                        "Bookmark removed".into()
                    });
                }
            }
            KeyCode::Char('B') => {
                self.mode = Mode::Bookmarks;
                self.list_index = 0;
            }
            KeyCode::Char('x') => {
                if let Some(p) = self.position() {
                    let h = self.study.cycle_highlight(p);
                    self.save();
                    self.set_status(format!("Highlight: {}", HIGHLIGHT_NAMES[h as usize]));
                }
            }
            KeyCode::Char('e') => {
                if let Some(p) = self.position() {
                    self.note_buffer = self.study.note(p).unwrap_or("").to_string();
                    self.mode = Mode::Note;
                }
            }
            KeyCode::Char('y') => {
                if let Some(t) = &self.translation {
                    let Some(verse) = t.verse_text(self.loc) else {
                        self.set_status("Wait for the chapter to load before copying".into());
                        return;
                    };
                    let text = format!(
                        "{} ({})\n{}",
                        t.reference(self.loc),
                        t.abbreviation.to_uppercase(),
                        verse
                    );
                    match Self::copy_to_clipboard(&text) {
                        Ok(()) => self.set_status("Verse copied to clipboard".into()),
                        Err(e) => self.set_status(format!("Copy failed: {e}")),
                    }
                }
            }
            KeyCode::Char('u') | KeyCode::Backspace => {
                if let Some(loc) = self
                    .history
                    .pop()
                    .and_then(|p| self.translation.as_ref()?.loc_from_position(p))
                {
                    self.loc = loc;
                }
            }
            // --- study ---
            KeyCode::Char('s') => {
                if self.sidebar == 0 {
                    let last = self.study.sidebar.clamp(1, 4);
                    self.show_tab(last);
                } else {
                    self.sidebar = 0;
                    self.focus_side = false;
                }
            }
            KeyCode::Char('1') => self.show_tab(TAB_REFS),
            KeyCode::Char('2') => self.show_tab(TAB_WORDS),
            KeyCode::Char('3') => self.show_tab(TAB_COMMENTARY),
            KeyCode::Char('4') => self.show_tab(TAB_NOTES),
            KeyCode::Tab => {
                if self.sidebar == 0 {
                    self.show_tab(self.study.sidebar.clamp(1, 4));
                }
                self.focus_side = true;
            }
            KeyCode::Char('w') => {
                self.show_tab(TAB_WORDS);
                self.focus_side = true;
            }
            KeyCode::Char('C') => self.cycle_commentary(),
            KeyCode::Char('D') => {
                self.mode = Mode::DictSearch;
                self.filter.clear();
                self.dict_results.clear();
                self.list_index = 0;
            }
            KeyCode::Char('P') => self.open_harmony(),
            KeyCode::Char('r') => {
                self.mode = Mode::Plans;
                self.plan_day = None;
                self.list_index = 0;
            }
            KeyCode::Char('R') => {
                self.mode = Mode::Resources;
                self.list_index = 0;
            }
            KeyCode::Char('v') => self.goto_votd(),
            KeyCode::Char('?') | KeyCode::F(1) => {
                self.mode = Mode::Help;
                self.text_scroll = 0;
            }
            KeyCode::F(10) => {
                self.mode = Mode::Menu;
                self.menu = 0;
                self.list_index = 0;
            }
            KeyCode::Char(',') | KeyCode::Char('S') => self.open_settings(),
            _ => {}
        }
    }

    fn key_side(&mut self, key: KeyEvent) {
        let n = self.side_len();
        let is_text = matches!(
            self.side_content(),
            SideContent::Text { .. } | SideContent::Empty(_)
        );
        match key.code {
            KeyCode::Esc | KeyCode::Tab => self.focus_side = false,
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('s') => {
                self.sidebar = 0;
                self.focus_side = false;
            }
            KeyCode::Char('1') => self.show_tab(TAB_REFS),
            KeyCode::Char('2') => self.show_tab(TAB_WORDS),
            KeyCode::Char('3') => self.show_tab(TAB_COMMENTARY),
            KeyCode::Char('4') => self.show_tab(TAB_NOTES),
            KeyCode::Char('C') => self.cycle_commentary(),
            KeyCode::Char('j') | KeyCode::Down => {
                if is_text {
                    self.side_scroll += 1;
                } else if n > 0 {
                    self.side_index = (self.side_index + 1).min(n - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if is_text {
                    self.side_scroll = self.side_scroll.saturating_sub(1);
                } else {
                    self.side_index = self.side_index.saturating_sub(1);
                }
            }
            KeyCode::Char('J') | KeyCode::PageDown | KeyCode::Char(' ') => {
                if is_text {
                    self.side_scroll += 10;
                } else if n > 0 {
                    self.side_index = (self.side_index + 10).min(n - 1);
                }
            }
            KeyCode::Char('K') | KeyCode::PageUp => {
                if is_text {
                    self.side_scroll = self.side_scroll.saturating_sub(10);
                } else {
                    self.side_index = self.side_index.saturating_sub(10);
                }
            }
            KeyCode::Char('g') => {
                self.side_index = 0;
                self.side_scroll = 0;
            }
            KeyCode::Char('G') => {
                self.side_index = n.saturating_sub(1);
                self.side_scroll = usize::MAX / 2;
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.prev_chapter();
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.next_chapter();
            }
            KeyCode::Char('n') => self.move_verse(1),
            KeyCode::Char('p') => self.move_verse(-1),
            KeyCode::Enter => self.side_enter(),
            _ => self.key_read(key),
        }
    }

    fn step_search(&mut self, delta: isize) {
        if self.search_hits.is_empty() {
            self.set_status("No search results yet. Open Search to find a word or phrase.".into());
            return;
        }
        let n = self.search_hits.len() as isize;
        self.search_index = ((self.search_index as isize + delta).rem_euclid(n)) as usize;
        let loc = self.search_hits[self.search_index].loc;
        self.jump(loc);
        self.set_status(format!(
            "Result {}/{} for \"{}\"",
            self.search_index + 1,
            self.search_hits.len(),
            self.search_query
        ));
    }

    fn list_nav(&mut self, key: &KeyEvent, n: usize) -> bool {
        if n == 0 {
            return false;
        }
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.list_index = (self.list_index + 1) % n,
            KeyCode::Char('k') | KeyCode::Up => self.list_index = (self.list_index + n - 1) % n,
            KeyCode::PageDown | KeyCode::Char('J') => {
                self.list_index = (self.list_index + 10).min(n - 1)
            }
            KeyCode::PageUp | KeyCode::Char('K') => {
                self.list_index = self.list_index.saturating_sub(10)
            }
            KeyCode::Char('g') => self.list_index = 0,
            KeyCode::Char('G') => self.list_index = n - 1,
            _ => return false,
        }
        true
    }

    fn key_books(&mut self, key: KeyEvent) {
        let rows = self.book_rows();
        match key.code {
            KeyCode::Esc => self.mode = Mode::Read,
            KeyCode::Down | KeyCode::Tab => {
                if !rows.is_empty() {
                    self.list_index = (self.list_index + 1) % rows.len();
                }
            }
            KeyCode::Up | KeyCode::BackTab => {
                if !rows.is_empty() {
                    self.list_index = (self.list_index + rows.len() - 1) % rows.len();
                }
            }
            KeyCode::Enter => {
                if let Some((idx, _)) = rows.get(self.list_index) {
                    self.book_pick = *idx;
                    self.chapter_pick = 0;
                    self.number_buffer.clear();
                    let chapters = self.translation.as_ref().unwrap().books[*idx]
                        .chapters
                        .len();
                    if chapters == 1 {
                        self.jump(Loc {
                            book: *idx,
                            chapter: 0,
                            verse: 0,
                        });
                    } else {
                        self.mode = Mode::Chapters;
                    }
                }
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.list_index = 0;
            }
            KeyCode::Char(c) => {
                if c == 'j' && self.filter.is_empty() {
                    if !rows.is_empty() {
                        self.list_index = (self.list_index + 1) % rows.len();
                    }
                } else if c == 'k' && self.filter.is_empty() {
                    if !rows.is_empty() {
                        self.list_index = (self.list_index + rows.len() - 1) % rows.len();
                    }
                } else {
                    self.filter.push(c);
                    self.list_index = 0;
                }
            }
            _ => {}
        }
    }

    pub fn chapter_grid_cols(&self, width: u16) -> usize {
        ((width.saturating_sub(4)) as usize / 5).clamp(1, 10)
    }

    fn key_chapters(&mut self, key: KeyEvent) {
        let Some(t) = &self.translation else { return };
        let n = t.books[self.book_pick].chapters.len();
        let cols = self.last_grid_cols.max(1);
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Read;
                self.number_buffer.clear();
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.chapter_pick = (self.chapter_pick + 1).min(n - 1)
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.chapter_pick = self.chapter_pick.saturating_sub(1)
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.chapter_pick = (self.chapter_pick + cols).min(n - 1)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.chapter_pick = self.chapter_pick.saturating_sub(cols)
            }
            KeyCode::Char('g') => self.chapter_pick = 0,
            KeyCode::Char('G') => self.chapter_pick = n - 1,
            KeyCode::Char('b') => {
                self.mode = Mode::Books;
                self.filter.clear();
            }
            KeyCode::Backspace => {
                self.number_buffer.pop();
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                self.number_buffer.push(c);
                let parsed = self
                    .number_buffer
                    .parse::<usize>()
                    .ok()
                    .filter(|num| *num >= 1 && *num <= n);
                match parsed {
                    Some(num) => self.chapter_pick = num - 1,
                    None => {
                        self.number_buffer = c.to_string();
                        if let Some(num) = self
                            .number_buffer
                            .parse::<usize>()
                            .ok()
                            .filter(|num| *num >= 1 && *num <= n)
                        {
                            self.chapter_pick = num - 1;
                        }
                    }
                }
            }
            KeyCode::Enter => {
                self.number_buffer.clear();
                self.jump(Loc {
                    book: self.book_pick,
                    chapter: self.chapter_pick,
                    verse: 0,
                });
            }
            _ => {}
        }
    }

    fn key_translations(&mut self, key: KeyEvent) {
        let rows = self.translation_rows();
        match key.code {
            KeyCode::Esc => {
                if self.translation.is_some() {
                    self.finish_translation_picker();
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Down | KeyCode::Tab => {
                if !rows.is_empty() {
                    self.list_index = (self.list_index + 1) % rows.len();
                }
            }
            KeyCode::Up | KeyCode::BackTab => {
                if !rows.is_empty() {
                    self.list_index = (self.list_index + rows.len() - 1) % rows.len();
                }
            }
            KeyCode::PageDown => {
                if !rows.is_empty() {
                    self.list_index = (self.list_index + 10).min(rows.len() - 1);
                }
            }
            KeyCode::PageUp => self.list_index = self.list_index.saturating_sub(10),
            KeyCode::Enter => {
                if self.busy.is_some() {
                    self.set_status("Please wait for the current download to finish".into());
                    return;
                }
                if let Some((info, installed)) = rows.get(self.list_index) {
                    let abbr = info.abbreviation.clone();
                    let slot = self.picker_slot;
                    if *installed || providers::is_online(&abbr) {
                        self.busy = Some(format!("Loading {}…", abbr.to_uppercase()));
                        self.spawn_load(abbr, slot);
                    } else {
                        self.spawn_download(abbr, slot);
                    }
                }
            }
            KeyCode::Delete => self.delete_selected_translation(&rows),
            KeyCode::F(5) => {
                self.set_status("Refreshing translation list…".into());
                self.refresh_catalog();
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.list_index = 0;
            }
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match c {
                        'r' => {
                            self.set_status("Refreshing translation list…".into());
                            self.refresh_catalog();
                        }
                        'd' => self.delete_selected_translation(&rows),
                        'n' => {
                            if !rows.is_empty() {
                                self.list_index = (self.list_index + 1) % rows.len();
                            }
                        }
                        'p' if !rows.is_empty() => {
                            self.list_index = (self.list_index + rows.len() - 1) % rows.len();
                        }
                        _ => {}
                    }
                    return;
                }
                self.filter.push(c);
                self.list_index = 0;
            }
            _ => {}
        }
    }

    fn delete_selected_translation(&mut self, rows: &[(TranslationInfo, bool)]) {
        let Some((info, true)) = rows.get(self.list_index) else {
            self.set_status("Only downloaded translations can be deleted".into());
            return;
        };
        let abbr = info.abbreviation.clone();
        let in_use = self
            .translation
            .as_ref()
            .map(|t| t.abbreviation == abbr)
            .unwrap_or(false);
        if in_use {
            self.set_status("Switch to another translation before deleting this one".into());
            return;
        }
        match bible::remove(&abbr) {
            Ok(()) => {
                if self
                    .parallel
                    .as_ref()
                    .map(|t| t.abbreviation == abbr)
                    .unwrap_or(false)
                {
                    self.parallel = None;
                    self.parallel_visible = false;
                    self.study.parallel = None;
                }
                self.installed = bible::installed();
                self.set_status(format!("Deleted {}", abbr.to_uppercase()));
            }
            Err(e) => self.set_status(format!("Delete failed: {e}")),
        }
    }

    fn key_search_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Read,
            KeyCode::Enter => {
                let Some(t) = &self.translation else { return };
                if t.online.is_some() {
                    self.set_status(
                        "Whole-Bible search needs an offline Bible. Choose a downloaded edition from your library."
                            .into(),
                    );
                    self.mode = Mode::Read;
                    return;
                }
                self.search_hits = t.search(&self.search_query, SEARCH_LIMIT);
                self.search_index = 0;
                self.list_index = 0;
                if self.search_hits.is_empty() {
                    self.set_status(format!("No results for \"{}\"", self.search_query));
                    self.mode = Mode::Read;
                } else {
                    self.mode = Mode::SearchResults;
                }
            }
            KeyCode::Backspace => {
                self.search_query.pop();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.search_query.clear()
            }
            KeyCode::Char(c) => self.search_query.push(c),
            _ => {}
        }
    }

    fn key_search_results(&mut self, key: KeyEvent) {
        let n = self.search_hits.len();
        if self.list_nav(&key, n) {
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.mode = Mode::Read,
            KeyCode::Char('/') => self.mode = Mode::Search,
            KeyCode::Enter => {
                if let Some(hit) = self.search_hits.get(self.list_index) {
                    self.search_index = self.list_index;
                    let loc = hit.loc;
                    self.jump(loc);
                }
            }
            _ => {}
        }
    }

    fn key_goto(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Read,
            KeyCode::Enter => {
                let input = self.filter.clone();
                if self.goto_reference(&input) {
                    self.filter.clear();
                } else {
                    self.mode = Mode::Read;
                }
            }
            KeyCode::Backspace => {
                self.filter.pop();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.filter.clear()
            }
            KeyCode::Char(c) => self.filter.push(c),
            _ => {}
        }
    }

    fn key_note(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Read;
                self.study.note_draft = None;
                self.save();
            }
            KeyCode::Char('s') if ctrl => {
                if let Some(p) = self.position() {
                    self.study.set_note(p, &self.note_buffer);
                    self.mode = Mode::Read;
                    self.study.note_draft = None;
                    if self.save() {
                        self.set_status("Note saved".into());
                    } else {
                        self.mode = Mode::Note;
                    }
                }
            }
            KeyCode::Char('u') if ctrl => self.note_buffer.clear(),
            KeyCode::Enter => self.note_buffer.push('\n'),
            KeyCode::Backspace => {
                self.note_buffer.pop();
            }
            KeyCode::Tab => self.note_buffer.push_str("    "),
            KeyCode::Char(c) => self.note_buffer.push(c),
            _ => {}
        }
    }

    fn key_bookmarks(&mut self, key: KeyEvent) {
        let n = self.study.bookmarks.len();
        if self.list_nav(&key, n) {
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('B') => self.mode = Mode::Read,
            KeyCode::Char('d') | KeyCode::Delete => {
                if self.list_index < n {
                    self.study.bookmarks.remove(self.list_index);
                    self.save();
                    if self.list_index >= self.study.bookmarks.len() {
                        self.list_index = self.study.bookmarks.len().saturating_sub(1);
                    }
                }
            }
            KeyCode::Enter => {
                if let Some(p) = self.study.bookmarks.get(self.list_index).copied() {
                    self.jump_position(p);
                }
            }
            _ => {}
        }
    }

    fn key_scroll_popup(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.mode = match self.mode {
                    Mode::DictEntry => Mode::DictSearch,
                    _ => Mode::Read,
                }
            }
            KeyCode::Char('?') | KeyCode::Enter if self.mode == Mode::Help => {
                self.mode = Mode::Read
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.text_scroll = self.text_scroll.saturating_add(1)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.text_scroll = self.text_scroll.saturating_sub(1)
            }
            KeyCode::Char('J') | KeyCode::PageDown | KeyCode::Char(' ') => {
                self.text_scroll = self.text_scroll.saturating_add(10)
            }
            KeyCode::Char('K') | KeyCode::PageUp => {
                self.text_scroll = self.text_scroll.saturating_sub(10)
            }
            KeyCode::Char('g') => self.text_scroll = 0,
            KeyCode::Char('G') => self.text_scroll = u16::MAX / 2,
            _ => {}
        }
    }

    fn key_resources(&mut self, key: KeyEvent) {
        let n = resources::PACKS.len();
        if self.list_nav(&key, n) {
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('R') => self.mode = Mode::Read,
            KeyCode::Enter | KeyCode::Char('i') => {
                let p = &resources::PACKS[self.list_index];
                if resources::is_installed(p.id) {
                    self.set_status(format!("{} is already installed. Choose Remove in Resources to uninstall it.", p.name));
                } else if self.busy.is_some() {
                    self.set_status("Please wait for the current download to finish".into());
                } else {
                    self.spawn_install(p.id.to_string());
                }
            }
            KeyCode::Delete | KeyCode::Char('d') => {
                let p = &resources::PACKS[self.list_index];
                if !resources::is_installed(p.id) {
                    self.set_status("Not installed".into());
                } else {
                    match resources::remove(p.id) {
                        Ok(()) => {
                            self.resources.reset();
                            self.side_cache = None;
                            self.set_status(format!("Removed {}", p.name));
                        }
                        Err(e) => self.set_status(format!("Remove failed: {e}")),
                    }
                }
            }
            KeyCode::Char('a') => {
                if self.busy.is_some() {
                    self.set_status("A download is already running".into());
                } else if let Some(p) = resources::PACKS
                    .iter()
                    .find(|p| !resources::is_installed(p.id))
                {
                    self.install_all = true;
                    self.spawn_install(p.id.to_string());
                    self.set_status("Installing every missing pack, one at a time…".into());
                } else {
                    self.set_status("Everything is already installed".into());
                }
            }
            _ => {}
        }
    }

    fn key_harmony(&mut self, key: KeyEvent) {
        let n = self.harmony_items.len();
        if self.list_nav(&key, n) {
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('P') => self.mode = Mode::Read,
            KeyCode::Enter => {
                if let Some(item) = self.harmony_items.get(self.list_index) {
                    let p = item.range.start_position();
                    self.jump_position(p);
                }
            }
            _ => {}
        }
    }

    pub fn plan_view_day(&self) -> usize {
        match (&self.study.plan, self.plan_day) {
            (_, Some(d)) => d,
            (Some(p), None) => p.current_day().min(p.days().saturating_sub(1)),
            _ => 0,
        }
    }

    fn key_plans(&mut self, key: KeyEvent) {
        if self.study.plan.is_none() {
            let n = plans::PLANS.len();
            if self.list_nav(&key, n) {
                return;
            }
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('r') => self.mode = Mode::Read,
                KeyCode::Enter => {
                    let Some(p) = plans::PLANS.get(self.list_index) else {
                        return;
                    };
                    self.study.plan = Some(plans::Progress::new(p.id));
                    self.save();
                    self.list_index = 0;
                    self.set_status(format!("Started: {}", p.name));
                }
                _ => {}
            }
            return;
        }
        let day = self.plan_view_day();
        let id = self.study.plan.as_ref().unwrap().id.clone();
        let readings = plans::readings(&id, day);
        let days = self.study.plan.as_ref().unwrap().days();
        if self.list_nav(&key, readings.len()) {
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('r') => self.mode = Mode::Read,
            KeyCode::Enter => {
                if let Some((b, c)) = readings.get(self.list_index) {
                    self.jump_position(Position {
                        book: *b,
                        chapter: *c,
                        verse: 1,
                    });
                }
            }
            KeyCode::Char('x') | KeyCode::Char(' ') => {
                let p = self.study.plan.as_mut().unwrap();
                if !p.done.remove(&(day as u32)) {
                    p.done.insert(day as u32);
                }
                self.save();
            }
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Char('n') => {
                self.plan_day = Some((day + 1).min(days.saturating_sub(1)));
                self.list_index = 0;
            }
            KeyCode::Char('h') | KeyCode::Left | KeyCode::Char('p') => {
                self.plan_day = Some(day.saturating_sub(1));
                self.list_index = 0;
            }
            KeyCode::Char('t') => {
                self.plan_day = None;
                self.list_index = 0;
            }
            KeyCode::Char('X') => {
                self.study.plan = None;
                self.save();
                self.plan_day = None;
                self.list_index = 0;
                self.set_status("Reading plan stopped".into());
            }
            _ => {}
        }
    }

    fn key_dict_search(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Read,
            KeyCode::Enter => self.open_dict_entry(),
            KeyCode::Down | KeyCode::Tab => {
                if !self.dict_results.is_empty() {
                    self.list_index = (self.list_index + 1) % self.dict_results.len();
                }
            }
            KeyCode::Up | KeyCode::BackTab => {
                if !self.dict_results.is_empty() {
                    self.list_index =
                        (self.list_index + self.dict_results.len() - 1) % self.dict_results.len();
                }
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.dict_search();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.filter.clear();
                self.dict_search();
            }
            KeyCode::Char(c) => {
                self.filter.push(c);
                self.dict_search();
            }
            _ => {}
        }
    }

    fn key_word_study(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.mode = Mode::Read,
            KeyCode::Char('o') | KeyCode::Enter => {
                let Some(ws) = &self.word_study else { return };
                let strong = ws.strong.clone();
                if ws.occurrences.is_none() {
                    let occ = self
                        .resources
                        .interlinear
                        .occurrences(&strong, OCCURRENCE_LIMIT);
                    self.word_study.as_mut().unwrap().occurrences = Some(occ);
                }
                self.list_index = 0;
                self.mode = Mode::Occurrences;
            }
            KeyCode::Char('y') => {
                if let Some(ws) = &self.word_study {
                    let text = match &ws.entry {
                        Some(e) => format!(
                            "{} {} ({}) — {}\n{}",
                            e.strong, e.lemma, e.translit, e.gloss, e.meaning
                        ),
                        None => ws.strong.clone(),
                    };
                    match Self::copy_to_clipboard(&text) {
                        Ok(()) => self.set_status("Lexicon entry copied".into()),
                        Err(e) => self.set_status(format!("Copy failed: {e}")),
                    }
                }
            }
            _ => self.key_scroll_popup(key),
        }
    }

    fn key_occurrences(&mut self, key: KeyEvent) {
        let n = self
            .word_study
            .as_ref()
            .and_then(|w| w.occurrences.as_ref())
            .map(|o| o.1.len())
            .unwrap_or(0);
        if self.list_nav(&key, n) {
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.mode = Mode::WordStudy,
            KeyCode::Enter => {
                let p = self
                    .word_study
                    .as_ref()
                    .and_then(|w| w.occurrences.as_ref())
                    .and_then(|o| o.1.get(self.list_index))
                    .map(|x| x.0);
                if let Some(p) = p {
                    self.jump_position(p);
                }
            }
            _ => {}
        }
    }
}

fn clean_md(s: &str) -> String {
    s.lines()
        .map(|l| {
            l.trim_start_matches('#')
                .trim()
                .replace("**", "")
                .replace("* ", "• ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// TSK book abbreviations → book number.
fn tsk_book(abbr: &str) -> Option<u32> {
    const T: [(&str, u32); 66] = [
        ("Ge", 1),
        ("Ex", 2),
        ("Le", 3),
        ("Nu", 4),
        ("De", 5),
        ("Jos", 6),
        ("Jud", 7),
        ("Ru", 8),
        ("1Sa", 9),
        ("2Sa", 10),
        ("1Ki", 11),
        ("2Ki", 12),
        ("1Ch", 13),
        ("2Ch", 14),
        ("Ezr", 15),
        ("Ne", 16),
        ("Es", 17),
        ("Job", 18),
        ("Ps", 19),
        ("Pr", 20),
        ("Ec", 21),
        ("So", 22),
        ("Isa", 23),
        ("Jer", 24),
        ("La", 25),
        ("Eze", 26),
        ("Da", 27),
        ("Ho", 28),
        ("Joe", 29),
        ("Am", 30),
        ("Ob", 31),
        ("Jon", 32),
        ("Mic", 33),
        ("Na", 34),
        ("Hab", 35),
        ("Zep", 36),
        ("Hag", 37),
        ("Zec", 38),
        ("Mal", 39),
        ("Mt", 40),
        ("Mr", 41),
        ("Lu", 42),
        ("Joh", 43),
        ("Ac", 44),
        ("Ro", 45),
        ("1Co", 46),
        ("2Co", 47),
        ("Ga", 48),
        ("Eph", 49),
        ("Php", 50),
        ("Col", 51),
        ("1Th", 52),
        ("2Th", 53),
        ("1Ti", 54),
        ("2Ti", 55),
        ("Tit", 56),
        ("Phm", 57),
        ("Heb", 58),
        ("Jas", 59),
        ("1Pe", 60),
        ("2Pe", 61),
        ("1Jo", 62),
        ("2Jo", 63),
        ("3Jo", 64),
        ("Jude", 65),
        ("Re", 66),
    ];
    T.iter().find(|(a, _)| *a == abbr).map(|(_, n)| *n)
}

/// Parse TSK scripRef contents such as "16,36; 1:12; Isa 45:22; Mr 16:16".
pub fn parse_tsk(raw: &str, here: Position) -> Vec<(Position, String)> {
    let mut out = Vec::new();
    let mut rest = raw;
    while let Some(s) = rest.find("<scripRef") {
        let Some(open_end) = rest[s..].find('>') else {
            break;
        };
        let body_start = s + open_end + 1;
        let Some(close) = rest[body_start..].find("</scripRef>") else {
            break;
        };
        let body = &rest[body_start..body_start + close];
        rest = &rest[body_start + close..];
        let mut book = here.book;
        let mut chapter = here.chapter;
        for token in body.split(';') {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }
            let mut parts = token.rsplitn(2, ' ');
            let nums = parts.next().unwrap_or("");
            if let Some(b) = parts.next()
                && let Some(nr) = tsk_book(b.trim())
            {
                book = nr;
                chapter = 0;
            }
            for piece in nums.split(',') {
                let piece = piece.trim();
                let (c, v) = match piece.split_once(':') {
                    Some((c, v)) => (c.parse::<u32>().ok(), v),
                    None => (None, piece),
                };
                if let Some(c) = c {
                    chapter = c;
                }
                if chapter == 0 {
                    continue;
                }
                let v: u32 = v
                    .split('-')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .parse()
                    .unwrap_or(0);
                if v == 0 {
                    continue;
                }
                let p = Position {
                    book,
                    chapter,
                    verse: v,
                };
                if p != here && !out.iter().any(|(q, _)| *q == p) {
                    out.push((p, String::new()));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_gui_launch_waits_for_a_choice_and_preserves_reference() {
        let dir = crate::storage::TestDir::new();
        let mut app = App::new();
        app.study = Study::load_from(dir.0.join("study.json"));
        app.start_gui(None, Some("John 3:16".into()));
        assert!(app.onboarding);
        assert!(app.load_pending.iter().all(Option::is_none));
        assert!(app.study.translation.is_none());
        assert_eq!(app.pending_reference.as_deref(), Some("John 3:16"));
        assert!(!app.finish_onboarding());
    }

    #[test]
    fn existing_users_skip_setup_but_incomplete_setup_resumes() {
        let dir = crate::storage::TestDir::new();
        let path = dir.0.join("study.json");
        std::fs::write(&path, r#"{"translation":"net"}"#).unwrap();
        let mut app = App::new();
        app.study = Study::load_from(path.clone());
        app.catalog = providers::catalog();
        app.start_gui(None, None);
        assert!(!app.onboarding);
        assert!(app.load_pending[0].is_some());

        let mut resumed = App::new();
        std::fs::write(&path, r#"{"translation":"net","onboarding_complete":false}"#).unwrap();
        resumed.study = Study::load_from(path);
        resumed.start_gui(None, None);
        assert!(resumed.onboarding);
        assert!(resumed.load_pending[0].is_none());
    }

    #[test]
    fn onboarding_requires_loaded_text_and_a_successful_save() {
        let dir = crate::storage::TestDir::new();
        let path = dir.0.join("study.json");
        let mut app = App::new();
        app.study = Study::load_from(path.clone());
        app.start_gui(None, None);
        app.translation = Some(providers::open("net").unwrap());
        assert!(!app.finish_onboarding());
        let online = app.translation.as_mut().unwrap().online.as_mut().unwrap();
        online.loaded = Some((0, 0));
        online.error = Some("Connection failed".into());
        assert!(!app.finish_onboarding());
        app.translation.as_mut().unwrap().online = None;
        app.study.translation = Some("net".into());
        std::fs::write(&path, "external edits").unwrap();
        assert!(!app.finish_onboarding());
        assert!(app.onboarding);
        assert_eq!(app.study.onboarding_complete, Some(false));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "external edits");
        std::fs::remove_file(&path).unwrap();
        assert!(app.finish_onboarding());
        assert!(!app.onboarding);
        let saved = Study::load_from(path);
        assert_eq!(saved.onboarding_complete, Some(true));
        assert_eq!(saved.translation.as_deref(), Some("net"));
    }

    #[test]
    fn stale_load_results_and_canceled_parallel_loads_are_discarded() {
        let mut app = App::new();
        app.study = Study::default();
        let first = app.begin_load(Slot::Primary);
        let latest = app.begin_load(Slot::Primary);
        let canceled = app.begin_load(Slot::Parallel);
        app.clear_parallel();
        let offline = |id: &str| {
            let mut t = providers::open("net").unwrap();
            t.abbreviation = id.into();
            t.online = None;
            t
        };
        for (generation, abbr, slot) in [
            (latest, "latest", Slot::Primary),
            (first, "old", Slot::Primary),
            (canceled, "canceled", Slot::Parallel),
        ] {
            app.tx
                .send(Msg::Loaded {
                    generation,
                    abbr: abbr.into(),
                    slot,
                    result: Ok(offline(abbr)),
                })
                .unwrap();
        }
        app.poll_messages();
        assert_eq!(app.translation.as_ref().unwrap().abbreviation, "latest");
        assert!(app.parallel.is_none());
        assert!(app.busy.is_none());
    }

    #[test]
    fn worker_panics_become_reportable_failures() {
        let result = worker::<()>(|| panic!("simulated worker failure"));
        assert!(result.unwrap_err().contains("Please try again"));
    }

    #[test]
    fn draft_restores_and_failed_note_save_keeps_editor_open() {
        let dir = crate::storage::TestDir::new();
        let path = dir.0.join("study.json");
        let mut app = online_app();
        app.translation.as_mut().unwrap().online = None;
        app.study = Study::load_from(path.clone());
        app.mode = Mode::Note;
        app.note_buffer = "unfinished thought".into();
        assert!(app.save());
        let mut restored = App::new();
        restored.study = Study::load_from(path.clone());
        let t = app.translation.take().unwrap();
        restored.install_translation(t, Slot::Primary);
        assert_eq!(restored.mode, Mode::Note);
        assert_eq!(restored.note_buffer, "unfinished thought");
        // Simulate an external write while this window is editing.
        fs_write_external(&path);
        restored.key_note(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert_eq!(restored.mode, Mode::Note);
        assert!(restored.save_error.is_some());
        assert_eq!(restored.note_buffer, "unfinished thought");
        fn fs_write_external(path: &std::path::Path) {
            std::fs::write(path, b"{}").unwrap();
        }
    }

    #[test]
    fn exit_before_translation_load_preserves_last_position() {
        let dir = crate::storage::TestDir::new();
        let path = dir.0.join("study.json");
        let mut app = App::new();
        app.study = Study::load_from(path.clone());
        let position = Position {
            book: 43,
            chapter: 3,
            verse: 16,
        };
        app.study.last = Some(position);
        assert!(app.save());
        assert_eq!(Study::load_from(path).last, Some(position));
    }

    fn online_app() -> App {
        let mut app = App::new();
        app.study = Study::default();
        app.sidebar = 0;
        app.translation = Some(providers::open("net").unwrap());
        app.loc = Loc {
            book: 42,
            chapter: 2,
            verse: 15,
        };
        app
    }
    fn passage() -> Result<providers::Passage, String> {
        Ok(providers::Passage {
            verses: vec![
                crate::bible::Verse {
                    verse: 16,
                    text: "Test passage".into(),
                },
                crate::bible::Verse {
                    verse: 18,
                    text: "Another verse".into(),
                },
            ],
            notice: "Test attribution".into(),
        })
    }
    #[test]
    fn online_response_preserves_selected_verse_and_ignores_stale_results() {
        let mut app = online_app();
        app.accept_chapter("net", Slot::Primary, 42, 1, passage());
        assert!(
            app.translation
                .as_ref()
                .unwrap()
                .online
                .as_ref()
                .unwrap()
                .loaded
                .is_none()
        );
        app.accept_chapter("esv", Slot::Primary, 42, 2, passage());
        assert!(
            app.translation
                .as_ref()
                .unwrap()
                .online
                .as_ref()
                .unwrap()
                .loaded
                .is_none()
        );
        app.accept_chapter("net", Slot::Primary, 42, 2, passage());
        assert_eq!(app.loc.verse, 0);
        assert_eq!(
            app.translation
                .as_ref()
                .unwrap()
                .position_from_loc(app.loc)
                .verse,
            16
        );
        assert_eq!(
            app.translation.as_ref().unwrap().verse_text(app.loc),
            Some("Test passage")
        );
    }
    #[test]
    fn online_error_renders_and_retry_evicts_old_text() {
        let mut app = online_app();
        app.accept_chapter(
            "net",
            Slot::Primary,
            42,
            2,
            Err("Test connection failure".into()),
        );
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
        terminal.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        let screen: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(screen.contains("Test connection failure"));
        assert!(
            app.translation
                .as_ref()
                .unwrap()
                .verse_text(app.loc)
                .is_none()
        );
        app.accept_chapter("net", Slot::Primary, 42, 2, passage());
        app.handle_key(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE));
        let t = app.translation.as_ref().unwrap();
        assert!(t.online.as_ref().unwrap().loaded.is_none());
        assert_eq!(t.position_from_loc(app.loc).verse, 16);
        assert!(
            t.books[42].chapters[2]
                .verses
                .iter()
                .all(|v| v.text.is_empty())
        );
    }
    #[test]
    fn online_back_history_survives_chapter_eviction() {
        let mut app = online_app();
        app.accept_chapter("net", Slot::Primary, 42, 2, passage());
        app.push_history();
        app.translation.as_mut().unwrap().books[42].chapters[2] = providers::placeholder(42, 2);
        app.loc = Loc {
            book: 42,
            chapter: 3,
            verse: 0,
        };
        app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));
        assert_eq!(
            app.position(),
            Some(Position {
                book: 43,
                chapter: 3,
                verse: 16
            })
        );
    }

    #[test]
    fn online_parallel_response_does_not_move_primary_selection() {
        let mut app = online_app();
        app.parallel = Some(providers::open("nlt").unwrap());
        app.parallel_visible = true;
        let loc = app.loc;
        app.accept_chapter("nlt", Slot::Parallel, 42, 2, passage());
        assert_eq!(app.loc, loc);
        assert_eq!(
            app.parallel
                .as_ref()
                .unwrap()
                .online
                .as_ref()
                .unwrap()
                .loaded,
            Some((42, 2))
        );
        app.parallel_visible = false;
        app.accept_chapter("nlt", Slot::Parallel, 42, 2, Err("late error".into()));
        assert!(
            app.parallel
                .as_ref()
                .unwrap()
                .online
                .as_ref()
                .unwrap()
                .error
                .is_none()
        );
    }
    #[test]
    fn online_rendering_handles_small_and_wide_terminals() {
        let mut app = online_app();
        app.accept_chapter("net", Slot::Primary, 42, 2, passage());
        for (w, h) in [(20, 8), (80, 24), (150, 40)] {
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
            terminal.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
            if w >= 80 {
                let screen: String = terminal
                    .backend()
                    .buffer()
                    .content()
                    .iter()
                    .map(|c| c.symbol())
                    .collect();
                assert!(screen.contains("Test passage"));
                assert!(screen.contains("Test attribution"));
            }
        }
    }

    #[test]
    fn parses_tsk_refs() {
        let raw = "whosoever.<br /><scripRef>16,36; 1:12; Isa 45:22; Mr 16:16</scripRef><br /><scripRef>1Jo 5:1,11-13</scripRef>";
        let here = Position {
            book: 43,
            chapter: 3,
            verse: 16,
        };
        let refs = parse_tsk(raw, here);
        let keys: Vec<String> = refs.iter().map(|(p, _)| p.key()).collect();
        assert_eq!(
            keys,
            vec![
                "43:3:36", "43:1:12", "23:45:22", "41:16:16", "62:5:1", "62:5:11"
            ]
        );
    }
}
