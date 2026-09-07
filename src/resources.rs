//! Study resource packs: registry, download/extract, and loaders.
//!
//! Each pack lives under `<data>/resources/<id>/` with an `installed.json`
//! marker. All sources are public domain or Creative Commons.

use crate::bible::{self, Position};
use crate::sword::{Commentary, Dictionary};
use crate::v11n;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::time::Duration;

const SWORD_BASE: &str = "https://www.crosswire.org/ftpmirror/pub/sword/packages/rawzip";
const STEP_BASE: &str = "https://raw.githubusercontent.com/STEPBible/STEPBible-Data/master";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    WordData,
    CrossRefs,
    InterlinearNT,
    InterlinearOT,
    LexiconGreek,
    LexiconHebrew,
    Commentary,
    Dictionary,
    UwNotes,
    UwWords,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::WordData => "Word studies",
            Kind::CrossRefs => "Cross-references",
            Kind::InterlinearNT | Kind::InterlinearOT => "Interlinear",
            Kind::LexiconGreek | Kind::LexiconHebrew => "Lexicon",
            Kind::Commentary => "Commentary",
            Kind::Dictionary => "Dictionary",
            Kind::UwNotes => "Study notes",
            Kind::UwWords => "Dictionary",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Pack {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub license: &'static str,
    pub size: &'static str,
    pub kind: Kind,
    pub urls: &'static [&'static str],
}

pub const PACKS: &[Pack] = &[
    Pack { id: "word-study", name: "Word study language data", description: "Tagged KJV phrase alignment and expanded Greek/Hebrew morphology", license: "CrossWire permission for any purpose; STEPBible CC BY 4.0", size: "29 MB", kind: Kind::WordData, urls: &[
        "https://gitlab.com/crosswire-bible-society/kjv/-/raw/master/kjv.osis.xml",
        "Morphology codes/TEGMC - Translators Expansion of Greek Morphhology Codes - STEPBible.org CC BY.txt",
        "Morphology codes/TEHMC - Translators Expansion of Hebrew Morphology Codes - STEPBible.org CC BY.txt",
    ] },
    Pack { id: "crossrefs", name: "OpenBible cross-references", description: "340,000 weighted cross-references, incorporating the Treasury of Scripture Knowledge", license: "CC BY (openbible.info)", size: "3 MB", kind: Kind::CrossRefs, urls: &["https://a.openbible.info/data/cross-references.zip"] },
    Pack { id: "tsk", name: "Treasury of Scripture Knowledge", description: "Classic verse-by-verse cross-reference commentary (1880)", license: "Public domain", size: "3 MB", kind: Kind::Commentary, urls: &["TSK"] },
    Pack { id: "interlinear-nt", name: "Greek NT interlinear (STEPBible TAGNT)", description: "Every Greek word with Strong's number, morphology and gloss, NA28/KJV text", license: "CC BY 4.0 (Tyndale House)", size: "29 MB", kind: Kind::InterlinearNT, urls: &[
        "Translators Amalgamated OT+NT/TAGNT Mat-Jhn - Translators Amalgamated Greek NT - STEPBible.org CC-BY.txt",
        "Translators Amalgamated OT+NT/TAGNT Act-Rev - Translators Amalgamated Greek NT - STEPBible.org CC-BY.txt",
    ] },
    Pack { id: "interlinear-ot", name: "Hebrew OT interlinear (STEPBible TAHOT)", description: "Every Hebrew word with Strong's number, morphology, transliteration and gloss", license: "CC BY 4.0 (Tyndale House)", size: "68 MB", kind: Kind::InterlinearOT, urls: &[
        "Translators Amalgamated OT+NT/TAHOT Gen-Deu - Translators Amalgamated Hebrew OT - STEPBible.org CC BY.txt",
        "Translators Amalgamated OT+NT/TAHOT Jos-Est - Translators Amalgamated Hebrew OT - STEPBible.org CC BY.txt",
        "Translators Amalgamated OT+NT/TAHOT Job-Sng - Translators Amalgamated Hebrew OT - STEPBible.org CC BY.txt",
        "Translators Amalgamated OT+NT/TAHOT Isa-Mal - Translators Amalgamated Hebrew OT - STEPBible.org CC BY.txt",
    ] },
    Pack { id: "lexicon-greek", name: "Greek lexicon (STEPBible TBESG)", description: "Brief lexicon of every Greek word, based on Abbott-Smith and LSJ", license: "CC BY 4.0 (Tyndale House)", size: "5 MB", kind: Kind::LexiconGreek, urls: &["Lexicons/TBESG - Translators Brief lexicon of Extended Strongs for Greek - STEPBible.org CC BY.txt"] },
    Pack { id: "lexicon-hebrew", name: "Hebrew lexicon (STEPBible TBESH)", description: "Brief lexicon of every Hebrew and Aramaic word, based on BDB", license: "CC BY 4.0 (Tyndale House)", size: "3 MB", kind: Kind::LexiconHebrew, urls: &["Lexicons/TBESH - Translators Brief lexicon of Extended Strongs for Hebrew - STEPBible.org CC BY.txt"] },
    Pack { id: "mhc", name: "Matthew Henry's Commentary", description: "Complete commentary on the whole Bible (1710)", license: "Public domain", size: "15 MB", kind: Kind::Commentary, urls: &["MHC"] },
    Pack { id: "barnes", name: "Barnes' Notes", description: "Albert Barnes' Notes on the Old and New Testaments", license: "Public domain", size: "6 MB", kind: Kind::Commentary, urls: &["Barnes"] },
    Pack { id: "clarke", name: "Adam Clarke's Commentary", description: "Commentary on the Bible by Adam Clarke (1831)", license: "Public domain", size: "9 MB", kind: Kind::Commentary, urls: &["Clarke"] },
    Pack { id: "jfb", name: "Jamieson-Fausset-Brown", description: "Commentary Critical and Explanatory on the Whole Bible (1871)", license: "Public domain", size: "6 MB", kind: Kind::Commentary, urls: &["JFB"] },
    Pack { id: "calvin", name: "Calvin's Commentaries", description: "John Calvin's commentaries on most of the Bible", license: "Public domain", size: "21 MB", kind: Kind::Commentary, urls: &["CalvinCommentaries"] },
    Pack { id: "wesley", name: "Wesley's Notes", description: "John Wesley's Explanatory Notes on the Bible", license: "Public domain", size: "2 MB", kind: Kind::Commentary, urls: &["Wesley"] },
    Pack { id: "geneva", name: "Geneva Bible notes", description: "Study notes from the 1599 Geneva Bible", license: "Public domain", size: "2 MB", kind: Kind::Commentary, urls: &["Geneva"] },
    Pack { id: "uw-notes", name: "unfoldingWord Translation Notes", description: "Verse-by-verse study notes on every book, with cultural and grammatical background", license: "CC BY-SA 4.0", size: "40 MB", kind: Kind::UwNotes, urls: &["https://git.door43.org/unfoldingWord/en_tn/archive/master.zip"] },
    Pack { id: "uw-words", name: "unfoldingWord Translation Words", description: "Definitions of key terms, names and concepts", license: "CC BY-SA 4.0", size: "3 MB", kind: Kind::UwWords, urls: &["https://git.door43.org/unfoldingWord/en_tw/archive/master.zip"] },
    Pack { id: "easton", name: "Easton's Bible Dictionary", description: "Bible dictionary by M.G. Easton (1897)", license: "Public domain", size: "1 MB", kind: Kind::Dictionary, urls: &["Easton"] },
    Pack { id: "smith", name: "Smith's Bible Dictionary", description: "William Smith's Dictionary of the Bible (1863)", license: "Public domain", size: "1 MB", kind: Kind::Dictionary, urls: &["Smith"] },
    Pack { id: "isbe", name: "International Standard Bible Encyclopedia", description: "ISBE (1915), the largest public-domain Bible encyclopedia", license: "Public domain", size: "10 MB", kind: Kind::Dictionary, urls: &["ISBE"] },
    Pack { id: "nave", name: "Nave's Topical Bible", description: "Topics with all the verses about them", license: "Public domain", size: "1 MB", kind: Kind::Dictionary, urls: &["Nave"] },
    Pack { id: "hitchcock", name: "Hitchcock's Bible Names", description: "Meanings of Bible names", license: "Public domain", size: "0.1 MB", kind: Kind::Dictionary, urls: &["Hitchcock"] },
];

