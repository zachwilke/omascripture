//! Read Omarchy's current palette by path, so directory swaps and symlink
//! changes are picked up without a hook or a window restart.
use eframe::egui::Color32;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub light: bool,
    pub background: Color32,
    pub panel: Color32,
    pub surface: Color32,
    pub foreground: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub selection: Color32,
    pub border: Color32,
    pub yellow: Color32,
    pub green: Color32,
    pub blue: Color32,
    pub red: Color32,
}

fn rgb(value: &str) -> Option<Color32> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let n = u32::from_str_radix(hex, 16).ok()?;
    Some(Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

pub fn blend(a: Color32, b: Color32, amount: f32) -> Color32 {
    let channel = |a: u8, b: u8| (a as f32 * (1.0 - amount) + b as f32 * amount).round() as u8;
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

impl Palette {
    pub fn fallback(light: bool) -> Self {
        static LIGHT: std::sync::OnceLock<Palette> = std::sync::OnceLock::new();
        static DARK: std::sync::OnceLock<Palette> = std::sync::OnceLock::new();
        *(if light { &LIGHT } else { &DARK }).get_or_init(|| {
            Self::parse(if light {
                "mode='light'\nbackground='#f9f7f1'\nforeground='#30353b'\naccent='#795b2e'"
            } else {
                "mode='dark'\nbackground='#14191f'\nforeground='#e1e2df'\naccent='#d6b57e'"
            })
            .expect("built-in palette")
        })
    }

    pub fn parse(text: &str) -> Option<Self> {
        let values: std::collections::BTreeMap<String, toml::Value> = toml::from_str(text).ok()?;
        let get = |key: &str| values.get(key).and_then(|v| v.as_str()).and_then(rgb);
        // Reject malformed known colors, including half-written updates.
        for key in [
            "background",
            "foreground",
            "accent",
            "selection",
            "muted",
            "dark_background",
            "lighter_background",
            "light_foreground",
            "yellow",
            "green",
            "blue",
            "red",
        ] {
            if values.contains_key(key) && get(key).is_none() {
                return None;
            }
        }
        let background = get("background")?;
        let foreground = get("foreground")?;
        let accent = get("accent")?;
        let light = match values.get("mode").and_then(|v| v.as_str()) {
            Some("light") => true,
            Some("dark") => false,
            None => {
                0.2126 * background.r() as f32
                    + 0.7152 * background.g() as f32
                    + 0.0722 * background.b() as f32
                    > 150.0
            }
            _ => return None,
        };
        Some(Self {
            light,
            background,
            foreground,
            accent,
            panel: get("dark_background").unwrap_or(blend(background, foreground, 0.035)),
            surface: get("lighter_background").unwrap_or(blend(background, foreground, 0.08)),
            // Omarchy's `muted` is a surface/border color, not readable text.
            muted: get("light_foreground").unwrap_or(blend(background, foreground, 0.7)),
            border: get("muted").unwrap_or(blend(background, foreground, 0.18)),
            selection: get("selection").unwrap_or(blend(background, accent, 0.25)),
            yellow: get("yellow").unwrap_or(Color32::from_rgb(184, 142, 47)),
            green: get("green").unwrap_or(Color32::from_rgb(77, 164, 118)),
            blue: get("blue").unwrap_or(Color32::from_rgb(71, 139, 200)),
            red: get("red").unwrap_or(Color32::from_rgb(187, 86, 95)),
        })
    }

    pub fn highlight(self, index: u8, selected: bool) -> Color32 {
        let color = match index {
            1 => self.yellow,
            2 => self.green,
            3 => self.blue,
            4 => self.red,
            _ => {
                return if selected {
                    blend(self.background, self.accent, 0.09)
                } else {
                    Color32::TRANSPARENT
                };
            }
        };
        blend(self.background, color, 0.17)
    }
}

pub struct ThemeSource {
    paths: Vec<PathBuf>,
    palette: Option<Palette>,
    next_check: Instant,
    source_text: Option<String>,
}

impl ThemeSource {
    pub fn new() -> Self {
        let mut paths = Vec::new();
        if let Some(path) = std::env::var_os("OMASCRIPTURE_THEME_FILE") {
            paths.push(PathBuf::from(path));
        } else {
            if let Some(state) = dirs::state_dir() {
                paths.push(state.join("omarchy/current/theme/colors.toml"));
            }
            if let Some(config) = dirs::config_dir() {
                paths.push(config.join("omarchy/current/theme/colors.toml"));
            }
        }
        Self::from_paths(paths)
    }

    pub(crate) fn from_paths(paths: Vec<PathBuf>) -> Self {
        let mut source = Self {
            paths,
            palette: None,
            next_check: Instant::now(),
            source_text: None,
        };
        source.refresh();
        source
    }

    fn refresh(&mut self) {
        for path in &self.paths {
            if let Ok(text) = std::fs::read_to_string(path) {
                if self.source_text.as_ref() == Some(&text) {
                    break;
                }
                let Some(palette) = Palette::parse(&text) else {
                    continue;
                };
                self.palette = Some(palette);
                self.source_text = Some(text);
                // Stay with the selected location across transient removal.
                self.paths = vec![path.clone()];
                break;
            }
        }
        self.next_check = Instant::now() + Duration::from_millis(750);
    }

    pub fn resolve(&mut self, preference: &str, fallback_light: bool) -> Palette {
        if Instant::now() >= self.next_check {
            self.refresh();
        }
        match preference {
            "light" => Palette::fallback(true),
            "dark" => Palette::fallback(false),
            _ => self
                .palette
                .unwrap_or_else(|| Palette::fallback(fallback_light)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const DARK: &str = "mode='dark'\nbackground='#101020'\nforeground='#eeeeff'\naccent='#aa88ff'\nmuted='#242438'\nlight_foreground='#b8b8cc'";
    const LIGHT: &str =
        "mode='light'\nbackground='#faf4ed'\nforeground='#575279'\naccent='#56949f'";

    #[test]
    fn parses_modes_and_distinguishes_muted_surface_from_text() {
        let dark = Palette::parse(DARK).unwrap();
        assert!(!dark.light);
        assert_eq!(dark.border, rgb("#242438").unwrap());
        assert_eq!(dark.muted, rgb("#b8b8cc").unwrap());
        assert!(Palette::parse(LIGHT).unwrap().light);
        assert!(
            Palette::parse(&LIGHT.replace("mode='light'\n", ""))
                .unwrap()
                .light
        );
        for bad in [
            "",
            "background='#fff'",
            &DARK.replace("#aa88ff", "#zzzzzz"),
            &format!("{DARK}\nred='oops'"),
        ] {
            assert!(Palette::parse(bad).is_none());
        }
    }

    #[test]
    fn reloads_after_directory_swap_and_preserves_palette_on_invalid_updates() {
        let root = std::env::temp_dir().join(format!("omascripture-theme-{}", std::process::id()));
        std::fs::create_dir_all(root.join("theme")).unwrap();
        let path = root.join("theme/colors.toml");
        std::fs::write(&path, DARK).unwrap();
        let mut source = ThemeSource::from_paths(vec![path.clone()]);
        let dark = source.resolve("", false);
        std::fs::rename(root.join("theme"), root.join("old-theme")).unwrap();
        source.refresh();
        assert_eq!(source.resolve("", false), dark);
        std::fs::create_dir(root.join("theme")).unwrap();
        std::fs::write(&path, "partial write").unwrap();
        source.refresh();
        assert_eq!(source.resolve("", false), dark);
        std::fs::write(&path, LIGHT).unwrap();
        source.next_check = Instant::now();
        assert!(source.resolve("", false).light);
        assert_eq!(source.resolve("dark", false), Palette::fallback(false));
        assert!(source.resolve("omarchy", false).light);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_theme_uses_manual_fallback_and_legacy_path_is_supported() {
        let source = &mut ThemeSource::from_paths(vec![]);
        assert_eq!(source.resolve("", true), Palette::fallback(true));
        assert_eq!(source.resolve("", false), Palette::fallback(false));
        let root =
            std::env::temp_dir().join(format!("omascripture-legacy-theme-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("colors.toml");
        std::fs::write(&path, LIGHT).unwrap();
        let mut source = ThemeSource::from_paths(vec![root.join("missing"), path]);
        assert!(source.resolve("", false).light);
        std::fs::remove_dir_all(root).unwrap();
    }
}
