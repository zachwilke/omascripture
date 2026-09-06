//! Rendering. Colors use the terminal's ANSI palette so the Omarchy theme applies.

use crate::app::{App, Mode, SideContent, Slot, TAB_NAMES};
use crate::bible::{Loc, Position, Translation};
use crate::harmony;
use crate::plans;
use crate::resources;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

const ACCENT: Color = Color::Blue;
const GUTTER: usize = 5;

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    draw_title(f, app, chunks[0]);
    draw_body(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);

    match app.mode {
        Mode::Books => draw_books(f, app, area),
        Mode::Chapters => draw_chapters(f, app, area),
        Mode::Translations => draw_translations(f, app, area),
        Mode::SearchResults => draw_search_results(f, app, area),
        Mode::Note => draw_note(f, app, area),
        Mode::Bookmarks => draw_bookmarks(f, app, area),
        Mode::Help => draw_help(f, app, area),
        Mode::Resources => draw_resources(f, app, area),
        Mode::Harmony => draw_harmony(f, app, area),
        Mode::Plans => draw_plans(f, app, area),
        Mode::DictSearch => draw_dict_search(f, app, area),
        Mode::DictEntry => draw_dict_entry(f, app, area),
        Mode::WordStudy => draw_word_study(f, app, area),
        Mode::Occurrences => draw_occurrences(f, app, area),
        _ => {}
    }
}

fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn accent() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

fn draw_title(f: &mut Frame, app: &App, area: Rect) {
    let mut left = vec![Span::styled(" OmaScripture ", accent())];
    if let Some(t) = &app.translation {
        left.push(Span::raw("│ "));
        left.push(Span::styled(t.reference(app.loc), Style::default().add_modifier(Modifier::BOLD)));
        left.push(Span::raw("  "));
        left.push(Span::styled(t.translation.clone(), dim()));
    }
    let right = match &app.busy {
        Some(b) => Span::styled(format!("⟳ {b} "), Style::default().fg(Color::Yellow)),
        None => {
            let mut s = String::new();
            if let Some(p) = &app.parallel {
                if app.parallel_visible {
                    s.push_str(&format!("∥ {}  ", p.abbreviation.to_uppercase()));
                }
            }
            if let Some(plan) = &app.study.plan {
                let behind = plan.behind();
                if behind > 0 {
                    s.push_str(&format!("plan: {behind} day{} behind  ", if behind == 1 { "" } else { "s" }));
                }
            }
            s.push_str("? help ");
            Span::styled(s, dim())
        }
    };
    let right_w = right.content.width() as u16;
    f.render_widget(Paragraph::new(Line::from(left)), area);
    if area.width > right_w {
        let r = Rect { x: area.x + area.width - right_w, width: right_w, ..area };
        f.render_widget(Paragraph::new(Line::from(right)), r);
    }
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let line = match app.mode {
        Mode::Search => {
            let l = Line::from(vec![Span::styled(" /", accent()), Span::raw(app.search_query.clone())]);
            f.set_cursor_position((area.x + 2 + app.search_query.width() as u16, area.y));
            l
        }
        Mode::GoTo => {
            let l = Line::from(vec![Span::styled(" :", accent()), Span::raw(app.filter.clone())]);
            f.set_cursor_position((area.x + 2 + app.filter.width() as u16, area.y));
            l
        }
        _ => {
            if let Some(s) = app.status_text() {
                Line::from(Span::styled(format!(" {s}"), Style::default().fg(Color::Yellow)))
            } else {
                let hints = match app.mode {
                    Mode::Read if app.focus_side => " sidebar: j/k move  Enter open  1-4 tabs  C commentary  n/p verse  Tab/Esc back to text  s hide",
                    Mode::Read => " j/k verse  h/l chapter  b books  t/T translation  / search  : go to  s sidebar  1-4 tabs  w words  D dictionary  P parallels  r plan  R resources  ? help",
                    Mode::Books => " type to filter  ↑/↓ move  Enter choose  Esc back",
                    Mode::Chapters => " h/j/k/l move  digits jump  Enter open  b books  Esc back",
                    Mode::Translations => " type to filter  ↑/↓ move  Enter select/download  Del remove  F5 refresh  Esc back",
                    Mode::SearchResults => " j/k move  Enter open  / new search  Esc back  (n/N step results while reading)",
                    Mode::Note => " Ctrl+S save  Esc cancel  Ctrl+U clear  Enter newline",
                    Mode::Bookmarks => " j/k move  Enter open  d delete  Esc back",
                    Mode::Help => " j/k scroll  Esc close",
                    Mode::Resources => " j/k move  Enter install  d remove  a install all  Esc back",
                    Mode::Harmony => " j/k move  Enter open passage  Esc back",
                    Mode::Plans if app.study.plan.is_some() => " j/k move  Enter read  x done  h/l other day  t today  X stop plan  Esc back",
                    Mode::Plans => " j/k move  Enter start plan  Esc back",
                    Mode::DictSearch => " type to search  ↑/↓ move  Enter open  Esc back",
                    Mode::DictEntry => " j/k scroll  Esc back to results",
                    Mode::WordStudy => " j/k scroll  o occurrences  y copy  Esc back",
                    Mode::Occurrences => " j/k move  Enter open verse  Esc back",
                    _ => "",
                };
                Line::from(Span::styled(hints, dim()))
            }
        }
    };
    f.render_widget(Paragraph::new(line), area);
}

