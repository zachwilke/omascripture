OmaScripture now includes an About dialog with a personal signature and author links.

- See “Made by Zach Wilke,” the app version, and links to GitHub @zachwilke and X @Zachwilke_1.
- Open About from Settings, the reading footer, or the first-launch welcome screen.
- Find the project source, releases, and license information in the same dialog.

Install or update:

```bash
curl -fsSL https://raw.githubusercontent.com/zachwilke/omascripture/main/setup.sh | bash
```

The installer verifies the download and preserves your Bible choice, downloaded resources, notes, and preferences. No Rust toolchain or sudo required.

Prebuilt for x86_64 Linux with glibc 2.39 or newer, a Wayland/X11 desktop, and OpenGL drivers.

Validation: 62 automated tests passed, Clippy passed with warnings denied, installer regression checks passed, and the About dialog was visually checked on Omarchy.