pub fn pack(id: &str) -> Option<&'static Pack> {
    PACKS.iter().find(|p| p.id == id)
}

pub fn resources_dir() -> PathBuf {
    bible::data_dir().join("resources")
}

pub fn pack_dir(id: &str) -> PathBuf {
    resources_dir().join(id)
}

pub fn is_installed(id: &str) -> bool {
    pack_dir(id).join("installed.json").exists()
}

pub fn installed_ids() -> Vec<&'static str> {
    PACKS.iter().filter(|p| is_installed(p.id)).map(|p| p.id).collect()
}

pub fn remove(id: &str) -> io::Result<()> {
    if pack(id).is_none() { return Err(io::Error::new(io::ErrorKind::InvalidInput, "Unknown resource ID")); }
    let lock = fs::OpenOptions::new().write(true).create(true).truncate(false)
        .open(resources_dir().join(format!(".{id}.lock")))?;
    lock.try_lock().map_err(|_| io::Error::other("This resource is being installed in another window"))?;
    fs::remove_dir_all(pack_dir(id))
}

#[derive(Serialize, Deserialize)]
struct Installed {
    id: String,
    name: String,
    license: String,
}

// ---------------------------------------------------------------------------
// Download
// ---------------------------------------------------------------------------

fn agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(600)))
        .user_agent("omascripture/0.2")
        .build();
    ureq::Agent::new_with_config(config)
}

fn get_bytes(url: &str) -> Result<Vec<u8>, String> {
    let mut resp = agent().get(url).call().map_err(|e| format!("{url}: {e}"))?;
    resp.body_mut()
        .with_config()
        .limit(512 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| e.to_string())
}

