use crate::models::{
    DedicatedServer, IconImage, InstalledProject, InstanceLogFile, LauncherPaths, LoaderKind,
    LogFileKind, ModLoader, ModrinthInstallOutcome, ModrinthProjectType, ModrinthSearchHit,
    ModrinthSearchPage, ModrinthSearchResponse, ModrinthSort, ModrinthVersion, MrpackExportOptions,
    MrpackIndex, Profile, ProjectSource, WorkerEvent,
};
use crate::mojang::{
    build_async_http_client, build_http_client, check_not_cancelled, download_file,
};
use crate::servers::server_supports_plugins_live;
use crate::storage::{
    atomic_write, ensure_instance_mods_dir, instance_dir, server_dir, unique_instance_dir_name,
};
use reqwest::blocking::Client;
use reqwest::StatusCode;
use sha1::{Digest as _, Sha1};
use sha2::Sha512;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

const MODRINTH_API_BASE: &str = "https://api.modrinth.com/v2";
pub const SEARCH_PAGE_SIZE: usize = 20;
const INSTALLED_INDEX_FILE: &str = ".mc-launcher-modrinth.json";

#[derive(Clone)]
pub struct SearchParams {
    pub query: String,
    pub project_type: ModrinthProjectType,
    pub sort: ModrinthSort,
    pub game_version: Option<String>,
    pub loader_filter: Option<LoaderKind>,
    pub server_software: Option<crate::models::ServerSoftware>,
    pub offset: usize,
}

fn icon_http_client() -> Result<&'static Client, String> {
    static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();
    match CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent(concat!("sunset-launcher/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .pool_max_idle_per_host(8)
            .build()
            .map_err(|e| format!("Failed to create icon HTTP client: {e}"))
    }) {
        Ok(c) => Ok(c),
        Err(e) => Err(e.clone()),
    }
}

/// Builds Modrinth search facets scoped to the active instance's MC version and loader.
/// Returns `None` when searching mods on a Vanilla instance (which does not support mods).
pub fn build_search_facets(params: &SearchParams) -> Option<Vec<Vec<String>>> {
    if params.project_type == ModrinthProjectType::Mod
        && (params.loader_filter == Some(LoaderKind::Vanilla)
            || params.server_software.is_some_and(|sw| !sw.supports_mods()))
    {
        return None;
    }

    let mut facets: Vec<Vec<String>> = if params.project_type == ModrinthProjectType::Datapack {
        vec![
            vec![
                "project_type:datapack".to_string(),
                "project_type:mod".to_string(),
            ],
            vec!["categories:datapack".to_string()],
        ]
    } else {
        vec![vec![format!(
            "project_type:{}",
            params.project_type.api_str()
        )]]
    };
    if let Some(ver) = params
        .game_version
        .as_deref()
        .filter(|v| !v.trim().is_empty())
    {
        facets.push(vec![format!("versions:{}", ver.trim())]);
    }
    if params.server_software.is_some() {
        facets.push(vec![
            "server_side:required".to_string(),
            "server_side:optional".to_string(),
        ]);
    }

    if params.project_type == ModrinthProjectType::Mod
        || params.project_type == ModrinthProjectType::Modpack
    {
        match params.loader_filter {
            Some(loader) => {
                if let Some(slug) = loader.modrinth_slug() {
                    facets.push(vec![format!("categories:{slug}")]);
                } else if params.project_type == ModrinthProjectType::Modpack {
                    facets.push(vec![
                        "categories:fabric".to_string(),
                        "categories:quilt".to_string(),
                        "categories:forge".to_string(),
                        "categories:neoforge".to_string(),
                    ]);
                }
            }
            None if params.project_type == ModrinthProjectType::Modpack => {
                facets.push(vec![
                    "categories:fabric".to_string(),
                    "categories:quilt".to_string(),
                    "categories:forge".to_string(),
                    "categories:neoforge".to_string(),
                ]);
            }
            None => {}
        }
    } else if params.project_type == ModrinthProjectType::Plugin {
        let plugin_cats: Vec<String> = if let Some(sw) = params.server_software {
            sw.modrinth_plugin_loaders()
                .iter()
                .map(|l| format!("categories:{l}"))
                .collect()
        } else {
            vec![
                "categories:paper".to_string(),
                "categories:purpur".to_string(),
                "categories:folia".to_string(),
                "categories:spigot".to_string(),
                "categories:bukkit".to_string(),
            ]
        };
        facets.push(plugin_cats);
    }

    Some(facets)
}

fn modrinth_runtime() -> Result<&'static tokio::runtime::Runtime, String> {
    static RT: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();
    match RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to start Modrinth async runtime: {e}"))
    }) {
        Ok(rt) => Ok(rt),
        Err(e) => Err(e.clone()),
    }
}

fn modrinth_async_client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    match CLIENT.get_or_init(build_async_http_client) {
        Ok(c) => Ok(c),
        Err(e) => Err(e.clone()),
    }
}

fn compat_cache() -> &'static Mutex<HashMap<(String, String, String), bool>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, String, String), bool>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Verifies via Modrinth's `/version` endpoint that each candidate project has at least one
/// actual build matching BOTH `game_version` and `loader_slug` on the same file/release
/// (Modrinth's `/search` facets evaluate versions and loaders independently across all releases).
fn filter_hits_by_version_loader(
    hits: Vec<ModrinthSearchHit>,
    game_version: &str,
    loader_slug: &str,
) -> Vec<ModrinthSearchHit> {
    if hits.is_empty() {
        return hits;
    }

    let mut cached: HashMap<String, bool> = HashMap::with_capacity(hits.len());
    let mut missing: Vec<String> = Vec::new();
    if let Ok(guard) = compat_cache().lock() {
        for hit in &hits {
            let key = (
                hit.project_id.clone(),
                game_version.to_string(),
                loader_slug.to_string(),
            );
            if let Some(&ok) = guard.get(&key) {
                cached.insert(hit.project_id.clone(), ok);
            } else {
                missing.push(hit.project_id.clone());
            }
        }
    }

    if !missing.is_empty() {
        if let (Ok(rt), Ok(client)) = (modrinth_runtime(), modrinth_async_client()) {
            let gv_param = serde_json::to_string(&[game_version]).unwrap_or_default();
            let loader_param = serde_json::to_string(&[loader_slug]).unwrap_or_default();
            let client = client.clone();

            let checked: Vec<(String, Option<bool>)> = rt.block_on(async move {
                let sem = Arc::new(tokio::sync::Semaphore::new(12));
                let mut set = tokio::task::JoinSet::new();
                for project_id in missing {
                    let permit = sem.clone().acquire_owned().await;
                    let client = client.clone();
                    let gv_param = gv_param.clone();
                    let loader_param = loader_param.clone();
                    set.spawn(async move {
                        let _permit = permit;
                        let url = format!("{MODRINTH_API_BASE}/project/{project_id}/version");
                        for attempt in 0..2 {
                            let res = client
                                .get(&url)
                                .query(&[
                                    ("game_versions", gv_param.as_str()),
                                    ("loaders", loader_param.as_str()),
                                ])
                                .send()
                                .await;
                            match res {
                                Ok(resp) if resp.status().is_success() => {
                                    let list = resp.json::<Vec<serde::de::IgnoredAny>>().await.ok();
                                    return (project_id, list.map(|v| !v.is_empty()));
                                }
                                Ok(resp)
                                    if resp.status() == StatusCode::TOO_MANY_REQUESTS
                                        && attempt == 0 =>
                                {
                                    tokio::time::sleep(Duration::from_millis(200)).await;
                                }
                                _ => break,
                            }
                        }
                        (project_id, None)
                    });
                }
                let mut out = Vec::new();
                while let Some(joined) = set.join_next().await {
                    if let Ok(item) = joined {
                        out.push(item);
                    }
                }
                out
            });

            if let Ok(mut guard) = compat_cache().lock() {
                for (project_id, result) in checked {
                    if let Some(ok) = result {
                        guard.insert(
                            (
                                project_id.clone(),
                                game_version.to_string(),
                                loader_slug.to_string(),
                            ),
                            ok,
                        );
                        cached.insert(project_id, ok);
                    }
                }
            }
        }
    }

    hits.into_iter()
        .filter(|h| cached.get(&h.project_id).copied().unwrap_or(false))
        .collect()
}

/// Searches Modrinth projects by query, type, and optional MC version/loader filters.
pub fn search_projects(params: &SearchParams) -> Result<ModrinthSearchPage, String> {
    let Some(facets) = build_search_facets(params) else {
        return Ok(ModrinthSearchPage {
            hits: Vec::new(),
            total_hits: 0,
            next_offset: 0,
            has_more: false,
        });
    };

    let client = build_http_client()?;

    let facets_json = serde_json::to_string(&facets)
        .map_err(|e| format!("Failed to serialize Modrinth facets: {e}"))?;
    let limit = SEARCH_PAGE_SIZE.to_string();

    let strict_pair = matches!(
        params.project_type,
        ModrinthProjectType::Mod | ModrinthProjectType::Modpack
    )
    .then(|| {
        params
            .game_version
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .zip(params.loader_filter.and_then(|l| l.modrinth_slug()))
    })
    .flatten();

    let mut cur_offset = params.offset;
    let mut collected = Vec::new();
    let mut total_hits = 0_u64;
    let max_batches = if strict_pair.is_some() { 2 } else { 1 };

    for _ in 0..max_batches {
        let offset = cur_offset.to_string();
        let resp: ModrinthSearchResponse = client
            .get(format!("{MODRINTH_API_BASE}/search"))
            .query(&[
                ("query", params.query.trim()),
                ("facets", facets_json.as_str()),
                ("index", params.sort.api_str()),
                ("limit", limit.as_str()),
                ("offset", offset.as_str()),
            ])
            .send()
            .map_err(|e| format!("Modrinth unavailable: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Modrinth search HTTP error: {e}"))?
            .json()
            .map_err(|e| format!("Modrinth search parse error: {e}"))?;

        total_hits = resp.total_hits;
        let raw_len = resp.hits.len();
        cur_offset += raw_len;

        let verified = if let Some((ver, loader_slug)) = strict_pair {
            filter_hits_by_version_loader(resp.hits, ver, loader_slug)
        } else {
            resp.hits
        };
        collected.extend(verified);

        if raw_len < SEARCH_PAGE_SIZE || (cur_offset as u64) >= total_hits || collected.len() >= 10
        {
            break;
        }
    }

    Ok(ModrinthSearchPage {
        hits: collected,
        total_hits,
        next_offset: cur_offset,
        has_more: (cur_offset as u64) < total_hits,
    })
}

/// Decodes raw image bytes (PNG, JPEG, WebP, GIF, ICO, BMP) into a 64x64 RGBA thumbnail.
pub fn decode_icon_bytes(bytes: &[u8]) -> Result<IconImage, String> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| format!("decode: {e}"))?
        .thumbnail(64, 64)
        .to_rgba8();
    Ok(IconImage {
        width: img.width() as usize,
        height: img.height() as usize,
        rgba: img.into_raw(),
    })
}

/// Downloads a project icon (with keep-alive pooling & rate-limit retry) and decodes it to RGBA.
pub fn fetch_icon(url: &str) -> Result<IconImage, String> {
    let client = icon_http_client()?;
    let mut last_err = String::new();
    for attempt in 0..3 {
        match client.get(url).send() {
            Ok(resp) => {
                let status = resp.status();
                if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
                    last_err = format!("HTTP {status}");
                    std::thread::sleep(Duration::from_millis(250 * (attempt + 1)));
                    continue;
                }
                if !status.is_success() {
                    return Err(format!("HTTP {status}"));
                }
                let bytes = resp.bytes().map_err(|e| e.to_string())?;
                return decode_icon_bytes(&bytes);
            }
            Err(e) => {
                last_err = e.to_string();
                std::thread::sleep(Duration::from_millis(200 * (attempt + 1)));
            }
        }
    }
    Err(last_err)
}

fn get_versions(
    client: &Client,
    project_id: &str,
    game_version: Option<&str>,
    loaders: Option<&str>,
) -> Result<Vec<ModrinthVersion>, String> {
    let mut req = client.get(format!("{MODRINTH_API_BASE}/project/{project_id}/version"));
    if let Some(gv) = game_version.filter(|v| !v.is_empty()) {
        let param = serde_json::to_string(&[gv]).unwrap_or_default();
        req = req.query(&[("game_versions", param)]);
    }
    if let Some(l) = loaders {
        req = req.query(&[("loaders", l)]);
    }
    req.send()
        .map_err(|e| format!("Modrinth version request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Modrinth version HTTP error: {e}"))?
        .json()
        .map_err(|e| format!("Modrinth version parse error: {e}"))
}

/// Picks the newest version compatible with the profile. Mods must match the profile loader;
/// resource packs may fall back to any version.
fn fetch_best_version(
    client: &Client,
    project_id: &str,
    project_type: ModrinthProjectType,
    game_version: &str,
    loader: &ModLoader,
) -> Result<ModrinthVersion, String> {
    match project_type {
        ModrinthProjectType::Mod => {
            let slug = loader.modrinth_slug().ok_or_else(|| {
                "Mods require a mod loader (Fabric, Quilt, Forge, or NeoForge).".to_string()
            })?;
            let param = format!("[\"{slug}\"]");
            if let Some(v) = get_versions(client, project_id, Some(game_version), Some(&param))?
                .into_iter()
                .next()
            {
                return Ok(v);
            }
            // Quilt often supports Fabric mods cleanly via QFAPI; fall back if no native Quilt build exists.
            if matches!(loader, ModLoader::Quilt { .. }) {
                if let Some(v) =
                    get_versions(client, project_id, Some(game_version), Some("[\"fabric\"]"))?
                        .into_iter()
                        .next()
                {
                    return Ok(v);
                }
            }
            Err(format!(
                "No compatible version of this mod for Minecraft {game_version} + {}.",
                loader.short_name()
            ))
        }
        ModrinthProjectType::Plugin => {
            let loaders = "[\"paper\",\"purpur\",\"folia\",\"spigot\",\"bukkit\"]";
            if let Some(v) = get_versions(client, project_id, Some(game_version), Some(loaders))?
                .into_iter()
                .next()
            {
                return Ok(v);
            }
            get_versions(client, project_id, None, Some(loaders))?
                .into_iter()
                .next()
                .ok_or_else(|| {
                    format!("No compatible plugin version found for Minecraft {game_version}.")
                })
        }
        ModrinthProjectType::Datapack => {
            if let Some(v) = get_versions(
                client,
                project_id,
                Some(game_version),
                Some("[\"datapack\"]"),
            )?
            .into_iter()
            .next()
            {
                return Ok(v);
            }
            get_versions(client, project_id, None, Some("[\"datapack\"]"))?
                .into_iter()
                .next()
                .ok_or_else(|| {
                    format!("No compatible datapack version found for Minecraft {game_version}.")
                })
        }
        ModrinthProjectType::Resourcepack | ModrinthProjectType::Shader => {
            if let Some(v) = get_versions(client, project_id, Some(game_version), None)?
                .into_iter()
                .next()
            {
                return Ok(v);
            }
            get_versions(client, project_id, None, None)?
                .into_iter()
                .next()
                .ok_or_else(|| "No files available for this project on Modrinth.".to_string())
        }
        ModrinthProjectType::Modpack => {
            let preferred = loader.modrinth_slug().map(|s| format!("[\"{s}\"]"));
            if let Some(ref p) = preferred {
                if let Some(v) = get_versions(client, project_id, None, Some(p))?
                    .into_iter()
                    .next()
                {
                    return Ok(v);
                }
            }
            if let Some(v) = get_versions(
                client,
                project_id,
                None,
                Some("[\"fabric\",\"neoforge\",\"forge\",\"quilt\"]"),
            )?
            .into_iter()
            .next()
            {
                return Ok(v);
            }
            get_versions(client, project_id, None, None)?
                .into_iter()
                .next()
                .ok_or_else(|| "This modpack has no compatible versions on Modrinth.".to_string())
        }
    }
}

fn index_path(paths: &LauncherPaths, profile: &Profile) -> PathBuf {
    instance_dir(paths, profile).join(INSTALLED_INDEX_FILE)
}

/// Loads `project_id -> InstalledProject` for an instance, dropping entries whose file is gone.
/// Recognizes both enabled (`foo.jar`) and temporarily disabled (`foo.jar.disabled`) files.
pub fn load_installed_index(
    paths: &LauncherPaths,
    profile: &Profile,
) -> HashMap<String, InstalledProject> {
    let inst = instance_dir(paths, profile);
    let mut map: HashMap<String, InstalledProject> = fs::read_to_string(index_path(paths, profile))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    map.retain(|_, p| {
        let dir = inst.join(&p.folder);
        dir.join(&p.filename).is_file() || dir.join(format!("{}.disabled", p.filename)).is_file()
    });
    map
}

pub(crate) fn save_installed_index(
    paths: &LauncherPaths,
    profile: &Profile,
    map: &HashMap<String, InstalledProject>,
) {
    if let Ok(json) = serde_json::to_string_pretty(map) {
        let _ = atomic_write(&index_path(paths, profile), json);
    }
}

pub(crate) fn default_datapack_rel_folder(inst_dir: &Path) -> String {
    let worlds = list_worlds(inst_dir);
    if let Some(first_world) = worlds.first() {
        format!("saves/{}/datapacks", first_world.folder_name)
    } else {
        "datapacks".to_string()
    }
}

