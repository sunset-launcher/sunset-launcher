use crate::models::{
    IconImage, InstalledProject, LauncherPaths, LoaderKind, ModLoader, ModrinthInstallOutcome,
    ModrinthProjectType, ModrinthSearchHit, ModrinthSearchPage, ModrinthSearchResponse,
    ModrinthSort, ModrinthVersion, MrpackIndex, Profile, WorkerEvent,
};
use crate::mojang::{build_async_http_client, build_http_client, download_file};
use crate::storage::instance_dir;
use reqwest::blocking::Client;
use reqwest::StatusCode;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io;
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
    pub offset: usize,
}

fn icon_http_client() -> Result<&'static Client, String> {
    static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();
    match CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent("svarka-launcher/0.1.0")
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
        && params.loader_filter == Some(LoaderKind::Vanilla)
    {
        return None;
    }

    let mut facets: Vec<Vec<String>> =
        vec![vec![format!("project_type:{}", params.project_type.api_str())]];
    if let Some(ver) = params.game_version.as_deref().filter(|v| !v.trim().is_empty()) {
        facets.push(vec![format!("versions:{}", ver.trim())]);
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
                                    let list = resp
                                        .json::<Vec<serde::de::IgnoredAny>>()
                                        .await
                                        .ok();
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

        if raw_len < SEARCH_PAGE_SIZE
            || (cur_offset as u64) >= total_hits
            || collected.len() >= 10
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
        ModrinthProjectType::Resourcepack => {
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
            let preferred = loader
                .modrinth_slug()
                .map(|s| format!("[\"{s}\"]"));
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

fn save_installed_index(
    paths: &LauncherPaths,
    profile: &Profile,
    map: &HashMap<String, InstalledProject>,
) {
    if let Ok(json) = serde_json::to_string_pretty(map) {
        let _ = fs::write(index_path(paths, profile), json);
    }
}

/// Downloads and installs a Modrinth mod, resourcepack, or modpack (.mrpack).
pub fn install_project(
    paths: &LauncherPaths,
    profile: &Profile,
    project_id: &str,
    project_type: ModrinthProjectType,
    title: &str,
    icon_url: Option<&str>,
    tx: &Sender<WorkerEvent>,
) -> Result<ModrinthInstallOutcome, String> {
    let client = build_http_client()?;
    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Finding version for {title}..."),
        fraction: 0.1,
    });

    match project_type {
        ModrinthProjectType::Mod | ModrinthProjectType::Resourcepack => {
            if project_type == ModrinthProjectType::Mod && !profile.loader.is_modded() {
                return Err("Mods require a modded loader (Fabric, Quilt, Forge, or NeoForge). Switch loader in Instance settings.".to_string());
            }

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
                dest_folder: folder_for(project_type).to_string(),
                dependencies_installed: deps_installed,
            })
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
    let mut queue =
        required_dependencies(client, root_project_id, &profile.version_id, &profile.loader)?;
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

fn folder_for(project_type: ModrinthProjectType) -> &'static str {
    match project_type {
        ModrinthProjectType::Resourcepack => "resourcepacks",
        _ => "mods",
    }
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
    let folder = folder_for(project_type);
    let dest_dir = instance_dir(paths, profile).join(folder);
    fs::create_dir_all(&dest_dir).map_err(|e| format!("Failed to create {folder}: {e}"))?;

    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Downloading {}...", file.filename),
        fraction: 0.5,
    });
    download_file(
        client,
        &file.url,
        &dest_dir.join(&file.filename),
        file.hashes.sha1.as_deref(),
    )?;

    // Replace an older file of the same project (updating a mod).
    if let Some(old) = index.get(project_id) {
        if old.filename != file.filename {
            let _ = fs::remove_file(dest_dir.join(&old.filename));
            let _ = fs::remove_file(dest_dir.join(format!("{}.disabled", old.filename)));
        }
    }
    index.insert(
        project_id.to_string(),
        InstalledProject {
            title: title.to_string(),
            filename: file.filename.clone(),
            folder: folder.to_string(),
            version_number: version.version_number.clone(),
            icon_url,
        },
    );
    Ok(file.filename.clone())
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
                list = get_versions(&client, project_id, Some(game_version), Some("[\"fabric\"]"))?;
            }
            Ok(list)
        }
        ModrinthProjectType::Resourcepack => {
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
    let client = build_http_client()?;
    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Installing {title} ({})...", version.version_number),
        fraction: 0.15,
    });
    match project_type {
        ModrinthProjectType::Mod | ModrinthProjectType::Resourcepack => {
            if project_type == ModrinthProjectType::Mod && !profile.loader.is_modded() {
                return Err("Mods require a modded loader (Fabric, Quilt, Forge, or NeoForge). Switch loader in Instance settings.".to_string());
            }
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
                dest_folder: folder_for(project_type).to_string(),
                dependencies_installed: deps_installed,
            })
        }
        ModrinthProjectType::Modpack => {
            install_modpack_version(&client, paths, profile, project_id, version, tx)
        }
    }
}

