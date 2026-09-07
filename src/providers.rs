//! Online Bible sources. Only the currently displayed chapter is kept in memory;
//! provider text and credentials never enter the offline translation store.
use crate::bible::{Book, Chapter, Translation, TranslationInfo, Verse};
use crate::v11n;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::PathBuf, time::Duration};

pub const NAMES: [&str; 66] = [
    "Genesis",
    "Exodus",
    "Leviticus",
    "Numbers",
    "Deuteronomy",
    "Joshua",
    "Judges",
    "Ruth",
    "1 Samuel",
    "2 Samuel",
    "1 Kings",
    "2 Kings",
    "1 Chronicles",
    "2 Chronicles",
    "Ezra",
    "Nehemiah",
    "Esther",
    "Job",
    "Psalms",
    "Proverbs",
    "Ecclesiastes",
    "Song of Solomon",
    "Isaiah",
    "Jeremiah",
    "Lamentations",
    "Ezekiel",
    "Daniel",
    "Hosea",
    "Joel",
    "Amos",
    "Obadiah",
    "Jonah",
    "Micah",
    "Nahum",
    "Habakkuk",
    "Zephaniah",
    "Haggai",
    "Zechariah",
    "Malachi",
    "Matthew",
    "Mark",
    "Luke",
    "John",
    "Acts",
    "Romans",
    "1 Corinthians",
    "2 Corinthians",
    "Galatians",
    "Ephesians",
    "Philippians",
    "Colossians",
    "1 Thessalonians",
    "2 Thessalonians",
    "1 Timothy",
    "2 Timothy",
    "Titus",
    "Philemon",
    "Hebrews",
    "James",
    "1 Peter",
    "2 Peter",
    "1 John",
    "2 John",
    "3 John",
    "Jude",
    "Revelation",
];
const USFM: [&str; 66] = [
    "GEN", "EXO", "LEV", "NUM", "DEU", "JOS", "JDG", "RUT", "1SA", "2SA", "1KI", "2KI", "1CH",
    "2CH", "EZR", "NEH", "EST", "JOB", "PSA", "PRO", "ECC", "SNG", "ISA", "JER", "LAM", "EZK",
    "DAN", "HOS", "JOL", "AMO", "OBA", "JON", "MIC", "NAM", "HAB", "ZEP", "HAG", "ZEC", "MAL",
    "MAT", "MRK", "LUK", "JHN", "ACT", "ROM", "1CO", "2CO", "GAL", "EPH", "PHP", "COL", "1TH",
    "2TH", "1TI", "2TI", "TIT", "PHM", "HEB", "JAS", "1PE", "2PE", "1JN", "2JN", "3JN", "JUD",
    "REV",
];