/// Downloads and installs a Modrinth mod, resourcepack, shader, datapack, or modpack (.mrpack).
pub fn install_project(
    paths: &LauncherPaths,
    profile: &Profile,
    project_id: &str,
    project_type: ModrinthProjectType,
    title: &str,
    icon_url: Option<&str>,
    tx: &Sender<WorkerEvent>,
) -> Result<ModrinthInstallOutcome, String> {
    check_not_cancelled()?;
    let client = build_http_client()?;
    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Finding version for {title}..."),
        fraction: 0.1,
    });

    match project_type {
        ModrinthProjectType::Mod
        | ModrinthProjectType::Resourcepack
        | ModrinthProjectType::Shader
        | ModrinthProjectType::Datapack => {
            if project_type == ModrinthProjectType::Mod && !profile.loader.is_modded() {
                return Err("Mods require a modded loader (Fabric, Quilt, Forge, or NeoForge). Switch loader in Instance settings.".to_string());
            }
            let (_, dest_folder, _) = ensure_instance_content_dir(paths, profile, project_type)?;

            let mut index = load_installed_index(paths, profile);
            let filename = install_single(
                &client,
                paths,
                profile,
                project_id,
                project_type,
                title,
                icon_url.map(str::to_string),
                &mut index,
                tx,
            )?;

            // Resolve required dependencies (e.g. Fabric API, QSL, Kotlin) breadth-first.
            let mut deps_installed = Vec::new();
            if project_type == ModrinthProjectType::Mod {
                install_required_deps(
                    &client,
                    paths,
                    profile,
                    project_id,
                    &mut index,
                    &mut deps_installed,
                    tx,
                )?;
            }
            save_installed_index(paths, profile, &index);

            let _ = tx.send(WorkerEvent::Progress {
                stage: format!("Installed: {filename}"),
                fraction: 1.0,
            });
            Ok(ModrinthInstallOutcome::FileInstalled {
                title: title.to_string(),
                filename,
                dest_folder,
                dependencies_installed: deps_installed,
            })
        }
        ModrinthProjectType::Plugin => {
            Err("Plugins can only be installed to Dedicated Servers.".to_string())
        }
        ModrinthProjectType::Modpack => {
            let version = fetch_best_version(
                &client,
                project_id,
                project_type,
                &profile.version_id,
                &profile.loader,
            )?;
            install_modpack_version(&client, paths, profile, project_id, &version, tx)
        }
    }
}

fn install_required_deps(
    client: &Client,
    paths: &LauncherPaths,
    profile: &Profile,
    root_project_id: &str,
    index: &mut HashMap<String, InstalledProject>,
    deps_installed: &mut Vec<String>,
    tx: &Sender<WorkerEvent>,
) -> Result<(), String> {
    let mut visited: HashSet<String> = HashSet::from([root_project_id.to_string()]);
    let mut queue = required_dependencies(
        client,
        root_project_id,
        &profile.version_id,
        &profile.loader,
    )?;
    while let Some(dep_id) = queue.pop() {
        if !visited.insert(dep_id.clone()) || index.contains_key(&dep_id) {
            continue;
        }
        let (dep_title, dep_icon) =
            project_meta(client, &dep_id).unwrap_or_else(|| (dep_id.clone(), None));
        let _ = tx.send(WorkerEvent::Progress {
            stage: format!("Dependency: {dep_title}..."),
            fraction: 0.7,
        });
        install_single(
            client,
            paths,
            profile,
            &dep_id,
            ModrinthProjectType::Mod,
            &dep_title,
            dep_icon,
            index,
            tx,
        )?;
        deps_installed.push(dep_title);
        queue.extend(required_dependencies(
            client,
            &dep_id,
            &profile.version_id,
            &profile.loader,
        )?);
    }
    Ok(())
}

fn install_modpack_version(
    client: &Client,
    paths: &LauncherPaths,
    profile: &Profile,
    project_id: &str,
    version: &ModrinthVersion,
    tx: &Sender<WorkerEvent>,
) -> Result<ModrinthInstallOutcome, String> {
    let file = primary_file(version)?;
    let tmp_mrpack = paths.root.join(format!("{project_id}.mrpack"));
    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Downloading modpack {}...", file.filename),
        fraction: 0.15,
    });
    download_file(client, &file.url, &tmp_mrpack, file.hashes.sha1.as_deref())?;
    let res = install_mrpack(paths, &tmp_mrpack, profile.username.clone(), tx);
    let _ = fs::remove_file(&tmp_mrpack);
    res
}

pub(crate) fn folder_for(project_type: ModrinthProjectType) -> &'static str {
    match project_type {
        ModrinthProjectType::Plugin => "plugins",
        ModrinthProjectType::Resourcepack => "resourcepacks",
        ModrinthProjectType::Shader => "shaderpacks",
        ModrinthProjectType::Datapack => "datapacks",
        _ => "mods",
    }
}

pub(crate) fn sanitize_mod_filename(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains('/')
        || trimmed.contains('\\')
    {
        return Err(format!("Invalid filename: {raw}"));
    }

    let mapped: String = trimmed
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = mapped.trim_end_matches([' ', '.']);
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        return Err(format!("Invalid filename: {raw}"));
    }

    let stem = cleaned.split('.').next().unwrap_or("");
    let is_windows_reserved = matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if is_windows_reserved {
        Ok(format!("_{cleaned}"))
    } else {
        Ok(cleaned.to_string())
    }
}

pub(crate) fn ensure_instance_content_dir(
    paths: &LauncherPaths,
    profile: &Profile,
    project_type: ModrinthProjectType,
) -> Result<(PathBuf, String, PathBuf), String> {
    let inst = instance_dir(paths, profile);
    if project_type == ModrinthProjectType::Mod || profile.loader.is_modded() {
        ensure_instance_mods_dir(paths, profile)?;
    }
    let folder_rel = if project_type == ModrinthProjectType::Datapack {
        default_datapack_rel_folder(&inst)
    } else {
        folder_for(project_type).to_string()
    };
    let dest_dir = inst.join(&folder_rel);
    fs::create_dir_all(&dest_dir).map_err(|e| {
        format!(
            "Failed to create {folder_rel} directory {}: {e}",
            dest_dir.display()
        )
    })?;
    Ok((inst, folder_rel, dest_dir))
}

/// Loads `project_id -> InstalledProject` for a dedicated server, dropping entries whose file is gone.
pub fn load_server_installed_index(
    paths: &LauncherPaths,
    server: &DedicatedServer,
) -> HashMap<String, InstalledProject> {
    let srv_dir = server_dir(paths, server);
    let mut map: HashMap<String, InstalledProject> =
        fs::read_to_string(srv_dir.join(INSTALLED_INDEX_FILE))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
    map.retain(|_, p| {
        let dir = srv_dir.join(&p.folder);
        dir.join(&p.filename).is_file() || dir.join(format!("{}.disabled", p.filename)).is_file()
    });
    map
}

pub(crate) fn save_server_installed_index(
    paths: &LauncherPaths,
    server: &DedicatedServer,
    map: &HashMap<String, InstalledProject>,
) {
    let srv_dir = server_dir(paths, server);
    if let Ok(json) = serde_json::to_string_pretty(map) {
        let _ = atomic_write(&srv_dir.join(INSTALLED_INDEX_FILE), json);
    }
}

fn ensure_project_supports_server(
    client: &Client,
    project_id: &str,
    title: &str,
) -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct ProjectSideInfo {
        #[serde(default)]
        server_side: String,
    }
    if let Ok(resp) = client
        .get(format!("{MODRINTH_API_BASE}/project/{project_id}"))
        .send()
        .and_then(|r| r.error_for_status())
    {
        if let Ok(info) = resp.json::<ProjectSideInfo>() {
            if info.server_side.eq_ignore_ascii_case("unsupported") {
                return Err(format!(
                    "'{title}' is a client-only mod and cannot be installed on a dedicated server."
                ));
            }
        }
    }
    Ok(())
}

/// Installs a Modrinth plugin or mod directly into a dedicated server's `plugins/` or `mods/` folder.
pub fn install_server_project(
    paths: &LauncherPaths,
    server: &DedicatedServer,
    project_id: &str,
    project_type: ModrinthProjectType,
    title: &str,
    icon_url: Option<&str>,
    tx: &Sender<WorkerEvent>,
) -> Result<ModrinthInstallOutcome, String> {
    check_not_cancelled()?;
    let srv_dir = server_dir(paths, server);
    let client = build_http_client()?;
    ensure_project_supports_server(&client, project_id, title)?;
    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Finding version for {title}..."),
        fraction: 0.15,
    });

    let (dest_folder, version) = match project_type {
        ModrinthProjectType::Plugin => {
            if !server_supports_plugins_live(&srv_dir, server.software) {
                return Err(format!(
                    "{} server does not support plugins (install a plugin-bridge mod or use Paper/Purpur/Folia/Arclight).",
                    server.software.label()
                ));
            }
            let loaders_json = format!(
                "[{}]",
                server
                    .software
                    .modrinth_plugin_loaders()
                    .iter()
                    .map(|l| format!("\"{l}\""))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let ver = get_versions(
                &client,
                project_id,
                Some(&server.mc_version),
                Some(&loaders_json),
            )?
            .into_iter()
            .next()
            .or_else(|| {
                get_versions(
                    &client,
                    project_id,
                    Some(&server.mc_version),
                    Some("[\"paper\",\"purpur\",\"folia\",\"spigot\",\"bukkit\"]"),
                )
                .ok()
                .and_then(|v| v.into_iter().next())
            })
            .or_else(|| {
                get_versions(
                    &client,
                    project_id,
                    None,
                    Some("[\"paper\",\"purpur\",\"folia\",\"spigot\",\"bukkit\"]"),
                )
                .ok()
                .and_then(|v| v.into_iter().next())
            })
            .ok_or_else(|| {
                format!(
                    "No compatible plugin version of '{title}' found for Minecraft {}.",
                    server.mc_version
                )
            })?;
            ("plugins", ver)
        }
        ModrinthProjectType::Mod => {
            if !server.software.supports_mods() {
                return Err(format!(
                    "{} server does not support mods (use Fabric, Quilt, Forge, NeoForge, or Arclight).",
                    server.software.label()
                ));
            }
            let loaders_json = if server.software == crate::models::ServerSoftware::Arclight {
                "[\"neoforge\",\"forge\",\"fabric\"]".to_string()
            } else if let Some(kind) = server.software.modrinth_mod_loader() {
                let slug = kind.modrinth_slug().unwrap_or("fabric");
                format!("[\"{slug}\"]")
            } else {
                "[\"fabric\"]".to_string()
            };
            let ver = get_versions(
                &client,
                project_id,
                Some(&server.mc_version),
                Some(&loaders_json),
            )?
            .into_iter()
            .next()
            .ok_or_else(|| {
                format!(
                    "No compatible mod version of '{title}' found for {} {}.",
                    server.software.label(),
                    server.mc_version
                )
            })?;
            ("mods", ver)
        }
        _ => {
            return Err(
                "Only plugins and mods can be installed to a dedicated server.".to_string(),
            );
        }
    };

    let file = primary_file(&version)?;
    let safe_name = sanitize_mod_filename(&file.filename)?;
    let dest_dir = srv_dir.join(dest_folder);
    fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Failed to create {}: {e}", dest_dir.display()))?;
    let dest_path = dest_dir.join(&safe_name);

    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Downloading {safe_name}..."),
        fraction: 0.6,
    });
    download_file(&client, &file.url, &dest_path, file.hashes.sha1.as_deref())?;

    let mut index = load_server_installed_index(paths, server);
    index.insert(
        project_id.to_string(),
        InstalledProject {
            title: title.to_string(),
            filename: safe_name.clone(),
            folder: dest_folder.to_string(),
            version_number: version.version_number.clone(),
            icon_url: icon_url.map(str::to_string),
            source: ProjectSource::Modrinth,
        },
    );
    save_server_installed_index(paths, server, &index);

    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Installed: {safe_name}"),
        fraction: 1.0,
    });

    Ok(ModrinthInstallOutcome::FileInstalled {
        title: title.to_string(),
        filename: safe_name,
        dest_folder: dest_folder.to_string(),
        dependencies_installed: Vec::new(),
    })
}

/// Resolves Modrinth metadata (title, version, icon_url) by SHA-1 for any `.jar` files
/// in a dedicated server's `plugins/` and `mods/` folders that aren't indexed yet.
pub fn backfill_server_installed_index(paths: &LauncherPaths, server: &DedicatedServer) {
    use sha1::{Digest, Sha1};

    let srv_dir = server_dir(paths, server);
    let mut index = load_server_installed_index(paths, server);
    let indexed_files: HashSet<String> = index.values().map(|p| p.filename.clone()).collect();

    let mut hash_to_meta: HashMap<String, (String, String)> = HashMap::new();
    for folder in ["plugins", "mods"] {
        let dir = srv_dir.join(folder);
        for item in list_content_items(&dir) {
            if indexed_files.contains(&item.base_name) {
                continue;
            }
            let path = dir.join(&item.disk_name);
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            let mut hasher = Sha1::new();
            hasher.update(&bytes);
            let sha1_hex = hex::encode(hasher.finalize());
            hash_to_meta.insert(sha1_hex, (folder.to_string(), item.base_name));
        }
    }
    if hash_to_meta.is_empty() {
        return;
    }

    #[derive(serde::Deserialize)]
    struct VersionFileMatch {
        project_id: String,
        version_number: String,
    }

    let Ok(client) = build_http_client() else {
        return;
    };
    let hashes: Vec<String> = hash_to_meta.keys().cloned().collect();
    let body = serde_json::json!({
        "hashes": hashes,
        "algorithm": "sha1"
    });
    let Ok(resp) = client
        .post("https://api.modrinth.com/v2/version_files")
        .json(&body)
        .send()
        .and_then(|r| r.error_for_status())
    else {
        return;
    };
    let Ok(versions_map) = resp.json::<HashMap<String, VersionFileMatch>>() else {
        return;
    };
    if versions_map.is_empty() {
        return;
    }

    #[derive(serde::Deserialize)]
    struct ProjectBrief {
        id: String,
        title: String,
        #[serde(default)]
        icon_url: Option<String>,
    }

    let project_ids: Vec<String> = versions_map
        .values()
        .map(|v| v.project_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let ids_param = serde_json::to_string(&project_ids).unwrap_or_else(|_| "[]".to_string());
    let Ok(proj_resp) = client
        .get("https://api.modrinth.com/v2/projects")
        .query(&[("ids", ids_param.as_str())])
        .send()
        .and_then(|r| r.error_for_status())
    else {
        return;
    };
    let Ok(projects) = proj_resp.json::<Vec<ProjectBrief>>() else {
        return;
    };
    let proj_by_id: HashMap<String, ProjectBrief> =
        projects.into_iter().map(|p| (p.id.clone(), p)).collect();

    let mut changed = false;
    for (hash, ver) in versions_map {
        let Some((folder, filename)) = hash_to_meta.get(&hash) else {
            continue;
        };
        let proj = proj_by_id.get(&ver.project_id);
        let title = proj
            .map(|p| p.title.clone())
            .unwrap_or_else(|| filename.clone());
        let icon_url = proj.and_then(|p| p.icon_url.clone());
        index.insert(
            ver.project_id.clone(),
            InstalledProject {
                title,
                filename: filename.clone(),
                folder: folder.clone(),
                version_number: ver.version_number,
                icon_url,
                source: ProjectSource::Modrinth,
            },
        );
        changed = true;
    }
    if changed {
        save_server_installed_index(paths, server, &index);
    }
}

/// Installs a specific Modrinth version into a dedicated server's `plugins/` or `mods/` folder.
#[allow(clippy::too_many_arguments)]
pub fn install_server_specific_version(
    paths: &LauncherPaths,
    server: &DedicatedServer,
    project_id: &str,
    project_type: ModrinthProjectType,
    title: &str,
    icon_url: Option<&str>,
    version: &ModrinthVersion,
    tx: &Sender<WorkerEvent>,
) -> Result<ModrinthInstallOutcome, String> {
    check_not_cancelled()?;
    let srv_dir = server_dir(paths, server);
    let dest_folder = match project_type {
        ModrinthProjectType::Plugin => {
            if !server_supports_plugins_live(&srv_dir, server.software) {
                return Err(format!(
                    "{} server does not support plugins.",
                    server.software.label()
                ));
            }
            "plugins"
        }
        ModrinthProjectType::Mod => {
            if !server.software.supports_mods() {
                return Err(format!(
                    "{} server does not support mods.",
                    server.software.label()
                ));
            }
            "mods"
        }
        _ => {
            return Err(
                "Only plugins and mods can be installed to a dedicated server.".to_string(),
            );
        }
    };

    let client = build_http_client()?;
    ensure_project_supports_server(&client, project_id, title)?;
    let file = primary_file(version)?;
    let safe_name = sanitize_mod_filename(&file.filename)?;
    let dest_dir = srv_dir.join(dest_folder);
    fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Failed to create {}: {e}", dest_dir.display()))?;
    let dest_path = dest_dir.join(&safe_name);

    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Downloading {safe_name}..."),
        fraction: 0.6,
    });
    download_file(&client, &file.url, &dest_path, file.hashes.sha1.as_deref())?;

    let mut index = load_server_installed_index(paths, server);
    index.insert(
        project_id.to_string(),
        InstalledProject {
            title: title.to_string(),
            filename: safe_name.clone(),
            folder: dest_folder.to_string(),
            version_number: version.version_number.clone(),
            icon_url: icon_url.map(str::to_string),
            source: ProjectSource::Modrinth,
        },
    );
    save_server_installed_index(paths, server, &index);
    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Installed: {safe_name}"),
        fraction: 1.0,
    });

    Ok(ModrinthInstallOutcome::FileInstalled {
        title: title.to_string(),
        filename: safe_name,
        dest_folder: dest_folder.to_string(),
        dependencies_installed: Vec::new(),
    })
}

fn primary_file(version: &ModrinthVersion) -> Result<&crate::models::ModrinthFile, String> {
    version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| format!("Version {} has no files", version.version_number))
}

