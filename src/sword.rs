//! Minimal reader for CrossWire SWORD modules: zCom/zCom4 commentaries and
//! zLD/RawLD dictionaries. Only what OmaScripture needs; KJV versification.

use crate::bible::Position;
use crate::v11n;
use flate2::read::ZlibDecoder;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn inflate(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    const MAX_BLOCK: u64 = 64 * 1024 * 1024;
    ZlibDecoder::new(data).take(MAX_BLOCK + 1).read_to_end(&mut out)?;
    if out.len() as u64 > MAX_BLOCK { return Err(io::Error::new(io::ErrorKind::InvalidData, "SWORD block exceeds size limit")); }
    Ok(out)
}

/// Parse a SWORD .conf file into key/value pairs (first section only).
pub fn parse_conf(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in text.lines() {
        let line = line.trim_start_matches('\u{feff}');
        if line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.entry(k.trim().to_string()).or_insert_with(|| v.trim().to_string());
        }
    }
    map
}

/// Index of a verse inside a SWORD testament file (KJV versification).
/// Layout: [module heading][testament heading] then per book: [book heading],
/// per chapter: [chapter heading] + verses. So Genesis 1:1 is index 4.
fn verse_index(pos: Position) -> Option<(bool, usize)> {
    let nt = pos.book >= 40;
    let first = if nt { 40 } else { 1 };
    if pos.book < 1 || pos.book > 66 {
        return None;
    }
    let mut i = 1usize;
    for b in first..=66u32 {
        if !nt && b >= 40 {
            break;
        }
        let chapters = v11n::VERSES[(b - 1) as usize];
        i += 1;
        for (ci, &n) in chapters.iter().enumerate() {
            i += 1;
            if b == pos.book && (ci as u32 + 1) == pos.chapter {
                if pos.verse == 0 || pos.verse > n as u32 {
                    return None;
                }
                return Some((nt, i + pos.verse as usize));
            }
            i += n as usize;
        }
        if b == pos.book {
            return None;
        }
    }
    None
}

struct Testament {
    bzs: Vec<u8>,
    bzv: Vec<u8>,
    bzz: Vec<u8>,
}

/// A zCom / zCom4 commentary module.
pub struct Commentary {
    entry_size: usize,
    ot: Option<Testament>,
    nt: Option<Testament>,
    cache: RefCell<Option<(bool, u32, Vec<u8>)>>,
}

impl Commentary {
    /// `dir` is the extracted package root containing mods.d/ and modules/.
    pub fn open(dir: &Path) -> io::Result<Commentary> {
        let conf_path = find_conf(dir)?;
        let conf = parse_conf(&fs::read_to_string(&conf_path)?);
        let drv = conf.get("ModDrv").cloned().unwrap_or_default();
        let entry_size = match drv.as_str() {
            "zCom" => 10,
            "zCom4" => 12,
            other => {
                return Err(io::Error::new(io::ErrorKind::Unsupported, format!("unsupported ModDrv {other}")));
            }
        };
        let data_path = conf.get("DataPath").cloned().unwrap_or_default();
        let data_dir = dir.join(data_path.trim_start_matches("./"));
        // File extensions encode the block type: bz* = BOOK, cz* = CHAPTER, vz* = VERSE.
        let ext = match conf.get("BlockType").map(|s| s.to_ascii_uppercase()).as_deref() {
            Some("CHAPTER") => "cz",
            Some("VERSE") => "vz",
            _ => "bz",
        };
        let load = |prefix: &str| -> Option<Testament> {
            let bzs = fs::read(data_dir.join(format!("{prefix}.{ext}s"))).ok()?;
            let bzv = fs::read(data_dir.join(format!("{prefix}.{ext}v"))).ok()?;
            let bzz = fs::read(data_dir.join(format!("{prefix}.{ext}z"))).ok()?;
            Some(Testament { bzs, bzv, bzz })
        };
        Ok(Commentary {
            entry_size,
            ot: load("ot"),
            nt: load("nt"),
            cache: RefCell::new(None),
        })
    }

