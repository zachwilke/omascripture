//! Bible data model, local translation store, and getbible.net client.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

const API_BASE: &str = "https://api.getbible.net/v2";
const MAX_BODY: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationInfo {
    pub abbreviation: String,
    pub translation: String,
    #[serde(default)]
    pub lang: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub distribution_license: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Translation {
    #[serde(skip)]
    pub online: Option<crate::providers::Online>,
    pub translation: String,
    pub abbreviation: String,
    #[serde(default)]
    pub lang: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub distribution_license: String,
    pub books: Vec<Book>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Book {
    pub nr: u32,
    pub name: String,
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Chapter {
    pub chapter: u32,
    pub verses: Vec<Verse>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Verse {
    pub verse: u32,
    pub text: String,
}

/// A verse location expressed by book number (1..=66+), chapter number, verse number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Position {
    pub book: u32,
    pub chapter: u32,
    pub verse: u32,
}

impl Position {
    pub fn key(&self) -> String {
        format!("{}:{}:{}", self.book, self.chapter, self.verse)
    }
}

/// Indices into a Translation's books/chapters/verses vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loc {
    pub book: usize,
    pub chapter: usize,
    pub verse: usize,
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub loc: Loc,
    pub reference: String,
    pub text: String,
}

impl Translation {
    /// Validate the invariants used by navigation before data reaches the UI.
    pub fn validate(&self, expected_id: &str) -> Result<(), String> {
        if !valid_id(expected_id) || !self.abbreviation.eq_ignore_ascii_case(expected_id) {
            return Err("Translation ID does not match the requested Bible".into());
        }
        if self.books.is_empty() {
            return Err("Translation contains no books".into());
        }
        let mut books = std::collections::HashSet::new();
        for book in &self.books {
            if book.nr == 0 || !books.insert(book.nr) || book.chapters.is_empty() {
                return Err(format!("Invalid or empty book: {}", book.name));
            }
            let mut last_chapter = 0;
            for chapter in &book.chapters {
                if chapter.chapter <= last_chapter || chapter.verses.is_empty() {
                    return Err(format!("Invalid or empty chapter in {}", book.name));
                }
                last_chapter = chapter.chapter;
                let mut last_verse = 0;
                for verse in &chapter.verses {
                    if verse.verse <= last_verse {
                        return Err(format!(
                            "Invalid verse numbering in {} {}",
                            book.name, chapter.chapter
                        ));
                    }
                    last_verse = verse.verse;
                }
            }
        }
        Ok(())
    }

    pub fn info(&self) -> TranslationInfo {
        TranslationInfo {
            abbreviation: self.abbreviation.clone(),
            translation: self.translation.clone(),
            lang: self.lang.clone(),
            language: self.language.clone(),
            direction: String::new(),
            description: self.description.clone(),
            distribution_license: self.distribution_license.clone(),
        }
    }

    pub fn book_by_nr(&self, nr: u32) -> Option<usize> {
        self.books.iter().position(|b| b.nr == nr)
    }

    pub fn loc_from_position(&self, p: Position) -> Option<Loc> {
        let book = self.book_by_nr(p.book)?;
        let b = &self.books[book];
        let chapter = b.chapters.iter().position(|c| c.chapter == p.chapter)?;
        let c = &b.chapters[chapter];
        let verse = c
            .verses
            .iter()
            .position(|v| v.verse == p.verse)
            .unwrap_or(0);
        Some(Loc {
            book,
            chapter,
            verse,
        })
    }

    pub fn position_from_loc(&self, l: Loc) -> Position {
        let b = &self.books[l.book];
        let c = &b.chapters[l.chapter];
        let v = c.verses.get(l.verse).map(|v| v.verse).unwrap_or(1);
        Position {
            book: b.nr,
            chapter: c.chapter,
            verse: v,
        }
    }

    pub fn reference(&self, l: Loc) -> String {
        let b = &self.books[l.book];
        let c = &b.chapters[l.chapter];
        match c.verses.get(l.verse) {
            Some(v) => format!("{} {}:{}", b.name, c.chapter, v.verse),
            None => format!("{} {}", b.name, c.chapter),
        }
    }

    pub fn verse_text(&self, l: Loc) -> Option<&str> {
        if self
            .online
            .as_ref()
            .is_some_and(|o| o.loaded != Some((l.book, l.chapter)) || o.error.is_some())
        {
            return None;
        }
        self.books
            .get(l.book)?
            .chapters
            .get(l.chapter)?
            .verses
            .get(l.verse)
            .map(|v| v.text.as_str())
    }

    /// Case-insensitive search. Every whitespace-separated word must appear;
    /// a query wrapped in double quotes matches as an exact phrase.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let q = query.trim();
        if q.is_empty() {
            return Vec::new();
        }
        let phrase = q.len() >= 2 && q.starts_with('"') && q.ends_with('"');
        let terms: Vec<String> = if phrase {
            vec![q[1..q.len() - 1].to_lowercase()]
        } else {
            q.split_whitespace().map(|w| w.to_lowercase()).collect()
        };
        let mut hits = Vec::new();
        for (bi, b) in self.books.iter().enumerate() {
            for (ci, c) in b.chapters.iter().enumerate() {
                for (vi, v) in c.verses.iter().enumerate() {
                    let lower = v.text.to_lowercase();
                    if terms.iter().all(|t| lower.contains(t.as_str())) {
                        hits.push(SearchHit {
                            loc: Loc {
                                book: bi,
                                chapter: ci,
                                verse: vi,
                            },
                            reference: format!("{} {}:{}", b.name, c.chapter, v.verse),
                            text: v.text.clone(),
                        });
                        if hits.len() >= limit {
                            return hits;
                        }
                    }
                }
            }
        }
        hits
    }

    /// Parse a human reference like "John 3:16", "1 Jn 2", "Ps 23", "rev".
    pub fn parse_reference(&self, input: &str) -> Option<Loc> {
        let tokens: Vec<&str> = input.split_whitespace().collect();
        if tokens.is_empty() {
            return None;
        }
        let is_chapter_token = |t: &str| {
            t.chars()
                .all(|c| c.is_ascii_digit() || c == ':' || c == '.' || c == '-')
        };
        let mut book_tokens: Vec<&str> = Vec::new();
        let mut rest: Vec<&str> = Vec::new();
        for (i, t) in tokens.iter().enumerate() {
            let roman_prefix =
                i == 0 && matches!(t.to_ascii_uppercase().as_str(), "I" | "II" | "III");
            let numeric_prefix = i == 0 && t.len() == 1 && t.chars().all(|c| c.is_ascii_digit());
            if rest.is_empty() && (roman_prefix || numeric_prefix || !is_chapter_token(t)) {
                book_tokens.push(t);
            } else {
                rest.push(t);
            }
        }
        if book_tokens.is_empty() {
            return None;
        }
        let book = self.find_book(&book_tokens.join(" "))?;
        let b = &self.books[book];
        let nums: Vec<u32> = rest
            .join(" ")
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        let chapter_nr = nums.first().copied().unwrap_or(1);
        let chapter = b
            .chapters
            .iter()
            .position(|c| c.chapter == chapter_nr)
            .unwrap_or(0);
        let c = &b.chapters[chapter];
        let verse = nums
            .get(1)
            .and_then(|vn| c.verses.iter().position(|v| v.verse == *vn))
            .unwrap_or(0);
        Some(Loc {
            book,
            chapter,
            verse,
        })
    }

    pub fn find_book(&self, name: &str) -> Option<usize> {
        let norm = normalize(&roman_prefix(name));
        if norm.is_empty() {
            return None;
        }
        let aliased = alias(&norm).unwrap_or(norm.as_str()).to_string();
        let names: Vec<String> = self.books.iter().map(|b| normalize(&b.name)).collect();
        if let Some(i) = names.iter().position(|n| *n == aliased) {
            return Some(i);
        }
        if let Some(i) = names.iter().position(|n| n.starts_with(&aliased)) {
            return Some(i);
        }
        names.iter().position(|n| n.contains(&aliased))
    }
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Turn a leading roman numeral ("I John", "ii kings") into a digit.
fn roman_prefix(name: &str) -> String {
    let trimmed = name.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let first = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("");
    let digit = match first.to_ascii_uppercase().as_str() {
        "I" => "1",
        "II" => "2",
        "III" => "3",
        _ => return trimmed.to_string(),
    };
    if rest.is_empty() {
        return trimmed.to_string();
    }
    format!("{digit} {rest}")
}

fn alias(norm: &str) -> Option<&'static str> {
    Some(match norm {
        "gn" | "ge" => "genesis",
        "ex" | "exo" => "exodus",
        "lv" => "leviticus",
        "nm" | "nu" => "numbers",
        "dt" | "deu" => "deuteronomy",
        "jos" | "jsh" => "joshua",
        "jdg" | "jgs" => "judges",
        "ru" | "rth" => "ruth",
        "1sa" | "1sm" => "1samuel",
        "2sa" | "2sm" => "2samuel",
        "1kg" | "1ki" => "1kings",
        "2kg" | "2ki" => "2kings",
        "1ch" => "1chronicles",
        "2ch" => "2chronicles",
        "ezr" => "ezra",
        "ne" | "neh" => "nehemiah",
        "est" => "esther",
        "jb" => "job",
        "ps" | "psa" | "psalm" | "pss" => "psalms",
        "pr" | "prv" | "pro" => "proverbs",
        "ec" | "ecc" | "qoh" => "ecclesiastes",
        "sos" | "song" | "sng" | "songofsongs" | "canticles" => "songofsolomon",
        "is" | "isa" => "isaiah",
        "je" | "jer" => "jeremiah",
        "la" | "lam" => "lamentations",
        "eze" | "ezk" => "ezekiel",
        "da" | "dn" => "daniel",
        "ho" | "hos" => "hosea",
        "jl" => "joel",
        "am" => "amos",
        "ob" | "oba" => "obadiah",
        "jon" | "jnh" => "jonah",
        "mi" | "mic" => "micah",
        "na" | "nah" => "nahum",
        "hab" => "habakkuk",
        "zep" | "zph" => "zephaniah",
        "hg" | "hag" => "haggai",
        "zec" | "zch" => "zechariah",
        "mal" => "malachi",
        "mt" | "mat" | "matt" => "matthew",
        "mk" | "mr" | "mrk" => "mark",
        "lk" | "luk" => "luke",
        "jn" | "jhn" | "joh" => "john",
        "ac" | "act" => "acts",
        "ro" | "rm" | "rom" => "romans",
        "1co" | "1cor" => "1corinthians",
        "2co" | "2cor" => "2corinthians",
        "ga" | "gal" => "galatians",
        "eph" => "ephesians",
        "php" | "phil" | "phili" => "philippians",
        "col" => "colossians",
        "1th" | "1thes" | "1thess" => "1thessalonians",
        "2th" | "2thes" | "2thess" => "2thessalonians",
        "1ti" | "1tim" => "1timothy",
        "2ti" | "2tim" => "2timothy",
        "ti" | "tit" => "titus",
        "phm" | "phlm" | "philem" => "philemon",
        "heb" => "hebrews",
        "ja" | "jas" | "jam" => "james",
        "1pe" | "1pt" | "1pet" => "1peter",
        "2pe" | "2pt" | "2pet" => "2peter",
        "1jn" | "1jo" | "1jhn" => "1john",
        "2jn" | "2jo" | "2jhn" => "2john",
        "3jn" | "3jo" | "3jhn" => "3john",
        "jud" | "jde" => "jude",
        "re" | "rev" | "rv" | "apocalypse" => "revelation",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Local store
// ---------------------------------------------------------------------------

pub fn data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("OMASCRIPTURE_DATA") {
        return PathBuf::from(p);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("omascripture")
}

pub fn translations_dir() -> PathBuf {
    data_dir().join("translations")
}

fn translation_path(abbr: &str) -> PathBuf {
    translations_dir().join(format!("{abbr}.json"))
}

fn meta_path(abbr: &str) -> PathBuf {
    translations_dir().join(format!("{abbr}.meta.json"))
}

fn catalog_path() -> PathBuf {
    data_dir().join("catalog.json")
}

fn valid_id(abbr: &str) -> bool {
    !abbr.is_empty()
        && abbr
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn checked_id(abbr: &str) -> io::Result<()> {
    if valid_id(abbr) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid translation ID",
        ))
    }
}

pub fn is_installed(abbr: &str) -> bool {
    valid_id(abbr) && translation_path(abbr).exists()
}

/// Translations available offline.
pub fn installed() -> Vec<TranslationInfo> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(translations_dir()) else {
        return out;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.ends_with(".meta.json") {
            continue;
        }
        let abbr = name.trim_end_matches(".meta.json");
        if !translation_path(abbr).exists() {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path)
            && let Ok(info) = serde_json::from_str::<TranslationInfo>(&text)
        {
            out.push(info);
        }
    }
    out.sort_by(|a, b| a.abbreviation.cmp(&b.abbreviation));
    out
}

pub fn load(abbr: &str) -> io::Result<Translation> {
    checked_id(abbr)?;
    let text = fs::read_to_string(translation_path(abbr))?;
    let mut t: Translation = serde_json::from_str(&text)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    t.validate(abbr)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    t.abbreviation = abbr.to_owned();
    for b in &mut t.books {
        for c in &mut b.chapters {
            for v in &mut c.verses {
                let cleaned = v.text.split_whitespace().collect::<Vec<_>>().join(" ");
                v.text = cleaned;
            }
        }
    }
    // Make sure the meta file exists (e.g. for manually copied translations).
    if !meta_path(abbr).exists() {
        let _ = write_meta(&t.info());
    }
    Ok(t)
}

pub fn remove(abbr: &str) -> io::Result<()> {
    checked_id(abbr)?;
    let _ = fs::remove_file(meta_path(abbr));
    fs::remove_file(translation_path(abbr))
}

fn write_meta(info: &TranslationInfo) -> io::Result<()> {
    fs::create_dir_all(translations_dir())?;
    let json = serde_json::to_string_pretty(info)?;
    checked_id(&info.abbreviation)?;
    crate::storage::atomic_write(&meta_path(&info.abbreviation), json.as_bytes())
}

pub fn cached_catalog() -> Vec<TranslationInfo> {
    fs::read_to_string(catalog_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Network
// ---------------------------------------------------------------------------

fn agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(180)))
        .user_agent("omascripture/0.1 (+https://github.com/pinefall/omascripture)")
        .build();
    ureq::Agent::new_with_config(config)
}

