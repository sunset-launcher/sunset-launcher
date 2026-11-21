use crate::launcher::{configure_background_command, ensure_java_runtime, split_jvm_args};
use crate::models::{
    DedicatedServer, LauncherPaths, LoaderKind, ModLoader, Profile, ServerHandle, ServerSoftware,
    TunnelHandle, UpnpMapping, VersionDetails, VersionManifest, WorkerEvent,
};
use crate::mojang::{
    build_http_client, download_file, fetch_loader_versions, fetch_version_manifest,
};
use crate::storage::server_dir;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn strip_ansi_codes(s: &str) -> String {
    let mut clean = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for esc in chars.by_ref() {
                if esc.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        clean.push(c);
    }
    clean
}

fn emit_server_log(tx: Option<&Sender<WorkerEvent>>, line: impl Into<String>) {
    if let Some(tx) = tx {
        let _ = tx.send(WorkerEvent::ServerLog(strip_ansi_codes(&line.into())));
    }
}

fn emit_progress(tx: Option<&Sender<WorkerEvent>>, stage: impl Into<String>, fraction: f32) {
    if let Some(tx) = tx {
        let _ = tx.send(WorkerEvent::Progress {
            stage: stage.into(),
            fraction: fraction.clamp(0.0, 1.0),
        });
    }
}

/// Infers the required Java major version for a Minecraft version string (`1.21.4` -> 21).
pub fn required_java_major_for_mc(mc_version: &str) -> u32 {
    let mut parts = mc_version
        .split(['.', '-', '+'])
        .filter_map(|p| p.parse::<u32>().ok());
    let major = parts.next().unwrap_or(1);
    let minor = parts.next().unwrap_or(21);
    let patch = parts.next().unwrap_or(0);

    if major > 1 {
        return 21;
    }
    if minor > 20 || (minor == 20 && patch >= 5) {
        21
    } else if minor >= 18 {
        17
    } else if minor == 17 {
        16
    } else {
        8
    }
}

/// Updates or inserts managed keys in `server.properties` while preserving all other entries.
pub fn update_server_properties_text(existing: &str, server: &DedicatedServer) -> String {
    let managed: [(&str, String); 4] = [
        ("server-port", server.port.to_string()),
        ("motd", server.motd.replace(['\r', '\n'], " ")),
        ("max-players", server.max_players.max(1).to_string()),
        (
            "online-mode",
            if server.online_mode { "true" } else { "false" }.to_string(),
        ),
    ];

    let mut seen = [false; 4];
    let mut out_lines: Vec<String> = Vec::new();

    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            out_lines.push(line.to_string());
            continue;
        }
        if let Some((key, _)) = trimmed.split_once('=') {
            let key_trim = key.trim();
            if let Some(idx) = managed.iter().position(|(k, _)| *k == key_trim) {
                out_lines.push(format!("{}={}", managed[idx].0, managed[idx].1));
                seen[idx] = true;
                continue;
            }
        }
        out_lines.push(line.to_string());
    }

    if out_lines.is_empty() {
        out_lines.push("#Minecraft server properties".to_string());
    }
    for (idx, (k, v)) in managed.iter().enumerate() {
        if !seen[idx] {
            out_lines.push(format!("{k}={v}"));
        }
    }

    let mut joined = out_lines.join("\n");
    joined.push('\n');
    joined
}

/// Writes `eula.txt` and updates `server.properties` inside `dir`.
pub fn write_eula_and_properties(dir: &Path, server: &DedicatedServer) -> Result<(), String> {
    fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create server directory {}: {e}", dir.display()))?;

    let eula_path = dir.join("eula.txt");
    let eula_val = if server.eula_accepted { "true" } else { "false" };
    let eula_body = format!(
        "#By changing the setting below to TRUE you are indicating your agreement to Mojang's EULA (https://aka.ms/MinecraftEULA).\neula={eula_val}\n"
    );
    fs::write(&eula_path, eula_body)
        .map_err(|e| format!("Failed to write {}: {e}", eula_path.display()))?;

    let props_path = dir.join("server.properties");
    let existing = fs::read_to_string(&props_path).unwrap_or_default();
    let updated = update_server_properties_text(&existing, server);
    fs::write(&props_path, updated)
        .map_err(|e| format!("Failed to write {}: {e}", props_path.display()))?;

    Ok(())
}

/// Parses `key=value` entries from `server.properties` text, ignoring comments and blank lines.
pub fn parse_server_properties_entries(text: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let key = k.trim().to_string();
            if !key.is_empty() {
                entries.push((key, v.to_string()));
            }
        }
    }
    entries
}

/// Reads all `server.properties` entries from `dir`, filling in any missing standard Minecraft defaults.
pub fn read_server_properties_entries(dir: &Path) -> Vec<(String, String)> {
    let props_path = dir.join("server.properties");
    let mut entries = if let Ok(raw) = fs::read_to_string(&props_path) {
        parse_server_properties_entries(&raw)
    } else {
        Vec::new()
    };

    const DEFAULTS: [(&str, &str); 22] = [
        ("server-port", "25565"),
        ("motd", "A Minecraft Server"),
        ("max-players", "20"),
        ("online-mode", "true"),
        ("gamemode", "survival"),
        ("difficulty", "easy"),
        ("pvp", "true"),
        ("hardcore", "false"),
        ("white-list", "false"),
        ("enforce-whitelist", "false"),
        ("allow-flight", "false"),
        ("allow-nether", "true"),
        ("enable-command-block", "false"),
        ("spawn-protection", "16"),
        ("view-distance", "10"),
        ("simulation-distance", "10"),
        ("level-name", "world"),
        ("level-seed", ""),
        ("level-type", "minecraft:normal"),
        ("spawn-monsters", "true"),
        ("spawn-animals", "true"),
        ("spawn-npcs", "true"),
    ];
    for (k, v) in DEFAULTS {
        if !entries.iter().any(|(ek, _)| ek == k) {
            entries.push((k.to_string(), v.to_string()));
        }
    }
    entries
}

/// Persists edited `server.properties` entries back to `dir/server.properties`, preserving comments.
pub fn write_server_properties_entries(
    dir: &Path,
    entries: &[(String, String)],
) -> Result<(), String> {
    fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create {}: {e}", dir.display()))?;
    let props_path = dir.join("server.properties");
    let existing = fs::read_to_string(&props_path).unwrap_or_default();

    let mut seen = vec![false; entries.len()];
    let mut out_lines = Vec::new();
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            out_lines.push(line.to_string());
            continue;
        }
        if let Some((k, _)) = trimmed.split_once('=') {
            let key_trim = k.trim();
            if let Some(pos) = entries.iter().position(|(ek, _)| ek == key_trim) {
                out_lines.push(format!(
                    "{}={}",
                    entries[pos].0,
                    entries[pos].1.replace(['\r', '\n'], " ")
                ));
                seen[pos] = true;
                continue;
            }
        }
        out_lines.push(line.to_string());
    }
    if out_lines.is_empty() {
        out_lines.push("#Minecraft server properties".to_string());
    }
    for (idx, (k, v)) in entries.iter().enumerate() {
        if !seen[idx] {
            out_lines.push(format!("{k}={}", v.replace(['\r', '\n'], " ")));
        }
    }
    let mut joined = out_lines.join("\n");
    joined.push('\n');
    fs::write(&props_path, joined)
        .map_err(|e| format!("Failed to write {}: {e}", props_path.display()))
}

