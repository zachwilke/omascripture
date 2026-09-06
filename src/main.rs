mod app;
mod bible;
mod harmony;
mod plans;
mod resources;
mod study;
mod sword;
mod ui;
mod v11n;
mod votd;

use app::App;
use crossterm::event::{self, Event, KeyEventKind};
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() {
    println!(
        "OmaScripture {VERSION} — read and study the Bible in your terminal

USAGE:
    omascripture [OPTIONS] [REFERENCE]

ARGS:
    REFERENCE                 Open at a reference, e.g. \"John 3:16\", \"Ps 23\", \"1 Jn 2\"

OPTIONS:
    -t, --translation <ID>    Use this translation (e.g. kjv, web, asv)
    -d, --download <ID>       Download a translation and exit
    -l, --list                List downloaded translations and exit
    -c, --catalog             List all translations available for download and exit
    -r, --resources           List study resource packs (commentaries, lexicons, ...) and exit
    -i, --install <ID>        Download a study resource pack and exit (repeatable)
    -x, --remove <ID>         Remove a study resource pack and exit
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
    let mut translation: Option<String> = None;
    let mut reference_parts: Vec<String> = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
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
                    println!("No translations downloaded yet. Run `omascripture -d kjv` or pick one in the app.");
                }
                for t in list {
                    println!("{:<10} {:<40} {}", t.abbreviation, t.translation, t.language);
                }
                return;
            }
            "-c" | "--catalog" => match bible::fetch_catalog() {
                Ok(list) => {
                    for t in list {
                        let mark = if bible::is_installed(&t.abbreviation) { "*" } else { " " };
                        println!("{mark} {:<14} {:<50} {}", t.abbreviation, t.translation, t.language);
                    }
                }
                Err(e) => {
                    eprintln!("Could not fetch catalog: {e}");
                    std::process::exit(1);
                }
            },
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
                    let mark = if resources::is_installed(p.id) { "*" } else { " " };
                    println!("{mark} {:<16} {:<44} {:<14} {:<7} {}", p.id, p.name, p.kind.label(), p.size, p.license);
                }
                println!("\n* = installed. Install with: omascripture --install <id>");
                return;
            }
            "-i" | "--install" => {
                let Some(id) = args.next() else {
                    eprintln!("--install needs a resource id (see --resources)");
                    std::process::exit(2);
                };
                let ids: Vec<String> = if id == "all" { resources::PACKS.iter().map(|p| p.id.to_string()).collect() } else { vec![id] };
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
    let reference = if reference_parts.is_empty() { None } else { Some(reference_parts.join(" ")) };

    let mut app = App::new();
    app.start(translation, reference);

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    app.save();
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
            println!("{{\"text\":\"󰂺 OmaScripture\",\"tooltip\":\"No translation downloaded yet\"}}");
        } else {
            eprintln!("No translation downloaded yet.");
        }
        return;
    };
    let Ok(t) = bible::load(&abbr) else {
        eprintln!("Could not load translation {abbr}");
        return;
    };
    let Some((loc, _)) = votd::today(&t) else { return };
    let reference = t.reference(loc);
    let text = t.verse_text(loc).unwrap_or("").to_string();
    if bar {
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        println!(
            "{{\"text\":\"󰂺 {}\",\"tooltip\":\"{} ({})\\n{}\"}}",
            esc(&reference),
            esc(&reference),
            t.abbreviation.to_uppercase(),
            esc(&text)
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
