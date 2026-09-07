# Performance checks — 0.6.1

The GUI uses a one-second idle maintenance interval for live themes and autosave.
Input and animations still request their own frames, and pending background work
uses a 150 ms poll. Finished word-study workers are drained even after closing
their dialog, so they cannot leave the idle window in fast polling mode.

Interlinear books, alignment books, translation-note books and commentaries keep
the current and previous items in memory. Dictionaries stay cached because a
dictionary search queries all installed dictionaries. Verse lookups and validated
alignment are reused while reading the same verse. Resource replacement still
invalidates alignment. Unchanged theme text and built-in palettes are no longer
parsed repeatedly.

Word-study scans reuse any already cached books and load other books one at a
time. Temporary scan books never populate the browsing cache. Corpus selection,
variant handling, extended Strong's identities and the 600-reference display
limit are unchanged.

## Installed-corpus measurements

Measured on this machine using separate debug test processes with the same
installed STEPBible data. These are peak resident memory measurements for the
scan test process, not total GUI memory or a promised saving on every system.

| Scan | 0.6.0 peak RSS | 0.6.1 peak RSS | Before / after elapsed |
|---|---:|---:|---:|
| Greek G0025, 27 books | 72,960 KiB | 24,528 KiB | 0.716 / 0.662 s |
| Hebrew H0430, 39 books | 133,968 KiB | 23,776 KiB | 2.622 / 2.440 s |

Both versions returned 143 occurrences in 110 verses for G0025, and 2,242
occurrences in 1,941 verses for H0430. Scans previously retained 27/39 books;
the new scans retain zero additional books. Allocator behavior and OS page caches
affect RSS and timing, so these are observations rather than fixed budgets.

Reproduce each corpus in a separate process:

```sh
cargo test --locked profile_installed_word_report -- --ignored --nocapture
OMASCRIPTURE_PROFILE_STRONG=H0430 cargo test --locked profile_installed_word_report -- --ignored --nocapture
```

## Native idle measurement

Twelve-second release-build samples after five seconds of startup, on the same
machine and reading setup:

| Measurement | 0.6.0 | 0.6.1 |
|---|---:|---:|
| CPU, percent of one core | 0.667% | 0.083% |
| RSS | 259,172 KiB | 259,172 KiB |
| PSS | 96,947 KiB | 96,960 KiB |
| Private clean + dirty | 58,372 KiB | 58,396 KiB |

Idle CPU is lower in this sample; baseline reading-window memory is effectively
unchanged. The large memory reduction applies to word-study scans and the limits
on cache growth during browsing. CPU samples are short and quantized by OS ticks.

`python scripts/profile-idle.py [binary]` opens a temporary KJV reading window,
waits five seconds, and measures twelve seconds of idle process CPU and memory
through Linux `/proc`. It uses isolated study/settings files and shared read-only
access to the installed resource files. Leave the window idle during sampling.
The temporary process is terminated after measurement; this is not a save/close
lifecycle test.

RSS includes shared libraries and graphics mappings. PSS apportions shared
pages, while private clean/dirty memory excludes those shared pages. None of
these readings captures all GPU-driver memory.

## Regression coverage

Tests cover bounded book browsing, verse-cache navigation and independent return
values, alignment changes and resource removal, closed-dialog worker completion,
dynamic theme updates, occurrence counts and reference navigation. The installed
corpus diagnostic is opt-in; regular tests do not need installed resources.