/// Resizes and center-crops `src_image` into a standard `64x64` `server-icon.png` inside `dir`.
pub fn save_server_icon_png(dir: &Path, src_image: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create {}: {e}", dir.display()))?;
    let img = image::open(src_image)
        .map_err(|e| format!("Failed to decode image {}: {e}", src_image.display()))?;
    let resized = img.resize_to_fill(64, 64, image::imageops::FilterType::Lanczos3);
    let dst = dir.join("server-icon.png");
    resized
        .save_with_format(&dst, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to save {}: {e}", dst.display()))?;
    Ok(dst)
}

/// Removes a `key=value` entry from `dir/server.properties`, preserving comments and blank lines.
pub fn delete_server_property_entry(dir: &Path, key: &str) -> Result<(), String> {
    let props_path = dir.join("server.properties");
    let Ok(existing) = fs::read_to_string(&props_path) else {
        return Ok(());
    };
    let mut out_lines: Vec<String> = Vec::new();
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            out_lines.push(line.to_string());
            continue;
        }
        if let Some((k, _)) = trimmed.split_once('=') {
            if k.trim() == key {
                continue;
            }
        }
        out_lines.push(line.to_string());
    }
    let mut joined = out_lines.join("\n");
    joined.push('\n');
    fs::write(&props_path, joined)
        .map_err(|e| format!("Failed to write {}: {e}", props_path.display()))
}

/// Updates cached `server.properties` entries in memory from the `DedicatedServer` model fields.
pub fn sync_server_properties_from_model(
    entries: &mut Vec<(String, String)>,
    server: &DedicatedServer,
) {
    let managed: [(&str, String); 4] = [
        ("server-port", server.port.to_string()),
        ("motd", server.motd.replace(['\r', '\n'], " ")),
        ("max-players", server.max_players.max(1).to_string()),
        (
            "online-mode",
            if server.online_mode { "true" } else { "false" }.to_string(),
        ),
    ];
    for (k, v) in managed {
        if let Some(item) = entries.iter_mut().find(|(ek, _)| ek == k) {
            item.1 = v;
        } else {
            entries.push((k.to_string(), v));
        }
    }
}

/// Parses Minecraft `§` color/formatting codes inside an MOTD into styled segments.
pub fn parse_mc_motd_segments(motd: &str) -> Vec<(String, Option<[u8; 3]>, bool)> {
    if motd.is_empty() {
        return Vec::new();
    }
    fn mc_color_rgb(code: char) -> Option<[u8; 3]> {
        match code.to_ascii_lowercase() {
            '0' => Some([0, 0, 0]),
            '1' => Some([0, 0, 170]),
            '2' => Some([0, 170, 0]),
            '3' => Some([0, 170, 170]),
            '4' => Some([170, 0, 0]),
            '5' => Some([170, 0, 170]),
            '6' => Some([255, 170, 0]),
            '7' => Some([170, 170, 170]),
            '8' => Some([85, 85, 85]),
            '9' => Some([85, 85, 255]),
            'a' => Some([85, 255, 85]),
            'b' => Some([85, 255, 255]),
            'c' => Some([255, 85, 85]),
            'd' => Some([255, 85, 255]),
            'e' => Some([255, 255, 85]),
            'f' => Some([255, 255, 255]),
            _ => None,
        }
    }

    let mut out: Vec<(String, Option<[u8; 3]>, bool)> = Vec::new();
    let mut cur_color: Option<[u8; 3]> = None;
    let mut cur_bold = false;
    let mut buf = String::new();
    let mut chars = motd.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '§' {
            if let Some(&code) = chars.peek() {
                chars.next();
                if !buf.is_empty() {
                    out.push((std::mem::take(&mut buf), cur_color, cur_bold));
                }
                let lower = code.to_ascii_lowercase();
                if let Some(rgb) = mc_color_rgb(lower) {
                    cur_color = Some(rgb);
                } else if lower == 'l' {
                    cur_bold = true;
                } else if lower == 'r' {
                    cur_color = None;
                    cur_bold = false;
                }
                continue;
            }
        }
        buf.push(c);
    }
    if !buf.is_empty() {
        out.push((buf, cur_color, cur_bold));
    }
    out
}

#[derive(serde::Deserialize)]
struct FillBuild {
    id: u64,
    #[serde(default)]
    channel: String,
    #[serde(default)]
    downloads: std::collections::HashMap<String, FillDownload>,
}

#[derive(serde::Deserialize)]
struct FillDownload {
    url: String,
}

#[derive(serde::Deserialize)]
struct PurpurVersionResponse {
    builds: PurpurBuilds,
}

#[derive(serde::Deserialize)]
struct PurpurBuilds {
    latest: String,
}

#[derive(serde::Deserialize)]
struct FabricLoaderMeta {
    loader: FabricVersionItem,
}

#[derive(serde::Deserialize)]
struct FabricInstallerMeta {
    version: String,
    stable: bool,
}

#[derive(serde::Deserialize)]
struct FabricVersionItem {
    version: String,
    stable: bool,
}

#[derive(serde::Deserialize)]
struct QuiltLoaderMeta {
    loader: FabricVersionItem,
}

#[derive(serde::Deserialize)]
struct QuiltInstallerMeta {
    version: String,
    url: String,
}

#[derive(serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(serde::Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// Target arguments for launching a provisioned dedicated server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerLaunchTarget {
    Jar(PathBuf),
    ArgsFile(PathBuf),
}

