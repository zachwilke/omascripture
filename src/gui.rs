//! Native desktop frontend. The terminal renderer is never involved here.
use crate::{
    app::{App, Mode, SideContent, Slot},
    bible::{Loc, Position, Translation},
    gui_theme::{Palette, ThemeSource, blend},
    plans, providers, resources,
    settings::Row,
    word_data::{self, LanguageData, WordChoice, WordReport},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use eframe::egui::{self, FontFamily, FontId, RichText, ScrollArea};
use std::time::Duration;

fn glyph_index(galley: &egui::Galley, point: egui::Vec2) -> Option<usize> {
    let mut offset = 0;
    for row in &galley.rows {
        for (i, glyph) in row.glyphs.iter().enumerate() {
            if glyph
                .logical_rect()
                .translate(row.pos.to_vec2())
                .contains(point.to_pos2())
            {
                return Some(offset + i);
            }
        }
        offset += row.char_count_including_newline();
    }
    None
}

pub fn run(app: App) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("OmaScripture")
            .with_app_id("io.github.zachwilke.OmaScripture")
            .with_inner_size([1380.0, 900.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "io.github.zachwilke.OmaScripture",
        options,
        Box::new(move |cc| {
            let desktop = Desktop::new(app);
            configure(&cc.egui_ctx, desktop.colors);
            Ok(Box::new(desktop))
        }),
    )
}

fn configure(ctx: &egui::Context, colors: Palette) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, path) in [
        ("reading", "/usr/share/fonts/noto/NotoSerif-Regular.ttf"),
        ("interface", "/usr/share/fonts/noto/NotoSans-Regular.ttf"),
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(name.into(), egui::FontData::from_owned(bytes).into());
            if name == "reading" {
                let mut fallback = fonts.families[&FontFamily::Proportional].clone();
                fallback.insert(0, name.into());
                fonts
                    .families
                    .insert(FontFamily::Name("reading".into()), fallback);
            } else {
                fonts
                    .families
                    .get_mut(&FontFamily::Proportional)
                    .unwrap()
                    .insert(0, name.into());
            }
        }
    }
    if !fonts
        .families
        .contains_key(&FontFamily::Name("reading".into()))
    {
        fonts.families.insert(
            FontFamily::Name("reading".into()),
            fonts.families[&FontFamily::Proportional].clone(),
        );
    }
    if let Ok(bytes) = std::fs::read("/usr/share/fonts/noto/NotoSansHebrew-Regular.ttf") {
        fonts
            .font_data
            .insert("hebrew".into(), egui::FontData::from_owned(bytes).into());
        for family in fonts.families.values_mut() {
            family.push("hebrew".into());
        }
    }
    ctx.set_fonts(fonts);
    theme(ctx, colors);
}
fn theme(ctx: &egui::Context, colors: Palette) {
    let mut style = egui::Style {
        visuals: if colors.light {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        },
        ..Default::default()
    };
    style.visuals.panel_fill = colors.panel;
    style.visuals.window_fill = colors.panel;
    style.visuals.extreme_bg_color = colors.background;
    style.visuals.faint_bg_color = colors.surface;
    style.visuals.override_text_color = Some(colors.foreground);
    style.visuals.hyperlink_color = colors.accent;
    style.visuals.warn_fg_color = colors.yellow;
    style.visuals.error_fg_color = colors.red;
    style.visuals.window_stroke = egui::Stroke::new(1.0_f32, colors.border);
    style.visuals.selection.bg_fill = colors.selection;
    style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, colors.foreground);
    for (widget, fill, stroke) in [
        (
            &mut style.visuals.widgets.noninteractive,
            colors.panel,
            colors.border,
        ),
        (
            &mut style.visuals.widgets.inactive,
            colors.surface,
            colors.border,
        ),
        (
            &mut style.visuals.widgets.hovered,
            blend(colors.surface, colors.accent, 0.15),
            colors.accent,
        ),
        (
            &mut style.visuals.widgets.active,
            colors.selection,
            colors.accent,
        ),
        (
            &mut style.visuals.widgets.open,
            colors.selection,
            colors.accent,
        ),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = egui::Stroke::new(1.0_f32, stroke);
        widget.fg_stroke = egui::Stroke::new(1.0_f32, colors.foreground);
    }
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.noninteractive,
    ] {
        widget.corner_radius = egui::CornerRadius::same(6);
    }
    style.visuals.window_corner_radius = egui::CornerRadius::same(12);
    style.spacing.item_spacing = egui::vec2(10.0, 9.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.interact_size.y = 32.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(15.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, FontId::proportional(25.0));
    ctx.set_style(style);
}

struct Desktop {
    onboarding_selection: String,
    onboarding_waiting: bool,
    onboarding_filter: String,
    show_about: bool,
    app: App,
    colors: Palette,
    theme_source: ThemeSource,
    reference: String,
    book_filter: String,
    last_book: Option<u32>,
    last_follow: Option<(Position, bool)>,
    keys: Option<providers::Config>,
    provider_error: Option<String>,
    show_keys: bool,
    api_id: String,
    api_name: String,
    allow_close: bool,
    language: LanguageData,
    word_choice: Option<WordChoice>,
    pending_word: Option<(Position, resources::Word)>,
    pending_search: Option<String>,
    report_pending: Option<(
        String,
        std::sync::mpsc::Receiver<Result<WordReport, String>>,
    )>,
    report: Option<(String, Result<WordReport, String>)>,
}
impl Desktop {
    fn new(app: App) -> Self {
        let mut theme_source = ThemeSource::new();
        let colors = theme_source.resolve(&app.study.gui_theme, app.study.gui_light);
        Self {
            onboarding_selection: app
                .study
                .translation
                .clone()
                .unwrap_or_else(|| "web".into()),
            onboarding_waiting: false,
            onboarding_filter: String::new(),
            show_about: false,
            app,
            colors,
            theme_source,
            reference: String::new(),
            book_filter: String::new(),
            last_book: None,
            last_follow: None,
            keys: None,
            provider_error: None,
            show_keys: false,
            api_id: String::new(),
            api_name: String::new(),
            allow_close: false,
            language: LanguageData::default(),
            word_choice: None,
            pending_word: None,
            pending_search: None,
            report_pending: None,
            report: None,
        }
    }
    fn key(&mut self, code: KeyCode) {
        self.app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }
    fn command(&mut self, c: char) {
        self.app.mode = Mode::Read;
        self.app.focus_side = false;
        self.key(KeyCode::Char(c));
    }
    fn select_row(&mut self, i: usize) {
        self.app.list_index = i;
        self.key(KeyCode::Enter);
    }
    fn persist(&mut self) {
        self.app.save();
    }

    fn export_recovery(&mut self) {
        self.app.save(); // Capture the latest position and unfinished note.
        self.app.study.load_warning = Some(match self.app.study.export_recovery() {
            Ok(path) => format!("Recovery copy saved to {}", path.display()),
            Err(e) => format!("Could not export recovery copy: {e}"),
        });
    }

    fn reference_input(&mut self, ui: &mut egui::Ui, width: f32) {
        let response = ui.add(
            egui::TextEdit::singleline(&mut self.reference)
                .id(egui::Id::new("reference-input"))
                .hint_text("Go to a passage, e.g. John 3:16")
                .desired_width(width),
        );
        if ui.button("Go").clicked()
            || response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
        {
            self.app.goto_reference(&self.reference.clone());
        }
    }

