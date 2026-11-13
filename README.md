<div align="center">
  <img src="logo.svg" width="96" height="96" alt="Svarka Launcher" />
  <h1>Svarka Launcher</h1>
  <p><strong>Fast, native Minecraft launcher built in pure Rust (<code>eframe</code> / <code>egui</code>)</strong></p>

  <p>
    <a href="https://github.com/svarkawm/svarka-launcher/releases">
      <img src="https://img.shields.io/github/v/release/svarkawm/svarka-launcher?color=1BD96A&label=Release&style=for-the-badge" alt="Release" />
    </a>
    <a href="https://github.com/svarkawm/svarka-launcher/actions/workflows/build.yml">
      <img src="https://img.shields.io/github/actions/workflow/status/svarkawm/svarka-launcher/build.yml?branch=main&label=Build&style=for-the-badge" alt="Build & Release CI" />
    </a>
    <img src="https://img.shields.io/badge/Rust-2021-orange?style=for-the-badge&logo=rust" alt="Rust 2021" />
    <img src="https://img.shields.io/badge/Platform-Linux%20%7C%20Windows-blue?style=for-the-badge" alt="Linux & Windows" />
    <a href="LICENSE">
      <img src="https://img.shields.io/badge/License-MIT-green?style=for-the-badge" alt="MIT License" />
    </a>
  </p>

  <p>
    <img width="920" alt="Svarka Launcher Preview" src="https://github.com/user-attachments/assets/926a4243-2ce9-4b93-8d6d-6c1eb5d41ddc" />
  </p>
</div>

## Features
- **Instances & Profiles**: create, duplicate, edit, and delete instances with isolated `instances/<profile_id>` game directories.
- **Mod Loaders**: supports **Vanilla**, **Fabric**, **Quilt**, **Forge**, and **NeoForge** via official Mojang, Fabric/Quilt Meta, and Forge/NeoForge Maven APIs (including Forge/NeoForge installer processors).
- **Modrinth Integration**: search and install mods, modpacks (`.mrpack`), resource packs, and shaders with file enable/disable toggling inside each instance's Content manager.
- **High-Speed Parallel Downloads**: pooled async downloads (up to 64 concurrent transfers) with streaming SHA-1 verification and automatic extraction of native libraries (`natives`).
- **Accounts**: supports **Microsoft** (OAuth Device Code), **Ely.by** (`authlib-injector`), and **Offline** local accounts.
- **JVM & Runtime Management**: RAM allocation (`-Xms` / `-Xmx`), custom JVM arguments & presets (`G1GC`, `ZGC`, `Aikar's`), and automatic Java runtime discovery.
- **Command Palette (`Ctrl+K`)**: quick keyboard navigation across instances, pages, and launcher actions.
- **Live Console**: real-time filtered `stdout` / `stderr` game log viewer with auto-scroll and copy/clear actions.

## Screenshots

<table>
  <tr>
    <td><img alt="Screenshot 1" src="https://github.com/user-attachments/assets/4e664c71-5f38-4eb8-9c67-5116fdcb4dd6" /></td>
    <td><img alt="Screenshot 2" src="https://github.com/user-attachments/assets/addc6e05-f003-4708-a5e8-f299b83dac26" /></td>
  </tr>
  <tr>
    <td><img alt="Screenshot 3" src="https://github.com/user-attachments/assets/ec388498-8890-4041-a68a-d5444caca487" /></td>
    <td><img alt="Screenshot 4" src="https://github.com/user-attachments/assets/4415462e-0d49-4631-8143-429063fcaf02" /></td>
  </tr>
</table>

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

