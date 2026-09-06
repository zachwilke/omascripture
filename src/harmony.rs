//! Parallel passages: a harmony of the Gospels plus a few Old Testament
//! parallels. Pericope boundaries follow the conventional synopsis divisions.

use crate::bible::Position;

/// A passage range within one book.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub book: u32,
    pub start: (u32, u32),
    pub end: (u32, u32),
}

impl Range {
    pub fn contains(&self, p: Position) -> bool {
        p.book == self.book && (p.chapter, p.verse) >= self.start && (p.chapter, p.verse) <= self.end
    }
    pub fn start_position(&self) -> Position {
        Position { book: self.book, chapter: self.start.0, verse: self.start.1 }
    }
}

pub struct Pericope {
    pub title: &'static str,
    pub refs: &'static [&'static str],
}

fn book_nr(abbr: &str) -> Option<u32> {
    Some(match abbr {
        "Ex" => 2,
        "Dt" => 5,
        "2Sa" => 10,
        "2Ki" => 12,
        "1Ch" => 13,
        "2Ch" => 14,
        "Ps" => 19,
        "Isa" => 23,
        "Mt" => 40,
        "Mk" => 41,
        "Lk" => 42,
        "Jn" => 43,
        _ => return None,
    })
}

pub fn book_abbr(nr: u32) -> &'static str {
    match nr {
        2 => "Exodus",
        5 => "Deuteronomy",
        10 => "2 Samuel",
        12 => "2 Kings",
        13 => "1 Chronicles",
        14 => "2 Chronicles",
        19 => "Psalms",
        23 => "Isaiah",
        40 => "Matthew",
        41 => "Mark",
        42 => "Luke",
        43 => "John",
        _ => "?",
    }
}

/// Parse "Mt 14:13-21", "Jn 18:28-19:16", "Mk 8:31-9:1", "2Sa 24".
pub fn parse_range(s: &str) -> Option<Range> {
    let (b, rest) = s.split_once(' ')?;
    let book = book_nr(b)?;
    let (start_s, end_s) = match rest.split_once('-') {
        Some((a, b)) => (a, Some(b)),
        None => (rest, None),
    };
    let parse_cv = |t: &str, default_ch: u32| -> Option<(u32, u32)> {
        match t.split_once(':') {
            Some((c, v)) => Some((c.parse().ok()?, v.parse().ok()?)),
            None => Some((default_ch, t.parse().ok()?)),
        }
    };
    let start = match start_s.split_once(':') {
        Some(_) => parse_cv(start_s, 0)?,
        None => (start_s.parse().ok()?, 1),
    };
    let end = match end_s {
        Some(e) => parse_cv(e, start.0)?,
        None => {
            if start_s.contains(':') { start } else { (start.0, 999) }
        }
    };
    Some(Range { book, start, end })
}

pub fn label(r: &Range) -> String {
    let b = book_abbr(r.book);
    if r.end.1 == 999 {
        format!("{b} {}", r.start.0)
    } else if r.start.0 == r.end.0 {
        if r.start.1 == r.end.1 { format!("{b} {}:{}", r.start.0, r.start.1) } else { format!("{b} {}:{}-{}", r.start.0, r.start.1, r.end.1) }
    } else {
        format!("{b} {}:{}-{}:{}", r.start.0, r.start.1, r.end.0, r.end.1)
    }
}

/// Pericopes that include the given position.
pub fn find(p: Position) -> Vec<(&'static Pericope, Vec<Range>)> {
    let mut out = Vec::new();
    for per in PERICOPES {
        let ranges: Vec<Range> = per.refs.iter().filter_map(|r| parse_range(r)).collect();
        if ranges.iter().any(|r| r.contains(p)) {
            out.push((per, ranges));
        }
    }
    out
}