    fn show(&mut self, ctx: &egui::Context) {
        if !self.allow_close && ctx.input(|i| i.viewport().close_requested()) && !self.app.save() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        let colors = self
            .theme_source
            .resolve(&self.app.study.gui_theme, self.app.study.gui_light);
        if colors != self.colors {
            self.colors = colors;
            theme(ctx, colors);
        }
        if self.app.mode == Mode::Read && !self.app.onboarding {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
                self.command('/');
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) {
                self.command(',');
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::L)) {
                ctx.memory_mut(|m| m.request_focus(egui::Id::new("reference-input")));
            }
        }
        self.app.poll_messages();
        self.poll_word_report();
        if let Some(error) = self
            .app
            .save_error
            .clone()
            .or_else(|| self.app.study.load_warning.clone())
        {
            egui::TopBottomPanel::top("storage-warning").show(ctx, |ui| {
                ui.colored_label(self.colors.red, error);
                ui.horizontal(|ui| {
                    if ui.button("Retry save").clicked() {
                        self.app.save();
                    }
                    if ui.button("Export recovery copy").clicked() {
                        self.export_recovery();
                    }
                });
                if self.app.save_error.is_some()
                    && let Some(message) = &self.app.study.load_warning
                {
                    ui.label(message);
                }
                if self.app.save_error.is_none() && ui.button("Dismiss notice").clicked() {
                    self.app.study.load_warning = None;
                }
                if self.app.save_error.is_some() && ui.button("Close without saving").clicked() {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        }
        if self.app.onboarding {
            egui::CentralPanel::default().show(ctx, |ui| self.onboarding(ui));
            self.about(ctx);
            ctx.request_repaint_after(if self.app.background_work_pending() {
                Duration::from_millis(150)
            } else {
                Duration::from_secs(1)
            });
            return;
        }
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(16),
            )
            .show(ctx, |ui| {
                let compact = ui.available_width() < 1050.0;
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("OmaScripture")
                            .size(22.0)
                            .strong()
                            .color(self.colors.accent),
                    );
                    ui.add_space(12.0);
                    if ui
                        .button("Back")
                        .on_hover_text("Return to the previous passage")
                        .clicked()
                    {
                        self.command('u');
                    }
                    if ui.button("‹").on_hover_text("Previous chapter").clicked() {
                        self.command('h');
                    }
                    if ui.button("›").on_hover_text("Next chapter").clicked() {
                        self.command('l');
                    }
                    if !compact {
                        self.reference_input(ui, 230.0);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Settings").clicked() {
                            self.command(',');
                        }
                        if ui
                            .selectable_label(self.app.sidebar != 0, "Study")
                            .clicked()
                        {
                            self.command('s');
                            self.persist();
                        }
                        if ui
                            .selectable_label(self.app.parallel_visible, "Parallel")
                            .clicked()
                        {
                            self.command('p');
                            self.persist();
                        }
                    });
                });
                if compact {
                    ui.horizontal(|ui| {
                        self.reference_input(ui, (ui.available_width() - 70.0).max(200.0))
                    });
                }
            });
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(egui::Margin::symmetric(20, 9)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(busy) = self.app.busy.clone() {
                        ui.spinner();
                        ui.label(busy);
                    } else if let Some(status) = self.app.status_text() {
                        ui.label(RichText::new(status).color(self.colors.accent));
                    } else {
                        ui.label(
                            RichText::new("A little time in the Word.").color(self.colors.muted),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("About").on_hover_text("Made by Zach Wilke").clicked() {
                            self.show_about = true;
                        }
                        if let Some(t) = &self.app.translation {
                            ui.label(
                                RichText::new(format!(
                                    "{}  ·  {}",
                                    t.reference(self.app.loc),
                                    t.abbreviation.to_uppercase()
                                ))
                                .color(self.colors.muted),
                            );
                        }
                    });
                });
            });
        egui::SidePanel::left("navigation")
            .resizable(true)
            .default_width(210.0)
            .width_range(175.0..=310.0)
            .show(ctx, |ui| self.navigation(ui));
        if self.app.sidebar != 0 {
            egui::SidePanel::right("study")
                .resizable(true)
                .default_width(335.0)
                .width_range(250.0..=550.0)
                .show(ctx, |ui| self.study_panel(ui));
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(self.colors.background)
                    .inner_margin(22),
            )
            .show(ctx, |ui| self.reading(ui));
        self.dialog(ctx);
        self.about(ctx);
        // Input/animations still repaint immediately. Idle frames only service
        // theme changes and autosave; pending workers keep their short poll.
        ctx.request_repaint_after(
            if self.app.background_work_pending() || self.report_pending.is_some() {
                Duration::from_millis(150)
            } else {
                Duration::from_secs(1)
            },
        );
        if self.app.should_quit {
            if self.app.save() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.app.should_quit = false;
            }
        }
    }

    fn onboarding(&mut self, ui: &mut egui::Ui) {
        let ready = self.app.translation.as_ref().is_some_and(|t| {
            t.abbreviation == self.onboarding_selection
                && t.online.as_ref().is_none_or(|o| {
                    o.error.is_none() && o.loaded == Some((self.app.loc.book, self.app.loc.chapter))
                })
        });
        if self.onboarding_waiting && ready {
            self.onboarding_waiting = false;
            if self.app.finish_onboarding() {
                ui.ctx().request_repaint();
                return;
            }
        }
        if !self.app.background_work_pending() {
            self.onboarding_waiting = false;
        }
        ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.set_max_width(640.0);
                ui.add_space(28.0);
                ui.label(RichText::new("Welcome to OmaScripture").size(30.0).color(self.colors.accent));
                ui.add_space(8.0);
                ui.heading("Choose your default Bible");
                ui.label("Make yourself at home. You can change this anytime in Settings.");
                ui.add_space(18.0);
                ui.add_enabled_ui(!self.onboarding_waiting, |ui| {
                    for (id, name, description) in [
                        ("web", "World English Bible", "Modern English · download once, read offline"),
                        ("kjv", "King James Version", "Classic English · offline reading and linked word studies"),
                        ("net", "NET Bible", "Modern English · reads online, internet required"),
                        ("nlt", "New Living Translation", "Easy-to-read English · reads online, internet required"),
                    ] {
                        let detail = if crate::bible::is_installed(id) { "Already downloaded · available offline" } else { description };
                        if ui.add(egui::Button::new(format!("{name}\n{detail}"))
                            .selected(self.onboarding_selection == id)
                            .min_size(egui::vec2(ui.available_width(), 60.0))).clicked() {
                            self.onboarding_selection = id.into();
                        }
                    }
                    ui.collapsing("More translations", |ui| {
                        if self.app.catalog.is_empty() {
                            ui.label("Load the catalog to choose another offline edition or language.");
                            if ui.add_enabled(!self.app.background_work_pending(), egui::Button::new("Load translation list")).clicked() {
                                self.app.refresh_catalog();
                            }
                        } else {
                            ui.add(egui::TextEdit::singleline(&mut self.onboarding_filter).hint_text("Find a Bible or language…"));
                            let filter = self.onboarding_filter.to_lowercase();
                            let mut found = false;
                            ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                                for edition in &self.app.catalog {
                                    if providers::is_online(&edition.abbreviation) { continue; }
                                    let label = format!("{} · {}", edition.translation, edition.language);
                                    if !filter.is_empty() && !label.to_lowercase().contains(&filter)
                                        && !edition.abbreviation.to_lowercase().contains(&filter) { continue; }
                                    found = true;
                                    ui.selectable_value(&mut self.onboarding_selection, edition.abbreviation.clone(), label);
                                }
                            });
                            if !found { ui.label("No matches. Try another name or language."); }
                        }
                        ui.small("Editions that need a provider key can be added in Settings after setup.");
                    });
                });
                ui.add_space(16.0);
                if self.onboarding_waiting {
                    ui.spinner();
                    ui.label(self.app.busy.as_deref().unwrap_or("Preparing your Bible…"));
                } else if ui.add_sized([240.0, 44.0], egui::Button::new("Start reading")).clicked() {
                    if ready {
                        if self.app.finish_onboarding() { ui.ctx().request_repaint(); }
                    } else {
                        self.app.choose_initial_translation(&self.onboarding_selection);
                        self.onboarding_waiting = true;
                    }
                }
                if let Some(error) = self.app.translation.as_ref().and_then(|t| t.online.as_ref()).and_then(|o| o.error.as_deref()) {
                    ui.colored_label(self.colors.red, error);
                    ui.label("Choose Start reading to retry, or select another Bible.");
                } else if let Some(status) = self.app.status_text() {
                    ui.label(status);
                }
                ui.add_space(12.0);
                ui.small("Study resources are optional. Add them later from Resources.");
                ui.add_space(12.0);
                if ui.small_button("Made by Zach Wilke · About").clicked() {
                    self.show_about = true;
                }
                ui.add_space(20.0);
            });
        });
    }

    fn about(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut open = true;
        let mut close = false;
        egui::Window::new("About OmaScripture")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(12.0);
                    ui.label(RichText::new("OmaScripture").size(28.0).strong().color(self.colors.accent));
                    ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                    ui.add_space(12.0);
                    ui.label("A little time in the Word.");
                    ui.add_space(16.0);
                    ui.label(RichText::new("Made by Zach Wilke").size(18.0).strong());
                    ui.add_space(8.0);
                    ui.hyperlink_to("GitHub · @zachwilke", "https://github.com/zachwilke");
                    ui.hyperlink_to("X · @Zachwilke_1", "https://x.com/Zachwilke_1");
                    ui.add_space(16.0);
                    ui.separator();
                    ui.hyperlink_to("Source code & releases", "https://github.com/zachwilke/omascripture");
                    ui.small("Open source · MIT license");
                    ui.small("Bible texts and study resources retain their own licenses.");
                    ui.add_space(16.0);
                    close = ui.button("Close").clicked();
                    ui.add_space(8.0);
                });
            });
        self.show_about = open && !close;
    }

    fn navigation(&mut self, ui: &mut egui::Ui) {
        ui.add_space(12.0);
        ui.label(
            RichText::new("YOUR LIBRARY")
                .size(11.0)
                .strong()
                .color(self.colors.muted),
        );
        let name = self
            .app
            .translation
            .as_ref()
            .map(|t| t.translation.clone())
            .unwrap_or("Choose a Bible".into());
        if ui
            .add_sized([ui.available_width(), 42.0], egui::Button::new(name))
            .clicked()
        {
            self.app.open_translations(Slot::Primary);
        }
        ui.add_space(5.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Search").clicked() {
                self.command('/');
            }
            if ui.button("Bookmarks").clicked() {
                self.command('B');
            }
        });
        ui.separator();
        ui.label(
            RichText::new("BOOKS")
                .size(11.0)
                .strong()
                .color(self.colors.muted),
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.book_filter)
                .hint_text("Find a book…")
                .desired_width(f32::INFINITY),
        );
        let books: Vec<_> = self
            .app
            .translation
            .as_ref()
            .map(|t| t.books.iter().map(|b| (b.nr, b.name.clone())).collect())
            .unwrap_or_default();
        let here = self.app.position().map(|p| p.book);
        let follow_book = self.last_book != here;
        self.last_book = here;
        let filter = self.book_filter.to_lowercase();
        ScrollArea::vertical()
            .id_salt("books")
            .max_height((ui.available_height() - 200.0).max(120.0))
            .show(ui, |ui| {
                for (nr, name) in books {
                    if !name.to_lowercase().contains(&filter) {
                        continue;
                    }
                    if filter.is_empty() && matches!(nr, 1 | 40) {
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(if nr == 1 {
                                "OLD TESTAMENT"
                            } else {
                                "NEW TESTAMENT"
                            })
                            .size(10.0)
                            .color(self.colors.muted),
                        );
                    }
                    let book_response = ui.add_sized(
                        [ui.available_width(), 29.0],
                        egui::Button::new(name)
                            .selected(here == Some(nr))
                            .frame(here == Some(nr)),
                    );
                    if follow_book && here == Some(nr) {
                        book_response.scroll_to_me(Some(egui::Align::Center));
                    }
                    if book_response.clicked() {
                        self.app.jump_position(Position {
                            book: nr,
                            chapter: 1,
                            verse: 1,
                        });
                    }
                }
            });
        ui.separator();
        ui.label(
            RichText::new("STUDY TOOLS")
                .size(11.0)
                .strong()
                .color(self.colors.muted),
        );
        ui.horizontal_wrapped(|ui| {
            for (label, key) in [
                ("Reading plan", 'r'),
                ("Dictionary", 'D'),
                ("Passages", 'P'),
                ("Resources", 'R'),
                ("Today's verse", 'v'),
            ] {
                if ui.button(label).clicked() {
                    self.command(key);
                }
            }
        });
    }

    fn reading(&mut self, ui: &mut egui::Ui) {
        let Some(t) = self.app.translation.take() else {
            ui.add_space(70.0);
            ui.heading("Make room for the Word.");
            ui.label("Choose a Bible to begin reading and studying.");
            ui.add_space(15.0);
            if ui.button("Choose a translation").clicked() {
                self.app.open_translations(Slot::Primary);
            }
            return;
        };
        let loc = self.app.loc;
        let pos = t.position_from_loc(loc);
        let ready = t
            .online
            .as_ref()
            .is_none_or(|o| o.loaded == Some((loc.book, loc.chapter)) && o.error.is_none());
        let follow = self.last_follow != Some((pos, ready));
        self.last_follow = Some((pos, ready));
        let mut selection = None;
        let mut action = None;
        if self.app.parallel_visible && self.app.parallel.is_some() {
            let p = self.app.parallel.take().unwrap();
            ui.columns(2, |cols| {
                self.chapter(
                    &mut cols[0],
                    &t,
                    loc,
                    "primary",
                    follow,
                    &mut selection,
                    &mut action,
                );
                if let Some(ploc) = p.loc_from_position(pos) {
                    self.chapter(
                        &mut cols[1],
                        &p,
                        ploc,
                        "parallel",
                        follow,
                        &mut selection,
                        &mut action,
                    );
                } else {
                    cols[1].label("This passage is unavailable in the parallel translation.");
                }
            });
            self.app.parallel = Some(p);
        } else {
            let width = if self.app.study.text_width > 0 {
                self.app.study.text_width as f32 * self.font_size() * 0.48
            } else {
                820.0
            };
            let pad = ((ui.available_width() - width) / 2.0).max(0.0);
            let rect = ui.available_rect_before_wrap();
            let reading_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left() + pad, rect.top()),
                egui::vec2((rect.width() - 2.0 * pad).max(100.0), rect.height()),
            );
            let mut reader = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(reading_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            self.chapter(
                &mut reader,
                &t,
                loc,
                "primary",
                follow,
                &mut selection,
                &mut action,
            );
            ui.advance_cursor_after_rect(rect);
        }
        self.app.translation = Some(t);
        if let Some(p) = selection
            && let Some(loc) = self
                .app
                .translation
                .as_ref()
                .and_then(|t| t.loc_from_position(p))
        {
            if loc.book != self.app.loc.book || loc.chapter != self.app.loc.chapter {
                self.app.jump_position(p);
            } else {
                self.app.loc = loc;
                self.last_follow = Some((p, true));
            }
        }
        if let Some(c) = action {
            if c == '\u{5}' {
                self.key(KeyCode::F(5));
            } else {
                self.command(c);
            }
        }
        if let Some((position, word)) = self.pending_word.take() {
            self.report = None;
            self.app.jump_position(position);
            self.app.open_word_study(word);
        }
        if let Some(query) = self.pending_search.take() {
            self.command('/');
            self.app.search_query = query;
            self.key(KeyCode::Enter);
        }
    }
    fn font_size(&self) -> f32 {
        if self.app.study.gui_font_size >= 14.0 {
            self.app.study.gui_font_size.min(32.0)
        } else {
            21.0
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn chapter(
        &mut self,
        ui: &mut egui::Ui,
        t: &Translation,
        loc: Loc,
        pane: &str,
        follow: bool,
        selection: &mut Option<Position>,
        action: &mut Option<char>,
    ) {
        let b = &t.books[loc.book];
        let ch = &b.chapters[loc.chapter];
        ui.horizontal_wrapped(|ui| {
            ui.heading(format!("{} {}", b.name, ch.chapter));
            if ui
                .button(RichText::new(t.abbreviation.to_uppercase()).color(self.colors.accent))
                .clicked()
            {
                *action = Some(if pane == "primary" { 't' } else { 'T' });
            }
            if pane == "primary" {
                egui::ComboBox::from_id_salt("chapter-picker")
                    .selected_text("Chapter")
                    .width(85.0)
                    .show_ui(ui, |ui| {
                        for c in &b.chapters {
                            if ui
                                .selectable_label(c.chapter == ch.chapter, c.chapter.to_string())
                                .clicked()
                            {
                                *selection = Some(Position {
                                    book: b.nr,
                                    chapter: c.chapter,
                                    verse: 1,
                                });
                            }
                        }
                    });
            }
        });
        ui.label(
            RichText::new(&t.translation)
                .size(13.0)
                .color(self.colors.muted),
        );
        ui.add_space(6.0);
        if pane == "primary" {
            ui.horizontal_wrapped(|ui| {
                for (name, key) in [("Bookmark", 'm'), ("Highlight", 'x'), ("Add note", 'e')] {
                    if ui.small_button(name).clicked() {
                        *action = Some(key);
                    }
                }
                if ui.small_button("Copy verse").clicked()
                    && let Some(text) = t.verse_text(loc)
                {
                    ui.ctx().copy_text(format!(
                        "{} ({})\n{}",
                        t.reference(loc),
                        t.abbreviation.to_uppercase(),
                        text
                    ));
                }
            });
        } else {
            ui.add_space(29.0);
        }
        ui.add_space(8.0);
        ui.separator();
        if let Some(o) = &t.online {
            if let Some(error) = &o.error {
                ui.add_space(24.0);
                ui.label(error);
                if ui.button("Retry chapter").clicked() {
                    *action = Some('\u{5}');
                }
                return;
            }
            if o.loaded != Some((loc.book, loc.chapter)) {
                ui.add_space(30.0);
                ui.spinner();
                ui.label("Loading chapter…");
                return;
            }
        }
        let font = FontId::new(self.font_size(), FontFamily::Name("reading".into()));
        ScrollArea::vertical()
            .id_salt((pane, &t.abbreviation, b.nr, ch.chapter))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(12.0);
                for (i, v) in ch.verses.iter().enumerate() {
                    let p = Position {
                        book: b.nr,
                        chapter: ch.chapter,
                        verse: v.verse,
                    };
                    let chosen = i == loc.verse;
                    let marked = self.app.study.is_bookmarked(p);
                    let note = self.app.study.note(p).is_some();
                    let hl = self.app.study.highlight(p);
                    let fill = self.colors.highlight(hl, chosen);
                    let response = egui::Frame::new()
                        .fill(fill)
                        .corner_radius(7)
                        .inner_margin(egui::Margin::symmetric(12, 10))
                        .show(ui, |ui| {
                            ui.horizontal_top(|ui| {
                                let number = format!(
                                    "{}{}{}",
                                    v.verse,
                                    if marked { " •" } else { "" },
                                    if note { " ✎" } else { "" }
                                );
                                if ui
                                    .add(
                                        egui::Label::new(RichText::new(number).size(12.0).color(
                                            if chosen {
                                                self.colors.accent
                                            } else {
                                                self.colors.muted
                                            },
                                        ))
                                        .sense(egui::Sense::click()),
                                    )
                                    .clicked()
                                {
                                    *selection = Some(p);
                                }
                                let galley = egui::WidgetText::from(
                                    RichText::new(&v.text).font(font.clone()),
                                )
                                .into_galley(
                                    ui,
                                    Some(egui::TextWrapMode::Wrap),
                                    ui.available_width(),
                                    egui::FontSelection::Default,
                                );
                                let r = ui.add(
                                    egui::Label::new(galley.clone())
                                        .selectable(true)
                                        .sense(egui::Sense::click()),
                                );
                                let hovered = r.hover_pos().and_then(|pointer| {
                                    let point = pointer - r.rect.min;
                                    let index = glyph_index(&galley, point)?;
                                    word_data::tokens(&v.text)
                                        .into_iter()
                                        .enumerate()
                                        .find(|(_, (range, _))| range.contains(&index))
                                        .map(|(index, (_, word))| {
                                            self.word_at(t, p, &v.text, index, word)
                                        })
                                });
                                if let Some(choice) = &hovered {
                                    r.clone().on_hover_ui(|ui| self.word_preview(ui, choice));
                                }
                                if r.clicked() || r.secondary_clicked() {
                                    *selection = Some(p);
                                    self.word_choice = hovered;
                                }
                                egui::Popup::menu(&r)
                                    .open_memory(
                                        (r.clicked() || r.secondary_clicked())
                                            .then_some(egui::SetOpenCommand::Bool(true)),
                                    )
                                    .at_pointer_fixed()
                                    .show(|ui| {
                                        if let Some(choice) = self
                                            .word_choice
                                            .clone()
                                            .filter(|choice| choice.position == p)
                                        {
                                            self.word_menu(ui, &choice);
                                            ui.separator();
                                        }
                                        if ui.button("Copy verse").clicked() {
                                            ui.ctx().copy_text(format!(
                                                "{} {}:{} ({})\n{}",
                                                b.name,
                                                ch.chapter,
                                                v.verse,
                                                t.abbreviation.to_uppercase(),
                                                v.text
                                            ));
                                            ui.close();
                                        }
                                        for (label, c) in [
                                            ("Bookmark", 'm'),
                                            ("Highlight", 'x'),
                                            ("Write a note", 'e'),
                                            ("Study this verse", '1'),
                                        ] {
                                            if ui.button(label).clicked() {
                                                *selection = Some(p);
                                                *action = Some(c);
                                                ui.close();
                                            }
                                        }
                                    });
                            });
                        })
                        .response;
                    if chosen && follow {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }
                    if chosen {
                        ui.painter().vline(
                            response.rect.left(),
                            response.rect.y_range(),
                            egui::Stroke::new(2.0_f32, self.colors.accent),
                        );
                    }
                    ui.add_space(3.0);
                }
                ui.add_space(30.0);
                if let Some(o) = &t.online {
                    ui.label(RichText::new(&o.notice).size(11.0).color(self.colors.muted));
                }
                ui.add_space(30.0);
            });
    }

    fn word_at(
        &mut self,
        translation: &Translation,
        position: Position,
        verse: &str,
        index: usize,
        text: String,
    ) -> WordChoice {
        let alignment = self
            .language
            .aligned(&translation.abbreviation, position, verse, index);
        let mut words = self.app.resources.interlinear.verse(position);
        if let Some(tag) = &alignment {
            words.retain(|word| {
                let base = resources::base_strong(&word.strong);
                tag.strongs.iter().enumerate().any(|(i, id)| {
                    if id != &base {
                        return false;
                    }
                    if tag.morphs.len() != tag.strongs.len() {
                        return true;
                    }
                    let morph = &tag.morphs[i];
                    word.morph == *morph
                        || word
                            .morph
                            .strip_prefix(morph)
                            .is_some_and(|suffix| suffix.starts_with('-'))
                })
            });
            for strong in &tag.strongs {
                if !words
                    .iter()
                    .any(|w| resources::base_strong(&w.strong) == *strong)
                    && let Some(entry) = self
                        .app
                        .resources
                        .lexicon(strong)
                        .and_then(|l| l.get(strong))
                        .cloned()
                {
                    words.push(resources::Word {
                        verse: position.verse,
                        chapter: position.chapter,
                        text: entry.lemma.clone(),
                        translit: entry.translit,
                        gloss: entry.gloss.clone(),
                        strong: entry.strong,
                        morph: String::new(),
                        lemma: entry.lemma,
                        lemma_gloss: entry.gloss,
                        variant: false,
                    });
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        words.retain(|w| seen.insert((w.strong.clone(), w.text.clone(), w.morph.clone())));
        WordChoice {
            position,
            text,
            phrase: alignment
                .as_ref()
                .map(|t| t.phrase.clone())
                .unwrap_or_default(),
            aligned: alignment.is_some(),
            words,
        }
    }

    fn word_preview(&mut self, ui: &mut egui::Ui, choice: &WordChoice) {
        ui.set_max_width(420.0);
        ui.heading(&choice.text);
        if choice.aligned {
            if !choice.phrase.is_empty() {
                ui.label(format!("Tagged phrase: {}", choice.phrase));
            }
            for word in choice.words.iter().take(4) {
                ui.label(
                    RichText::new(format!(
                        "{} · {} · {}",
                        word.text, word.translit, word.strong
                    ))
                    .color(self.colors.accent),
                );
                ui.label(format!("Lemma: {} — {}", word.lemma, word.lemma_gloss));
                if !word.morph.is_empty() {
                    ui.label(self.language.morphology(&word.morph));
                }
            }
            if choice.words.is_empty() {
                ui.label("No original-language tag for this word. It may be supplied by the translation.");
            }
            if choice.words.len() > 1 {
                ui.label("Multiple original words/forms: choose one in Word Study.");
            }
            ui.small("KJV phrase tags: CrossWire · Language data: STEPBible");
        } else {
            ui.label("No verified word alignment for this verse and edition. Click to choose a Greek or Hebrew word from this verse.");
        }
        ui.small("Click or right-click for Word Study, search, and copying.");
    }

    fn word_menu(&mut self, ui: &mut egui::Ui, choice: &WordChoice) {
        ui.set_max_width(480.0);
        ui.label(RichText::new(&choice.text).strong());
        if !choice.aligned {
            ui.label("Choose an original word from this verse; English alignment is unavailable.");
        }
        if choice.words.len() == 1 {
            if ui.button("Word Study").clicked() {
                self.pending_word = Some((choice.position, choice.words[0].clone()));
                ui.close();
            }
        } else if !choice.words.is_empty() {
            ui.menu_button("Word Study", |ui| {
                ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    for (i, word) in choice.words.iter().enumerate() {
                        ui.push_id(i, |ui| {
                            if ui
                                .button(format!("{} · {} · {}", word.text, word.gloss, word.strong))
                                .clicked()
                            {
                                self.pending_word = Some((choice.position, word.clone()));
                                ui.close();
                            }
                        });
                    }
                });
            });
        } else {
            ui.label("Language data unavailable for this word.");
            if ui.button("Open language resources").clicked() {
                self.app.mode = Mode::Resources;
                self.app.list_index = 0;
                ui.close();
            }
        }
        if ui.button("Copy word").clicked() {
            ui.ctx().copy_text(choice.text.clone());
            ui.close();
        }
        if ui.button("Search this word").clicked() {
            self.pending_search = Some(format!("\"{}\"", choice.text));
            ui.close();
        }
    }

    fn study_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.heading("Study");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Close").clicked() {
                    self.app.sidebar = 0;
                    self.persist();
                }
            });
        });
        if let Some(t) = &self.app.translation {
            ui.label(RichText::new(t.reference(self.app.loc)).color(self.colors.accent));
        }
        ui.horizontal_wrapped(|ui| {
            for (tab, label) in [
                (1, "References"),
                (2, "Words"),
                (3, "Commentary"),
                (4, "Study notes"),
            ] {
                if ui
                    .selectable_label(self.app.sidebar == tab, label)
                    .clicked()
                {
                    self.app.show_tab(tab);
                    self.persist();
                }
            }
        });
        ui.separator();
        if self.app.sidebar == 3 {
            let mut selected = self.app.commentary_id().unwrap_or_default();
            egui::ComboBox::from_id_salt("commentary")
                .selected_text(
                    resources::pack(&selected)
                        .map(|p| p.name)
                        .unwrap_or("Choose commentary"),
                )
                .show_ui(ui, |ui| {
                    for id in resources::Store::commentary_ids() {
                        ui.selectable_value(
                            &mut selected,
                            id.to_string(),
                            resources::pack(id).map(|p| p.name).unwrap_or(id),
                        );
                    }
                });
            if self.app.study.commentary.as_deref() != Some(&selected) && !selected.is_empty() {
                self.app.study.commentary = Some(selected);
                self.app.side_cache = None;
                self.persist();
            }
        }
        let content = self.app.side_content().clone();
        ScrollArea::vertical()
            .id_salt(("study-content", self.app.sidebar, self.app.position()))
            .show(ui, |ui| match content {
                SideContent::Empty(text) => {
                    ui.add_space(20.0);
                    ui.label(text);
                    if ui.button("Browse resources").clicked() {
                        self.command('R');
                    }
                }
                SideContent::Refs(items) => {
                    for r in items {
                        if ui
                            .link(RichText::new(r.label).color(self.colors.accent))
                            .clicked()
                        {
                            self.app.jump_position(r.pos);
                        }
                        if !r.text.is_empty() {
                            ui.label(r.text);
                        }
                        ui.add_space(5.0);
                        ui.separator();
                    }
                }
                SideContent::Words(words) => {
                    for word in words {
                        let response = ui.button(
                            RichText::new(format!("{}  {}", word.text, word.translit)).size(18.0),
                        );
                        response.clone().on_hover_ui(|ui| {
                            ui.label(format!("Lemma: {} · {}", word.lemma, word.strong));
                            ui.label(&word.lemma_gloss);
                            ui.label(self.language.morphology(&word.morph));
                        });
                        if response.clicked() {
                            self.report = None;
                            self.app.open_word_study(word.clone());
                        }
                        ui.label(format!("{}  ·  {}", word.gloss, word.strong));
                        ui.label(RichText::new(word.morph).small().color(self.colors.muted));
                        ui.separator();
                    }
                }
                SideContent::Text { title, body } => {
                    ui.label(RichText::new(title).strong().color(self.colors.accent));
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(body).wrap().selectable(true));
                }
            });
    }

    fn dialog(&mut self, ctx: &egui::Context) {
        let mode = self.app.mode;
        if matches!(mode, Mode::Read | Mode::Menu) {
            return;
        }
        let title = match mode {
            Mode::Translations => {
                if self.app.picker_slot == Slot::Primary {
                    "Choose your Bible"
                } else {
                    "Choose a parallel Bible"
                }
            }
            Mode::Search | Mode::SearchResults => "Search Scripture",
            Mode::GoTo => "Go to a passage",
            Mode::Note => "Your notes",
            Mode::Bookmarks => "Bookmarks",
            Mode::Resources => "Study resources",
            Mode::Harmony => "Parallel passages",
            Mode::Plans => "Reading plan",
            Mode::DictSearch | Mode::DictEntry => "Dictionary",
            Mode::WordStudy => "Word study",
            Mode::Occurrences => "Word occurrences",
            Mode::Settings => "Settings",
            Mode::Books => "Books",
            Mode::Chapters => "Chapters",
            _ => "About OmaScripture",
        };
        let response = egui::Modal::new(egui::Id::new("dialog")).show(ctx, |ui| {
            ui.set_width(650.0_f32.min(ctx.content_rect().width() - 70.0));
            ui.horizontal(|ui| { ui.heading(title); ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { if ui.button("Close").clicked() { self.close_dialog(); } }); });
            ui.separator();
            ScrollArea::vertical().id_salt(("dialog-scroll", format!("{mode:?}"))).max_height((ctx.content_rect().height() - 240.0).clamp(220.0, 600.0)).min_scrolled_height(220.0).show(ui, |ui| {
                match mode {
                    Mode::Translations => self.translations(ui),
                    Mode::Settings => self.settings(ui),
                    Mode::Search | Mode::SearchResults => self.search(ui),
                    Mode::Note => self.note(ui),
                    Mode::Resources => self.resources(ui),
                    Mode::Plans => self.plans(ui),
                    Mode::DictSearch | Mode::DictEntry => self.dictionary(ui),
                    Mode::WordStudy | Mode::Occurrences => self.word_study(ui),
                    Mode::Bookmarks => {
                        let rows = self.app.bookmark_rows();
                        if rows.is_empty() { ui.label("Select a verse and click Bookmark to save it here."); }
                        for (p, reference, text) in rows {
                            ui.horizontal(|ui| {
                                if ui.link(&reference).clicked() { self.app.jump_position(p); }
                                if ui.small_button("Remove").clicked() { self.app.study.toggle_bookmark(p); self.persist(); }
                            });
                            ui.label(text); ui.separator();
                        }
                    }
                    Mode::Harmony => {
                        let rows: Vec<_> = self.app.harmony_items.iter().map(|r| (r.title, r.range.start_position())).collect();
                        if rows.is_empty() { ui.label("No parallel passages for this chapter."); }
                        for (title, pos) in rows { if ui.button(format!("{title} — {}", self.app.label(pos))).clicked() { self.app.jump_position(pos); } }
                    }
                    Mode::GoTo => { ui.text_edit_singleline(&mut self.reference); if ui.button("Open passage").clicked() { self.app.goto_reference(&self.reference.clone()); } }
                    Mode::Books => { for (i, name) in self.app.book_rows() { if ui.button(name).clicked() { self.select_row(i); } } }
                    Mode::Chapters => {
                        let n = self.app.translation.as_ref().map(|t| t.books[self.app.book_pick].chapters.len()).unwrap_or(0);
                        ui.horizontal_wrapped(|ui| { for i in 0..n { if ui.button((i + 1).to_string()).clicked() { self.app.chapter_pick = i; self.key(KeyCode::Enter); } } });
                    }
                    _ => { ui.label("OmaScripture — a desktop space for reading and studying Scripture."); ui.label("Select verses to study them. Right-click a verse for notes, bookmarks and copying. Drag over text to select it."); ui.hyperlink_to("Project and resource credits", "https://github.com/zachwilke/omascripture"); }
                }
            });
            if mode == Mode::Note { self.note_actions(ui); }
            if let Some(busy) = self.app.busy.clone() { ui.horizontal(|ui| { ui.spinner(); ui.label(busy); }); }
            if let Some(status) = self.app.status_text() { ui.separator(); ui.label(RichText::new(status).color(self.colors.accent)); }
        });
        // An unsaved note closes only through the explicit Cancel/Close controls.
        if response.should_close() && mode != Mode::Note {
            self.close_dialog();
        }
    }
    fn close_dialog(&mut self) {
        if self.app.mode == Mode::Note {
            self.key(KeyCode::Esc);
            return;
        }
        if self.app.mode == Mode::Translations && self.app.return_to == Some(Mode::Settings) {
            self.app.mode = Mode::Settings;
            self.app.return_to = None;
        } else {
            self.app.mode = Mode::Read;
        }
    }
    fn translations(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.app.filter)
                    .hint_text("Find a translation or language…")
                    .desired_width(440.0),
            );
            if ui.button("Refresh").clicked() {
                self.app.refresh_catalog();
            }
        });
        ui.label(
            RichText::new("Online editions stream chapters. Offline editions download once.")
                .small()
                .color(self.colors.muted),
        );
        let rows = self.app.translation_rows();
        for (i, (info, installed)) in rows.iter().enumerate() {
            ui.push_id(&info.abbreviation, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&info.translation).strong());
                        let detail = if providers::is_online(&info.abbreviation) {
                            providers::status(&info.abbreviation)
                        } else if *installed {
                            "Available offline".into()
                        } else {
                            format!("{} · Download", info.language)
                        };
                        ui.label(
                            RichText::new(format!(
                                "{}  ·  {detail}",
                                info.abbreviation.to_uppercase()
                            ))
                            .small()
                            .color(self.colors.muted),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                self.app.busy.is_none(),
                                egui::Button::new(
                                    if *installed || providers::is_online(&info.abbreviation) {
                                        "Open"
                                    } else {
                                        "Download"
                                    },
                                ),
                            )
                            .clicked()
                        {
                            self.select_row(i);
                        }
                    });
                });
                ui.separator();
            });
        }
        if rows.is_empty() {
            ui.label("No translations match. Try refreshing the catalog.");
        }
    }
    fn search(&mut self, ui: &mut egui::Ui) {
        if self
            .app
            .translation
            .as_ref()
            .is_none_or(|t| t.online.is_some())
        {
            ui.label("Whole-Bible search needs a downloaded Bible.");
            if ui.button("Choose an offline Bible").clicked() {
                self.app.open_translations(Slot::Primary);
            }
            return;
        }
        let enter = ui
            .add(
                egui::TextEdit::singleline(&mut self.app.search_query)
                    .hint_text("Words or a phrase…")
                    .desired_width(f32::INFINITY),
            )
            .lost_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.button("Search this Bible").clicked() || enter {
            self.app.mode = Mode::Search;
            self.key(KeyCode::Enter);
            // Keep the GUI search dialog open even when no matches were found.
            self.app.mode = Mode::SearchResults;
        }
        if self.app.search_hits.is_empty() {
            ui.label(if self.app.mode == Mode::SearchResults {
                "No matches. Try different words, then choose Search this Bible."
            } else {
                "Enter a word or phrase, then choose Search this Bible."
            });
        }
        for (i, hit) in self.app.search_hits.clone().iter().enumerate() {
            if ui.link(&hit.reference).clicked() {
                self.app.mode = Mode::SearchResults;
                self.select_row(i);
            }
            ui.label(&hit.text);
            ui.separator();
        }
    }
    fn note(&mut self, ui: &mut egui::Ui) {
        if let Some(t) = &self.app.translation {
            ui.label(RichText::new(t.reference(self.app.loc)).color(self.colors.accent));
        }
        ui.add(
            egui::TextEdit::multiline(&mut self.app.note_buffer)
                .hint_text("What stands out to you in this passage?")
                .desired_width(f32::INFINITY)
                .desired_rows(14),
        );
    }
    fn note_actions(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Save note").clicked()
                || ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S))
            {
                self.app
                    .handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
            }
            if self.app.save_error.is_some() && ui.button("Export unsaved work").clicked() {
                self.export_recovery();
            }
            let cancel = ui.button("Cancel");

            if cancel.clicked() {
                self.key(KeyCode::Esc);
            }
        });
        if self.app.save_error.is_some()
            && let Some(message) = &self.app.study.load_warning
        {
            ui.label(message);
        }
    }
    fn resources(&mut self, ui: &mut egui::Ui) {
        ui.label("Build your offline study library.");
        for (i, pack) in resources::PACKS.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(pack.name).strong());
                    ui.label(
                        RichText::new(format!(
                            "{} · {} · {}",
                            pack.kind.label(),
                            pack.size,
                            pack.license
                        ))
                        .small()
                        .color(self.colors.muted),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let installed = resources::is_installed(pack.id);
                    if ui
                        .add_enabled(
                            self.app.busy.is_none(),
                            egui::Button::new(if installed { "Remove" } else { "Install" }),
                        )
                        .clicked()
                    {
                        self.app.list_index = i;
                        self.key(if installed {
                            KeyCode::Delete
                        } else {
                            KeyCode::Enter
                        });
                    }
                });
            });
            ui.separator();
        }
    }
    fn plans(&mut self, ui: &mut egui::Ui) {
        if let Some(progress) = self.app.study.plan.clone() {
            let day = self.app.plan_view_day();
            ui.label(
                RichText::new(
                    plans::plan(&progress.id)
                        .map(|p| p.name)
                        .unwrap_or("Reading plan"),
                )
                .size(20.0),
            );
            ui.add(
                egui::ProgressBar::new(progress.done.len() as f32 / progress.days() as f32).text(
                    format!(
                        "{} of {} days completed",
                        progress.done.len(),
                        progress.days()
                    ),
                ),
            );
            ui.horizontal(|ui| {
                if ui.button("Previous day").clicked() {
                    self.key(KeyCode::Left);
                }
                ui.label(format!("Day {}", day + 1));
                if ui.button("Next day").clicked() {
                    self.key(KeyCode::Right);
                }
                if ui.button("Today").clicked() {
                    self.key(KeyCode::Char('t'));
                }
            });
            for (i, (b, c)) in plans::readings(&progress.id, day).iter().enumerate() {
                if ui
                    .button(self.app.label(Position {
                        book: *b,
                        chapter: *c,
                        verse: 1,
                    }))
                    .clicked()
                {
                    self.select_row(i);
                }
            }
            if ui
                .button(if progress.done.contains(&(day as u32)) {
                    "Mark unfinished"
                } else {
                    "Mark day complete"
                })
                .clicked()
            {
                self.key(KeyCode::Char('x'));
            }
            ui.add_space(12.0);
            if ui.small_button("Stop this plan").clicked() {
                self.key(KeyCode::Char('X'));
            }
        } else {
            for (i, p) in plans::PLANS.iter().enumerate() {
                ui.label(RichText::new(p.name).strong());
                ui.label(p.description);
                if ui.button("Start plan").clicked() {
                    self.select_row(i);
                }
                ui.separator();
            }
        }
    }
    fn dictionary(&mut self, ui: &mut egui::Ui) {
        if self.app.mode == Mode::DictEntry {
            if ui.button("Back to results").clicked() {
                self.app.mode = Mode::DictSearch;
            }
            if let Some((title, body)) = &self.app.dict_entry {
                ui.heading(title);
                ui.add(egui::Label::new(body).wrap().selectable(true));
            }
        } else {
            let changed = ui
                .add(
                    egui::TextEdit::singleline(&mut self.app.filter)
                        .hint_text("Find a name, place or topic…")
                        .desired_width(f32::INFINITY),
                )
                .changed();
            if changed {
                self.app.dict_search();
            }
            for (i, (_, source, term)) in self.app.dict_results.clone().iter().enumerate() {
                if ui.button(format!("{term}  ·  {source}")).clicked() {
                    self.select_row(i);
                }
            }
            if self.app.dict_results.is_empty() {
                ui.label("Search your installed dictionaries. Add dictionaries from Resources.");
            }
        }
    }
    fn poll_word_report(&mut self) {
        if let Some((id, receiver)) = &self.report_pending {
            match receiver.try_recv() {
                Ok(result) => {
                    self.report = Some((id.clone(), result));
                    self.report_pending = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.report = Some((
                        id.clone(),
                        Err("Word index stopped unexpectedly. Reopen Word Study to retry.".into()),
                    ));
                    self.report_pending = None;
                }
                _ => {}
            }
        }
    }

    fn update_word_report(&mut self, strong: &str) {
        self.poll_word_report();
        if self.report_pending.is_none() && self.report.as_ref().is_none_or(|(id, _)| id != strong)
        {
            let (tx, rx) = std::sync::mpsc::channel();
            let id = strong.to_string();
            self.report_pending = Some((id.clone(), rx));
            std::thread::spawn(move || {
                let report = std::panic::catch_unwind(|| {
                    resources::Interlinear::default().report(&id, crate::app::OCCURRENCE_LIMIT)
                })
                .map_err(|_| "Could not build word index".into());
                let _ = tx.send(report);
            });
        }
    }

    fn word_study(&mut self, ui: &mut egui::Ui) {
        let Some(ws) = &self.app.word_study else {
            return;
        };
        let strong = ws.strong.clone();
        let word = ws.word.clone();
        let entry = ws.entry.clone();
        self.update_word_report(&strong);
        if self.app.mode == Mode::WordStudy {
            ui.label(
                RichText::new(format!(
                    "{} · {}",
                    if strong.starts_with('G') {
                        "Greek"
                    } else {
                        "Hebrew / Aramaic"
                    },
                    strong
                ))
                .color(self.colors.accent),
            );
            if let Some(word) = &word {
                ui.heading(if word.lemma.is_empty() {
                    &word.text
                } else {
                    &word.lemma
                });
                ui.label(format!(
                    "Form in this verse: {}   {}",
                    word.text, word.translit
                ));
                ui.label(format!("Gloss: {}", word.gloss));
                if !word.morph.is_empty() {
                    ui.add_space(8.0);
                    ui.strong("Grammar in context");
                    ui.label(self.language.morphology(&word.morph));
                    ui.small(format!("Source code: {}", word.morph));
                } else {
                    ui.label("Contextual morphology is unavailable for this lexicon-only entry.");
                }
                if word.variant {
                    ui.label(
                        "This form is a textual variant outside the NA28 occurrence corpus below.",
                    );
                }
            }
            if !resources::is_installed("word-study")
                && ui
                    .button("Get expanded grammar and word alignment")
                    .clicked()
            {
                self.app.mode = Mode::Resources;
                self.app.list_index = 0;
            }
            ui.separator();
            ui.strong("Lexicon");
            if let Some(entry) = &entry {
                ui.label(format!(
                    "{} · {} · {}",
                    entry.lemma, entry.translit, entry.gloss
                ));
                ui.add(egui::Label::new(&entry.meaning).wrap().selectable(true));
                if ui.button("Copy definition").clicked() {
                    ui.ctx().copy_text(format!(
                        "{} ({}) — {}\n{}\nSTEPBible TBESG/TBESH, CC BY 4.0",
                        entry.lemma, strong, entry.gloss, entry.meaning
                    ));
                }
            } else {
                ui.label("Install the Greek or Hebrew lexicon in Resources for definitions.");
            }
            ui.small("Definitions: STEPBible TBESG/TBESH · Parsing: TAGNT/TAHOT and TEGMC/TEHMC · CC BY 4.0, Tyndale House Cambridge.");
            ui.hyperlink_to(
                "About this language data",
                "https://github.com/STEPBible/STEPBible-Data",
            );
            ui.separator();
            ui.strong("Usage in Scripture");
        } else if ui.button("Back to word study").clicked() {
            self.app.mode = Mode::WordStudy;
        }
        let Some((_, result)) = self.report.as_ref().filter(|(id, _)| id == &strong) else {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Indexing installed original-language books…");
            });
            return;
        };
        match result {
            Err(error) => {
                ui.label(error);
            }
            Ok(report) => {
                ui.label(format!(
                    "{} word occurrences · {} verses · {}/{} books indexed",
                    report.total,
                    report.verse_count,
                    report.loaded_books,
                    if strong.starts_with('G') { 27 } else { 39 }
                ));
                ui.small(if strong.starts_with('G') { "Corpus: STEPBible NA28 Greek text; KJV-only variants excluded." } else { "Corpus: STEPBible Leningrad Hebrew/Aramaic text; qere/ketiv alternatives excluded." });
                if self.app.mode == Mode::WordStudy {
                    let mut books: Vec<_> = report.book_counts.iter().collect();
                    books.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
                    for (book, count) in books.into_iter().take(8) {
                        ui.add(
                            egui::ProgressBar::new(*count as f32 / report.total.max(1) as f32)
                                .text(format!(
                                    "{} — {count}",
                                    providers::NAMES[*book as usize - 1]
                                )),
                        );
                    }
                    ui.collapsing("Inflected forms", |ui| {
                        let mut forms: Vec<_> = report.forms.iter().collect();
                        forms.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
                        for (form, count) in forms {
                            ui.label(format!("{form} — {count}"));
                        }
                    });
                    if ui.button("Find every occurrence").clicked() {
                        self.app.mode = Mode::Occurrences;
                    }
                } else {
                    if report.verse_count > report.verses.len() {
                        ui.label(format!(
                            "Showing the first {} matching verses.",
                            report.verses.len()
                        ));
                    }
                    let mut selected = None;
                    for (pos, gloss) in &report.verses {
                        if ui
                            .link(format!("{} · {gloss}", self.app.label(*pos)))
                            .clicked()
                        {
                            selected = Some(*pos);
                        }
                        let context = self.app.text_at(*pos);
                        if !context.is_empty() {
                            ui.label(context);
                        }
                        ui.separator();
                    }
                    if let Some(pos) = selected {
                        self.app.jump_position(pos);
                    }
                }
            }
        }
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        if ui.button("About OmaScripture").clicked() {
            self.show_about = true;
        }
        ui.add_space(8.0);
        ui.label(RichText::new("READING").strong().color(self.colors.accent));
        for (row, label) in [
            (Row::Translation, "Default Bible"),
            (Row::Parallel, "Parallel Bible"),
        ] {
            ui.horizontal(|ui| {
                ui.label(label);
                if ui.button(self.app.setting_value(row)).clicked() {
                    self.app.adjust_setting(row, 1);
                }
            });
        }
        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut self.app.parallel_visible, "Show parallel reading pane")
                .changed()
            {
                self.app.study.parallel_visible = self.app.parallel_visible;
                self.persist();
            }
            if ui.small_button("Remove parallel Bible").clicked() {
                self.app.clear_parallel();
            }
        });
        let mut size = self.font_size();
        if ui
            .add(egui::Slider::new(&mut size, 14.0..=32.0).text("Reading text size"))
            .changed()
        {
            self.app.study.gui_font_size = size;
            self.persist();
        }
        let before = self.app.study.gui_theme.clone();
        egui::ComboBox::from_label("Appearance")
            .selected_text(match before.as_str() {
                "light" => "Light",
                "dark" => "Dark",
                _ => "Follow Omarchy",
            })
            .show_ui(ui, |ui| {
                for (value, label) in [("", "Follow Omarchy"), ("light", "Light"), ("dark", "Dark")]
                {
                    ui.selectable_value(&mut self.app.study.gui_theme, value.into(), label);
                }
            });
        if before != self.app.study.gui_theme {
            self.colors = self
                .theme_source
                .resolve(&self.app.study.gui_theme, self.app.study.gui_light);
            theme(ui.ctx(), self.colors);
            self.persist();
        }
        for row in [
            Row::Start,
            Row::SidebarStart,
            Row::SidebarTab,
            Row::Commentary,
            Row::TextWidth,
        ] {
            ui.horizontal(|ui| {
                ui.label(row.label());
                if ui.button("‹").clicked() {
                    self.app.adjust_setting(row, -1);
                }
                ui.label(self.app.setting_value(row));
                if ui.button("›").clicked() {
                    self.app.adjust_setting(row, 1);
                }
            });
        }
        ui.add_space(14.0);
        ui.separator();
        ui.label(
            RichText::new("ONLINE BIBLES")
                .strong()
                .color(self.colors.accent),
        );
        ui.label("NET and NLT are ready to use. Add your provider keys for more editions.");
        if self.keys.is_none() {
            match providers::config() {
                Ok(c) => self.keys = Some(c),
                Err(e) => self.provider_error = Some(e),
            }
        }
        if let Some(keys) = &mut self.keys {
            egui::Grid::new("provider-keys")
                .num_columns(2)
                .spacing([16.0, 12.0])
                .show(ui, |ui| {
                    for (label, value) in [
                        ("ESV key", &mut keys.esv_key),
                        ("NLT key (optional)", &mut keys.nlt_key),
                        ("API.Bible key", &mut keys.api_bible_key),
                    ] {
                        ui.label(label);
                        ui.add(
                            egui::TextEdit::singleline(value)
                                .password(!self.show_keys)
                                .desired_width(400.0),
                        );
                        ui.end_row();
                    }
                });
            ui.checkbox(&mut self.show_keys, "Show keys");
            ui.horizontal_wrapped(|ui| {
                ui.hyperlink_to("Get an ESV key", "https://api.esv.org/");
                ui.hyperlink_to("NLT access", "https://api.nlt.to/");
                ui.hyperlink_to("API.Bible account", "https://scripture.api.bible/");
            });
            ui.collapsing("API.Bible editions", |ui| {
                ui.label("Add a Bible ID authorized for your API.Bible account.");
                ui.add(egui::TextEdit::singleline(&mut self.api_id).hint_text("Bible ID"));
                ui.add(
                    egui::TextEdit::singleline(&mut self.api_name)
                        .hint_text("Display name (e.g. NIV)"),
                );
                if ui.button("Add edition").clicked()
                    && !self.api_id.trim().is_empty()
                    && !self.api_name.trim().is_empty()
                {
                    keys.api_bible.push(providers::ApiBible {
                        id: self.api_id.trim().into(),
                        name: self.api_name.trim().into(),
                    });
                    self.api_id.clear();
                    self.api_name.clear();
                }
                let mut remove = None;
                for (i, edition) in keys.api_bible.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} · {}", edition.name, edition.id));
                        if ui.small_button("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    keys.api_bible.remove(i);
                }
            });
            if ui.button("Save provider keys").clicked() {
                match providers::save_config(keys) {
                    Ok(()) => {
                        self.provider_error = None;
                        self.app
                            .set_status("Provider settings saved securely".into());
                    }
                    Err(e) => self.provider_error = Some(e),
                }
            }
            ui.label(RichText::new("Environment-variable keys take precedence. Keys are stored privately on this computer.").small().color(self.colors.muted));
        }
        if let Some(error) = &self.provider_error {
            ui.colored_label(self.colors.red, error);
        }
    }
}
impl eframe::App for Desktop {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.show(ctx);
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        if !self.app.save() {
            eprintln!(
                "{}",
                self.app
                    .save_error
                    .as_deref()
                    .unwrap_or("Study save failed")
            );
            if let Ok(path) = self.app.study.export_recovery() {
                eprintln!("Recovery copy: {}", path.display());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn welcome_choice_starts_reading_and_saves_the_default() {
        let (mut desktop, ctx) = fixture();
        let dir = crate::storage::TestDir::new();
        let path = dir.0.join("study.json");
        desktop.app.study = crate::study::Study::load_from(path.clone());
        desktop.app.onboarding = true;
        desktop.app.study.onboarding_complete = Some(false);
        desktop.app.study.translation = Some("kjv".into());
        desktop.app.translation.as_mut().unwrap().abbreviation = "kjv".into();
        let detail = if crate::bible::is_installed("kjv") {
            "Already downloaded · available offline"
        } else {
            "Classic English · offline reading and linked word studies"
        };
        click(&mut desktop, &ctx, &format!("King James Version\n{detail}"));
        assert_eq!(desktop.onboarding_selection, "kjv");
        click(&mut desktop, &ctx, "Start reading");
        assert!(!desktop.app.onboarding);
        let saved = crate::study::Study::load_from(path);
        assert_eq!(saved.translation.as_deref(), Some("kjv"));
        assert_eq!(saved.onboarding_complete, Some(true));
    }

    #[test]
    fn search_empty_states_keep_actions_available() {
        let (mut desktop, ctx) = fixture();
        click(&mut desktop, &ctx, "Search");
        desktop.app.search_query = "no-such-phrase-in-this-fixture".into();
        click(&mut desktop, &ctx, "Search this Bible");
        assert_eq!(desktop.app.mode, Mode::SearchResults);
        assert!(desktop.app.search_hits.is_empty());
        let output = frame(&mut desktop, &ctx, vec![], [1380.0, 900.0]);
        text_center(
            &output,
            "No matches. Try different words, then choose Search this Bible.",
        );
        click(&mut desktop, &ctx, "Close");
        desktop.app.translation = Some(providers::open("net").unwrap());
        click(&mut desktop, &ctx, "Search");
        click(&mut desktop, &ctx, "Choose an offline Bible");
        assert_eq!(desktop.app.mode, Mode::Translations);
        assert_eq!(desktop.app.picker_slot, Slot::Primary);
    }

    #[test]
    fn completed_word_worker_is_drained_after_dialog_closes() {
        let (mut desktop, ctx) = fixture();
        let (tx, rx) = std::sync::mpsc::channel();
        desktop.report_pending = Some(("G0025".into(), rx));
        desktop.app.mode = Mode::Read;
        tx.send(Ok(WordReport::default())).unwrap();
        frame(&mut desktop, &ctx, vec![], [1380.0, 900.0]);
        assert!(desktop.report_pending.is_none());
        assert_eq!(desktop.report.as_ref().unwrap().0, "G0025");
    }

    #[test]
    fn clicked_word_opens_menu_and_correct_lemma_study() {
        let (mut desktop, ctx) = fixture();
        let dir = crate::storage::TestDir::new();
        word_data::import_kjv(br#"<osis><verse osisID="John.3.16"><w lemma="strong:G1063">For</w> <w lemma="strong:G25" morph="robinson:V-AAI-3S">loved</w></verse></osis>"#, &dir.0).unwrap();
        std::fs::write(dir.0.join("installed.json"), "{}").unwrap();
        desktop.language = LanguageData::test_root(dir.0.clone());
        desktop.app.resources.interlinear = resources::Interlinear::test_book(
            43,
            "Jhn.3.16#01=NKO\tἠγάπησεν (ēgapēsen)\tloved\tG0025=V-AAI-3S\tἀγαπάω=to love\nJhn.3.16#02=NKO\tἠγαπήσαμεν (ēgapēsamen)\twe loved\tG0025=V-AAI-1P\tἀγαπάω=to love\n",
        );
        let translation = desktop.app.translation.as_mut().unwrap();
        translation.abbreviation = "kjv".into();
        translation.books[42].chapters[2].verses = vec![crate::bible::Verse {
            verse: 16,
            text: "For loved".into(),
        }];
        desktop.app.loc.verse = 0;
        let mut point = None;
        for _ in 0..3 {
            let out = frame(&mut desktop, &ctx, vec![], [1380.0, 900.0]);
            fn find(shape: &egui::Shape) -> Option<egui::Pos2> {
                match shape {
                    egui::Shape::Text(text) if text.galley.text() == "For loved" => {
                        let row = &text.galley.rows[0];
                        Some(
                            text.pos
                                + row.pos.to_vec2()
                                + row.glyphs[6].logical_rect().center().to_vec2(),
                        )
                    }
                    egui::Shape::Vec(shapes) => shapes.iter().find_map(find),
                    _ => None,
                }
            }
            point = out
                .shapes
                .iter()
                .find_map(|shape| find(&shape.shape))
                .or(point);
        }
        let pos = point.expect("visible verse");
        ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
        frame(
            &mut desktop,
            &ctx,
            vec![egui::Event::PointerMoved(pos)],
            [1380.0, 900.0],
        );
        let hover = frame(&mut desktop, &ctx, vec![], [1380.0, 900.0]);
        assert!(hover.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("G0025"))), "word hover should show language data");
        frame(
            &mut desktop,
            &ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            [1380.0, 900.0],
        );
        frame(
            &mut desktop,
            &ctx,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            [1380.0, 900.0],
        );
        assert_eq!(desktop.word_choice.as_ref().unwrap().text, "loved");
        assert!(desktop.word_choice.as_ref().unwrap().aligned);
        assert_eq!(desktop.word_choice.as_ref().unwrap().words.len(), 1);
        assert_eq!(
            desktop.word_choice.as_ref().unwrap().words[0].morph,
            "V-AAI-3S"
        );
        click(&mut desktop, &ctx, "Word Study");
        assert_eq!(desktop.app.mode, Mode::WordStudy);
        assert_eq!(desktop.app.word_study.as_ref().unwrap().strong, "G0025");
    }

    #[test]
    fn occurrence_dialog_renders_capped_results_and_navigates() {
        let (mut desktop, ctx) = fixture();
        let verses = desktop.app.translation.as_ref().unwrap().books[42]
            .chapters
            .iter()
            .flat_map(|chapter| {
                chapter.verses.iter().map(move |verse| {
                    (
                        Position {
                            book: 43,
                            chapter: chapter.chapter,
                            verse: verse.verse,
                        },
                        "test gloss".to_string(),
                    )
                })
            })
            .take(crate::app::OCCURRENCE_LIMIT)
            .collect::<Vec<_>>();
        assert_eq!(verses.len(), crate::app::OCCURRENCE_LIMIT);
        let first = verses[0].0;
        let label = format!("{} · test gloss", desktop.app.label(first));
        desktop.app.word_study = Some(crate::app::WordStudy {
            strong: "G0025".into(),
            word: None,
            entry: None,
            occurrences: None,
        });
        desktop.report = Some((
            "G0025".into(),
            Ok(WordReport {
                total: 602,
                verse_count: 601,
                loaded_books: 27,
                verses,
                ..Default::default()
            }),
        ));
        desktop.app.mode = Mode::Occurrences;
        frame(&mut desktop, &ctx, vec![], [1380.0, 900.0]);
        let output = frame(&mut desktop, &ctx, vec![], [1380.0, 900.0]);
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.text().contains("Showing the first 600"))));
        click(&mut desktop, &ctx, &label);
        assert_eq!(desktop.app.position(), Some(first));
    }

    #[test]
    fn recovery_export_is_accessible_inside_failed_note_editor() {
        let (mut desktop, ctx) = fixture();
        let dir = crate::storage::TestDir::new();
        let path = dir.0.join("study.json");
        std::fs::write(&path, b"damaged").unwrap();
        desktop.app.study = crate::study::Study::load_from(path);
        desktop.app.mode = Mode::Note;
        desktop.app.note_buffer = "export this unfinished note".into();
        assert!(!desktop.app.save());
        click(&mut desktop, &ctx, "Export unsaved work");
        assert_eq!(desktop.app.mode, Mode::Note);
        let recovery = std::fs::read_dir(&dir.0)
            .unwrap()
            .flatten()
            .find(|f| f.file_name().to_string_lossy().contains(".recovery."))
            .unwrap();
        assert_eq!(
            crate::study::Study::load_from(recovery.path())
                .note_draft
                .unwrap()
                .text,
            "export this unfinished note"
        );
    }

    #[test]
    fn fast_save_shortcut_uses_the_key_events_modifiers() {
        let (mut desktop, ctx) = fixture();
        let dir = crate::storage::TestDir::new();
        let path = dir.0.join("study.json");
        desktop.app.study = crate::study::Study::load_from(path.clone());
        desktop.app.mode = Mode::Note;
        desktop.app.note_buffer = "quick save".into();
        frame(
            &mut desktop,
            &ctx,
            vec![
                egui::Event::Key {
                    key: egui::Key::S,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                },
                egui::Event::Key {
                    key: egui::Key::S,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            [1380.0, 900.0],
        );
        assert_eq!(desktop.app.mode, Mode::Read);
        assert_eq!(
            crate::study::Study::load_from(path).note(desktop.app.position().unwrap()),
            Some("quick save")
        );
    }

    #[test]
    fn failed_close_is_canceled_and_note_draft_survives_normal_close() {
        let (mut desktop, ctx) = fixture();
        let dir = crate::storage::TestDir::new();
        let path = dir.0.join("study.json");
        desktop.app.study = crate::study::Study::load_from(path.clone());
        desktop.app.mode = Mode::Note;
        desktop.app.note_buffer = "latest unfinished text".into();
        let close_frame = |desktop: &mut Desktop| {
            let mut input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1380.0, 900.0),
                )),
                ..Default::default()
            };
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .events
                .push(egui::ViewportEvent::Close);
            ctx.run(input, |ctx| desktop.show(ctx))
        };
        close_frame(&mut desktop);
        assert_eq!(
            crate::study::Study::load_from(path.clone())
                .note_draft
                .unwrap()
                .text,
            "latest unfinished text"
        );
        std::fs::write(&path, b"{}").unwrap();
        desktop.app.note_buffer = "keep this in memory".into();
        let output = close_frame(&mut desktop);
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::CancelClose))
        );
        assert!(desktop.app.save_error.is_some());
        assert_eq!(desktop.app.note_buffer, "keep this in memory");
        assert_eq!(std::fs::read(path).unwrap(), b"{}");
    }

    #[test]
    fn live_theme_change_updates_open_gui_without_resetting_reading_state() {
        let (mut desktop, ctx) = fixture();
        let root =
            std::env::temp_dir().join(format!("omascripture-gui-theme-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("colors.toml");
        let dark = "mode='dark'\nbackground='#101020'\nforeground='#eeeeff'\naccent='#aa88ff'";
        let light = "mode='light'\nbackground='#faf4ed'\nforeground='#575279'\naccent='#56949f'";
        std::fs::write(&path, dark).unwrap();
        desktop.theme_source = ThemeSource::from_paths(vec![path.clone()]);
        desktop.reference = "John 3:16".into();
        desktop.app.mode = Mode::Settings;
        let position = desktop.app.loc;
        frame(&mut desktop, &ctx, vec![], [1380.0, 900.0]);
        assert!(ctx.style().visuals.dark_mode);
        std::fs::write(root.join("next.toml"), light).unwrap();
        std::fs::rename(root.join("next.toml"), &path).unwrap();
        std::thread::sleep(Duration::from_millis(800));
        frame(&mut desktop, &ctx, vec![], [1380.0, 900.0]);
        assert!(!ctx.style().visuals.dark_mode);
        assert_eq!(
            ctx.style().visuals.panel_fill,
            Palette::parse(light).unwrap().panel
        );
        assert_eq!(desktop.app.loc, position);
        assert_eq!(desktop.reference, "John 3:16");
        assert_eq!(desktop.app.mode, Mode::Settings);
        desktop.app.study.gui_theme = "dark".into();
        frame(&mut desktop, &ctx, vec![], [1380.0, 900.0]);
        assert!(ctx.style().visuals.dark_mode);
        desktop.app.study.gui_theme.clear();
        frame(&mut desktop, &ctx, vec![], [1380.0, 900.0]);
        assert!(!ctx.style().visuals.dark_mode);
        let saved = serde_json::to_string(&desktop.app.study).unwrap();
        let restored: crate::study::Study = serde_json::from_str(&saved).unwrap();
        assert!(restored.gui_theme.is_empty());
        let old: crate::study::Study = serde_json::from_str(r#"{"gui_light":true}"#).unwrap();
        assert!(old.gui_theme.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    fn fixture() -> (Desktop, egui::Context) {
        let mut app = App::new();
        app.study = crate::study::Study::default();
        app.sidebar = 0;
        app.mode = Mode::Read;
        let mut t = providers::open("net").unwrap();
        t.online = None;
        t.abbreviation = "test".into();
        t.translation = "Test Bible".into();
        for chapter in &mut t.books[42].chapters {
            for verse in &mut chapter.verses {
                verse.text = format!(
                    "Chapter {} verse {}: a passage for reading.",
                    chapter.chapter, verse.verse
                );
            }
        }
        app.translation = Some(t);
        app.loc = Loc {
            book: 42,
            chapter: 2,
            verse: 15,
        };
        let ctx = egui::Context::default();
        let desktop = Desktop::new(app);
        configure(&ctx, desktop.colors);
        (desktop, ctx)
    }
    fn frame(
        d: &mut Desktop,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        size: [f32; 2],
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size.into())),
                events,
                ..Default::default()
            },
            |ctx| d.show(ctx),
        )
    }
    fn text_center(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.text() == label
            {
                return text.pos + text.galley.size() * 0.5;
            }
        }
        panic!("Missing visible control {label}");
    }
    fn click(d: &mut Desktop, ctx: &egui::Context, label: &str) {
        let _ = frame(d, ctx, vec![], [1380.0, 900.0]);
        let output = frame(d, ctx, vec![], [1380.0, 900.0]);
        let pos = text_center(&output, label);
        frame(
            d,
            ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            [1380.0, 900.0],
        );
        frame(
            d,
            ctx,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            [1380.0, 900.0],
        );
    }
    #[test]
    fn graphical_navigation_and_dialogs_respond_to_mouse() {
        let (mut d, ctx) = fixture();
        click(&mut d, &ctx, "›");
        assert_eq!(d.app.loc.chapter, 3);
        click(&mut d, &ctx, "Settings");
        assert_eq!(d.app.mode, Mode::Settings);
        click(&mut d, &ctx, "Close");
        assert_eq!(d.app.mode, Mode::Read);
        click(&mut d, &ctx, "Add note");
        assert_eq!(d.app.mode, Mode::Note);
        d.app.note_buffer = "Unsaved test note".into();
        click(&mut d, &ctx, "Cancel");
        assert_eq!(d.app.mode, Mode::Read);
        assert!(d.app.study.notes.is_empty());
        click(&mut d, &ctx, "Search");
        assert_eq!(d.app.mode, Mode::Search);
        d.app.search_query = "passage".into();
        click(&mut d, &ctx, "Search this Bible");
        assert_eq!(d.app.mode, Mode::SearchResults);
        assert!(!d.app.search_hits.is_empty());
    }
    #[test]
    fn native_layout_renders_reading_and_settings_at_supported_sizes() {
        let (mut d, ctx) = fixture();
        for size in [[800.0, 600.0], [1380.0, 900.0], [1920.0, 1080.0]] {
            d.app.mode = Mode::Read;
            let _ = frame(&mut d, &ctx, vec![], size);
            let output = frame(&mut d, &ctx, vec![], size);
            let _ = text_center(&output, "John 3");
            d.app.mode = Mode::Settings;
            let _ = frame(&mut d, &ctx, vec![], size);
            let output = frame(&mut d, &ctx, vec![], size);
            let _ = text_center(&output, "Default Bible");
        }
    }
}
