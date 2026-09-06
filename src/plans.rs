//! Reading plans generated from the versification table, with progress
//! tracked in the study file.

use crate::v11n;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Plan {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub days: usize,
}

pub const PLANS: &[Plan] = &[
    Plan { id: "bible-365", name: "Whole Bible in a year", description: "Genesis to Revelation in canonical order, about 3 chapters a day", days: 365 },
    Plan { id: "nt-90", name: "New Testament in 90 days", description: "Matthew to Revelation, about 3 chapters a day", days: 90 },
    Plan { id: "gospels-30", name: "Gospels in 30 days", description: "Matthew, Mark, Luke and John, 3 chapters a day", days: 30 },
    Plan { id: "psalms-proverbs-31", name: "Psalms and Proverbs in a month", description: "Five psalms and one chapter of Proverbs each day", days: 31 },
    Plan { id: "ot-270", name: "Old Testament in 9 months", description: "Genesis to Malachi, about 3.5 chapters a day", days: 270 },
    Plan { id: "wisdom-60", name: "Wisdom books in 60 days", description: "Job, Psalms, Proverbs, Ecclesiastes and Song of Solomon", days: 60 },
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    pub id: String,
    /// Start date as days since the Unix epoch.
    pub start_day: i64,
    #[serde(default)]
    pub done: BTreeSet<u32>,
}

pub fn plan(id: &str) -> Option<&'static Plan> {
    PLANS.iter().find(|p| p.id == id)
}

/// Chapters (book nr, chapter nr) for a day (0-based) of a plan.
pub fn readings(id: &str, day: usize) -> Vec<(u32, u32)> {
    let Some(p) = plan(id) else { return Vec::new() };
    if day >= p.days {
        return Vec::new();
    }
    if id == "psalms-proverbs-31" {
        let d = day as u32 + 1;
        let mut out: Vec<(u32, u32)> = (0..5).map(|k| d + 30 * k).filter(|c| *c <= 150).map(|c| (19, c)).collect();
        out.push((20, d));
        return out;
    }
    let books: Vec<u32> = match id {
        "bible-365" => (1..=66).collect(),
        "nt-90" => (40..=66).collect(),
        "gospels-30" => (40..=43).collect(),
        "ot-270" => (1..=39).collect(),
        "wisdom-60" => (18..=22).collect(),
        _ => Vec::new(),
    };
    let chapters: Vec<(u32, u32)> = books
        .iter()
        .flat_map(|b| (1..=v11n::VERSES[(*b - 1) as usize].len() as u32).map(move |c| (*b, c)))
        .collect();
    let total = chapters.len();
    let start = day * total / p.days;
    let end = (day + 1) * total / p.days;
    chapters[start..end].to_vec()
}

pub fn today() -> i64 {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    // Local-time offset is not available without a tz crate; use UTC days.
    secs.div_euclid(86_400)
}

/// Civil date (y, m, d) from days since epoch. Howard Hinnant's algorithm.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn format_day(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

impl Progress {
    pub fn new(id: &str) -> Progress {
        Progress { id: id.to_string(), start_day: today(), done: BTreeSet::new() }
    }

    /// The plan day (0-based) that the calendar says is today.
    pub fn current_day(&self) -> usize {
        (today() - self.start_day).max(0) as usize
    }

    pub fn days(&self) -> usize {
        plan(&self.id).map(|p| p.days).unwrap_or(0)
    }

    pub fn behind(&self) -> usize {
        let cur = self.current_day().min(self.days());
        (0..cur).filter(|d| !self.done.contains(&(*d as u32))).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_cover_everything() {
        for p in PLANS {
            let mut total = 0;
            for d in 0..p.days {
                let r = readings(p.id, d);
                assert!(!r.is_empty(), "{} day {d} empty", p.id);
                total += r.len();
            }
            if p.id == "bible-365" {
                assert_eq!(total, 1189);
            }
            if p.id == "nt-90" {
                assert_eq!(total, 260);
            }
        }
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(format_day(20_702), "2026-09-06");
    }
}
