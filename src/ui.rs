//! Rendering. Colors use the terminal's ANSI palette so the Omarchy theme applies.

use crate::app::{Action, App, Hit, Mode, ScrollArea, SideContent, Slot, TAB_NAMES};
use crate::bible::{Loc, Position, Translation};
use crate::harmony;
use crate::menu::{Button, MENUS};
use crate::plans;
use crate::resources;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap,
};
use unicode_width::UnicodeWidthStr;

const ACCENT: Color = Color::Blue;
const GUTTER: usize = 5;

/// Everything drawn also records the regions the mouse can click or scroll
/// into `app.hits`; `App::handle_mouse` resolves clicks against that list.
pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let mut hits: Vec<Hit> = Vec::new();
    let buttons = app.footer_buttons();
    let footer_h = if app.study.hide_toolbar {
        u16::from(matches!(app.mode, Mode::Search | Mode::GoTo))
    } else {
        button_rows(&buttons, area.width)
            .max(button_rows(&crate::menu::read_toolbar(), area.width))
            .clamp(1, 4)
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(footer_h),
        ])
        .split(area);

    draw_title(f, app, chunks[0], &mut hits);
    draw_body(f, app, chunks[1], &mut hits);
    draw_footer(f, app, chunks[2], &buttons, &mut hits);

    match app.mode {
        Mode::Books => draw_books(f, app, area, &mut hits),
        Mode::Chapters => draw_chapters(f, app, area, &mut hits),
        Mode::Translations => draw_translations(f, app, area, &mut hits),
        Mode::SearchResults => draw_search_results(f, app, area, &mut hits),
        Mode::Note => draw_note(f, app, area, &mut hits),
        Mode::Bookmarks => draw_bookmarks(f, app, area, &mut hits),
        Mode::Help => draw_help(f, app, area, &mut hits),
        Mode::Resources => draw_resources(f, app, area, &mut hits),
        Mode::Harmony => draw_harmony(f, app, area, &mut hits),
        Mode::Plans => draw_plans(f, app, area, &mut hits),
        Mode::DictSearch => draw_dict_search(f, app, area, &mut hits),
        Mode::DictEntry => draw_dict_entry(f, app, area, &mut hits),
        Mode::WordStudy => draw_word_study(f, app, area, &mut hits),
        Mode::Occurrences => draw_occurrences(f, app, area, &mut hits),
        Mode::Menu => draw_menu(f, app, area, &mut hits),
        Mode::Settings => draw_settings(f, app, area, &mut hits),
        _ => {}
    }
    app.hits = hits;
}

fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn accent() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

fn key(c: char) -> Action {
    Action::Key(KeyCode::Char(c), KeyModifiers::NONE)
}

fn hit(hits: &mut Vec<Hit>, rect: Rect, action: Action) {
    if rect.width > 0 && rect.height > 0 {
        hits.push(Hit { rect, action });
    }
}

/// A run of styled text segments, each optionally clickable.
type Segments = Vec<(String, Style, Option<Action>)>;

fn segments_width(segs: &Segments) -> u16 {
    segs.iter().map(|(t, _, _)| t.width() as u16).sum()
}

/// Turn segments into a Line and record a hit for each clickable segment
/// starting at column `x` on row `y`.
fn segments_line(segs: Segments, x: u16, y: u16, hits: &mut Vec<Hit>) -> Line<'static> {
    let mut spans = Vec::new();
    let mut cx = x;
    for (text, style, action) in segs {
        let w = text.width() as u16;
        if let Some(a) = action {
            hit(
                hits,
                Rect {
                    x: cx,
                    y,
                    width: w,
                    height: 1,
                },
                a,
            );
        }
        spans.push(Span::styled(text, style));
        cx += w;
    }
    Line::from(spans)
}

// ---------------------------------------------------------------------------
// Buttons and menus
// ---------------------------------------------------------------------------

fn button_style() -> Style {
    Style::default().fg(Color::White).bg(Color::DarkGray)
}

fn button_width(b: &Button) -> u16 {
    (b.label.width() + b.hint.width() + 3) as u16
}

/// Positions (row, x) relative to the footer origin, wrapping to new rows.
fn layout_buttons(buttons: &[Button], width: u16) -> (Vec<(u16, u16)>, u16) {
    let mut out = Vec::with_capacity(buttons.len());
    let mut row = 0u16;
    let mut x = 1u16;
    for b in buttons {
        let w = button_width(b);
        if x + w > width && x > 1 {
            row += 1;
            x = 1;
        }
        out.push((row, x));
        x += w + 1;
    }
    (out, row + 1)
}

fn button_rows(buttons: &[Button], width: u16) -> u16 {
    layout_buttons(buttons, width).1
}

fn render_button(f: &mut Frame, b: &Button, rect: Rect, hits: &mut Vec<Hit>) {
    let line = Line::from(vec![
        Span::styled(format!(" {} ", b.label), button_style()),
        Span::styled(format!("{} ", b.hint), button_style().fg(Color::Yellow)),
    ]);
    f.render_widget(Paragraph::new(line), rect);
    hit(hits, rect, Action::Key(b.code, b.mods));
}

/// Where each menu title sits in the title bar.
fn menu_label_rects(area: Rect) -> Vec<Rect> {
    let mut x = area.x + " OmaScripture ".width() as u16 + 1;
    let mut rects = Vec::new();
    for m in MENUS {
        let w = m.title.width() as u16 + 2;
        rects.push(Rect {
            x,
            y: area.y,
            width: w,
            height: 1,
        });
        x += w + 1;
    }
    rects
}

