//! Menu bar and footer button definitions. Every action here dispatches a
//! key event, so the keyboard and mouse paths share one implementation.

use crate::app::{App, Mode};
use crossterm::event::{KeyCode, KeyModifiers};

pub struct MenuItem {
    pub label: &'static str,
    pub hint: &'static str,
    pub code: KeyCode,
}

pub struct Menu {
    pub title: &'static str,
    pub items: &'static [MenuItem],
}

const fn item(label: &'static str, hint: &'static str, code: KeyCode) -> MenuItem {
    MenuItem { label, hint, code }
}

pub const MENUS: &[Menu] = &[
    Menu {
        title: "Navigate",
        items: &[
            item("Previous chapter", "h", KeyCode::Char('h')),
            item("Next chapter", "l", KeyCode::Char('l')),
            item("Previous book", "[", KeyCode::Char('[')),
            item("Next book", "]", KeyCode::Char(']')),
            item("Books…", "b", KeyCode::Char('b')),
            item("Chapters…", "c", KeyCode::Char('c')),
            item("Go to reference…", ":", KeyCode::Char(':')),
            item("Search…", "/", KeyCode::Char('/')),
            item("Next search result", "n", KeyCode::Char('n')),
            item("Previous search result", "N", KeyCode::Char('N')),
            item("Back", "u", KeyCode::Char('u')),
            item("Verse of the day", "v", KeyCode::Char('v')),
        ],
    },
    Menu {
        title: "View",
        items: &[
            item("Translation…", "t", KeyCode::Char('t')),
            item("Parallel translation…", "T", KeyCode::Char('T')),
            item("Show / hide parallel", "p", KeyCode::Char('p')),
            item("Show / hide sidebar", "s", KeyCode::Char('s')),
            item("Cross-references", "1", KeyCode::Char('1')),
            item("Interlinear", "2", KeyCode::Char('2')),
            item("Commentary", "3", KeyCode::Char('3')),
            item("Translation notes", "4", KeyCode::Char('4')),
            item("Next commentary", "C", KeyCode::Char('C')),
            item("Retry online chapter", "F5", KeyCode::F(5)),
        ],
    },
    Menu {
        title: "Study",
        items: &[
            item("Word study", "w", KeyCode::Char('w')),
            item("Dictionaries…", "D", KeyCode::Char('D')),
            item("Parallel passages", "P", KeyCode::Char('P')),
            item("Reading plan", "r", KeyCode::Char('r')),
            item("Bookmark verse", "m", KeyCode::Char('m')),
            item("Bookmarks…", "B", KeyCode::Char('B')),
            item("Highlight verse", "x", KeyCode::Char('x')),
            item("Note on verse…", "e", KeyCode::Char('e')),
            item("Copy verse", "y", KeyCode::Char('y')),
        ],
    },
    Menu {
        title: "Tools",
        items: &[
            item("Study resources…", "R", KeyCode::Char('R')),
            item("Settings…", ",", KeyCode::Char(',')),
            item("Keyboard help", "?", KeyCode::Char('?')),
            item("Quit", "q", KeyCode::Char('q')),
        ],
    },
];