#[allow(clippy::too_many_arguments)]
fn install_version_single(
    client: &Client,
    paths: &LauncherPaths,
    profile: &Profile,
    project_id: &str,
    project_type: ModrinthProjectType,
    title: &str,
    icon_url: Option<String>,
    version: &ModrinthVersion,
    index: &mut HashMap<String, InstalledProject>,
    tx: &Sender<WorkerEvent>,
) -> Result<String, String> {
    let file = primary_file(version)?;
    let safe_name = sanitize_mod_filename(&file.filename)?;
    let (inst, folder_rel, dest_dir) = ensure_instance_content_dir(paths, profile, project_type)?;

    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Downloading {safe_name}..."),
        fraction: 0.5,
    });
    download_file(
        client,
        &file.url,
        &dest_dir.join(&safe_name),
        file.hashes.sha1.as_deref(),
    )?;

    // Replace an older file of the same project (updating a mod).
    if let Some(old) = index.get(project_id) {
        if old.filename != safe_name {
            let old_dir = inst.join(&old.folder);
            let _ = fs::remove_file(old_dir.join(&old.filename));
            let _ = fs::remove_file(old_dir.join(format!("{}.disabled", old.filename)));
        }
    }
    index.insert(
        project_id.to_string(),
        InstalledProject {
            title: title.to_string(),
            filename: safe_name.clone(),
            folder: folder_rel,
            version_number: version.version_number.clone(),
            icon_url,
            source: ProjectSource::Modrinth,
        },
    );
    Ok(safe_name)
}

#[allow(clippy::too_many_arguments)]
fn install_single(
    client: &Client,
    paths: &LauncherPaths,
    profile: &Profile,
    project_id: &str,
    project_type: ModrinthProjectType,
    title: &str,
    icon_url: Option<String>,
    index: &mut HashMap<String, InstalledProject>,
    tx: &Sender<WorkerEvent>,
) -> Result<String, String> {
    let version = fetch_best_version(
        client,
        project_id,
        project_type,
        &profile.version_id,
        &profile.loader,
    )?;
    install_version_single(
        client,
        paths,
        profile,
        project_id,
        project_type,
        title,
        icon_url,
        &version,
        index,
        tx,
    )
}

/// Fetches all compatible Modrinth versions for a project scoped to the active instance.
pub fn fetch_project_versions_for_profile(
    project_id: &str,
    project_type: ModrinthProjectType,
    game_version: &str,
    loader: &ModLoader,
) -> Result<Vec<ModrinthVersion>, String> {
    let client = build_http_client()?;
    match project_type {
        ModrinthProjectType::Mod => {
            let slug = loader.modrinth_slug().ok_or_else(|| {
                "Mods require a mod loader (Fabric, Quilt, Forge, or NeoForge).".to_string()
            })?;
            let param = format!("[\"{slug}\"]");
            let mut list = get_versions(&client, project_id, Some(game_version), Some(&param))?;
            if list.is_empty() && matches!(loader, ModLoader::Quilt { .. }) {
                list = get_versions(
                    &client,
                    project_id,
                    Some(game_version),
                    Some("[\"fabric\"]"),
                )?;
            }
            Ok(list)
        }
        ModrinthProjectType::Plugin => {
            let loaders = "[\"paper\",\"purpur\",\"folia\",\"spigot\",\"bukkit\"]";
            let list = get_versions(&client, project_id, Some(game_version), Some(loaders))?;
            if !list.is_empty() {
                return Ok(list);
            }
            get_versions(&client, project_id, None, Some(loaders))
        }
        ModrinthProjectType::Datapack => {
            let list = get_versions(
                &client,
                project_id,
                Some(game_version),
                Some("[\"datapack\"]"),
            )?;
            if !list.is_empty() {
                return Ok(list);
            }
            get_versions(&client, project_id, None, Some("[\"datapack\"]"))
        }
        ModrinthProjectType::Resourcepack | ModrinthProjectType::Shader => {
            let list = get_versions(&client, project_id, Some(game_version), None)?;
            if !list.is_empty() {
                return Ok(list);
            }
            get_versions(&client, project_id, None, None)
        }
        ModrinthProjectType::Modpack => {
            if let Some(slug) = loader.modrinth_slug() {
                let param = format!("[\"{slug}\"]");
                let list = get_versions(&client, project_id, None, Some(&param))?;
                if !list.is_empty() {
                    return Ok(list);
                }
            }
            get_versions(
                &client,
                project_id,
                None,
                Some("[\"fabric\",\"neoforge\",\"forge\",\"quilt\"]"),
            )
        }
    }
}

/// Installs a user-chosen specific `ModrinthVersion` into the active instance.
#[allow(clippy::too_many_arguments)]
pub fn install_specific_version(
    paths: &LauncherPaths,
    profile: &Profile,
    project_id: &str,
    project_type: ModrinthProjectType,
    title: &str,
    icon_url: Option<&str>,
    version: &ModrinthVersion,
    tx: &Sender<WorkerEvent>,
) -> Result<ModrinthInstallOutcome, String> {
    check_not_cancelled()?;
    let client = build_http_client()?;
    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Installing {title} ({})...", version.version_number),
        fraction: 0.15,
    });
    match project_type {
        ModrinthProjectType::Mod
        | ModrinthProjectType::Resourcepack
        | ModrinthProjectType::Shader
        | ModrinthProjectType::Datapack => {
            if project_type == ModrinthProjectType::Mod && !profile.loader.is_modded() {
                return Err("Mods require a modded loader (Fabric, Quilt, Forge, or NeoForge). Switch loader in Instance settings.".to_string());
            }
            let (_, dest_folder, _) = ensure_instance_content_dir(paths, profile, project_type)?;
            let mut index = load_installed_index(paths, profile);
            let filename = install_version_single(
                &client,
                paths,
                profile,
                project_id,
                project_type,
                title,
                icon_url.map(str::to_string),
                version,
                &mut index,
                tx,
            )?;
            let mut deps_installed = Vec::new();
            if project_type == ModrinthProjectType::Mod {
                install_required_deps(
                    &client,
                    paths,
                    profile,
                    project_id,
                    &mut index,
                    &mut deps_installed,
                    tx,
                )?;
            }
            save_installed_index(paths, profile, &index);
            let _ = tx.send(WorkerEvent::Progress {
                stage: format!("Installed: {filename}"),
                fraction: 1.0,
            });
            Ok(ModrinthInstallOutcome::FileInstalled {
                title: title.to_string(),
                filename,
                dest_folder,
                dependencies_installed: deps_installed,
            })
        }
        ModrinthProjectType::Plugin => {
            Err("Plugins can only be installed to Dedicated Servers.".to_string())
        }
        ModrinthProjectType::Modpack => {
            install_modpack_version(&client, paths, profile, project_id, version, tx)
        }
    }
}

/// Computes `(sha1_hex, sha512_hex, byte_len)` of a file in a single streaming pass.
pub fn compute_file_hashes(path: &Path) -> Result<(String, String, u64), String> {
    let mut file =
        File::open(path).map_err(|e| format!("Failed to open {}: {e}", path.display()))?;
    let mut sha1 = Sha1::new();
    let mut sha512 = Sha512::new();
    let mut total = 0_u64;
    let mut buf = [0_u8; 65_536];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        sha1.update(&buf[..n]);
        sha512.update(&buf[..n]);
        total += n as u64;
    }
    Ok((
        hex::encode(sha1.finalize()),
        hex::encode(sha512.finalize()),
        total,
    ))
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ModrinthVersionWithProject {
    #[serde(default)]
    project_id: String,
    #[serde(default)]
    version_number: String,
    files: Vec<crate::models::ModrinthFile>,
}

