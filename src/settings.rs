//! Settings page: preferences that persist in study.json.

use crate::app::{App, Mode, Slot, TAB_NAMES};
use crate::resources::{self, Store};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Translation,
    Parallel,
    ParallelPane,
    SidebarStart,
    SidebarTab,
    Commentary,
    Start,
    TextWidth,
    Toolbar,
    Providers,
}

pub const ROWS: [Row; 10] = [
    Row::Translation,
    Row::Parallel,
    Row::ParallelPane,
    Row::SidebarStart,
    Row::SidebarTab,
    Row::Commentary,
    Row::Start,
    Row::TextWidth,
    Row::Toolbar,
    Row::Providers,
];

impl Row {
    pub fn label(self) -> &'static str {
        match self {
            Row::Translation => "Reading translation",
            Row::Parallel => "Parallel translation",
            Row::ParallelPane => "Parallel pane",
            Row::SidebarStart => "Sidebar at start",
            Row::SidebarTab => "Sidebar tab",
            Row::Commentary => "Commentary",
            Row::Start => "Open at start",
            Row::TextWidth => "Text width",
            Row::Toolbar => "Toolbar",
            Row::Providers => "Online providers",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            Row::Translation => {
                "The Bible you read by default. Enter opens the picker. Offline editions download once; online editions load chapters from their provider."
            }
            Row::Parallel => {
                "A second translation shown beside the first. Enter opens the picker, x removes it."
            }
            Row::ParallelPane => {
                "Whether the parallel translation is visible. Press p while reading to toggle it at any time."
            }
            Row::SidebarStart => {
                "Open the study sidebar when the app starts, keep it hidden, or restore whatever you had last time."
            }
            Row::SidebarTab => {
                "Which sidebar tab opens first: cross-references, interlinear, commentary or translation notes."
            }
            Row::Commentary => {
                "The commentary shown in the sidebar. Install more under Resources; C cycles through them while reading."
            }
            Row::Start => {
                "Where the app opens: where you left off, today's verse of the day, or the first chapter of today's reading plan."
            }
            Row::TextWidth => "Limit the reading column on wide screens. Full uses the whole pane.",
            Row::Providers => {
                "NET and NLT work without keys. Configure ESV and API.Bible in providers.json (see README). Keys are reloaded when opening a chapter. Enter shows the config path."
            }
            Row::Toolbar => {
                "The button bar at the bottom. Hide it for more text; the menu bar and keys still work."
            }
        }
    }
}

const WIDTHS: [u16; 9] = [0, 50, 60, 70, 80, 90, 100, 110, 120];

impl App {
    pub fn open_settings(&mut self) {
        self.mode = Mode::Settings;
        self.list_index = 0;
    }

    fn translation_name(&self, abbr: &str) -> String {
        let name = self
            .installed
            .iter()
            .chain(self.catalog.iter())
            .find(|i| i.abbreviation == abbr)
            .map(|i| i.translation.clone())
            .unwrap_or_default();
        if name.is_empty() {
            abbr.to_uppercase()
        } else {
            format!("{} · {name}", abbr.to_uppercase())
        }
    }

    pub fn setting_value(&self, row: Row) -> String {
        let s = &self.study;
        match row {
            Row::Translation => match self
                .translation
                .as_ref()
                .map(|t| t.abbreviation.clone())
                .or(s.translation.clone())
            {
                Some(a) => self.translation_name(&a),
                None => "none".into(),
            },
            Row::Parallel => match self
                .parallel
                .as_ref()
                .map(|t| t.abbreviation.clone())
                .or(s.parallel.clone())
            {
                Some(a) => self.translation_name(&a),
                None => "none".into(),
            },
            Row::ParallelPane => (if self.parallel_visible {
                "Shown"
            } else {
                "Hidden"
            })
            .into(),
            Row::SidebarStart => match s.sidebar_start.as_str() {
                "hidden" => "Hidden",
                "shown" => "Shown",
                _ => "As you left it",
            }
            .into(),
            Row::SidebarTab => TAB_NAMES[s.sidebar.clamp(1, 4) as usize].into(),
            Row::Commentary => match self.commentary_id() {
                Some(id) => resources::pack(&id)
                    .map(|p| p.name)
                    .unwrap_or("Commentary")
                    .into(),
                None => "none installed".into(),
            },
            Row::Start => match s.start.as_str() {
                "votd" => "Verse of the day",
                "plan" => "Today's plan reading",
                _ => "Last position",
            }
            .into(),
            Row::TextWidth => {
                if s.text_width == 0 {
                    "Full".into()
                } else {
                    format!("{} columns", s.text_width)
                }
            }
            Row::Providers => match crate::providers::config() {
                Ok(c) => format!(
                    "NET · NLT · ESV {} · API.Bible {}",
                    if c.esv_key.is_empty() {
                        "needs key"
                    } else {
                        "key set"
                    },
                    if c.api_bible_key.is_empty() {
                        "needs key"
                    } else {
                        "key set"
                    }
                ),
                Err(_) => "Cannot read provider settings".into(),
            },
            Row::Toolbar => (if s.hide_toolbar { "Hidden" } else { "Shown" }).into(),
        }
    }