fn draw_menu(f: &mut Frame, app: &App, area: Rect, hits: &mut Vec<Hit>) {
    let title_row = Rect { height: 1, ..area };
    let rects = menu_label_rects(title_row);
    let m = &MENUS[app.menu.min(MENUS.len() - 1)];
    let label_w = m.items.iter().map(|i| i.label.width()).max().unwrap_or(10);
    let hint_w = m.items.iter().map(|i| i.hint.width()).max().unwrap_or(1);
    let w = (label_w + hint_w + 5) as u16;
    let h = (m.items.len() as u16 + 2).min(area.height.saturating_sub(1));
    let lr = rects[app.menu.min(rects.len() - 1)];
    let x = lr.x.min(area.x + area.width.saturating_sub(w));
    let r = Rect {
        x,
        y: area.y + 1,
        width: w,
        height: h,
    };

    hit(hits, area, Action::Dismiss);
    for (i, lr) in rects.iter().enumerate() {
        hit(hits, *lr, Action::Menu(i));
    }
    f.render_widget(Clear, r);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT));
    let inner = block.inner(r);
    f.render_widget(block, r);
    hit(hits, r, Action::None);
    for (i, it) in m.items.iter().enumerate() {
        if i as u16 >= inner.height {
            break;
        }
        let row = Rect {
            x: inner.x,
            y: inner.y + i as u16,
            width: inner.width,
            height: 1,
        };
        let selected = i == app.list_index;
        let base = if selected {
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default()
        };
        let line = Line::from(vec![
            Span::styled(format!(" {} ", pad(it.label, label_w)), base),
            Span::styled(
                format!("{} ", pad(it.hint, hint_w)),
                if selected {
                    base
                } else {
                    Style::default().fg(Color::Yellow)
                },
            ),
        ]);
        f.render_widget(Paragraph::new(line), row);
        hit(hits, row, Action::MenuItem(i));
    }
}

fn draw_title(f: &mut Frame, app: &App, area: Rect, hits: &mut Vec<Hit>) {
    let mut left: Segments = vec![
        (" OmaScripture ".into(), accent(), None),
        (" ".into(), Style::default(), None),
    ];
    for (i, m) in MENUS.iter().enumerate() {
        let open = app.mode == Mode::Menu && app.menu == i;
        let style = if open {
            accent().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        left.push((format!(" {} ", m.title), style, Some(Action::Menu(i))));
        left.push((" ".into(), Style::default(), None));
    }
    let left_w = segments_width(&left);
    let line = segments_line(left, area.x, area.y, hits);
    f.render_widget(Paragraph::new(line), area);

    let yellow = Style::default().fg(Color::Yellow);
    let mut right: Segments = Vec::new();
    if let Some(s) = app.status_text() {
        right.push((format!("{s}  "), yellow, None));
    } else if let Some(b) = &app.busy {
        right.push((format!("⟳ {b}  "), yellow, None));
    } else {
        if let Some(p) = &app.parallel
            && app.parallel_visible {
                right.push((
                    format!("∥ {}  ", p.abbreviation.to_uppercase()),
                    dim(),
                    Some(key('p')),
                ));
            }
        if let Some(plan) = &app.study.plan {
            let behind = plan.behind();
            if behind > 0 {
                right.push((
                    format!(
                        "plan: {behind} day{} behind  ",
                        if behind == 1 { "" } else { "s" }
                    ),
                    dim(),
                    Some(key('r')),
                ));
            }
        }
    }
    if let Some(t) = &app.translation {
        right.push((
            t.reference(app.loc),
            Style::default().add_modifier(Modifier::BOLD),
            Some(key(':')),
        ));
        right.push(("  ".into(), Style::default(), None));
        right.push((
            format!("{} ", t.abbreviation.to_uppercase()),
            dim(),
            Some(key('t')),
        ));
    }
    let right_w = segments_width(&right);
    let visible_w = right_w.min(area.width.saturating_sub(left_w + 1));
    if visible_w > 0 {
        let x = area.x + area.width - visible_w;
        let r = Rect {
            x,
            width: visible_w,
            ..area
        };
        let line = segments_line(right, x, area.y, hits);
        f.render_widget(Paragraph::new(line), r);
    }
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect, buttons: &[Button], hits: &mut Vec<Hit>) {
    match app.mode {
        Mode::Search | Mode::GoTo => {
            let (prefix, text) = if app.mode == Mode::Search {
                (" /", &app.search_query)
            } else {
                (" :", &app.filter)
            };
            let line = Line::from(vec![
                Span::styled(prefix, accent()),
                Span::raw(text.clone()),
            ]);
            f.render_widget(Paragraph::new(line), area);
            f.set_cursor_position((area.x + 2 + text.width() as u16, area.y));
            let total: u16 = buttons.iter().map(|b| button_width(b) + 1).sum();
            let mut x = area.x + area.width.saturating_sub(total);
            if x >= area.x + 4 + text.width() as u16 {
                for b in buttons {
                    let w = button_width(b);
                    render_button(
                        f,
                        b,
                        Rect {
                            x,
                            y: area.y,
                            width: w,
                            height: 1,
                        },
                        hits,
                    );
                    x += w + 1;
                }
            }
        }
        _ => {
            let (positions, _) = layout_buttons(buttons, area.width);
            for (b, (row, x)) in buttons.iter().zip(positions) {
                if row >= area.height {
                    break;
                }
                let w = button_width(b).min(area.width.saturating_sub(x));
                render_button(
                    f,
                    b,
                    Rect {
                        x: area.x + x,
                        y: area.y + row,
                        width: w,
                        height: 1,
                    },
                    hits,
                );
            }
        }
    }
}

fn draw_body(f: &mut Frame, app: &mut App, area: Rect, hits: &mut Vec<Hit>) {
    let Some(t) = app.translation.take() else {
        let msg = if app.busy.is_some() {
            "Loading…"
        } else {
            "No translation loaded.\n\nPress t or click Translation to choose one, or ? for help."
        };
        let p = Paragraph::new(msg)
            .alignment(Alignment::Center)
            .block(reading_block(
                Line::from(Span::styled(" OmaScripture ", accent())),
                false,
            ))
            .style(dim());
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
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    let position = t.position_from_loc(app.loc);
    let loc = app.loc;
    let study = app.study.clone();

    let mut scroll = app.main_pane.scroll;
    render_chapter(
        f,
        panes[0],
        &t,
        loc,
        Some(loc.verse),
        &study,
        &mut scroll,
        ScrollArea::Main,
        !app.focus_side,
        hits,
    );
    app.main_pane.scroll = scroll;

    let mut next = 1;
    if show_parallel {
        let p = app.parallel.take().unwrap();
        let mut pscroll = app.side_pane.scroll;
        match p.loc_from_position(position) {
            Some(ploc) => {
                render_chapter(
                    f,
                    panes[next],
                    &p,
                    ploc,
                    Some(ploc.verse),
                    &study,
                    &mut pscroll,
                    ScrollArea::Parallel,
                    false,
                    hits,
                );
            }
            None => {
                let title = pane_title(&p, None, ScrollArea::Parallel, panes[next], hits);
                let msg =
                    Paragraph::new("This passage is not available in the parallel translation.")
                        .wrap(Wrap { trim: true })
                        .style(dim())
                        .block(reading_block(title, false));
                f.render_widget(msg, panes[next]);
            }
        }
        app.side_pane.scroll = pscroll;
        app.parallel = Some(p);
        next += 1;
    }
    app.translation = Some(t);
    if show_side {
        draw_sidebar(f, app, panes[next], hits);
    }
}

fn reading_block(title: Line<'static>, focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if focused {
            Style::default().fg(ACCENT)
        } else {
            dim()
        })
        .title(title)
        .padding(Padding::horizontal(1))
}

