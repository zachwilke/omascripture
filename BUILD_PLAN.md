# GUI messages — 0.6.2

- Audited GUI-visible status messages, errors, empty states, provider failures,
  and settings values. Shared messages now name Resources, Settings, Search and
  library actions instead of terminal shortcuts. Provider failures no longer
  recommend F5, and missing credentials direct users to Settings.
- Removed the GUI's partial replacement of "Press R" text; the source messages
  are now suitable for the desktop UI.
- Empty searches stay open with a visible explanation and search controls.
  Online-only Bibles offer a Choose an offline Bible button for whole-Bible search.
  A GUI interaction regression covers both empty-state recovery paths.
- Validation: 58 tests passed, 3 opt-in diagnostics skipped; Clippy and release
  build passed. Version 0.6.2 is installed in ~/.local/bin/omascripture.

---

# Lean resource use — 0.6.1

- Reduced idle GUI maintenance redraws from about 6.7 Hz to 1 Hz; retained fast
  input/animation handling and 150 ms background-result polling. Completed word
  workers drain after their dialog closes, allowing idle polling to resume.
- Word-study scans load one temporary book at a time without growing the reader
  cache. Cache limits retain the current/previous interlinear, alignment and
  translation-note books and commentaries. Repeated verse/alignment lookups and
  unchanged theme parsing are avoided.
- Installed-corpus scan peak RSS fell from 72,960 to 24,528 KiB (Greek) and
  133,968 to 23,776 KiB (Hebrew), with identical counts. Sampled native idle CPU
  fell from 0.667% to 0.083% of one core; baseline reading memory was unchanged.
  See PERFORMANCE.md for methodology, limits and reproducible commands.
- Validation: 57 regular tests passed, 3 opt-in diagnostics skipped in the regular
  suite; both installed-corpus benchmarks were run explicitly. Clippy passed with
  warnings denied. Release 0.6.1 built and installed in ~/.local/bin/omascripture.

---

# Word studies — 0.6.0

- Click/right-click a reading word for Word Study, copying, search, and verse actions.
  Hover cards show aligned language data; text dragging remains available.
- CrossWire KJV OSIS importer preserves phrase-to-Strong’s links and Greek morphology,
  excludes editorial material and validates the whole displayed verse before linking.
  Actual installed KJV comparison: 31,088 / 31,102 verse word sequences agree;
  unmatched verses and other editions use explicit original-word selection.
- Original words expose lemma, transliteration, gloss, Strong’s ID and expanded
  Greek/Hebrew grammar from STEPBible TEGMC/TEHMC. Original-word sidebar hovers work too.
- Word Study presents lexicon definitions, book distribution, inflected forms and
  occurrence references. Background indexing keeps the GUI responsive. Counts use
  NA28/Leningrad data, distinguish word occurrences from unique verses, and report
  partial corpus installation and result truncation. Extended Strong’s IDs remain distinct.
- New downloadable word-study resource pack, plus source attribution and setup docs.
- Regression coverage: tagged-phrase import, editorial-note exclusion, supplied words,
  exact verse matching, Unicode offsets, morphology expansions, variant-free counts,
  and mouse hover → word menu → correct contextual lemma study.
- Validation: 53 tests pass, with 2 opt-in checks skipped; Clippy passes with
  warnings denied. The capped 600-result occurrence dialog is covered through
  rendering, its truncation notice, and reference navigation. Native Wayland
  inspection confirmed KJV hover data, word menus, definitions and usage charts.
  One long-running debug QA window became unresponsive; the cause was not
  established. Fresh release windows rendered studies and exited cleanly;
  native occurrence navigation remains unverified because automation was unreliable.
- Release build completed; 0.6.0 is installed in `~/.local/bin/omascripture`,
  with the word-study language pack installed alongside the existing interlinears
  and lexicons. Reopen the app to load the updated executable.

---

# Reliability hardening — 0.5.2

Completed a failure-path audit of persistence, asynchronous translation loading,
local Bible data, and resource readers/installers.

- Durable, private atomic writes for studies, credentials, catalogs and Bibles.
- Previous-version study backup, recovery from malformed JSON, preservation of
  damaged originals, and recovery export of in-memory edits.
- Nonblocking file locks plus optimistic concurrency checks prevent stale
  windows from overwriting newer study data. Unknown JSON fields survive saves.
- Persistent save errors, retry/export controls, failed-close interception, and
  failed note saves leave the editor open. Explicit close without saving remains
  available. The TUI reports exit-save failures and attempts a recovery export.
- Three-second checkpoints preserve reading position and unfinished notes;
  drafts reopen after restart. Cancel/Close in the note editor discards drafts.
- Translation load generations reject stale responses; removing a parallel
  Bible invalidates its pending load. Catalog refresh requests are deduplicated.
  Worker panics become reportable errors instead of leaving an endless spinner.
- Offline Bible validation rejects empty structures, duplicate numbering and
  mismatched IDs before navigation. Storage IDs cannot escape their directory.
- Resource install/remove locks prevent overlapping mutations. Failed publishes
  restore the previously installed pack; ZIP expansion and SWORD block sizes are
  bounded. Damaged dictionary indexes fail safely instead of panicking.
- Out-of-range saved highlight values and extreme reading-plan dates no longer
  overflow. Closing during initial loading preserves the saved reading position.
