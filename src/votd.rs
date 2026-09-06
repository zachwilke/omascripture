//! Verse of the day: a fixed rotation of well-known passages chosen by date.

use crate::bible::{Loc, Translation};
use crate::plans;

pub const VERSES: &[&str] = &[
    "John 3:16", "Psalm 23:1", "Romans 8:28", "Philippians 4:13", "Jeremiah 29:11", "Proverbs 3:5", "Isaiah 41:10", "Psalm 46:1",
    "Matthew 11:28", "Romans 12:2", "Joshua 1:9", "Psalm 119:105", "Isaiah 40:31", "2 Timothy 1:7", "Galatians 5:22", "Hebrews 11:1",
    "Romans 5:8", "1 Corinthians 13:4", "Ephesians 2:8", "Psalm 27:1", "Matthew 6:33", "Philippians 4:6", "1 Peter 5:7", "Lamentations 3:22",
    "Micah 6:8", "John 14:6", "Psalm 37:4", "Isaiah 53:5", "Romans 10:9", "Proverbs 18:10", "Psalm 121:1", "Matthew 28:19",
    "John 1:1", "Genesis 1:1", "Psalm 139:14", "Colossians 3:23", "James 1:5", "1 John 1:9", "Romans 6:23", "Psalm 34:8",
    "Deuteronomy 31:6", "2 Corinthians 5:17", "Hebrews 13:8", "Psalm 118:24", "Isaiah 26:3", "John 16:33", "Ephesians 6:11", "Psalm 91:1",
    "Matthew 5:16", "Proverbs 22:6", "Romans 15:13", "1 Thessalonians 5:16", "Psalm 100:4", "Philippians 1:6", "Galatians 2:20", "John 15:5",
    "Psalm 51:10", "Isaiah 43:2", "Matthew 22:37", "Romans 8:38", "James 4:7", "Psalm 19:14", "Nahum 1:7", "1 Corinthians 10:13",
    "Hebrews 4:12", "Psalm 32:8", "Zephaniah 3:17", "John 10:10", "2 Chronicles 7:14", "Psalm 30:5", "Ecclesiastes 3:1", "Titus 3:5",
    "Psalm 150:6", "Matthew 7:7", "Luke 1:37", "Acts 1:8", "1 John 4:19", "Revelation 21:4", "Psalm 62:1", "Proverbs 16:3",
    "Isaiah 9:6", "Luke 2:11", "Matthew 1:23", "John 11:25", "1 Corinthians 15:55", "Romans 1:16", "Psalm 90:12", "Habakkuk 3:19",
    "Exodus 14:14", "Numbers 6:24", "Job 19:25", "Psalm 16:11", "Isaiah 55:8", "Jeremiah 33:3", "Daniel 3:17", "Hosea 6:6",
    "Joel 2:13", "Amos 5:24", "Jonah 2:9", "Malachi 3:10", "Mark 10:45", "Luke 6:31", "John 13:34", "Acts 4:12",
    "1 Corinthians 16:14", "2 Corinthians 12:9", "Ephesians 4:32", "Colossians 3:2", "1 Timothy 4:12", "Hebrews 12:1", "1 Peter 2:9", "Jude 1:24",
];

/// The reference chosen for today, resolved in the given translation.
pub fn today(t: &Translation) -> Option<(Loc, &'static str)> {
    let idx = (plans::today().rem_euclid(VERSES.len() as i64)) as usize;
    // Walk forward if a translation lacks the book (e.g. NT-only translations).
    for k in 0..VERSES.len() {
        let r = VERSES[(idx + k) % VERSES.len()];
        if let Some(loc) = t.parse_reference(r) {
            return Some((loc, r));
        }
    }
    None
}