/// Pane title with clickable chapter arrows, chapter picker and translation picker.
fn pane_title(
    t: &Translation,
    loc: Option<Loc>,
    pane: ScrollArea,
    area: Rect,
    hits: &mut Vec<Hit>,
) -> Line<'static> {
    let tkey = if pane == ScrollArea::Main { 't' } else { 'T' };
    let mut segs: Segments = vec![(" ◂ ".into(), accent(), Some(key('h')))];
    if let Some(l) = loc {
        segs.push((t.reference_chapter(l), accent(), Some(key('c'))));
    }
    segs.push((" ▸ ".into(), accent(), Some(key('l'))));
    segs.push((
        format!("· {} ", t.abbreviation.to_uppercase()),
        accent(),
        Some(key(tkey)),
    ));
    segments_line(segs, area.x + 1, area.y, hits)
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
    pane: ScrollArea,
    focused: bool,
    hits: &mut Vec<Hit>,
) {
    let primary = pane == ScrollArea::Main;
    let title = pane_title(t, Some(loc), pane, area, hits);
    let block = reading_block(title, focused && primary);
    let mut inner = block.inner(area);
    f.render_widget(block, area);
    if let Some(online) = &t.online {
        let ready = online.loaded == Some((loc.book, loc.chapter));
        if !ready || online.error.is_some() {
            let message = online.error.as_deref().unwrap_or("Loading chapter online…");
            f.render_widget(
                Paragraph::new(format!(
                    "{message}\nClick here or F5 to retry · t chooses another translation"
                ))
                .wrap(Wrap { trim: true })
                .style(dim()),
                inner,
            );
            hit(hits, inner, Action::Key(KeyCode::F(5), KeyModifiers::NONE));
            return;
        }
        if inner.height > 2 {
            let notice_height = ((online.notice.width() as u16 / inner.width.max(1)) + 1)
                .min(4)
                .min(inner.height - 1);
            let notice = Rect {
                x: inner.x,
                y: inner.y + inner.height - notice_height,
                width: inner.width,
                height: notice_height,
            };
            f.render_widget(
                Paragraph::new(online.notice.clone())
                    .wrap(Wrap { trim: true })
                    .style(dim()),
                notice,
            );
            inner.height -= notice_height;
        }
    }
    hit(hits, area, Action::Scroll(pane));
    hit(hits, inner, Action::FocusMain);
    if inner.width < GUTTER as u16 + 4 || inner.height == 0 {
        return;
    }
    let mut text_w = inner.width as usize - GUTTER;
    if study.text_width > 0 {
        text_w = text_w.min(study.text_width as usize);
    }
    let book = &t.books[loc.book];
    let chapter = &book.chapters[loc.chapter];

    let mut lines: Vec<Line> = Vec::new();
    let mut line_verse: Vec<Position> = Vec::new();
    let mut sel_range = (0usize, 0usize);
    for (vi, v) in chapter.verses.iter().enumerate() {
        let pos = Position {
            book: book.nr,
            chapter: chapter.chapter,
            verse: v.verse,
        };
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
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            dim()
        };
        let marker = if is_sel {
            Span::styled("▎", Style::default().fg(ACCENT))
        } else {
            Span::raw(" ")
        };

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
            badges.push(Span::styled(
                " ❖ bookmarked",
                Style::default().fg(Color::Magenta),
            ));
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
        line_verse.resize(lines.len(), pos);
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
    for (row, pos) in line_verse.iter().skip(*scroll).take(height).enumerate() {
        hit(
            hits,
            Rect {
                x: inner.x,
                y: inner.y + row as u16,
                width: inner.width,
                height: 1,
            },
            Action::Verse(*pos),
        );
    }
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
            f.render_widget(
                Paragraph::new(Span::styled(label, dim())),
                Rect {
                    x,
                    y,
                    width: lw,
                    height: 1,
                },
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Study sidebar
// ---------------------------------------------------------------------------

fn draw_sidebar(f: &mut Frame, app: &mut App, area: Rect, hits: &mut Vec<Hit>) {
    let focused = app.focus_side;
    const SHORT: [&str; 5] = ["", "Refs", "Words", "Comm", "Notes"];
    let short = area.width < 56;
    let mut segs: Segments = vec![(" ".into(), Style::default(), None)];
    for tab in 1..=4u8 {
        let style = if tab == app.sidebar { accent() } else { dim() };
        let name = if short {
            SHORT[tab as usize]
        } else {
            TAB_NAMES[tab as usize]
        };
        segs.push((format!("{tab} {name}"), style, Some(Action::Tab(tab))));
        segs.push((" ".into(), Style::default(), None));
    }
    segs.push(("✕ ".into(), Style::default().fg(Color::Red), Some(key('s'))));
    let title = segments_line(segs, area.x + 1, area.y, hits);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if focused {
            Style::default().fg(ACCENT)
        } else {
            dim()
        })
        .title(title)
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);
    f.render_widget(block, area);
    hit(hits, area, Action::Scroll(ScrollArea::Side));
    hit(hits, inner, Action::FocusSide);
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
                let marker = if is_sel {
                    Span::styled("▎", Style::default().fg(ACCENT))
                } else {
                    Span::raw(" ")
                };
                let label_style = if is_sel {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                } else {
                    Style::default().fg(Color::Yellow)
                };
                let mut head = vec![marker.clone(), Span::styled(r.label.clone(), label_style)];
                if r.source == "tsk" {
                    head.push(Span::styled("  TSK", dim()));
                }
                lines.push(Line::from(head));
                for l in wrap_text(&r.text, width.saturating_sub(3))
                    .into_iter()
                    .take(3)
                {
                    lines.push(Line::from(vec![
                        marker.clone(),
                        Span::raw("  "),
                        Span::styled(l, if is_sel { Style::default() } else { dim() }),
                    ]));
                }
                item_ranges.push((start, lines.len()));
            }
        }
        SideContent::Words(words) => {
            for (i, w) in words.iter().enumerate() {
                let start = lines.len();
                let is_sel = i == sel && focused;
                let marker = if is_sel {
                    Span::styled("▎", Style::default().fg(ACCENT))
                } else {
                    Span::raw(" ")
                };
                let word_style = if is_sel {
                    Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
                } else {
                    Style::default().add_modifier(Modifier::BOLD)
                };
                let mut first = vec![
                    marker.clone(),
                    Span::styled(w.text.clone(), word_style),
                    Span::raw("  "),
                    Span::styled(w.translit.clone(), dim()),
                ];
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
                        Line::from(vec![
                            marker.clone(),
                            Span::raw("  "),
                            Span::styled(
                                truncate(&w.gloss, gloss_w),
                                Style::default().fg(Color::Green),
                            ),
                        ]),
                        Line::from(vec![
                            marker.clone(),
                            Span::raw("  "),
                            Span::styled(meta, dim()),
                        ]),
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
    for (i, (s, e)) in item_ranges.iter().enumerate() {
        let top = (*s).max(scroll);
        let bottom = (*e).min(scroll + height);
        if top < bottom {
            let rect = Rect {
                x: inner.x,
                y: inner.y + (top - scroll) as u16,
                width: inner.width,
                height: (bottom - top) as u16,
            };
            hit(hits, rect, Action::SideItem(i));
        }
    }
    draw_percent(f, area, scroll, height, total);
}

// ---------------------------------------------------------------------------
// Popups
// ---------------------------------------------------------------------------

fn popup(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width.saturating_sub(2));
    let h = height.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

fn popup_block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(format!(" {title} "), accent()))
        .title_top(Line::from(Span::styled(" ✕ ", Style::default().fg(Color::Red))).right_aligned())
        .padding(Padding::horizontal(1))
}