fn find_forge_args_file(dir: &Path) -> Option<PathBuf> {
    let args_name = if cfg!(windows) {
        "win_args.txt"
    } else {
        "unix_args.txt"
    };
    for rel in [
        "libraries/net/minecraftforge/forge",
        "libraries/net/neoforged/neoforge",
    ] {
        let base = dir.join(rel);
        if let Ok(entries) = fs::read_dir(&base) {
            let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            dirs.sort();
            for sub in dirs.into_iter().rev() {
                let candidate = sub.join(args_name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn find_legacy_forge_jar(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".jar")
            && (lower.starts_with("forge-") || lower.starts_with("neoforge-"))
            && !lower.contains("installer")
        {
            return Some(path);
        }
    }
    None
}

/// Resolves how a provisioned server should be launched (`-jar ...` or `@.../unix_args.txt`).
pub fn resolve_server_launch_target(dir: &Path, software: ServerSoftware) -> Option<ServerLaunchTarget> {
    match software {
        ServerSoftware::Forge | ServerSoftware::NeoForge => {
            if let Some(args) = find_forge_args_file(dir) {
                return Some(ServerLaunchTarget::ArgsFile(args));
            }
            if let Some(legacy) = find_legacy_forge_jar(dir) {
                return Some(ServerLaunchTarget::Jar(legacy));
            }
            let jar = dir.join("server.jar");
            jar.is_file().then_some(ServerLaunchTarget::Jar(jar))
        }
        ServerSoftware::Quilt => {
            let quilt_jar = dir.join("quilt-server-launch.jar");
            if quilt_jar.is_file() {
                return Some(ServerLaunchTarget::Jar(quilt_jar));
            }
            let jar = dir.join("server.jar");
            jar.is_file().then_some(ServerLaunchTarget::Jar(jar))
        }
        _ => {
            let jar = dir.join("server.jar");
            jar.is_file().then_some(ServerLaunchTarget::Jar(jar))
        }
    }
}

/// Returns `true` if `dir/mods/` contains an enabled plugin-bridge mod (e.g. Cardboard, Banner, Arclight).
pub fn server_has_plugin_mod(dir: &Path) -> bool {
    let mods_dir = dir.join("mods");
    let Ok(entries) = fs::read_dir(mods_dir) else {
        return false;
    };
    const BRIDGE_MODS: [&str; 8] = [
        "cardboard",
        "banner",
        "arclight",
        "ketting",
        "mohist",
        "youer",
        "magma",
        "spigot",
    ];
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(|s| s.to_ascii_lowercase()) else {
            continue;
        };
        if name.ends_with(".jar") && BRIDGE_MODS.iter().any(|k| name.contains(k)) {
            return true;
        }
    }
    false
}

/// Returns `true` if the server natively supports plugins or has a plugin-bridge mod installed.
pub fn server_supports_plugins_live(dir: &Path, software: ServerSoftware) -> bool {
    software.supports_plugins() || (software.supports_mods() && server_has_plugin_mod(dir))
}

/// Returns `true` if the server directory already contains a runnable server artifact.
pub fn is_server_provisioned(dir: &Path, software: ServerSoftware) -> bool {
    resolve_server_launch_target(dir, software).is_some()
}

fn ensure_server_java(
    paths: &LauncherPaths,
    server: &DedicatedServer,
    tx: Option<&Sender<WorkerEvent>>,
) -> Result<PathBuf, String> {
    let req_java = required_java_major_for_mc(&server.mc_version);
    let mut temp_profile = Profile::new(
        server.name.clone(),
        server.mc_version.clone(),
        ModLoader::Vanilla,
        "Server".to_string(),
    );
    temp_profile.java_path = server.java_path.clone();
    ensure_java_runtime(paths, &temp_profile, Some(req_java), tx)
}

fn provision_papermc_project(
    client: &reqwest::blocking::Client,
    project: &str,
    display_name: &str,
    mc_version: &str,
    jar_path: &Path,
    tx: Option<&Sender<WorkerEvent>>,
) -> Result<String, String> {
    let url = format!("https://fill.papermc.io/v3/projects/{project}/versions/{mc_version}/builds");
    let builds: Vec<FillBuild> = client
        .get(&url)
        .header(
            reqwest::header::USER_AGENT,
            concat!(
                "SunsetLauncher/",
                env!("CARGO_PKG_VERSION"),
                " (https://github.com/sunset-launcher/sunset-launcher)"
            ),
        )
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("{display_name} does not have a build for Minecraft {mc_version} ({e})"))?
        .json()
        .map_err(|e| format!("Failed to parse {display_name} builds: {e}"))?;
    let latest = builds
        .iter()
        .find(|b| b.channel.eq_ignore_ascii_case("STABLE"))
        .or_else(|| builds.first())
        .ok_or_else(|| format!("No {display_name} builds available for {mc_version}"))?;
    let dl_url = latest
        .downloads
        .get("server:default")
        .or_else(|| latest.downloads.values().next())
        .map(|d| d.url.clone())
        .ok_or_else(|| {
            format!(
                "{display_name} build #{} for {mc_version} has no download URL",
                latest.id
            )
        })?;
    emit_progress(
        tx,
        format!("Downloading {display_name} build #{}...", latest.id),
        0.5,
    );
    download_file(client, &dl_url, jar_path, None)?;
    Ok(latest.id.to_string())
}

fn run_installer_command(
    java_bin: &Path,
    dir: &Path,
    args: &[&str],
    tx: Option<&Sender<WorkerEvent>>,
) -> Result<(), String> {
    let mut cmd = Command::new(java_bin);
    cmd.current_dir(dir)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_background_command(&mut cmd);
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run server installer: {e}"))?;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if !line.trim().is_empty() {
            emit_server_log(tx, line);
        }
    }
    for line in String::from_utf8_lossy(&output.stderr).lines() {
        if !line.trim().is_empty() {
            emit_server_log(tx, format!("[Installer] {line}"));
        }
    }
    if !output.status.success() {
        return Err(format!(
            "Server installer exited with status {}",
            output.status
        ));
    }
    Ok(())
}

/// Selects the best matching Arclight asset for `mc_version` from recent GitHub releases.
fn select_arclight_asset<'a>(
    releases: &'a [GitHubRelease],
    mc_version: &str,
) -> Option<(&'a str, &'a GitHubAsset)> {
    let needle = format!("-{mc_version}-");
    let preferred_prefixes = ["arclight-neoforge-", "arclight-forge-", "arclight-fabric-"];
    for rel in releases {
        for prefix in preferred_prefixes {
            if let Some(asset) = rel.assets.iter().find(|a| {
                a.name.starts_with(prefix)
                    && a.name.contains(&needle)
                    && a.name.ends_with(".jar")
            }) {
                return Some((rel.tag_name.as_str(), asset));
            }
        }
        if let Some(asset) = rel
            .assets
            .iter()
            .find(|a| a.name.contains(&needle) && a.name.ends_with(".jar"))
        {
            return Some((rel.tag_name.as_str(), asset));
        }
    }
    None
}