- Ctrl+S uses key-event modifiers so fast modifier releases cannot drop note saves.
- GitHub Actions workflow for locked tests, warning-free Clippy, and release builds.

Validation includes regression tests with isolated storage for corrupt files,
locked/unwritable destinations, concurrent saves, recovery exports, draft restore,
failed window close, fast save shortcuts, in-dialog recovery export, stale workers,
damaged dictionaries and resource rollback. Native Wayland checks confirmed draft
restore, blocked conflicting saves/close, successful retry, and clean exit.
Final checks: 47 tests passed, 2 opt-in tests skipped; Clippy passed with warnings
denied; release build and native lifecycle checks passed. Version 0.5.2 is
installed in `~/.local/bin/omascripture`.

The live NET/NLT and SWORD diagnostics remain opt-in; authenticated ESV/API.Bible
validation still requires credentials. CI is configured locally, not yet run on
GitHub. Backups hold one previous save and do not replace external disk backups.

---

# Dynamic Omarchy themes — 0.5.1

Implemented automatic GUI palette reloads from Omarchy’s active `colors.toml`.
Backgrounds, text, controls, accents, selections, and verse highlights all use the
palette; `mode` controls dark/light widget defaults. Readable secondary text uses
`light_foreground`, while `muted` is reserved for borders. Settings defaults to
Follow Omarchy and offers persistent Light/Dark overrides. Existing study files
opt into automatic theming without losing their other settings.

Poll the active path every 750 ms, supporting both current state and legacy config
locations. Retain the last valid palette during directory swaps or malformed
updates. Reload colors without resetting fonts, reading position, or open dialogs.
No changes to Omarchy’s hooks or system theme files are required.

Validation covers dark/light parsing, malformed updates, directory replacement,
legacy paths, fallback palettes, and manual overrides, plus existing GUI tests.
All 30 automated tests pass (2 unrelated live-data tests ignored). Native Wayland
screenshots confirmed the active green palette and a live Rose Pine light palette
switch in the same window. Release 0.5.1 is installed in `~/.local/bin`.

---

# Native desktop app — 0.5.0

The user requested a real GUI after the provider work. The default launch now
opens a standalone eframe/egui window; the terminal renderer requires `--tui`.

Implemented:

- Wayland/X11 desktop window with its own graphical application ID.
- Book navigator, chapter picker, reference bar, proportional interface fonts,
  serif reading text, selection, copying and verse context menus.
- Parallel Bibles and adjustable study/navigation sidebars.
- Graphical search, bookmarks, note editing, study resources, reading plans,
  dictionaries, parallel passages, word study and occurrence lists.
- Reading settings, font size, light/dark appearance, masked provider-key entry,
  private atomic key saving, and API.Bible edition configuration.
- GUI desktop/menu launchers; retained headless CLI and explicit terminal mode.

Validation includes native Wayland screenshots, headless GUI mouse interaction
and layout tests, private key-file tests, and the existing Bible/provider suite.

---

# Modern translation provider plan

Recovered from Claude's project conversation on 2026-09-06. The last approved
request was to implement modern Bible access: NET, then ESV, then NLT, then
API.Bible. The existing uncommitted mouse controls and settings page preceded
that request and are retained.

## Implemented in 0.4.0

- Online source state alongside the existing offline translation model.
- Background chapter requests for reading and parallel panes; stale responses
  discarded after navigation or source changes.
- Current-chapter memory only, without provider text written to disk.
- NET JSON, ESV numbered text, NLT HTML, and API.Bible numbered-text adapters.
- Anonymous NET/NLT access; NLT long chapters split into at most 50-verse requests.
- Private provider config template, environment overrides, redacted errors,
  provider status in Settings and the translation picker.
- Authorized API.Bible edition discovery through `--api-bibles`.
- Default/parallel selection, reference navigation, reading plans and study tools.
- Provider attribution, persistent chapter errors and clickable retry controls.
- Explicit offline-translation requirement for whole-Bible search.
- README setup instructions and CLI help.

## Review fixes

- Preserve verse number on retry and in Back history after chapter eviction.
- Restore the appropriate settings row when exiting the translation picker.
- Keep a saved hidden parallel pane hidden at startup.
- Keep provider status and long error messages visible in the picker/title.
- Prevent copying an empty verse while an online chapter is loading.

## Validation

- Unit coverage for parsers, malformed responses, late responses, selection,
  parallel isolation, retry eviction, and terminal rendering at different sizes.
- Live NET/NLT checks: John 3, John 1 (crosses the anonymous NLT batch boundary),
  and Psalm 119 (176 verses).
- Isolated terminal smoke checks: NET/NLT parallel reading, next chapter,
  retry, settings, missing ESV key, no downloaded provider text, and private
  config creation without overwriting an existing file.

## External validation still needed

- Exercise ESV with a personal Crossway API key.
- Exercise API.Bible discovery and chapter reading with an authorized key and
  edition ID. NIV/NASB availability depends on the account's permissions.
- Only NET and NLT have been tested against unauthenticated live endpoints;
  ESV/API.Bible parsers follow the current official response documentation.

## Later enhancements (not included)

- NET translator-note ingestion and provider-native remote search.
- Alternate canons and versification metadata beyond the standard 66 books.
- The broader Logos wishlist (SWORD tagged Bibles, passage guide, library search,
  richer notes and workspaces) was discussed but was not the final build request.