fn draw_body(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(t) = app.translation.take() else {
        let msg = if app.busy.is_some() {
            "Loading…"
        } else {
            "No translation loaded.\n\nPress t to choose one, or ? for help."
        };
        let p = Paragraph::new(msg).alignment(Alignment::Center).block(reading_block(" OmaScripture ", false)).style(dim());
        f.render_widget(p, area);
        return;
    };

    let show_parallel = app.parallel_visible && app.parallel.is_some();
    let show_side = app.sidebar != 0;
    let mut constraints = Vec::new();
    match (show_parallel, show_side) {
        (false, false) => constraints.push(Constraint::Percentage(100)),
        (true, false) => {
            constraints.push(Constraint::Percentage(50));
            constraints.push(Constraint::Percentage(50));
        }
        (false, true) => {
            constraints.push(Constraint::Percentage(60));
            constraints.push(Constraint::Percentage(40));
        }
        (true, true) => {
            constraints.push(Constraint::Percentage(34));
            constraints.push(Constraint::Percentage(33));
            constraints.push(Constraint::Percentage(33));
        }
    }
    let panes = Layout::default().direction(Direction::Horizontal).constraints(constraints).split(area);

    let position = t.position_from_loc(app.loc);
    let loc = app.loc;
    let study = app.study.clone();

    let title = format!(" {}  ·  {} ", t.reference_chapter(loc), t.abbreviation.to_uppercase());
    let mut scroll = app.main_pane.scroll;
    render_chapter(f, panes[0], &t, loc, Some(loc.verse), &study, &mut scroll, &title, true, !app.focus_side);
    app.main_pane.scroll = scroll;

    let mut next = 1;
    if show_parallel {
        let p = app.parallel.take().unwrap();
        let ptitle = format!(" {} ", p.abbreviation.to_uppercase());
        let mut pscroll = app.side_pane.scroll;
        match p.loc_from_position(position) {
            Some(ploc) => {
                let ptitle = format!(" {}  ·  {} ", p.reference_chapter(ploc), p.abbreviation.to_uppercase());
                render_chapter(f, panes[next], &p, ploc, Some(ploc.verse), &study, &mut pscroll, &ptitle, false, false);
            }
            None => {
                let msg = Paragraph::new("This passage is not available in the parallel translation.")
                    .wrap(Wrap { trim: true })
                    .style(dim())
                    .block(reading_block(&ptitle, false));
                f.render_widget(msg, panes[next]);
            }
        }
        app.side_pane.scroll = pscroll;
        app.parallel = Some(p);
        next += 1;
    }
    app.translation = Some(t);
    if show_side {
        draw_sidebar(f, app, panes[next]);
    }
}

fn reading_block(title: &str, focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if focused { Style::default().fg(ACCENT) } else { dim() })
        .title(Span::styled(title.to_string(), accent()))
        .padding(Padding::horizontal(1))
}

impl Translation {
    pub fn reference_chapter(&self, l: Loc) -> String {
        let b = &self.books[l.book];
        format!("{} {}", b.name, b.chapters[l.chapter].chapter)
    }
}

/// Word-wrap `text` to `width` columns.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0usize;
    for word in text.split_whitespace() {
        let w = word.width();
        if cur_w == 0 {
            cur.push_str(word);
            cur_w = w;
        } else if cur_w + 1 + w <= width {
            cur.push(' ');
            cur.push_str(word);
            cur_w += 1 + w;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur.push_str(word);
            cur_w = w;
        }
        while cur_w > width {
            let mut acc = 0;
            let mut cut = cur.len();
            for (i, ch) in cur.char_indices() {
                let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
                if acc + cw > width {
                    cut = i;
                    break;
                }
                acc += cw;
            }
            if cut == 0 {
                break;
            }
            let rest = cur.split_off(cut);
            lines.push(std::mem::replace(&mut cur, rest));
            cur_w = cur.width();
        }
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Wrap a multi-paragraph text, preserving blank lines.
fn wrap_paragraphs(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        if para.trim().is_empty() {
            out.push(String::new());
        } else {
            out.extend(wrap_text(para, width));
        }
    }
    out
}

