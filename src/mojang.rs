use crate::models::{
    AssetIndex, FabricLoaderEntry, LauncherPaths, Library, LoaderKind, LoaderProfileOverlay,
    LoaderVersionEntry, ModLoader, Rule, VersionDetails, VersionManifest, WorkerEvent,
};
use reqwest::blocking::Client;
use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const FABRIC_META_BASE: &str = "https://meta.fabricmc.net/v2";
const QUILT_META_BASE: &str = "https://meta.quiltmc.org/v3";
const FORGE_MAVEN_META: &str =
    "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";
const FORGE_PROMOTIONS_URL: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const NEOFORGE_MAVEN_META: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
const NEOFORGE_LEGACY_META: &str =
    "https://maven.neoforged.net/releases/net/neoforged/forge/maven-metadata.xml";
const RESOURCES_BASE: &str = "https://resources.download.minecraft.net";

pub fn build_http_client() -> Result<Client, String> {
    Client::builder()
        .user_agent("svarka-launcher/0.1.0")
        .pool_max_idle_per_host(32)
        .pool_idle_timeout(Duration::from_secs(60))
        .tcp_nodelay(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))
}

pub(crate) fn build_async_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("svarka-launcher/0.1.0")
        .pool_max_idle_per_host(64)
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_nodelay(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| format!("Failed to create async HTTP client: {e}"))
}

pub fn fetch_version_manifest() -> Result<VersionManifest, String> {
    let client = build_http_client()?;
    client
        .get(VERSION_MANIFEST_URL)
        .send()
        .map_err(|e| format!("Failed to fetch version manifest: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Version manifest HTTP error: {e}"))?
        .json::<VersionManifest>()
        .map_err(|e| format!("Failed to parse version manifest: {e}"))
}

fn fetch_meta_loader_versions(
    base_url: &str,
    name: &str,
    game_version: &str,
) -> Result<Vec<LoaderVersionEntry>, String> {
    let client = build_http_client()?;
    let url = format!("{base_url}/versions/loader/{game_version}");
    let list = client
        .get(&url)
        .send()
        .map_err(|e| format!("Failed to fetch {name} versions: {e}"))?
        .error_for_status()
        .map_err(|e| format!("{name} Meta HTTP error: {e}"))?
        .json::<Vec<FabricLoaderEntry>>()
        .map_err(|e| format!("Failed to parse {name} versions: {e}"))?;
    Ok(list
        .into_iter()
        .map(|entry| {
            let v = entry.loader.version;
            let stable = if entry.loader.stable {
                true
            } else {
                !v.contains("beta") && !v.contains("alpha")
            };
            LoaderVersionEntry { version: v, stable }
        })
        .collect())
}

fn fetch_meta_loader_overlay(
    base_url: &str,
    name: &str,
    game_version: &str,
    loader_version: &str,
) -> Result<LoaderProfileOverlay, String> {
    let client = build_http_client()?;
    let url = format!("{base_url}/versions/loader/{game_version}/{loader_version}/profile/json");
    client
        .get(&url)
        .send()
        .map_err(|e| format!("Failed to fetch {name} profile: {e}"))?
        .error_for_status()
        .map_err(|e| format!("{name} profile HTTP error: {e}"))?
        .json::<LoaderProfileOverlay>()
        .map_err(|e| format!("Failed to parse {name} profile: {e}"))
}

#[derive(Deserialize)]
struct ForgePromotions {
    #[serde(default)]
    promos: HashMap<String, String>,
}

/// Extracts `<version>...</version>` tags from Maven `maven-metadata.xml`.
pub fn parse_maven_metadata_versions(xml: &str) -> Vec<String> {
    let mut versions = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<version>") {
        let after = &rest[start + "<version>".len()..];
        if let Some(end) = after.find("</version>") {
            let v = after[..end].trim();
            if !v.is_empty() {
                versions.push(v.to_string());
            }
            rest = &after[end + "</version>".len()..];
        } else {
            break;
        }
    }
    versions
}

/// Computes the NeoForge Maven version prefix (`21.4.` for MC `1.21.4`, `21.0.` for `1.21`).
pub fn neoforge_mc_prefix(game_version: &str) -> Option<String> {
    if let Some(rest) = game_version.strip_prefix("1.") {
        let mut parts = rest.split('.');
        let minor = parts.next()?;
        let patch = parts.next().unwrap_or("0");
        if minor.chars().all(|c| c.is_ascii_digit()) && patch.chars().all(|c| c.is_ascii_digit()) {
            return Some(format!("{minor}.{patch}."));
        }
    }
    None
}

pub fn filter_forge_versions(
    all_maven_versions: &[String],
    game_version: &str,
    recommended: Option<&str>,
) -> Vec<LoaderVersionEntry> {
    let prefix = format!("{game_version}-");
    let mut result: Vec<LoaderVersionEntry> = all_maven_versions
        .iter()
        .filter_map(|full| full.strip_prefix(&prefix))
        .map(|short| LoaderVersionEntry {
            stable: recommended == Some(short),
            version: short.to_string(),
        })
        .collect();
    result.reverse();
    if let Some(rec) = recommended {
        if let Some(pos) = result.iter().position(|e| e.version == rec) {
            let item = result.remove(pos);
            result.insert(0, item);
        }
    }
    result
}