/// Downloads `server.jar` for the server's software and Minecraft version, plus writes `eula.txt` and `server.properties`.
pub fn provision_server_jar(
    paths: &LauncherPaths,
    server: &DedicatedServer,
    manifest: Option<&VersionManifest>,
    tx: Option<&Sender<WorkerEvent>>,
) -> Result<DedicatedServer, String> {
    let mut updated = server.clone();
    let dir = server_dir(paths, &updated);
    let jar_path = dir.join("server.jar");
    let client = build_http_client()?;

    emit_progress(
        tx,
        format!(
            "Downloading {} {} server...",
            updated.software.label(),
            updated.mc_version
        ),
        0.15,
    );
    emit_server_log(
        tx,
        format!(
            "[Server] Provisioning {} {} in {}",
            updated.software.label(),
            updated.mc_version,
            dir.display()
        ),
    );

    match updated.software {
        ServerSoftware::Vanilla => {
            let owned_manifest;
            let mf = match manifest {
                Some(m) => m,
                None => {
                    owned_manifest = fetch_version_manifest()?;
                    &owned_manifest
                }
            };
            let entry = mf
                .versions
                .iter()
                .find(|v| v.id == updated.mc_version)
                .ok_or_else(|| {
                    format!(
                        "Minecraft version {} not found in Mojang manifest",
                        updated.mc_version
                    )
                })?;
            let details: VersionDetails = client
                .get(&entry.url)
                .send()
                .and_then(|r| r.error_for_status())
                .map_err(|e| format!("Failed to fetch version metadata: {e}"))?
                .json()
                .map_err(|e| format!("Failed to parse version metadata: {e}"))?;
            let srv_dl = details
                .downloads
                .as_ref()
                .and_then(|d| d.server.as_ref())
                .ok_or_else(|| {
                    format!(
                        "Mojang does not provide a dedicated server JAR for {}",
                        updated.mc_version
                    )
                })?;
            emit_progress(tx, "Downloading vanilla server.jar...", 0.45);
            download_file(&client, &srv_dl.url, &jar_path, srv_dl.sha1.as_deref())?;
            updated.loader_version = "vanilla".to_string();
        }
        ServerSoftware::Paper => {
            updated.loader_version = provision_papermc_project(
                &client,
                "paper",
                "Paper",
                &updated.mc_version,
                &jar_path,
                tx,
            )?;
        }
        ServerSoftware::Folia => {
            updated.loader_version = provision_papermc_project(
                &client,
                "folia",
                "Folia",
                &updated.mc_version,
                &jar_path,
                tx,
            )?;
        }
        ServerSoftware::Purpur => {
            let meta_url = format!("https://api.purpurmc.org/v2/purpur/{}", updated.mc_version);
            let resp: PurpurVersionResponse = client
                .get(&meta_url)
                .send()
                .and_then(|r| r.error_for_status())
                .map_err(|e| {
                    format!(
                        "Purpur does not have a build for Minecraft {} ({e})",
                        updated.mc_version
                    )
                })?
                .json()
                .map_err(|e| format!("Failed to parse Purpur metadata: {e}"))?;
            let build = resp.builds.latest;
            let dl_url = format!(
                "https://api.purpurmc.org/v2/purpur/{}/{}/download",
                updated.mc_version, build
            );
            emit_progress(tx, format!("Downloading Purpur build #{build}..."), 0.5);
            download_file(&client, &dl_url, &jar_path, None)?;
            updated.loader_version = build;
        }
        ServerSoftware::Fabric => {
            let loaders_url = format!(
                "https://meta.fabricmc.net/v2/versions/loader/{}",
                updated.mc_version
            );
            let loaders: Vec<FabricLoaderMeta> = client
                .get(&loaders_url)
                .send()
                .and_then(|r| r.error_for_status())
                .map_err(|e| {
                    format!(
                        "Fabric does not support Minecraft {} ({e})",
                        updated.mc_version
                    )
                })?
                .json()
                .map_err(|e| format!("Failed to parse Fabric loader versions: {e}"))?;
            let loader_ver = loaders
                .iter()
                .find(|l| l.loader.stable)
                .or_else(|| loaders.first())
                .map(|l| l.loader.version.clone())
                .ok_or_else(|| {
                    format!("No Fabric loader builds found for {}", updated.mc_version)
                })?;

            let installers: Vec<FabricInstallerMeta> = client
                .get("https://meta.fabricmc.net/v2/versions/installer")
                .send()
                .and_then(|r| r.error_for_status())
                .map_err(|e| format!("Failed to fetch Fabric installer versions: {e}"))?
                .json()
                .map_err(|e| format!("Failed to parse Fabric installer versions: {e}"))?;
            let installer_ver = installers
                .iter()
                .find(|i| i.stable)
                .or_else(|| installers.first())
                .map(|i| i.version.clone())
                .ok_or_else(|| "No Fabric installer versions found".to_string())?;

            let dl_url = format!(
                "https://meta.fabricmc.net/v2/versions/loader/{}/{}/{}/server/jar",
                updated.mc_version, loader_ver, installer_ver
            );
            emit_progress(tx, format!("Downloading Fabric {loader_ver} server..."), 0.5);
            download_file(&client, &dl_url, &jar_path, None)?;
            updated.loader_version = loader_ver;
        }
        ServerSoftware::Quilt => {
            let loaders_url = format!(
                "https://meta.quiltmc.org/v3/versions/loader/{}",
                updated.mc_version
            );
            let loaders: Vec<QuiltLoaderMeta> = client
                .get(&loaders_url)
                .send()
                .and_then(|r| r.error_for_status())
                .map_err(|e| {
                    format!(
                        "Quilt does not support Minecraft {} ({e})",
                        updated.mc_version
                    )
                })?
                .json()
                .map_err(|e| format!("Failed to parse Quilt loader versions: {e}"))?;
            let loader_ver = loaders
                .iter()
                .find(|l| l.loader.stable)
                .or_else(|| loaders.first())
                .map(|l| l.loader.version.clone())
                .ok_or_else(|| {
                    format!("No Quilt loader builds found for {}", updated.mc_version)
                })?;

            let installers: Vec<QuiltInstallerMeta> = client
                .get("https://meta.quiltmc.org/v3/versions/installer")
                .send()
                .and_then(|r| r.error_for_status())
                .map_err(|e| format!("Failed to fetch Quilt installer versions: {e}"))?
                .json()
                .map_err(|e| format!("Failed to parse Quilt installer versions: {e}"))?;
            let installer = installers
                .first()
                .ok_or_else(|| "No Quilt installer versions found".to_string())?;

            let installer_jar = dir.join("quilt-installer.jar");
            emit_progress(
                tx,
                format!("Downloading Quilt installer {}...", installer.version),
                0.4,
            );
            download_file(&client, &installer.url, &installer_jar, None)?;

            let java_bin = ensure_server_java(paths, &updated, tx)?;
            emit_progress(tx, "Installing Quilt server...", 0.7);
            run_installer_command(
                &java_bin,
                &dir,
                &[
                    "-jar",
                    "quilt-installer.jar",
                    "install",
                    "server",
                    &updated.mc_version,
                    &loader_ver,
                    "--download-server",
                    "--install-dir=.",
                ],
                tx,
            )?;
            let _ = fs::remove_file(&installer_jar);
            updated.loader_version = loader_ver;
        }
        ServerSoftware::Forge | ServerSoftware::NeoForge => {
            let kind = if updated.software == ServerSoftware::Forge {
                LoaderKind::Forge
            } else {
                LoaderKind::NeoForge
            };
            let versions = fetch_loader_versions(kind, &updated.mc_version)?;
            let chosen = versions
                .iter()
                .find(|v| v.stable)
                .or_else(|| versions.first())
                .ok_or_else(|| {
                    format!(
                        "No {} builds found for Minecraft {}",
                        updated.software.label(),
                        updated.mc_version
                    )
                })?
                .version
                .clone();

            let (installer_url, installer_name) = if kind == LoaderKind::Forge {
                (
                    format!(
                        "https://maven.minecraftforge.net/net/minecraftforge/forge/{chosen}/forge-{chosen}-installer.jar"
                    ),
                    format!("forge-{chosen}-installer.jar"),
                )
            } else {
                (
                    format!(
                        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{chosen}/neoforge-{chosen}-installer.jar"
                    ),
                    format!("neoforge-{chosen}-installer.jar"),
                )
            };

            let installer_path = dir.join(&installer_name);
            emit_progress(
                tx,
                format!("Downloading {} {chosen} installer...", updated.software.label()),
                0.4,
            );
            download_file(&client, &installer_url, &installer_path, None)?;

            let java_bin = ensure_server_java(paths, &updated, tx)?;
            emit_progress(
                tx,
                format!("Running {} --installServer...", updated.software.label()),
                0.7,
            );
            run_installer_command(
                &java_bin,
                &dir,
                &["-jar", &installer_name, "--installServer"],
                tx,
            )?;
            let _ = fs::remove_file(&installer_path);
            updated.loader_version = chosen;
        }
        ServerSoftware::Arclight => {
            emit_progress(tx, "Fetching Arclight releases...", 0.3);
            let releases: Vec<GitHubRelease> = client
                .get("https://api.github.com/repos/IzzelAliz/Arclight/releases?per_page=30")
                .send()
                .and_then(|r| r.error_for_status())
                .map_err(|e| format!("Failed to fetch Arclight releases: {e}"))?
                .json()
                .map_err(|e| format!("Failed to parse Arclight releases: {e}"))?;
            let (tag, asset) = select_arclight_asset(&releases, &updated.mc_version)
                .ok_or_else(|| {
                    format!(
                        "No Arclight server release found for Minecraft {}",
                        updated.mc_version
                    )
                })?;
            emit_progress(tx, format!("Downloading {}...", asset.name), 0.55);
            download_file(&client, &asset.browser_download_url, &jar_path, None)?;
            updated.loader_version = tag.to_string();
        }
    }

    write_eula_and_properties(&dir, &updated)?;
    emit_progress(tx, "Server ready", 1.0);
    emit_server_log(
        tx,
        format!(
            "[Server] {} ({} {}) ready at {}",
            updated.name,
            updated.software.label(),
            updated.mc_version,
            dir.display()
        ),
    );
    Ok(updated)
}