/// Checks and updates all installed mods, resourcepacks, and shaderpacks in `profile`
/// using Modrinth's batch `POST /v2/version_files/update` hash lookup (covering both tracked
/// and manually dropped files) plus tracked `.mc-launcher-modrinth.json` entries.
pub fn update_all_installed_projects(
    paths: &LauncherPaths,
    profile: &Profile,
    tx: &Sender<WorkerEvent>,
) -> Result<Vec<String>, String> {
    check_not_cancelled()?;
    let client = build_http_client()?;
    let inst = instance_dir(paths, profile);
    let mut index = load_installed_index(paths, profile);
    let mut updated_titles = Vec::new();
    let mut updated_project_ids = HashSet::new();

    let mod_loaders: Vec<&str> = match &profile.loader {
        ModLoader::Vanilla => Vec::new(),
        ModLoader::Fabric { .. } => vec!["fabric"],
        ModLoader::Quilt { .. } => vec!["quilt", "fabric"],
        ModLoader::Forge { .. } => vec!["forge"],
        ModLoader::NeoForge { .. } => vec!["neoforge"],
    };

    let buckets: [(&str, Vec<&str>); 3] = [
        ("mods", mod_loaders),
        ("resourcepacks", vec!["minecraft"]),
        ("shaderpacks", vec!["iris", "optifine", "canvas"]),
    ];

    for (b_idx, (folder, loaders)) in buckets.iter().enumerate() {
        check_not_cancelled()?;
        if loaders.is_empty() {
            continue;
        }
        let dir = inst.join(folder);
        let items: Vec<InstalledContentItem> = list_content_items(&dir)
            .into_iter()
            .filter(|i| i.enabled)
            .collect();
        if items.is_empty() {
            continue;
        }

        let _ = tx.send(WorkerEvent::Progress {
            stage: format!("Hashing {folder} ({} files)...", items.len()),
            fraction: 0.08 + 0.20 * (b_idx as f32 / 3.0),
        });

        let mut hash_to_item: HashMap<String, InstalledContentItem> = HashMap::new();
        for item in items {
            if let Ok((sha1, _, _)) = compute_file_hashes(&dir.join(&item.disk_name)) {
                hash_to_item.insert(sha1, item);
            }
        }
        if hash_to_item.is_empty() {
            continue;
        }

        let hashes: Vec<String> = hash_to_item.keys().cloned().collect();
        let body = serde_json::json!({
            "hashes": hashes,
            "algorithm": "sha1",
            "loaders": loaders,
            "game_versions": [&profile.version_id],
        });

        let Ok(resp) = client
            .post(format!("{MODRINTH_API_BASE}/version_files/update"))
            .json(&body)
            .send()
            .and_then(|r| r.error_for_status())
        else {
            continue;
        };
        let Ok(updates_map) = resp.json::<HashMap<String, ModrinthVersionWithProject>>() else {
            continue;
        };

        for (old_sha1, latest_ver) in updates_map {
            check_not_cancelled()?;
            let Some(local_item) = hash_to_item
                .get(&old_sha1.to_ascii_lowercase())
                .or_else(|| hash_to_item.get(&old_sha1))
            else {
                continue;
            };
            let Some(new_file) = latest_ver
                .files
                .iter()
                .find(|f| f.primary)
                .or_else(|| latest_ver.files.first())
            else {
                continue;
            };
            if new_file
                .hashes
                .sha1
                .as_deref()
                .is_some_and(|h| h.eq_ignore_ascii_case(&old_sha1))
            {
                continue;
            }

            let safe_new_name =
                sanitize_mod_filename(&new_file.filename).unwrap_or_else(|_| "mod.jar".to_string());

            let display_title = index
                .get(&latest_ver.project_id)
                .map(|p| p.title.clone())
                .or_else(|| {
                    (!latest_ver.project_id.is_empty())
                        .then(|| project_meta(&client, &latest_ver.project_id))
                        .flatten()
                        .map(|(t, _)| t)
                })
                .unwrap_or_else(|| local_item.base_name.clone());

            let _ = tx.send(WorkerEvent::Progress {
                stage: format!("Updating {display_title}..."),
                fraction: 0.55,
            });
            download_file(
                &client,
                &new_file.url,
                &dir.join(&safe_new_name),
                new_file.hashes.sha1.as_deref(),
            )?;
            if safe_new_name != local_item.disk_name {
                let _ = fs::remove_file(dir.join(&local_item.disk_name));
            }
            if !latest_ver.project_id.is_empty() {
                let existing_icon = index
                    .get(&latest_ver.project_id)
                    .and_then(|p| p.icon_url.clone());
                index.insert(
                    latest_ver.project_id.clone(),
                    InstalledProject {
                        title: display_title.clone(),
                        filename: safe_new_name,
                        folder: (*folder).to_string(),
                        version_number: latest_ver.version_number,
                        icon_url: existing_icon,
                        source: ProjectSource::Modrinth,
                    },
                );
                updated_project_ids.insert(latest_ver.project_id);
            }
            updated_titles.push(display_title);
        }
    }

    // Fallback check for tracked Modrinth entries (e.g. disabled mods or projects skipped by batch lookup).
    let entries: Vec<(String, InstalledProject)> = index
        .iter()
        .filter(|(pid, p)| {
            p.source == ProjectSource::Modrinth && !updated_project_ids.contains(*pid)
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let total = entries.len().max(1);
    for (i, (project_id, entry)) in entries.into_iter().enumerate() {
        check_not_cancelled()?;
        let project_type = match entry.folder.as_str() {
            "resourcepacks" => ModrinthProjectType::Resourcepack,
            "shaderpacks" => ModrinthProjectType::Shader,
            s if s.ends_with("datapacks") => ModrinthProjectType::Datapack,
            _ => ModrinthProjectType::Mod,
        };
        let _ = tx.send(WorkerEvent::Progress {
            stage: format!("Checking ({}/{}): {}...", i + 1, total, entry.title),
            fraction: 0.70 + 0.25 * ((i as f32 + 1.0) / total as f32),
        });
        if let Ok(best) = fetch_best_version(
            &client,
            &project_id,
            project_type,
            &profile.version_id,
            &profile.loader,
        ) {
            if best.version_number != entry.version_number {
                install_version_single(
                    &client,
                    paths,
                    profile,
                    &project_id,
                    project_type,
                    &entry.title,
                    entry.icon_url.clone(),
                    &best,
                    &mut index,
                    tx,
                )?;
                updated_titles.push(entry.title);
            }
        }
    }

    save_installed_index(paths, profile, &index);
    let _ = tx.send(WorkerEvent::Progress {
        stage: "Update check complete".to_string(),
        fraction: 1.0,
    });
    Ok(updated_titles)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ZipModpackFlavor {
    ModrinthMrpack,
    MultiMcPrism,
    CurseForge,
}

fn detect_zip_modpack_flavor(path: &Path) -> Option<ZipModpackFlavor> {
    let file = File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    if archive.by_name("modrinth.index.json").is_ok() {
        return Some(ZipModpackFlavor::ModrinthMrpack);
    }
    let mut has_mmc = false;
    let mut has_cf = false;
    for i in 0..archive.len() {
        let Ok(entry) = archive.by_index_raw(i) else {
            continue;
        };
        let name = entry.name();
        let clean = name.trim_matches('/');
        if clean == "mmc-pack.json" || clean.ends_with("/mmc-pack.json") {
            has_mmc = true;
            break;
        }
        if clean == "manifest.json" {
            has_cf = true;
        }
    }
    if has_mmc {
        return Some(ZipModpackFlavor::MultiMcPrism);
    }
    if has_cf {
        // Distinguish CurseForge modpack `manifest.json` from resourcepack manifests by checking `"minecraft"`.
        if let Ok(mut entry) = archive.by_name("manifest.json") {
            let mut s = String::new();
            if entry.read_to_string(&mut s).is_ok()
                && s.contains("\"minecraft\"")
                && s.contains("\"modLoaders\"")
            {
                return Some(ZipModpackFlavor::CurseForge);
            }
        }
    }
    None
}

/// Imports a `.mrpack`, Prism/MultiMC `.zip` instance, or CurseForge `.zip` modpack into a new profile.
pub fn import_modpack_archive(
    paths: &LauncherPaths,
    archive_path: &Path,
    username: String,
    curseforge_api_key: &str,
    tx: &Sender<WorkerEvent>,
) -> Result<ModrinthInstallOutcome, String> {
    check_not_cancelled()?;
    match detect_zip_modpack_flavor(archive_path) {
        Some(ZipModpackFlavor::ModrinthMrpack) => install_mrpack(paths, archive_path, username, tx),
        Some(ZipModpackFlavor::MultiMcPrism) => {
            install_multimc_prism_zip(paths, archive_path, username, tx)
        }
        Some(ZipModpackFlavor::CurseForge) => {
            install_curseforge_zip(paths, archive_path, username, curseforge_api_key, tx)
        }
        None => {
            if archive_path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("mrpack"))
            {
                install_mrpack(paths, archive_path, username, tx)
            } else {
                Err(
                    "Archive is not a recognized modpack (.mrpack, Prism/MultiMC .zip, or CurseForge .zip)"
                        .to_string(),
                )
            }
        }
    }
}

/// Imports a dropped local `.jar`, `.zip`, or `.mrpack` file into the active instance
/// (automatically detecting `.mrpack`, Prism/MultiMC `.zip`, and CurseForge `.zip` modpacks).
pub fn import_local_file(
    paths: &LauncherPaths,
    profile: &Profile,
    src_path: &Path,
    active_folder: &str,
    curseforge_api_key: &str,
    tx: &Sender<WorkerEvent>,
) -> Result<ModrinthInstallOutcome, String> {
    let ext = src_path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    let filename = src_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| "Invalid file path".to_string())?;

    match ext.as_str() {
        "mrpack" => install_mrpack(paths, src_path, profile.username.clone(), tx),
        "zip" if is_profile_backup_archive(src_path) => {
            let (new_profile, files_count) = import_profile_archive(paths, src_path, tx)?;
            Ok(ModrinthInstallOutcome::ModpackProfileCreated {
                profile: new_profile,
                files_count,
            })
        }
        "zip" if detect_zip_modpack_flavor(src_path).is_some() => import_modpack_archive(
            paths,
            src_path,
            profile.username.clone(),
            curseforge_api_key,
            tx,
        ),
        "jar" | "zip" => {
            let dest_folder = if ext == "jar" {
                "mods"
            } else if active_folder == "shaderpacks" {
                "shaderpacks"
            } else {
                "resourcepacks"
            };
            let dest_dir = instance_dir(paths, profile).join(dest_folder);
            fs::create_dir_all(&dest_dir)
                .map_err(|e| format!("Failed to create {dest_folder}/: {e}"))?;
            fs::copy(src_path, dest_dir.join(&filename))
                .map_err(|e| format!("Failed to import {filename}: {e}"))?;
            Ok(ModrinthInstallOutcome::FileInstalled {
                title: filename.clone(),
                filename,
                dest_folder: dest_folder.to_string(),
                dependencies_installed: Vec::new(),
            })
        }
        _ => Err(format!(
            "Unsupported file '{filename}' (expected .jar, .zip, or .mrpack)"
        )),
    }
}

fn required_dependencies(
    client: &Client,
    project_id: &str,
    game_version: &str,
    loader: &ModLoader,
) -> Result<Vec<String>, String> {
    let Some(slug) = loader.modrinth_slug() else {
        return Ok(Vec::new());
    };
    let loader_param = if matches!(loader, ModLoader::Quilt { .. }) {
        "[\"quilt\",\"fabric\"]".to_string()
    } else {
        format!("[\"{slug}\"]")
    };
    let Some(version) = get_versions(client, project_id, Some(game_version), Some(&loader_param))?
        .into_iter()
        .next()
    else {
        return Ok(Vec::new());
    };
    Ok(version
        .dependencies
        .iter()
        .filter(|d| d.dependency_type == "required")
        .filter_map(|d| d.project_id.clone())
        .collect())
}

fn project_meta(client: &Client, project_id: &str) -> Option<(String, Option<String>)> {
    #[derive(serde::Deserialize)]
    struct P {
        title: String,
        #[serde(default)]
        icon_url: Option<String>,
    }
    client
        .get(format!("{MODRINTH_API_BASE}/project/{project_id}"))
        .send()
        .ok()?
        .json::<P>()
        .ok()
        .map(|p| (p.title, p.icon_url))
}

/// Parses `.mrpack` `dependencies` map into `(minecraft_version, ModLoader)`.
pub fn parse_mrpack_loader(deps: &HashMap<String, String>) -> Result<(String, ModLoader), String> {
    let mc_version = deps
        .get("minecraft")
        .cloned()
        .ok_or_else(|| "Modpack does not specify a Minecraft version".to_string())?;

    let loader = if let Some(v) = deps.get("fabric-loader") {
        ModLoader::Fabric {
            loader_version: v.clone(),
        }
    } else if let Some(v) = deps.get("quilt-loader") {
        ModLoader::Quilt {
            loader_version: v.clone(),
        }
    } else if let Some(v) = deps.get("forge") {
        ModLoader::Forge {
            loader_version: v.clone(),
        }
    } else if let Some(v) = deps.get("neoforge") {
        ModLoader::NeoForge {
            loader_version: v.clone(),
        }
    } else {
        ModLoader::Vanilla
    };

    Ok((mc_version, loader))
}

fn parse_exclude_patterns(raw: &str) -> Vec<String> {
    raw.split([',', '\n', ';'])
        .map(|s| s.trim().replace('\\', "/"))
        .map(|s| s.trim_start_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub(crate) fn path_matches_exclude(rel_posix: &str, patterns: &[String]) -> bool {
    let rel_lower = rel_posix.to_ascii_lowercase();
    let file_name = rel_posix.rsplit('/').next().unwrap_or(rel_posix);
    let file_lower = file_name.to_ascii_lowercase();
    for pat in patterns {
        let p = pat.to_ascii_lowercase();
        let p_trimmed = p.trim_end_matches('/');
        if p.ends_with('/') {
            if rel_lower.starts_with(&p) || rel_lower == p_trimmed {
                return true;
            }
        } else if let Some(ext) = p.strip_prefix("*.") {
            if file_lower.ends_with(&format!(".{ext}")) {
                return true;
            }
        } else if let Some(prefix) = p.strip_suffix('*') {
            if rel_lower.starts_with(prefix) || file_lower.starts_with(prefix) {
                return true;
            }
        } else if rel_lower == p || rel_lower.starts_with(&format!("{p}/")) || file_lower == p {
            return true;
        }
    }
    false
}

fn add_dir_to_zip(
    writer: &mut zip::ZipWriter<File>,
    options: zip::write::SimpleFileOptions,
    base_dir: &Path,
    dir: &Path,
    archive_prefix: &str,
    excludes: &[String],
) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if let Ok(rel) = p.strip_prefix(base_dir) {
            let rel_posix = rel.to_string_lossy().replace('\\', "/");
            if path_matches_exclude(&rel_posix, excludes) {
                continue;
            }
            if p.is_dir() {
                add_dir_to_zip(writer, options, base_dir, &p, archive_prefix, excludes)?;
            } else if p.is_file() {
                let zip_name = if archive_prefix.is_empty() {
                    rel_posix
                } else {
                    format!("{archive_prefix}{rel_posix}")
                };
                writer
                    .start_file(&zip_name, options)
                    .map_err(|e| format!("Failed to add {zip_name}: {e}"))?;
                let mut src =
                    File::open(&p).map_err(|e| format!("Failed to open {}: {e}", p.display()))?;
                io::copy(&mut src, writer)
                    .map_err(|e| format!("Failed to copy {}: {e}", p.display()))?;
            }
        }
    }
    Ok(())
}

/// Exports an instance's loader metadata, mods, resourcepacks, shaderpacks, and configs
/// into a spec-compliant `.mrpack` archive under `paths.root/exports/`.
#[allow(dead_code)]
pub fn export_instance_mrpack(paths: &LauncherPaths, profile: &Profile) -> Result<PathBuf, String> {
    export_instance_mrpack_with_options(paths, profile, &MrpackExportOptions::default())
}

/// Same as `export_instance_mrpack`, honoring `MrpackExportOptions` exclusions.
pub fn export_instance_mrpack_with_options(
    paths: &LauncherPaths,
    profile: &Profile,
    export_opts: &MrpackExportOptions,
) -> Result<PathBuf, String> {
    let inst = instance_dir(paths, profile);
    let exports_dir = paths.root.join("exports");
    fs::create_dir_all(&exports_dir)
        .map_err(|e| format!("Failed to create exports directory: {e}"))?;

    let safe_name: String = profile
        .name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let file_stem = if safe_name.trim_matches('_').is_empty() {
        "modpack"
    } else {
        safe_name.trim_matches('_')
    };
    let out_path = exports_dir.join(format!("{file_stem}.mrpack"));

    let mut deps = HashMap::new();
    deps.insert("minecraft".to_string(), profile.version_id.clone());
    match &profile.loader {
        ModLoader::Vanilla => {}
        ModLoader::Fabric { loader_version } if !loader_version.is_empty() => {
            deps.insert("fabric-loader".to_string(), loader_version.clone());
        }
        ModLoader::Quilt { loader_version } if !loader_version.is_empty() => {
            deps.insert("quilt-loader".to_string(), loader_version.clone());
        }
        ModLoader::Forge { loader_version } if !loader_version.is_empty() => {
            deps.insert("forge".to_string(), loader_version.clone());
        }
        ModLoader::NeoForge { loader_version } if !loader_version.is_empty() => {
            deps.insert("neoforge".to_string(), loader_version.clone());
        }
        _ => {}
    }

    let excludes = parse_exclude_patterns(&export_opts.extra_excludes);

    struct CandidatePackFile {
        rel_posix: String,
        disk_path: PathBuf,
        sha1: String,
        sha512: String,
        size: u64,
        include_as_override: bool,
    }

    let mut candidates: Vec<CandidatePackFile> = Vec::new();
    let folder_checks = [
        ("mods", true, export_opts.include_unmatched_mods),
        (
            "resourcepacks",
            export_opts.include_resourcepacks,
            export_opts.include_resourcepacks,
        ),
        (
            "shaderpacks",
            export_opts.include_shaderpacks,
            export_opts.include_shaderpacks,
        ),
    ];
    for (sub, include_indexed, include_override) in folder_checks {
        if !include_indexed && !include_override {
            continue;
        }
        let dir = inst.join(sub);
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            let Some(fname) = p.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if fname.starts_with('.') || fname.ends_with(".disabled") {
                continue;
            }
            let rel_posix = format!("{sub}/{fname}");
            if path_matches_exclude(&rel_posix, &excludes) {
                continue;
            }
            if let Ok((sha1, sha512, size)) = compute_file_hashes(&p) {
                candidates.push(CandidatePackFile {
                    rel_posix,
                    disk_path: p,
                    sha1,
                    sha512,
                    size,
                    include_as_override: include_override,
                });
            }
        }
    }

    let mut resolved_urls: HashMap<String, String> = HashMap::new();
    if !candidates.is_empty() {
        if let Ok(client) = build_http_client() {
            for chunk in candidates.chunks(100) {
                let sha1_list: Vec<&str> = chunk.iter().map(|c| c.sha1.as_str()).collect();
                let body = serde_json::json!({
                    "hashes": sha1_list,
                    "algorithm": "sha1",
                });
                if let Ok(resp) = client
                    .post(format!("{MODRINTH_API_BASE}/version_files"))
                    .json(&body)
                    .send()
                    .and_then(|r| r.error_for_status())
                {
                    if let Ok(map) = resp.json::<HashMap<String, ModrinthVersion>>() {
                        for (hash_key, ver) in map {
                            if let Some(matched_file) = ver
                                .files
                                .iter()
                                .find(|f| {
                                    f.hashes
                                        .sha1
                                        .as_deref()
                                        .is_some_and(|h| h.eq_ignore_ascii_case(&hash_key))
                                })
                                .or_else(|| ver.files.first())
                            {
                                if !matched_file.url.is_empty() {
                                    resolved_urls.insert(
                                        hash_key.to_ascii_lowercase(),
                                        matched_file.url.clone(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut mrpack_files = Vec::new();
    let mut override_files = Vec::new();
    for cand in candidates {
        if let Some(url) = resolved_urls.get(&cand.sha1.to_ascii_lowercase()) {
            mrpack_files.push(crate::models::MrpackFile {
                path: cand.rel_posix,
                hashes: crate::models::ModrinthHashes {
                    sha1: Some(cand.sha1),
                    sha512: Some(cand.sha512),
                },
                env: Some(crate::models::MrpackEnv {
                    client: Some("required".to_string()),
                    server: Some("optional".to_string()),
                }),
                downloads: vec![url.clone()],
                file_size: Some(cand.size),
            });
        } else if cand.include_as_override {
            override_files.push((cand.rel_posix, cand.disk_path));
        }
    }

    let index = MrpackIndex {
        format_version: 1,
        game: "minecraft".to_string(),
        version_id: "1.0.0".to_string(),
        name: profile.name.clone(),
        summary: Some(format!(
            "Exported from Sunset Launcher ({})",
            profile.version_id
        )),
        files: mrpack_files,
        dependencies: deps,
    };

    let file = File::create(&out_path)
        .map_err(|e| format!("Failed to create {}: {e}", out_path.display()))?;
    let mut zip_writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip_writer
        .start_file("modrinth.index.json", options)
        .map_err(|e| format!("Failed to write modrinth.index.json: {e}"))?;
    let index_bytes = serde_json::to_vec_pretty(&index)
        .map_err(|e| format!("Failed to serialize modrinth.index.json: {e}"))?;
    io::Write::write_all(&mut zip_writer, &index_bytes)
        .map_err(|e| format!("Failed to write index bytes: {e}"))?;

    for (rel_posix, disk_path) in override_files {
        let zip_name = format!("overrides/{rel_posix}");
        zip_writer
            .start_file(&zip_name, options)
            .map_err(|e| format!("Failed to add {zip_name}: {e}"))?;
        let mut src = File::open(&disk_path)
            .map_err(|e| format!("Failed to open {}: {e}", disk_path.display()))?;
        io::copy(&mut src, &mut zip_writer)
            .map_err(|e| format!("Failed to copy {}: {e}", disk_path.display()))?;
    }

    if export_opts.include_config {
        let config_dir = inst.join("config");
        if config_dir.is_dir() && !path_matches_exclude("config", &excludes) {
            add_dir_to_zip(
                &mut zip_writer,
                options,
                &inst,
                &config_dir,
                "overrides/",
                &excludes,
            )?;
        }
    }
    if export_opts.include_options_txt {
        let options_txt = inst.join("options.txt");
        if options_txt.is_file() && !path_matches_exclude("options.txt", &excludes) {
            zip_writer
                .start_file("overrides/options.txt", options)
                .map_err(|e| format!("Failed to add overrides/options.txt: {e}"))?;
            let mut src =
                File::open(&options_txt).map_err(|e| format!("Failed to open options.txt: {e}"))?;
            io::copy(&mut src, &mut zip_writer)
                .map_err(|e| format!("Failed to copy options.txt: {e}"))?;
        }
    }
    if export_opts.include_saves {
        let saves_dir = inst.join("saves");
        if saves_dir.is_dir() && !path_matches_exclude("saves", &excludes) {
            add_dir_to_zip(
                &mut zip_writer,
                options,
                &inst,
                &saves_dir,
                "overrides/",
                &excludes,
            )?;
        }
    }
    if export_opts.include_screenshots {
        let shots_dir = inst.join("screenshots");
        if shots_dir.is_dir() && !path_matches_exclude("screenshots", &excludes) {
            add_dir_to_zip(
                &mut zip_writer,
                options,
                &inst,
                &shots_dir,
                "overrides/",
                &excludes,
            )?;
        }
    }

    zip_writer
        .finish()
        .map_err(|e| format!("Failed to finalize .mrpack: {e}"))?;
    Ok(out_path)
}

/// Marker file identifying a full profile backup archive created by Sunrise Launcher.
const PROFILE_BACKUP_FORMAT: &str = "sunset-launcher-profile";
const PROFILE_BACKUP_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
struct ProfileBackupManifest {
    format: String,
    version: u32,
    profile: Profile,
}

/// Exports an entire instance profile (settings/metadata plus the full game directory)
/// into a single `.sunsetprofile.zip` archive under `paths.root/exports/`. This differs
/// from `.mrpack` in that it captures the instance exactly as configured, including the
/// selected account name, RAM/JVM settings, and every file under the instance directory.
pub fn export_profile_archive(
    paths: &LauncherPaths,
    profile: &Profile,
    tx: Option<&Sender<WorkerEvent>>,
) -> Result<PathBuf, String> {
    let inst = instance_dir(paths, profile);
    let exports_dir = paths.root.join("exports");
    fs::create_dir_all(&exports_dir).map_err(|e| {
        format!(
            "Failed to create exports dir {}: {e}",
            exports_dir.display()
        )
    })?;

    let safe_name = crate::storage::sanitize_instance_dir_name(&profile.name);
    let out_path = exports_dir.join(format!("{safe_name}.sunsetprofile.zip"));
    let file = File::create(&out_path)
        .map_err(|e| format!("Failed to create {}: {e}", out_path.display()))?;
    let mut zip_writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let manifest = ProfileBackupManifest {
        format: PROFILE_BACKUP_FORMAT.to_string(),
        version: PROFILE_BACKUP_VERSION,
        profile: profile.clone(),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize profile backup manifest: {e}"))?;
    zip_writer
        .start_file("sunsetprofile.json", options)
        .map_err(|e| format!("Failed to add sunsetprofile.json: {e}"))?;
    io::Write::write_all(&mut zip_writer, manifest_json.as_bytes())
        .map_err(|e| format!("Failed to write sunsetprofile.json: {e}"))?;

    if let Some(tx) = tx {
        let _ = tx.send(WorkerEvent::Progress {
            stage: "Packaging instance files...".to_string(),
            fraction: 0.5,
        });
    }

    let skips = ["logs", ".cache", ".fabric", "crash-reports"];
    let mut adds: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&inst) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if skips.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
                continue;
            }
            adds.push(entry.path());
        }
    }
    for p in adds {
        if p.is_dir() {
            add_dir_to_zip(&mut zip_writer, options, &inst, &p, "instance/", &[])?;
        } else if p.is_file() {
            if let Ok(rel) = p.strip_prefix(&inst) {
                let rel_posix = rel.to_string_lossy().replace('\\', "/");
                let zip_name = format!("instance/{rel_posix}");
                zip_writer
                    .start_file(&zip_name, options)
                    .map_err(|e| format!("Failed to add {zip_name}: {e}"))?;
                let mut src =
                    File::open(&p).map_err(|e| format!("Failed to open {}: {e}", p.display()))?;
                io::copy(&mut src, &mut zip_writer)
                    .map_err(|e| format!("Failed to copy {}: {e}", p.display()))?;
            }
        }
    }

    zip_writer
        .finish()
        .map_err(|e| format!("Failed to finalize profile archive: {e}"))?;
    if let Some(tx) = tx {
        let _ = tx.send(WorkerEvent::Progress {
            stage: "Profile archive exported".to_string(),
            fraction: 1.0,
        });
    }
    Ok(out_path)
}

/// Returns true when `path` is a zip archive containing a Sunset profile manifest.
pub fn is_profile_backup_archive(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return false;
    };
    let has_manifest = archive.by_name("sunsetprofile.json").is_ok();
    has_manifest
}

/// Imports a `.sunsetprofile.zip` archive produced by [`export_profile_archive`] as a new
/// profile. Returns the new profile plus the number of instance files restored.
pub fn import_profile_archive(
    paths: &LauncherPaths,
    archive_path: &Path,
    tx: &Sender<WorkerEvent>,
) -> Result<(Profile, usize), String> {
    check_not_cancelled()?;
    let file = File::open(archive_path)
        .map_err(|e| format!("Failed to open {}: {e}", archive_path.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Invalid profile archive: {e}"))?;

    let mut profile: Option<Profile> = None;
    if let Ok(mut entry) = archive.by_name("sunsetprofile.json") {
        let mut buf = String::new();
        entry
            .read_to_string(&mut buf)
            .map_err(|e| format!("Failed to read sunsetprofile.json: {e}"))?;
        let manifest: ProfileBackupManifest = serde_json::from_str(&buf)
            .map_err(|e| format!("Failed to parse sunsetprofile.json: {e}"))?;
        if manifest.format != PROFILE_BACKUP_FORMAT {
            return Err("Archive is not a Sunset Launcher profile backup".to_string());
        }
        if manifest.version > PROFILE_BACKUP_VERSION {
            return Err(format!(
                "Profile backup version {} is newer than supported ({PROFILE_BACKUP_VERSION})",
                manifest.version
            ));
        }
        profile = Some(manifest.profile);
    }
    let mut profile = profile.ok_or_else(|| {
        "Archive is missing sunsetprofile.json (not a profile backup)".to_string()
    })?;

    let existing = crate::storage::load_profiles(paths);
    let base_name = if profile.name.trim().is_empty() {
        "Imported Profile".to_string()
    } else {
        profile.name.clone()
    };
    let mut display_name = base_name.clone();
    let mut suffix = 2;
    while existing
        .iter()
        .any(|p| p.name.eq_ignore_ascii_case(&display_name))
    {
        display_name = format!("{base_name} ({suffix})");
        suffix += 1;
    }

    profile.id = uuid::Uuid::new_v4().to_string();
    profile.name = display_name;
    profile.dir_name = unique_instance_dir_name(paths, &profile.name, &existing);
    profile.created_at = chrono::Utc::now().to_rfc3339();
    profile.last_played = None;
    profile.playtime_secs = 0;
    let inst = instance_dir(paths, &profile);

    let mut restored = 0_usize;
    let total = archive.len();
    for i in 0..total {
        if check_not_cancelled().is_err() {
            return Err(crate::mojang::CANCELLED_ERR_MSG.to_string());
        }
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Zip entry error: {e}"))?;
        let name = entry.name().to_string();
        let Some(rel) = name.strip_prefix("instance/") else {
            continue;
        };
        if rel.is_empty() || name.ends_with('/') {
            continue;
        }
        let Some(target_rel) = safe_rel_path(rel) else {
            continue;
        };
        let target = inst.join(target_rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
        }
        if let Ok(mut out) = File::create(&target) {
            io::copy(&mut entry, &mut out)
                .map_err(|e| format!("Failed to write {}: {e}", target.display()))?;
            restored += 1;
        }
        let _ = tx.send(WorkerEvent::Progress {
            stage: format!("Restoring files: {restored}/{total}"),
            fraction: 0.10 + 0.85 * ((i + 1) as f32 / total.max(1) as f32),
        });
    }

    Ok((profile, restored))
}

/// Installs a local or downloaded `.mrpack` modpack into a new instance profile.
pub fn install_mrpack(
    paths: &LauncherPaths,
    mrpack_path: &Path,
    username: String,
    tx: &Sender<WorkerEvent>,
) -> Result<ModrinthInstallOutcome, String> {
    check_not_cancelled()?;
    let file = File::open(mrpack_path)
        .map_err(|e| format!("Failed to open .mrpack {}: {e}", mrpack_path.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Invalid .mrpack archive: {e}"))?;

    let index: MrpackIndex = {
        let mut idx_entry = archive
            .by_name("modrinth.index.json")
            .map_err(|_| "Archive is missing modrinth.index.json".to_string())?;
        let mut buf = String::new();
        io::Read::read_to_string(&mut idx_entry, &mut buf)
            .map_err(|e| format!("Failed to read modrinth.index.json: {e}"))?;
        serde_json::from_str(&buf)
            .map_err(|e| format!("Failed to parse modrinth.index.json: {e}"))?
    };

    let (mc_version, loader) = parse_mrpack_loader(&index.dependencies)?;

    let mut new_profile = Profile::new(index.name.clone(), mc_version, loader, username);
    new_profile.dir_name = crate::storage::unique_instance_dir_name(paths, &new_profile.name, &[]);
    let inst_dir = instance_dir(paths, &new_profile);
    let _ = fs::create_dir_all(inst_dir.join("mods"));

    let client = build_http_client()?;
    let client_files: Vec<_> = index
        .files
        .iter()
        .filter(|f| f.env.as_ref().and_then(|e| e.client.as_deref()) != Some("unsupported"))
        .collect();

    let jobs: Vec<crate::mojang::DownloadJob> = client_files
        .iter()
        .filter_map(|mf| {
            let rel = safe_rel_path(&mf.path)?;
            Some(crate::mojang::DownloadJob {
                url: mf.downloads.first()?.clone(),
                dest: inst_dir.join(rel),
                sha1: mf.hashes.sha1.clone(),
                size: mf.file_size,
            })
        })
        .collect();
    let files_count = jobs.len();
    crate::mojang::download_parallel(&client, jobs, 24, &|done, total| {
        let _ = tx.send(WorkerEvent::Progress {
            stage: format!("Modpack files: {done}/{total}"),
            fraction: 0.20 + 0.65 * (done as f32 / total.max(1) as f32),
        });
    })?;

    // Verify SHA-512 digests when provided by modrinth.index.json.
    for mf in &client_files {
        if let Some(ref expected_sha512) = mf.hashes.sha512 {
            if let Some(rel) = safe_rel_path(&mf.path) {
                let target = inst_dir.join(rel);
                if target.is_file() {
                    let (_, actual_sha512, _) = compute_file_hashes(&target)?;
                    if !actual_sha512.eq_ignore_ascii_case(expected_sha512) {
                        let _ = fs::remove_file(&target);
                        return Err(format!("SHA-512 mismatch for modpack file {}", mf.path));
                    }
                }
            }
        }
    }

    // Extract overrides/ then client-overrides/ (the latter wins).
    let _ = tx.send(WorkerEvent::Progress {
        stage: "Extracting overrides...".to_string(),
        fraction: 0.90,
    });
    for prefix in ["overrides/", "client-overrides/"] {
        for i in 0..archive.len() {
            check_not_cancelled()?;
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("Zip entry error: {e}"))?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().to_string();
            let Some(rel) = name.strip_prefix(prefix).and_then(safe_rel_path) else {
                continue;
            };
            let out_path = inst_dir.join(rel);
            if let Some(parent) = out_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let mut out = File::create(&out_path)
                .map_err(|e| format!("Failed to create override {}: {e}", out_path.display()))?;
            io::copy(&mut entry, &mut out)
                .map_err(|e| format!("Failed to extract override: {e}"))?;
        }
    }

    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Modpack '{}' installed!", new_profile.name),
        fraction: 1.0,
    });

    Ok(ModrinthInstallOutcome::ModpackProfileCreated {
        profile: new_profile,
        files_count,
    })
}

#[derive(serde::Deserialize)]
struct MmcPackJson {
    #[serde(default)]
    components: Vec<MmcComponent>,
}

#[derive(serde::Deserialize)]
struct MmcComponent {
    #[serde(default)]
    uid: String,
    #[serde(default)]
    version: String,
}

pub fn parse_mmc_pack_components(raw_json: &str) -> Result<(String, ModLoader), String> {
    let pack: MmcPackJson = serde_json::from_str(raw_json)
        .map_err(|e| format!("Failed to parse mmc-pack.json: {e}"))?;
    let mut mc_ver: Option<String> = None;
    let mut loader = ModLoader::Vanilla;
    for comp in pack.components {
        match comp.uid.as_str() {
            "net.minecraft" if !comp.version.is_empty() => mc_ver = Some(comp.version),
            "net.fabricmc.fabric-loader" => {
                loader = ModLoader::Fabric {
                    loader_version: comp.version,
                };
            }
            "org.quiltmc.quilt-loader" => {
                loader = ModLoader::Quilt {
                    loader_version: comp.version,
                };
            }
            "net.minecraftforge" => {
                loader = ModLoader::Forge {
                    loader_version: comp.version,
                };
            }
            "net.neoforged" => {
                loader = ModLoader::NeoForge {
                    loader_version: comp.version,
                };
            }
            _ => {}
        }
    }
    let mc =
        mc_ver.ok_or_else(|| "mmc-pack.json does not specify net.minecraft version".to_string())?;
    Ok((mc, loader))
}

fn parse_mmc_instance_name(cfg_text: &str) -> Option<String> {
    for line in cfg_text.lines() {
        if let Some((k, v)) = line.trim().split_once('=') {
            if k.trim().eq_ignore_ascii_case("name") {
                let clean = v.trim();
                if !clean.is_empty() {
                    return Some(clean.to_string());
                }
            }
        }
    }
    None
}

/// Imports a Prism Launcher / MultiMC `.zip` exported instance into a new Sunset Launcher profile.
pub fn install_multimc_prism_zip(
    paths: &LauncherPaths,
    zip_path: &Path,
    username: String,
    tx: &Sender<WorkerEvent>,
) -> Result<ModrinthInstallOutcome, String> {
    check_not_cancelled()?;
    let file =
        File::open(zip_path).map_err(|e| format!("Failed to open {}: {e}", zip_path.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Invalid Prism/MultiMC zip: {e}"))?;

    let mut root_prefix = String::new();
    let mut mmc_raw = None;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Zip entry error: {e}"))?;
        let name = entry.name().to_string();
        if name == "mmc-pack.json" || name.ends_with("/mmc-pack.json") {
            root_prefix = name.strip_suffix("mmc-pack.json").unwrap_or("").to_string();
            let mut buf = String::new();
            entry
                .read_to_string(&mut buf)
                .map_err(|e| format!("Failed to read mmc-pack.json: {e}"))?;
            mmc_raw = Some(buf);
            break;
        }
    }
    let mmc_json =
        mmc_raw.ok_or_else(|| "Prism/MultiMC archive is missing mmc-pack.json".to_string())?;
    let (mc_version, loader) = parse_mmc_pack_components(&mmc_json)?;

    let cfg_entry_name = format!("{root_prefix}instance.cfg");
    let mut display_name = None;
    if let Ok(mut cfg_entry) = archive.by_name(&cfg_entry_name) {
        let mut cfg_text = String::new();
        if cfg_entry.read_to_string(&mut cfg_text).is_ok() {
            display_name = parse_mmc_instance_name(&cfg_text);
        }
    }
    let pack_name = display_name.unwrap_or_else(|| {
        zip_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Imported Instance".to_string())
    });

    let mut new_profile = Profile::new(pack_name, mc_version, loader, username);
    new_profile.dir_name = crate::storage::unique_instance_dir_name(paths, &new_profile.name, &[]);
    let inst_dir = instance_dir(paths, &new_profile);
    let _ = fs::create_dir_all(inst_dir.join("mods"));

    let dot_mc = format!("{root_prefix}.minecraft/");
    let plain_mc = format!("{root_prefix}minecraft/");
    let mut extracted_files = 0usize;
    let total_entries = archive.len().max(1);

    for i in 0..archive.len() {
        check_not_cancelled()?;
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Zip entry error: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let rel_opt = name
            .strip_prefix(&dot_mc)
            .or_else(|| name.strip_prefix(&plain_mc))
            .and_then(safe_rel_path);
        let Some(rel) = rel_opt else {
            continue;
        };
        let out_path = inst_dir.join(rel);
        if let Some(parent) = out_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut out = File::create(&out_path)
            .map_err(|e| format!("Failed to extract {}: {e}", out_path.display()))?;
        io::copy(&mut entry, &mut out)
            .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))?;
        extracted_files += 1;
        if extracted_files.is_multiple_of(15) {
            let _ = tx.send(WorkerEvent::Progress {
                stage: format!("Extracting Prism/MultiMC instance ({extracted_files} files)..."),
                fraction: 0.15 + 0.80 * (i as f32 / total_entries as f32),
            });
        }
    }

    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Imported '{}'!", new_profile.name),
        fraction: 1.0,
    });
    Ok(ModrinthInstallOutcome::ModpackProfileCreated {
        profile: new_profile,
        files_count: extracted_files,
    })
}

#[derive(serde::Deserialize)]
struct CurseForgeManifest {
    name: String,
    minecraft: CurseForgeMinecraft,
    #[serde(default)]
    files: Vec<CurseForgeFileRef>,
    #[serde(default = "default_cf_overrides")]
    overrides: String,
}

fn default_cf_overrides() -> String {
    "overrides".to_string()
}

#[derive(serde::Deserialize)]
struct CurseForgeMinecraft {
    version: String,
    #[serde(rename = "modLoaders", default)]
    mod_loaders: Vec<CurseForgeLoaderEntry>,
}

#[derive(serde::Deserialize)]
struct CurseForgeLoaderEntry {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(serde::Deserialize)]
struct CurseForgeFileRef {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    #[serde(default = "default_cf_required")]
    required: bool,
}

fn default_cf_required() -> bool {
    true
}

fn parse_curseforge_loader(
    mc_version: &str,
    loaders: &[CurseForgeLoaderEntry],
) -> (String, ModLoader) {
    let chosen = loaders
        .iter()
        .find(|l| l.primary)
        .or_else(|| loaders.first());
    let loader = match chosen.map(|c| c.id.as_str()) {
        Some(id) if id.starts_with("fabric-") => ModLoader::Fabric {
            loader_version: id.trim_start_matches("fabric-").to_string(),
        },
        Some(id) if id.starts_with("quilt-") => ModLoader::Quilt {
            loader_version: id.trim_start_matches("quilt-").to_string(),
        },
        Some(id) if id.starts_with("neoforge-") => ModLoader::NeoForge {
            loader_version: id.trim_start_matches("neoforge-").to_string(),
        },
        Some(id) if id.starts_with("forge-") => ModLoader::Forge {
            loader_version: id.trim_start_matches("forge-").to_string(),
        },
        _ => ModLoader::Vanilla,
    };
    (mc_version.to_string(), loader)
}

#[derive(serde::Deserialize)]
struct CfFilesApiResponse {
    data: Vec<CfFileData>,
}

#[derive(serde::Deserialize)]
struct CfFileData {
    id: u64,
    #[serde(rename = "fileName")]
    file_name: String,
    #[serde(rename = "downloadUrl")]
    download_url: Option<String>,
    #[serde(default)]
    hashes: Vec<CfHashEntry>,
    #[serde(rename = "fileLength", default)]
    file_length: Option<u64>,
}

#[derive(serde::Deserialize)]
struct CfHashEntry {
    value: String,
    algo: u8,
}

pub(crate) fn curseforge_edge_cdn_url(file_id: u64, file_name: &str) -> String {
    let major = file_id / 1000;
    let minor = file_id % 1000;
    let mut encoded = String::with_capacity(file_name.len());
    for b in file_name.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            encoded.push(*b as char);
        } else {
            use std::fmt::Write;
            let _ = write!(encoded, "%{b:02X}");
        }
    }
    format!("https://edge.forgecdn.net/files/{major}/{minor}/{encoded}")
}

/// Imports a CurseForge `.zip` modpack (`manifest.json` + `overrides/`).
/// Uses `curseforge_api_key` when configured in Settings to resolve mod download URLs via
/// `https://api.curseforge.com/v1/mods/files` (with automatic edge CDN fallback).
pub fn install_curseforge_zip(
    paths: &LauncherPaths,
    zip_path: &Path,
    username: String,
    curseforge_api_key: &str,
    tx: &Sender<WorkerEvent>,
) -> Result<ModrinthInstallOutcome, String> {
    check_not_cancelled()?;
    let file =
        File::open(zip_path).map_err(|e| format!("Failed to open {}: {e}", zip_path.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Invalid CurseForge zip: {e}"))?;

    let manifest: CurseForgeManifest = {
        let mut entry = archive
            .by_name("manifest.json")
            .map_err(|_| "CurseForge archive is missing manifest.json".to_string())?;
        let mut s = String::new();
        entry
            .read_to_string(&mut s)
            .map_err(|e| format!("Failed to read manifest.json: {e}"))?;
        serde_json::from_str(&s)
            .map_err(|e| format!("Failed to parse CurseForge manifest.json: {e}"))?
    };

    let (mc_version, loader) =
        parse_curseforge_loader(&manifest.minecraft.version, &manifest.minecraft.mod_loaders);
    let mut new_profile = Profile::new(manifest.name.clone(), mc_version, loader, username);
    new_profile.dir_name = crate::storage::unique_instance_dir_name(paths, &new_profile.name, &[]);
    let inst_dir = instance_dir(paths, &new_profile);
    let _ = fs::create_dir_all(inst_dir.join("mods"));

    let required_files: Vec<&CurseForgeFileRef> =
        manifest.files.iter().filter(|f| f.required).collect();
    let mut downloaded_count = 0usize;

    if !required_files.is_empty() {
        let (base_url, opt_key) = crate::curseforge::cf_api_endpoint(curseforge_api_key);
        let client = build_http_client()?;
        let file_ids: Vec<u64> = required_files.iter().map(|f| f.file_id).collect();
        let _ = tx.send(WorkerEvent::Progress {
            stage: format!("Resolving {} CurseForge files...", file_ids.len()),
            fraction: 0.12,
        });

        let mut all_cf_files = Vec::new();
        for chunk in file_ids.chunks(250) {
            let mut req = client
                .post(format!("{base_url}/mods/files"))
                .json(&serde_json::json!({ "fileIds": chunk }));
            if let Some(key) = opt_key {
                req = req.header("x-api-key", key);
            }
            let resp = req
                .send()
                .map_err(|e| format!("CurseForge API request failed: {e}"))?
                .error_for_status()
                .map_err(|e| format!("CurseForge API HTTP error: {e}"))?;
            let parsed: CfFilesApiResponse = resp
                .json()
                .map_err(|e| format!("Invalid CurseForge API response: {e}"))?;
            all_cf_files.extend(parsed.data);
        }

        let mut jobs = Vec::new();
        let mods_dir = inst_dir.join("mods");
        let rpacks_dir = inst_dir.join("resourcepacks");
        let shaders_dir = inst_dir.join("shaderpacks");
        let _ = fs::create_dir_all(&mods_dir);

        for cf in all_cf_files {
            let safe_name =
                sanitize_mod_filename(&cf.file_name).unwrap_or_else(|_| "mod.jar".to_string());
            let url = cf
                .download_url
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| curseforge_edge_cdn_url(cf.id, &cf.file_name));
            let sha1 = cf
                .hashes
                .iter()
                .find(|h| h.algo == 1)
                .map(|h| h.value.clone());
            let target_dir = if safe_name.to_ascii_lowercase().ends_with(".zip") {
                if safe_name.to_ascii_lowercase().contains("shader") {
                    &shaders_dir
                } else {
                    &rpacks_dir
                }
            } else {
                &mods_dir
            };
            let _ = fs::create_dir_all(target_dir);
            jobs.push(crate::mojang::DownloadJob {
                url,
                dest: target_dir.join(safe_name),
                sha1,
                size: cf.file_length,
            });
        }

        downloaded_count = jobs.len();
        let _ = required_files.first().map(|f| f.project_id);
        crate::mojang::download_parallel(&client, jobs, 20, &|done, total| {
            let _ = tx.send(WorkerEvent::Progress {
                stage: format!("CurseForge files: {done}/{total}"),
                fraction: 0.18 + 0.65 * (done as f32 / total.max(1) as f32),
            });
        })?;
    }

    let ov_trimmed = manifest.overrides.trim_matches('/');
    let ov_prefix = if ov_trimmed.is_empty() {
        "overrides/".to_string()
    } else {
        format!("{ov_trimmed}/")
    };
    let _ = tx.send(WorkerEvent::Progress {
        stage: "Extracting CurseForge overrides...".to_string(),
        fraction: 0.88,
    });
    for i in 0..archive.len() {
        check_not_cancelled()?;
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Zip entry error: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let Some(rel) = name.strip_prefix(&ov_prefix).and_then(safe_rel_path) else {
            continue;
        };
        let out_path = inst_dir.join(rel);
        if let Some(parent) = out_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut out = File::create(&out_path)
            .map_err(|e| format!("Failed to extract {}: {e}", out_path.display()))?;
        io::copy(&mut entry, &mut out)
            .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))?;
        downloaded_count += 1;
    }

    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("CurseForge modpack '{}' installed!", new_profile.name),
        fraction: 1.0,
    });
    Ok(ModrinthInstallOutcome::ModpackProfileCreated {
        profile: new_profile,
        files_count: downloaded_count,
    })
}