pub const PERICOPES: &[Pericope] = &[
    Pericope { title: "Prologue: the Word became flesh", refs: &["Jn 1:1-18"] },
    Pericope { title: "Genealogy of Jesus", refs: &["Mt 1:1-17", "Lk 3:23-38"] },
    Pericope { title: "Annunciation to Mary", refs: &["Lk 1:26-38"] },
    Pericope { title: "Birth of John the Baptist", refs: &["Lk 1:57-80"] },
    Pericope { title: "Birth of Jesus", refs: &["Mt 1:18-25", "Lk 2:1-7"] },
    Pericope { title: "Shepherds and angels", refs: &["Lk 2:8-20"] },
    Pericope { title: "Visit of the Magi", refs: &["Mt 2:1-12"] },
    Pericope { title: "Flight into Egypt and return", refs: &["Mt 2:13-23"] },
    Pericope { title: "Boy Jesus in the temple", refs: &["Lk 2:41-52"] },
    Pericope { title: "Ministry of John the Baptist", refs: &["Mt 3:1-12", "Mk 1:1-8", "Lk 3:1-20", "Jn 1:19-28"] },
    Pericope { title: "Baptism of Jesus", refs: &["Mt 3:13-17", "Mk 1:9-11", "Lk 3:21-22", "Jn 1:29-34"] },
    Pericope { title: "Temptation in the wilderness", refs: &["Mt 4:1-11", "Mk 1:12-13", "Lk 4:1-13"] },
    Pericope { title: "First disciples", refs: &["Jn 1:35-51"] },
    Pericope { title: "Wedding at Cana", refs: &["Jn 2:1-12"] },
    Pericope { title: "Early cleansing of the temple", refs: &["Jn 2:13-25"] },
    Pericope { title: "Nicodemus", refs: &["Jn 3:1-21"] },
    Pericope { title: "Samaritan woman at the well", refs: &["Jn 4:1-42"] },
    Pericope { title: "Beginning of the Galilean ministry", refs: &["Mt 4:12-17", "Mk 1:14-15", "Lk 4:14-15", "Jn 4:43-45"] },
    Pericope { title: "Rejection at Nazareth", refs: &["Mt 13:53-58", "Mk 6:1-6", "Lk 4:16-30"] },
    Pericope { title: "Calling of the fishermen", refs: &["Mt 4:18-22", "Mk 1:16-20", "Lk 5:1-11"] },
    Pericope { title: "Man with an unclean spirit in Capernaum", refs: &["Mk 1:21-28", "Lk 4:31-37"] },
    Pericope { title: "Peter's mother-in-law healed", refs: &["Mt 8:14-17", "Mk 1:29-34", "Lk 4:38-41"] },
    Pericope { title: "Preaching tour through Galilee", refs: &["Mt 4:23-25", "Mk 1:35-39", "Lk 4:42-44"] },
    Pericope { title: "Cleansing of a leper", refs: &["Mt 8:1-4", "Mk 1:40-45", "Lk 5:12-16"] },
    Pericope { title: "Healing of the paralytic", refs: &["Mt 9:1-8", "Mk 2:1-12", "Lk 5:17-26"] },
    Pericope { title: "Call of Matthew (Levi)", refs: &["Mt 9:9-13", "Mk 2:13-17", "Lk 5:27-32"] },
    Pericope { title: "Question about fasting", refs: &["Mt 9:14-17", "Mk 2:18-22", "Lk 5:33-39"] },
    Pericope { title: "Plucking grain on the Sabbath", refs: &["Mt 12:1-8", "Mk 2:23-28", "Lk 6:1-5"] },
    Pericope { title: "Man with a withered hand", refs: &["Mt 12:9-14", "Mk 3:1-6", "Lk 6:6-11"] },
    Pericope { title: "Choosing the Twelve", refs: &["Mt 10:1-4", "Mk 3:13-19", "Lk 6:12-16"] },
    Pericope { title: "Sermon on the Mount / Plain", refs: &["Mt 5:1-7:29", "Lk 6:17-49"] },
    Pericope { title: "Beatitudes", refs: &["Mt 5:1-12", "Lk 6:20-26"] },
    Pericope { title: "Love your enemies", refs: &["Mt 5:38-48", "Lk 6:27-36"] },
    Pericope { title: "The Lord's Prayer", refs: &["Mt 6:9-13", "Lk 11:1-4"] },
    Pericope { title: "Do not worry", refs: &["Mt 6:25-34", "Lk 12:22-34"] },
    Pericope { title: "Judging others", refs: &["Mt 7:1-5", "Lk 6:37-42"] },
    Pericope { title: "Ask, seek, knock", refs: &["Mt 7:7-11", "Lk 11:9-13"] },
    Pericope { title: "The Golden Rule", refs: &["Mt 7:12", "Lk 6:31"] },
    Pericope { title: "A tree and its fruit", refs: &["Mt 7:15-20", "Lk 6:43-45"] },
    Pericope { title: "House on the rock", refs: &["Mt 7:24-27", "Lk 6:46-49"] },
    Pericope { title: "Centurion's servant", refs: &["Mt 8:5-13", "Lk 7:1-10"] },
    Pericope { title: "Widow's son at Nain", refs: &["Lk 7:11-17"] },
    Pericope { title: "John the Baptist's question", refs: &["Mt 11:2-19", "Lk 7:18-35"] },
    Pericope { title: "Woman anoints Jesus' feet", refs: &["Lk 7:36-50"] },
    Pericope { title: "Beelzebul controversy", refs: &["Mt 12:22-32", "Mk 3:20-30", "Lk 11:14-23"] },
    Pericope { title: "Jesus' true family", refs: &["Mt 12:46-50", "Mk 3:31-35", "Lk 8:19-21"] },
    Pericope { title: "Parable of the sower", refs: &["Mt 13:1-23", "Mk 4:1-20", "Lk 8:4-15"] },
    Pericope { title: "Parable of the mustard seed", refs: &["Mt 13:31-32", "Mk 4:30-32", "Lk 13:18-19"] },
    Pericope { title: "Stilling the storm", refs: &["Mt 8:23-27", "Mk 4:35-41", "Lk 8:22-25"] },
    Pericope { title: "The Gerasene demoniac", refs: &["Mt 8:28-34", "Mk 5:1-20", "Lk 8:26-39"] },
    Pericope { title: "Jairus' daughter and the woman with a haemorrhage", refs: &["Mt 9:18-26", "Mk 5:21-43", "Lk 8:40-56"] },
    Pericope { title: "Sending out the Twelve", refs: &["Mt 10:5-15", "Mk 6:7-13", "Lk 9:1-6"] },
    Pericope { title: "Death of John the Baptist", refs: &["Mt 14:1-12", "Mk 6:14-29", "Lk 9:7-9"] },
    Pericope { title: "Feeding the five thousand", refs: &["Mt 14:13-21", "Mk 6:30-44", "Lk 9:10-17", "Jn 6:1-15"] },
    Pericope { title: "Walking on the water", refs: &["Mt 14:22-33", "Mk 6:45-52", "Jn 6:16-21"] },
    Pericope { title: "Bread of life discourse", refs: &["Jn 6:22-59"] },
    Pericope { title: "Clean and unclean", refs: &["Mt 15:1-20", "Mk 7:1-23"] },
    Pericope { title: "The Syrophoenician woman", refs: &["Mt 15:21-28", "Mk 7:24-30"] },
    Pericope { title: "Feeding the four thousand", refs: &["Mt 15:32-39", "Mk 8:1-10"] },
    Pericope { title: "Peter's confession", refs: &["Mt 16:13-20", "Mk 8:27-30", "Lk 9:18-21"] },
    Pericope { title: "First passion prediction", refs: &["Mt 16:21-28", "Mk 8:31-9:1", "Lk 9:22-27"] },
    Pericope { title: "Transfiguration", refs: &["Mt 17:1-13", "Mk 9:2-13", "Lk 9:28-36"] },
    Pericope { title: "Boy with an unclean spirit", refs: &["Mt 17:14-21", "Mk 9:14-29", "Lk 9:37-43"] },
    Pericope { title: "Who is the greatest?", refs: &["Mt 18:1-5", "Mk 9:33-37", "Lk 9:46-48"] },
    Pericope { title: "Parable of the lost sheep", refs: &["Mt 18:10-14", "Lk 15:1-7"] },
    Pericope { title: "The unforgiving servant", refs: &["Mt 18:21-35"] },
    Pericope { title: "The good Samaritan", refs: &["Lk 10:25-37"] },
    Pericope { title: "Mary and Martha", refs: &["Lk 10:38-42"] },
    Pericope { title: "Man born blind", refs: &["Jn 9:1-41"] },
    Pericope { title: "The good shepherd", refs: &["Jn 10:1-21"] },
    Pericope { title: "The prodigal son", refs: &["Lk 15:11-32"] },
    Pericope { title: "The rich man and Lazarus", refs: &["Lk 16:19-31"] },
    Pericope { title: "Raising of Lazarus", refs: &["Jn 11:1-44"] },
    Pericope { title: "Ten lepers", refs: &["Lk 17:11-19"] },
    Pericope { title: "Pharisee and tax collector", refs: &["Lk 18:9-14"] },
    Pericope { title: "Blessing the children", refs: &["Mt 19:13-15", "Mk 10:13-16", "Lk 18:15-17"] },
    Pericope { title: "The rich young ruler", refs: &["Mt 19:16-30", "Mk 10:17-31", "Lk 18:18-30"] },
    Pericope { title: "Workers in the vineyard", refs: &["Mt 20:1-16"] },
    Pericope { title: "Third passion prediction", refs: &["Mt 20:17-19", "Mk 10:32-34", "Lk 18:31-34"] },
    Pericope { title: "Request of James and John", refs: &["Mt 20:20-28", "Mk 10:35-45"] },
    Pericope { title: "Blind Bartimaeus", refs: &["Mt 20:29-34", "Mk 10:46-52", "Lk 18:35-43"] },
    Pericope { title: "Zacchaeus", refs: &["Lk 19:1-10"] },
    Pericope { title: "Anointing at Bethany", refs: &["Mt 26:6-13", "Mk 14:3-9", "Jn 12:1-8"] },
    Pericope { title: "Triumphal entry", refs: &["Mt 21:1-11", "Mk 11:1-11", "Lk 19:28-40", "Jn 12:12-19"] },
    Pericope { title: "Cleansing of the temple", refs: &["Mt 21:12-17", "Mk 11:15-19", "Lk 19:45-48"] },
    Pericope { title: "Cursing of the fig tree", refs: &["Mt 21:18-22", "Mk 11:12-25"] },
    Pericope { title: "Jesus' authority questioned", refs: &["Mt 21:23-27", "Mk 11:27-33", "Lk 20:1-8"] },
    Pericope { title: "Parable of the wicked tenants", refs: &["Mt 21:33-46", "Mk 12:1-12", "Lk 20:9-19"] },
    Pericope { title: "Paying taxes to Caesar", refs: &["Mt 22:15-22", "Mk 12:13-17", "Lk 20:20-26"] },
    Pericope { title: "Question about the resurrection", refs: &["Mt 22:23-33", "Mk 12:18-27", "Lk 20:27-40"] },
    Pericope { title: "The greatest commandment", refs: &["Mt 22:34-40", "Mk 12:28-34", "Lk 10:25-28"] },
    Pericope { title: "The widow's offering", refs: &["Mk 12:41-44", "Lk 21:1-4"] },
    Pericope { title: "Olivet discourse", refs: &["Mt 24:1-51", "Mk 13:1-37", "Lk 21:5-36"] },
    Pericope { title: "Parable of the ten virgins", refs: &["Mt 25:1-13"] },
    Pericope { title: "Parable of the talents / minas", refs: &["Mt 25:14-30", "Lk 19:11-27"] },
    Pericope { title: "The sheep and the goats", refs: &["Mt 25:31-46"] },
    Pericope { title: "Judas agrees to betray Jesus", refs: &["Mt 26:14-16", "Mk 14:10-11", "Lk 22:3-6"] },
    Pericope { title: "The Last Supper", refs: &["Mt 26:17-30", "Mk 14:12-26", "Lk 22:7-23", "Jn 13:1-30"] },
    Pericope { title: "Washing the disciples' feet", refs: &["Jn 13:1-20"] },
    Pericope { title: "Peter's denial foretold", refs: &["Mt 26:31-35", "Mk 14:27-31", "Lk 22:31-34", "Jn 13:36-38"] },
    Pericope { title: "Farewell discourse", refs: &["Jn 14:1-16:33"] },
    Pericope { title: "High priestly prayer", refs: &["Jn 17:1-26"] },
    Pericope { title: "Gethsemane", refs: &["Mt 26:36-46", "Mk 14:32-42", "Lk 22:39-46", "Jn 18:1"] },
    Pericope { title: "Arrest of Jesus", refs: &["Mt 26:47-56", "Mk 14:43-52", "Lk 22:47-53", "Jn 18:2-12"] },
    Pericope { title: "Before the Sanhedrin", refs: &["Mt 26:57-68", "Mk 14:53-65", "Lk 22:66-71", "Jn 18:19-24"] },
    Pericope { title: "Peter's denial", refs: &["Mt 26:69-75", "Mk 14:66-72", "Lk 22:54-62", "Jn 18:15-27"] },
    Pericope { title: "Before Pilate", refs: &["Mt 27:11-26", "Mk 15:1-15", "Lk 23:1-25", "Jn 18:28-19:16"] },
    Pericope { title: "Crucifixion", refs: &["Mt 27:32-56", "Mk 15:21-41", "Lk 23:26-49", "Jn 19:17-37"] },
    Pericope { title: "Burial of Jesus", refs: &["Mt 27:57-61", "Mk 15:42-47", "Lk 23:50-56", "Jn 19:38-42"] },
    Pericope { title: "The empty tomb", refs: &["Mt 28:1-10", "Mk 16:1-8", "Lk 24:1-12", "Jn 20:1-10"] },
    Pericope { title: "Appearance to Mary Magdalene", refs: &["Mk 16:9-11", "Jn 20:11-18"] },
    Pericope { title: "Road to Emmaus", refs: &["Mk 16:12-13", "Lk 24:13-35"] },
    Pericope { title: "Appearance to the disciples", refs: &["Lk 24:36-49", "Jn 20:19-23"] },
    Pericope { title: "Thomas", refs: &["Jn 20:24-29"] },
    Pericope { title: "Appearance by the Sea of Galilee", refs: &["Jn 21:1-25"] },
    Pericope { title: "The Great Commission", refs: &["Mt 28:16-20", "Mk 16:14-18"] },
    Pericope { title: "Ascension", refs: &["Mk 16:19-20", "Lk 24:50-53"] },
    // Old Testament parallels
    Pericope { title: "The Ten Commandments", refs: &["Ex 20:1-17", "Dt 5:6-21"] },
    Pericope { title: "David's song of deliverance", refs: &["2Sa 22", "Ps 18"] },
    Pericope { title: "David's census", refs: &["2Sa 24", "1Ch 21"] },
    Pericope { title: "Psalm of thanksgiving at the ark's return", refs: &["1Ch 16:8-36", "Ps 105:1-15", "Ps 96"] },
    Pericope { title: "The fool says there is no God", refs: &["Ps 14", "Ps 53"] },
    Pericope { title: "Hezekiah and Sennacherib", refs: &["2Ki 18:13-19:37", "2Ch 32:1-23", "Isa 36:1-37:38"] },
    Pericope { title: "Hezekiah's illness", refs: &["2Ki 20:1-11", "2Ch 32:24-26", "Isa 38:1-22"] },
    Pericope { title: "Envoys from Babylon", refs: &["2Ki 20:12-19", "Isa 39:1-8"] },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ranges() {
        let r = parse_range("Jn 18:28-19:16").unwrap();
        assert_eq!((r.book, r.start, r.end), (43, (18, 28), (19, 16)));
        let r = parse_range("Mk 8:31-9:1").unwrap();
        assert_eq!((r.start, r.end), ((8, 31), (9, 1)));
        let r = parse_range("2Sa 24").unwrap();
        assert_eq!((r.book, r.start, r.end), (10, (24, 1), (24, 999)));
        let r = parse_range("Mt 7:12").unwrap();
        assert_eq!((r.start, r.end), ((7, 12), (7, 12)));
    }

    #[test]
    fn all_refs_parse() {
        for p in PERICOPES {
            for r in p.refs {
                assert!(parse_range(r).is_some(), "bad ref {r} in {}", p.title);
            }
        }
    }

    #[test]
    fn finds_parallels() {
        let hits = find(Position { book: 43, chapter: 6, verse: 9 });
        assert!(hits.iter().any(|(p, _)| p.title == "Feeding the five thousand"));
    }
}