/// Starts a dedicated Minecraft server, provisioning `server.jar` and Java if needed.
pub fn start_dedicated_server(
    paths: &LauncherPaths,
    server: &DedicatedServer,
    manifest: Option<&VersionManifest>,
    tx: Option<&Sender<WorkerEvent>>,
) -> Result<ServerHandle, String> {
    if !server.eula_accepted {
        return Err("You must accept the Minecraft EULA before starting the server.".to_string());
    }

    let dir = server_dir(paths, server);
    let effective = if !is_server_provisioned(&dir, server.software) {
        provision_server_jar(paths, server, manifest, tx)?
    } else {
        write_eula_and_properties(&dir, server)?;
        server.clone()
    };

    let launch_target = resolve_server_launch_target(&dir, effective.software).ok_or_else(|| {
        format!(
            "No runnable server artifact found in {} after provisioning",
            dir.display()
        )
    })?;

    emit_progress(
        tx,
        format!("Checking Java {}...", required_java_major_for_mc(&effective.mc_version)),
        0.75,
    );
    let java_bin = ensure_server_java(paths, &effective, tx)?;

    let min_ram = effective.min_ram_mb.max(256);
    let max_ram = effective.max_ram_mb.max(min_ram);

    let mut cmd = Command::new(&java_bin);
    cmd.current_dir(&dir)
        .arg(format!("-Xms{min_ram}M"))
        .arg(format!("-Xmx{max_ram}M"));
    let parsed_jvm = split_jvm_args(&effective.jvm_args);
    if !parsed_jvm
        .iter()
        .any(|a| a.starts_with("-Dlog4j2.formatMsgNoLookups="))
    {
        cmd.arg("-Dlog4j2.formatMsgNoLookups=true");
    }
    for extra in parsed_jvm {
        cmd.arg(extra);
    }
    match launch_target {
        ServerLaunchTarget::Jar(jar_path) => {
            cmd.arg("-jar").arg(jar_path);
        }
        ServerLaunchTarget::ArgsFile(args_path) => {
            cmd.arg(format!("@{}", args_path.display()));
        }
    }
    cmd.arg("nogui")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_background_command(&mut cmd);

    emit_server_log(
        tx,
        format!(
            "[Server] Starting '{}' ({} {}) on port {}...",
            effective.name,
            effective.software.label(),
            effective.mc_version,
            effective.port
        ),
    );
    emit_progress(tx, "Starting server...", 0.95);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start dedicated server: {e}"))?;

    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let mut handle = ServerHandle {
        server_id: effective.id.clone(),
        child: Arc::new(Mutex::new(child)),
        stdin: Arc::new(Mutex::new(stdin)),
        upnp_mapping: None,
        tunnel: None,
    };

    if effective.use_tunnel {
        match start_public_tunnel(effective.port, tx) {
            Ok(t_handle) => {
                handle.tunnel = Some(t_handle);
            }
            Err(err) => {
                emit_server_log(tx, format!("[Network] Tunnel failed: {err}"));
                if let Some(tx) = tx {
                    let _ = tx.send(WorkerEvent::ServerTunnelFailed(err));
                }
            }
        }
    } else if effective.public_upnp {
        match open_upnp_port(effective.port, tx) {
            Ok(mapping) => {
                if let Some(ip) = &mapping.public_ip {
                    emit_server_log(
                        tx,
                        format!(
                            "[Network] Port {} opened via UPnP -> {ip}:{}",
                            effective.port, effective.port
                        ),
                    );
                } else {
                    emit_server_log(
                        tx,
                        format!("[Network] Port {} opened via UPnP.", effective.port),
                    );
                }
                handle.upnp_mapping = Some(mapping);
            }
            Err(err) => {
                if let Some(lan) = detect_local_lan_ip() {
                    emit_server_log(
                        tx,
                        format!(
                            "[Network] UPnP unavailable ({err}). LAN: {lan}:{} (enable Public Tunnel for gray/CGNAT IP)",
                            effective.port
                        ),
                    );
                } else {
                    emit_server_log(
                        tx,
                        format!("[Network] UPnP unavailable ({err}). Enable Public Tunnel for gray/CGNAT IP."),
                    );
                }
            }
        }
    }

    if let (Some(out), Some(tx_out)) = (stdout, tx.cloned()) {
        thread::spawn(move || {
            let reader = BufReader::new(out);
            for line in reader.lines().map_while(Result::ok) {
                let _ = tx_out.send(WorkerEvent::ServerLog(strip_ansi_codes(&line)));
            }
        });
    }
    if let (Some(err), Some(tx_err)) = (stderr, tx.cloned()) {
        thread::spawn(move || {
            let reader = BufReader::new(err);
            for line in reader.lines().map_while(Result::ok) {
                let _ = tx_err.send(WorkerEvent::ServerLog(format!(
                    "[ERR] {}",
                    strip_ansi_codes(&line)
                )));
            }
        });
    }

    if let Some(tx_wait) = tx.cloned() {
        let child_arc = Arc::clone(&handle.child);
        let mapping_cleanup = handle.upnp_mapping.clone();
        let tunnel_cleanup = handle.tunnel.clone();
        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(350));
            let status = {
                if let Ok(mut guard) = child_arc.lock() {
                    guard.try_wait()
                } else {
                    break;
                }
            };
            match status {
                Ok(Some(code)) => {
                    if let Some(tc) = tunnel_cleanup {
                        tc.stop();
                    }
                    if let Some(m) = mapping_cleanup {
                        close_upnp_port(&m);
                    }
                    let _ = tx_wait.send(WorkerEvent::ServerExited(code.code()));
                    break;
                }
                Ok(None) => {}
                Err(_) => {
                    if let Some(tc) = tunnel_cleanup {
                        tc.stop();
                    }
                    if let Some(m) = mapping_cleanup {
                        close_upnp_port(&m);
                    }
                    let _ = tx_wait.send(WorkerEvent::ServerExited(None));
                    break;
                }
            }
        });
    }

    emit_progress(tx, "Server running", 1.0);
    Ok(handle)
}

/// Sends a console command line to the running dedicated server's stdin.
pub fn send_server_command(handle: &ServerHandle, command: &str) -> Result<(), String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let mut guard = handle
        .stdin
        .lock()
        .map_err(|_| "Server stdin lock poisoned".to_string())?;
    let stdin = guard
        .as_mut()
        .ok_or_else(|| "Server stdin is closed".to_string())?;
    writeln!(stdin, "{trimmed}")
        .and_then(|_| stdin.flush())
        .map_err(|e| format!("Failed to send command to server: {e}"))
}

/// Gracefully requests server shutdown by sending `stop` to stdin.
pub fn stop_dedicated_server(handle: &ServerHandle) -> Result<(), String> {
    send_server_command(handle, "stop")
}