fn safe_rel_path(raw: &str) -> Option<PathBuf> {
    let mut clean = PathBuf::new();
    for comp in Path::new(raw).components() {
        match comp {
            std::path::Component::Normal(s) => clean.push(s),
            std::path::Component::CurDir => {}
            _ => return None,
        }
    }
    (!clean.as_os_str().is_empty()).then_some(clean)
}

/// Represents an installed mod/pack file inside an instance folder.
#[derive(Debug, Clone)]
pub struct InstalledContentItem {
    /// Actual filename on disk (e.g. `sodium.jar` or `sodium.jar.disabled`).
    pub disk_name: String,
    /// Original active filename without `.disabled` suffix (matches index entries).
    pub base_name: String,
    /// `true` if active, `false` if temporarily disabled with `.disabled`.
    pub enabled: bool,
    pub size_bytes: u64,
}

fn is_single_safe_filename(name: &str) -> bool {
    let Some(rel) = safe_rel_path(name) else {
        return false;
    };
    rel.components().count() == 1
}

/// Lists installed files in a folder (e.g. `mods`, `resourcepacks`, `shaderpacks`),
/// recognizing `.disabled` files for non-destructive toggling.
pub fn list_content_items(dir: &Path) -> Vec<InstalledContentItem> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut items: Vec<InstalledContentItem> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if !path.is_file() {
                return None;
            }
            let disk_name = path.file_name()?.to_string_lossy().to_string();
            if disk_name.ends_with(".part") || disk_name.starts_with('.') {
                return None;
            }
            let (base_name, enabled) = if let Some(stripped) = disk_name.strip_suffix(".disabled") {
                (stripped.to_string(), false)
            } else {
                (disk_name.clone(), true)
            };
            let size_bytes = e.metadata().map(|m| m.len()).unwrap_or(0);
            Some(InstalledContentItem {
                disk_name,
                base_name,
                enabled,
                size_bytes,
            })
        })
        .collect();
    items.sort_by(|a, b| {
        b.enabled
            .cmp(&a.enabled)
            .then_with(|| a.base_name.to_lowercase().cmp(&b.base_name.to_lowercase()))
    });
    items
}