fn highlight_color(idx: u8) -> Option<Color> {
    match idx {
        1 => Some(Color::Yellow),
        2 => Some(Color::Green),
        3 => Some(Color::Cyan),
        4 => Some(Color::Red),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn render_chapter(
    f: &mut Frame,
    area: Rect,
    t: &Translation,
    loc: Loc,
    selected: Option<usize>,
    study: &crate::study::Study,
    scroll: &mut usize,
    title: &str,
    primary: bool,
    focused: bool,
) {
    let block = reading_block(title, focused && primary);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width < GUTTER as u16 + 4 || inner.height == 0 {
        return;
    }
    let text_w = inner.width as usize - GUTTER;
    let book = &t.books[loc.book];
    let chapter = &book.chapters[loc.chapter];

    let mut lines: Vec<Line> = Vec::new();
    let mut sel_range = (0usize, 0usize);
    for (vi, v) in chapter.verses.iter().enumerate() {
        let pos = Position { book: book.nr, chapter: chapter.chapter, verse: v.verse };
        let is_sel = selected == Some(vi);
        let hl = highlight_color(study.highlight(pos));
        let marked = study.is_bookmarked(pos);
        let note = study.note(pos);
        let start = lines.len();

        let mut text_style = Style::default();
        if let Some(c) = hl {
            text_style = text_style.fg(c);
        }
        if is_sel {
            text_style = text_style.add_modifier(Modifier::BOLD);
        }
        let num_style = if is_sel {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            dim()
        };
        let marker = if is_sel { Span::styled("▎", Style::default().fg(ACCENT)) } else { Span::raw(" ") };

        let wrapped = wrap_text(&v.text, text_w);
        for (li, l) in wrapped.into_iter().enumerate() {
            let mut spans = vec![marker.clone()];
            if li == 0 {
                spans.push(Span::styled(format!("{:>3} ", v.verse), num_style));
            } else {
                spans.push(Span::raw("    "));
            }
            spans.push(Span::styled(l, text_style));
            lines.push(Line::from(spans));
        }
        let mut badges = Vec::new();
        if marked {
            badges.push(Span::styled(" ❖ bookmarked", Style::default().fg(Color::Magenta)));
        }
        if let Some(n) = note {
            if primary {
                let note_lines = wrap_text(n, text_w.saturating_sub(2));
                for (i, nl) in note_lines.into_iter().enumerate() {
                    let prefix = if i == 0 { "    ✎ " } else { "      " };
                    lines.push(Line::from(vec![
                        marker.clone(),
                        Span::styled(prefix, Style::default().fg(Color::Green)),
                        Span::styled(nl, dim().add_modifier(Modifier::ITALIC)),
                    ]));
                }
            } else if badges.is_empty() {
                badges.push(Span::styled(" ✎ note", Style::default().fg(Color::Green)));
            }
        }
        if !badges.is_empty() && primary {
            let mut spans = vec![marker.clone(), Span::raw("   ")];
            spans.extend(badges);
            lines.push(Line::from(spans));
        }
        if is_sel {
            sel_range = (start, lines.len());
        }
    }

    let height = inner.height as usize;
    let total = lines.len();
    if selected.is_some() {
        let (s, e) = sel_range;
        let margin = 2usize;
        if s < *scroll + margin {
            *scroll = s.saturating_sub(margin);
        } else if e + margin > *scroll + height {
            *scroll = (e + margin).saturating_sub(height);
        }
    }
    *scroll = (*scroll).min(total.saturating_sub(height));

    let visible: Vec<Line> = lines.into_iter().skip(*scroll).take(height).collect();
    f.render_widget(Paragraph::new(visible), inner);
    draw_percent(f, area, *scroll, height, total);
}

fn draw_percent(f: &mut Frame, area: Rect, scroll: usize, height: usize, total: usize) {
    if total > height && area.height > 1 {
        let pct = ((scroll + height) * 100 / total).min(100);
        let label = format!(" {pct}% ");
        let lw = label.width() as u16;
        if area.width > lw + 2 {
            let x = area.x + area.width - lw - 2;
            let y = area.y + area.height - 1;
            f.render_widget(Paragraph::new(Span::styled(label, dim())), Rect { x, y, width: lw, height: 1 });
        }
    }
}

// ---------------------------------------------------------------------------
// Study sidebar
// ---------------------------------------------------------------------------

fn draw_sidebar(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus_side;
    const SHORT: [&str; 5] = ["", "Refs", "Words", "Comm", "Notes"];
    let short = area.width < 52;
    let mut title_spans = vec![Span::raw(" ")];
    for tab in 1..=4u8 {
        let style = if tab == app.sidebar { accent() } else { dim() };
        let name = if short { SHORT[tab as usize] } else { TAB_NAMES[tab as usize] };
        title_spans.push(Span::styled(format!("{tab} {name}"), style));
        title_spans.push(Span::raw(" "));
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if focused { Style::default().fg(ACCENT) } else { dim() })
        .title(Line::from(title_spans))
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width < 10 || inner.height == 0 {
        return;
    }
    let width = inner.width as usize;
    let height = inner.height as usize;
    let content = app.side_content().clone();
    let sel = app.side_index;

    // Build lines with item boundaries for list-like content.
    let mut lines: Vec<Line> = Vec::new();
    let mut item_ranges: Vec<(usize, usize)> = Vec::new();
    match &content {
        SideContent::Empty(msg) => {
            for l in wrap_paragraphs(msg, width) {
                lines.push(Line::from(Span::styled(l, dim())));
            }
        }
        SideContent::Refs(items) => {
            for (i, r) in items.iter().enumerate() {
                let start = lines.len();
                let is_sel = i == sel && focused;
                let marker = if is_sel { Span::styled("▎", Style::default().fg(ACCENT)) } else { Span::raw(" ") };
                let label_style = if is_sel { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD | Modifier::REVERSED) } else { Style::default().fg(Color::Yellow) };
                let mut head = vec![marker.clone(), Span::styled(r.label.clone(), label_style)];
                if r.source == "tsk" {
                    head.push(Span::styled("  TSK", dim()));
                }
                lines.push(Line::from(head));
                for l in wrap_text(&r.text, width.saturating_sub(3)).into_iter().take(3) {
                    lines.push(Line::from(vec![marker.clone(), Span::raw("  "), Span::styled(l, if is_sel { Style::default() } else { dim() })]));
                }
                item_ranges.push((start, lines.len()));
            }
        }
        SideContent::Words(words) => {
            for (i, w) in words.iter().enumerate() {
                let start = lines.len();
                let is_sel = i == sel && focused;
                let marker = if is_sel { Span::styled("▎", Style::default().fg(ACCENT)) } else { Span::raw(" ") };
                let word_style = if is_sel { Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED) } else { Style::default().add_modifier(Modifier::BOLD) };
                let mut first = vec![marker.clone(), Span::styled(w.text.clone(), word_style), Span::raw("  "), Span::styled(w.translit.clone(), dim())];
                if w.variant {
                    first.push(Span::styled(" (var.)", dim()));
                }
                lines.push(Line::from(first));
                let gloss_w = width.saturating_sub(3);
                let meta = format!("{}  {}", w.strong, w.morph);
                let gloss = if w.gloss.width() + meta.width() + 4 <= gloss_w {
                    vec![Line::from(vec![
                        marker.clone(),
                        Span::raw("  "),
                        Span::styled(w.gloss.clone(), Style::default().fg(Color::Green)),
                        Span::raw("  "),
                        Span::styled(meta, dim()),
                    ])]
                } else {
                    vec![
                        Line::from(vec![marker.clone(), Span::raw("  "), Span::styled(truncate(&w.gloss, gloss_w), Style::default().fg(Color::Green))]),
                        Line::from(vec![marker.clone(), Span::raw("  "), Span::styled(meta, dim())]),
                    ]
                };
                lines.extend(gloss);
                item_ranges.push((start, lines.len()));
            }
        }
        SideContent::Text { title, body } => {
            lines.push(Line::from(Span::styled(truncate(title, width), accent())));
            lines.push(Line::raw(""));
            for l in wrap_paragraphs(body, width) {
                lines.push(Line::from(Span::raw(l)));
            }
        }
    }

    let total = lines.len();
    let mut scroll = app.side_scroll;
    if let Some((s, e)) = item_ranges.get(sel).copied() {
        if s < scroll {
            scroll = s;
        } else if e > scroll + height {
            scroll = e.saturating_sub(height);
        }
    }
    scroll = scroll.min(total.saturating_sub(height));
    app.side_scroll = scroll;
    let visible: Vec<Line> = lines.into_iter().skip(scroll).take(height).collect();
    f.render_widget(Paragraph::new(visible), inner);
    draw_percent(f, area, scroll, height, total);
}