pub fn filter_neoforge_versions(
    all_maven_versions: &[String],
    game_version: &str,
) -> Vec<LoaderVersionEntry> {
    let mut result: Vec<LoaderVersionEntry> = if game_version == "1.20.1" {
        all_maven_versions
            .iter()
            .filter_map(|full| full.strip_prefix("1.20.1-"))
            .map(|short| LoaderVersionEntry {
                stable: !short.contains("beta") && !short.contains("alpha"),
                version: short.to_string(),
            })
            .collect()
    } else if let Some(prefix) = neoforge_mc_prefix(game_version) {
        all_maven_versions
            .iter()
            .filter(|v| v.starts_with(&prefix))
            .map(|v| LoaderVersionEntry {
                stable: !v.contains("beta") && !v.contains("alpha"),
                version: v.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };
    result.reverse();
    result
}

fn fetch_forge_versions(game_version: &str) -> Result<Vec<LoaderVersionEntry>, String> {
    let client = build_http_client()?;
    let xml = client
        .get(FORGE_MAVEN_META)
        .send()
        .map_err(|e| format!("Failed to fetch Forge metadata: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Forge metadata HTTP error: {e}"))?
        .text()
        .map_err(|e| format!("Failed to read Forge metadata: {e}"))?;

    let recommended = client
        .get(FORGE_PROMOTIONS_URL)
        .send()
        .ok()
        .and_then(|r| r.error_for_status().ok())
        .and_then(|r| r.json::<ForgePromotions>().ok())
        .and_then(|p| p.promos.get(&format!("{game_version}-recommended")).cloned());

    let versions = parse_maven_metadata_versions(&xml);
    Ok(filter_forge_versions(
        &versions,
        game_version,
        recommended.as_deref(),
    ))
}

fn fetch_neoforge_versions(game_version: &str) -> Result<Vec<LoaderVersionEntry>, String> {
    let client = build_http_client()?;
    let meta_url = if game_version == "1.20.1" {
        NEOFORGE_LEGACY_META
    } else {
        NEOFORGE_MAVEN_META
    };
    let xml = client
        .get(meta_url)
        .send()
        .map_err(|e| format!("Failed to fetch NeoForge metadata: {e}"))?
        .error_for_status()
        .map_err(|e| format!("NeoForge metadata HTTP error: {e}"))?
        .text()
        .map_err(|e| format!("Failed to read NeoForge metadata: {e}"))?;

    let versions = parse_maven_metadata_versions(&xml);
    Ok(filter_neoforge_versions(&versions, game_version))
}

/// Fetches available loader versions for the given loader kind and Minecraft version.
pub fn fetch_loader_versions(
    loader: LoaderKind,
    game_version: &str,
) -> Result<Vec<LoaderVersionEntry>, String> {
    match loader {
        LoaderKind::Vanilla => Ok(Vec::new()),
        LoaderKind::Fabric => fetch_meta_loader_versions(FABRIC_META_BASE, "Fabric", game_version),
        LoaderKind::Quilt => fetch_meta_loader_versions(QUILT_META_BASE, "Quilt", game_version),
        LoaderKind::Forge => fetch_forge_versions(game_version),
        LoaderKind::NeoForge => fetch_neoforge_versions(game_version),
    }
}

/// Returns the current OS name as used by Mojang's version JSON rules.
pub fn current_os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

/// Evaluates Mojang library/argument rules for the current OS (ignoring feature-gated rules).
pub fn rules_allow(rules: Option<&Vec<Rule>>) -> bool {
    let Some(rules) = rules else {
        return true;
    };
    if rules.is_empty() {
        return true;
    }

    let os_name = current_os_name();
    let mut allowed = false;

    for rule in rules {
        // Skip feature-specific rules (demo, custom resolution, quick play)
        if rule.features.as_ref().is_some_and(|f| !f.is_empty()) {
            continue;
        }

        let os_matches = match &rule.os {
            None => true,
            Some(os) => {
                let name_ok = os.name.as_deref().map_or(true, |n| n == os_name);
                let arch_ok = os.arch.as_deref().map_or(true, |a| match a {
                    "x86" => cfg!(target_arch = "x86"),
                    "x86_64" | "amd64" => cfg!(target_arch = "x86_64"),
                    "arm64" | "aarch64" => cfg!(target_arch = "aarch64"),
                    _ => true,
                });
                name_ok && arch_ok
            }
        };

        if os_matches {
            allowed = rule.action == "allow";
        }
    }

    allowed
}

/// Converts Maven coordinates `group:artifact:version[:classifier]` to a relative repository path.
pub fn maven_to_path(coords: &str) -> Option<String> {
    let parts: Vec<&str> = coords.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let group_path = parts[0].replace('.', "/");
    let artifact = parts[1];
    let (version, ext) = if let Some((v, e)) = parts[2].split_once('@') {
        (v, e)
    } else {
        (parts[2], "jar")
    };

    let filename = if parts.len() >= 4 && !parts[3].is_empty() {
        let (cls, cls_ext) = if let Some((c, e)) = parts[3].split_once('@') {
            (c, e)
        } else {
            (parts[3], ext)
        };
        format!("{artifact}-{version}-{cls}.{cls_ext}")
    } else {
        format!("{artifact}-{version}.{ext}")
    };

    Some(format!("{group_path}/{artifact}/{version}/{filename}"))
}

fn join_url(base: &str, rel_path: &str) -> String {
    if base.ends_with('/') {
        format!("{base}{rel_path}")
    } else {
        format!("{base}/{rel_path}")
    }
}

/// `group:artifact[:classifier]` without the version, used to detect duplicate libraries.
pub fn maven_key(coords: &str) -> Option<String> {
    let parts: Vec<&str> = coords.split(':').collect();
    match parts.as_slice() {
        [group, artifact, _version] => Some(format!("{group}:{artifact}")),
        [group, artifact, _version, classifier, ..] => {
            Some(format!("{group}:{artifact}:{classifier}"))
        }
        _ => None,
    }
}

fn verify_sha1(path: &Path, expected_sha1: &str) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 65536];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buf[..n]),
            Err(_) => return false,
        }
    }
    let digest = hex::encode(hasher.finalize());
    digest.eq_ignore_ascii_case(expected_sha1)
}

fn tmp_part_path(dest: &Path) -> PathBuf {
    let mut tmp_name = dest.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".part");
    dest.with_file_name(tmp_name)
}

pub fn download_file(
    client: &Client,
    url: &str,
    dest: &Path,
    expected_sha1: Option<&str>,
) -> Result<(), String> {
    if dest.exists() {
        match expected_sha1 {
            Some(sha1) if verify_sha1(dest, sha1) => return Ok(()),
            Some(_) => {}
            None => return Ok(()),
        }
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create dir {}: {e}", parent.display()))?;
    }

    // Retry transient network failures a few times before giving up.
    let mut last_err = String::new();
    for attempt in 1..=3 {
        match download_once(client, url, dest, expected_sha1) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = e;
                if attempt < 3 {
                    std::thread::sleep(Duration::from_millis(350 * attempt));
                }
            }
        }
    }
    Err(last_err)
}