    fn raw_entry(&self, pos: Position) -> Option<String> {
        let (nt, idx) = verse_index(pos)?;
        let t = if nt { self.nt.as_ref()? } else { self.ot.as_ref()? };
        let off = idx * self.entry_size;
        if off + self.entry_size > t.bzv.len() {
            return None;
        }
        let block = u32_at(&t.bzv, off);
        let start = u32_at(&t.bzv, off + 4) as usize;
        let size = if self.entry_size == 10 { u16_at(&t.bzv, off + 8) as usize } else { u32_at(&t.bzv, off + 8) as usize };
        if size == 0 {
            return None;
        }
        let mut cache = self.cache.borrow_mut();
        let need_load = match &*cache {
            Some((cnt, cb, _)) => *cnt != nt || *cb != block,
            None => true,
        };
        if need_load {
            let boff = block as usize * 12;
            if boff + 12 > t.bzs.len() {
                return None;
            }
            let zoff = u32_at(&t.bzs, boff) as usize;
            let zsize = u32_at(&t.bzs, boff + 4) as usize;
            let data = inflate(t.bzz.get(zoff..zoff + zsize)?).ok()?;
            *cache = Some((nt, block, data));
        }
        let data = &cache.as_ref().unwrap().2;
        let bytes = data.get(start..start + size)?;
        Some(String::from_utf8_lossy(bytes).into_owned())
    }

    /// Plain-text entry for a verse, falling back to the nearest preceding
    /// verse in the chapter that has one. Returns (text, verse the text is from).
    pub fn lookup(&self, pos: Position) -> Option<(String, u32)> {
        let mut v = pos.verse;
        while v >= 1 {
            let p = Position { verse: v, ..pos };
            if let Some(raw) = self.raw_entry(p) {
                let text = strip_markup(&raw);
                if !text.trim().is_empty() {
                    return Some((text, v));
                }
            }
            v -= 1;
        }
        None
    }

    /// Raw (marked-up) entry for exactly this verse.
    pub fn raw(&self, pos: Position) -> Option<String> {
        self.raw_entry(pos)
    }
}

fn find_conf(dir: &Path) -> io::Result<PathBuf> {
    let mods = dir.join("mods.d");
    for entry in fs::read_dir(&mods)?.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("conf") {
            return Ok(p);
        }
    }
    Err(io::Error::new(io::ErrorKind::NotFound, "no .conf in mods.d"))
}

/// A zLD or RawLD dictionary module.
pub struct Dictionary {
    keys: Vec<(String, u32, u32)>, // key, a, b  (zLD: block, entry;  RawLD: offset, size)
    zld: bool,
    dat: Vec<u8>,
    zdx: Vec<u8>,
    zdt: Vec<u8>,
    cache: RefCell<Option<(u32, Vec<u8>)>>,
}

impl Dictionary {
    pub fn open(dir: &Path) -> io::Result<Dictionary> {
        let conf_path = find_conf(dir)?;
        let conf = parse_conf(&fs::read_to_string(&conf_path)?);
        let drv = conf.get("ModDrv").cloned().unwrap_or_default();
        let zld = match drv.as_str() {
            "zLD" => true,
            "RawLD" | "RawLD4" => false,
            other => {
                return Err(io::Error::new(io::ErrorKind::Unsupported, format!("unsupported ModDrv {other}")));
            }
        };
        let data_path = conf.get("DataPath").cloned().unwrap_or_default();
        let base = dir.join(data_path.trim_start_matches("./"));
        let base_str = base.to_string_lossy().to_string();
        let idx = fs::read(format!("{base_str}.idx"))?;
        let dat = fs::read(format!("{base_str}.dat"))?;
        let (zdx, zdt) = if zld {
            (fs::read(format!("{base_str}.zdx"))?, fs::read(format!("{base_str}.zdt"))?)
        } else {
            (Vec::new(), Vec::new())
        };
        let raw4 = drv == "RawLD4";
        let idx_entry = if zld || raw4 { 8 } else { 6 };
        let mut keys = Vec::new();
        let mut i = 0;
        while i + idx_entry <= idx.len() {
            let off = u32_at(&idx, i) as usize;
            let size = if idx_entry == 8 { u32_at(&idx, i + 4) as usize } else { u16_at(&idx, i + 4) as usize };
            i += idx_entry;
            let Some(entry) = dat.get(off..off + size) else { continue };
            let nl = entry.iter().position(|&c| c == b'\n').unwrap_or(entry.len());
            let key = String::from_utf8_lossy(&entry[..nl]).trim_end_matches('\r').to_string();
            if key.is_empty() {
                continue;
            }
            if zld {
                let rest = &entry[(nl + 1).min(entry.len())..];
                if rest.len() >= 8 {
                    keys.push((key, u32_at(rest, 0), u32_at(rest, 4)));
                }
            } else {
                keys.push((key, off as u32 + nl as u32 + 1, (size.saturating_sub(nl + 1)) as u32));
            }
        }
        Ok(Dictionary {
            keys,
            zld,
            dat,
            zdx,
            zdt,
            cache: RefCell::new(None),
        })
    }