fn get_string(url: &str) -> Result<String, String> {
    let mut resp = agent().get(url).call().map_err(|e| e.to_string())?;
    resp.body_mut()
        .with_config()
        .limit(MAX_BODY)
        .read_to_string()
        .map_err(|e| e.to_string())
}

/// Fetch the list of all translations from getbible.net and cache it.
pub fn fetch_catalog() -> Result<Vec<TranslationInfo>, String> {
    let text = get_string(&format!("{API_BASE}/translations.json"))?;
    let map: HashMap<String, TranslationInfo> =
        serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut list: Vec<TranslationInfo> = map
        .into_values()
        .filter(|info| valid_id(&info.abbreviation))
        .collect();
    list.sort_by(|a, b| {
        (
            a.language.is_empty(),
            a.language.as_str(),
            a.translation.as_str(),
        )
            .cmp(&(
                b.language.is_empty(),
                b.language.as_str(),
                b.translation.as_str(),
            ))
    });
    let _ = fs::create_dir_all(data_dir());
    if let Ok(json) = serde_json::to_string(&list) {
        let _ = crate::storage::atomic_write(&catalog_path(), json.as_bytes());
    }
    Ok(list)
}

/// Download a whole translation into the local store and return it parsed.
pub fn download(abbr: &str) -> Result<Translation, String> {
    if crate::providers::is_online(abbr) {
        return Err("Online translations cannot be downloaded; open with -t instead".into());
    }
    let abbr = abbr.trim().to_lowercase();
    if abbr.is_empty()
        || !abbr
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!("invalid translation id '{abbr}'"));
    }
    let text = get_string(&format!("{API_BASE}/{abbr}.json"))?;
    let t: Translation = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    t.validate(&abbr)?;
    fs::create_dir_all(translations_dir()).map_err(|e| e.to_string())?;
    crate::storage::atomic_write(&translation_path(&abbr), text.as_bytes())
        .map_err(|e| e.to_string())?;
    write_meta(&t.info()).map_err(|e| e.to_string())?;
    load(&abbr).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Translation {
        let mk = |nr: u32, name: &str, chapters: u32, verses: u32| Book {
            nr,
            name: name.to_string(),
            chapters: (1..=chapters)
                .map(|c| Chapter {
                    chapter: c,
                    verses: (1..=verses)
                        .map(|v| Verse {
                            verse: v,
                            text: format!("{name} {c}:{v} text"),
                        })
                        .collect(),
                })
                .collect(),
        };
        Translation {
            online: None,
            translation: "Test".into(),
            abbreviation: "test".into(),
            lang: "en".into(),
            language: "English".into(),
            description: String::new(),
            distribution_license: String::new(),
            books: vec![
                mk(1, "Genesis", 3, 5),
                mk(7, "Judges", 2, 5),
                mk(19, "Psalms", 3, 5),
                mk(23, "Isaiah", 2, 5),
                mk(43, "John", 4, 20),
                mk(50, "Philippians", 2, 5),
                mk(57, "Philemon", 1, 5),
                mk(62, "1 John", 2, 5),
                mk(65, "Jude", 1, 5),
                mk(66, "Revelation", 2, 5),
            ],
        }
    }

    fn r(t: &Translation, s: &str) -> Option<(u32, u32, u32)> {
        t.parse_reference(s).map(|l| {
            let p = t.position_from_loc(l);
            (p.book, p.chapter, p.verse)
        })
    }

    #[test]
    fn malformed_translation_structures_are_rejected_before_navigation() {
        let good = sample();
        good.validate(&good.abbreviation).unwrap();
        for case in 0..6 {
            let mut bad = good.clone();
            match case {
                0 => bad.books.clear(),
                1 => bad.books[0].chapters.clear(),
                2 => bad.books[0].chapters[0].verses.clear(),
                3 => bad.books[1].nr = bad.books[0].nr,
                4 => bad.books[0].chapters[0].verses[1].verse = 1,
                _ => bad.abbreviation = "../outside".into(),
            }
            assert!(bad.validate(&good.abbreviation).is_err());
        }
        assert!(load("../outside").is_err());
        assert!(remove("../outside").is_err());
    }

    #[test]
    fn parses_references() {
        let t = sample();
        assert_eq!(r(&t, "John 3:16"), Some((43, 3, 16)));
        assert_eq!(r(&t, "john 3"), Some((43, 3, 1)));
        assert_eq!(r(&t, "jn 3.16"), Some((43, 3, 16)));
        assert_eq!(r(&t, "1 John 2:3"), Some((62, 2, 3)));
        assert_eq!(r(&t, "1Jn 2"), Some((62, 2, 1)));
        assert_eq!(r(&t, "I John 2"), Some((62, 2, 1)));
        assert_eq!(r(&t, "Ps 3"), Some((19, 3, 1)));
        assert_eq!(r(&t, "rev"), Some((66, 1, 1)));
        assert_eq!(r(&t, "Isa 2"), Some((23, 2, 1)));
        assert_eq!(r(&t, "Jude 1:3"), Some((65, 1, 3)));
        assert_eq!(r(&t, "Judg 2"), Some((7, 2, 1)));
        assert_eq!(r(&t, "Phil 2"), Some((50, 2, 1)));
        assert_eq!(r(&t, "Phlm 1:2"), Some((57, 1, 2)));
        assert_eq!(r(&t, "gen 99:99"), Some((1, 1, 1)));
        assert_eq!(r(&t, "nonsense 3"), None);
        assert_eq!(r(&t, ""), None);
    }

    #[test]
    fn searches() {
        let t = sample();
        assert_eq!(t.search("john 3:16", 10).len(), 1);
        assert_eq!(t.search("JOHN 3:1", 100).len(), 11);
        assert_eq!(t.search("\"John 3:1 \"", 100).len(), 1);
        assert!(t.search("", 10).is_empty());
    }
}