// ---------------------------------------------------------------------------
// Popups
// ---------------------------------------------------------------------------

fn popup(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width.saturating_sub(2));
    let h = height.min(area.height.saturating_sub(2));
    Rect { x: area.x + (area.width - w) / 2, y: area.y + (area.height - h) / 2, width: w, height: h }
}

fn popup_block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(format!(" {title} "), accent()))
        .padding(Padding::horizontal(1))
}

fn filter_line(label: &str, filter: &str) -> Line<'static> {
    Line::from(vec![Span::styled(format!("{label} "), dim()), Span::raw(filter.to_string()), Span::styled("▏", Style::default().fg(ACCENT))])
}

fn render_list(f: &mut Frame, area: Rect, items: Vec<ListItem>, selected: usize) {
    let mut state = ListState::default().with_selected(Some(selected));
    let list = List::new(items)
        .highlight_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD | Modifier::REVERSED))
        .highlight_symbol("▎");
    f.render_stateful_widget(list, area, &mut state);
}

/// Scrollable text popup; returns nothing, clamps app.text_scroll.
fn render_text_popup(f: &mut Frame, app: &mut App, area: Rect, title: &str, header: Vec<Line<'static>>, body: &str, width: u16, height: u16) {
    let r = popup(area, width, height);
    f.render_widget(Clear, r);
    let block = popup_block(title);
    let inner = block.inner(r);
    f.render_widget(block, r);
    let mut lines: Vec<Line> = header;
    for l in wrap_paragraphs(body, inner.width as usize) {
        lines.push(Line::from(Span::raw(l)));
    }
    let total = lines.len();
    let h = inner.height as usize;
    let max = total.saturating_sub(h) as u16;
    app.text_scroll = app.text_scroll.min(max);
    let visible: Vec<Line> = lines.into_iter().skip(app.text_scroll as usize).take(h).collect();
    f.render_widget(Paragraph::new(visible), inner);
    draw_percent(f, r, app.text_scroll as usize, h, total);
}

/// Pad or truncate to exactly `w` display columns.
fn pad(s: &str, w: usize) -> String {
    let t = truncate(s, w);
    let fill = w.saturating_sub(t.width());
    format!("{t}{}", " ".repeat(fill))
}

fn truncate(s: &str, w: usize) -> String {
    if s.width() <= w {
        return s.to_string();
    }
    let mut out = String::new();
    let mut acc = 0;
    for ch in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
        if acc + cw > w.saturating_sub(1) {
            break;
        }
        acc += cw;
        out.push(ch);
    }
    out.push('…');
    out
}

