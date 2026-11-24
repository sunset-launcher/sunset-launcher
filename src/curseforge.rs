use crate::models::{
    DedicatedServer, InstalledProject, LauncherPaths, LoaderKind, ModLoader, ModrinthFile,
    ModrinthHashes, ModrinthInstallOutcome, ModrinthProjectType, ModrinthSearchHit,
    ModrinthSearchPage, ModrinthSort, ModrinthVersion, Profile, ProjectSource, WorkerEvent,
};
use crate::modrinth::{
    curseforge_edge_cdn_url, default_datapack_rel_folder, ensure_instance_content_dir, folder_for,
    install_curseforge_zip, load_installed_index, load_server_installed_index,
    sanitize_mod_filename, save_installed_index, save_server_installed_index, SearchParams,
    SEARCH_PAGE_SIZE,
};
use crate::mojang::{build_http_client, check_not_cancelled, download_file};
use crate::servers::server_supports_plugins_live;
use crate::storage::{instance_dir, server_dir};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::mpsc::Sender;

const CF_OFFICIAL_API_BASE: &str = "https://api.curseforge.com/v1";
const CF_KEYLESS_API_BASE: &str = "https://api.curse.tools/v1/cf";
const CF_MINECRAFT_GAME_ID: u64 = 432;

pub(crate) fn cf_api_endpoint(api_key: &str) -> (&'static str, Option<&str>) {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        (CF_KEYLESS_API_BASE, None)
    } else {
        (CF_OFFICIAL_API_BASE, Some(trimmed))
    }
}

fn cf_apply_auth(
    req: reqwest::blocking::RequestBuilder,
    api_key: Option<&str>,
) -> reqwest::blocking::RequestBuilder {
    match api_key {
        Some(key) => req.header("x-api-key", key),
        None => req,
    }
}

fn cf_class_id(project_type: ModrinthProjectType) -> u64 {
    match project_type {
        ModrinthProjectType::Mod => 6,
        ModrinthProjectType::Resourcepack => 12,
        ModrinthProjectType::Shader => 6552,
        ModrinthProjectType::Modpack => 4471,
        ModrinthProjectType::Datapack => 6945,
        ModrinthProjectType::Plugin => 5,
    }
}

pub(crate) fn cf_loader_type(loader: LoaderKind) -> Option<u8> {
    match loader {
        LoaderKind::Forge => Some(1),
        LoaderKind::Fabric => Some(4),
        LoaderKind::Quilt => Some(5),
        LoaderKind::NeoForge => Some(6),
        LoaderKind::Vanilla => None,
    }
}

fn cf_sort_field(sort: ModrinthSort) -> u8 {
    match sort {
        ModrinthSort::Relevance => 2, // Popularity
        ModrinthSort::Downloads => 6, // TotalDownloads
        ModrinthSort::Updated => 3,   // LastUpdated
        ModrinthSort::Newest => 11,   // ReleasedDate
    }
}

#[derive(Deserialize)]
struct CfSearchResp {
    data: Vec<CfModEntry>,
    pagination: CfPagination,
}

#[derive(Deserialize)]
struct CfSingleModResp {
    data: CfModEntry,
}

#[derive(Deserialize)]
struct CfPagination {
    #[serde(default)]
    index: usize,
    #[serde(rename = "resultCount", default)]
    result_count: usize,
    #[serde(rename = "totalCount", default)]
    total_count: u64,
}

#[derive(Deserialize)]
struct CfModEntry {
    id: u64,
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(rename = "downloadCount", default)]
    download_count: f64,
    #[serde(default)]
    authors: Vec<CfAuthor>,
    #[serde(default)]
    logo: Option<CfLogo>,
    #[serde(default)]
    categories: Vec<CfCategory>,
    #[serde(default)]
    links: Option<CfLinks>,
}

#[derive(Deserialize)]
struct CfAuthor {
    name: String,
}

#[derive(Deserialize)]
struct CfLogo {
    #[serde(rename = "thumbnailUrl", default)]
    thumbnail_url: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Deserialize)]
struct CfCategory {
    name: String,
}