/// Clear and frame a popup. Clicks outside it act as Esc, clicks on its body
/// are swallowed, and the ✕ in the corner closes it. Returns the inner area.
fn popup_frame(f: &mut Frame, screen: Rect, r: Rect, title: &str, hits: &mut Vec<Hit>) -> Rect {
    hit(hits, screen, Action::Dismiss);
    hit(hits, r, Action::None);
    f.render_widget(Clear, r);
    let block = popup_block(title);
    let inner = block.inner(r);
    f.render_widget(block, r);
    hit(
        hits,
        Rect {
            x: r.x + r.width.saturating_sub(4),
            y: r.y,
            width: 3,
            height: 1,
        },
        Action::Key(KeyCode::Esc, KeyModifiers::NONE),
    );
    inner
}

fn filter_line(label: &str, filter: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label} "), dim()),
        Span::raw(filter.to_string()),
        Span::styled("▏", Style::default().fg(ACCENT)),
    ])
}

/// Render a list and record a hit per visible row. `open` means a single
/// click opens the item; otherwise the first click only selects it.
fn render_list(
    f: &mut Frame,
    area: Rect,
    items: Vec<ListItem>,
    selected: usize,
    item_height: u16,
    open: bool,
    hits: &mut Vec<Hit>,
) {
    let n = items.len();
    let mut state = ListState::default().with_selected(Some(selected));
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED),
        )
        .highlight_symbol("▎");
    f.render_stateful_widget(list, area, &mut state);
    hit(hits, area, Action::Scroll(ScrollArea::List));
    let offset = state.offset();
    let ih = item_height.max(1);
    for row in 0..area.height {
        let idx = offset + (row / ih) as usize;
        if idx >= n {
            break;
        }
        let rect = Rect {
            x: area.x,
            y: area.y + row,
            width: area.width,
            height: 1,
        };
        hit(
            hits,
            rect,
            if open {
                Action::Open(idx)
            } else {
                Action::Select(idx)
            },
        );
    }
}