/// Forcefully terminates the dedicated server process (and any active tunnel/UPnP).
pub fn kill_dedicated_server(handle: &ServerHandle) {
    if let Some(tc) = &handle.tunnel {
        tc.stop();
    }
    if let Some(m) = &handle.upnp_mapping {
        close_upnp_port(m);
    }
    if let Ok(mut guard) = handle.child.lock() {
        let _ = guard.kill();
    }
}

/// Parses `listening at bore.pub:12345` from `bore` output (stripping any ANSI codes).
pub fn parse_bore_listening_address(line: &str) -> Option<String> {
    let clean = strip_ansi_codes(line);
    let idx = clean.find("listening at ")?;
    let rest = clean[idx + "listening at ".len()..].trim();
    let addr = rest.split_whitespace().next()?;
    if addr.contains(':') {
        Some(addr.to_string())
    } else {
        None
    }
}

const BORE_CONTROL_HOST: &str = "bore.pub";
const BORE_CONTROL_PORT: u16 = 7835;

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
enum BoreClientMsg {
    #[allow(dead_code)]
    Authenticate(String),
    Hello(u16),
    Accept(String),
}

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
enum BoreServerMsg {
    Challenge(String),
    Hello(u16),
    Heartbeat,
    Connection(String),
    Error(String),
}

fn send_bore_msg(stream: &mut TcpStream, msg: &BoreClientMsg) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(msg).map_err(|e| format!("bore encode error: {e}"))?;
    bytes.push(0);
    stream
        .write_all(&bytes)
        .and_then(|_| stream.flush())
        .map_err(|e| format!("bore write error: {e}"))
}

enum BoreReadState {
    Msg(BoreServerMsg),
    Timeout,
    Eof,
}

fn read_bore_msg(stream: &mut TcpStream, buf: &mut Vec<u8>) -> Result<BoreReadState, String> {
    loop {
        if let Some(pos) = buf.iter().position(|&b| b == 0) {
            let frame: Vec<u8> = buf.drain(..=pos).collect();
            let json_slice = &frame[..frame.len() - 1];
            let msg: BoreServerMsg = serde_json::from_slice(json_slice)
                .map_err(|e| format!("bore decode error: {e}"))?;
            return Ok(BoreReadState::Msg(msg));
        }
        if buf.len() > 64 * 1024 {
            return Err("bore control frame too large".to_string());
        }
        let mut chunk = [0u8; 1024];
        match stream.read(&mut chunk) {
            Ok(0) => return Ok(BoreReadState::Eof),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                return Ok(BoreReadState::Timeout);
            }
            Err(e) => return Err(format!("bore read error: {e}")),
        }
    }
}

fn connect_bore_control() -> Result<(TcpStream, SocketAddr), String> {
    let addrs_iter = (BORE_CONTROL_HOST, BORE_CONTROL_PORT)
        .to_socket_addrs()
        .map_err(|e| format!("Failed to resolve {BORE_CONTROL_HOST}: {e}"))?;
    let mut addrs: Vec<_> = addrs_iter.collect();
    addrs.sort_by_key(|a| a.is_ipv6());
    let mut last_err = "no addresses resolved".to_string();
    for addr in addrs.into_iter().take(4) {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(4)) {
            Ok(s) => {
                let _ = s.set_nodelay(true);
                return Ok((s, addr));
            }
            Err(e) => last_err = e.to_string(),
        }
    }
    Err(format!("Failed to connect {BORE_CONTROL_HOST}:{BORE_CONTROL_PORT}: {last_err}"))
}

fn forward_bore_connection(server_addr: SocketAddr, conn_id: String, local_port: u16) {
    let Ok(mut remote) = TcpStream::connect_timeout(&server_addr, Duration::from_secs(5)) else {
        return;
    };
    let _ = remote.set_nodelay(true);
    if send_bore_msg(&mut remote, &BoreClientMsg::Accept(conn_id)).is_err() {
        return;
    }
    let local_addr = SocketAddr::from(([127, 0, 0, 1], local_port));
    let Ok(mut local) = TcpStream::connect_timeout(&local_addr, Duration::from_secs(5)) else {
        return;
    };
    let _ = local.set_nodelay(true);

    let (Ok(mut r_read), Ok(mut l_write)) = (remote.try_clone(), local.try_clone()) else {
        return;
    };
    let t = thread::spawn(move || {
        let _ = std::io::copy(&mut r_read, &mut l_write);
        let _ = l_write.shutdown(Shutdown::Write);
    });
    let _ = std::io::copy(&mut local, &mut remote);
    let _ = remote.shutdown(Shutdown::Write);
    let _ = t.join();
}

/// Starts the built-in Rust `bore` client (zero external binaries required).
fn start_bore_tunnel(
    port: u16,
    tx: Option<&Sender<WorkerEvent>>,
) -> Result<Arc<TunnelHandle>, String> {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_worker = Arc::clone(&stop_flag);
    let tx_owned = tx.cloned();

    thread::spawn(move || {
        let (mut stream, server_addr) = match connect_bore_control() {
            Ok(res) => res,
            Err(err) => {
                if let Some(tx) = &tx_owned {
                    let _ = tx.send(WorkerEvent::ServerTunnelFailed(err));
                }
                return;
            }
        };
        let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));

        if let Err(err) = send_bore_msg(&mut stream, &BoreClientMsg::Hello(0)) {
            if let Some(tx) = &tx_owned {
                let _ = tx.send(WorkerEvent::ServerTunnelFailed(err));
            }
            return;
        }

        let mut buf = Vec::with_capacity(2048);
        let mut assigned_port: Option<u16> = None;

        while !stop_worker.load(Ordering::Relaxed) {
            match read_bore_msg(&mut stream, &mut buf) {
                Ok(BoreReadState::Timeout) => continue,
                Ok(BoreReadState::Eof) => {
                    if assigned_port.is_none() {
                        if let Some(tx) = &tx_owned {
                            let _ = tx.send(WorkerEvent::ServerTunnelFailed(
                                "bore server closed connection".to_string(),
                            ));
                        }
                    }
                    break;
                }
                Ok(BoreReadState::Msg(msg)) => match msg {
                    BoreServerMsg::Hello(remote_port) => {
                        assigned_port = Some(remote_port);
                        let addr = format!("{BORE_CONTROL_HOST}:{remote_port}");
                        if let Some(tx) = &tx_owned {
                            let _ = tx.send(WorkerEvent::ServerTunnelAddress(addr.clone()));
                            let _ = tx.send(WorkerEvent::ServerLog(format!(
                                "[Network] Public tunnel active -> {addr} (built-in bore, works behind gray/CGNAT IP)"
                            )));
                        }
                    }
                    BoreServerMsg::Connection(conn_id) => {
                        thread::spawn(move || {
                            forward_bore_connection(server_addr, conn_id, port);
                        });
                    }
                    BoreServerMsg::Heartbeat => {}
                    BoreServerMsg::Challenge(_) => {
                        if assigned_port.is_none() {
                            if let Some(tx) = &tx_owned {
                                let _ = tx.send(WorkerEvent::ServerTunnelFailed(
                                    "bore server requested authentication challenge".to_string(),
                                ));
                            }
                        }
                        break;
                    }
                    BoreServerMsg::Error(e) => {
                        if assigned_port.is_none() {
                            if let Some(tx) = &tx_owned {
                                let _ = tx.send(WorkerEvent::ServerTunnelFailed(format!(
                                    "bore error: {e}"
                                )));
                            }
                        } else if let Some(tx) = &tx_owned {
                            let _ = tx.send(WorkerEvent::ServerLog(format!(
                                "[Tunnel] bore error: {e}"
                            )));
                        }
                        break;
                    }
                },
                Err(err) => {
                    if assigned_port.is_none() {
                        if let Some(tx) = &tx_owned {
                            let _ = tx.send(WorkerEvent::ServerTunnelFailed(err));
                        }
                    }
                    break;
                }
            }
        }
    });

    Ok(Arc::new(TunnelHandle(stop_flag)))
}

