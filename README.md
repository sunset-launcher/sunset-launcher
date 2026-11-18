<div align="center">
  <img src="logo.png" width="96" height="96" alt="Sunset Launcher" />
  <h1>Sunset Launcher</h1>
  <p><strong>Fast, native Minecraft launcher built in pure Rust (<code>eframe</code> / <code>egui</code>)</strong></p>

  <p>
    <a href="https://github.com/svarkawm/sunset-launcher/releases">
      <img src="https://img.shields.io/github/v/release/svarkawm/sunset-launcher?color=FF7833&label=Release&style=for-the-badge" alt="Release" />
    </a>
    <a href="https://github.com/svarkawm/sunset-launcher/actions/workflows/build.yml">
      <img src="https://img.shields.io/github/actions/workflow/status/svarkawm/sunset-launcher/build.yml?branch=main&label=Build&style=for-the-badge" alt="Build & Release CI" />
    </a>
    <img src="https://img.shields.io/badge/Rust-2021-orange?style=for-the-badge&logo=rust" alt="Rust 2021" />
    <img src="https://img.shields.io/badge/Platform-Linux%20%7C%20Windows-blue?style=for-the-badge" alt="Linux & Windows" />
    <a href="LICENSE">
      <img src="https://img.shields.io/badge/License-MIT-green?style=for-the-badge" alt="MIT License" />
    </a>
  </p>

  <p>
    <img width="920" alt="Sunset Launcher — Instances" src="docs/screenshots/library.jpg" />
  </p>
</div>

## Features
- **Instances & Profiles**: create, duplicate, edit, and delete instances with isolated `instances/<instance_name>` game directories.
- **Mod Loaders**: supports **Vanilla**, **Fabric**, **Quilt**, **Forge**, and **NeoForge** via official Mojang, Fabric/Quilt Meta, and Forge/NeoForge Maven APIs (including Forge/NeoForge installer processors).
- **Modrinth Integration & `.mrpack`**: search and install mods, modpacks (`.mrpack`), resource packs, and shaders; import/export `.mrpack` archives and toggle enabled content files.
- **Worlds, Quick-Play & Backups**: browse instance worlds, launch directly into a world (`--quickPlaySingleplayer`), and create, restore, or delete `.zip` world backups.
- **High-Speed Parallel Downloads**: pooled async downloads (up to 64 concurrent transfers) with streaming SHA-1 verification and automatic extraction of native libraries (`natives`).
- **Accounts & Skins**: supports **Microsoft** (OAuth Device Code), **Ely.by** (`authlib-injector`), and **Offline** local accounts with cached skin head/body previews.
- **JVM & Runtime Management**: RAM allocation (`-Xms` / `-Xmx`), custom JVM arguments & presets (`G1GC`, `ZGC`, `Aikar's`), automatic Java runtime discovery, and minimize-on-launch behavior.
- **Live Console & Crash Diagnostics**: real-time filtered `stdout` / `stderr` game log viewer, crash detection banner, and one-click crash report copying.
- **Auto-Updates & Desktop Integration**: built-in GitHub Releases update checker/installer, single-instance lock, Linux `.desktop` entry, and Windows Start Menu shortcut with `.ico`.
- **Localization, Themes & Discord RPC**: 9 UI languages, customizable themes/fonts, Command Palette (`Ctrl+K`), and language-aware Discord Rich Presence.

## Screenshots

<table>
  <tr>
    <td><img alt="Modrinth Discover" src="docs/screenshots/discover.jpg" /></td>
    <td><img alt="Instance Content &amp; Mods" src="docs/screenshots/mods.jpg" /></td>
  </tr>
  <tr>
    <td><img alt="Accounts &amp; Skin Preview" src="docs/screenshots/accounts.jpg" /></td>
    <td><img alt="Launcher Settings &amp; Sunset Theme" src="docs/screenshots/settings.jpg" /></td>
  </tr>
  <tr>
    <td colspan="2" align="center"><img width="520" alt="Live Console" src="docs/screenshots/console.png" /></td>
  </tr>
</table>

## Quick Start (Releases)

Download the prebuilt binary for your OS from [GitHub Releases](https://github.com/svarkawm/sunset-launcher/releases):

- **Linux (`sunset-launcher-linux-x64`)**: mark the file as executable before the first launch:
  ```bash
  chmod +x ~/Downloads/sunset-launcher-linux-x64
  ~/Downloads/sunset-launcher-linux-x64
  ```
  On first run, the launcher automatically installs its icon and application menu entry (`~/.local/share/applications/sunset-launcher.desktop`).
- **Windows (`sunset-launcher-windows-x64.exe`)**: run the `.exe` directly (automatically creates a Start Menu shortcut and icon on first launch).
  > **Note on Windows SmartScreen**: Release binaries are currently unsigned open-source builds, so Windows may show an *"Unknown publisher"* SmartScreen prompt on first launch (`More info` → `Run anyway`). You can verify binary integrity with the published `.sha256` checksum or build directly from source.

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
- **Linux**: `~/.local/share/sunset-launcher`
- **Windows**: `%LOCALAPPDATA%\sunset-launcher` (`C:\Users\<User>\AppData\Local\sunset-launcher`)

Inside this folder:
- `profiles.json` — instance profiles
- `accounts.json` — saved accounts
- `versions/` — version JSONs, client JARs, loader overlays, and extracted `natives`
- `libraries/` — shared Maven libraries
- `assets/` — asset indexes and resource objects
- `instances/` — isolated game directories per instance
- `java/` — optional launcher-local Java runtimes

## Privacy
- **Privacy Policy**: Sunset Launcher does **not** collect telemetry, analytics, or crash tracking, and does not send any personal data to developer-controlled servers. All credentials (Microsoft / Ely.by tokens, accounts, and profiles) are stored locally in the launcher data directory. Network requests occur only to third-party services explicitly required by user features (Mojang/Minecraft, Fabric/Quilt/Forge/NeoForge, Modrinth, Adoptium Java, Microsoft OAuth / Ely.by authentication, GitHub Releases for update checks, and local Discord RPC IPC when enabled).

## Credits & License
- **Author**: `svarka` ([@svarkawm](https://github.com/svarkawm))
- **Co-Author / AI Contributor**: **Antigravity (Gemini)**
- **License**: [MIT](LICENSE)