/// Scrollable text popup; clamps app.text_scroll.
#[allow(clippy::too_many_arguments)]
fn render_text_popup(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
    title: &str,
    header: Vec<Line<'static>>,
    body: &str,
    width: u16,
    height: u16,
    hits: &mut Vec<Hit>,
) {
    let r = popup(area, width, height);
    let inner = popup_frame(f, area, r, title, hits);
    hit(hits, inner, Action::Scroll(ScrollArea::Text));
    let mut lines: Vec<Line> = header;
    for l in wrap_paragraphs(body, inner.width as usize) {
        lines.push(Line::from(Span::raw(l)));
    }
    let total = lines.len();
    let h = inner.height as usize;
    let max = total.saturating_sub(h) as u16;
    app.text_scroll = app.text_scroll.min(max);
    let visible: Vec<Line> = lines
        .into_iter()
        .skip(app.text_scroll as usize)
        .take(h)
        .collect();
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

fn draw_books(f: &mut Frame, app: &App, area: Rect, hits: &mut Vec<Hit>) {
    let rows = app.book_rows();
    let r = popup(area, 40, 30);
    let inner = popup_frame(f, area, r, "Books", hits);
    let chunks = Layout::default()
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(inner);
    f.render_widget(
        Paragraph::new(filter_line("Filter:", &app.filter)),
        chunks[0],
    );
    let items: Vec<ListItem> = rows
        .iter()
        .map(|(i, name)| {
            let chapters = app
                .translation
                .as_ref()
                .map(|t| t.books[*i].chapters.len())
                .unwrap_or(0);
            ListItem::new(Line::from(vec![
                Span::raw(pad(name, 24)),
                Span::styled(format!("{chapters:>4} ch"), dim()),
            ]))
        })
        .collect();
    if items.is_empty() {
        f.render_widget(Paragraph::new("No matching books").style(dim()), chunks[1]);
    } else {
        render_list(
            f,
            chunks[1],
            items,
            app.list_index.min(rows.len() - 1),
            1,
            true,
            hits,
        );
    }
}

fn draw_chapters(f: &mut Frame, app: &mut App, area: Rect, hits: &mut Vec<Hit>) {
    let Some(t) = &app.translation else { return };
    let book = &t.books[app.book_pick];
    let n = book.chapters.len();
    let r = popup(area, 56, 20);
    let inner = popup_frame(f, area, r, &format!("{} · choose chapter", book.name), hits);
    hit(hits, inner, Action::Scroll(ScrollArea::Chapters));
    let cols = app.chapter_grid_cols(inner.width);
    app.last_grid_cols = cols;
    let rows_total = n.div_ceil(cols);
    let visible_rows = inner.height.saturating_sub(1) as usize;
    let sel_row = app.chapter_pick / cols;
    let first_row = sel_row
        .saturating_sub(visible_rows.saturating_sub(1))
        .min(rows_total.saturating_sub(visible_rows));
    let mut lines: Vec<Line> = Vec::new();
    for row in first_row..(first_row + visible_rows).min(rows_total) {
        let mut spans = Vec::new();
        for col in 0..cols {
            let idx = row * cols + col;
            if idx >= n {
                break;
            }
            let cell = Rect {
                x: inner.x + (col * 5) as u16,
                y: inner.y + (row - first_row) as u16,
                width: 4,
                height: 1,
            };
            hit(hits, cell, Action::Chapter(idx));
            let label = format!("{:>4}", book.chapters[idx].chapter);
            let style = if idx == app.chapter_pick {
                Style::default()
                    .fg(ACCENT)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
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
    let hint = if app.number_buffer.is_empty() {
        format!("{n} chapters")
    } else {
        format!("→ {}", app.number_buffer)
    };
    lines.push(Line::from(Span::styled(hint, dim())));
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_translations(f: &mut Frame, app: &App, area: Rect, hits: &mut Vec<Hit>) {
    let rows = app.translation_rows();
    let r = popup(area, 90, 34);
    let title = match app.picker_slot {
        Slot::Primary => "Translations",
        Slot::Parallel => "Parallel translation",
    };
    let inner = popup_frame(f, area, r, title, hits);
    let chunks = Layout::default()
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);
    f.render_widget(
        Paragraph::new(filter_line("Filter:", &app.filter)),
        chunks[0],
    );

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
            let online = crate::providers::is_online(&info.abbreviation);
            let detail = if online {
                crate::providers::status(&info.abbreviation)
            } else {
                info.language.clone()
            };
            let detail_w = if online { 22 } else { 16 };
            let name_w = inner
                .width
                .saturating_sub(20 + detail_w + tag.width() as u16)
                as usize;
            ListItem::new(Line::from(vec![
                Span::styled(format!("{mark} "), Style::default().fg(Color::Green)),
                Span::styled(
                    pad(&info.abbreviation, 15),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw(format!("{} ", pad(&info.translation, name_w))),
                Span::styled(pad(&detail, detail_w as usize), dim()),
                Span::styled(tag, Style::default().fg(ACCENT)),
            ]))
        })
        .collect();
    if items.is_empty() {
        let msg = if app.catalog.is_empty() {
            "Fetching translation list… (needs network on first run; F5 to retry)"
        } else {
            "No matching translations"
        };
        f.render_widget(Paragraph::new(msg).style(dim()), chunks[1]);
    } else {
        render_list(
            f,
            chunks[1],
            items,
            app.list_index.min(rows.len() - 1),
            1,
            false,
            hits,
        );
    }
    let footer = if let Some(status) = app.status_text() {
        status.to_string()
    } else if let Some((info, _)) = rows.get(app.list_index) {
        format!("{} · {}", info.abbreviation, info.description)
    } else {
        format!(
            "{} downloaded · offline + online · select, then open",
            app.installed.len()
        )
    };
    f.render_widget(Paragraph::new(footer).style(dim()), chunks[2]);
}

fn draw_search_results(f: &mut Frame, app: &App, area: Rect, hits: &mut Vec<Hit>) {
    let r = popup(area, 100, 34);
    let title = format!(
        "Search: \"{}\" · {} result{}{}",
        app.search_query,
        app.search_hits.len(),
        if app.search_hits.len() == 1 { "" } else { "s" },
        if app.search_hits.len() >= crate::app::SEARCH_LIMIT {
            " (limit reached)"
        } else {
            ""
        }
    );
    let inner = popup_frame(f, area, r, &title, hits);
    let text_w = inner.width.saturating_sub(24) as usize;
    let items: Vec<ListItem> = app
        .search_hits
        .iter()
        .map(|h| {
            ListItem::new(Line::from(vec![
                Span::styled(pad(&h.reference, 22), Style::default().fg(Color::Yellow)),
                Span::raw(truncate(&h.text, text_w)),
            ]))
        })
        .collect();
    render_list(f, inner, items, app.list_index, 1, true, hits);
}

fn draw_note(f: &mut Frame, app: &App, area: Rect, hits: &mut Vec<Hit>) {
    let Some(t) = &app.translation else { return };
    let r = popup(area, 80, 18);
    let inner = popup_frame(
        f,
        area,
        r,
        &format!("Note · {}", t.reference(app.loc)),
        hits,
    );
    let chunks = Layout::default()
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner);
    let verse = Paragraph::new(t.verse_text(app.loc).unwrap_or(""))
        .wrap(Wrap { trim: true })
        .style(dim().add_modifier(Modifier::ITALIC));
    f.render_widget(verse, chunks[0]);
    let editor = Paragraph::new(format!("{}▏", app.note_buffer)).wrap(Wrap { trim: false });
    f.render_widget(editor, chunks[1]);
}

fn draw_bookmarks(f: &mut Frame, app: &App, area: Rect, hits: &mut Vec<Hit>) {
    let rows = app.bookmark_rows();
    let r = popup(area, 100, 30);
    let inner = popup_frame(f, area, r, "Bookmarks", hits);
    if rows.is_empty() {
        f.render_widget(
            Paragraph::new("No bookmarks yet. Press m while reading to bookmark a verse.")
                .style(dim()),
            inner,
        );
        return;
    }
    let text_w = inner.width.saturating_sub(24) as usize;
    let items: Vec<ListItem> = rows
        .iter()
        .map(|(_, reference, text)| {
            ListItem::new(Line::from(vec![
                Span::styled(pad(reference, 22), Style::default().fg(Color::Magenta)),
                Span::raw(truncate(text, text_w)),
            ]))
        })
        .collect();
    render_list(
        f,
        inner,
        items,
        app.list_index.min(rows.len() - 1),
        1,
        true,
        hits,
    );
}

fn draw_resources(f: &mut Frame, app: &App, area: Rect, hits: &mut Vec<Hit>) {
    let r = popup(area, 110, 30);
    let inner = popup_frame(
        f,
        area,
        r,
        "Study resources · all public domain or Creative Commons",
        hits,
    );
    let chunks = Layout::default()
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(inner);
    let name_w = inner.width.saturating_sub(58) as usize;
    let items: Vec<ListItem> = resources::PACKS
        .iter()
        .map(|p| {
            let installed = resources::is_installed(p.id);
            let mark = if installed { "✓" } else { " " };
            let busy = app.busy.is_some()
                && app
                    .busy
                    .as_deref()
                    .map(|b| b.contains(p.name))
                    .unwrap_or(false);
            ListItem::new(Line::from(vec![
                Span::styled(format!("{mark} "), Style::default().fg(Color::Green)),
                Span::styled(pad(p.kind.label(), 17), Style::default().fg(Color::Yellow)),
                Span::raw(pad(p.name, name_w)),
                Span::styled(pad(p.size, 8), dim()),
                Span::styled(pad(p.license, 26), dim()),
                Span::styled(
                    if busy { "⟳" } else { "" },
                    Style::default().fg(Color::Yellow),
                ),
            ]))
        })
        .collect();
    render_list(f, chunks[0], items, app.list_index, 1, false, hits);
    let p = &resources::PACKS[app.list_index.min(resources::PACKS.len() - 1)];
    let installed = resources::installed_ids().len();
    let desc = format!(
        "{}\n{installed}/{} installed · stored in {}",
        p.description,
        resources::PACKS.len(),
        resources::resources_dir().display()
    );
    f.render_widget(
        Paragraph::new(desc).style(dim()).wrap(Wrap { trim: true }),
        chunks[1],
    );
}

fn draw_harmony(f: &mut Frame, app: &App, area: Rect, hits: &mut Vec<Hit>) {
    let r = popup(area, 80, 26);
    let inner = popup_frame(f, area, r, "Parallel passages", hits);
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
        let style = if item.current {
            dim()
        } else {
            Style::default().fg(Color::Yellow)
        };
        let mut spans = vec![title, Span::styled(label, style)];
        if item.current {
            spans.push(Span::styled("  ← here", dim()));
        }
        items.push(ListItem::new(Line::from(spans)));
    }
    render_list(f, inner, items, app.list_index, 1, true, hits);
}

fn draw_plans(f: &mut Frame, app: &App, area: Rect, hits: &mut Vec<Hit>) {
    let r = popup(area, 80, 24);
    match &app.study.plan {
        None => {
            let inner = popup_frame(f, area, r, "Reading plans", hits);
            let items: Vec<ListItem> = plans::PLANS
                .iter()
                .map(|p| {
                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(
                                pad(p.name, 36),
                                Style::default().add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(format!("{} days", p.days), dim()),
                        ]),
                        Line::from(Span::styled(format!("  {}", p.description), dim())),
                    ])
                })
                .collect();
            render_list(f, inner, items, app.list_index, 2, false, hits);
        }
        Some(progress) => {
            let plan = plans::plan(&progress.id);
            let name = plan.map(|p| p.name).unwrap_or("Plan");
            let inner = popup_frame(f, area, r, &format!("Reading plan · {name}"), hits);
            let day = app.plan_view_day();
            let days = progress.days();
            let done = progress.done.contains(&(day as u32));
            let is_today = day == progress.current_day().min(days.saturating_sub(1));
            let chunks = Layout::default()
                .constraints([Constraint::Length(3), Constraint::Min(1)])
                .split(inner);
            let header = vec![
                Line::from(vec![
                    Span::styled(format!("Day {} of {days}", day + 1), accent()),
                    Span::raw("  "),
                    Span::styled(plans::format_day(progress.start_day.saturating_add(day as i64)), dim()),
                    Span::raw("  "),
                    Span::styled(
                        if is_today { "today" } else { "" },
                        Style::default().fg(Color::Green),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        if done { "✓ done" } else { "not yet done (x)" },
                        if done {
                            Style::default().fg(Color::Green)
                        } else {
                            dim()
                        },
                    ),
                ]),
                Line::from(Span::styled(
                    format!(
                        "{} of {days} days complete · {} behind schedule",
                        progress.done.len(),
                        progress.behind()
                    ),
                    dim(),
                )),
                Line::raw(""),
            ];
            f.render_widget(Paragraph::new(header), chunks[0]);
            let readings = plans::readings(&progress.id, day);
            let items: Vec<ListItem> = readings
                .iter()
                .map(|(b, c)| {
                    let label = app
                        .translation
                        .as_ref()
                        .and_then(|t| t.book_by_nr(*b).map(|i| format!("{} {c}", t.books[i].name)))
                        .unwrap_or(format!("{b}:{c}"));
                    ListItem::new(Line::from(Span::raw(label)))
                })
                .collect();
            render_list(
                f,
                chunks[1],
                items,
                app.list_index.min(readings.len().saturating_sub(1)),
                1,
                true,
                hits,
            );
        }
    }
}