#[derive(Deserialize)]
struct CfLinks {
    #[serde(rename = "websiteUrl", default)]
    website_url: Option<String>,
}

#[derive(Deserialize)]
struct CfModFilesResp {
    data: Vec<CfModFile>,
}

#[derive(Deserialize)]
struct CfModFile {
    id: u64,
    #[serde(rename = "displayName", default)]
    display_name: String,
    #[serde(rename = "fileName")]
    file_name: String,
    #[serde(rename = "releaseType", default)]
    release_type: u8,
    #[serde(rename = "fileDate", default)]
    file_date: String,
    #[serde(rename = "downloadUrl", default)]
    download_url: Option<String>,
    #[serde(default)]
    hashes: Vec<CfFileHash>,
    #[serde(default)]
    dependencies: Vec<CfFileDep>,
}

#[derive(Deserialize)]
struct CfFileHash {
    value: String,
    algo: u8,
}

#[derive(Deserialize)]
struct CfFileDep {
    #[serde(rename = "modId")]
    mod_id: u64,
    #[serde(rename = "relationType")]
    relation_type: u8,
}

pub fn parse_cf_project_id(project_id: &str) -> Option<u64> {
    project_id.strip_prefix("cf:")?.parse::<u64>().ok()
}

pub fn search_curseforge_projects(
    api_key: &str,
    params: &SearchParams,
) -> Result<ModrinthSearchPage, String> {
    let (base_url, opt_key) = cf_api_endpoint(api_key);
    let client = build_http_client()?;

    let class_id = cf_class_id(params.project_type).to_string();
    let game_id = CF_MINECRAFT_GAME_ID.to_string();
    let sort_field = cf_sort_field(params.sort).to_string();
    let page_size = SEARCH_PAGE_SIZE.to_string();
    let index = params.offset.to_string();

    let mut query_pairs: Vec<(&str, String)> = vec![
        ("gameId", game_id),
        ("classId", class_id),
        ("sortField", sort_field),
        ("sortOrder", "desc".to_string()),
        ("pageSize", page_size),
        ("index", index),
    ];

    let trimmed_q = params.query.trim();
    if !trimmed_q.is_empty() {
        query_pairs.push(("searchFilter", trimmed_q.to_string()));
    }
    if let Some(ref gv) = params.game_version {
        if !gv.trim().is_empty() {
            query_pairs.push(("gameVersion", gv.clone()));
        }
    }
    if params.project_type == ModrinthProjectType::Mod {
        if let Some(loader) = params.loader_filter {
            if let Some(lid) = cf_loader_type(loader) {
                query_pairs.push(("modLoaderType", lid.to_string()));
            }
        }
    }

    let req = cf_apply_auth(
        client
            .get(format!("{base_url}/mods/search"))
            .query(&query_pairs),
        opt_key,
    );
    let resp = req
        .send()
        .map_err(|e| format!("CurseForge search failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("CurseForge search HTTP error: {e}"))?;

    let parsed: CfSearchResp = resp
        .json()
        .map_err(|e| format!("CurseForge search parse error: {e}"))?;

    let hits: Vec<ModrinthSearchHit> = parsed
        .data
        .into_iter()
        .map(|m| {
            let author = m
                .authors
                .first()
                .map(|a| a.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            let icon_url = m
                .logo
                .and_then(|l| l.thumbnail_url.or(l.url))
                .filter(|u| !u.is_empty());
            let mut categories = Vec::new();
            let mut seen = HashSet::new();
            for c in m.categories {
                if seen.insert(c.name.clone()) {
                    categories.push(c.name);
                }
            }
            ModrinthSearchHit {
                project_id: format!("cf:{}", m.id),
                title: m.name,
                description: m.summary,
                author,
                downloads: m.download_count.max(0.0) as u64,
                icon_url,
                categories,
                source: ProjectSource::CurseForge,
                website_url: m.links.and_then(|l| l.website_url),
            }
        })
        .collect();

    let next_offset = parsed.pagination.index + parsed.pagination.result_count;
    let has_more = parsed.pagination.result_count >= SEARCH_PAGE_SIZE
        && (next_offset as u64) < parsed.pagination.total_count;

    Ok(ModrinthSearchPage {
        hits,
        total_hits: parsed.pagination.total_count,
        next_offset,
        has_more,
    })
}

fn cf_file_to_modrinth_version(f: CfModFile) -> ModrinthVersion {
    let sha1 = f
        .hashes
        .iter()
        .find(|h| h.algo == 1)
        .map(|h| h.value.clone());
    let url = f
        .download_url
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| curseforge_edge_cdn_url(f.id, &f.file_name));
    let version_type = match f.release_type {
        2 => "beta",
        3 => "alpha",
        _ => "release",
    }
    .to_string();
    let version_number = if f.display_name.trim().is_empty() {
        f.file_name.clone()
    } else {
        f.display_name.clone()
    };
    let cf_dep_mod_ids = f
        .dependencies
        .into_iter()
        .filter(|d| d.relation_type == 3)
        .map(|d| d.mod_id)
        .collect();

    ModrinthVersion {
        id: format!("cf:{}", f.id),
        name: f.display_name,
        version_number,
        version_type,
        date_published: f.file_date,
        files: vec![ModrinthFile {
            hashes: ModrinthHashes { sha1, sha512: None },
            url,
            filename: f.file_name,
            primary: true,
        }],
        dependencies: Vec::new(),
        cf_dep_mod_ids,
    }
}

fn fetch_cf_files_raw(
    client: &Client,
    api_key: &str,
    mod_id: u64,
    game_version: Option<&str>,
    loader_type: Option<u8>,
) -> Result<Vec<ModrinthVersion>, String> {
    let (base_url, opt_key) = cf_api_endpoint(api_key);
    let mut query: Vec<(&str, String)> = vec![("pageSize", "30".to_string())];
    if let Some(gv) = game_version.filter(|s| !s.trim().is_empty()) {
        query.push(("gameVersion", gv.to_string()));
    }
    if let Some(lt) = loader_type {
        query.push(("modLoaderType", lt.to_string()));
    }
    let resp = cf_apply_auth(
        client
            .get(format!("{base_url}/mods/{mod_id}/files"))
            .query(&query),
        opt_key,
    )
    .send()
    .map_err(|e| format!("CurseForge files request failed: {e}"))?
    .error_for_status()
    .map_err(|e| format!("CurseForge files HTTP error: {e}"))?;
    let parsed: CfModFilesResp = resp
        .json()
        .map_err(|e| format!("CurseForge files parse error: {e}"))?;
    Ok(parsed
        .data
        .into_iter()
        .map(cf_file_to_modrinth_version)
        .collect())
}

pub fn fetch_curseforge_versions_for_profile(
    api_key: &str,
    mod_id: u64,
    project_type: ModrinthProjectType,
    game_version: &str,
    loader: &ModLoader,
) -> Result<Vec<ModrinthVersion>, String> {
    let client = build_http_client()?;

    match project_type {
        ModrinthProjectType::Mod => {
            let lid = cf_loader_type(loader.kind()).ok_or_else(|| {
                "Mods require a mod loader (Fabric, Quilt, Forge, or NeoForge). Switch loader in Instance settings.".to_string()
            })?;
            let list = fetch_cf_files_raw(&client, api_key, mod_id, Some(game_version), Some(lid))?;
            if !list.is_empty() {
                return Ok(list);
            }
            fetch_cf_files_raw(&client, api_key, mod_id, Some(game_version), None)
        }
        ModrinthProjectType::Resourcepack
        | ModrinthProjectType::Shader
        | ModrinthProjectType::Datapack => {
            let list = fetch_cf_files_raw(&client, api_key, mod_id, Some(game_version), None)?;
            if !list.is_empty() {
                return Ok(list);
            }
            fetch_cf_files_raw(&client, api_key, mod_id, None, None)
        }
        ModrinthProjectType::Modpack => {
            let lid = cf_loader_type(loader.kind());
            let list = fetch_cf_files_raw(&client, api_key, mod_id, None, lid)?;
            if !list.is_empty() {
                return Ok(list);
            }
            fetch_cf_files_raw(&client, api_key, mod_id, None, None)
        }
        ModrinthProjectType::Plugin => {
            fetch_cf_files_raw(&client, api_key, mod_id, Some(game_version), None)
        }
    }
}

fn cf_project_meta(
    client: &Client,
    api_key: &str,
    mod_id: u64,
) -> Option<(String, Option<String>)> {
    let (base_url, opt_key) = cf_api_endpoint(api_key);
    let resp: CfSingleModResp =
        cf_apply_auth(client.get(format!("{base_url}/mods/{mod_id}")), opt_key)
            .send()
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .ok()?;
    let icon = resp
        .data
        .logo
        .and_then(|l| l.thumbnail_url.or(l.url))
        .filter(|u| !u.is_empty());
    Some((resp.data.name, icon))
}

#[allow(clippy::too_many_arguments)]
fn install_cf_file_to_dir(
    client: &Client,
    base_dir: &Path,
    folder_rel: String,
    mod_id: u64,
    title: &str,
    icon_url: Option<String>,
    version: &ModrinthVersion,
    index: &mut HashMap<String, InstalledProject>,
    tx: &Sender<WorkerEvent>,
) -> Result<String, String> {
    let file = version
        .files
        .first()
        .ok_or_else(|| format!("Version {} has no files", version.version_number))?;
    let safe_name = sanitize_mod_filename(&file.filename)?;

    let dest_dir = base_dir.join(&folder_rel);
    fs::create_dir_all(&dest_dir).map_err(|e| format!("Failed to create {folder_rel}: {e}"))?;

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

    let project_key = format!("cf:{mod_id}");
    if let Some(old) = index.get(&project_key) {
        if old.filename != safe_name {
            let old_dir = base_dir.join(&old.folder);
            let _ = fs::remove_file(old_dir.join(&old.filename));
            let _ = fs::remove_file(old_dir.join(format!("{}.disabled", old.filename)));
        }
    }
    index.insert(
        project_key,
        InstalledProject {
            title: title.to_string(),
            filename: safe_name.clone(),
            folder: folder_rel,
            version_number: version.version_number.clone(),
            icon_url,
            source: ProjectSource::CurseForge,
        },
    );
    Ok(safe_name)
}

#[allow(clippy::too_many_arguments)]
fn install_cf_version_single(
    client: &Client,
    paths: &LauncherPaths,
    profile: &Profile,
    mod_id: u64,
    project_type: ModrinthProjectType,
    title: &str,
    icon_url: Option<String>,
    version: &ModrinthVersion,
    index: &mut HashMap<String, InstalledProject>,
    tx: &Sender<WorkerEvent>,
) -> Result<String, String> {
    let inst = instance_dir(paths, profile);
    let folder_rel = if project_type == ModrinthProjectType::Datapack {
        default_datapack_rel_folder(&inst)
    } else {
        folder_for(project_type).to_string()
    };
    install_cf_file_to_dir(
        client, &inst, folder_rel, mod_id, title, icon_url, version, index, tx,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn install_curseforge_project(
    api_key: &str,
    paths: &LauncherPaths,
    profile: &Profile,
    mod_id: u64,
    project_type: ModrinthProjectType,
    title: &str,
    icon_url: Option<&str>,
    chosen_version: Option<&ModrinthVersion>,
    tx: &Sender<WorkerEvent>,
) -> Result<ModrinthInstallOutcome, String> {
    check_not_cancelled()?;
    if project_type == ModrinthProjectType::Mod && !profile.loader.is_modded() {
        return Err("Mods require a modded loader (Fabric, Quilt, Forge, or NeoForge). Switch loader in Instance settings.".to_string());
    }
    if project_type == ModrinthProjectType::Plugin {
        return Err("Plugins can only be installed to Dedicated Servers.".to_string());
    }
    let precreated_dest_folder = if project_type != ModrinthProjectType::Modpack {
        let (_, dest_folder, _) = ensure_instance_content_dir(paths, profile, project_type)?;
        Some(dest_folder)
    } else {
        None
    };

    let client = build_http_client()?;

    let version = match chosen_version {
        Some(v) => v.clone(),
        None => {
            let _ = tx.send(WorkerEvent::Progress {
                stage: format!("Finding version for {title}..."),
                fraction: 0.1,
            });
            let list = fetch_curseforge_versions_for_profile(
                api_key,
                mod_id,
                project_type,
                &profile.version_id,
                &profile.loader,
            )?;
            list.into_iter().next().ok_or_else(|| {
                format!(
                    "No compatible CurseForge file found for {title} ({} {}).",
                    profile.version_id,
                    profile.loader.kind().label()
                )
            })?
        }
    };

    if project_type == ModrinthProjectType::Modpack {
        let file = version
            .files
            .first()
            .ok_or_else(|| "Modpack version has no files".to_string())?;
        let tmp_zip = paths.root.join(format!("cf_pack_{mod_id}.zip"));
        let _ = tx.send(WorkerEvent::Progress {
            stage: format!("Downloading modpack {}...", file.filename),
            fraction: 0.12,
        });
        download_file(&client, &file.url, &tmp_zip, file.hashes.sha1.as_deref())?;
        let res = install_curseforge_zip(paths, &tmp_zip, profile.username.clone(), api_key, tx);
        let _ = fs::remove_file(&tmp_zip);
        return res;
    }

    let mut index = load_installed_index(paths, profile);
    let filename = install_cf_version_single(
        &client,
        paths,
        profile,
        mod_id,
        project_type,
        title,
        icon_url.map(str::to_string),
        &version,
        &mut index,
        tx,
    )?;

    let mut deps_installed = Vec::new();
    if project_type == ModrinthProjectType::Mod && !version.cf_dep_mod_ids.is_empty() {
        let mut visited: HashSet<u64> = HashSet::from([mod_id]);
        let mut queue: Vec<u64> = version.cf_dep_mod_ids.clone();
        while let Some(dep_id) = queue.pop() {
            if !visited.insert(dep_id) || index.contains_key(&format!("cf:{dep_id}")) {
                continue;
            }
            let (dep_title, dep_icon) = cf_project_meta(&client, api_key, dep_id)
                .unwrap_or_else(|| (format!("CF-{dep_id}"), None));
            let _ = tx.send(WorkerEvent::Progress {
                stage: format!("Dependency: {dep_title}..."),
                fraction: 0.75,
            });
            if let Ok(dep_vers) = fetch_curseforge_versions_for_profile(
                api_key,
                dep_id,
                ModrinthProjectType::Mod,
                &profile.version_id,
                &profile.loader,
            ) {
                if let Some(dep_ver) = dep_vers.into_iter().next() {
                    if install_cf_version_single(
                        &client,
                        paths,
                        profile,
                        dep_id,
                        ModrinthProjectType::Mod,
                        &dep_title,
                        dep_icon,
                        &dep_ver,
                        &mut index,
                        tx,
                    )
                    .is_ok()
                    {
                        deps_installed.push(dep_title);
                        queue.extend(dep_ver.cf_dep_mod_ids);
                    }
                }
            }
        }
    }

    save_installed_index(paths, profile, &index);
    let dest_folder = precreated_dest_folder.unwrap_or_else(|| {
        if project_type == ModrinthProjectType::Datapack {
            default_datapack_rel_folder(&instance_dir(paths, profile))
        } else {
            folder_for(project_type).to_string()
        }
    });
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

/// Installs a CurseForge mod or plugin directly into a dedicated server's `mods/` or `plugins/` folder.
#[allow(clippy::too_many_arguments)]
pub fn install_curseforge_server_project(
    api_key: &str,
    paths: &LauncherPaths,
    server: &DedicatedServer,
    mod_id: u64,
    project_type: ModrinthProjectType,
    title: &str,
    icon_url: Option<&str>,
    chosen_version: Option<&ModrinthVersion>,
    tx: &Sender<WorkerEvent>,
) -> Result<ModrinthInstallOutcome, String> {
    check_not_cancelled()?;
    let srv_dir = server_dir(paths, server);
    let dest_folder = match project_type {
        ModrinthProjectType::Plugin => {
            if !server_supports_plugins_live(&srv_dir, server.software) {
                return Err(format!(
                    "{} server does not support plugins (install a plugin-bridge mod or use Paper/Purpur/Folia/Arclight).",
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

    let loader = if project_type == ModrinthProjectType::Mod {
        ModLoader::from_kind(
            server
                .software
                .modrinth_mod_loader()
                .unwrap_or(LoaderKind::Vanilla),
            String::new(),
        )
    } else {
        ModLoader::Vanilla
    };

    let client = build_http_client()?;
    let version = match chosen_version {
        Some(v) => v.clone(),
        None => {
            let _ = tx.send(WorkerEvent::Progress {
                stage: format!("Finding version for {title}..."),
                fraction: 0.1,
            });
            let mut list = fetch_curseforge_versions_for_profile(
                api_key,
                mod_id,
                project_type,
                &server.mc_version,
                &loader,
            )?;
            if list.is_empty() && project_type == ModrinthProjectType::Plugin {
                list = fetch_cf_files_raw(&client, api_key, mod_id, None, None)?;
            }
            list.into_iter().next().ok_or_else(|| {
                format!(
                    "No compatible CurseForge file found for {title} ({} {}).",
                    server.mc_version,
                    server.software.label()
                )
            })?
        }
    };

    let mut index = load_server_installed_index(paths, server);
    let filename = install_cf_file_to_dir(
        &client,
        &srv_dir,
        dest_folder.to_string(),
        mod_id,
        title,
        icon_url.map(str::to_string),
        &version,
        &mut index,
        tx,
    )?;

    let mut deps_installed = Vec::new();
    if project_type == ModrinthProjectType::Mod && !version.cf_dep_mod_ids.is_empty() {
        let mut visited: HashSet<u64> = HashSet::from([mod_id]);
        let mut queue: Vec<u64> = version.cf_dep_mod_ids.clone();
        while let Some(dep_id) = queue.pop() {
            if !visited.insert(dep_id) || index.contains_key(&format!("cf:{dep_id}")) {
                continue;
            }
            let (dep_title, dep_icon) = cf_project_meta(&client, api_key, dep_id)
                .unwrap_or_else(|| (format!("CF-{dep_id}"), None));
            let _ = tx.send(WorkerEvent::Progress {
                stage: format!("Dependency: {dep_title}..."),
                fraction: 0.75,
            });
            if let Ok(dep_vers) = fetch_curseforge_versions_for_profile(
                api_key,
                dep_id,
                ModrinthProjectType::Mod,
                &server.mc_version,
                &loader,
            ) {
                if let Some(dep_ver) = dep_vers.into_iter().next() {
                    if install_cf_file_to_dir(
                        &client,
                        &srv_dir,
                        dest_folder.to_string(),
                        dep_id,
                        &dep_title,
                        dep_icon,
                        &dep_ver,
                        &mut index,
                        tx,
                    )
                    .is_ok()
                    {
                        deps_installed.push(dep_title);
                        queue.extend(dep_ver.cf_dep_mod_ids);
                    }
                }
            }
        }
    }

    save_server_installed_index(paths, server, &index);
    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Installed: {filename}"),
        fraction: 1.0,
    });
    Ok(ModrinthInstallOutcome::FileInstalled {
        title: title.to_string(),
        filename,
        dest_folder: dest_folder.to_string(),
        dependencies_installed: deps_installed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cf_project_id_and_loader_mapping() {
        assert_eq!(parse_cf_project_id("cf:238222"), Some(238222));
        assert_eq!(parse_cf_project_id("AANobbMI"), None);
        assert_eq!(parse_cf_project_id("cf:abc"), None);

        assert_eq!(cf_loader_type(LoaderKind::Forge), Some(1));
        assert_eq!(cf_loader_type(LoaderKind::Fabric), Some(4));
        assert_eq!(cf_loader_type(LoaderKind::Quilt), Some(5));
        assert_eq!(cf_loader_type(LoaderKind::NeoForge), Some(6));
        assert_eq!(cf_loader_type(LoaderKind::Vanilla), None);

        assert_eq!(cf_api_endpoint(""), ("https://api.curse.tools/v1/cf", None));
        assert_eq!(
            cf_api_endpoint(" my_secret_key "),
            ("https://api.curseforge.com/v1", Some("my_secret_key"))
        );
    }
}