fn encode_path(p: &str) -> String {
    p.split('/')
        .map(|seg| {
            let mut s = String::new();
            for b in seg.bytes() {
                match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => s.push(b as char),
                    _ => s.push_str(&format!("%{b:02X}")),
                }
            }
            s
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Download and unpack a resource. Blocking; call from a worker thread.
pub fn install(id: &str) -> Result<(), String> {
    let p = pack(id).ok_or_else(|| format!("unknown resource '{id}'"))?;
    let dir = pack_dir(id);
    fs::create_dir_all(resources_dir()).map_err(|e| e.to_string())?;
    let lock = fs::OpenOptions::new().write(true).create(true).truncate(false)
        .open(resources_dir().join(format!(".{id}.lock"))).map_err(|e| e.to_string())?;
    lock.try_lock().map_err(|_| "This resource is being installed in another window".to_string())?;
    let previous = resources_dir().join(format!(".{id}.previous"));
    if !dir.exists() && previous.exists() { fs::rename(&previous, &dir).map_err(|e| e.to_string())?; }
    let tmp = resources_dir().join(format!(".{id}.partial"));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        match p.kind {
            Kind::WordData => {
                crate::word_data::import_kjv(&get_bytes(p.urls[0])?, &tmp)?;
                for (url, file) in p.urls[1..].iter().zip(["greek.tsv", "hebrew.tsv"]) {
                    fs::write(tmp.join(file), get_bytes(&format!("{STEP_BASE}/{}", encode_path(url)))?).map_err(|e| e.to_string())?;
                }
            }
            Kind::CrossRefs => {
                let zip = get_bytes(p.urls[0])?;
                extract_zip(&zip, &tmp, |n| n.ends_with(".txt"))?;
            }
            Kind::Commentary | Kind::Dictionary => {
                let zip = get_bytes(&format!("{SWORD_BASE}/{}.zip", p.urls[0]))?;
                extract_zip(&zip, &tmp, |_| true)?;
            }
            Kind::InterlinearNT | Kind::InterlinearOT => {
                for u in p.urls {
                    let text = get_bytes(&format!("{STEP_BASE}/{}", encode_path(u)))?;
                    split_step_by_book(&String::from_utf8_lossy(&text), &tmp)?;
                }
            }
            Kind::LexiconGreek | Kind::LexiconHebrew => {
                let text = get_bytes(&format!("{STEP_BASE}/{}", encode_path(p.urls[0])))?;
                fs::write(tmp.join("lexicon.tsv"), text).map_err(|e| e.to_string())?;
            }
            Kind::UwNotes => {
                let zip = get_bytes(p.urls[0])?;
                extract_zip(&zip, &tmp, |n| n.ends_with(".tsv"))?;
            }
            Kind::UwWords => {
                let zip = get_bytes(p.urls[0])?;
                build_words_json(&zip, &tmp)?;
            }
        }
        let marker = Installed { id: p.id.into(), name: p.name.into(), license: p.license.into() };
        fs::write(tmp.join("installed.json"), serde_json::to_string_pretty(&marker).unwrap()).map_err(|e| e.to_string())?;
        Ok(())
    })();
    match result {
        Ok(()) => publish_resource(&tmp, &dir, &resources_dir().join(format!(".{id}.previous"))),
        Err(e) => {
            let _ = fs::remove_dir_all(&tmp);
            Err(e)
        }
    }
}

fn publish_resource(staged: &std::path::Path, destination: &std::path::Path, previous: &std::path::Path) -> Result<(), String> {
    if destination.exists() {
        if previous.exists() { fs::remove_dir_all(previous).map_err(|e| e.to_string())?; }
        fs::rename(destination, previous).map_err(|e| e.to_string())?;
    }
    if let Err(e) = fs::rename(staged, destination) {
        if previous.exists() { let _ = fs::rename(previous, destination); }
        return Err(e.to_string());
    }
    let _ = fs::remove_dir_all(previous);
    Ok(())
}