    /// Keys matching a query (case-insensitive substring; prefix matches first).
    pub fn search(&self, query: &str, limit: usize) -> Vec<String> {
        let q = query.trim().to_uppercase();
        if q.is_empty() {
            return Vec::new();
        }
        let mut prefix = Vec::new();
        let mut contains = Vec::new();
        for (k, _, _) in &self.keys {
            let ku = k.to_uppercase();
            if ku.starts_with(&q) {
                prefix.push(k.clone());
            } else if ku.contains(&q) {
                contains.push(k.clone());
            }
            if prefix.len() >= limit {
                break;
            }
        }
        prefix.extend(contains);
        prefix.truncate(limit);
        prefix
    }

    pub fn entry(&self, key: &str) -> Option<String> {
        let ku = key.to_uppercase();
        let (_, a, b) = self.keys.iter().find(|(k, _, _)| k.to_uppercase() == ku)?;
        let raw = if self.zld {
            let block = *a;
            let entry = *b as usize;
            let mut cache = self.cache.borrow_mut();
            if cache.as_ref().map(|(bl, _)| *bl != block).unwrap_or(true) {
                let offset = (block as usize).checked_mul(8)?;
                self.zdx.get(offset..offset.checked_add(8)?)?;
                let zo = u32_at(&self.zdx, offset) as usize;
                let zs = u32_at(&self.zdx, offset + 4) as usize;
                let data = inflate(self.zdt.get(zo..zo + zs)?).ok()?;
                *cache = Some((block, data));
            }
            let data = &cache.as_ref().unwrap().1;
            data.get(..4)?;
            let count = u32_at(data, 0) as usize;
            data.get(..4usize.checked_add(count.checked_mul(8)?)?)?;
            if entry >= count {
                return None;
            }
            let eo = u32_at(data, 4 + entry * 8) as usize;
            let es = u32_at(data, 4 + entry * 8 + 4) as usize;
            String::from_utf8_lossy(data.get(eo..eo + es)?).into_owned()
        } else {
            String::from_utf8_lossy(self.dat.get(*a as usize..(*a as usize).checked_add(*b as usize)?)?).into_owned()
        };
        Some(strip_markup(&raw))
    }
}

/// Reduce ThML / OSIS / TEI markup to readable plain text.
pub fn strip_markup(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            let mut tag = String::new();
            for t in chars.by_ref() {
                if t == '>' {
                    break;
                }
                tag.push(t);
            }
            let name = tag.trim_start_matches('/').split_whitespace().next().unwrap_or("").to_ascii_lowercase();
            let closing = tag.starts_with('/');
            match name.as_str() {
                "br" | "lb" | "l" => out.push('\n'),
                "p" | "div" | "list" | "item" | "lg" | "milestone" => {
                    if closing || name == "milestone" || (!out.ends_with('\n') && !out.is_empty()) {
                        out.push('\n');
                    }
                }
                "title" | "head" => {
                    if closing || (!out.is_empty() && !out.ends_with('\n')) {
                        out.push('\n');
                    }
                }
                "verse" if !closing => {
                    if let Some(n) = attr(&tag, "n") {
                        out.push_str(&format!("[{n}] "));
                    }
                }
                "note" if !closing => out.push_str(" ("),
                "note" => out.push(')'),
                _ => {}
            }
        } else if c == '&' {
            let mut ent = String::new();
            let mut ok = false;
            while let Some(&n) = chars.peek() {
                chars.next();
                if n == ';' {
                    ok = true;
                    break;
                }
                ent.push(n);
                if ent.len() > 8 {
                    break;
                }
            }
            if ok {
                out.push_str(&decode_entity(&ent));
            } else {
                out.push('&');
                out.push_str(&ent);
            }
        } else {
            out.push(c);
        }
    }
    // Collapse whitespace but keep paragraph breaks.
    let mut result = String::with_capacity(out.len());
    let mut blank = 0;
    for line in out.lines() {
        let l = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if l.is_empty() {
            blank += 1;
            if blank == 1 && !result.is_empty() {
                result.push('\n');
            }
        } else {
            blank = 0;
            result.push_str(&l);
            result.push('\n');
        }
    }
    result.trim().to_string()
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let pat = format!("{name}=\"");
    let start = tag.find(&pat)? + pat.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