fn draw_dict_search(f: &mut Frame, app: &App, area: Rect, hits: &mut Vec<Hit>) {
    let r = popup(area, 90, 30);
    let inner = popup_frame(f, area, r, "Dictionaries and encyclopedias", hits);
    let chunks = Layout::default()
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(inner);
    f.render_widget(
        Paragraph::new(filter_line("Look up:", &app.filter)),
        chunks[0],
    );
    let sources =
        resources::Store::dictionary_ids().len() + usize::from(resources::is_installed("uw-words"));
    if sources == 0 {
        f.render_widget(Paragraph::new("No dictionaries installed. Press R to add Easton's, Smith's, ISBE, Nave's, Hitchcock's or unfoldingWord Translation Words.").wrap(Wrap { trim: true }).style(dim()), chunks[1]);
        return;
    }
    if app.dict_results.is_empty() {
        let msg = if app.filter.is_empty() {
            format!(
                "Type a name, place or topic. Searching {sources} source{}.",
                if sources == 1 { "" } else { "s" }
            )
        } else {
            "No matches.".into()
        };
        f.render_widget(Paragraph::new(msg).style(dim()), chunks[1]);
        return;
    }
    let key_w = inner.width.saturating_sub(40) as usize;
    let items: Vec<ListItem> = app
        .dict_results
        .iter()
        .map(|(_, name, key)| {
            ListItem::new(Line::from(vec![
                Span::raw(pad(key, key_w)),
                Span::styled(truncate(name, 36), dim()),
            ]))
        })
        .collect();
    render_list(f, chunks[1], items, app.list_index, 1, true, hits);
}