    /// Move a setting forward (+1) or backward (-1).
    pub fn adjust_setting(&mut self, row: Row, dir: i8) {
        fn cycle<T: PartialEq + Copy>(opts: &[T], cur: T, dir: i8) -> T {
            let n = opts.len() as i32;
            let i = opts.iter().position(|o| *o == cur).unwrap_or(0) as i32;
            opts[((i + dir as i32).rem_euclid(n)) as usize]
        }
        match row {
            Row::Translation => {
                self.open_translations(Slot::Primary);
                self.return_to = Some(Mode::Settings);
                return;
            }
            Row::Parallel => {
                self.open_translations(Slot::Parallel);
                self.return_to = Some(Mode::Settings);
                return;
            }
            Row::ParallelPane => {
                if self.parallel.is_none() {
                    self.set_status("Choose a parallel translation first".into());
                } else {
                    self.parallel_visible = !self.parallel_visible;
                    self.study.parallel_visible = self.parallel_visible;
                }
            }
            Row::SidebarStart => {
                let cur = self.study.sidebar_start.as_str();
                self.study.sidebar_start = cycle(&["", "hidden", "shown"], cur, dir).to_string();
            }
            Row::SidebarTab => {
                let next = cycle(&[1u8, 2, 3, 4], self.study.sidebar.clamp(1, 4), dir);
                if self.sidebar != 0 {
                    self.show_tab(next);
                } else {
                    self.study.sidebar = next;
                }
            }
            Row::Commentary => {
                let ids = Store::commentary_ids();
                if ids.is_empty() {
                    self.set_status("No commentaries installed. Add some under Resources.".into());
                } else {
                    let cur = self.commentary_id().unwrap_or_default();
                    let next = cycle(&ids, cur.as_str(), dir);
                    self.study.commentary = Some(next.to_string());
                    self.side_cache = None;
                }
            }
            Row::Start => {
                let cur = self.study.start.as_str();
                self.study.start = cycle(&["", "votd", "plan"], cur, dir).to_string();
            }
            Row::TextWidth => self.study.text_width = cycle(&WIDTHS, self.study.text_width, dir),
            Row::Providers => self.set_status(format!(
                "Provider config: {}",
                crate::providers::config_path().display()
            )),
            Row::Toolbar => self.study.hide_toolbar = !self.study.hide_toolbar,
        }
        self.save();
    }

    pub fn clear_parallel(&mut self) {
        self.cancel_parallel_load();
        self.parallel = None;
        self.parallel_visible = false;
        self.study.parallel = None;
        self.study.parallel_visible = false;
        self.save();
    }

    pub fn reset_settings(&mut self) {
        self.study.sidebar_start.clear();
        self.study.start.clear();
        self.study.text_width = 0;
        self.study.hide_toolbar = false;
        self.study.commentary = None;
        self.study.sidebar = 1;
        self.side_cache = None;
        self.save();
        self.set_status("Settings reset (translations kept)".into());
    }

    pub fn key_settings(&mut self, key: KeyEvent) {
        let n = ROWS.len();
        let row = ROWS[self.list_index.min(n - 1)];
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char(',') => self.mode = Mode::Read,
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => {
                self.list_index = (self.list_index + 1) % n
            }
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => {
                self.list_index = (self.list_index + n - 1) % n
            }
            KeyCode::Char('g') | KeyCode::Home => self.list_index = 0,
            KeyCode::Char('G') | KeyCode::End => self.list_index = n - 1,
            KeyCode::Char('h') | KeyCode::Left => self.adjust_setting(row, -1),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => {
                self.adjust_setting(row, 1)
            }
            KeyCode::Char('x') | KeyCode::Delete if row == Row::Parallel => self.clear_parallel(),
            KeyCode::Char('r') if ctrl => self.reset_settings(),
            _ => {}
        }
    }
}