fn extract_zip(bytes: &[u8], dest: &std::path::Path, keep: impl Fn(&str) -> bool) -> Result<(), String> {
    let cursor = io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;
    if archive.len() > 100_000 { return Err("Archive has too many entries".into()); }
    let mut expanded = 0u64;
    for i in 0..archive.len() {
        let f = archive.by_index(i).map_err(|e| e.to_string())?;
        expanded = expanded.checked_add(f.size()).ok_or("Archive is too large")?;
        if f.size() > 128 * 1024 * 1024 || expanded > 1024 * 1024 * 1024 { return Err("Archive exceeds expanded size limit".into()); }
        if f.is_dir() || !keep(f.name()) {
            continue;
        }
        let Some(rel) = f.enclosed_name() else { continue };
        let out = dest.join(rel);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::new();
        f.take(128 * 1024 * 1024 + 1).read_to_end(&mut buf).map_err(|e| e.to_string())?;
        if buf.len() > 128 * 1024 * 1024 { return Err("Archive entry exceeds size limit".into()); }
        fs::write(&out, buf).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// STEPBible book codes → book number.
pub const STEP_BOOKS: [&str; 66] = [
    "Gen", "Exo", "Lev", "Num", "Deu", "Jos", "Jdg", "Rut", "1Sa", "2Sa", "1Ki", "2Ki", "1Ch", "2Ch", "Ezr", "Neh", "Est", "Job", "Psa", "Pro",
    "Ecc", "Sng", "Isa", "Jer", "Lam", "Ezk", "Dan", "Hos", "Jol", "Amo", "Oba", "Jon", "Mic", "Nam", "Hab", "Zep", "Hag", "Zec", "Mal", "Mat",
    "Mrk", "Luk", "Jhn", "Act", "Rom", "1Co", "2Co", "Gal", "Eph", "Php", "Col", "1Th", "2Th", "1Ti", "2Ti", "Tit", "Phm", "Heb", "Jas", "1Pe",
    "2Pe", "1Jn", "2Jn", "3Jn", "Jud", "Rev",
];

pub fn step_book_nr(code: &str) -> Option<u32> {
    STEP_BOOKS.iter().position(|c| c.eq_ignore_ascii_case(code)).map(|i| i as u32 + 1)
}

pub fn osis_book_nr(code: &str) -> Option<u32> {
    v11n::OSIS.iter().position(|c| c.eq_ignore_ascii_case(code)).map(|i| i as u32 + 1)
}

fn split_step_by_book(text: &str, dest: &std::path::Path) -> Result<(), String> {
    let mut files: BTreeMap<u32, String> = BTreeMap::new();
    for line in text.lines() {
        // Data lines look like "Jhn.3.16#01=NKO\t..."
        let Some(dot) = line.find('.') else { continue };
        if dot == 0 || dot > 3 || !line[dot + 1..].starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        let Some(nr) = step_book_nr(&line[..dot]) else { continue };
        files.entry(nr).or_default().push_str(line);
        files.entry(nr).or_default().push('\n');
    }
    for (nr, content) in files {
        let path = dest.join(format!("{nr}.tsv"));
        let mut existing = fs::read_to_string(&path).unwrap_or_default();
        existing.push_str(&content);
        fs::write(&path, existing).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn build_words_json(zip_bytes: &[u8], dest: &std::path::Path) -> Result<(), String> {
    let cursor = io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;
    let mut words: BTreeMap<String, UwWord> = BTreeMap::new();
    if archive.len() > 100_000 { return Err("Archive has too many entries".into()); }
    let mut expanded = 0u64;
    for i in 0..archive.len() {
        let f = archive.by_index(i).map_err(|e| e.to_string())?;
        expanded = expanded.checked_add(f.size()).ok_or("Archive is too large")?;
        if f.size() > 128 * 1024 * 1024 || expanded > 1024 * 1024 * 1024 { return Err("Archive exceeds expanded size limit".into()); }
        let name = f.name().to_string();
        if !name.contains("/bible/") || !name.ends_with(".md") {
            continue;
        }
        let mut text = String::new();
        f.take(8 * 1024 * 1024 + 1).read_to_string(&mut text).map_err(|e| e.to_string())?;
        if text.len() > 8 * 1024 * 1024 { return Err("Dictionary entry exceeds size limit".into()); }
        let slug = name.rsplit('/').next().unwrap_or("").trim_end_matches(".md").to_string();
        let category = if name.contains("/kt/") { "key term" } else if name.contains("/names/") { "name" } else { "other" };
        let title = text.lines().next().unwrap_or("").trim_start_matches('#').trim().to_string();
        let body = text.lines().skip(1).collect::<Vec<_>>().join("\n");
        words.insert(slug, UwWord { title, category: category.into(), body });
    }
    fs::write(dest.join("words.json"), serde_json::to_string(&words).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Cross-references
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CrossRef {
    pub to: Position,
    pub to_end: Option<Position>,
    pub votes: i32,
}

pub struct CrossRefs {
    map: HashMap<Position, Vec<CrossRef>>,
}

pub fn parse_osis_ref(s: &str) -> Option<Position> {
    let mut it = s.split('.');
    let book = osis_book_nr(it.next()?)?;
    let chapter = it.next()?.parse().ok()?;
    let verse = it.next()?.parse().ok()?;
    Some(Position { book, chapter, verse })
}

impl CrossRefs {
    pub fn load() -> io::Result<CrossRefs> {
        let dir = pack_dir("crossrefs");
        let mut path = dir.join("cross_references.txt");
        if !path.exists()
            && let Some(p) = fs::read_dir(&dir)?.flatten().map(|e| e.path()).find(|p| p.extension().map(|e| e == "txt").unwrap_or(false)) {
                path = p;
            }
        let text = fs::read_to_string(path)?;
        let mut map: HashMap<Position, Vec<CrossRef>> = HashMap::new();
        for line in text.lines().skip(1) {
            let mut cols = line.split('\t');
            let (Some(from), Some(to), Some(votes)) = (cols.next(), cols.next(), cols.next()) else { continue };
            let Some(from) = parse_osis_ref(from) else { continue };
            let (to_s, to_end_s) = match to.split_once('-') {
                Some((a, b)) => (a, Some(b)),
                None => (to, None),
            };
            let Some(to) = parse_osis_ref(to_s) else { continue };
            let to_end = to_end_s.and_then(parse_osis_ref);
            let votes = votes.trim().parse().unwrap_or(0);
            map.entry(from).or_default().push(CrossRef { to, to_end, votes });
        }
        for v in map.values_mut() {
            v.sort_by_key(|r| std::cmp::Reverse(r.votes));
        }
        Ok(CrossRefs { map })
    }

    pub fn get(&self, pos: Position) -> &[CrossRef] {
        self.map.get(&pos).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

// ---------------------------------------------------------------------------
// Interlinear
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Word {
    pub verse: u32,
    pub chapter: u32,
    pub text: String,
    pub translit: String,
    pub gloss: String,
    pub strong: String,
    pub morph: String,
    pub lemma: String,
    pub lemma_gloss: String,
    pub variant: bool,
}

#[derive(Default)]
pub struct Interlinear {
    books: HashMap<u32, Vec<Word>>,
    missing: HashMap<u32, bool>,
    last_book: Option<u32>,
    last_verse: Option<(Position, Vec<Word>)>,
}

/// Retain the current and previous book for navigation/parallel study, without
/// allowing a long reading session to accumulate the entire corpus in RAM.
fn retain_recent_books<T>(books: &mut HashMap<u32, T>, book: u32, previous: &mut Option<u32>) {
    if *previous != Some(book) {
        books.retain(|key, _| *key == book || Some(*key) == *previous);
        *previous = Some(book);
    }
}

fn strip_braces(s: &str) -> String {
    s.chars().filter(|c| *c != '{' && *c != '}').collect()
}

/// Strong's numbers referenced by a tag such as "H9003/{H7225G}" or "G2424G".
pub fn strongs_in(tag: &str) -> Vec<String> {
    strip_braces(tag)
        .split(['/', '+', '|'])
        .filter_map(|s| {
            let s = s.trim();
            let core: String = s.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            if core.len() >= 2 && (core.starts_with('G') || core.starts_with('H')) && core[1..].starts_with(|c: char| c.is_ascii_digit()) {
                Some(core)
            } else {
                None
            }
        })
        .collect()
}

/// Primary Strong's number of a tag: the braced one if present, else the last.
pub fn main_strong(tag: &str) -> String {
    if let Some(start) = tag.find('{')
        && let Some(end) = tag[start..].find('}') {
            let inner = &tag[start + 1..start + end];
            return strongs_in(inner).into_iter().next().unwrap_or_default();
        }
    strongs_in(tag).into_iter().last().unwrap_or_default()
}

/// Normalise "G0025G" / "H7225G" to the base "G0025" / "H7225" form.
pub fn base_strong(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i == 0 || c.is_ascii_digit() {
            out.push(c);
        } else {
            break;
        }
    }
    // pad number to 4 digits
    if out.len() > 1 {
        let (p, n) = out.split_at(1);
        if let Ok(v) = n.parse::<u32>() {
            return format!("{p}{v:04}");
        }
    }
    out
}

impl Interlinear {
    #[cfg(test)]
    pub(crate) fn test_book(book: u32, text: &str) -> Self {
        let mut interlinear = Self::default();
        interlinear.books.insert(book, if book >= 40 { parse_tagnt(text) } else { parse_tahot(text) });
        interlinear.missing.extend((1..=66).map(|b| (b, true)));
        interlinear
    }

    fn ensure(&mut self, book: u32) {
        retain_recent_books(&mut self.books, book, &mut self.last_book);
        if self.books.contains_key(&book) || self.missing.contains_key(&book) {
            return;
        }
        let id = if book >= 40 { "interlinear-nt" } else { "interlinear-ot" };
        let path = pack_dir(id).join(format!("{book}.tsv"));
        match fs::read_to_string(&path) {
            Ok(text) => {
                let words = if book >= 40 { parse_tagnt(&text) } else { parse_tahot(&text) };
                self.books.insert(book, words);
            }
            Err(_) => {
                self.missing.insert(book, true);
            }
        }
    }

    pub fn verse(&mut self, pos: Position) -> Vec<Word> {
        if let Some((cached, words)) = &self.last_verse
            && *cached == pos {
            return words.clone();
        }
        self.ensure(pos.book);
        let words: Vec<Word> = self.books
            .get(&pos.book)
            .map(|ws| ws.iter().filter(|w| w.chapter == pos.chapter && w.verse == pos.verse).cloned().collect())
            .unwrap_or_default();
        self.last_verse = Some((pos, words.clone()));
        words
    }


    pub fn report(&self, strong: &str, limit: usize) -> crate::word_data::WordReport {
        let mut report = crate::word_data::WordReport::default();
        let greek = strong.starts_with('G');
        let base = base_strong(strong);
        let extended = strong != base;
        let mut verses = std::collections::HashSet::new();
        for b in if greek { 40..=66 } else { 1..=39 } {
            // A corpus scan must not fill the interactive reader's book cache.
            // Each temporary book is released before loading the next one.
            let loaded = if self.books.contains_key(&b) || self.missing.contains_key(&b) {
                None
            } else {
                let id = if greek { "interlinear-nt" } else { "interlinear-ot" };
                fs::read_to_string(pack_dir(id).join(format!("{b}.tsv"))).ok()
                    .map(|text| if greek { parse_tagnt(&text) } else { parse_tahot(&text) })
            };
            let Some(words) = self.books.get(&b).or(loaded.as_ref()) else { continue };
            report.loaded_books += 1;
            for w in words.iter().filter(|w| !w.variant && if extended { w.strong == strong } else { base_strong(&w.strong) == base }) {
                report.total += 1;
                *report.book_counts.entry(b).or_default() += 1;
                *report.forms.entry(w.text.trim_matches(|c: char| !c.is_alphanumeric() && !(0x0300..=0x036f).contains(&(c as u32))).to_string()).or_default() += 1;
                let pos = Position { book: b, chapter: w.chapter, verse: w.verse };
                if verses.insert(pos) && report.verses.len() < limit { report.verses.push((pos, w.gloss.clone())); }
            }
        }
        report.verse_count = verses.len(); report
    }

    /// All verses across installed books containing a Strong's number.
    pub fn occurrences(&mut self, strong: &str, limit: usize) -> (usize, Vec<(Position, String)>) {
        let base = base_strong(strong);
        let range: Vec<u32> = if base.starts_with('G') { (40..=66).collect() } else { (1..=39).collect() };
        let mut total = 0;
        let mut out = Vec::new();
        for b in range {
            self.ensure(b);
            let Some(words) = self.books.get(&b) else { continue };
            let mut last: Option<(u32, u32)> = None;
            for w in words {
                if base_strong(&w.strong) == base {
                    total += 1;
                    if last != Some((w.chapter, w.verse)) && out.len() < limit {
                        out.push((Position { book: b, chapter: w.chapter, verse: w.verse }, w.gloss.clone()));
                        last = Some((w.chapter, w.verse));
                    }
                }
            }
        }
        (total, out)
    }
}

fn parse_ref(cell: &str) -> Option<(u32, u32, u32, String)> {
    // "Jhn.3.16#01=NKO"
    let (r, rest) = cell.split_once('#')?;
    let mut it = r.split('.');
    it.next()?;
    let chapter = it.next()?.parse().ok()?;
    let verse: u32 = it.next()?.trim_end_matches(|c: char| !c.is_ascii_digit()).parse().ok()?;
    let src = rest.split_once('=').map(|(_, s)| s.to_string()).unwrap_or_default();
    Some((chapter, verse, 0, src))
}

fn parse_tagnt(text: &str) -> Vec<Word> {
    let mut out = Vec::new();
    let mut seen: Option<String> = None;
    for line in text.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 5 {
            continue;
        }
        let Some((chapter, verse, _, src)) = parse_ref(cols[0]) else { continue };
        // Skip words that appear only in "other" editions (pure variants).
        if !src.contains('N') && !src.contains('K') {
            continue;
        }
        if seen.as_deref() == Some(cols[0]) {
            continue;
        }
        seen = Some(cols[0].to_string());
        let (greek, translit) = match cols[1].split_once(" (") {
            Some((g, t)) => (g.trim().to_string(), t.trim_end_matches(')').to_string()),
            None => (cols[1].trim().to_string(), String::new()),
        };
        let (strong, morph) = cols[3].split_once('=').map(|(s, m)| (s.to_string(), m.to_string())).unwrap_or((cols[3].to_string(), String::new()));
        let (lemma, lemma_gloss) = cols
            .get(4)
            .and_then(|c| c.split_once('='))
            .map(|(l, g)| (l.to_string(), g.to_string()))
            .unwrap_or_default();
        out.push(Word {
            verse,
            chapter,
            text: greek,
            translit,
            gloss: cols[2].trim().to_string(),
            strong: main_strong(&strong),
            morph,
            lemma,
            lemma_gloss,
            variant: !src.contains('N'),
        });
    }
    out
}

fn parse_tahot(text: &str) -> Vec<Word> {
    let mut out = Vec::new();
    let mut seen: Option<String> = None;
    for line in text.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 6 {
            continue;
        }
        let Some((chapter, verse, _, src)) = parse_ref(cols[0]) else { continue };
        // "L" = Leningrad text; Q/K are qere/ketiv alternates — keep L only.
        if !src.starts_with('L') {
            continue;
        }
        if seen.as_deref() == Some(cols[0]) {
            continue;
        }
        seen = Some(cols[0].to_string());
        let tag = cols[4];
        let expanded = cols.get(11).copied().unwrap_or("");
        // Expanded tags: "H9003=ב=in/{H7225G=רֵאשִׁית=: beginning»first:1_beginning}"
        let main = main_strong(tag);
        let mut lemma = String::new();
        let mut lemma_gloss = String::new();
        for part in strip_braces(expanded).split('/') {
            let mut it = part.splitn(3, '=');
            let s = it.next().unwrap_or("");
            if base_strong(s) == base_strong(&main) {
                lemma = it.next().unwrap_or("").to_string();
                lemma_gloss = it.next().unwrap_or("").replace('»', " » ").replace('_', " ").trim_start_matches(": ").to_string();
            }
        }
        out.push(Word {
            verse,
            chapter,
            text: cols[1].replace('/', ""),
            translit: cols[2].replace('/', ""),
            gloss: cols[3].replace("/ ", "-").trim().to_string(),
            strong: main,
            morph: cols[5].to_string(),
            lemma,
            lemma_gloss,
            variant: false,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Lexicons
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LexEntry {
    pub strong: String,
    pub lemma: String,
    pub translit: String,
    pub morph: String,
    pub gloss: String,
    pub meaning: String,
}

pub struct Lexicon {
    entries: HashMap<String, LexEntry>,
}

impl Lexicon {
    pub fn load(greek: bool) -> io::Result<Lexicon> {
        let id = if greek { "lexicon-greek" } else { "lexicon-hebrew" };
        let text = fs::read_to_string(pack_dir(id).join("lexicon.tsv"))?;
        let mut entries = HashMap::new();
        for line in text.lines() {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 8 || !(cols[0].starts_with('G') || cols[0].starts_with('H')) {
                continue;
            }
            let e = LexEntry {
                strong: cols[0].to_string(),
                lemma: cols[3].to_string(),
                translit: cols[4].to_string(),
                morph: cols[5].to_string(),
                gloss: cols[6].to_string(),
                meaning: crate::sword::strip_markup(&cols[7].replace("<br>", "\n").replace("<BR />", "\n").replace("<BR/>", "\n"))
                    .replace("\n__", "\n  ")
                    .replace("\n  __", "\n    "),
            };
            entries.entry(base_strong(cols[0])).or_insert_with(|| e.clone());
            entries.insert(cols[0].to_string(), e);
        }
        Ok(Lexicon { entries })
    }

    pub fn get(&self, strong: &str) -> Option<&LexEntry> {
        self.entries.get(strong).or_else(|| self.entries.get(&base_strong(strong)))
    }
}

// ---------------------------------------------------------------------------
// unfoldingWord notes & words
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Note {
    pub chapter: u32,
    pub verse: u32,
    pub quote: String,
    pub text: String,
}

#[derive(Default)]
pub struct UwNotes {
    books: HashMap<u32, Vec<Note>>,
    missing: HashMap<u32, bool>,
    last_book: Option<u32>,
}

pub const USFM_BOOKS: [&str; 66] = [
    "GEN", "EXO", "LEV", "NUM", "DEU", "JOS", "JDG", "RUT", "1SA", "2SA", "1KI", "2KI", "1CH", "2CH", "EZR", "NEH", "EST", "JOB", "PSA", "PRO",
    "ECC", "SNG", "ISA", "JER", "LAM", "EZK", "DAN", "HOS", "JOL", "AMO", "OBA", "JON", "MIC", "NAM", "HAB", "ZEP", "HAG", "ZEC", "MAL", "MAT",
    "MRK", "LUK", "JHN", "ACT", "ROM", "1CO", "2CO", "GAL", "EPH", "PHP", "COL", "1TH", "2TH", "1TI", "2TI", "TIT", "PHM", "HEB", "JAS", "1PE",
    "2PE", "1JN", "2JN", "3JN", "JUD", "REV",
];

fn clean_markdown(s: &str) -> String {
    s.replace("\\n", "\n")
        .replace("**", "")
        .replace("\\*", "*")
        .replace("<br>", "\n")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

impl UwNotes {
    fn ensure(&mut self, book: u32) {
        retain_recent_books(&mut self.books, book, &mut self.last_book);
        if self.books.contains_key(&book) || self.missing.contains_key(&book) {
            return;
        }
        let code = USFM_BOOKS[(book - 1) as usize];
        let dir = pack_dir("uw-notes");
        let mut found = None;
        if let Ok(rd) = fs::read_dir(&dir) {
            for e in rd.flatten() {
                if e.path().is_dir() {
                    let p = e.path().join(format!("tn_{code}.tsv"));
                    if p.exists() {
                        found = Some(p);
                    }
                }
            }
        }
        let path = found.unwrap_or_else(|| dir.join(format!("tn_{code}.tsv")));
        match fs::read_to_string(path) {
            Ok(text) => {
                let mut notes = Vec::new();
                for line in text.lines().skip(1) {
                    let cols: Vec<&str> = line.split('\t').collect();
                    if cols.len() < 7 {
                        continue;
                    }
                    let Some((c, v)) = cols[0].split_once(':') else { continue };
                    let chapter = c.parse().unwrap_or(0);
                    let verse = v.split(['-', ',']).next().and_then(|s| s.parse().ok()).unwrap_or(0);
                    notes.push(Note { chapter, verse, quote: cols[4].to_string(), text: clean_markdown(cols[6]) });
                }
                self.books.insert(book, notes);
            }
            Err(_) => {
                self.missing.insert(book, true);
            }
        }
    }

    pub fn verse(&mut self, pos: Position) -> Vec<Note> {
        self.ensure(pos.book);
        self.books
            .get(&pos.book)
            .map(|ns| ns.iter().filter(|n| n.chapter == pos.chapter && n.verse == pos.verse).cloned().collect())
            .unwrap_or_default()
    }

    pub fn chapter_intro(&mut self, pos: Position) -> Option<Note> {
        self.ensure(pos.book);
        self.books.get(&pos.book)?.iter().find(|n| n.chapter == pos.chapter && n.verse == 0).cloned()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UwWord {
    pub title: String,
    pub category: String,
    pub body: String,
}

pub struct UwWords {
    words: BTreeMap<String, UwWord>,
}

impl UwWords {
    pub fn load() -> io::Result<UwWords> {
        let text = fs::read_to_string(pack_dir("uw-words").join("words.json"))?;
        let words = serde_json::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(UwWords { words })
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<(String, String)> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (slug, w) in &self.words {
            if slug.contains(&q) || w.title.to_lowercase().contains(&q) {
                out.push((slug.clone(), w.title.clone()));
                if out.len() >= limit {
                    break;
                }
            }
        }
        out
    }

    pub fn get(&self, slug: &str) -> Option<&UwWord> {
        self.words.get(slug)
    }
}

// ---------------------------------------------------------------------------
// Aggregate store with lazy loading
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct Store {
    crossrefs: Option<Result<CrossRefs, String>>,
    pub interlinear: Interlinear,
    lex_greek: Option<Result<Lexicon, String>>,
    lex_hebrew: Option<Result<Lexicon, String>>,
    commentaries: HashMap<String, Result<Commentary, String>>,
    last_commentary: Option<String>,
    dictionaries: HashMap<String, Result<Dictionary, String>>,
    pub notes: UwNotes,
    words: Option<Result<UwWords, String>>,
}

impl Store {
    /// Forget everything loaded so newly installed packs are picked up.
    pub fn reset(&mut self) {
        *self = Store::default();
    }

    pub fn crossrefs(&mut self) -> Option<&CrossRefs> {
        if self.crossrefs.is_none() && is_installed("crossrefs") {
            self.crossrefs = Some(CrossRefs::load().map_err(|e| e.to_string()));
        }
        self.crossrefs.as_ref().and_then(|r| r.as_ref().ok())
    }

    pub fn lexicon(&mut self, strong: &str) -> Option<&Lexicon> {
        let greek = strong.starts_with('G');
        let slot = if greek { &mut self.lex_greek } else { &mut self.lex_hebrew };
        let id = if greek { "lexicon-greek" } else { "lexicon-hebrew" };
        if slot.is_none() && is_installed(id) {
            *slot = Some(Lexicon::load(greek).map_err(|e| e.to_string()));
        }
        slot.as_ref().and_then(|r| r.as_ref().ok())
    }

    pub fn commentary(&mut self, id: &str) -> Option<&Commentary> {
        if self.last_commentary.as_deref() != Some(id) {
            self.commentaries.retain(|key, _| key == id || Some(key.as_str()) == self.last_commentary.as_deref());
            self.last_commentary = Some(id.to_string());
        }
        if !self.commentaries.contains_key(id) {
            if !is_installed(id) {
                return None;
            }
            let r = Commentary::open(&pack_dir(id)).map_err(|e| e.to_string());
            self.commentaries.insert(id.to_string(), r);
        }
        self.commentaries.get(id).and_then(|r| r.as_ref().ok())
    }

    pub fn commentary_ids() -> Vec<&'static str> {
        PACKS.iter().filter(|p| p.kind == Kind::Commentary && p.id != "tsk" && is_installed(p.id)).map(|p| p.id).collect()
    }

    pub fn dictionary(&mut self, id: &str) -> Option<&Dictionary> {
        if !self.dictionaries.contains_key(id) {
            if !is_installed(id) {
                return None;
            }
            let r = Dictionary::open(&pack_dir(id)).map_err(|e| e.to_string());
            self.dictionaries.insert(id.to_string(), r);
        }
        self.dictionaries.get(id).and_then(|r| r.as_ref().ok())
    }

    pub fn dictionary_ids() -> Vec<&'static str> {
        PACKS.iter().filter(|p| p.kind == Kind::Dictionary && is_installed(p.id)).map(|p| p.id).collect()
    }

    pub fn words(&mut self) -> Option<&UwWords> {
        if self.words.is_none() && is_installed("uw-words") {
            self.words = Some(UwWords::load().map_err(|e| e.to_string()));
        }
        self.words.as_ref().and_then(|r| r.as_ref().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browsing_books_keeps_only_current_and_previous() {
        let mut books = HashMap::new();
        let mut previous = None;
        for book in 1..=66 {
            retain_recent_books(&mut books, book, &mut previous);
            books.entry(book).or_insert(book);
            assert!(books.len() <= 2);
            assert_eq!(books.get(&book), Some(&book));
            if book > 1 { assert!(books.contains_key(&(book - 1))); }
        }
        retain_recent_books(&mut books, 65, &mut previous);
        assert!(books.contains_key(&66));
        retain_recent_books(&mut books, 64, &mut previous);
        books.insert(64, 64);
        assert!(books.contains_key(&65));
        assert!(!books.contains_key(&66));
    }

    #[test]
    fn cached_verse_tracks_navigation_and_returns_independent_words() {
        let mut reader = Interlinear::test_book(43,
            "Jhn.3.16#01=NKO\tἠγάπησεν (ēgapēsen)\tloved\tG0025=V-AAI-3S\tἀγαπάω=to love\nJhn.3.17#01=NKO\tθεός (theos)\tGod\tG2316=N-NSM\tθεός=God\n");
        let pos = Position { book: 43, chapter: 3, verse: 16 };
        let mut words = reader.verse(pos);
        words[0].gloss = "modified by caller".into();
        assert_eq!(reader.verse(pos)[0].gloss, "loved");
        assert_eq!(reader.verse(Position { verse: 17, ..pos })[0].gloss, "God");
        assert_eq!(reader.verse(pos)[0].gloss, "loved");
    }

    /// Run separately for each corpus so VmHWM measures that scan, not earlier tests.
    #[test]
    #[ignore = "performance diagnostic requiring installed interlinear packs"]
    fn profile_installed_word_report() {
        let strong = std::env::var("OMASCRIPTURE_PROFILE_STRONG").unwrap_or("G0025".into());
        let start = std::time::Instant::now();
        let reader = Interlinear::default();
        let report = reader.report(&strong, crate::app::OCCURRENCE_LIMIT);
        assert!(report.loaded_books > 0, "Install the interlinear corpus first");
        println!("{strong}: {} occurrences, {} verses, {} books, {:?}, {} retained books",
            report.total, report.verse_count, report.loaded_books, start.elapsed(), reader.books.len());
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines().filter(|line| line.starts_with("VmHWM:") || line.starts_with("VmRSS:")) {
                println!("{line}");
            }
        }
    }

    #[test]
    fn usage_counts_tokens_and_unique_verses_without_variant_double_counting() {
        let source = "Jhn.1.1#01=NKO\tλόγος (logos)\tword\tG3056=N-NSM\tλόγος=word\nJhn.1.1#02=NKO\tλόγον (logon)\tword\tG3056=N-ASM\tλόγος=word\nJhn.1.2#01=KO\tλόγος (logos)\tword\tG3056=N-NSM\tλόγος=word\n";
        let interlinear = Interlinear::test_book(43, source);
        let report = interlinear.report("G3056", 1);
        assert_eq!(report.total, 2); assert_eq!(report.verse_count, 1);
        assert_eq!(report.loaded_books, 1); assert_eq!(report.forms.len(), 2);
        assert_eq!(report.verses.len(), 1);
    }

    #[test]
    fn failed_resource_publish_restores_existing_pack() {
        let dir = crate::storage::TestDir::new();
        let destination = dir.0.join("resource"); let previous = dir.0.join("previous");
        fs::create_dir(&destination).unwrap(); fs::write(destination.join("data"), "installed").unwrap();
        assert!(publish_resource(&dir.0.join("missing-stage"), &destination, &previous).is_err());
        assert_eq!(fs::read_to_string(destination.join("data")).unwrap(), "installed");
        assert!(!previous.exists());
        let staged = dir.0.join("stage"); fs::create_dir(&staged).unwrap(); fs::write(staged.join("data"), "replacement").unwrap();
        publish_resource(&staged, &destination, &previous).unwrap();
        assert_eq!(fs::read_to_string(destination.join("data")).unwrap(), "replacement");
        assert!(!previous.exists());
        assert!(remove("../outside").is_err());
    }

    #[test]
    fn strong_helpers() {
        assert_eq!(main_strong("H9003/{H7225G}"), "H7225G");
        assert_eq!(main_strong("G2424G"), "G2424G");
        assert_eq!(main_strong("H1732|G1138«G1138"), "G1138");
        assert_eq!(base_strong("H7225G"), "H7225");
        assert_eq!(base_strong("G25"), "G0025");
        assert_eq!(strongs_in("H9003/{H7225G}"), vec!["H9003", "H7225G"]);
    }

    #[test]
    fn parses_tagnt_line() {
        let line = "Jhn.3.16#03=NKO\tἠγάπησεν (ēgapēsen)\tloved\tG0025=V-AAI-3S\tἀγαπάω=to love\tNA28\t\t\tamó\tto love\t#03\tG0025\t";
        let w = parse_tagnt(line);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].text, "ἠγάπησεν");
        assert_eq!(w[0].translit, "ēgapēsen");
        assert_eq!(w[0].strong, "G0025");
        assert_eq!(w[0].morph, "V-AAI-3S");
        assert_eq!(w[0].lemma, "ἀγαπάω");
        assert_eq!((w[0].chapter, w[0].verse), (3, 16));
    }

    #[test]
    fn parses_tahot_line() {
        let line = "Gen.1.1#01=L\tבְּ/רֵאשִׁ֖ית\tbe./re.Shit\tin/ beginning\tH9003/{H7225G}\tHR/Ncfsa\t\t\tH7225G\t\t\tH9003=ב=in/{H7225G=רֵאשִׁית=: beginning»first:1_beginning}";
        let w = parse_tahot(line);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].strong, "H7225G");
        assert_eq!(w[0].lemma, "רֵאשִׁית");
        assert_eq!(w[0].translit, "be.re.Shit");
        assert_eq!(w[0].gloss, "in-beginning");
    }

    #[test]
    fn osis_refs() {
        let p = parse_osis_ref("1John.1.1").unwrap();
        assert_eq!((p.book, p.chapter, p.verse), (62, 1, 1));
        assert_eq!(parse_osis_ref("Ps.121.2").unwrap().book, 19);
    }
}