fn draw_dict_entry(f: &mut Frame, app: &mut App, area: Rect, hits: &mut Vec<Hit>) {
    let Some((title, body)) = app.dict_entry.clone() else {
        return;
    };
    render_text_popup(f, app, area, &title, Vec::new(), &body, 96, 34, hits);
}

fn draw_word_study(f: &mut Frame, app: &mut App, area: Rect, hits: &mut Vec<Hit>) {
    let Some(ws) = &app.word_study else { return };
    let strong = ws.strong.clone();
    let mut header: Vec<Line<'static>> = Vec::new();

    let body = match &ws.entry {
        Some(e) => {
            header.push(Line::from(vec![
                Span::styled(
                    e.lemma.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(e.translit.clone(), dim()),
                Span::raw("  "),
                Span::styled(e.strong.clone(), Style::default().fg(Color::Yellow)),
                Span::raw("  "),
                Span::styled(e.morph.clone(), dim()),
            ]));
            header.push(Line::from(Span::styled(
                e.gloss.clone(),
                Style::default().fg(Color::Green),
            )));
            e.meaning.clone()
        }
        None => {
            let w = ws.word.as_ref();
            header.push(Line::from(vec![
                Span::styled(
                    w.map(|w| w.lemma.clone()).unwrap_or_default(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(strong.clone(), Style::default().fg(Color::Yellow)),
                Span::raw("  "),
                Span::styled(
                    w.map(|w| w.lemma_gloss.clone()).unwrap_or_default(),
                    Style::default().fg(Color::Green),
                ),
            ]));
            let lex = if strong.starts_with('G') {
                "Greek"
            } else {
                "Hebrew"
            };
            format!(
                "No lexicon entry loaded. Install the {lex} lexicon pack (R) to see definitions."
            )
        }
    };
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
    header.push(Line::from(Span::styled(
        "Press o or click Occurrences for every use of this word.",
        dim(),
    )));
    header.push(Line::raw(""));
    render_text_popup(
        f,
        app,
        area,
        &format!("Word study · {strong}"),
        header,
        &body,
        90,
        30,
        hits,
    );
}

fn draw_occurrences(f: &mut Frame, app: &App, area: Rect, hits: &mut Vec<Hit>) {
    let Some(ws) = &app.word_study else { return };
    let Some((total, list)) = &ws.occurrences else {
        return;
    };
    let r = popup(area, 90, 32);
    f.render_widget(Clear, r);
    let lemma = ws
        .entry
        .as_ref()
        .map(|e| e.lemma.clone())
        .unwrap_or_default();
    let title = format!(
        "{} {} · {total} occurrence{}{}",
        ws.strong,
        lemma,
        if *total == 1 { "" } else { "s" },
        if list.len() < *total && list.len() >= crate::app::OCCURRENCE_LIMIT {
            " (showing first 600 verses)"
        } else {
            ""
        }
    );
    let inner = popup_frame(f, area, r, &title, hits);
    if list.is_empty() {
        f.render_widget(
            Paragraph::new("No occurrences found in the installed interlinear packs.").style(dim()),
            inner,
        );
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
    render_list(f, inner, items, app.list_index, 1, true, hits);
}

fn draw_settings(f: &mut Frame, app: &App, area: Rect, hits: &mut Vec<Hit>) {
    use crate::settings::ROWS;
    let r = popup(area, 84, ROWS.len() as u16 + 9);
    let inner = popup_frame(f, area, r, "Settings", hits);
    let label_w = ROWS.iter().map(|r| r.label().width()).max().unwrap_or(20);
    let sel = app.list_index.min(ROWS.len() - 1);
    let chunks = Layout::default()
        .constraints([
            Constraint::Length(ROWS.len() as u16),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(inner);
    for (i, row) in ROWS.iter().enumerate() {
        if i as u16 >= chunks[0].height {
            break;
        }
        let line_rect = Rect {
            x: chunks[0].x,
            y: chunks[0].y + i as u16,
            width: chunks[0].width,
            height: 1,
        };
        let selected = i == sel;
        let marker = if selected {
            Span::styled("▎", Style::default().fg(ACCENT))
        } else {
            Span::raw(" ")
        };
        let label_style = if selected {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let value = app.setting_value(*row);
        let value_w = inner.width.saturating_sub(label_w as u16 + 8) as usize;
        let arrow = if selected {
            Style::default().fg(Color::Yellow)
        } else {
            dim()
        };
        let segs: Segments = vec![
            (String::new(), Style::default(), None),
            (
                format!("{}  ", pad(row.label(), label_w)),
                label_style,
                Some(Action::Adjust(i, 1)),
            ),
            ("◂ ".into(), arrow, Some(Action::Adjust(i, -1))),
            (
                pad(&value, value_w),
                if selected {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
                Some(Action::Adjust(i, 1)),
            ),
            (" ▸".into(), arrow, Some(Action::Adjust(i, 1))),
        ];
        let mut line = segments_line(segs, line_rect.x + 1, line_rect.y, hits);
        line.spans.insert(0, marker);
        f.render_widget(Paragraph::new(line), line_rect);
    }
    let help = Paragraph::new(ROWS[sel].help())
        .wrap(Wrap { trim: true })
        .style(dim());
    f.render_widget(help, chunks[2]);
    let foot = format!(
        "Saved to {}",
        crate::bible::data_dir().join("study.json").display()
    );
    f.render_widget(Paragraph::new(foot).style(dim()), chunks[3]);
}

fn draw_help(f: &mut Frame, app: &mut App, area: Rect, hits: &mut Vec<Hit>) {
    let key = |k: &str, d: &str| {
        Line::from(vec![
            Span::styled(format!("  {k:<20}"), Style::default().fg(Color::Yellow)),
            Span::raw(d.to_string()),
        ])
    };
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
        key(
            "1 / 2 / 3 / 4",
            "cross-references / interlinear / commentary / notes",
        ),
        key("Tab", "focus the sidebar (j/k move, Enter open, Esc back)"),
        key("w", "interlinear words for this verse (Enter = word study)"),
        key("C", "cycle through installed commentaries"),
        Line::raw(""),
        head("Study tools"),
        key("D", "look up a name, place or topic in the dictionaries"),
        key("P", "parallel passages (Gospel harmony)"),
        key("r", "reading plans"),
        key("R", "install / remove study resources"),
        key(
            "/",
            "search the whole translation (\"quoted\" = exact phrase)",
        ),
        key("n / N", "next / previous search result"),
        key("m / B", "bookmark verse / list bookmarks"),
        key("x", "cycle highlight: yellow, green, blue, red, none"),
        key("e", "write a note on the verse (Ctrl+S saves)"),
        key("y", "copy verse and reference to clipboard"),
        Line::raw(""),
        head("General"),
        key(
            ", or S",
            "settings: default translation, sidebar, start page, text width",
        ),
        key("F10", "open the menu bar"),
        key("?", "this help"),
        key("q", "quit (position and study data are saved)"),
        Line::raw(""),
        head("Mouse"),
        key(
            "click",
            "select a verse, open a menu or button, pick a list item",
        ),
        key("click again", "open the selected translation or resource"),
        key("wheel", "scroll the text, sidebar, lists and popups"),
        key("right click", "close a popup (same as Esc)"),
        key(
            "shift + drag",
            "select terminal text while the mouse is captured",
        ),
        Line::raw(""),
        Line::from(Span::styled(
            format!("Data lives in {}", crate::bible::data_dir().display()),
            dim(),
        )),
        Line::from(Span::styled(
            "Texts from getbible.net; study resources from STEPBible, CrossWire, OpenBible.info and unfoldingWord.",
            dim(),
        )),
    ];
    render_text_popup(
        f,
        app,
        area,
        "OmaScripture · keys and mouse",
        lines,
        "",
        80,
        50,
        hits,
    );
}