#[derive(Clone, Debug, Default)]
pub struct Online {
    pub loaded: Option<(usize, usize)>,
    pub error: Option<String>,
    pub notice: String,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub esv_key: String,
    pub nlt_key: String,
    pub api_bible_key: String,
    pub api_bible: Vec<ApiBible>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApiBible {
    pub id: String,
    pub name: String,
}

/// Replace provider settings atomically, with owner-only permissions.
pub fn save_config(config: &Config) -> Result<(), String> {
    write_config(config, &config_path())
}

fn write_config(config: &Config, path: &std::path::Path) -> Result<(), String> {
    if config
        .api_bible
        .iter()
        .any(|b| b.id.is_empty() || !b.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
    {
        return Err("Bible IDs can only contain letters, numbers and hyphens".into());
    }
    let bytes = serde_json::to_vec_pretty(config).map_err(|_| "Cannot encode provider settings")?;
    crate::storage::atomic_write(path, &bytes).map_err(|_| "Cannot save provider settings".into())
}

pub fn config_path() -> PathBuf {
    std::env::var_os("OMASCRIPTURE_PROVIDERS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("omascripture/providers.json")
        })
}
pub fn config() -> Result<Config, String> {
    let mut c: Config = match fs::read_to_string(config_path()) {
        Ok(s) => serde_json::from_str(&s).map_err(|_| {
            format!(
                "Invalid provider config: {} (check README format)",
                config_path().display()
            )
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
        Err(_) => return Err(format!("Cannot read {}", config_path().display())),
    };
    for (name, value) in [
        ("OMASCRIPTURE_ESV_KEY", &mut c.esv_key),
        ("OMASCRIPTURE_NLT_KEY", &mut c.nlt_key),
        ("OMASCRIPTURE_API_BIBLE_KEY", &mut c.api_bible_key),
    ] {
        if let Ok(s) = std::env::var(name) {
            *value = s;
        }
        *value = value.trim().to_string();
    }
    if c.api_bible
        .iter()
        .any(|b| b.id.is_empty() || !b.id.chars().all(|x| x.is_ascii_alphanumeric() || x == '-'))
    {
        return Err("API.Bible IDs must contain only letters, numbers and hyphens".into());
    }
    Ok(c)
}
pub fn is_online(id: &str) -> bool {
    matches!(id, "net" | "esv" | "nlt") || id.starts_with("api-bible-")
}

pub fn status(id: &str) -> String {
    let Ok(c) = config() else {
        return "config error".into();
    };
    match id {
        "net" => "online · no key".into(),
        "nlt" if c.nlt_key.is_empty() => "online · anonymous".into(),
        "esv" if c.esv_key.is_empty() => "needs ESV key".into(),
        x if x.starts_with("api-bible-") && c.api_bible_key.is_empty() => {
            "needs API.Bible key".into()
        }
        _ => "online · key set".into(),
    }
}
fn info(id: &str, name: &str) -> TranslationInfo {
    TranslationInfo {
        abbreviation: id.into(),
        translation: name.into(),
        lang: "en".into(),
        language: "Online".into(),
        direction: String::new(),
        description: status(id),
        distribution_license: "Provider terms apply; internet required".into(),
    }
}
pub fn catalog() -> Vec<TranslationInfo> {
    let mut out = vec![
        info("net", "NET Bible"),
        info("esv", "English Standard Version"),
        info("nlt", "New Living Translation"),
    ];
    if let Ok(c) = config() {
        out.extend(
            c.api_bible
                .iter()
                .map(|b| info(&format!("api-bible-{}", b.id), &b.name)),
        );
    }
    out
}

/// Discover only Bible editions this API key is authorized to access.
pub fn available_api_bibles() -> Result<Vec<(String, String)>, String> {
    let c = config()?;
    if c.api_bible_key.is_empty() {
        return Err("Add your API.Bible key in Settings before discovering editions.".into());
    }
    let body = request(
        "https://rest.api.bible/v1/bibles",
        &[],
        Some(("api-key", c.api_bible_key)),
    )?;
    let v = json(&body)?;
    let mut out = Vec::new();
    for b in v["data"]
        .as_array()
        .ok_or("Missing authorized Bible list")?
    {
        if let (Some(id), Some(name)) = (b["id"].as_str(), b["name"].as_str()) {
            out.push((id.into(), name.into()));
        }
    }
    Ok(out)
}

pub fn init_config() -> Result<PathBuf, String> {
    use std::io::Write;
    #[cfg(unix)]
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let path = config_path();
    let parent = path.parent().ok_or("Invalid provider config path")?;
    if !parent.exists() {
        fs::create_dir_all(parent).map_err(|_| "Cannot create provider config directory")?;
        #[cfg(unix)]
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Cannot protect provider config directory")?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            "Provider config already exists; left unchanged"
        } else {
            "Cannot create provider config"
        }
    })?;
    file.write_all(b"{\n  \"esv_key\": \"\",\n  \"nlt_key\": \"\",\n  \"api_bible_key\": \"\",\n  \"api_bible\": []\n}\n").map_err(|_| "Cannot write provider config")?;
    Ok(path)
}

/// Canonical coordinates exist before the text arrives, so references and plans
/// can navigate to any chapter without downloading intervening chapters.
pub fn open(id: &str) -> Result<Translation, String> {
    let c = config()?;
    if id == "esv" && c.esv_key.is_empty() {
        return Err("Add your ESV API key in Settings before opening this Bible.".into());
    }
    if id.starts_with("api-bible-") && c.api_bible_key.is_empty() {
        return Err("Add your API.Bible key in Settings before opening this Bible.".into());
    }
    let i = catalog()
        .into_iter()
        .find(|i| i.abbreviation == id)
        .ok_or("Unknown online translation; configure its API.Bible ID first")?;
    Ok(Translation {
        translation: i.translation,
        abbreviation: id.into(),
        lang: "en".into(),
        language: "English".into(),
        description: i.description,
        distribution_license: i.distribution_license,
        online: Some(Online::default()),
        books: NAMES
            .iter()
            .enumerate()
            .map(|(b, name)| Book {
                nr: b as u32 + 1,
                name: (*name).into(),
                chapters: v11n::VERSES[b]
                    .iter()
                    .enumerate()
                    .map(|(ch, _)| placeholder(b, ch))
                    .collect(),
            })
            .collect(),
    })
}
pub fn placeholder(book: usize, chapter: usize) -> Chapter {
    Chapter {
        chapter: chapter as u32 + 1,
        verses: (1..=v11n::VERSES[book][chapter] as u32)
            .map(|verse| Verse {
                verse,
                text: String::new(),
            })
            .collect(),
    }
}

pub struct Passage {
    pub verses: Vec<Verse>,
    pub notice: String,
}
fn request(
    url: &str,
    query: &[(&str, String)],
    header: Option<(&str, String)>,
) -> Result<String, String> {
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(25)))
            .https_only(true)
            .build(),
    );
    let mut req = agent.get(url);
    for (k, v) in query {
        req = req.query(k, v);
    }
    if let Some((k, v)) = header {
        req = req.header(k, v);
    }
    // Never format a request/error URL: NLT credentials are query parameters.
    let mut response = req.call().map_err(|e| match e {
        ureq::Error::StatusCode(401 | 403) => {
            "Provider denied access; check your key and translation permissions".to_string()
        }
        ureq::Error::StatusCode(429) => {
            "Provider rate limit reached. Please wait before trying again.".to_string()
        }
        ureq::Error::StatusCode(s) => format!("Provider returned HTTP {s}. Please try again."),
        _ => "Could not reach provider. Check your connection and try again.".to_string(),
    })?;
    response
        .body_mut()
        .with_config()
        .limit(4 * 1024 * 1024)
        .read_to_string()
        .map_err(|_| "Could not read provider response".into())
}
fn json(s: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(s).map_err(|_| "Provider returned invalid JSON".into())
}
fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn fetch(id: &str, book: usize, chapter: usize) -> Result<Passage, String> {
    if book >= 66 || chapter >= v11n::VERSES[book].len() {
        return Err("Invalid chapter".into());
    }
    let c = config()?;
    let reference = format!("{} {}", NAMES[book], chapter + 1);
    let passage = match id {
        "net" => {
            let body = request(
                "https://labs.bible.org/api/",
                &[
                    ("passage", reference),
                    ("type", "json".into()),
                    ("formatting", "plain".into()),
                ],
                None,
            )?;
            let verses = parse_net(&body, chapter as u32 + 1)?;
            Passage {
                verses,
                notice: "NET Bible® · © Biblical Studies Press · https://netbible.com/copyright/"
                    .into(),
            }
        }
        "esv" => {
            if c.esv_key.is_empty() {
                return Err("Add your ESV API key in Settings first.".into());
            }
            let body = request(
                "https://api.esv.org/v3/passage/text/",
                &[
                    ("q", reference),
                    ("include-passage-references", "false".into()),
                    ("include-headings", "false".into()),
                    ("include-footnotes", "false".into()),
                    ("include-verse-numbers", "true".into()),
                    ("include-first-verse-numbers", "true".into()),
                ],
                Some(("Authorization", format!("Token {}", c.esv_key))),
            )?;
            let v = json(&body)?;
            let text = v["passages"]
                .as_array()
                .ok_or("ESV response has no passages")?
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            Passage { verses: parse_numbered(&text)?, notice: "ESV® · © 2001 Crossway · Used by permission. All rights reserved. https://www.esv.org".into() }
        }
        "nlt" => {
            let mut verses = Vec::new();
            let count = v11n::VERSES[book][chapter] as u32;
            // Anonymous requests are limited to 50 verses, including long psalms.
            for start in (1..=count).step_by(50) {
                let end = (start + 49).min(count);
                let body = request(
                    "https://api.nlt.to/api/passages",
                    &[
                        (
                            "ref",
                            format!("{}.{}.{}-{}", NAMES[book], chapter + 1, start, end),
                        ),
                        ("version", "NLT".into()),
                        (
                            "key",
                            if c.nlt_key.is_empty() {
                                "TEST".into()
                            } else {
                                c.nlt_key.clone()
                            },
                        ),
                    ],
                    None,
                )?;
                verses.extend(parse_nlt(&body, chapter as u32 + 1)?);
            }
            Passage { verses, notice: "NLT · © 1996, 2004, 2015 Tyndale House Foundation. Used by permission. https://www.tyndale.com/permissions".into() }
        }
        _ => {
            let bible_id = id.strip_prefix("api-bible-").ok_or("Unknown provider")?;
            if c.api_bible_key.is_empty() || !c.api_bible.iter().any(|b| b.id == bible_id) {
                return Err("Add your API.Bible key and Bible ID in Settings first.".into());
            }
            let url = format!(
                "https://rest.api.bible/v1/bibles/{bible_id}/chapters/{}.{}",
                USFM[book],
                chapter + 1
            );
            let body = request(
                &url,
                &[
                    ("content-type", "text".into()),
                    ("include-titles", "false".into()),
                    ("include-notes", "false".into()),
                    ("include-chapter-numbers", "false".into()),
                    ("include-verse-numbers", "true".into()),
                ],
                Some(("api-key", c.api_bible_key)),
            )?;
            let v = json(&body)?;
            Passage {
                verses: parse_numbered(
                    v["data"]["content"]
                        .as_str()
                        .ok_or("Missing chapter text")?,
                )?,
                notice: crate::sword::strip_markup(
                    v["data"]["copyright"]
                        .as_str()
                        .ok_or("Missing provider copyright notice")?,
                ),
            }
        }
    };
    validate(&passage.verses)?;
    Ok(passage)
}
fn validate(verses: &[Verse]) -> Result<(), String> {
    if verses.is_empty()
        || verses.iter().any(|v| v.verse == 0 || v.text.is_empty())
        || verses.windows(2).any(|w| w[0].verse >= w[1].verse)
    {
        return Err("Provider returned empty or invalid verses. Please try again.".into());
    }
    Ok(())
}
fn parse_net(body: &str, chapter: u32) -> Result<Vec<Verse>, String> {
    let value = json(body)?;
    value
        .as_array()
        .ok_or("NET response is not a verse list")?
        .iter()
        .map(|v| {
            if v["chapter"].as_str().and_then(|s| s.parse::<u32>().ok()) != Some(chapter) {
                return Err("NET returned a different chapter".into());
            }
            Ok(Verse {
                verse: v["verse"]
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .ok_or("Invalid NET verse number")?,
                text: clean(v["text"].as_str().ok_or("Missing NET verse text")?),
            })
        })
        .collect()
}
/// ESV and API.Bible text responses delimit verses using bracketed numbers.
fn parse_numbered(text: &str) -> Result<Vec<Verse>, String> {
    let mut markers = Vec::new();
    for (offset, _) in text.match_indices('[') {
        if let Some(end) = text[offset..].find(']')
            && let Ok(n) = text[offset + 1..offset + end].parse::<u32>()
        {
            markers.push((offset, offset + end + 1, n));
        }
    }
    let verses = markers
        .iter()
        .enumerate()
        .map(|(i, &(_, start, verse))| Verse {
            verse,
            text: clean(&text[start..markers.get(i + 1).map(|m| m.0).unwrap_or(text.len())]),
        })
        .collect::<Vec<_>>();
    validate(&verses)?;
    Ok(verses)
}
fn parse_nlt(body: &str, chapter: u32) -> Result<Vec<Verse>, String> {
    let mut out = BTreeMap::new();
    // Split before HTML parsing: NLT's paragraph tags can cross verse_export
    // boundaries, which HTML tree repair otherwise nests unpredictably.
    for part in body.split("<verse_export ").skip(1) {
        let (attrs, rest) = part.split_once('>').ok_or("Invalid NLT verse wrapper")?;
        let attr = |key: &str| {
            attrs
                .split_once(&format!("{key}=\""))
                .and_then(|(_, s)| s.split('"').next())
        };
        if attr("ch").and_then(|s| s.parse::<u32>().ok()) != Some(chapter) {
            return Err("NLT returned a different chapter".into());
        }
        let verse = attr("vn")
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or("Invalid NLT verse number")?;
        let fragment =
            scraper::Html::parse_fragment(rest.split("</verse_export>").next().unwrap_or(rest));
        let mut text = String::new();
        for node in fragment.tree.nodes() {
            if let Some(t) = node.value().as_text() {
                let skip = node.ancestors().any(|n| {
                    n.value().as_element().is_some_and(|e| {
                        e.classes()
                            .any(|c| matches!(c, "vn" | "cn" | "tn" | "a-tn" | "bk_ch_vs_header"))
                            || matches!(e.name(), "h1" | "h2" | "h3" | "script" | "style")
                    })
                });
                if !skip {
                    text.push_str(t);
                }
            }
        }
        if out.insert(verse, clean(&text)).is_some() {
            return Err("Duplicate NLT verse".into());
        }
    }
    let verses = out
        .into_iter()
        .map(|(verse, text)| Verse { verse, text })
        .collect::<Vec<_>>();
    validate(&verses)?;
    Ok(verses)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn graphical_key_save_is_private_and_validated() {
        use std::os::unix::fs::PermissionsExt;
        let root =
            std::env::temp_dir().join(format!("omascripture-keys-test-{}", std::process::id()));
        let path = root.join("providers.json");
        let config = Config {
            esv_key: "example-key-for-test".into(),
            ..Default::default()
        };
        write_config(&config, &path).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let saved: Config = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved.esv_key, "example-key-for-test");
        let mut bad = config;
        bad.api_bible.push(ApiBible {
            id: "../invalid".into(),
            name: "Bad ID".into(),
        });
        assert!(write_config(&bad, &path).is_err());
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .contains("example-key-for-test")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn numbered_preserves_omitted_verse_numbers_and_attribution() {
        let v = parse_numbered("[16] First line\nsecond line [18] Last. (ESV)").unwrap();
        assert_eq!(v[0].text, "First line second line");
        assert_eq!(v[1].verse, 18);
        assert!(v[1].text.ends_with("(ESV)"));
        assert!(parse_numbered("Rate limit exceeded").is_err());
        assert!(parse_numbered("[1] One [1] Duplicate").is_err());
    }
    #[test]
    fn nlt_handles_crossing_paragraphs_and_removes_notes() {
        let v = parse_nlt(r#"<verse_export ch="3" vn="16"><p><span class="vn">16</span><span class="red">Word<a class="a-tn">*</a><span class="tn">Note <em>detail</em></span> &amp; word.</span></verse_export><verse_export ch="3" vn="17"><span class="vn">17</span>Next.</p></verse_export>"#, 3).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].text, "Word & word.");
        assert_eq!(v[1].text, "Next.");
        assert!(parse_nlt("Unauthorized", 3).is_err());
    }
    #[test]
    fn net_rejects_wrong_chapter() {
        assert!(parse_net(r#"[{"chapter":"2","verse":"1","text":"Test"}]"#, 3).is_err());
    }
    #[test]
    fn online_coordinates_work_without_downloads() {
        let t = open("net").unwrap();
        let l = t.parse_reference("John 3:16").unwrap();
        assert_eq!(t.position_from_loc(l).verse, 16);
        assert_eq!(t.books.len(), 66);
        assert_eq!(t.books[18].chapters[118].verses.len(), 176);
    }
    #[test]
    #[ignore = "live provider network check"]
    fn live_public_providers() {
        for id in ["net", "nlt"] {
            for (book, chapter, count) in [(42, 2, 36), (42, 0, 51), (18, 118, 176)] {
                let p = fetch(id, book, chapter).unwrap();
                assert_eq!(p.verses.len(), count, "{id} {book}:{chapter}");
                assert_eq!(p.verses[15].verse, 16);
                assert!(!p.verses[15].text.contains("<span"));
            }
        }
    }
}