fn download_once(
    client: &Client,
    url: &str,
    dest: &Path,
    expected_sha1: Option<&str>,
) -> Result<(), String> {
    let mut resp = client
        .get(url)
        .send()
        .map_err(|e| format!("Download failed for {url}: {e}"))?
        .error_for_status()
        .map_err(|e| format!("HTTP error for {url}: {e}"))?;

    let tmp_path = tmp_part_path(dest);
    let file = File::create(&tmp_path)
        .map_err(|e| format!("Failed to create {}: {e}", tmp_path.display()))?;
    let mut out = BufWriter::with_capacity(65536, file);
    let mut hasher = expected_sha1.map(|_| Sha1::new());
    let mut buf = [0u8; 65536];

    loop {
        let n = resp
            .read(&mut buf)
            .map_err(|e| format!("Download stream error for {url}: {e}"))?;
        if n == 0 {
            break;
        }
        if let Some(ref mut h) = hasher {
            h.update(&buf[..n]);
        }
        out.write_all(&buf[..n])
            .map_err(|e| format!("Failed to write {}: {e}", tmp_path.display()))?;
    }
    out.flush()
        .map_err(|e| format!("Failed to flush {}: {e}", tmp_path.display()))?;
    drop(out);

    if let (Some(sha1), Some(h)) = (expected_sha1, hasher) {
        let digest = hex::encode(h.finalize());
        if !digest.eq_ignore_ascii_case(sha1) {
            let _ = fs::remove_file(&tmp_path);
            return Err(format!("SHA-1 mismatch for {url}"));
        }
    }

    fs::rename(&tmp_path, dest)
        .map_err(|e| format!("Failed to finalize {}: {e}", dest.display()))?;
    Ok(())
}

async fn download_async_with_retry(
    client: &reqwest::Client,
    job: &DownloadJob,
) -> Result<(), String> {
    if job.dest.is_file() {
        match job.sha1.as_deref() {
            Some(sha1) if verify_sha1(&job.dest, sha1) => return Ok(()),
            Some(_) => {}
            None => return Ok(()),
        }
    }

    let mut last_err = String::new();
    for attempt in 1..=3 {
        match download_async_once(client, &job.url, &job.dest, job.sha1.as_deref()).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                let extra_delay = if e.contains("429") { 800 } else { 250 };
                last_err = e;
                if attempt < 3 {
                    tokio::time::sleep(Duration::from_millis(extra_delay * attempt as u64)).await;
                }
            }
        }
    }
    Err(last_err)
}

async fn download_async_once(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_sha1: Option<&str>,
) -> Result<(), String> {
    let mut resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download failed for {url}: {e}"))?
        .error_for_status()
        .map_err(|e| format!("HTTP error for {url}: {e}"))?;

    let tmp_path = tmp_part_path(dest);
    let file = File::create(&tmp_path)
        .map_err(|e| format!("Failed to create {}: {e}", tmp_path.display()))?;
    let mut out = BufWriter::with_capacity(65536, file);
    let mut hasher = expected_sha1.map(|_| Sha1::new());

    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("Download stream error for {url}: {e}"))?
    {
        if let Some(ref mut h) = hasher {
            h.update(&chunk);
        }
        out.write_all(&chunk)
            .map_err(|e| format!("Failed to write {}: {e}", tmp_path.display()))?;
    }
    out.flush()
        .map_err(|e| format!("Failed to flush {}: {e}", tmp_path.display()))?;
    drop(out);

    if let (Some(sha1), Some(h)) = (expected_sha1, hasher) {
        let digest = hex::encode(h.finalize());
        if !digest.eq_ignore_ascii_case(sha1) {
            let _ = fs::remove_file(&tmp_path);
            return Err(format!("SHA-1 mismatch for {url}"));
        }
    }

    fs::rename(&tmp_path, dest)
        .map_err(|e| format!("Failed to finalize {}: {e}", dest.display()))?;
    Ok(())
}

/// A single file to fetch as part of a parallel batch.
pub struct DownloadJob {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    /// When set, an existing file of exactly this size is trusted without hashing
    /// (used for the thousands of asset objects, where hashing everything is slow).
    pub size: Option<u64>,
}

