//! Verified phrase alignments and language reference tables.
use crate::{
    bible::Position,
    resources::{self, Word},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaggedToken {
    pub text: String,
    pub phrase: String,
    pub strongs: Vec<String>,
    #[serde(default)]
    pub morphs: Vec<String>,
}
type TagPiece = (std::ops::Range<usize>, Vec<String>, Vec<String>);
pub type TaggedBook = BTreeMap<String, Vec<TaggedToken>>;

/// Character ranges, so egui's Unicode cursor offsets never become byte indexes.
pub fn tokens(text: &str) -> Vec<(std::ops::Range<usize>, String)> {
    let chars: Vec<char> = text.chars().collect();
    let mut result = Vec::new();
    let mut start = None;
    for (i, c) in chars
        .iter()
        .copied()
        .chain(std::iter::once(' '))
        .enumerate()
    {
        let word = c.is_alphanumeric()
            || (matches!(c, '\'' | '’' | '-')
                && start.is_some()
                && chars.get(i + 1).is_some_and(|c| c.is_alphanumeric()));
        if word {
            start.get_or_insert(i);
        } else if let Some(begin) = start.take() {
            result.push((begin..i, chars[begin..i].iter().collect()));
        }
    }
    result
}
fn normalized(text: &str) -> String {
    text.to_lowercase().replace('’', "'")
}

pub fn import_kjv(xml: &[u8], destination: &Path) -> Result<(), String> {
    use quick_xml::{Reader, events::Event};
    let mut reader = Reader::from_reader(xml);
    let mut books: BTreeMap<u32, TaggedBook> = BTreeMap::new();
    let mut position = None;
    let mut verse_text = String::new();
    let mut pieces: Vec<TagPiece> = Vec::new();
    let mut word = None;
    let mut skip = 0usize;
    fn finish(
        position: &mut Option<Position>,
        text: &mut String,
        pieces: &mut Vec<TagPiece>,
        books: &mut BTreeMap<u32, TaggedBook>,
    ) {
        if let Some(p) = position.take() {
            let all = tokens(text);
            let tagged = all
                .into_iter()
                .map(|(range, text_token)| {
                    let (phrase, strongs, morphs) = pieces
                        .iter()
                        .find(|(span, _, _)| span.start <= range.start && span.end >= range.end)
                        .map(|(span, ids, morphs)| {
                            (
                                text.chars().skip(span.start).take(span.len()).collect(),
                                ids.clone(),
                                morphs.clone(),
                            )
                        })
                        .unwrap_or_default();
                    TaggedToken {
                        text: text_token,
                        phrase,
                        strongs,
                        morphs,
                    }
                })
                .collect();
            books
                .entry(p.book)
                .or_default()
                .insert(format!("{}:{}", p.chapter, p.verse), tagged);
        }
        text.clear();
        pieces.clear();
    }
    loop {
        let event = reader.read_event().map_err(|e| e.to_string())?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let name = e.local_name();
                let name = name.as_ref();
                let empty = matches!(event, Event::Empty(_));
                if skip > 0 {
                    if !empty {
                        skip += 1;
                    }
                    continue;
                }
                if matches!(name, b"note" | b"title" | b"header") {
                    if !empty {
                        skip = 1;
                    }
                    continue;
                }
                let attr = |key: &[u8]| -> Option<String> {
                    e.attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == key)
                        .and_then(|a| {
                            a.decode_and_unescape_value(reader.decoder())
                                .ok()
                                .map(|v| v.into_owned())
                        })
                };
                if name == b"verse" {
                    finish(&mut position, &mut verse_text, &mut pieces, &mut books);
                    position = attr(b"osisID").and_then(|s| resources::parse_osis_ref(&s));
                    word = None;
                } else if name == b"w" && position.is_some() && !empty {
                    let strongs = attr(b"lemma")
                        .unwrap_or_default()
                        .split_whitespace()
                        .filter_map(|s| s.strip_prefix("strong:"))
                        .map(resources::base_strong)
                        .collect();
                    let morphs = attr(b"morph")
                        .unwrap_or_default()
                        .split_whitespace()
                        .filter_map(|s| s.strip_prefix("robinson:"))
                        .map(str::to_string)
                        .collect();
                    word = Some((verse_text.chars().count(), strongs, morphs));
                }
            }
            Event::End(e) => {
                if skip > 0 {
                    skip -= 1;
                    continue;
                }
                if e.local_name().as_ref() == b"w"
                    && let Some((start, ids, morphs)) = word.take()
                {
                    pieces.push((start..verse_text.chars().count(), ids, morphs));
                } else if e.local_name().as_ref() == b"verse" {
                    finish(&mut position, &mut verse_text, &mut pieces, &mut books);
                }
            }
            Event::Text(e) if skip == 0 && position.is_some() => {
                let text = e.decode().map_err(|e| e.to_string())?;
                verse_text
                    .push_str(&quick_xml::escape::unescape(&text).map_err(|e| e.to_string())?);
            }
            Event::GeneralRef(e) if skip == 0 && position.is_some() => {
                let name = e.decode().map_err(|e| e.to_string())?;
                verse_text.push_str(
                    &quick_xml::escape::unescape(&format!("&{name};"))
                        .map_err(|e| e.to_string())?,
                );
            }
            Event::Eof => break,
            _ => {}
        }
    }
    finish(&mut position, &mut verse_text, &mut pieces, &mut books);
    if books.is_empty() {
        return Err("No tagged Bible verses found".into());
    }
    for (book, verses) in books {
        crate::storage::atomic_write(
            &destination.join(format!("{book}.json")),
            &serde_json::to_vec(&verses).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub struct LanguageData {
    root: PathBuf,
    books: BTreeMap<u32, TaggedBook>,
    morphology: Option<BTreeMap<String, String>>,
    installed: bool,
    generation: Option<std::time::SystemTime>,
    last_book: Option<u32>,
    aligned_verse: Option<AlignedVerse>,
}

struct AlignedVerse {
    position: Position,
    text: String,
    tokens: Option<Vec<TaggedToken>>,
}
impl Default for LanguageData {
    fn default() -> Self {
        Self {
            root: resources::pack_dir("word-study"),
            books: BTreeMap::new(),
            morphology: None,
            installed: false,
            generation: None,
            last_book: None,
            aligned_verse: None,
        }
    }
}
impl LanguageData {
    #[cfg(test)]
    pub(crate) fn test_root(root: PathBuf) -> Self {
        Self {
            root,
            ..Self::default()
        }
    }

    pub fn refresh(&mut self) {
        let metadata = fs::metadata(self.root.join("installed.json")).ok();
        let installed = metadata.is_some();
        let generation = metadata.and_then(|m| m.modified().ok());
        if installed != self.installed || generation != self.generation {
            self.books.clear();
            self.last_book = None;
            self.aligned_verse = None;
            self.morphology = None;
            self.installed = installed;
            self.generation = generation;
        }
    }
    pub fn aligned(
        &mut self,
        translation: &str,
        pos: Position,
        text: &str,
        index: usize,
    ) -> Option<TaggedToken> {
        self.refresh();
        if translation != "kjv" || !self.installed {
            return None;
        }
        if let Some(cached) = &self.aligned_verse
            && cached.position == pos
            && cached.text == text
        {
            return cached.tokens.as_ref()?.get(index).cloned();
        }
        if self.last_book != Some(pos.book) {
            self.books
                .retain(|key, _| *key == pos.book || Some(*key) == self.last_book);
            self.last_book = Some(pos.book);
        }
        let book = self.books.entry(pos.book).or_insert_with(|| {
            fs::read(self.root.join(format!("{}.json", pos.book)))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default()
        });
        let tagged = book.get(&format!("{}:{}", pos.chapter, pos.verse))?;
        let actual = tokens(text);
        // Alignment is valid only if the entire verse's word sequence agrees.
        let matches = tagged.len() == actual.len()
            && tagged
                .iter()
                .zip(&actual)
                .all(|(a, (_, b))| normalized(&a.text) == normalized(b));
        let matched = matches.then(|| tagged.clone());
        let token = matched
            .as_ref()
            .and_then(|tokens| tokens.get(index))
            .cloned();
        self.aligned_verse = Some(AlignedVerse {
            position: pos,
            text: text.into(),
            tokens: matched,
        });
        token
    }
    pub fn morphology(&mut self, code: &str) -> String {
        self.refresh();
        let table = self.morphology.get_or_insert_with(|| {
            let mut result = BTreeMap::new();
            for file in ["greek.tsv", "hebrew.tsv"] {
                if let Ok(text) = fs::read_to_string(self.root.join(file)) {
                    for line in text.lines() {
                        let mut c = line.split('\t');
                        if let (Some(code), Some(description)) = (c.next(), c.next())
                            && description.contains("Function=")
                        {
                            result.insert(code.trim().into(), description.trim().into());
                        }
                    }
                }
            }
            result
        });
        code.split('/')
            .map(|part| {
                let key = if code.starts_with('H') && !part.starts_with('H') {
                    format!("H{part}")
                } else if code.starts_with('A') && !part.starts_with('A') {
                    format!("A{part}")
                } else {
                    part.into()
                };
                table.get(&key).cloned().unwrap_or(key)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Clone)]
pub struct WordChoice {
    pub position: Position,
    pub text: String,
    pub phrase: String,
    pub aligned: bool,
    pub words: Vec<Word>,
}

#[derive(Default, Debug)]
pub struct WordReport {
    pub total: usize,
    pub verse_count: usize,
    pub loaded_books: usize,
    pub book_counts: BTreeMap<u32, usize>,
    pub forms: BTreeMap<String, usize>,
    pub verses: Vec<(Position, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alignment_cache_is_bounded_and_invalidated_on_resource_removal() {
        let dir = crate::storage::TestDir::new();
        import_kjv(
            br#"<osis><verse osisID="John.3.16"><w lemma="strong:G25">loved</w></verse></osis>"#,
            &dir.0,
        )
        .unwrap();
        for book in 1..=3 {
            fs::copy(dir.0.join("43.json"), dir.0.join(format!("{book}.json"))).unwrap();
        }
        fs::write(dir.0.join("installed.json"), "{}").unwrap();
        let mut data = LanguageData::test_root(dir.0.clone());
        for book in 1..=3 {
            let pos = Position {
                book,
                chapter: 3,
                verse: 16,
            };
            assert!(data.aligned("kjv", pos, "loved", 0).is_some());
            assert!(data.books.len() <= 2);
            // A changed edition/verse must not reuse the cached alignment.
            assert!(data.aligned("nlt", pos, "loved", 0).is_none());
            assert!(data.aligned("kjv", pos, "different", 0).is_none());
            assert!(data.aligned("kjv", pos, "loved", 0).is_some());
        }
        fs::remove_file(dir.0.join("installed.json")).unwrap();
        assert!(
            data.aligned(
                "kjv",
                Position {
                    book: 3,
                    chapter: 3,
                    verse: 16
                },
                "loved",
                0
            )
            .is_none()
        );
        assert!(data.books.is_empty());
        assert!(data.aligned_verse.is_none());
    }

    #[test]
    fn alignment_requires_matching_translation_and_complete_verse() {
        let dir = crate::storage::TestDir::new();
        import_kjv(br#"<osis><verse osisID="John.3.16"><w lemma="strong:G25">loved</w> <w lemma="strong:G2316">God</w></verse></osis>"#, &dir.0).unwrap();
        fs::write(dir.0.join("installed.json"), "{}").unwrap();
        fs::write(
            dir.0.join("greek.tsv"),
            "V-AAI-3S\tFunction=Verb; Tense=Aorist; Voice=Active\n",
        )
        .unwrap();
        fs::write(
            dir.0.join("hebrew.tsv"),
            "HR\tFunction=Preposition\nHNcfsa\tFunction=Noun; Gender=Feminine\n",
        )
        .unwrap();
        let mut data = LanguageData::test_root(dir.0.clone());
        let p = Position {
            book: 43,
            chapter: 3,
            verse: 16,
        };
        assert_eq!(
            data.aligned("kjv", p, "Loved God!", 0).unwrap().strongs,
            ["G0025"]
        );
        assert!(data.aligned("nlt", p, "Loved God!", 0).is_none());
        assert!(data.aligned("kjv", p, "God loved", 0).is_none());
        assert!(data.aligned("kjv", p, "Loved God more", 0).is_none());
        assert!(data.morphology("V-AAI-3S").contains("Aorist"));
        assert_eq!(
            data.morphology("HR/Ncfsa"),
            "Function=Preposition\nFunction=Noun; Gender=Feminine"
        );
        assert_eq!(data.morphology("UNKNOWN"), "UNKNOWN");
    }

    #[test]
    fn imports_phrase_tags_and_excludes_notes_and_untranslated_words() {
        let dir = crate::storage::TestDir::new();
        import_kjv(br#"<osis><verse osisID="John.3.16" sID="John.3.16"/><w lemma="strong:G3588"/><w lemma="strong:G25" morph="robinson:V-AAI-3S">so loved</w> <note>ignore this</note><transChange>indeed</transChange> &amp; <w lemma="strong:G2316">God</w><verse eID="John.3.16"/></osis>"#, &dir.0).unwrap();
        let book: TaggedBook =
            serde_json::from_slice(&fs::read(dir.0.join("43.json")).unwrap()).unwrap();
        let verse = &book["3:16"];
        assert_eq!(verse.len(), 4);
        assert_eq!(verse[1].strongs, ["G0025"]);
        assert_eq!(verse[1].phrase, "so loved");
        assert_eq!(verse[1].morphs, ["V-AAI-3S"]);
        assert!(verse[2].strongs.is_empty());
        assert_eq!(verse[3].text, "God");
    }
    #[test]
    fn token_offsets_are_unicode_safe_and_preserve_repeated_words() {
        let words = tokens("God’s λόγος, word word.");
        assert_eq!(
            words.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>(),
            ["God’s", "λόγος", "word", "word"]
        );
        assert_eq!(words[1].0, 6..11);
    }
}