#[derive(Clone, Copy)]
pub struct Button {
    pub label: &'static str,
    pub hint: &'static str,
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

const fn b(label: &'static str, hint: &'static str, code: KeyCode) -> Button {
    Button {
        label,
        hint,
        code,
        mods: KeyModifiers::NONE,
    }
}

const fn ctrl(label: &'static str, hint: &'static str, c: char) -> Button {
    Button {
        label,
        hint,
        code: KeyCode::Char(c),
        mods: KeyModifiers::CONTROL,
    }
}

const OPEN: Button = b("Open", "⏎", KeyCode::Enter);
const CLOSE: Button = b("Close", "Esc", KeyCode::Esc);
const BACK: Button = b("Back", "Esc", KeyCode::Esc);

/// The reading-mode toolbar. Its height sets the footer height in every
/// mode so the layout never jumps.
pub fn read_toolbar() -> Vec<Button> {
    vec![
        b("◂ Prev", "h", KeyCode::Char('h')),
        b("Next ▸", "l", KeyCode::Char('l')),
        b("Books", "b", KeyCode::Char('b')),
        b("Go to", ":", KeyCode::Char(':')),
        b("Search", "/", KeyCode::Char('/')),
        b("Translation", "t", KeyCode::Char('t')),
        b("Parallel", "p", KeyCode::Char('p')),
        b("Sidebar", "s", KeyCode::Char('s')),
        b("Refs", "1", KeyCode::Char('1')),
        b("Words", "2", KeyCode::Char('2')),
        b("Comm", "3", KeyCode::Char('3')),
        b("Notes", "4", KeyCode::Char('4')),
        b("Bookmark", "m", KeyCode::Char('m')),
        b("Highlight", "x", KeyCode::Char('x')),
        b("Note", "e", KeyCode::Char('e')),
        b("Dictionary", "D", KeyCode::Char('D')),
        b("Passages", "P", KeyCode::Char('P')),
        b("Plan", "r", KeyCode::Char('r')),
        b("Resources", "R", KeyCode::Char('R')),
        b("Settings", ",", KeyCode::Char(',')),
        b("Back", "u", KeyCode::Char('u')),
        b("Help", "?", KeyCode::Char('?')),
    ]
}

/// Buttons shown in the footer for the current mode.
pub fn footer(app: &App) -> Vec<Button> {
    match app.mode {
        Mode::Read if app.translation.is_none() => vec![
            b("Choose translation", "t", KeyCode::Char('t')),
            b("Resources", "R", KeyCode::Char('R')),
            b("Settings", ",", KeyCode::Char(',')),
            b("Help", "?", KeyCode::Char('?')),
            b("Quit", "q", KeyCode::Char('q')),
        ],
        Mode::Read if app.focus_side => vec![
            OPEN,
            b("Back to text", "Esc", KeyCode::Esc),
            b("Refs", "1", KeyCode::Char('1')),
            b("Words", "2", KeyCode::Char('2')),
            b("Comm", "3", KeyCode::Char('3')),
            b("Notes", "4", KeyCode::Char('4')),
            b("Next commentary", "C", KeyCode::Char('C')),
            b("Hide sidebar", "s", KeyCode::Char('s')),
        ],
        Mode::Read | Mode::Menu => {
            let mut buttons = read_toolbar();
            if app.translation.as_ref().is_some_and(|t| t.online.is_some())
                || app.parallel.as_ref().is_some_and(|t| t.online.is_some())
            {
                buttons.insert(0, b("Retry online", "F5", KeyCode::F(5)));
            }
            buttons
        }
        Mode::Books => vec![OPEN, CLOSE],
        Mode::Chapters => vec![OPEN, b("Books", "b", KeyCode::Char('b')), CLOSE],
        Mode::Translations => vec![
            b("Open / download", "⏎", KeyCode::Enter),
            b("Delete", "Del", KeyCode::Delete),
            b("Refresh list", "F5", KeyCode::F(5)),
            CLOSE,
        ],
        Mode::Search => vec![
            b("Search", "⏎", KeyCode::Enter),
            b("Cancel", "Esc", KeyCode::Esc),
        ],
        Mode::GoTo => vec![
            b("Go", "⏎", KeyCode::Enter),
            b("Cancel", "Esc", KeyCode::Esc),
        ],
        Mode::SearchResults => vec![OPEN, b("New search", "/", KeyCode::Char('/')), CLOSE],
        Mode::Note => vec![
            ctrl("Save", "^S", 's'),
            ctrl("Clear", "^U", 'u'),
            b("Cancel", "Esc", KeyCode::Esc),
        ],
        Mode::Bookmarks => vec![OPEN, b("Delete", "d", KeyCode::Char('d')), CLOSE],
        Mode::Help => vec![CLOSE],
        Mode::Resources => vec![
            b("Install", "⏎", KeyCode::Enter),
            b("Remove", "d", KeyCode::Char('d')),
            b("Install all", "a", KeyCode::Char('a')),
            CLOSE,
        ],
        Mode::Harmony => vec![b("Open passage", "⏎", KeyCode::Enter), CLOSE],
        Mode::Plans if app.study.plan.is_some() => vec![
            b("Read", "⏎", KeyCode::Enter),
            b("Mark done", "x", KeyCode::Char('x')),
            b("◂ Day", "h", KeyCode::Char('h')),
            b("Day ▸", "l", KeyCode::Char('l')),
            b("Today", "t", KeyCode::Char('t')),
            b("Stop plan", "X", KeyCode::Char('X')),
            CLOSE,
        ],
        Mode::Plans => vec![b("Start plan", "⏎", KeyCode::Enter), CLOSE],
        Mode::DictSearch => vec![OPEN, CLOSE],
        Mode::DictEntry => vec![BACK],
        Mode::WordStudy => vec![
            b("Occurrences", "o", KeyCode::Char('o')),
            b("Copy", "y", KeyCode::Char('y')),
            CLOSE,
        ],
        Mode::Occurrences => vec![b("Open verse", "⏎", KeyCode::Enter), BACK],
        Mode::Settings => vec![
            b("Change", "⏎", KeyCode::Enter),
            b("◂ Previous value", "h", KeyCode::Char('h')),
            b("Next value ▸", "l", KeyCode::Char('l')),
            b("Remove parallel", "x", KeyCode::Char('x')),
            ctrl("Reset defaults", "^R", 'r'),
            CLOSE,
        ],
    }
}