/// Enables or disables a single file in `dir` by toggling the `.disabled` suffix.
/// Returns `Ok(true)` if now enabled, `Ok(false)` if now disabled.
pub fn toggle_content_file(dir: &Path, disk_name: &str) -> Result<bool, String> {
    if !is_single_safe_filename(disk_name) {
        return Err("Invalid file name".to_string());
    }
    let src = dir.join(disk_name);
    if !src.is_file() {
        return Err(format!("File not found: {disk_name}"));
    }
    let (target_name, now_enabled) = if let Some(base) = disk_name.strip_suffix(".disabled") {
        (base.to_string(), true)
    } else {
        (format!("{disk_name}.disabled"), false)
    };
    let dst = dir.join(&target_name);
    if dst.exists() {
        return Err(format!("Target file already exists: {target_name}"));
    }
    fs::rename(&src, &dst).map_err(|e| format!("Rename failed: {e}"))?;
    Ok(now_enabled)
}

/// Bulk enables (`enable = true`) or disables (`enable = false`) all content items in `dir`.
pub fn set_all_content_enabled(dir: &Path, enable: bool) -> Result<usize, String> {
    let items = list_content_items(dir);
    let mut changed = 0usize;
    for item in items {
        if item.enabled != enable {
            toggle_content_file(dir, &item.disk_name)?;
            changed += 1;
        }
    }
    Ok(changed)
}

fn toml_mentions_mod_id(toml: &str, target: &str) -> bool {
    let quoted_double = format!("\"{target}\"");
    let quoted_single = format!("'{target}'");
    toml.lines().any(|line| {
        let trimmed = line.split('#').next().unwrap_or("").trim();
        let Some((lhs, rhs)) = trimmed.split_once('=') else {
            return false;
        };
        if !lhs.trim().eq_ignore_ascii_case("modId") {
            return false;
        }
        let val = rhs.trim();
        val.eq_ignore_ascii_case(&quoted_double) || val.eq_ignore_ascii_case(&quoted_single)
    })
}

/// Inspects a mod `.jar` archive's metadata files to determine which mod loaders it supports.
pub fn detect_jar_loaders(jar_path: &Path) -> HashSet<LoaderKind> {
    let mut loaders = HashSet::new();
    let Ok(file) = File::open(jar_path) else {
        return loaders;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return loaders;
    };

    if archive.by_name("fabric.mod.json").is_ok() {
        loaders.insert(LoaderKind::Fabric);
    }
    if archive.by_name("quilt.mod.json").is_ok() {
        loaders.insert(LoaderKind::Quilt);
    }
    if archive.by_name("META-INF/neoforge.mods.toml").is_ok() {
        loaders.insert(LoaderKind::NeoForge);
    }
    if archive.by_name("mcmod.info").is_ok() {
        loaders.insert(LoaderKind::Forge);
    }
    if let Ok(mut entry) = archive.by_name("META-INF/mods.toml") {
        let mut toml = String::new();
        if io::Read::read_to_string(&mut entry, &mut toml).is_ok() {
            let has_neo = toml_mentions_mod_id(&toml, "neoforge");
            let has_forge = toml_mentions_mod_id(&toml, "forge");
            if has_neo {
                loaders.insert(LoaderKind::NeoForge);
            }
            if has_forge || !has_neo {
                loaders.insert(LoaderKind::Forge);
            }
        } else {
            loaders.insert(LoaderKind::Forge);
        }
    }

    loaders
}

/// Disables (renames to `.disabled`) any enabled mods in the instance's `mods/` directory
/// that target a different mod loader than `profile.loader`. Returns the number of disabled mods.
pub fn disable_incompatible_mods_for_loader(paths: &LauncherPaths, profile: &Profile) -> usize {
    let mods_dir = instance_dir(paths, profile).join("mods");
    let target_loader = profile.loader.kind();
    let mut disabled = 0usize;

    for item in list_content_items(&mods_dir) {
        if !item.enabled {
            continue;
        }
        let should_disable = if target_loader == LoaderKind::Vanilla {
            true
        } else {
            let detected = detect_jar_loaders(&mods_dir.join(&item.disk_name));
            !detected.is_empty() && !detected.contains(&target_loader)
        };
        if should_disable && toggle_content_file(&mods_dir, &item.disk_name).is_ok() {
            disabled += 1;
        }
    }

    disabled
}

/// Deletes a content file (enabled or `.disabled`) from `folder` and cleans `.mc-launcher-modrinth.json`.
pub fn delete_content_file(
    paths: &LauncherPaths,
    profile: &Profile,
    folder: &str,
    disk_name: &str,
) -> Result<(), String> {
    if !is_single_safe_filename(disk_name) {
        return Err("Invalid file name".to_string());
    }
    let dir = instance_dir(paths, profile).join(folder);
    let path = dir.join(disk_name);
    if path.is_file() {
        fs::remove_file(&path).map_err(|e| format!("Delete failed: {e}"))?;
    }
    let base_name = disk_name.strip_suffix(".disabled").unwrap_or(disk_name);
    let mut map = load_installed_index(paths, profile);
    let before = map.len();
    map.retain(|_, p| !(p.folder == folder && p.filename == base_name));
    if map.len() != before {
        save_installed_index(paths, profile, &map);
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct WorldEntry {
    pub folder_name: String,
    pub display_name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct WorldBackupEntry {
    pub file_name: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct ScreenshotEntry {
    pub filename: String,
    pub path: PathBuf,
    pub size_bytes: u64,
}

fn shallow_dir_size(dir: &Path, depth: usize) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0_u64;
    for entry in entries.flatten() {
        let p = entry.path();
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                total = total.saturating_add(meta.len());
            } else if meta.is_dir() && depth > 0 {
                total = total.saturating_add(shallow_dir_size(&p, depth - 1));
            }
        }
    }
    total
}

/// Lists saved worlds inside `inst_dir/saves`.
pub fn list_worlds(inst_dir: &Path) -> Vec<WorldEntry> {
    let saves_dir = inst_dir.join("saves");
    let Ok(entries) = fs::read_dir(&saves_dir) else {
        return Vec::new();
    };
    let mut worlds: Vec<WorldEntry> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if !path.is_dir() {
                return None;
            }
            let folder_name = path.file_name()?.to_string_lossy().to_string();
            if folder_name.starts_with('.') {
                return None;
            }
            let size_bytes = shallow_dir_size(&path, 2);
            Some(WorldEntry {
                display_name: folder_name.clone(),
                folder_name,
                path,
                size_bytes,
            })
        })
        .collect();
    worlds.sort_by_key(|a| a.folder_name.to_lowercase());
    worlds
}

/// Creates a `.zip` backup of `inst_dir/saves/<folder_name>` inside `inst_dir/backups/`.
pub fn backup_world(inst_dir: &Path, folder_name: &str) -> Result<PathBuf, String> {
    if !is_single_safe_filename(folder_name) {
        return Err("Refusing unsafe world path".to_string());
    }
    let world_dir = inst_dir.join("saves").join(folder_name);
    if !world_dir.is_dir() {
        return Err(format!("World folder not found: {folder_name}"));
    }
    let backups_dir = inst_dir.join("backups");
    fs::create_dir_all(&backups_dir)
        .map_err(|e| format!("Failed to create backups directory: {e}"))?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let safe_base: String = folder_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let out_path = backups_dir.join(format!("{safe_base}_{stamp}.zip"));
    let out_file = File::create(&out_path)
        .map_err(|e| format!("Failed to create world backup archive: {e}"))?;
    let mut writer = zip::ZipWriter::new(out_file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let prefix = format!("{folder_name}/");
    add_dir_to_zip(&mut writer, options, &world_dir, &world_dir, &prefix, &[])?;
    writer
        .finish()
        .map_err(|e| format!("Failed to finalize world backup: {e}"))?;
    Ok(out_path)
}

/// Deletes `inst_dir/saves/<folder_name>` safely after verifying single-component path safety.
pub fn delete_world(inst_dir: &Path, folder_name: &str) -> Result<(), String> {
    if !is_single_safe_filename(folder_name) {
        return Err("Refusing unsafe world path".to_string());
    }
    let world_dir = inst_dir.join("saves").join(folder_name);
    if !world_dir.exists() {
        return Ok(());
    }
    fs::remove_dir_all(&world_dir)
        .map_err(|e| format!("Failed to delete world '{folder_name}': {e}"))
}

/// Lists saved world backup `.zip` archives inside `inst_dir/backups/`, newest first.
pub fn list_world_backups(inst_dir: &Path) -> Vec<WorldBackupEntry> {
    let backups_dir = inst_dir.join("backups");
    let Ok(entries) = fs::read_dir(&backups_dir) else {
        return Vec::new();
    };
    let mut backups: Vec<WorldBackupEntry> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if !path.is_file() {
                return None;
            }
            let file_name = path.file_name()?.to_string_lossy().to_string();
            if file_name.starts_with('.') || !file_name.to_lowercase().ends_with(".zip") {
                return None;
            }
            let size_bytes = e.metadata().map(|m| m.len()).unwrap_or(0);
            Some(WorldBackupEntry {
                file_name,
                size_bytes,
            })
        })
        .collect();
    backups.sort_by_key(|b| std::cmp::Reverse(b.file_name.to_lowercase()));
    backups
}

fn infer_world_name_from_backup_filename(file_name: &str) -> String {
    let stem = file_name
        .strip_suffix(".zip")
        .or_else(|| file_name.strip_suffix(".ZIP"))
        .unwrap_or(file_name);
    // Strip `_YYYYMMDD-HHMMSS` suffix produced by `backup_world` when present.
    if stem.len() > 16 {
        let split_idx = stem.len() - 16;
        let suffix = &stem[split_idx..];
        if suffix.starts_with('_')
            && suffix.chars().nth(9) == Some('-')
            && suffix[1..9].chars().all(|c| c.is_ascii_digit())
            && suffix[10..16].chars().all(|c| c.is_ascii_digit())
        {
            return stem[..split_idx].to_string();
        }
    }
    stem.to_string()
}

/// Restores a saved world `.zip` backup from `inst_dir/backups/` back into `inst_dir/saves/`.
pub fn restore_world_backup(inst_dir: &Path, backup_file_name: &str) -> Result<String, String> {
    if !is_single_safe_filename(backup_file_name) {
        return Err("Refusing unsafe backup path".to_string());
    }
    let zip_path = inst_dir.join("backups").join(backup_file_name);
    let file = File::open(&zip_path).map_err(|e| format!("Failed to open backup archive: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Invalid backup archive: {e}"))?;

    let mut top_dirs: HashSet<String> = HashSet::new();
    let mut has_root_level_dat = false;
    let mut nested_level_dat_root: Option<String> = None;

    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to inspect backup entry: {e}"))?;
        let Some(rel) = safe_rel_path(entry.name()) else {
            return Err(format!("Unsafe path in backup archive: {}", entry.name()));
        };
        let comps: Vec<String> = rel
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => Some(s.to_string_lossy().to_string()),
                _ => None,
            })
            .collect();
        if comps.is_empty() {
            continue;
        }
        if comps.len() == 1 && !entry.is_dir() && comps[0] == "level.dat" {
            has_root_level_dat = true;
        }
        if comps.len() == 2 && !entry.is_dir() && comps[1] == "level.dat" {
            nested_level_dat_root = Some(comps[0].clone());
        }
        top_dirs.insert(comps[0].clone());
    }

    let (world_name, strip_root) = if !has_root_level_dat && top_dirs.len() == 1 {
        let root = top_dirs.into_iter().next().unwrap_or_default();
        if nested_level_dat_root.as_deref() != Some(root.as_str())
            || !is_single_safe_filename(&root)
        {
            return Err("Backup archive does not contain a valid world folder".to_string());
        }
        (root, true)
    } else if has_root_level_dat {
        let inferred = infer_world_name_from_backup_filename(backup_file_name);
        if !is_single_safe_filename(&inferred) {
            return Err("Could not determine safe world name from backup".to_string());
        }
        (inferred, false)
    } else {
        return Err("Backup archive is missing level.dat".to_string());
    };

    let saves_dir = inst_dir.join("saves");
    fs::create_dir_all(&saves_dir).map_err(|e| format!("Failed to create saves directory: {e}"))?;
    let staging_dir = saves_dir.join(format!(".restore_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("Failed to create staging directory: {e}"))?;

    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(e) => {
                let _ = fs::remove_dir_all(&staging_dir);
                return Err(format!("Failed to read backup entry: {e}"));
            }
        };
        let Some(rel) = safe_rel_path(entry.name()) else {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(format!("Unsafe path in backup archive: {}", entry.name()));
        };
        let out_rel = if strip_root {
            let mut comps = rel.components();
            comps.next();
            let rest: PathBuf = comps.collect();
            if rest.as_os_str().is_empty() {
                continue;
            }
            rest
        } else {
            rel
        };
        let out_path = staging_dir.join(out_rel);
        if entry.is_dir() {
            if let Err(e) = fs::create_dir_all(&out_path) {
                let _ = fs::remove_dir_all(&staging_dir);
                return Err(format!("Failed to create restored directory: {e}"));
            }
            continue;
        }
        if let Some(parent) = out_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                let _ = fs::remove_dir_all(&staging_dir);
                return Err(format!("Failed to create restored directory: {e}"));
            }
        }
        let mut out_file = match File::create(&out_path) {
            Ok(f) => f,
            Err(e) => {
                let _ = fs::remove_dir_all(&staging_dir);
                return Err(format!("Failed to extract restored file: {e}"));
            }
        };
        if let Err(e) = io::copy(&mut entry, &mut out_file) {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(format!("Failed to write restored file: {e}"));
        }
    }

    let target_world = saves_dir.join(&world_name);
    let prev_tmp = if target_world.exists() {
        let backup_existing = saves_dir.join(format!(".prev_{}", uuid::Uuid::new_v4()));
        if let Err(e) = fs::rename(&target_world, &backup_existing) {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(format!(
                "Failed to replace existing world '{world_name}': {e}"
            ));
        }
        Some(backup_existing)
    } else {
        None
    };

    if let Err(e) = fs::rename(&staging_dir, &target_world) {
        if let Some(prev) = prev_tmp {
            let _ = fs::rename(&prev, &target_world);
        }
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(format!(
            "Failed to install restored world '{world_name}': {e}"
        ));
    }
    if let Some(prev) = prev_tmp {
        let _ = fs::remove_dir_all(&prev);
    }

    Ok(world_name)
}

/// Deletes a world backup `.zip` file inside `inst_dir/backups/` safely.
pub fn delete_world_backup(inst_dir: &Path, backup_file_name: &str) -> Result<(), String> {
    if !is_single_safe_filename(backup_file_name) {
        return Err("Refusing unsafe backup path".to_string());
    }
    let path = inst_dir.join("backups").join(backup_file_name);
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(&path).map_err(|e| format!("Failed to delete backup '{backup_file_name}': {e}"))
}