fn decode_entity(e: &str) -> String {
    match e {
        "amp" => "&".into(),
        "lt" => "<".into(),
        "gt" => ">".into(),
        "quot" => "\"".into(),
        "apos" => "'".into(),
        "nbsp" => " ".into(),
        "mdash" => "—".into(),
        "ndash" => "–".into(),
        _ => {
            if let Some(num) = e.strip_prefix('#') {
                let code = if let Some(hex) = num.strip_prefix('x').or_else(|| num.strip_prefix('X')) {
                    u32::from_str_radix(hex, 16).ok()
                } else {
                    num.parse::<u32>().ok()
                };
                if let Some(ch) = code.and_then(char::from_u32) {
                    return ch.to_string();
                }
            }
            format!("&{e};")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damaged_dictionary_blocks_return_none_instead_of_panicking() {
        let mut dictionary = Dictionary { keys: vec![("word".into(), u32::MAX, 0)], zld: true, dat: vec![], zdx: vec![], zdt: vec![], cache: RefCell::new(None) };
        assert!(dictionary.entry("word").is_none());
        dictionary.keys[0].1 = 0;
        *dictionary.cache.borrow_mut() = Some((0, vec![1, 0, 0, 0]));
        assert!(dictionary.entry("word").is_none());
        *dictionary.cache.borrow_mut() = Some((0, vec![]));
        assert!(dictionary.entry("word").is_none());
        dictionary.zld = false;
        dictionary.keys[0].1 = u32::MAX;
        dictionary.keys[0].2 = u32::MAX;
        assert!(dictionary.entry("word").is_none());
    }

    #[test]
    fn indexes_match_layout() {
        // Genesis 1:1 is index 4 in the OT file: [module][testament][book][chapter] then verse 1.
        assert_eq!(verse_index(Position { book: 1, chapter: 1, verse: 1 }), Some((false, 4)));
        // Matthew 1:1 likewise in the NT file.
        assert_eq!(verse_index(Position { book: 40, chapter: 1, verse: 1 }), Some((true, 4)));
        // John 3:16 verified against Clarke, JFB and TSK: 3068.
        assert_eq!(verse_index(Position { book: 43, chapter: 3, verse: 16 }), Some((true, 3068)));
        assert_eq!(verse_index(Position { book: 1, chapter: 1, verse: 99 }), None);
    }

    #[test]
    fn strips_markup() {
        let s = strip_markup("<p>Hello <b>world</b>&amp; friends<br />next</p><scripRef>Joh 3:16</scripRef>");
        assert_eq!(s, "Hello world& friends\nnext\nJoh 3:16");
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    /// Diagnostic: `cargo test dump_commentary -- --ignored --nocapture` with packs installed.
    #[test]
    #[ignore]
    fn dump_commentary_entries() {
        for id in ["clarke", "barnes", "mhc", "jfb", "tsk"] {
            let dir = crate::resources::pack_dir(id);
            if !dir.exists() {
                continue;
            }
            let c = Commentary::open(&dir).unwrap();
            for (b, ch, v) in [(45, 8, 28), (43, 3, 16), (1, 1, 1), (1, 1, 2), (39, 4, 6), (40, 1, 1), (66, 22, 21), (19, 119, 176), (2, 1, 1)] {
                let p = Position { book: b, chapter: ch, verse: v };
                let raw = c.raw(p).map(|s| strip_markup(&s).chars().take(60).collect::<String>().replace('\n', " "));
                println!("{id:>7} {b}:{ch}:{v}: {:?}", raw);
            }
        }
    }
}