fn start_public_tunnel(
    port: u16,
    tx: Option<&Sender<WorkerEvent>>,
) -> Result<Arc<TunnelHandle>, String> {
    emit_server_log(tx, "[Network] Connecting built-in bore.pub tunnel...");
    start_bore_tunnel(port, tx)
}

/// Detects the primary local LAN IPv4 address without sending network traffic.
pub fn detect_local_lan_ip() -> Option<String> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("1.1.1.1:80").ok()?;
    let ip = sock.local_addr().ok()?.ip();
    if ip.is_loopback() || ip.is_unspecified() {
        None
    } else {
        Some(ip.to_string())
    }
}

fn parse_ssdp_location(response: &str) -> Option<String> {
    for line in response.lines() {
        let trimmed = line.trim();
        if let Some((k, v)) = trimmed.split_once(':') {
            if k.trim().eq_ignore_ascii_case("location") {
                let val = v.trim();
                if val.starts_with("http://") || val.starts_with("https://") {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim().to_string())
}

fn extract_upnp_control_url(device_xml: &str, location_url: &str) -> Option<(String, String)> {
    let service_types = [
        "urn:schemas-upnp-org:service:WANIPConnection:1",
        "urn:schemas-upnp-org:service:WANIPConnection:2",
        "urn:schemas-upnp-org:service:WANPPPConnection:1",
    ];
    let base = reqwest::Url::parse(location_url).ok()?;
    for st in service_types {
        if let Some(pos) = device_xml.find(st) {
            let tail = &device_xml[pos..];
            let block_end = tail.find("</service>").unwrap_or(tail.len());
            let block = &tail[..block_end];
            if let Some(ctrl_rel) = extract_xml_tag(block, "controlURL") {
                if let Ok(resolved) = base.join(&ctrl_rel) {
                    return Some((st.to_string(), resolved.to_string()));
                }
            }
        }
    }
    None
}

fn soap_call(
    client: &reqwest::blocking::Client,
    control_url: &str,
    service_type: &str,
    action: &str,
    body_inner: &str,
) -> Result<String, String> {
    let envelope = format!(
        r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:{action} xmlns:u="{service_type}">
      {body_inner}
    </u:{action}>
  </s:Body>
</s:Envelope>"#
    );
    let resp = client
        .post(control_url)
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .header("SOAPAction", format!("\"{service_type}#{action}\""))
        .body(envelope)
        .send()
        .map_err(|e| format!("UPnP {action} failed: {e}"))?;
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("UPnP {action} HTTP {status}"));
    }
    Ok(text)
}

/// Discovers a UPnP IGD router on the local network and forwards TCP `port`.
pub fn open_upnp_port(port: u16, tx: Option<&Sender<WorkerEvent>>) -> Result<UpnpMapping, String> {
    let local_ip = detect_local_lan_ip()
        .ok_or_else(|| "could not determine local LAN IP".to_string())?;

    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("SSDP bind failed: {e}"))?;
    sock.set_read_timeout(Some(Duration::from_millis(1200)))
        .ok();

    let mut location: Option<String> = None;
    for st in [
        "urn:schemas-upnp-org:device:InternetGatewayDevice:1",
        "urn:schemas-upnp-org:device:InternetGatewayDevice:2",
    ] {
        let req = format!(
            "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 1\r\nST: {st}\r\n\r\n"
        );
        let _ = sock.send_to(req.as_bytes(), "239.255.255.250:1900");
        let mut buf = [0u8; 2048];
        if let Ok((len, _)) = sock.recv_from(&mut buf) {
            let text = String::from_utf8_lossy(&buf[..len]);
            if let Some(loc) = parse_ssdp_location(&text) {
                location = Some(loc);
                break;
            }
        }
    }

    let loc = location.ok_or_else(|| "no UPnP router responded".to_string())?;
    let client = build_http_client()?;
    let device_xml = client
        .get(&loc)
        .timeout(Duration::from_secs(3))
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("failed to fetch router description: {e}"))?
        .text()
        .map_err(|e| format!("failed to read router description: {e}"))?;

    let (service_type, control_url) = extract_upnp_control_url(&device_xml, &loc)
        .ok_or_else(|| "router has no WANIPConnection/WANPPPConnection service".to_string())?;

    emit_progress(tx, format!("Opening port {port} via UPnP..."), 0.97);
    let add_body = format!(
        "<NewRemoteHost></NewRemoteHost>\
         <NewExternalPort>{port}</NewExternalPort>\
         <NewProtocol>TCP</NewProtocol>\
         <NewInternalPort>{port}</NewInternalPort>\
         <NewInternalClient>{local_ip}</NewInternalClient>\
         <NewEnabled>1</NewEnabled>\
         <NewPortMappingDescription>Sunset Launcher MC Server</NewPortMappingDescription>\
         <NewLeaseDuration>0</NewLeaseDuration>"
    );
    soap_call(&client, &control_url, &service_type, "AddPortMapping", &add_body)?;

    let public_ip = soap_call(
        &client,
        &control_url,
        &service_type,
        "GetExternalIPAddress",
        "",
    )
    .ok()
    .and_then(|xml| extract_xml_tag(&xml, "NewExternalIPAddress"))
    .filter(|ip| !ip.is_empty());

    Ok(UpnpMapping {
        control_url,
        service_type,
        port,
        public_ip,
    })
}