/// Lists screenshots inside `inst_dir/screenshots`, newest/reverse-alphabetical first.
pub fn list_screenshots(inst_dir: &Path) -> Vec<ScreenshotEntry> {
    let shots_dir = inst_dir.join("screenshots");
    let Ok(entries) = fs::read_dir(&shots_dir) else {
        return Vec::new();
    };
    let mut shots: Vec<ScreenshotEntry> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if !path.is_file() {
                return None;
            }
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .map(str::to_lowercase)
                .unwrap_or_default();
            if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp") {
                return None;
            }
            let filename = path.file_name()?.to_string_lossy().to_string();
            let size_bytes = e.metadata().map(|m| m.len()).unwrap_or(0);
            Some(ScreenshotEntry {
                filename,
                path,
                size_bytes,
            })
        })
        .collect();
    shots.sort_by_key(|s| std::cmp::Reverse(s.filename.to_lowercase()));
    shots
}

/// Decodes a local screenshot image into a crisp 16:9 thumbnail (`240x135` max) for the UI gallery.
pub fn decode_screenshot_thumbnail(path: &Path) -> Result<IconImage, String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    let img = image::load_from_memory(&bytes)
        .map_err(|e| format!("Failed to decode screenshot: {e}"))?
        .thumbnail(240, 135)
        .to_rgba8();
    Ok(IconImage {
        width: img.width() as usize,
        height: img.height() as usize,
        rgba: img.into_raw(),
    })
}

/// Deletes a screenshot inside `inst_dir/screenshots/<filename>` safely.
pub fn delete_screenshot(inst_dir: &Path, filename: &str) -> Result<(), String> {
    if !is_single_safe_filename(filename) {
        return Err("Refusing unsafe screenshot filename".to_string());
    }
    let path = inst_dir.join("screenshots").join(filename);
    if path.is_file() {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete screenshot '{filename}': {e}"))?;
    }
    Ok(())
}

/// Lists datapacks inside `inst_dir/saves/<world_folder>/datapacks/`.
pub fn list_world_datapacks(inst_dir: &Path, world_folder: &str) -> Vec<InstalledContentItem> {
    if !is_single_safe_filename(world_folder) {
        return Vec::new();
    }
    list_content_items(&inst_dir.join("saves").join(world_folder).join("datapacks"))
}

/// Enables or disables a datapack inside `inst_dir/saves/<world_folder>/datapacks/`.
pub fn toggle_world_datapack(
    inst_dir: &Path,
    world_folder: &str,
    disk_name: &str,
) -> Result<bool, String> {
    if !is_single_safe_filename(world_folder) {
        return Err("Refusing unsafe world path".to_string());
    }
    let dir = inst_dir.join("saves").join(world_folder).join("datapacks");
    toggle_content_file(&dir, disk_name)
}

/// Deletes a datapack file inside `inst_dir/saves/<world_folder>/datapacks/`.
pub fn delete_world_datapack(
    inst_dir: &Path,
    world_folder: &str,
    disk_name: &str,
) -> Result<(), String> {
    if !is_single_safe_filename(world_folder) || !is_single_safe_filename(disk_name) {
        return Err("Refusing unsafe datapack path".to_string());
    }
    let path = inst_dir
        .join("saves")
        .join(world_folder)
        .join("datapacks")
        .join(disk_name);
    if path.is_file() {
        fs::remove_file(&path).map_err(|e| format!("Failed to delete datapack: {e}"))?;
    }
    Ok(())
}

/// Imports a local `.zip` datapack into `inst_dir/saves/<world_folder>/datapacks/`.
pub fn import_world_datapack(
    inst_dir: &Path,
    world_folder: &str,
    src_path: &Path,
) -> Result<String, String> {
    if !is_single_safe_filename(world_folder) {
        return Err("Refusing unsafe world path".to_string());
    }
    let filename = src_path
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| is_single_safe_filename(n))
        .ok_or_else(|| "Invalid datapack filename".to_string())?
        .to_string();
    let dir = inst_dir.join("saves").join(world_folder).join("datapacks");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create datapacks/: {e}"))?;
    fs::copy(src_path, dir.join(&filename))
        .map_err(|e| format!("Failed to import datapack '{filename}': {e}"))?;
    Ok(filename)
}

/// Identifies locally added mods, resourcepacks, and shaderpacks in `profile`
/// by querying Modrinth's `POST /v2/version_files` with their SHA-1 hashes.
/// Returns `(newly_identified_count, total_unmatched_checked)`.
pub fn identify_instance_content_by_hash(
    paths: &LauncherPaths,
    profile: &Profile,
) -> Result<(usize, usize), String> {
    let inst = instance_dir(paths, profile);
    let mut index = load_installed_index(paths, profile);
    let indexed_files: HashSet<String> = index
        .values()
        .map(|p| format!("{}/{}", p.folder, p.filename))
        .collect();

    let mut hash_to_meta: HashMap<String, (String, String)> = HashMap::new();
    for folder in ["mods", "resourcepacks", "shaderpacks"] {
        let dir = inst.join(folder);
        for item in list_content_items(&dir) {
            if indexed_files.contains(&format!("{folder}/{}", item.base_name)) {
                continue;
            }
            let path = dir.join(&item.disk_name);
            if let Ok((sha1_hex, _, _)) = compute_file_hashes(&path) {
                hash_to_meta.insert(
                    sha1_hex.to_ascii_lowercase(),
                    (folder.to_string(), item.base_name),
                );
            }
        }
    }

    let total_unmatched = hash_to_meta.len();
    if total_unmatched == 0 {
        return Ok((0, 0));
    }

    #[derive(serde::Deserialize)]
    struct VersionFileMatch {
        project_id: String,
        version_number: String,
    }

    #[derive(serde::Deserialize)]
    struct ProjectBrief {
        id: String,
        title: String,
        #[serde(default)]
        icon_url: Option<String>,
    }

    let client = build_http_client()?;
    let all_hashes: Vec<String> = hash_to_meta.keys().cloned().collect();
    let mut matched_versions: HashMap<String, VersionFileMatch> = HashMap::new();

    for chunk in all_hashes.chunks(100) {
        let body = serde_json::json!({
            "hashes": chunk,
            "algorithm": "sha1"
        });
        let resp = client
            .post(format!("{MODRINTH_API_BASE}/version_files"))
            .json(&body)
            .send()
            .map_err(|e| format!("Modrinth hash lookup failed: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Modrinth hash lookup HTTP error: {e}"))?;
        let part: HashMap<String, VersionFileMatch> = resp
            .json()
            .map_err(|e| format!("Invalid Modrinth version_files response: {e}"))?;
        matched_versions.extend(part);
    }

    if matched_versions.is_empty() {
        return Ok((0, total_unmatched));
    }

    let project_ids: Vec<String> = matched_versions
        .values()
        .map(|v| v.project_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let mut proj_by_id: HashMap<String, ProjectBrief> = HashMap::new();
    for chunk in project_ids.chunks(100) {
        let ids_param = serde_json::to_string(chunk).unwrap_or_else(|_| "[]".to_string());
        if let Ok(resp) = client
            .get(format!("{MODRINTH_API_BASE}/projects"))
            .query(&[("ids", ids_param.as_str())])
            .send()
            .and_then(|r| r.error_for_status())
        {
            if let Ok(projects) = resp.json::<Vec<ProjectBrief>>() {
                for p in projects {
                    proj_by_id.insert(p.id.clone(), p);
                }
            }
        }
    }

    let mut identified = 0usize;
    for (hash, ver) in matched_versions {
        let Some((folder, filename)) = hash_to_meta.get(&hash.to_ascii_lowercase()) else {
            continue;
        };
        let proj = proj_by_id.get(&ver.project_id);
        let title = proj
            .map(|p| p.title.clone())
            .unwrap_or_else(|| filename.clone());
        let icon_url = proj.and_then(|p| p.icon_url.clone());
        index.insert(
            ver.project_id,
            InstalledProject {
                title,
                filename: filename.clone(),
                folder: folder.clone(),
                version_number: ver.version_number,
                icon_url,
                source: ProjectSource::Modrinth,
            },
        );
        identified += 1;
    }

    if identified > 0 {
        save_installed_index(paths, profile, &index);
    }
    Ok((identified, total_unmatched))
}

/// Lists crash reports (`crash-reports/*.txt`) and logs (`logs/*.log`) in an instance directory.
pub fn list_instance_log_files(inst_dir: &Path) -> Vec<InstanceLogFile> {
    let mut out = Vec::new();
    for (sub, kind, allowed_ext) in [
        ("crash-reports", LogFileKind::CrashReport, "txt"),
        ("logs", LogFileKind::Log, "log"),
    ] {
        let dir = inst_dir.join(sub);
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut group = Vec::new();
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_file() {
                continue;
            }
            let ext_ok = p
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case(allowed_ext));
            if !ext_ok {
                continue;
            }
            let Some(filename) = p.file_name().and_then(|s| s.to_str()).map(str::to_string) else {
                continue;
            };
            let meta = e.metadata().ok();
            let size_bytes = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let mtime = meta
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::UNIX_EPOCH);
            group.push((
                mtime,
                InstanceLogFile {
                    filename,
                    path: p,
                    size_bytes,
                    kind,
                },
            ));
        }
        group.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.filename.cmp(&a.1.filename)));
        out.extend(group.into_iter().map(|(_, f)| f));
    }
    out
}

