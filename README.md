<img width="1881" height="1007" alt="screenshot_2026-11-13_22-03-30" src="https://github.com/user-attachments/assets/926a4243-2ce9-4b93-8d6d-6c1eb5d41ddc" />
<img width="1883" height="1007" alt="screenshot_2026-11-13_22-03-22" src="https://github.com/user-attachments/assets/4e664c71-5f38-4eb8-9c67-5116fdcb4dd6" />
<img width="1882" height="1008" alt="screenshot_2026-11-13_22-03-17" src="https://github.com/user-attachments/assets/addc6e05-f003-4708-a5e8-f299b83dac26" />
<img width="1882" height="1006" alt="screenshot_2026-11-13_22-03-09" src="https://github.com/user-attachments/assets/ec388498-8890-4041-a68a-d5444caca487" />
<img width="1880" height="1006" alt="screenshot_2026-11-13_22-02-57" src="https://github.com/user-attachments/assets/4415462e-0d49-4631-8143-429063fcaf02" />
# Svarka Launcher

Native Minecraft launcher built in Rust (`eframe` / `egui`).

## Features
- **Instances & Profiles**: create, duplicate, edit, and delete instances with isolated `instances/<profile_id>` game directories.
- **Mod Loaders**: supports **Vanilla**, **Fabric**, **Quilt**, **Forge**, and **NeoForge** via official Mojang, Fabric/Quilt Meta, and Forge/NeoForge Maven APIs (including Forge/NeoForge installer processors).
- **Modrinth Integration**: search and install mods, modpacks (`.mrpack`), resource packs, and shaders with file enable/disable toggling inside each instance's Content manager.
- **High-Speed Parallel Downloads**: pooled async downloads (up to 64 concurrent transfers) with streaming SHA-1 verification and automatic extraction of native libraries (`natives`).
- **Accounts**: supports **Microsoft** (OAuth Device Code), **Ely.by** (`authlib-injector`), and **Offline** local accounts.
- **JVM & Runtime Management**: RAM allocation (`-Xms` / `-Xmx`), custom JVM arguments & presets (`G1GC`, `ZGC`, `Aikar's`), and automatic Java runtime discovery.
- **Live Console**: real-time filtered `stdout` / `stderr` game log viewer with auto-scroll and copy/clear actions.

## Quick Start (Releases)

Download the prebuilt binary for your OS from [GitHub Releases](https://github.com/svarkawm/svarka-launcher/releases):

- **Linux (`svarka-launcher-linux-x64`)**: mark the file as executable before the first launch:
  ```bash
  chmod +x ~/Downloads/svarka-launcher-linux-x64
  ~/Downloads/svarka-launcher-linux-x64
  ```
  On first run, the launcher automatically installs its icon and application menu entry (`~/.local/share/applications/svarka-launcher.desktop`).
- **Windows (`svarka-launcher-windows-x64.exe`)**: run the `.exe` directly.

## Build & Run

```bash
cargo build --release
cargo run
```

## Tests

```bash
cargo test
```

## Data Storage
By default, launcher data is stored in:
- **Linux**: `~/.local/share/mc-launcher`
- **Windows**: `%LOCALAPPDATA%\mc-launcher` (`C:\Users\<User>\AppData\Local\mc-launcher`)
- **macOS**: `~/Library/Application Support/mc-launcher`

Inside this folder:
- `profiles.json` — instance profiles
- `accounts.json` — saved accounts
- `versions/` — version JSONs, client JARs, loader overlays, and extracted `natives`
- `libraries/` — shared Maven libraries
- `assets/` — asset indexes and resource objects
- `instances/` — isolated game directories per instance
- `java/` — optional launcher-local Java runtimes

## Credits & License
- **Author**: `svarka` ([@svarkawm](https://github.com/svarkawm))
- **Co-Author / AI Contributor**: **Antigravity (Gemini)**
- **License**: [MIT](LICENSE)