/// Downloads many files concurrently over pooled async HTTP connections.
/// Jobs with the same destination are deduplicated, and parent directories are created once upfront.
pub fn download_parallel(
    _client: &Client,
    jobs: Vec<DownloadJob>,
    concurrency: usize,
    on_progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<(), String> {
    let mut seen = HashSet::new();
    let jobs: Vec<DownloadJob> = jobs
        .into_iter()
        .filter(|j| seen.insert(j.dest.clone()))
        .filter(|j| match (j.size, j.dest.metadata()) {
            (Some(size), Ok(meta)) => meta.len() != size,
            _ => true,
        })
        .collect();

    let total = jobs.len();
    if total == 0 {
        on_progress(0, 0);
        return Ok(());
    }

    let mut seen_dirs = HashSet::new();
    for job in &jobs {
        if let Some(parent) = job.dest.parent() {
            if !parent.as_os_str().is_empty() && seen_dirs.insert(parent) {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create dir {}: {e}", parent.display()))?;
            }
        }
    }

    let async_client = build_async_http_client()?;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to start download runtime: {e}"))?;

    let worker_count = concurrency.clamp(1, 64).min(total);
    let jobs = Arc::new(jobs);
    let next = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicUsize::new(0));
    let aborted = Arc::new(AtomicBool::new(false));
    let first_error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let (prog_tx, prog_rx) = std::sync::mpsc::channel::<usize>();

    std::thread::scope(|scope| {
        scope.spawn(move || {
            while let Ok(finished) = prog_rx.recv() {
                on_progress(finished, total);
            }
        });

        let first_error = Arc::clone(&first_error);
        rt.block_on(async move {
            let mut handles = Vec::with_capacity(worker_count);
            for _ in 0..worker_count {
                let client = async_client.clone();
                let jobs = Arc::clone(&jobs);
                let next = Arc::clone(&next);
                let done = Arc::clone(&done);
                let aborted = Arc::clone(&aborted);
                let first_error = Arc::clone(&first_error);
                let prog_tx = prog_tx.clone();
                handles.push(tokio::spawn(async move {
                    loop {
                        if aborted.load(Ordering::Relaxed) {
                            break;
                        }
                        let idx = next.fetch_add(1, Ordering::Relaxed);
                        let Some(job) = jobs.get(idx) else {
                            break;
                        };
                        if let Err(e) = download_async_with_retry(&client, job).await {
                            aborted.store(true, Ordering::Relaxed);
                            if let Ok(mut slot) = first_error.lock() {
                                slot.get_or_insert(e);
                            }
                            break;
                        }
                        let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                        let _ = prog_tx.send(finished);
                    }
                }));
            }
            drop(prog_tx);
            for handle in handles {
                let _ = handle.await;
            }
        });
    });

    match first_error.lock().ok().and_then(|mut g| g.take()) {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

fn extract_natives(jar_path: &Path, natives_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(natives_dir)
        .map_err(|e| format!("Failed to create natives dir: {e}"))?;
    let file = File::open(jar_path)
        .map_err(|e| format!("Failed to open native jar {}: {e}", jar_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Invalid zip {}: {e}", jar_path.display()))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Zip entry error: {e}"))?;
        let name = entry.name().to_string();
        if entry.is_dir() || name.starts_with("META-INF/") {
            continue;
        }
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let out_path = natives_dir.join(rel);
        if let Some(parent) = out_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut out = File::create(&out_path)
            .map_err(|e| format!("Failed to extract {}: {e}", out_path.display()))?;
        io::copy(&mut entry, &mut out)
            .map_err(|e| format!("Failed to write extracted file: {e}"))?;
    }
    Ok(())
}

pub struct PreparedLaunch {
    pub version_details: VersionDetails,
    pub loader_overlay: Option<LoaderProfileOverlay>,
    pub classpath: Vec<PathBuf>,
    pub natives_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
struct ForgeInstallProfile {
    #[serde(default)]
    libraries: Vec<Library>,
    #[serde(default)]
    data: HashMap<String, InstallerSideValue>,
    #[serde(default)]
    processors: Vec<InstallerProcessor>,
}

#[derive(Debug, Clone, Deserialize)]
struct InstallerSideValue {
    client: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct InstallerProcessor {
    #[serde(default)]
    sides: Option<Vec<String>>,
    jar: String,
    #[serde(default)]
    classpath: Vec<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    outputs: HashMap<String, String>,
}

/// Collects download jobs, optional natives, and optional classpath entries for a list of libraries.
fn collect_libraries(
    libs: &[Library],
    default_maven_base: &str,
    libraries_dir: &Path,
    lib_jobs: &mut Vec<DownloadJob>,
    native_jars: &mut Vec<PathBuf>,
    mut cp_out: Option<&mut Vec<(PathBuf, Option<String>)>>,
) -> Result<(), String> {
    for lib in libs.iter().filter(|lib| rules_allow(lib.rules.as_ref())) {
        if let Some(downloads) = &lib.downloads {
            if let Some(artifact) = &downloads.artifact {
                let rel_path = artifact
                    .path
                    .clone()
                    .or_else(|| maven_to_path(&lib.name))
                    .ok_or_else(|| format!("Cannot resolve path for library {}", lib.name))?;
                let dest = libraries_dir.join(&rel_path);
                if !artifact.url.is_empty() {
                    lib_jobs.push(DownloadJob {
                        url: artifact.url.clone(),
                        dest: dest.clone(),
                        sha1: artifact.sha1.clone(),
                        size: None,
                    });
                }
                if let Some(ref mut cp) = cp_out {
                    cp.push((dest, maven_key(&lib.name)));
                }
            }

            // Legacy natives (pre-1.19): a classifier jar that must be extracted.
            let classifier = lib
                .natives
                .as_ref()
                .and_then(|m| m.get(current_os_name()))
                .map(|tmpl| {
                    let bits = if cfg!(target_pointer_width = "64") {
                        "64"
                    } else {
                        "32"
                    };
                    tmpl.replace("${arch}", bits)
                });
            if let (Some(classifier), Some(classifiers)) = (classifier, &downloads.classifiers) {
                if let Some(native_art) = classifiers.get(&classifier) {
                    let rel_path = native_art
                        .path
                        .clone()
                        .unwrap_or_else(|| format!("natives/{}-{classifier}.jar", lib.name));
                    let dest = libraries_dir.join(&rel_path);
                    if !native_art.url.is_empty() {
                        lib_jobs.push(DownloadJob {
                            url: native_art.url.clone(),
                            dest: dest.clone(),
                            sha1: native_art.sha1.clone(),
                            size: None,
                        });
                    }
                    native_jars.push(dest);
                }
            }
        } else if let Some(rel_path) = maven_to_path(&lib.name) {
            let base = lib.url.as_deref().unwrap_or(default_maven_base);
            let dest = libraries_dir.join(&rel_path);
            if !base.is_empty() {
                lib_jobs.push(DownloadJob {
                    url: join_url(base, &rel_path),
                    dest: dest.clone(),
                    sha1: None,
                    size: None,
                });
            }
            if let Some(ref mut cp) = cp_out {
                cp.push((dest, maven_key(&lib.name)));
            }
        }
    }
    Ok(())
}

/// Parses `Main-Class` from a JAR's `META-INF/MANIFEST.MF` (handling continuation lines).
pub fn parse_manifest_main_class(manifest_text: &str) -> Option<String> {
    let unfolded = manifest_text.replace("\r\n ", "").replace("\n ", "");
    for line in unfolded.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("Main-Class:") {
            let cls = val.trim();
            if !cls.is_empty() {
                return Some(cls.to_string());
            }
        }
    }
    None
}

fn read_jar_main_class(jar_path: &Path) -> Result<String, String> {
    let file = File::open(jar_path)
        .map_err(|e| format!("Failed to open processor jar {}: {e}", jar_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Invalid processor jar {}: {e}", jar_path.display()))?;
    let mut mf = archive
        .by_name("META-INF/MANIFEST.MF")
        .map_err(|e| format!("Missing MANIFEST.MF in {}: {e}", jar_path.display()))?;
    let mut text = String::new();
    mf.read_to_string(&mut text)
        .map_err(|e| format!("Failed to read MANIFEST.MF in {}: {e}", jar_path.display()))?;
    parse_manifest_main_class(&text)
        .ok_or_else(|| format!("No Main-Class in {}", jar_path.display()))
}

fn resolve_data_entry(
    raw: &str,
    data_dir: &Path,
    libraries_dir: &Path,
) -> Result<String, String> {
    if let Some(quoted) = raw.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return Ok(quoted.to_string());
    }
    if let Some(coord) = raw.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let rel = maven_to_path(coord)
            .ok_or_else(|| format!("Invalid installer Maven coordinate: {coord}"))?;
        return Ok(libraries_dir.join(rel).to_string_lossy().to_string());
    }
    if let Some(rel) = raw.strip_prefix('/') {
        return Ok(data_dir.join(rel).to_string_lossy().to_string());
    }
    Ok(raw.to_string())
}

pub fn resolve_processor_arg(
    raw: &str,
    vars: &HashMap<String, String>,
    libraries_dir: &Path,
) -> Result<String, String> {
    let mut s = raw.to_string();
    for (k, v) in vars {
        let pat = format!("{{{k}}}");
        if s.contains(&pat) {
            s = s.replace(&pat, v);
        }
    }
    if let Some(coord) = s.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
        let rel = maven_to_path(coord)
            .ok_or_else(|| format!("Invalid processor arg coordinate: {coord}"))?;
        return Ok(libraries_dir.join(rel).to_string_lossy().to_string());
    }
    Ok(s)
}

fn extract_installer_jar(
    installer_jar: &Path,
    loader_dir: &Path,
    libraries_dir: &Path,
) -> Result<(LoaderProfileOverlay, Option<ForgeInstallProfile>), String> {
    let file = File::open(installer_jar)
        .map_err(|e| format!("Failed to open installer {}: {e}", installer_jar.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Invalid installer jar {}: {e}", installer_jar.display()))?;

    let data_dir = loader_dir.join("installer_data");
    fs::create_dir_all(&data_dir)
        .map_err(|e| format!("Failed to create installer data dir: {e}"))?;

    let mut version_json_raw: Option<String> = None;
    let mut install_profile_raw: Option<String> = None;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Installer zip entry error: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if name == "version.json" {
            let mut s = String::new();
            entry
                .read_to_string(&mut s)
                .map_err(|e| format!("Failed to read version.json: {e}"))?;
            version_json_raw = Some(s);
        } else if name == "install_profile.json" {
            let mut s = String::new();
            entry
                .read_to_string(&mut s)
                .map_err(|e| format!("Failed to read install_profile.json: {e}"))?;
            install_profile_raw = Some(s);
        } else if let Some(maven_rel) = name.strip_prefix("maven/") {
            let Some(enclosed) = Path::new(maven_rel).file_name().map(|_| PathBuf::from(maven_rel))
            else {
                continue;
            };
            if enclosed
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir))
            {
                continue;
            }
            let dest = libraries_dir.join(enclosed);
            if !dest.exists() {
                if let Some(parent) = dest.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if let Ok(mut out) = File::create(&dest) {
                    let _ = io::copy(&mut entry, &mut out);
                }
            }
        } else if name.starts_with("data/") || name == "client.lzma" {
            let Some(rel) = entry.enclosed_name() else {
                continue;
            };
            let dest = data_dir.join(rel);
            if let Some(parent) = dest.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let mut out = File::create(&dest)
                .map_err(|e| format!("Failed to extract {}: {e}", dest.display()))?;
            io::copy(&mut entry, &mut out)
                .map_err(|e| format!("Failed to write {}: {e}", dest.display()))?;
        }
    }

    let overlay_str = version_json_raw
        .ok_or_else(|| "Installer jar does not contain version.json".to_string())?;
    let _ = fs::write(loader_dir.join("version.json"), &overlay_str);
    let overlay: LoaderProfileOverlay = serde_json::from_str(&overlay_str)
        .map_err(|e| format!("Failed to parse installer version.json: {e}"))?;

    let install_profile = match install_profile_raw {
        Some(raw) => {
            let _ = fs::write(loader_dir.join("install_profile.json"), &raw);
            let parsed: ForgeInstallProfile = serde_json::from_str(&raw)
                .map_err(|e| format!("Failed to parse install_profile.json: {e}"))?;
            Some(parsed)
        }
        None => None,
    };

    Ok((overlay, install_profile))
}