fn draw_books(f: &mut Frame, app: &App, area: Rect) {
    let rows = app.book_rows();
    let r = popup(area, 40, 30);
    f.render_widget(Clear, r);
    let block = popup_block("Books");
    let inner = block.inner(r);
    f.render_widget(block, r);
    let chunks = Layout::default().constraints([Constraint::Length(2), Constraint::Min(1)]).split(inner);
    f.render_widget(Paragraph::new(filter_line("Filter:", &app.filter)), chunks[0]);
    let items: Vec<ListItem> = rows
        .iter()
        .map(|(i, name)| {
            let chapters = app.translation.as_ref().map(|t| t.books[*i].chapters.len()).unwrap_or(0);
            ListItem::new(Line::from(vec![Span::raw(pad(name, 24)), Span::styled(format!("{chapters:>4} ch"), dim())]))
        })
        .collect();
    if items.is_empty() {
        f.render_widget(Paragraph::new("No matching books").style(dim()), chunks[1]);
    } else {
        render_list(f, chunks[1], items, app.list_index.min(rows.len() - 1));
    }
}

fn draw_chapters(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(t) = &app.translation else { return };
    let book = &t.books[app.book_pick];
    let n = book.chapters.len();
    let r = popup(area, 56, 20);
    f.render_widget(Clear, r);
    let block = popup_block(&format!("{} · choose chapter", book.name));
    let inner = block.inner(r);
    f.render_widget(block, r);
    let cols = app.chapter_grid_cols(inner.width);
    app.last_grid_cols = cols;
    let rows_total = n.div_ceil(cols);
    let visible_rows = inner.height.saturating_sub(1) as usize;
    let sel_row = app.chapter_pick / cols;
    let first_row = sel_row.saturating_sub(visible_rows.saturating_sub(1)).min(rows_total.saturating_sub(visible_rows));
    let mut lines: Vec<Line> = Vec::new();
    for row in first_row..(first_row + visible_rows).min(rows_total) {
        let mut spans = Vec::new();
        for col in 0..cols {
            let idx = row * cols + col;
            if idx >= n {
                break;
            }
            let label = format!("{:>4}", book.chapters[idx].chapter);
            let style = if idx == app.chapter_pick {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else if idx == app.loc.chapter && app.book_pick == app.loc.book {
                Style::default().fg(ACCENT)
            } else {
                Style::default()
            };
            spans.push(Span::styled(label, style));
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
    }
    let hint = if app.number_buffer.is_empty() { format!("{n} chapters") } else { format!("→ {}", app.number_buffer) };
    lines.push(Line::from(Span::styled(hint, dim())));
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_translations(f: &mut Frame, app: &App, area: Rect) {
    let rows = app.translation_rows();
    let r = popup(area, 90, 34);
    f.render_widget(Clear, r);
    let title = match app.picker_slot {
        Slot::Primary => "Translations",
        Slot::Parallel => "Parallel translation",
    };
    let block = popup_block(title);
    let inner = block.inner(r);
    f.render_widget(block, r);
    let chunks = Layout::default().constraints([Constraint::Length(2), Constraint::Min(1), Constraint::Length(1)]).split(inner);
    f.render_widget(Paragraph::new(filter_line("Filter:", &app.filter)), chunks[0]);

    let current = app.translation.as_ref().map(|t| t.abbreviation.clone());
    let parallel = app.parallel.as_ref().map(|t| t.abbreviation.clone());
    let items: Vec<ListItem> = rows
        .iter()
        .map(|(info, installed)| {
            let mark = if *installed { "✓" } else { " " };
            let mut tag = String::new();
            if current.as_deref() == Some(&info.abbreviation) {
                tag.push_str(" [reading]");
            }
            if parallel.as_deref() == Some(&info.abbreviation) {
                tag.push_str(" [parallel]");
            }
            let name_w = inner.width.saturating_sub(44) as usize;
            ListItem::new(Line::from(vec![
                Span::styled(format!("{mark} "), Style::default().fg(Color::Green)),
                Span::styled(pad(&info.abbreviation, 15), Style::default().fg(Color::Yellow)),
                Span::raw(format!("{} ", pad(&info.translation, name_w))),
                Span::styled(pad(&info.language, 16), dim()),
                Span::styled(tag, Style::default().fg(ACCENT)),
            ]))
        })
        .collect();
    if items.is_empty() {
        let msg = if app.catalog.is_empty() { "Fetching translation list… (needs network on first run; F5 to retry)" } else { "No matching translations" };
        f.render_widget(Paragraph::new(msg).style(dim()), chunks[1]);
    } else {
        render_list(f, chunks[1], items, app.list_index.min(rows.len() - 1));
    }
    let footer = format!("{} downloaded · {} available · source: getbible.net", app.installed.len(), app.catalog.len());
    f.render_widget(Paragraph::new(footer).style(dim()), chunks[2]);
}

fn draw_search_results(f: &mut Frame, app: &App, area: Rect) {
    let r = popup(area, 100, 34);
    f.render_widget(Clear, r);
    let title = format!(
        "Search: \"{}\" · {} result{}{}",
        app.search_query,
        app.search_hits.len(),
        if app.search_hits.len() == 1 { "" } else { "s" },
        if app.search_hits.len() >= crate::app::SEARCH_LIMIT { " (limit reached)" } else { "" }
    );
    let block = popup_block(&title);
    let inner = block.inner(r);
    f.render_widget(block, r);
    let text_w = inner.width.saturating_sub(24) as usize;
    let items: Vec<ListItem> = app
        .search_hits
        .iter()
        .map(|h| ListItem::new(Line::from(vec![Span::styled(pad(&h.reference, 22), Style::default().fg(Color::Yellow)), Span::raw(truncate(&h.text, text_w))])))
        .collect();
    render_list(f, inner, items, app.list_index);
}

fn draw_note(f: &mut Frame, app: &App, area: Rect) {
    let Some(t) = &app.translation else { return };
    let r = popup(area, 80, 18);
    f.render_widget(Clear, r);
    let block = popup_block(&format!("Note · {}", t.reference(app.loc)));
    let inner = block.inner(r);
    f.render_widget(block, r);
    let chunks = Layout::default().constraints([Constraint::Length(3), Constraint::Min(1)]).split(inner);
    let verse = Paragraph::new(t.verse_text(app.loc).unwrap_or("")).wrap(Wrap { trim: true }).style(dim().add_modifier(Modifier::ITALIC));
    f.render_widget(verse, chunks[0]);
    let editor = Paragraph::new(format!("{}▏", app.note_buffer)).wrap(Wrap { trim: false });
    f.render_widget(editor, chunks[1]);
}

fn draw_bookmarks(f: &mut Frame, app: &App, area: Rect) {
    let rows = app.bookmark_rows();
    let r = popup(area, 100, 30);
    f.render_widget(Clear, r);
    let block = popup_block("Bookmarks");
    let inner = block.inner(r);
    f.render_widget(block, r);
    if rows.is_empty() {
        f.render_widget(Paragraph::new("No bookmarks yet. Press m while reading to bookmark a verse.").style(dim()), inner);
        return;
    }
    let text_w = inner.width.saturating_sub(24) as usize;
    let items: Vec<ListItem> = rows
        .iter()
        .map(|(_, reference, text)| ListItem::new(Line::from(vec![Span::styled(pad(reference, 22), Style::default().fg(Color::Magenta)), Span::raw(truncate(text, text_w))])))
        .collect();
    render_list(f, inner, items, app.list_index.min(rows.len() - 1));
}

fn draw_resources(f: &mut Frame, app: &App, area: Rect) {
    let r = popup(area, 110, 30);
    f.render_widget(Clear, r);
    let block = popup_block("Study resources · all public domain or Creative Commons");
    let inner = block.inner(r);
    f.render_widget(block, r);
    let chunks = Layout::default().constraints([Constraint::Min(1), Constraint::Length(2)]).split(inner);
    let name_w = inner.width.saturating_sub(58) as usize;
    let items: Vec<ListItem> = resources::PACKS
        .iter()
        .map(|p| {
            let installed = resources::is_installed(p.id);
            let mark = if installed { "✓" } else { " " };
            let busy = app.busy.is_some() && app.busy.as_deref().map(|b| b.contains(p.name)).unwrap_or(false);
            ListItem::new(Line::from(vec![
                Span::styled(format!("{mark} "), Style::default().fg(Color::Green)),
                Span::styled(pad(p.kind.label(), 17), Style::default().fg(Color::Yellow)),
                Span::raw(pad(p.name, name_w)),
                Span::styled(pad(p.size, 8), dim()),
                Span::styled(pad(p.license, 26), dim()),
                Span::styled(if busy { "⟳" } else { "" }, Style::default().fg(Color::Yellow)),
            ]))
        })
        .collect();
    render_list(f, chunks[0], items, app.list_index);
    let p = &resources::PACKS[app.list_index.min(resources::PACKS.len() - 1)];
    let installed = resources::installed_ids().len();
    let desc = format!("{}\n{installed}/{} installed · stored in {}", p.description, resources::PACKS.len(), resources::resources_dir().display());
    f.render_widget(Paragraph::new(desc).style(dim()).wrap(Wrap { trim: true }), chunks[1]);
}

fn draw_harmony(f: &mut Frame, app: &App, area: Rect) {
    let r = popup(area, 80, 26);
    f.render_widget(Clear, r);
    let block = popup_block("Parallel passages");
    let inner = block.inner(r);
    f.render_widget(block, r);
    if app.harmony_items.is_empty() {
        f.render_widget(
            Paragraph::new("No parallel passages are catalogued for this verse.\n\nThe harmony covers the Gospels and major Old Testament parallels.").wrap(Wrap { trim: true }).style(dim()),
            inner,
        );
        return;
    }
    let mut items: Vec<ListItem> = Vec::new();
    let mut last_title = "";
    for item in &app.harmony_items {
        let title = if item.title != last_title {
            last_title = item.title;
            Span::styled(pad(item.title, 44), accent())
        } else {
            Span::raw(pad("", 44))
        };
        let label = harmony::label(&item.range);
        let style = if item.current { dim() } else { Style::default().fg(Color::Yellow) };
        let mut spans = vec![title, Span::styled(label, style)];
        if item.current {
            spans.push(Span::styled("  ← here", dim()));
        }
        items.push(ListItem::new(Line::from(spans)));
    }
    render_list(f, inner, items, app.list_index);
}

fn draw_plans(f: &mut Frame, app: &App, area: Rect) {
    let r = popup(area, 80, 24);
    f.render_widget(Clear, r);
    match &app.study.plan {
        None => {
            let block = popup_block("Reading plans");
            let inner = block.inner(r);
            f.render_widget(block, r);
            let items: Vec<ListItem> = plans::PLANS
                .iter()
                .map(|p| {
                    ListItem::new(vec![
                        Line::from(vec![Span::styled(pad(p.name, 36), Style::default().add_modifier(Modifier::BOLD)), Span::styled(format!("{} days", p.days), dim())]),
                        Line::from(Span::styled(format!("  {}", p.description), dim())),
                    ])
                })
                .collect();
            render_list(f, inner, items, app.list_index);
        }
        Some(progress) => {
            let plan = plans::plan(&progress.id);
            let name = plan.map(|p| p.name).unwrap_or("Plan");
            let block = popup_block(&format!("Reading plan · {name}"));
            let inner = block.inner(r);
            f.render_widget(block, r);
            let day = app.plan_view_day();
            let days = progress.days();
            let done = progress.done.contains(&(day as u32));
            let is_today = day == progress.current_day().min(days.saturating_sub(1));
            let chunks = Layout::default().constraints([Constraint::Length(3), Constraint::Min(1)]).split(inner);
            let header = vec![
                Line::from(vec![
                    Span::styled(format!("Day {} of {days}", day + 1), accent()),
                    Span::raw("  "),
                    Span::styled(plans::format_day(progress.start_day + day as i64), dim()),
                    Span::raw("  "),
                    Span::styled(if is_today { "today" } else { "" }, Style::default().fg(Color::Green)),
                    Span::raw("  "),
                    Span::styled(if done { "✓ done" } else { "not yet done (x)" }, if done { Style::default().fg(Color::Green) } else { dim() }),
                ]),
                Line::from(Span::styled(
                    format!("{} of {days} days complete · {} behind schedule", progress.done.len(), progress.behind()),
                    dim(),
                )),
                Line::raw(""),
            ];
            f.render_widget(Paragraph::new(header), chunks[0]);
            let readings = plans::readings(&progress.id, day);
            let items: Vec<ListItem> = readings
                .iter()
                .map(|(b, c)| {
                    let label = app.translation.as_ref().and_then(|t| t.book_by_nr(*b).map(|i| format!("{} {c}", t.books[i].name))).unwrap_or(format!("{b}:{c}"));
                    ListItem::new(Line::from(Span::raw(label)))
                })
                .collect();
            render_list(f, chunks[1], items, app.list_index.min(readings.len().saturating_sub(1)));
        }
    }
}

fn draw_dict_search(f: &mut Frame, app: &App, area: Rect) {
    let r = popup(area, 90, 30);
    f.render_widget(Clear, r);
    let block = popup_block("Dictionaries and encyclopedias");
    let inner = block.inner(r);
    f.render_widget(block, r);
    let chunks = Layout::default().constraints([Constraint::Length(2), Constraint::Min(1)]).split(inner);
    f.render_widget(Paragraph::new(filter_line("Look up:", &app.filter)), chunks[0]);
    let sources = resources::Store::dictionary_ids().len() + usize::from(resources::is_installed("uw-words"));
    if sources == 0 {
        f.render_widget(Paragraph::new("No dictionaries installed. Press R to add Easton's, Smith's, ISBE, Nave's, Hitchcock's or unfoldingWord Translation Words.").wrap(Wrap { trim: true }).style(dim()), chunks[1]);
        return;
    }
    if app.dict_results.is_empty() {
        let msg = if app.filter.is_empty() { format!("Type a name, place or topic. Searching {sources} source{}.", if sources == 1 { "" } else { "s" }) } else { "No matches.".into() };
        f.render_widget(Paragraph::new(msg).style(dim()), chunks[1]);
        return;
    }
    let key_w = inner.width.saturating_sub(40) as usize;
    let items: Vec<ListItem> = app
        .dict_results
        .iter()
        .map(|(_, name, key)| ListItem::new(Line::from(vec![Span::raw(pad(key, key_w)), Span::styled(truncate(name, 36), dim())])))
        .collect();
    render_list(f, chunks[1], items, app.list_index);
}

fn draw_dict_entry(f: &mut Frame, app: &mut App, area: Rect) {
    let Some((title, body)) = app.dict_entry.clone() else { return };
    render_text_popup(f, app, area, &title, Vec::new(), &body, 96, 34);
}

fn draw_word_study(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(ws) = &app.word_study else { return };
    let strong = ws.strong.clone();
    let mut header: Vec<Line<'static>> = Vec::new();
    let body;
    match &ws.entry {
        Some(e) => {
            header.push(Line::from(vec![
                Span::styled(e.lemma.clone(), Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled(e.translit.clone(), dim()),
                Span::raw("  "),
                Span::styled(e.strong.clone(), Style::default().fg(Color::Yellow)),
                Span::raw("  "),
                Span::styled(e.morph.clone(), dim()),
            ]));
            header.push(Line::from(Span::styled(e.gloss.clone(), Style::default().fg(Color::Green))));
            body = e.meaning.clone();
        }
        None => {
            let w = ws.word.as_ref();
            header.push(Line::from(vec![
                Span::styled(w.map(|w| w.lemma.clone()).unwrap_or_default(), Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled(strong.clone(), Style::default().fg(Color::Yellow)),
                Span::raw("  "),
                Span::styled(w.map(|w| w.lemma_gloss.clone()).unwrap_or_default(), Style::default().fg(Color::Green)),
            ]));
            let lex = if strong.starts_with('G') { "Greek" } else { "Hebrew" };
            body = format!("No lexicon entry loaded. Install the {lex} lexicon pack (R) to see definitions.");
        }
    }
    if let Some(w) = &ws.word {
        header.push(Line::from(vec![
            Span::styled("In this verse: ", dim()),
            Span::raw(w.text.clone()),
            Span::raw("  "),
            Span::styled(w.gloss.clone(), Style::default().fg(Color::Green)),
            Span::raw("  "),
            Span::styled(w.morph.clone(), dim()),
        ]));
    }
    header.push(Line::from(Span::styled("Press o for every occurrence of this word.", dim())));
    header.push(Line::raw(""));
    render_text_popup(f, app, area, &format!("Word study · {strong}"), header, &body, 90, 30);
}

fn draw_occurrences(f: &mut Frame, app: &App, area: Rect) {
    let Some(ws) = &app.word_study else { return };
    let Some((total, list)) = &ws.occurrences else { return };
    let r = popup(area, 90, 32);
    f.render_widget(Clear, r);
    let lemma = ws.entry.as_ref().map(|e| e.lemma.clone()).unwrap_or_default();
    let title = format!(
        "{} {} · {total} occurrence{}{}",
        ws.strong,
        lemma,
        if *total == 1 { "" } else { "s" },
        if list.len() < *total && list.len() >= crate::app::OCCURRENCE_LIMIT { " (showing first 600 verses)" } else { "" }
    );
    let block = popup_block(&title);
    let inner = block.inner(r);
    f.render_widget(block, r);
    if list.is_empty() {
        f.render_widget(Paragraph::new("No occurrences found in the installed interlinear packs.").style(dim()), inner);
        return;
    }
    let text_w = inner.width.saturating_sub(24) as usize;
    let items: Vec<ListItem> = list
        .iter()
        .map(|(p, gloss)| {
            let label = app.label(*p);
            let text = app.text_at(*p);
            ListItem::new(Line::from(vec![
                Span::styled(pad(&label, 20), Style::default().fg(Color::Yellow)),
                Span::styled(pad(gloss, 14), Style::default().fg(Color::Green)),
                Span::raw(" "),
                Span::styled(truncate(&text, text_w.saturating_sub(15)), dim()),
            ]))
        })
        .collect();
    render_list(f, inner, items, app.list_index);
}

fn draw_help(f: &mut Frame, app: &mut App, area: Rect) {
    let key = |k: &str, d: &str| Line::from(vec![Span::styled(format!("  {k:<20}"), Style::default().fg(Color::Yellow)), Span::raw(d.to_string())]);
    let head = |s: &str| Line::from(Span::styled(s.to_string(), accent()));
    let lines: Vec<Line<'static>> = vec![
        head("Navigate"),
        key("j / k  ↓ / ↑", "next / previous verse"),
        key("J / K  PgDn / PgUp", "jump 10 verses"),
        key("Ctrl+d / Ctrl+u", "jump 5 verses"),
        key("g / G", "first / last verse of chapter"),
        key("h / l  ← / →", "previous / next chapter"),
        key("[ / ]  H / L", "previous / next book"),
        key("b / c", "choose book / chapter"),
        key(": or o", "go to reference, e.g. John 3:16, Ps 23, 1 Jn 2"),
        key("u / Backspace", "go back to where you jumped from"),
        key("v", "verse of the day"),
        Line::raw(""),
        head("Translations"),
        key("t", "choose reading translation (downloads on demand)"),
        key("T", "choose a parallel translation"),
        key("p", "show / hide the parallel pane"),
        Line::raw(""),
        head("Study sidebar"),
        key("s", "show / hide the sidebar"),
        key("1 / 2 / 3 / 4", "cross-references / interlinear / commentary / notes"),
        key("Tab", "focus the sidebar (j/k move, Enter open, Esc back)"),
        key("w", "interlinear words for this verse (Enter = word study)"),
        key("C", "cycle through installed commentaries"),
        Line::raw(""),
        head("Study tools"),
        key("D", "look up a name, place or topic in the dictionaries"),
        key("P", "parallel passages (Gospel harmony)"),
        key("r", "reading plans"),
        key("R", "install / remove study resources"),
        key("/", "search the whole translation (\"quoted\" = exact phrase)"),
        key("n / N", "next / previous search result"),
        key("m / B", "bookmark verse / list bookmarks"),
        key("x", "cycle highlight: yellow, green, blue, red, none"),
        key("e", "write a note on the verse (Ctrl+S saves)"),
        key("y", "copy verse and reference to clipboard"),
        Line::raw(""),
        head("General"),
        key("?", "this help"),
        key("q", "quit (position and study data are saved)"),
        Line::raw(""),
        Line::from(Span::styled(format!("Data lives in {}", crate::bible::data_dir().display()), dim())),
        Line::from(Span::styled("Texts from getbible.net; study resources from STEPBible, CrossWire, OpenBible.info and unfoldingWord.", dim())),
    ];
    render_text_popup(f, app, area, "OmaScripture · keys", lines, "", 80, 44);
}
