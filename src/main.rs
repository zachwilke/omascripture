mod app;
mod bible;
mod gui;
mod gui_theme;
mod harmony;
mod menu;
mod plans;
mod providers;
mod resources;
mod settings;
mod study;
mod storage;
mod sword;
mod ui;
mod v11n;
mod votd;
mod word_data;

use app::App;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() {
    println!(
        "OmaScripture {VERSION} — read and study the Bible on your desktop

USAGE:
    omascripture [OPTIONS] [REFERENCE]

ARGS:
    REFERENCE                 Open at a reference, e.g. \"John 3:16\", \"Ps 23\", \"1 Jn 2\"

OPTIONS:
    --gui                     Open the desktop app (default)
    --tui                     Use the optional terminal interface
    -t, --translation <ID>    Use this translation (e.g. kjv, web, asv)
    -d, --download <ID>       Download a translation and exit
    -l, --list                List downloaded translations and exit
    -c, --catalog             List all translations available for download and exit
    -r, --resources           List study resource packs (commentaries, lexicons, ...) and exit
    -i, --install <ID>        Download a study resource pack and exit (repeatable)
    -x, --remove <ID>         Remove a study resource pack and exit
    --providers               Show online providers and configuration status
    --init-providers          Create a private provider configuration template
    --api-bibles              List Bible IDs authorized for your API.Bible key
    --votd                    Print the verse of the day and exit
    --votd-bar                Print the verse of the day as Omarchy bar JSON and exit
    -h, --help                Show this help
    -V, --version             Show version

Data directory: {}",
        bible::data_dir().display()
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut tui = false;
    let mut translation: Option<String> = None;
    let mut reference_parts: Vec<String> = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--tui" => tui = true,
            "--gui" => tui = false,
            "--providers" => {
                println!("Config: {}", providers::config_path().display());
                if let Err(e) = providers::config() {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
                for p in providers::catalog() {
                    println!(
                        "{:<32} {:<30} {}",
                        p.abbreviation,
                        p.translation,
                        providers::status(&p.abbreviation)
                    );
                }
                return;
            }
            "--init-providers" => {
                match providers::init_config() {
                    Ok(path) => {
                        println!("Edit {} to add provider keys (see README)", path.display())
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        std::process::exit(1);
                    }
                }
                return;
            }
            "--api-bibles" => {
                match providers::available_api_bibles() {
                    Ok(list) => {
                        for (id, name) in list {
                            println!("{id}  {name}");
                        }
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        std::process::exit(1);
                    }
                }
                return;
            }
            "-h" | "--help" => {
                usage();
                return;
            }
            "-V" | "--version" => {
                println!("omascripture {VERSION}");
                return;
            }
            "-l" | "--list" => {
                let list = bible::installed();
                if list.is_empty() {
                    println!(
                        "No translations downloaded yet. Run `omascripture -d kjv` or pick one in the app."
                    );
                }
                for t in list {
                    println!(
                        "{:<10} {:<40} {}",
                        t.abbreviation, t.translation, t.language
                    );
                }
                return;
            }
            "-c" | "--catalog" => {
                match bible::fetch_catalog() {
                    Ok(list) => {
                        for t in list {
                            let mark = if bible::is_installed(&t.abbreviation) {
                                "*"
                            } else {
                                " "
                            };
                            println!(
                                "{mark} {:<14} {:<50} {}",
                                t.abbreviation, t.translation, t.language
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Could not fetch catalog: {e}");
                        std::process::exit(1);
                    }
                }
                return;
            }
            "-d" | "--download" => {
                let Some(id) = args.next() else {
                    eprintln!("--download needs a translation id");
                    std::process::exit(2);
                };
                eprint!("Downloading {id}… ");
                match bible::download(&id) {
                    Ok(t) => eprintln!("done: {} ({} books)", t.translation, t.books.len()),
                    Err(e) => {
                        eprintln!("failed: {e}");
                        std::process::exit(1);
                    }
                }
                return;
            }
            "-r" | "--resources" => {
                for p in resources::PACKS {
                    let mark = if resources::is_installed(p.id) {
                        "*"
                    } else {
                        " "
                    };
                    println!(
                        "{mark} {:<16} {:<44} {:<14} {:<7} {}",
                        p.id,
                        p.name,
                        p.kind.label(),
                        p.size,
                        p.license
                    );
                }
                println!("\n* = installed. Install with: omascripture --install <id>");
                return;
            }
            "-i" | "--install" => {
                let Some(id) = args.next() else {
                    eprintln!("--install needs a resource id (see --resources)");
                    std::process::exit(2);
                };
                let ids: Vec<String> = if id == "all" {
                    resources::PACKS.iter().map(|p| p.id.to_string()).collect()
                } else {
                    vec![id]
                };
                for id in ids {
                    if resources::is_installed(&id) {
                        eprintln!("{id}: already installed");
                        continue;
                    }
                    eprint!("Installing {id}… ");
                    match resources::install(&id) {
                        Ok(()) => eprintln!("done"),
                        Err(e) => {
                            eprintln!("failed: {e}");
                            std::process::exit(1);
                        }
                    }
                }
                return;
            }
            "-x" | "--remove" => {
                let Some(id) = args.next() else {
                    eprintln!("--remove needs a resource id");
                    std::process::exit(2);
                };
                match resources::remove(&id) {
                    Ok(()) => eprintln!("removed {id}"),
                    Err(e) => eprintln!("could not remove {id}: {e}"),
                }
                return;
            }
            "--votd" | "--votd-bar" => {
                print_votd(a == "--votd-bar", translation.clone());
                return;
            }
            "-t" | "--translation" => {
                translation = args.next().map(|s| s.to_lowercase());
            }
            s if s.starts_with("--translation=") => {
                translation = Some(s["--translation=".len()..].to_lowercase());
            }
            _ => reference_parts.push(a),
        }
    }
    let reference = if reference_parts.is_empty() {
        None
    } else {
        Some(reference_parts.join(" "))
    };

    let mut app = App::new();
    if !tui {
        app.start_gui(translation, reference);
        if let Err(e) = gui::run(app) {
            eprintln!("Could not open OmaScripture: {e}");
            std::process::exit(1);
        }
        return;
    }
    app.start(translation, reference);

    let mut terminal = ratatui::init();
    let _ = crossterm::execute!(std::io::stdout(), EnableMouseCapture);
    let result = run(&mut terminal, &mut app);
    let _ = crossterm::execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    if !app.save() {
        eprintln!("{}", app.save_error.as_deref().unwrap_or("Study save failed"));
        if let Ok(path) = app.study.export_recovery() { eprintln!("Recovery copy: {}", path.display()); }
    }
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn print_votd(bar: bool, translation: Option<String>) {
    let study = study::Study::load();
    let abbr = translation
        .or(study.translation)
        .or_else(|| bible::installed().first().map(|i| i.abbreviation.clone()));
    let Some(abbr) = abbr else {
        if bar {
            println!(
                "{{\"text\":\"󰂺 OmaScripture\",\"tooltip\":\"No translation downloaded yet\"}}"
            );
        } else {
            eprintln!("No translation downloaded yet.");
        }
        return;
    };
    let result = if providers::is_online(&abbr) {
        providers::open(&abbr)
    } else {
        bible::load(&abbr).map_err(|e| e.to_string())
    };
    let Ok(t) = result else {
        eprintln!("Could not load translation {abbr}");
        return;
    };
    let Some((loc, _)) = votd::today(&t) else {
        return;
    };
    let reference = t.reference(loc);
    let text = if t.online.is_some() {
        match providers::fetch(&abbr, loc.book, loc.chapter) {
            Ok(p) => {
                let number = t.position_from_loc(loc).verse;
                let Some(v) = p.verses.iter().find(|v| v.verse == number) else {
                    eprintln!("Provider did not return the requested verse");
                    return;
                };
                format!("{}\n{}", v.text, p.notice)
            }
            Err(e) => {
                eprintln!("{e}");
                return;
            }
        }
    } else {
        t.verse_text(loc).unwrap_or("").to_string()
    };
    if bar {
        println!(
            "{}",
            serde_json::json!({"text": format!("󰂺 {reference}"), "tooltip": format!("{} ({})\n{}", reference, t.abbreviation.to_uppercase(), text)})
        );
    } else {
        println!("{reference} ({})\n{text}", t.abbreviation.to_uppercase());
    }
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;
        if event::poll(Duration::from_millis(150))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => app.handle_key(key),
                Event::Mouse(m) => app.handle_mouse(m),
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
        app.poll_messages();
        if app.should_quit {
            return Ok(());
        }
    }
}