/// Reads an instance log or crash-report file (capping at the last 512 KB if huge).
pub fn read_instance_log_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    const MAX_BYTES: usize = 512 * 1024;
    if bytes.len() > MAX_BYTES {
        let slice = &bytes[bytes.len() - MAX_BYTES..];
        Ok(format!(
            "[... truncated to last 512 KB ...]\n{}",
            String::from_utf8_lossy(slice)
        ))
    } else {
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// Masks local OS usernames and access tokens before sharing logs externally.
pub fn sanitize_log_for_upload(raw: &str) -> String {
    let mut out = raw.to_string();
    for env_key in ["USER", "USERNAME", "LOGNAME"] {
        if let Ok(user) = std::env::var(env_key) {
            let trimmed = user.trim();
            if trimmed.len() >= 2 {
                out = out.replace(&format!("/home/{trimmed}"), "/home/<user>");
                out = out.replace(&format!("/Users/{trimmed}"), "/Users/<user>");
                out = out.replace(&format!("C:\\Users\\{trimmed}"), "C:\\Users\\<user>");
            }
        }
    }
    let mut masked_lines = Vec::new();
    for line in out.lines() {
        if let Some(idx) = line.find("--accessToken ") {
            let prefix = &line[..idx + "--accessToken ".len()];
            let rest = &line[idx + "--accessToken ".len()..];
            let suffix = rest
                .find(char::is_whitespace)
                .map(|sp| &rest[sp..])
                .unwrap_or("");
            masked_lines.push(format!("{prefix}<redacted>{suffix}"));
        } else {
            masked_lines.push(line.to_string());
        }
    }
    masked_lines.join("\n")
}

/// Uploads sanitized log/crash text to `https://api.mclo.gs/1/log` and returns the shareable URL.
pub fn upload_to_mclogs(raw_content: &str) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct McloGsResp {
        success: bool,
        url: Option<String>,
        error: Option<String>,
    }

    let sanitized = sanitize_log_for_upload(raw_content);
    if sanitized.trim().is_empty() {
        return Err("Log is empty".to_string());
    }
    let client = build_http_client()?;
    let resp = client
        .post("https://api.mclo.gs/1/log")
        .form(&[("content", sanitized.as_str())])
        .send()
        .map_err(|e| format!("mclo.gs upload failed: {e}"))?;
    let parsed: McloGsResp = resp
        .json()
        .map_err(|e| format!("Invalid mclo.gs response: {e}"))?;
    if parsed.success {
        parsed
            .url
            .ok_or_else(|| "mclo.gs succeeded without URL".to_string())
    } else {
        Err(parsed
            .error
            .unwrap_or_else(|| "mclo.gs rejected log upload".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{list_content_items, safe_rel_path, set_all_content_enabled, toggle_content_file};
    use crate::models::{LoaderKind, ModrinthProjectType, ModrinthSort};
    use std::fs;

    #[test]
    fn rejects_path_traversal() {
        assert!(safe_rel_path("../evil.jar").is_none());
        assert!(safe_rel_path("/etc/passwd").is_none());
        assert!(safe_rel_path("mods/../../x").is_none());
        assert_eq!(
            safe_rel_path("mods/sodium.jar").unwrap(),
            std::path::PathBuf::from("mods/sodium.jar")
        );
    }

    #[test]
    fn toggle_and_bulk_enable_mods() {
        let dir =
            std::env::temp_dir().join(format!("mc_launcher_mod_toggle_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("sodium.jar"), b"jar").unwrap();
        fs::write(dir.join("iris.jar.disabled"), b"jar").unwrap();

        let items = list_content_items(&dir);
        assert_eq!(items.len(), 2);
        assert!(items
            .iter()
            .any(|i| i.base_name == "sodium.jar" && i.enabled));
        assert!(items
            .iter()
            .any(|i| i.base_name == "iris.jar" && !i.enabled));

        let enabled_now = toggle_content_file(&dir, "sodium.jar").unwrap();
        assert!(!enabled_now);
        assert!(dir.join("sodium.jar.disabled").is_file());

        let count = set_all_content_enabled(&dir, true).unwrap();
        assert_eq!(count, 2);
        assert!(dir.join("sodium.jar").is_file());
        assert!(dir.join("iris.jar").is_file());

        assert!(toggle_content_file(&dir, "../evil.jar").is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn decodes_png_jpeg_gif_and_webp_icons() {
        use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
        use std::io::Cursor;

        let mut img = RgbaImage::new(80, 80);
        for p in img.pixels_mut() {
            *p = Rgba([27, 217, 106, 255]);
        }
        let dyn_img = DynamicImage::ImageRgba8(img);

        for fmt in [
            ImageFormat::Png,
            ImageFormat::Jpeg,
            ImageFormat::Gif,
            ImageFormat::WebP,
        ] {
            let mut buf = Cursor::new(Vec::new());
            if fmt == ImageFormat::Jpeg {
                DynamicImage::ImageRgb8(dyn_img.to_rgb8())
                    .write_to(&mut buf, fmt)
                    .unwrap();
            } else {
                dyn_img.write_to(&mut buf, fmt).unwrap();
            }
            let decoded = super::decode_icon_bytes(&buf.into_inner()).expect("decode icon");
            assert_eq!(decoded.width, 64);
            assert_eq!(decoded.height, 64);
            assert_eq!(decoded.rgba.len(), 64 * 64 * 4);
        }
    }

    #[test]
    fn parses_mrpack_loaders_for_all_supported_types() {
        use crate::models::ModLoader;
        use std::collections::HashMap;

        for (key, ver, expected) in [
            (
                "fabric-loader",
                "0.16.10",
                ModLoader::Fabric {
                    loader_version: "0.16.10".to_string(),
                },
            ),
            (
                "quilt-loader",
                "0.26.4",
                ModLoader::Quilt {
                    loader_version: "0.26.4".to_string(),
                },
            ),
            (
                "forge",
                "47.3.0",
                ModLoader::Forge {
                    loader_version: "47.3.0".to_string(),
                },
            ),
            (
                "neoforge",
                "21.4.120",
                ModLoader::NeoForge {
                    loader_version: "21.4.120".to_string(),
                },
            ),
        ] {
            let mut deps = HashMap::from([
                ("minecraft".to_string(), "1.21.4".to_string()),
                (key.to_string(), ver.to_string()),
            ]);
            let (mc, loader) = super::parse_mrpack_loader(&deps).unwrap();
            assert_eq!(mc, "1.21.4");
            assert_eq!(loader, expected);
            deps.remove(key);
            let (_, vanilla) = super::parse_mrpack_loader(&deps).unwrap();
            assert_eq!(vanilla, ModLoader::Vanilla);
        }
    }

    #[test]
    fn build_search_facets_enforces_current_instance_compatibility() {
        let vanilla_mods = super::SearchParams {
            query: String::new(),
            project_type: ModrinthProjectType::Mod,
            sort: ModrinthSort::Downloads,
            game_version: Some("1.21.4".to_string()),
            loader_filter: Some(LoaderKind::Vanilla),
            server_software: None,
            offset: 0,
        };
        assert_eq!(super::build_search_facets(&vanilla_mods), None);

        let fabric_mods = super::SearchParams {
            loader_filter: Some(LoaderKind::Fabric),
            ..vanilla_mods.clone()
        };
        assert_eq!(
            super::build_search_facets(&fabric_mods),
            Some(vec![
                vec!["project_type:mod".to_string()],
                vec!["versions:1.21.4".to_string()],
                vec!["categories:fabric".to_string()],
            ])
        );

        let quilt_mods = super::SearchParams {
            loader_filter: Some(LoaderKind::Quilt),
            ..vanilla_mods.clone()
        };
        assert_eq!(
            super::build_search_facets(&quilt_mods),
            Some(vec![
                vec!["project_type:mod".to_string()],
                vec!["versions:1.21.4".to_string()],
                vec!["categories:quilt".to_string()],
            ])
        );

        let vanilla_packs = super::SearchParams {
            project_type: ModrinthProjectType::Resourcepack,
            ..vanilla_mods.clone()
        };
        assert_eq!(
            super::build_search_facets(&vanilla_packs),
            Some(vec![
                vec!["project_type:resourcepack".to_string()],
                vec!["versions:1.21.4".to_string()],
            ])
        );

        let vanilla_shaders = super::SearchParams {
            project_type: ModrinthProjectType::Shader,
            ..vanilla_mods
        };
        assert_eq!(
            super::build_search_facets(&vanilla_shaders),
            Some(vec![
                vec!["project_type:shader".to_string()],
                vec!["versions:1.21.4".to_string()],
            ])
        );

        let paper_mods = super::SearchParams {
            query: String::new(),
            project_type: ModrinthProjectType::Mod,
            sort: ModrinthSort::Downloads,
            game_version: Some("1.21.4".to_string()),
            loader_filter: None,
            server_software: Some(crate::models::ServerSoftware::Paper),
            offset: 0,
        };
        assert_eq!(super::build_search_facets(&paper_mods), None);

        let folia_plugins = super::SearchParams {
            project_type: ModrinthProjectType::Plugin,
            server_software: Some(crate::models::ServerSoftware::Folia),
            ..paper_mods
        };
        assert_eq!(
            super::build_search_facets(&folia_plugins),
            Some(vec![
                vec!["project_type:plugin".to_string()],
                vec!["versions:1.21.4".to_string()],
                vec![
                    "server_side:required".to_string(),
                    "server_side:optional".to_string(),
                ],
                vec![
                    "categories:folia".to_string(),
                    "categories:paper".to_string(),
                ],
            ])
        );
    }

    fn write_test_jar(path: &std::path::Path, entries: &[(&str, &str)]) {
        use std::io::Write;
        let file = fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default();
        for (name, body) in entries {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn switching_loader_disables_incompatible_mods() {
        let root = std::env::temp_dir().join(format!(
            "mc_launcher_loader_switch_{}",
            uuid::Uuid::new_v4()
        ));
        let paths = crate::models::LauncherPaths {
            versions_dir: root.join("versions"),
            libraries_dir: root.join("libraries"),
            assets_dir: root.join("assets"),
            instances_dir: root.join("instances"),
            servers_dir: root.join("servers"),
            profiles_file: root.join("profiles.json"),
            servers_file: root.join("servers.json"),
            accounts_file: root.join("accounts.json"),
            root: root.clone(),
        };
        let mut profile = crate::models::Profile::new(
            "Test".to_string(),
            "1.21.4".to_string(),
            crate::models::ModLoader::Fabric {
                loader_version: String::new(),
            },
            "Player".to_string(),
        );
        let mods_dir = paths.instances_dir.join(&profile.id).join("mods");
        fs::create_dir_all(&mods_dir).unwrap();

        write_test_jar(
            &mods_dir.join("sodium-fabric.jar"),
            &[("fabric.mod.json", "{}")],
        );
        write_test_jar(
            &mods_dir.join("jei-forge.jar"),
            &[(
                "META-INF/mods.toml",
                "[[dependencies.jei]]\nmodId = \"forge\"\n",
            )],
        );
        write_test_jar(
            &mods_dir.join("iris-neoforge.jar"),
            &[("META-INF/neoforge.mods.toml", "modLoader=\"javafml\"\n")],
        );
        write_test_jar(
            &mods_dir.join("architectury-multi.jar"),
            &[
                ("fabric.mod.json", "{}"),
                (
                    "META-INF/mods.toml",
                    "[[dependencies.arch]]\nmodId=\"forge\"\n",
                ),
            ],
        );

        // Switch Fabric -> Forge: Fabric-only & NeoForge-only disabled; Forge & multi stay active.
        profile.loader = crate::models::ModLoader::Forge {
            loader_version: String::new(),
        };
        let disabled_on_forge = super::disable_incompatible_mods_for_loader(&paths, &profile);
        assert_eq!(disabled_on_forge, 2);
        assert!(mods_dir.join("sodium-fabric.jar.disabled").is_file());
        assert!(mods_dir.join("iris-neoforge.jar.disabled").is_file());
        assert!(mods_dir.join("jei-forge.jar").is_file());
        assert!(mods_dir.join("architectury-multi.jar").is_file());

        // Switch Forge -> Vanilla: remaining enabled mods are disabled.
        profile.loader = crate::models::ModLoader::Vanilla;
        let disabled_on_vanilla = super::disable_incompatible_mods_for_loader(&paths, &profile);
        assert_eq!(disabled_on_vanilla, 2);
        assert!(mods_dir.join("jei-forge.jar.disabled").is_file());
        assert!(mods_dir.join("architectury-multi.jar.disabled").is_file());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn world_backup_and_delete_rejects_traversal() {
        let root = std::env::temp_dir().join(format!(
            "mc_launcher_world_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let world_dir = root.join("saves").join("SurvivalOne");
        fs::create_dir_all(&world_dir).unwrap();
        fs::write(world_dir.join("level.dat"), b"test-level-data").unwrap();

        assert!(super::backup_world(&root, "../escape").is_err());
        assert!(super::delete_world(&root, "../escape").is_err());

        let zip_path = super::backup_world(&root, "SurvivalOne").unwrap();
        assert!(zip_path.is_file());
        let file = fs::File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        assert!(archive.by_name("SurvivalOne/level.dat").is_ok());

        super::delete_world(&root, "SurvivalOne").unwrap();
        assert!(!world_dir.exists());

        let backups = super::list_world_backups(&root);
        assert_eq!(backups.len(), 1);
        let backup_name = backups[0].file_name.clone();
        assert!(super::restore_world_backup(&root, "../escape.zip").is_err());
        assert!(super::delete_world_backup(&root, "../escape.zip").is_err());

        let restored = super::restore_world_backup(&root, &backup_name).unwrap();
        assert_eq!(restored, "SurvivalOne");
        assert_eq!(
            fs::read(world_dir.join("level.dat")).unwrap(),
            b"test-level-data"
        );

        super::delete_world_backup(&root, &backup_name).unwrap();
        assert!(super::list_world_backups(&root).is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn multimc_curseforge_and_datapack_imports_work() {
        use crate::models::{LauncherPaths, ModLoader, ModrinthInstallOutcome};
        use std::io::Write;

        let root =
            std::env::temp_dir().join(format!("mc_launcher_pack_formats_{}", uuid::Uuid::new_v4()));
        let paths = LauncherPaths {
            versions_dir: root.join("versions"),
            libraries_dir: root.join("libraries"),
            assets_dir: root.join("assets"),
            instances_dir: root.join("instances"),
            servers_dir: root.join("servers"),
            profiles_file: root.join("profiles.json"),
            servers_file: root.join("servers.json"),
            accounts_file: root.join("accounts.json"),
            root: root.clone(),
        };
        fs::create_dir_all(&paths.instances_dir).unwrap();
        let (tx, _rx) = std::sync::mpsc::channel();

        // 1. Prism/MultiMC .zip
        let mmc_zip = root.join("prism_pack.zip");
        {
            let f = fs::File::create(&mmc_zip).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("MyPack/mmc-pack.json", opts).unwrap();
            zw.write_all(
                br#"{"components":[{"uid":"net.minecraft","version":"1.21.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.16.9"}]}"#,
            )
            .unwrap();
            zw.start_file("MyPack/instance.cfg", opts).unwrap();
            zw.write_all(b"InstanceType=OneSix\nname=Prism Cobblemon\n")
                .unwrap();
            zw.start_file("MyPack/.minecraft/config/test.toml", opts)
                .unwrap();
            zw.write_all(b"enabled = true\n").unwrap();
            zw.finish().unwrap();
        }
        let outcome =
            super::import_modpack_archive(&paths, &mmc_zip, "Steve".into(), "", &tx).unwrap();
        match outcome {
            ModrinthInstallOutcome::ModpackProfileCreated {
                profile,
                files_count,
            } => {
                assert_eq!(profile.name, "Prism Cobblemon");
                assert_eq!(profile.version_id, "1.21.1");
                assert_eq!(
                    profile.loader,
                    ModLoader::Fabric {
                        loader_version: "0.16.9".into()
                    }
                );
                assert_eq!(files_count, 1);
                let inst = crate::storage::instance_dir(&paths, &profile);
                assert_eq!(
                    fs::read_to_string(inst.join("config/test.toml")).unwrap(),
                    "enabled = true\n"
                );
            }
            _ => panic!("Expected ModpackProfileCreated"),
        }

        // 2. CurseForge .zip (with overrides)
        let cf_zip = root.join("cf_pack.zip");
        {
            let f = fs::File::create(&cf_zip).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("manifest.json", opts).unwrap();
            zw.write_all(
                br#"{"name":"ATM Lite","minecraft":{"version":"1.20.1","modLoaders":[{"id":"forge-47.3.0","primary":true}]},"files":[],"overrides":"overrides"}"#,
            )
            .unwrap();
            zw.start_file("overrides/options.txt", opts).unwrap();
            zw.write_all(b"fov:1.0\n").unwrap();
            zw.finish().unwrap();
        }
        let cf_outcome =
            super::import_modpack_archive(&paths, &cf_zip, "Steve".into(), "", &tx).unwrap();
        match cf_outcome {
            ModrinthInstallOutcome::ModpackProfileCreated { profile, .. } => {
                assert_eq!(profile.name, "ATM Lite");
                assert_eq!(profile.version_id, "1.20.1");
                assert_eq!(
                    profile.loader,
                    ModLoader::Forge {
                        loader_version: "47.3.0".into()
                    }
                );
                let inst = crate::storage::instance_dir(&paths, &profile);
                assert_eq!(
                    fs::read_to_string(inst.join("options.txt")).unwrap(),
                    "fov:1.0\n"
                );

                // 3. World datapacks roundtrip
                fs::create_dir_all(inst.join("saves/WorldA")).unwrap();
                let dp_src = root.join("terralith.zip");
                fs::write(&dp_src, b"pk-datapack").unwrap();
                super::import_world_datapack(&inst, "WorldA", &dp_src).unwrap();
                assert_eq!(super::list_world_datapacks(&inst, "WorldA").len(), 1);
                assert!(!super::toggle_world_datapack(&inst, "WorldA", "terralith.zip").unwrap());
                super::delete_world_datapack(&inst, "WorldA", "terralith.zip.disabled").unwrap();
                assert!(super::list_world_datapacks(&inst, "WorldA").is_empty());
            }
            _ => panic!("Expected CurseForge ModpackProfileCreated"),
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn sanitize_log_and_mrpack_exclusions_work() {
        let sanitized = super::sanitize_log_for_upload(
            "Starting with --accessToken secret_abc_123 --uuid 0000",
        );
        assert!(!sanitized.contains("secret_abc_123"));
        assert!(sanitized.contains("--accessToken <redacted> --uuid 0000"));

        let pats = super::parse_exclude_patterns("*.bak, config/secret.toml, journey-map/");
        assert!(super::path_matches_exclude("mods/test.jar.bak", &pats));
        assert!(super::path_matches_exclude("config/secret.toml", &pats));
        assert!(super::path_matches_exclude("journey-map/data.bin", &pats));
        assert!(!super::path_matches_exclude("config/sodium.json", &pats));

        let root = std::env::temp_dir().join(format!(
            "mc_launcher_logs_exp_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let paths = crate::models::LauncherPaths {
            versions_dir: root.join("versions"),
            libraries_dir: root.join("libraries"),
            assets_dir: root.join("assets"),
            instances_dir: root.join("instances"),
            servers_dir: root.join("servers"),
            profiles_file: root.join("profiles.json"),
            servers_file: root.join("servers.json"),
            accounts_file: root.join("accounts.json"),
            root: root.clone(),
        };
        fs::create_dir_all(&paths.instances_dir).unwrap();
        let profile = crate::models::Profile::new(
            "ExportTest".into(),
            "1.21.4".into(),
            crate::models::ModLoader::Vanilla,
            "P".into(),
        );
        let inst = crate::storage::instance_dir(&paths, &profile);
        fs::create_dir_all(inst.join("config")).unwrap();
        fs::create_dir_all(inst.join("saves/world1")).unwrap();
        fs::create_dir_all(inst.join("crash-reports")).unwrap();
        fs::create_dir_all(inst.join("logs")).unwrap();
        fs::write(inst.join("config/keep.toml"), b"a=1").unwrap();
        fs::write(inst.join("config/secret.toml"), b"token=x").unwrap();
        fs::write(inst.join("options.txt"), b"fov:1.0").unwrap();
        fs::write(inst.join("saves/world1/level.dat"), b"world").unwrap();
        fs::write(inst.join("crash-reports/crash-1.txt"), b"crash").unwrap();
        fs::write(inst.join("logs/latest.log"), b"log").unwrap();

        let logs = super::list_instance_log_files(&inst);
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].kind, crate::models::LogFileKind::CrashReport);
        assert_eq!(logs[1].kind, crate::models::LogFileKind::Log);

        let mut opts = crate::models::MrpackExportOptions::default();
        opts.include_options_txt = false;
        opts.include_saves = false;
        opts.extra_excludes = "config/secret.toml".to_string();
        let mrpack = super::export_instance_mrpack_with_options(&paths, &profile, &opts).unwrap();
        let zr = zip::ZipArchive::new(fs::File::open(&mrpack).unwrap()).unwrap();
        let names: Vec<String> = zr.file_names().map(str::to_string).collect();
        assert!(names.contains(&"overrides/config/keep.toml".to_string()));
        assert!(!names.contains(&"overrides/config/secret.toml".to_string()));
        assert!(!names.contains(&"overrides/options.txt".to_string()));
        assert!(!names.iter().any(|n| n.starts_with("overrides/saves/")));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn sanitize_mod_filename_and_content_dir_and_cdn_url() {
        assert_eq!(
            super::sanitize_mod_filename("sodium-fabric-0.6.13+mc1.21.4.jar").unwrap(),
            "sodium-fabric-0.6.13+mc1.21.4.jar"
        );
        assert_eq!(
            super::sanitize_mod_filename("[1.20.1..1.20.4]-mod.jar").unwrap(),
            "[1.20.1..1.20.4]-mod.jar"
        );
        assert_eq!(
            super::sanitize_mod_filename("mod:name*test?.jar ").unwrap(),
            "mod_name_test_.jar"
        );
        assert_eq!(super::sanitize_mod_filename("CON.jar").unwrap(), "_CON.jar");
        assert!(super::sanitize_mod_filename("../evil.jar").is_err());
        assert!(super::sanitize_mod_filename("sub/mod.jar").is_err());

        assert_eq!(
            super::curseforge_edge_cdn_url(5123456, "My Mod+1.20 [Fabric].jar"),
            "https://edge.forgecdn.net/files/5123/456/My%20Mod%2B1.20%20%5BFabric%5D.jar"
        );

        let root =
            std::env::temp_dir().join(format!("mc_launcher_content_dir_{}", uuid::Uuid::new_v4()));
        let paths = crate::models::LauncherPaths {
            versions_dir: root.join("versions"),
            libraries_dir: root.join("libraries"),
            assets_dir: root.join("assets"),
            instances_dir: root.join("instances"),
            servers_dir: root.join("servers"),
            profiles_file: root.join("profiles.json"),
            servers_file: root.join("servers.json"),
            accounts_file: root.join("accounts.json"),
            root: root.clone(),
        };
        let mut profile = crate::models::Profile::new(
            "FreshFabric".into(),
            "1.21.4".into(),
            crate::models::ModLoader::Fabric {
                loader_version: String::new(),
            },
            "Player".into(),
        );
        profile.dir_name = "FreshFabric".into();

        let (inst, folder_rel, dest_dir) =
            super::ensure_instance_content_dir(&paths, &profile, ModrinthProjectType::Mod).unwrap();
        assert_eq!(folder_rel, "mods");
        assert!(inst.join("mods").is_dir());
        assert!(dest_dir.is_dir());

        let _ = fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod profile_backup_tests {
    use super::*;

    fn temp_paths(tag: &str) -> (LauncherPaths, PathBuf) {
        let root = std::env::temp_dir().join(format!("sl-export-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let paths = LauncherPaths {
            root: root.clone(),
            versions_dir: root.join("versions"),
            libraries_dir: root.join("libraries"),
            assets_dir: root.join("assets"),
            instances_dir: root.join("instances"),
            servers_dir: root.join("servers"),
            profiles_file: root.join("profiles.json"),
            servers_file: root.join("servers.json"),
            accounts_file: root.join("accounts.json"),
        };
        (paths, root)
    }

    #[test]
    fn export_and_import_profile_archive_roundtrip() {
        let (paths, root) = temp_paths("roundtrip");
        let mut profile = Profile::new(
            "My Pack".to_string(),
            "1.20.1".to_string(),
            ModLoader::Fabric {
                loader_version: "0.15.0".to_string(),
            },
            "Steve".to_string(),
        );
        profile.max_ram_mb = 4096;
        profile.min_ram_mb = 1024;
        profile.dir_name = "My Pack".to_string();

        let inst = instance_dir(&paths, &profile);
        std::fs::create_dir_all(inst.join("mods")).unwrap();
        std::fs::write(inst.join("mods").join("sodium.jar"), b"modjar").unwrap();
        std::fs::create_dir_all(inst.join("config")).unwrap();
        std::fs::write(inst.join("config").join("opts.toml"), b"k = 1").unwrap();
        std::fs::create_dir_all(inst.join("logs")).unwrap();
        std::fs::write(inst.join("logs").join("latest.log"), b"skip me").unwrap();

        let archive = export_profile_archive(&paths, &profile, None).unwrap();
        assert!(archive.is_file());
        assert!(is_profile_backup_archive(&archive));

        let (tx, _rx) = std::sync::mpsc::channel();
        let (imported, restored) = import_profile_archive(&paths, &archive, &tx).unwrap();
        assert_ne!(imported.id, profile.id);
        assert_eq!(imported.version_id, "1.20.1");
        assert_eq!(imported.loader.kind(), LoaderKind::Fabric);
        assert_eq!(imported.max_ram_mb, 4096);
        assert!(restored >= 2);

        let new_inst = instance_dir(&paths, &imported);
        assert!(new_inst.join("mods").join("sodium.jar").is_file());
        assert_eq!(
            std::fs::read(new_inst.join("config").join("opts.toml")).unwrap(),
            b"k = 1"
        );
        assert!(!new_inst.join("logs").join("latest.log").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn import_rejects_non_backup_zip() {
        let (paths, root) = temp_paths("reject");
        let bogus = root.join("bogus.zip");
        let file = std::fs::File::create(&bogus).unwrap();
        let mut w = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default();
        w.start_file("modrinth.index.json", opts).unwrap();
        io::Write::write_all(&mut w, b"{}").unwrap();
        w.finish().unwrap();
        assert!(!is_profile_backup_archive(&bogus));
        let (tx, _rx) = std::sync::mpsc::channel();
        assert!(import_profile_archive(&paths, &bogus, &tx).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
