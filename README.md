# OmaScripture

A Logos-style Bible study tool for the terminal, built for
[Omarchy](https://omarchy.org), using only public-domain and Creative Commons
resources. No licences, no accounts, nothing to buy.

Read in any of 117 translations, open a study sidebar beside the text with
cross-references, a Greek and Hebrew interlinear, classic commentaries and
verse-by-verse notes, do word studies with lexicons and concordance, look up
names and places in five dictionaries, follow parallel passages, and keep a
reading plan. Every resource is downloaded once and works offline.

```
 OmaScripture │ John 3:16  King James Version                                        ? help
╭ John 3  ·  KJV ─────────────────────────────╮╭ 1 Refs 2 Words 3 Comm 4 Notes ────────────╮
│   15 That whosoever believeth in him should ││  οὕτως  houtōs                            │
│      not perish, but have eternal life.     ││    Thus  G3779  ADV                       │
│ ▎ 16 For God so loved the world, that he    ││  γὰρ  gar                                 │
│ ▎    gave his only begotten Son, that       ││    for  G1063  CONJ                       │
│ ▎    whosoever believeth in him should not  ││ ▎ἠγάπησεν  ēgapēsen                       │
│ ▎    perish, but have everlasting life.     ││ ▎  loved  G0025  V-AAI-3S                 │
│   17 For God sent not his Son into the      ││  ὁ  ho                                    │
│      world to condemn the world; but that   ││    <the>  G3588  T-NSM                    │
│      the world through him might be saved.  ││  θεὸς  theos                              │
╰─────────────────────────────────────── 61% ─╯╰───────────────────────────────────────────╯
 sidebar: j/k move  Enter open  1-4 tabs  C commentary  Tab/Esc back to text
```

## What you get

| Logos feature | OmaScripture | Source | Licence |
|---|---|---|---|
| Bible text | 117 translations: KJV, WEB, ASV, BSB, YLT, Darby, Douay-Rheims, Luther, Reina Valera, ... | getbible.net | Public domain / free |
| Parallel Bibles | Two translations side by side | | |
| Reverse interlinear | Every Greek and Hebrew word with Strong's number, morphology, transliteration, gloss | STEPBible TAGNT / TAHOT | CC BY 4.0 |
| Lexicons | Greek lexicon based on Abbott-Smith and LSJ; Hebrew based on BDB | STEPBible TBESG / TBESH | CC BY 4.0 |
| Concordance | Every occurrence of a word, by Strong's number | derived from the interlinear | |
| Cross-references | 340,000 weighted links | OpenBible.info | CC BY |
| | Treasury of Scripture Knowledge | CrossWire | Public domain |
| Commentaries | Matthew Henry, Barnes, Clarke, Jamieson-Fausset-Brown, Calvin, Wesley, Geneva notes | CrossWire SWORD modules | Public domain |
| Study notes | unfoldingWord Translation Notes on every verse | unfoldingWord | CC BY-SA 4.0 |
| Dictionaries | Easton's, Smith's, ISBE, Nave's Topical, Hitchcock's Names, unfoldingWord Translation Words | CrossWire, unfoldingWord | Public domain / CC BY-SA |
| Passage guide | Harmony of the Gospels, Old Testament parallels | built in | |
| Reading plans | Whole Bible, NT, Gospels, Psalms and Proverbs, OT, Wisdom | built in | |
| Notes and highlights | Bookmarks, four highlight colours, per-verse notes | stored as JSON | |
| Verse of the day | In the app and as an Omarchy bar widget | | |

What you do not get: modern copyrighted translations (NIV, ESV, NASB, NLT) or
commercial lexicons (BDAG, HALOT). Those need licences.

## Install on Omarchy

Requires a Rust toolchain (`omarchy install dev-env rust`).

```bash
git clone https://github.com/zachwilke/omascripture ~/Projects/omascripture
cd ~/Projects/omascripture
./install.sh --translation kjv --study basic --bar
```

| Flag | What it does |
|---|---|
| `--translation kjv` | download a Bible (any id from `omascripture --catalog`) |
| `--study basic` | cross-references, Greek interlinear and lexicon, Matthew Henry, Easton's (~60 MB) |
| `--study all` | every study pack (~200 MB) |
| `--study mhc,jfb,isbe` | a chosen set of pack ids |
| `--bar` | verse-of-the-day widget in the Omarchy bar; click opens the app |
| `--bind "SUPER + SHIFT + ALT + B"` | Hyprland keybinding |

The installer builds a release binary and sets up:

| What | Where |
|------|-------|
| Binary | `~/.local/bin/omascripture` |
| App launcher entry and icon | `~/.local/share/applications/OmaScripture.desktop` |
| Omarchy menu entry | `Super+Space → Learn → Bible`, also `omarchy menu summon bible` |
| Bar widget (with `--bar`) | a `command` module in `~/.config/omarchy/shell.json` |

Nothing under `/usr/share/omarchy` is modified. `./uninstall.sh` reverses all
of it and keeps your data unless you pass `--purge`.

## Using it

```bash
omascripture                       # resume where you left off
omascripture "Romans 8:28"         # open at a reference
omascripture -t web "Ps 23"        # pick the translation for this session
omarchy launch tui omascripture    # Omarchy-styled terminal window
```

### The study sidebar

Press `s` to open the sidebar, or `1` to `4` to jump straight to a tab. It
follows the selected verse.

1. **Cross-refs**: OpenBible links ranked by usefulness, plus Treasury of
   Scripture Knowledge entries. `Tab` into the sidebar, `Enter` jumps.
2. **Interlinear**: every original-language word with transliteration, gloss,
   Strong's number and morphology. `Enter` on a word opens a word study with
   the lexicon entry; `o` lists every occurrence, `Enter` jumps to one.
3. **Commentary**: the current commentary's entry for this verse, falling back
   to the nearest preceding comment. `C` cycles through installed commentaries.
4. **Notes**: unfoldingWord Translation Notes for the verse, with the
   original-language phrase each note is about.

### Other study tools

| Key | Tool |
|-----|------|
| `w` | interlinear words for this verse, ready for word study |
| `D` | look up a name, place or topic across every installed dictionary |
| `P` | parallel passages: Gospel harmony and Old Testament parallels |
| `r` | reading plans: start one, see today's chapters, mark days done |
| `R` | install or remove study resources |
| `v` | verse of the day |
| `/` | search the whole translation; `n` / `N` step through results |
| `m` `B` | bookmark a verse, list bookmarks |
| `x` | cycle highlight colour |
| `e` | write a note on the verse (`Ctrl+S` saves) |
| `y` | copy verse and reference to the clipboard |

### Reading keys

| Key | Action |
|-----|--------|
| `j` / `k`, `↓` / `↑` | next / previous verse |
| `J` / `K`, `PgDn` / `PgUp` | jump 10 verses |
| `g` / `G` | first / last verse of chapter |
| `h` / `l`, `←` / `→` | previous / next chapter |
| `[` / `]` | previous / next book |
| `b`, `c` | choose book, choose chapter |
| `:` or `o` | go to a reference: `John 3:16`, `Ps 23`, `1 Jn 2`, `rev` |
| `u` / `Backspace` | back to where you jumped from |
| `t` / `T` | choose reading / parallel translation |
| `p` | show or hide the parallel pane |
| `?` | help |
| `q` | quit; position and study data are saved |

### Command line

```bash
omascripture --catalog             # every translation available
omascripture --download asv        # fetch a translation
omascripture --list                # downloaded translations
omascripture --resources           # every study pack, with licence and size
omascripture --install mhc         # fetch a pack (or: --install all)
omascripture --remove mhc
omascripture --votd                # verse of the day, plain text
omascripture --votd-bar            # verse of the day as bar JSON
```

## Data

Everything lives in `~/.local/share/omascripture/` (override with the
`OMASCRIPTURE_DATA` environment variable):

```
translations/<id>.json      cached translations from getbible.net
resources/<pack>/           study packs, each with installed.json
study.json                  bookmarks, highlights, notes, plan, last position
```

Bookmarks, highlights and notes are keyed by book, chapter and verse number,
so they follow you across translations. Study packs are ordinary files: SWORD
modules are kept as downloaded, STEPBible tables are split per book, and
unfoldingWord notes are the original TSV files.

## Development

```bash
cargo run -- "John 1"
cargo test
```

| Module | Purpose |
|---|---|
| `bible.rs` | text model, translation store, getbible client, reference parsing |
| `resources.rs` | study pack registry, downloads, and parsers for STEPBible, OpenBible and unfoldingWord data |
| `sword.rs` | reader for CrossWire SWORD zCom/zCom4 commentaries and zLD/RawLD dictionaries |
| `v11n.rs` | KJV versification table (generated) used to index SWORD modules |
| `harmony.rs` | parallel passage table |
| `plans.rs` | reading plans and date arithmetic |
| `votd.rs` | verse of the day rotation |
| `study.rs` | persisted study data |
| `app.rs` | state and key handling |
| `ui.rs` | rendering |

## Sources and licences

- Bible texts: [getbible.net](https://getbible.net), which serves public-domain and freely licensed texts from the CrossWire SWORD project.
- Interlinear and lexicons: [STEPBible-Data](https://github.com/STEPBible/STEPBible-Data) by Tyndale House, Cambridge, CC BY 4.0.
- Cross-references: [OpenBible.info](https://www.openbible.info/labs/cross-references/), CC BY.
- Commentaries, TSK and dictionaries: [CrossWire Bible Society](https://crosswire.org) SWORD modules, public domain.
- Translation Notes and Translation Words: [unfoldingWord](https://www.unfoldingword.org), CC BY-SA 4.0.

Check each resource's licence before redistributing its text. The application
itself is MIT licensed.