/// Checks and updates all Modrinth-tracked mods/resourcepacks in `profile`.
pub fn update_all_installed_projects(
    paths: &LauncherPaths,
    profile: &Profile,
    tx: &Sender<WorkerEvent>,
) -> Result<Vec<String>, String> {
    let client = build_http_client()?;
    let mut index = load_installed_index(paths, profile);
    let entries: Vec<(String, InstalledProject)> =
        index.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let total = entries.len().max(1);
    let mut updated_titles = Vec::new();

    for (i, (project_id, entry)) in entries.into_iter().enumerate() {
        let project_type = if entry.folder == "resourcepacks" {
            ModrinthProjectType::Resourcepack
        } else {
            ModrinthProjectType::Mod
        };
        let _ = tx.send(WorkerEvent::Progress {
            stage: format!("Checking updates ({}/{}): {}...", i + 1, total, entry.title),
            fraction: (i as f32 + 0.3) / total as f32,
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

/// Imports a dropped local `.jar`, `.zip`, or `.mrpack` file into the active instance.
pub fn import_local_file(
    paths: &LauncherPaths,
    profile: &Profile,
    src_path: &Path,
    active_folder: &str,
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
    let Some(version) =
        get_versions(client, project_id, Some(game_version), Some(&loader_param))?
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

/// Exports an instance's loader metadata, mods, resourcepacks, shaderpacks, and configs
/// into a portable `.mrpack` archive under `paths.root/exports/`.
pub fn export_instance_mrpack(paths: &LauncherPaths, profile: &Profile) -> Result<PathBuf, String> {
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

    let index = serde_json::json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.0.0",
        "name": profile.name,
        "summary": format!("Exported from Svarka Launcher ({})", profile.version_id),
        "files": [],
        "dependencies": deps,
    });

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

    fn add_dir_to_overrides(
        writer: &mut zip::ZipWriter<File>,
        options: zip::write::SimpleFileOptions,
        base_inst: &Path,
        dir: &Path,
    ) -> Result<(), String> {
        let Ok(entries) = fs::read_dir(dir) else {
            return Ok(());
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                add_dir_to_overrides(writer, options, base_inst, &p)?;
            } else if p.is_file() {
                if let Ok(rel) = p.strip_prefix(base_inst) {
                    let zip_name = format!(
                        "overrides/{}",
                        rel.to_string_lossy().replace('\\', "/")
                    );
                    writer
                        .start_file(&zip_name, options)
                        .map_err(|e| format!("Failed to add {zip_name}: {e}"))?;
                    let mut src = File::open(&p)
                        .map_err(|e| format!("Failed to open {}: {e}", p.display()))?;
                    io::copy(&mut src, writer)
                        .map_err(|e| format!("Failed to copy {}: {e}", p.display()))?;
                }
            }
        }
        Ok(())
    }

    for sub in ["mods", "resourcepacks", "shaderpacks", "config"] {
        let dir = inst.join(sub);
        if dir.is_dir() {
            add_dir_to_overrides(&mut zip_writer, options, &inst, &dir)?;
        }
    }
    let options_txt = inst.join("options.txt");
    if options_txt.is_file() {
        zip_writer
            .start_file("overrides/options.txt", options)
            .map_err(|e| format!("Failed to add overrides/options.txt: {e}"))?;
        let mut src =
            File::open(&options_txt).map_err(|e| format!("Failed to open options.txt: {e}"))?;
        io::copy(&mut src, &mut zip_writer)
            .map_err(|e| format!("Failed to copy options.txt: {e}"))?;
    }

    zip_writer
        .finish()
        .map_err(|e| format!("Failed to finalize .mrpack: {e}"))?;
    Ok(out_path)
}

fn install_mrpack(
    paths: &LauncherPaths,
    mrpack_path: &Path,
    username: String,
    tx: &Sender<WorkerEvent>,
) -> Result<ModrinthInstallOutcome, String> {
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

    let new_profile = Profile::new(index.name.clone(), mc_version, loader, username);
    let inst_dir = instance_dir(paths, &new_profile);

    let client = build_http_client()?;
    let client_files: Vec<_> = index
        .files
        .iter()
        .filter(|f| {
            f.env
                .as_ref()
                .and_then(|e| e.client.as_deref())
                .map_or(true, |c| c != "unsupported")
        })
        .collect();

    let jobs: Vec<crate::mojang::DownloadJob> = client_files
        .iter()
        .filter_map(|mf| {
            let rel = safe_rel_path(&mf.path)?;
            Some(crate::mojang::DownloadJob {
                url: mf.downloads.first()?.clone(),
                dest: inst_dir.join(rel),
                sha1: mf.hashes.sha1.clone(),
                size: None,
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

    // Extract overrides/ then client-overrides/ (the latter wins).
    let _ = tx.send(WorkerEvent::Progress {
        stage: "Extracting overrides...".to_string(),
        fraction: 0.90,
    });
    for prefix in ["overrides/", "client-overrides/"] {
        for i in 0..archive.len() {
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
    worlds.sort_by(|a, b| a.folder_name.to_lowercase().cmp(&b.folder_name.to_lowercase()));
    worlds
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
    shots.sort_by(|a, b| b.filename.to_lowercase().cmp(&a.filename.to_lowercase()));
    shots
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
        let dir = std::env::temp_dir().join(format!("mc_launcher_mod_toggle_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("sodium.jar"), b"jar").unwrap();
        fs::write(dir.join("iris.jar.disabled"), b"jar").unwrap();

        let items = list_content_items(&dir);
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| i.base_name == "sodium.jar" && i.enabled));
        assert!(items.iter().any(|i| i.base_name == "iris.jar" && !i.enabled));

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
            ..vanilla_mods
        };
        assert_eq!(
            super::build_search_facets(&vanilla_packs),
            Some(vec![
                vec!["project_type:resourcepack".to_string()],
                vec!["versions:1.21.4".to_string()],
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
            profiles_file: root.join("profiles.json"),
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

        write_test_jar(&mods_dir.join("sodium-fabric.jar"), &[("fabric.mod.json", "{}")]);
        write_test_jar(
            &mods_dir.join("jei-forge.jar"),
            &[("META-INF/mods.toml", "[[dependencies.jei]]\nmodId = \"forge\"\n")],
        );
        write_test_jar(
            &mods_dir.join("iris-neoforge.jar"),
            &[("META-INF/neoforge.mods.toml", "modLoader=\"javafml\"\n")],
        );
        write_test_jar(
            &mods_dir.join("architectury-multi.jar"),
            &[
                ("fabric.mod.json", "{}"),
                ("META-INF/mods.toml", "[[dependencies.arch]]\nmodId=\"forge\"\n"),
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
}
