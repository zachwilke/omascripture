Install OmaScripture with one command, then choose your default Bible on first launch:

```bash
curl -fsSL https://raw.githubusercontent.com/zachwilke/omascripture/main/setup.sh | bash
```

- Prebuilt x86_64 Linux app: no Rust toolchain or sudo needed.
- Download checksums are verified before installation.
- First launch offers WEB, KJV, NET and NLT, plus the offline translation catalog.
- Offline Bibles download once; online editions load chapters as you read.
- Setup completes only after the Bible loads and your preference is saved.
- Existing study files, notes and Bible choices are preserved on upgrades.

Built on Ubuntu 24.04 for x86_64 Linux (glibc 2.39 or newer), tested on Omarchy.
Requires a Wayland/X11 graphical session and working OpenGL drivers.

For installation without launching the app, append `-s -- --no-launch` to `bash`.
Uninstall with `bash ~/.local/share/omascripture/uninstall.sh`; study data is kept.