fn run_installer_processors(
    install_profile: &ForgeInstallProfile,
    loader_dir: &Path,
    installer_jar: &Path,
    client_jar: &Path,
    paths: &LauncherPaths,
    java_bin: &Path,
    tx: &Sender<WorkerEvent>,
) -> Result<(), String> {
    let stamp_path = loader_dir.join(".processors_done");
    let data_dir = loader_dir.join("installer_data");

    let mut vars: HashMap<String, String> = HashMap::new();
    vars.insert("SIDE".to_string(), "client".to_string());
    vars.insert(
        "MINECRAFT_JAR".to_string(),
        client_jar.to_string_lossy().to_string(),
    );
    vars.insert(
        "ROOT".to_string(),
        paths.root.to_string_lossy().to_string(),
    );
    vars.insert(
        "INSTALLER".to_string(),
        installer_jar.to_string_lossy().to_string(),
    );
    vars.insert(
        "LIBRARY_DIR".to_string(),
        paths.libraries_dir.to_string_lossy().to_string(),
    );
    for (k, side_val) in &install_profile.data {
        if let Some(raw) = side_val.client.as_deref() {
            let resolved = resolve_data_entry(raw, &data_dir, &paths.libraries_dir)?;
            vars.insert(k.clone(), resolved);
        }
    }

    let client_procs: Vec<&InstallerProcessor> = install_profile
        .processors
        .iter()
        .filter(|p| {
            p.sides
                .as_ref()
                .map_or(true, |s| s.iter().any(|side| side == "client"))
        })
        .collect();

    if client_procs.is_empty() {
        let _ = fs::write(&stamp_path, b"ok");
        return Ok(());
    }

    // Fast path: if already stamped and every expected output exists, skip re-running.
    if stamp_path.is_file() {
        let all_outputs_exist = client_procs.iter().all(|proc_item| {
            proc_item.outputs.keys().all(|k| {
                resolve_processor_arg(k, &vars, &paths.libraries_dir)
                    .map(|p| Path::new(&p).is_file())
                    .unwrap_or(false)
            })
        });
        if all_outputs_exist {
            return Ok(());
        }
    }

    let sep = if cfg!(windows) { ";" } else { ":" };
    let total = client_procs.len();
    for (idx, proc_item) in client_procs.iter().enumerate() {
        // Check SHA-1 of outputs if declared; skip processor when up to date.
        if !proc_item.outputs.is_empty() {
            let up_to_date = proc_item.outputs.iter().all(|(k, v)| {
                let Ok(out_file) = resolve_processor_arg(k, &vars, &paths.libraries_dir) else {
                    return false;
                };
                let Ok(expected_sha) = resolve_processor_arg(v, &vars, &paths.libraries_dir) else {
                    return false;
                };
                let expected_sha = expected_sha.trim_matches('\'');
                Path::new(&out_file).is_file()
                    && (expected_sha.is_empty()
                        || verify_sha1(Path::new(&out_file), expected_sha))
            });
            if up_to_date {
                continue;
            }
        }

        let _ = tx.send(WorkerEvent::Progress {
            stage: format!("Running installer processor {}/{total}...", idx + 1),
            fraction: 0.36 + 0.05 * ((idx + 1) as f32 / total as f32),
        });

        let jar_rel = maven_to_path(&proc_item.jar)
            .ok_or_else(|| format!("Invalid processor jar coord: {}", proc_item.jar))?;
        let jar_path = paths.libraries_dir.join(jar_rel);
        let main_class = read_jar_main_class(&jar_path)?;

        let mut cp_entries = vec![jar_path.to_string_lossy().to_string()];
        for cp_coord in &proc_item.classpath {
            let rel = maven_to_path(cp_coord)
                .ok_or_else(|| format!("Invalid processor classpath coord: {cp_coord}"))?;
            cp_entries.push(paths.libraries_dir.join(rel).to_string_lossy().to_string());
        }
        let cp_joined = cp_entries.join(sep);

        let mut args = Vec::with_capacity(proc_item.args.len());
        for raw_arg in &proc_item.args {
            args.push(resolve_processor_arg(raw_arg, &vars, &paths.libraries_dir)?);
        }

        let _ = tx.send(WorkerEvent::Log(format!(
            "[Installer] Running {main_class} ({}/{total})",
            idx + 1
        )));

        let mut proc_cmd = Command::new(java_bin);
        proc_cmd
            .arg("-cp")
            .arg(&cp_joined)
            .arg(&main_class)
            .args(&args);
        crate::launcher::configure_background_command(&mut proc_cmd);
        let output = proc_cmd
            .output()
            .map_err(|e| format!("Failed to spawn installer processor {main_class}: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let detail = stderr
                .lines()
                .chain(stdout.lines())
                .rev()
                .take(8)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n");
            return Err(format!(
                "Installer processor {main_class} failed (exit {:?}):\n{detail}",
                output.status.code()
            ));
        }
    }

    let _ = fs::write(&stamp_path, b"ok");
    Ok(())
}

fn resolve_effective_loader_version(
    kind: LoaderKind,
    game_version: &str,
    configured: &str,
) -> Result<String, String> {
    if !configured.is_empty() {
        return Ok(configured.to_string());
    }
    let versions = fetch_loader_versions(kind, game_version)?;
    versions
        .iter()
        .find(|v| v.stable)
        .or_else(|| versions.first())
        .map(|v| v.version.clone())
        .ok_or_else(|| format!("No {} version available for Minecraft {game_version}", kind.label()))
}

fn installer_url_for(
    client: &Client,
    kind: LoaderKind,
    game_version: &str,
    loader_version: &str,
) -> Result<String, String> {
    match kind {
        LoaderKind::Forge => {
            let exact = format!("{game_version}-{loader_version}");
            let std_url = format!(
                "https://maven.minecraftforge.net/net/minecraftforge/forge/{exact}/forge-{exact}-installer.jar"
            );
            // Fast path for standard 1.13+ coordinates; fall back to metadata lookup on legacy versions.
            if client.head(&std_url).send().is_ok_and(|r| r.status().is_success()) {
                return Ok(std_url);
            }
            let alt_suffix = format!("{game_version}-{loader_version}-{game_version}");
            let alt_url = format!(
                "https://maven.minecraftforge.net/net/minecraftforge/forge/{alt_suffix}/forge-{alt_suffix}-installer.jar"
            );
            if client.head(&alt_url).send().is_ok_and(|r| r.status().is_success()) {
                return Ok(alt_url);
            }
            Ok(std_url)
        }
        LoaderKind::NeoForge => {
            if game_version == "1.20.1" {
                Ok(format!(
                    "https://maven.neoforged.net/releases/net/neoforged/forge/1.20.1-{loader_version}/forge-1.20.1-{loader_version}-installer.jar"
                ))
            } else {
                Ok(format!(
                    "https://maven.neoforged.net/releases/net/neoforged/neoforge/{loader_version}/neoforge-{loader_version}-installer.jar"
                ))
            }
        }
        _ => Err(format!("{} does not use an installer JAR", kind.label())),
    }
}

/// Ensures the requested Minecraft version, optional mod loader (Fabric, Quilt, Forge, NeoForge),
/// libraries, natives, and assets are downloaded and verified.
pub fn prepare_version(
    paths: &LauncherPaths,
    version_id: &str,
    loader: &ModLoader,
    manifest: Option<&VersionManifest>,
    resolve_java: &dyn Fn(Option<u32>) -> Result<PathBuf, String>,
    tx: &Sender<WorkerEvent>,
) -> Result<PreparedLaunch, String> {
    let client = build_http_client()?;
    let version_dir = paths.versions_dir.join(version_id);
    fs::create_dir_all(&version_dir)
        .map_err(|e| format!("Failed to create version dir: {e}"))?;
    let json_path = version_dir.join(format!("{version_id}.json"));

    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Fetching metadata for {version_id}..."),
        fraction: 0.05,
    });

    if !json_path.exists() {
        let manifest_owned;
        let manifest_ref = match manifest {
            Some(m) => m,
            None => {
                manifest_owned = fetch_version_manifest()?;
                &manifest_owned
            }
        };
        let entry = manifest_ref
            .versions
            .iter()
            .find(|v| v.id == version_id)
            .ok_or_else(|| format!("Version '{version_id}' not found in Mojang manifest"))?;
        download_file(&client, &entry.url, &json_path, entry.sha1.as_deref())?;
    }

    let details_raw = fs::read_to_string(&json_path)
        .map_err(|e| format!("Failed to read {}: {e}", json_path.display()))?;
    let details: VersionDetails = serde_json::from_str(&details_raw)
        .map_err(|e| format!("Failed to parse {}: {e}", json_path.display()))?;

    // Check Java early so missing/incompatible runtimes are reported before big downloads.
    let required_java = details.java_version.as_ref().map(|j| j.major_version);
    let java_bin = resolve_java(required_java)?;

    // Prepare client JAR download job (batched concurrently with libraries & assets below)
    let client_jar = version_dir.join(format!("{version_id}.jar"));
    let client_job = if let Some(client_dl) = details.downloads.as_ref().and_then(|d| d.client.as_ref()) {
        Some(DownloadJob {
            url: client_dl.url.clone(),
            dest: client_jar.clone(),
            sha1: client_dl.sha1.clone(),
            size: None,
        })
    } else if !client_jar.exists() {
        return Err(format!("No client download info for {version_id}"));
    } else {
        None
    };

    // Collect Mojang libraries & natives
    let natives_dir = version_dir.join("natives");
    let mut classpath: Vec<PathBuf> = Vec::new();
    let mut vanilla_cp: Vec<(PathBuf, Option<String>)> = Vec::new();
    let mut lib_jobs: Vec<DownloadJob> = Vec::new();
    let mut native_jars: Vec<PathBuf> = Vec::new();

    collect_libraries(
        &details.libraries,
        "https://libraries.minecraft.net/",
        &paths.libraries_dir,
        &mut lib_jobs,
        &mut native_jars,
        Some(&mut vanilla_cp),
    )?;

    // Resolve and prepare mod loader (Fabric, Quilt, Forge, NeoForge).
    let mut loader_overlay: Option<LoaderProfileOverlay> = None;
    let mut pending_install_profile: Option<(ForgeInstallProfile, PathBuf, PathBuf)> = None;

    match loader {
        ModLoader::Vanilla => {}
        ModLoader::Fabric { loader_version } | ModLoader::Quilt { loader_version } => {
            let kind = loader.kind();
            let resolved_ver =
                resolve_effective_loader_version(kind, version_id, loader_version)?;
            let _ = tx.send(WorkerEvent::Progress {
                stage: format!("{} {resolved_ver}...", kind.label()),
                fraction: 0.13,
            });

            let slug = kind.modrinth_slug().unwrap_or("fabric");
            let cache_path = version_dir.join(format!("{slug}-{resolved_ver}.json"));
            let (base_api, default_maven) = match kind {
                LoaderKind::Quilt => (
                    QUILT_META_BASE,
                    "https://maven.quiltmc.org/repository/release/",
                ),
                _ => (FABRIC_META_BASE, "https://maven.fabricmc.net/"),
            };

            let overlay: LoaderProfileOverlay = if cache_path.exists() {
                let raw = fs::read_to_string(&cache_path)
                    .map_err(|e| format!("Failed to read {} cache: {e}", kind.label()))?;
                serde_json::from_str(&raw)
                    .map_err(|e| format!("Failed to parse {} cache: {e}", kind.label()))?
            } else {
                let ov =
                    fetch_meta_loader_overlay(base_api, kind.label(), version_id, &resolved_ver)?;
                if let Ok(json) = serde_json::to_string_pretty(&ov) {
                    let _ = fs::write(&cache_path, json);
                }
                ov
            };

            let mut loader_cp: Vec<(PathBuf, Option<String>)> = Vec::new();
            collect_libraries(
                &overlay.libraries,
                default_maven,
                &paths.libraries_dir,
                &mut lib_jobs,
                &mut native_jars,
                Some(&mut loader_cp),
            )?;

            let override_keys: HashSet<String> = overlay
                .libraries
                .iter()
                .filter_map(|l| maven_key(&l.name))
                .collect();
            vanilla_cp.retain(|(_, key)| key.as_ref().map_or(true, |k| !override_keys.contains(k)));
            classpath.extend(loader_cp.into_iter().map(|(p, _)| p));
            loader_overlay = Some(overlay);
        }
        ModLoader::Forge { loader_version } | ModLoader::NeoForge { loader_version } => {
            let kind = loader.kind();
            let resolved_ver =
                resolve_effective_loader_version(kind, version_id, loader_version)?;
            let slug = kind.modrinth_slug().unwrap_or("forge");
            let loader_dir = version_dir.join(format!("{slug}-{resolved_ver}"));
            fs::create_dir_all(&loader_dir)
                .map_err(|e| format!("Failed to create {} dir: {e}", kind.label()))?;

            let _ = tx.send(WorkerEvent::Progress {
                stage: format!("Downloading {} {resolved_ver} installer...", kind.label()),
                fraction: 0.13,
            });

            let installer_jar = loader_dir.join("installer.jar");
            if !installer_jar.is_file() {
                let url = installer_url_for(&client, kind, version_id, &resolved_ver)?;
                download_file(&client, &url, &installer_jar, None)?;
            }

            let (overlay, install_profile) =
                extract_installer_jar(&installer_jar, &loader_dir, &paths.libraries_dir)?;

            let default_maven = match kind {
                LoaderKind::NeoForge => "https://maven.neoforged.net/releases/",
                _ => "https://maven.minecraftforge.net/",
            };
            let mut loader_cp: Vec<(PathBuf, Option<String>)> = Vec::new();
            collect_libraries(
                &overlay.libraries,
                default_maven,
                &paths.libraries_dir,
                &mut lib_jobs,
                &mut native_jars,
                Some(&mut loader_cp),
            )?;

            if let Some(ref ip) = install_profile {
                collect_libraries(
                    &ip.libraries,
                    default_maven,
                    &paths.libraries_dir,
                    &mut lib_jobs,
                    &mut native_jars,
                    None,
                )?;
                pending_install_profile = Some((ip.clone(), loader_dir, installer_jar));
            }

            let override_keys: HashSet<String> = overlay
                .libraries
                .iter()
                .filter_map(|l| maven_key(&l.name))
                .collect();
            vanilla_cp.retain(|(_, key)| key.as_ref().map_or(true, |k| !override_keys.contains(k)));
            classpath.extend(loader_cp.into_iter().map(|(p, _)| p));
            loader_overlay = Some(overlay);
        }
    }

    classpath.extend(vanilla_cp.into_iter().map(|(p, _)| p));

    // Fetch asset index early so Vanilla/Fabric/Quilt can download client.jar, libraries, and
    // thousands of assets in a single 64-connection async batch.
    let asset_jobs: Vec<DownloadJob> = if let Some(asset_info) = &details.asset_index {
        let _ = tx.send(WorkerEvent::Progress {
            stage: format!("Asset index ({})...", asset_info.id),
            fraction: 0.14,
        });
        let index_path = paths
            .assets_dir
            .join("indexes")
            .join(format!("{}.json", asset_info.id));
        download_file(&client, &asset_info.url, &index_path, Some(&asset_info.sha1))?;

        let index_raw = fs::read_to_string(&index_path)
            .map_err(|e| format!("Failed to read asset index: {e}"))?;
        let asset_index: AssetIndex = serde_json::from_str(&index_raw)
            .map_err(|e| format!("Failed to parse asset index: {e}"))?;

        asset_index
            .objects
            .values()
            .filter(|obj| obj.hash.len() > 2)
            .map(|obj| {
                let prefix = &obj.hash[..2];
                DownloadJob {
                    url: format!("{RESOURCES_BASE}/{prefix}/{}", obj.hash),
                    dest: paths.assets_dir.join("objects").join(prefix).join(&obj.hash),
                    sha1: Some(obj.hash.clone()),
                    size: Some(obj.size),
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    if let Some((ip, loader_dir, installer_jar)) = pending_install_profile {
        if let Some(cj) = client_job {
            lib_jobs.push(cj);
        }
        let lib_total = lib_jobs.len();
        let _ = tx.send(WorkerEvent::Progress {
            stage: format!("Libraries: 0/{lib_total}"),
            fraction: 0.16,
        });
        let last_lib_report = AtomicUsize::new(0);
        download_parallel(&client, lib_jobs, 32, &|done, total| {
            let prev = last_lib_report.load(Ordering::Relaxed);
            if done == total || done >= prev + 5 {
                last_lib_report.store(done, Ordering::Relaxed);
                let _ = tx.send(WorkerEvent::Progress {
                    stage: format!("Libraries: {done}/{total}"),
                    fraction: 0.16 + 0.22 * (done as f32 / total.max(1) as f32),
                });
            }
        })?;

        run_installer_processors(
            &ip,
            &loader_dir,
            &installer_jar,
            &client_jar,
            paths,
            &java_bin,
            tx,
        )?;

        for jar in &native_jars {
            extract_natives(jar, &natives_dir)?;
        }

        if !asset_jobs.is_empty() {
            let last_report = AtomicUsize::new(0);
            download_parallel(&client, asset_jobs, 64, &|done, total| {
                let prev = last_report.load(Ordering::Relaxed);
                if done == total || done >= prev + 25 {
                    last_report.store(done, Ordering::Relaxed);
                    let _ = tx.send(WorkerEvent::Progress {
                        stage: format!("Assets: {done}/{total}"),
                        fraction: 0.48 + 0.50 * (done as f32 / total.max(1) as f32),
                    });
                }
            })?;
        }
    } else {
        let mut all_jobs = Vec::with_capacity(lib_jobs.len() + asset_jobs.len() + 1);
        if let Some(cj) = client_job {
            all_jobs.push(cj);
        }
        all_jobs.extend(lib_jobs);
        all_jobs.extend(asset_jobs);

        let last_report = AtomicUsize::new(0);
        download_parallel(&client, all_jobs, 64, &|done, total| {
            let prev = last_report.load(Ordering::Relaxed);
            if done == total || done >= prev + 25 {
                last_report.store(done, Ordering::Relaxed);
                let _ = tx.send(WorkerEvent::Progress {
                    stage: format!("Downloading: {done}/{total}"),
                    fraction: 0.16 + 0.80 * (done as f32 / total.max(1) as f32),
                });
            }
        })?;

        for jar in &native_jars {
            extract_natives(jar, &natives_dir)?;
        }
    }

    classpath.push(client_jar);

    let _ = tx.send(WorkerEvent::Progress {
        stage: "Ready to launch".to_string(),
        fraction: 1.0,
    });

    Ok(PreparedLaunch {
        version_details: details,
        loader_overlay,
        classpath,
        natives_dir,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        filter_forge_versions, filter_neoforge_versions, neoforge_mc_prefix,
        parse_manifest_main_class, parse_maven_metadata_versions, resolve_processor_arg,
    };
    use std::collections::HashMap;
    use std::path::Path;

    #[test]
    fn test_neoforge_prefix_mapping() {
        assert_eq!(neoforge_mc_prefix("1.21"), Some("21.0.".to_string()));
        assert_eq!(neoforge_mc_prefix("1.21.4"), Some("21.4.".to_string()));
        assert_eq!(neoforge_mc_prefix("1.20.4"), Some("20.4.".to_string()));
        assert_eq!(neoforge_mc_prefix("24w45a"), None);
    }

    #[test]
    fn test_maven_metadata_parsing_and_filtering() {
        let xml = r#"<metadata><versioning><versions>
            <version>1.20.1-47.2.0</version>
            <version>1.20.1-47.3.0</version>
            <version>1.21.4-54.1.0</version>
        </versions></versioning></metadata>"#;
        let all = parse_maven_metadata_versions(xml);
        assert_eq!(all.len(), 3);

        let forge = filter_forge_versions(&all, "1.20.1", Some("47.2.0"));
        assert_eq!(forge.len(), 2);
        assert_eq!(forge[0].version, "47.2.0");
        assert!(forge[0].stable);
        assert_eq!(forge[1].version, "47.3.0");
        assert!(!forge[1].stable);

        let nf_all = vec![
            "1.20.1-47.1.106".to_string(),
            "21.4.80-beta".to_string(),
            "21.4.120".to_string(),
        ];
        let nf_1214 = filter_neoforge_versions(&nf_all, "1.21.4");
        assert_eq!(nf_1214.len(), 2);
        assert_eq!(nf_1214[0].version, "21.4.120");
        assert!(nf_1214[0].stable);
        assert_eq!(nf_1214[1].version, "21.4.80-beta");
        assert!(!nf_1214[1].stable);

        let nf_legacy = filter_neoforge_versions(&nf_all, "1.20.1");
        assert_eq!(nf_legacy.len(), 1);
        assert_eq!(nf_legacy[0].version, "47.1.106");
    }

    #[test]
    fn test_manifest_main_class_and_processor_args() {
        let mf = "Manifest-Version: 1.0\r\nMain-Class: net.minecraftforge.installertools.\r\n ConsoleTool\r\nImplementation-Version: 1.0\r\n";
        assert_eq!(
            parse_manifest_main_class(mf).as_deref(),
            Some("net.minecraftforge.installertools.ConsoleTool")
        );

        let mut vars = HashMap::new();
        vars.insert("PATCHED".to_string(), "/libs/patched.jar".to_string());
        let libs = Path::new("/tmp/libs");
        assert_eq!(
            resolve_processor_arg("{PATCHED}", &vars, libs).unwrap(),
            "/libs/patched.jar"
        );
        assert_eq!(
            resolve_processor_arg("[com.example:helper:1.2]", &vars, libs).unwrap(),
            libs.join("com/example/helper/1.2/helper-1.2.jar")
                .to_string_lossy()
        );
    }
}