/// Removes a previously opened UPnP TCP port mapping.
pub fn close_upnp_port(mapping: &UpnpMapping) {
    if let Ok(client) = build_http_client() {
        let body = format!(
            "<NewRemoteHost></NewRemoteHost>\
             <NewExternalPort>{}</NewExternalPort>\
             <NewProtocol>TCP</NewProtocol>",
            mapping.port
        );
        let _ = soap_call(
            &client,
            &mapping.control_url,
            &mapping.service_type,
            "DeletePortMapping",
            &body,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_java_major_maps_mc_versions() {
        assert_eq!(required_java_major_for_mc("1.21.4"), 21);
        assert_eq!(required_java_major_for_mc("1.20.5"), 21);
        assert_eq!(required_java_major_for_mc("1.20.4"), 17);
        assert_eq!(required_java_major_for_mc("1.18.2"), 17);
        assert_eq!(required_java_major_for_mc("1.17.1"), 16);
        assert_eq!(required_java_major_for_mc("1.16.5"), 8);
        assert_eq!(required_java_major_for_mc("1.12.2"), 8);
    }

    #[test]
    fn update_server_properties_preserves_existing_keys_and_updates_managed() {
        let existing = "\
#Minecraft server properties
difficulty=hard
server-port=25565
motd=Old MOTD
pvp=true
online-mode=true
";
        let mut srv = DedicatedServer::new(
            "My SMP".to_string(),
            ServerSoftware::Paper,
            "1.21.4".to_string(),
            25570,
            4096,
            false,
            "Sunset SMP".to_string(),
            true,
        );
        srv.max_players = 32;

        let updated = update_server_properties_text(existing, &srv);
        assert!(updated.contains("difficulty=hard"));
        assert!(updated.contains("pvp=true"));
        assert!(updated.contains("server-port=25570"));
        assert!(updated.contains("motd=Sunset SMP"));
        assert!(updated.contains("max-players=32"));
        assert!(updated.contains("online-mode=false"));
        assert!(!updated.contains("Old MOTD"));
    }

    #[test]
    fn server_software_capabilities() {
        assert!(!ServerSoftware::Vanilla.supports_plugins());
        assert!(ServerSoftware::Paper.supports_plugins());
        assert!(ServerSoftware::Purpur.supports_plugins());
        assert!(ServerSoftware::Folia.supports_plugins());
        assert!(ServerSoftware::Fabric.supports_mods());
        assert!(ServerSoftware::Quilt.supports_mods());
        assert!(ServerSoftware::Forge.supports_mods());
        assert!(ServerSoftware::NeoForge.supports_mods());
        assert!(ServerSoftware::Arclight.supports_mods());
        assert!(ServerSoftware::Arclight.supports_plugins());
    }

    #[test]
    fn select_arclight_asset_prefers_matching_mc_version() {
        let releases = vec![GitHubRelease {
            tag_name: "1.0.3".to_string(),
            assets: vec![
                GitHubAsset {
                    name: "arclight-fabric-1.21.1-1.0.3.jar".to_string(),
                    browser_download_url: "https://example.com/fabric.jar".to_string(),
                },
                GitHubAsset {
                    name: "arclight-neoforge-1.21.1-1.0.3.jar".to_string(),
                    browser_download_url: "https://example.com/neoforge.jar".to_string(),
                },
            ],
        }];
        let (tag, asset) = select_arclight_asset(&releases, "1.21.1").unwrap();
        assert_eq!(tag, "1.0.3");
        assert_eq!(asset.name, "arclight-neoforge-1.21.1-1.0.3.jar");
        assert!(select_arclight_asset(&releases, "1.19.2").is_none());
    }

    #[test]
    fn server_plugin_bridge_mod_detection_works() {
        let tmp = std::env::temp_dir().join(format!("srv_plug_test_{}", uuid::Uuid::new_v4()));
        let mods = tmp.join("mods");
        fs::create_dir_all(&mods).unwrap();
        assert!(!server_supports_plugins_live(&tmp, ServerSoftware::Fabric));
        assert!(server_supports_plugins_live(&tmp, ServerSoftware::Paper));

        fs::write(mods.join("cardboard-1.21.1.jar"), b"jar").unwrap();
        assert!(server_supports_plugins_live(&tmp, ServerSoftware::Fabric));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn server_properties_entries_roundtrip_preserves_comments() {
        let tmp = std::env::temp_dir().join(format!("srv_props_rt_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(
            tmp.join("server.properties"),
            "#Header comment\npvp=true\nview-distance=10\n",
        )
        .unwrap();
        let mut entries = read_server_properties_entries(&tmp);
        assert!(entries.len() >= 22);
        if let Some(pvp) = entries.iter_mut().find(|(k, _)| k == "pvp") {
            pvp.1 = "false".to_string();
        }
        entries.push(("custom-flag".to_string(), "true".to_string()));
        write_server_properties_entries(&tmp, &entries).unwrap();

        let saved = fs::read_to_string(tmp.join("server.properties")).unwrap();
        assert!(saved.contains("#Header comment"));
        assert!(saved.contains("pvp=false"));
        assert!(saved.contains("view-distance=10"));
        assert!(saved.contains("custom-flag=true"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn bore_protocol_json_framing_roundtrip() {
        let hello = serde_json::to_string(&BoreClientMsg::Hello(0)).unwrap();
        assert_eq!(hello, "{\"Hello\":0}");
        let accept = serde_json::to_string(&BoreClientMsg::Accept(
            "550e8400-e29b-41d4-a716-446655440000".to_string(),
        ))
        .unwrap();
        assert_eq!(
            accept,
            "{\"Accept\":\"550e8400-e29b-41d4-a716-446655440000\"}"
        );
        let srv_hello: BoreServerMsg = serde_json::from_str("{\"Hello\":41923}").unwrap();
        assert_eq!(srv_hello, BoreServerMsg::Hello(41923));
        let srv_conn: BoreServerMsg =
            serde_json::from_str("{\"Connection\":\"550e8400-e29b-41d4-a716-446655440000\"}")
                .unwrap();
        assert_eq!(
            srv_conn,
            BoreServerMsg::Connection("550e8400-e29b-41d4-a716-446655440000".to_string())
        );
        let hb: BoreServerMsg = serde_json::from_str("\"Heartbeat\"").unwrap();
        assert_eq!(hb, BoreServerMsg::Heartbeat);
    }

    #[test]
    fn parse_bore_and_upnp_helpers_work() {
        assert_eq!(
            parse_bore_listening_address("2026-11-19T20:20:00Z INFO bore_cli::client: listening at bore.pub:41923")
                .as_deref(),
            Some("bore.pub:41923")
        );
        let ssdp = "HTTP/1.1 200 OK\r\nLOCATION: http://192.168.1.1:5000/rootDesc.xml\r\n";
        assert_eq!(
            parse_ssdp_location(ssdp).as_deref(),
            Some("http://192.168.1.1:5000/rootDesc.xml")
        );
        let xml = r#"<service><serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType><controlURL>/ctl/IPConn</controlURL></service>"#;
        let (st, url) =
            extract_upnp_control_url(xml, "http://192.168.1.1:5000/rootDesc.xml").unwrap();
        assert_eq!(st, "urn:schemas-upnp-org:service:WANIPConnection:1");
        assert_eq!(url, "http://192.168.1.1:5000/ctl/IPConn");
    }

    #[test]
    fn save_server_icon_png_resizes_to_64x64_png() {
        let tmp = std::env::temp_dir().join(format!("srv_icon_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        let src = tmp.join("raw.png");
        let img = image::RgbaImage::from_pixel(120, 80, image::Rgba([255, 128, 66, 255]));
        image::DynamicImage::ImageRgba8(img)
            .save_with_format(&src, image::ImageFormat::Png)
            .unwrap();

        let saved = save_server_icon_png(&tmp, &src).unwrap();
        assert_eq!(saved, tmp.join("server-icon.png"));
        assert!(saved.is_file());
        let decoded = image::open(&saved).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (64, 64));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn delete_server_property_entry_removes_only_target_and_keeps_comments() {
        let tmp = std::env::temp_dir().join(format!("srv_del_prop_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(
            tmp.join("server.properties"),
            "# Header comment\npvp=true\ncustom-remove=42\nview-distance=10\n",
        )
        .unwrap();

        delete_server_property_entry(&tmp, "custom-remove").unwrap();
        let content = fs::read_to_string(tmp.join("server.properties")).unwrap();
        assert!(content.contains("# Header comment"));
        assert!(content.contains("pvp=true"));
        assert!(content.contains("view-distance=10"));
        assert!(!content.contains("custom-remove"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn parse_mc_motd_segments_parses_colors_and_reset() {
        let segs = parse_mc_motd_segments("§6Sunset §l§aSMP§r world");
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], ("Sunset ".to_string(), Some([255, 170, 0]), false));
        assert_eq!(segs[1], ("SMP".to_string(), Some([85, 255, 85]), true));
        assert_eq!(segs[2], (" world".to_string(), None, false));
    }
}

