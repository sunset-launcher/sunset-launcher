use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Coarse loader category used by UI pickers, caches, and Modrinth filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoaderKind {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

impl LoaderKind {
    pub const ALL: [LoaderKind; 5] = [
        LoaderKind::Vanilla,
        LoaderKind::Fabric,
        LoaderKind::Quilt,
        LoaderKind::Forge,
        LoaderKind::NeoForge,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LoaderKind::Vanilla => "Vanilla",
            LoaderKind::Fabric => "Fabric",
            LoaderKind::Quilt => "Quilt",
            LoaderKind::Forge => "Forge",
            LoaderKind::NeoForge => "NeoForge",
        }
    }

    pub fn modrinth_slug(self) -> Option<&'static str> {
        match self {
            LoaderKind::Vanilla => None,
            LoaderKind::Fabric => Some("fabric"),
            LoaderKind::Quilt => Some("quilt"),
            LoaderKind::Forge => Some("forge"),
            LoaderKind::NeoForge => Some("neoforge"),
        }
    }

    pub fn is_modded(self) -> bool {
        !matches!(self, LoaderKind::Vanilla)
    }
}

/// Mod loader type for a Minecraft profile/instance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[derive(Default)]
pub enum ModLoader {
    #[default]
    Vanilla,
    Fabric {
        #[serde(default)]
        loader_version: String,
    },
    Quilt {
        #[serde(default)]
        loader_version: String,
    },
    Forge {
        #[serde(default)]
        loader_version: String,
    },
    NeoForge {
        #[serde(default)]
        loader_version: String,
    },
}

impl ModLoader {
    pub fn kind(&self) -> LoaderKind {
        match self {
            ModLoader::Vanilla => LoaderKind::Vanilla,
            ModLoader::Fabric { .. } => LoaderKind::Fabric,
            ModLoader::Quilt { .. } => LoaderKind::Quilt,
            ModLoader::Forge { .. } => LoaderKind::Forge,
            ModLoader::NeoForge { .. } => LoaderKind::NeoForge,
        }
    }

    pub fn short_name(&self) -> &'static str {
        self.kind().label()
    }

    pub fn modrinth_slug(&self) -> Option<&'static str> {
        self.kind().modrinth_slug()
    }

    pub fn is_modded(&self) -> bool {
        self.kind().is_modded()
    }

    pub fn loader_version(&self) -> Option<&str> {
        match self {
            ModLoader::Vanilla => None,
            ModLoader::Fabric { loader_version }
            | ModLoader::Quilt { loader_version }
            | ModLoader::Forge { loader_version }
            | ModLoader::NeoForge { loader_version } => Some(loader_version.as_str()),
        }
    }

    pub fn loader_version_mut(&mut self) -> Option<&mut String> {
        match self {
            ModLoader::Vanilla => None,
            ModLoader::Fabric { loader_version }
            | ModLoader::Quilt { loader_version }
            | ModLoader::Forge { loader_version }
            | ModLoader::NeoForge { loader_version } => Some(loader_version),
        }
    }

    pub fn from_kind(kind: LoaderKind, loader_version: String) -> Self {
        match kind {
            LoaderKind::Vanilla => ModLoader::Vanilla,
            LoaderKind::Fabric => ModLoader::Fabric { loader_version },
            LoaderKind::Quilt => ModLoader::Quilt { loader_version },
            LoaderKind::Forge => ModLoader::Forge { loader_version },
            LoaderKind::NeoForge => ModLoader::NeoForge { loader_version },
        }
    }

    pub fn label(&self) -> String {
        let name = self.short_name();
        match self.loader_version() {
            Some(v) if !v.is_empty() => format!("{name} ({v})"),
            _ => name.to_string(),
        }
    }
}

/// A user-configured Minecraft profile / instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub version_id: String,
    pub loader: ModLoader,
    pub username: String,
    pub min_ram_mb: u32,
    pub max_ram_mb: u32,
    pub java_path: String,
    pub jvm_args: String,
    pub created_at: String,
    pub last_played: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub playtime_secs: u64,
    #[serde(default)]
    pub window_width: Option<u32>,
    #[serde(default)]
    pub window_height: Option<u32>,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub server_address: String,
    #[serde(default)]
    pub prefer_discrete_gpu: bool,
    #[serde(default)]
    pub icon_path: String,
    #[serde(default)]
    pub dir_name: String,
    #[serde(default)]
    pub pre_launch_cmd: String,
    #[serde(default)]
    pub post_exit_cmd: String,
    #[serde(default)]
    pub wrapper_cmd: String,
    #[serde(default)]
    pub env_vars: String,
}

impl Profile {
    pub fn new(name: String, version_id: String, loader: ModLoader, username: String) -> Self {
        Self::new_with_defaults(
            name,
            version_id,
            loader,
            username,
            &LauncherSettings::default(),
        )
    }

    pub fn new_with_defaults(
        name: String,
        version_id: String,
        loader: ModLoader,
        username: String,
        defaults: &LauncherSettings,
    ) -> Self {
        let min_ram = defaults.default_min_ram_mb.clamp(256, 32768);
        let max_ram = defaults.default_max_ram_mb.max(min_ram).clamp(512, 32768);
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            version_id,
            loader,
            username,
            min_ram_mb: min_ram,
            max_ram_mb: max_ram,
            java_path: defaults.default_java_path.clone(),
            jvm_args: defaults.default_jvm_args.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            last_played: None,
            pinned: false,
            playtime_secs: 0,
            window_width: defaults.default_window_width,
            window_height: defaults.default_window_height,
            fullscreen: defaults.default_fullscreen,
            server_address: String::new(),
            prefer_discrete_gpu: defaults.default_prefer_discrete_gpu,
            icon_path: String::new(),
            dir_name: String::new(),
            pre_launch_cmd: String::new(),
            post_exit_cmd: String::new(),
            wrapper_cmd: String::new(),
            env_vars: String::new(),
        }
    }
}

/// Mojang version_manifest_v2.json root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VersionType {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: VersionType,
    pub url: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    pub sha1: Option<String>,
}

/// Detailed version JSON (<version>.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionDetails {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: Option<String>,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    pub downloads: Option<VersionDownloads>,
    pub libraries: Vec<Library>,
    #[serde(rename = "assetIndex")]
    pub asset_index: Option<AssetIndexInfo>,
    pub assets: Option<String>,
    pub arguments: Option<Arguments>,
    #[serde(rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    #[serde(rename = "javaVersion")]
    pub java_version: Option<JavaVersionInfo>,
    #[serde(default)]
    pub logging: Option<VersionLogging>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionLogging {
    #[serde(default)]
    pub client: Option<LoggingClientConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingClientConfig {
    pub argument: String,
    pub file: LoggingFileArtifact,
    #[serde(rename = "type", default)]
    pub logging_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingFileArtifact {
    pub id: String,
    pub sha1: String,
    #[serde(default)]
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaVersionInfo {
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionDownloads {
    pub client: Option<DownloadArtifact>,
    #[serde(default)]
    pub server: Option<DownloadArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadArtifact {
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub url: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    pub rules: Option<Vec<Rule>>,
    pub natives: Option<HashMap<String, String>>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<DownloadArtifact>,
    pub classifiers: Option<HashMap<String, DownloadArtifact>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetIndexInfo {
    pub id: String,
    pub sha1: String,
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    #[serde(rename = "virtual", default)]
    pub is_virtual: bool,
    #[serde(default)]
    pub map_to_resources: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Arguments {
    pub game: Option<Vec<serde_json::Value>>,
    pub jvm: Option<Vec<serde_json::Value>>,
}

/// Fabric / Quilt Meta API response for /versions/loader/<game_version>.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricLoaderEntry {
    pub loader: FabricLoaderInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricLoaderInfo {
    pub version: String,
    #[serde(default)]
    pub stable: bool,
}

/// Normalized loader version item shared across Fabric, Quilt, Forge, and NeoForge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderVersionEntry {
    pub version: String,
    pub stable: bool,
}

/// Mod loader profile overlay merged on top of Vanilla version metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoaderProfileOverlay {
    pub id: String,
    #[serde(rename = "inheritsFrom", default)]
    pub inherits_from: String,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    pub arguments: Option<Arguments>,
    #[serde(rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    #[serde(default)]
    pub libraries: Vec<Library>,
}

/// Purpose tag for non-blocking OS file selection dialogs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilePickPurpose {
    ModpackImport,
    InstanceContentImport,
    WorldDatapackImport { world_folder: String },
    InstanceIcon,
    ServerIcon,
    JavaBinary,
    ServerJavaBinary,
    CustomUiFont,
}

/// Messages sent from background worker threads to the UI.
#[derive(Debug, Clone)]
pub enum WorkerEvent {
    ManifestLoaded(Result<VersionManifest, String>),
    LoaderVersionsLoaded {
        loader: LoaderKind,
        game_version: String,
        result: Result<Vec<LoaderVersionEntry>, String>,
    },
    Progress {
        stage: String,
        fraction: f32,
    },
    Log(String),
    InstanceLog {
        profile_id: String,
        line: String,
    },
    LaunchFinished {
        profile_id: String,
        result: Result<GameHandle, String>,
    },
    GameExited {
        profile_id: String,
        code: Option<i32>,
    },
    /// Account tokens were refreshed in the background and must be persisted.
    AccountRefreshed(Account),
    /// Manual account token refresh triggered from the Accounts page.
    AccountManualRefreshed(Result<Account, String>),
    MsDeviceCodeReady(Result<DeviceCodeInfo, String>),
    MsAuthFinished(Result<Account, String>),
    ElyAuthFinished(Result<Account, String>),
    ModrinthSearchFinished {
        generation: u64,
        append: bool,
        result: Result<ModrinthSearchPage, String>,
    },
    ModrinthInstallFinished(Result<ModrinthInstallOutcome, String>),
    BatchImportFinished {
        imported: usize,
        errors: Vec<String>,
    },
    ModUpdatesFinished(Result<Vec<String>, String>),
    ModrinthIdentifyFinished {
        profile_id: String,
        result: Result<(usize, usize), String>,
    },
    McloGsUploaded(Result<String, String>),
    ProjectVersionsLoaded {
        project_id: String,
        result: Result<Vec<ModrinthVersion>, String>,
    },
    IconLoaded {
        url: String,
        image: Result<IconImage, String>,
    },
    SkinLoaded {
        key: String,
        head: Result<IconImage, String>,
        body: Result<IconImage, String>,
    },
    FilePicked {
        purpose: FilePickPurpose,
        path: Option<PathBuf>,
    },
    WorldBackupFinished(Result<PathBuf, String>),
    WorldRestoreFinished(Result<String, String>),
    LauncherUpdateChecked(Result<Option<LauncherUpdateInfo>, String>),
    LauncherUpdateInstalled(Result<PathBuf, String>),
    JavaDetected(Vec<JavaInstall>),
    ServerProvisioned(Result<DedicatedServer, String>),
    ServerStarted(Result<ServerHandle, String>),
    ServerLog(String),
    ServerTunnelAddress(String),
    ServerTunnelFailed(String),
    ServerExited(Option<i32>),
    ServerProjectInstalled(Result<ModrinthInstallOutcome, String>),
}

/// Supported dedicated server software.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServerSoftware {
    #[default]
    Vanilla,
    Paper,
    Purpur,
    Folia,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
    Arclight,
}

impl ServerSoftware {
    pub const ALL: [ServerSoftware; 9] = [
        ServerSoftware::Vanilla,
        ServerSoftware::Paper,
        ServerSoftware::Purpur,
        ServerSoftware::Folia,
        ServerSoftware::Fabric,
        ServerSoftware::Quilt,
        ServerSoftware::Forge,
        ServerSoftware::NeoForge,
        ServerSoftware::Arclight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ServerSoftware::Vanilla => "Vanilla",
            ServerSoftware::Paper => "Paper",
            ServerSoftware::Purpur => "Purpur",
            ServerSoftware::Folia => "Folia",
            ServerSoftware::Fabric => "Fabric",
            ServerSoftware::Quilt => "Quilt",
            ServerSoftware::Forge => "Forge",
            ServerSoftware::NeoForge => "NeoForge",
            ServerSoftware::Arclight => "Arclight",
        }
    }

    #[allow(dead_code)]
    pub fn supports_plugins(self) -> bool {
        matches!(
            self,
            ServerSoftware::Paper
                | ServerSoftware::Purpur
                | ServerSoftware::Folia
                | ServerSoftware::Arclight
        )
    }

    #[allow(dead_code)]
    pub fn supports_mods(self) -> bool {
        matches!(
            self,
            ServerSoftware::Fabric
                | ServerSoftware::Quilt
                | ServerSoftware::Forge
                | ServerSoftware::NeoForge
                | ServerSoftware::Arclight
        )
    }

    pub fn modrinth_mod_loader(self) -> Option<LoaderKind> {
        match self {
            ServerSoftware::Fabric => Some(LoaderKind::Fabric),
            ServerSoftware::Quilt => Some(LoaderKind::Quilt),
            ServerSoftware::Forge => Some(LoaderKind::Forge),
            ServerSoftware::NeoForge => Some(LoaderKind::NeoForge),
            ServerSoftware::Arclight => Some(LoaderKind::Forge),
            _ => None,
        }
    }

    pub fn modrinth_plugin_loaders(self) -> &'static [&'static str] {
        match self {
            ServerSoftware::Folia => &["folia", "paper"],
            ServerSoftware::Purpur => &["purpur", "paper", "spigot", "bukkit"],
            _ => &["paper", "purpur", "spigot", "bukkit"],
        }
    }
}

/// A user-configured dedicated Minecraft server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedicatedServer {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub dir_name: String,
    #[serde(default)]
    pub software: ServerSoftware,
    pub mc_version: String,
    #[serde(default)]
    pub loader_version: String,
    #[serde(default = "default_server_min_ram")]
    pub min_ram_mb: u32,
    #[serde(default = "default_server_max_ram")]
    pub max_ram_mb: u32,
    #[serde(default = "default_server_port")]
    pub port: u16,
    #[serde(default = "default_server_motd")]
    pub motd: String,
    #[serde(default = "default_server_max_players")]
    pub max_players: u32,
    #[serde(default = "default_true")]
    pub online_mode: bool,
    #[serde(default = "default_true")]
    pub public_upnp: bool,
    #[serde(default)]
    pub use_tunnel: bool,
    #[serde(default)]
    pub eula_accepted: bool,
    #[serde(default)]
    pub java_path: String,
    #[serde(default)]
    pub jvm_args: String,
    pub created_at: String,
    #[serde(default)]
    pub last_started: Option<String>,
}

fn default_server_min_ram() -> u32 {
    1024
}

fn default_server_max_ram() -> u32 {
    2048
}

fn default_server_port() -> u16 {
    25565
}

fn default_server_motd() -> String {
    "A Minecraft Server".to_string()
}

fn default_server_max_players() -> u32 {
    20
}

impl DedicatedServer {
    pub fn new(
        name: String,
        software: ServerSoftware,
        mc_version: String,
        port: u16,
        max_ram_mb: u32,
        online_mode: bool,
        motd: String,
        eula_accepted: bool,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            dir_name: String::new(),
            software,
            mc_version,
            loader_version: String::new(),
            min_ram_mb: (max_ram_mb / 2).clamp(512, 2048),
            max_ram_mb: max_ram_mb.max(512),
            port,
            motd: if motd.trim().is_empty() {
                default_server_motd()
            } else {
                motd.trim().to_string()
            },
            max_players: 20,
            online_mode,
            public_upnp: true,
            use_tunnel: false,
            eula_accepted,
            java_path: String::new(),
            jvm_args: String::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
            last_started: None,
        }
    }
}

/// Active UPnP IGD port mapping created when starting a public server.
#[derive(Debug, Clone)]
pub struct UpnpMapping {
    pub control_url: String,
    pub service_type: String,
    pub port: u16,
    pub public_ip: Option<String>,
}

/// Handle to an active public tunnel (built-in `bore` client or Pinggy SSH fallback).
#[derive(Debug)]
pub enum TunnelHandle {
    Builtin(std::sync::Arc<std::sync::atomic::AtomicBool>),
    External(std::sync::Arc<std::sync::Mutex<std::process::Child>>),
}

impl TunnelHandle {
    pub fn stop(&self) {
        match self {
            Self::Builtin(stop_flag) => {
                stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            Self::External(child) => {
                if let Ok(mut guard) = child.lock() {
                    let _ = guard.kill();
                }
            }
        }
    }
}

/// Shared handle to a running dedicated server process and its stdin pipe.
#[derive(Debug, Clone)]
pub struct ServerHandle {
    pub server_id: String,
    pub child: std::sync::Arc<std::sync::Mutex<std::process::Child>>,
    pub stdin: std::sync::Arc<std::sync::Mutex<Option<std::process::ChildStdin>>>,
    pub upnp_mapping: Option<UpnpMapping>,
    pub tunnel: Option<std::sync::Arc<TunnelHandle>>,
}

/// Information about a newer release on GitHub.
#[derive(Debug, Clone)]
pub struct LauncherUpdateInfo {
    pub version: String,
    pub html_url: String,
    pub notes: String,
    pub download_url: Option<String>,
    pub sha256_url: Option<String>,
    pub sha256_digest: Option<String>,
    pub asset_name: Option<String>,
    pub asset_size: Option<u64>,
}

/// A Java runtime found on the system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaInstall {
    pub path: PathBuf,
    pub major: u32,
}

/// Shared handle to the running game process so the UI can stop it.
pub type GameHandle = std::sync::Arc<std::sync::Mutex<std::process::Child>>;

/// Decoded RGBA icon ready to upload as an egui texture.
#[derive(Debug, Clone)]
pub struct IconImage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ModrinthSearchPage {
    pub hits: Vec<ModrinthSearchHit>,
    pub total_hits: u64,
    pub next_offset: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModrinthSort {
    Relevance,
    Downloads,
    Newest,
    Updated,
}

impl ModrinthSort {
    pub fn api_str(self) -> &'static str {
        match self {
            ModrinthSort::Relevance => "relevance",
            ModrinthSort::Downloads => "downloads",
            ModrinthSort::Newest => "newest",
            ModrinthSort::Updated => "updated",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModrinthProjectType {
    Mod,
    Plugin,
    Resourcepack,
    Shader,
    Datapack,
    Modpack,
}

impl ModrinthProjectType {
    pub fn api_str(self) -> &'static str {
        match self {
            ModrinthProjectType::Mod => "mod",
            ModrinthProjectType::Plugin => "plugin",
            ModrinthProjectType::Resourcepack => "resourcepack",
            ModrinthProjectType::Shader => "shader",
            ModrinthProjectType::Datapack => "datapack",
            ModrinthProjectType::Modpack => "modpack",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProjectSource {
    #[default]
    Modrinth,
    CurseForge,
}

impl ProjectSource {
    pub fn label(self) -> &'static str {
        match self {
            ProjectSource::Modrinth => "Modrinth",
            ProjectSource::CurseForge => "CurseForge",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthSearchResponse {
    pub hits: Vec<ModrinthSearchHit>,
    #[serde(default)]
    pub total_hits: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthSearchHit {
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub source: ProjectSource,
    #[serde(default)]
    pub website_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthVersion {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub version_number: String,
    #[serde(default)]
    pub version_type: String,
    #[serde(default)]
    pub date_published: String,
    pub files: Vec<ModrinthFile>,
    #[serde(default)]
    pub dependencies: Vec<ModrinthDependency>,
    #[serde(default)]
    pub cf_dep_mod_ids: Vec<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthDependency {
    pub project_id: Option<String>,
    pub dependency_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthFile {
    pub hashes: ModrinthHashes,
    pub url: String,
    pub filename: String,
    pub primary: bool,
}

/// Record of a Modrinth or CurseForge project installed into an instance (stored per instance).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledProject {
    pub title: String,
    pub filename: String,
    pub folder: String,
    pub version_number: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub source: ProjectSource,
}

/// Options controlling what gets included in `.mrpack` `overrides/`.
#[derive(Debug, Clone)]
pub struct MrpackExportOptions {
    pub include_config: bool,
    pub include_options_txt: bool,
    pub include_unmatched_mods: bool,
    pub include_resourcepacks: bool,
    pub include_shaderpacks: bool,
    pub include_saves: bool,
    pub include_screenshots: bool,
    pub extra_excludes: String,
}

impl Default for MrpackExportOptions {
    fn default() -> Self {
        Self {
            include_config: true,
            include_options_txt: true,
            include_unmatched_mods: true,
            include_resourcepacks: true,
            include_shaderpacks: true,
            include_saves: false,
            include_screenshots: false,
            extra_excludes: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFileKind {
    CrashReport,
    Log,
}

#[derive(Debug, Clone)]
pub struct InstanceLogFile {
    pub filename: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub kind: LogFileKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModrinthHashes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha512: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MrpackIndex {
    #[serde(rename = "formatVersion", default = "default_mrpack_format_version")]
    pub format_version: u32,
    #[serde(default = "default_mrpack_game")]
    pub game: String,
    #[serde(rename = "versionId", default)]
    pub version_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub files: Vec<MrpackFile>,
    pub dependencies: HashMap<String, String>,
}

fn default_mrpack_format_version() -> u32 {
    1
}

fn default_mrpack_game() -> String {
    "minecraft".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MrpackFile {
    pub path: String,
    pub hashes: ModrinthHashes,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<MrpackEnv>,
    pub downloads: Vec<String>,
    #[serde(rename = "fileSize", skip_serializing_if = "Option::is_none")]
    pub file_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MrpackEnv {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ModrinthInstallOutcome {
    FileInstalled {
        title: String,
        filename: String,
        dest_folder: String,
        dependencies_installed: Vec<String>,
    },
    ModpackProfileCreated {
        profile: Profile,
        files_count: usize,
    },
    ModpackExported {
        path: PathBuf,
    },
}

/// Device code prompt shown to the user during Microsoft OAuth login.
#[derive(Debug, Clone)]
pub struct DeviceCodeInfo {
    pub user_code: String,
    pub verification_uri: String,
    pub message: String,
}

/// Authentication provider and credentials for a Minecraft account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AccountKind {
    Offline,
    Microsoft {
        uuid: String,
        access_token: String,
        refresh_token: String,
        expires_at: i64,
    },
    ElyBy {
        uuid: String,
        access_token: String,
        client_token: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub username: String,
    pub kind: AccountKind,
}

impl Account {
    pub fn offline(username: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            username,
            kind: AccountKind::Offline,
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            AccountKind::Offline => "Offline",
            AccountKind::Microsoft { .. } => "Microsoft",
            AccountKind::ElyBy { .. } => "Ely.by",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountStore {
    pub active_id: Option<String>,
    pub accounts: Vec<Account>,
}

/// Resolved directories for launcher data.
#[derive(Debug, Clone)]
pub struct LauncherPaths {
    pub root: PathBuf,
    pub versions_dir: PathBuf,
    pub libraries_dir: PathBuf,
    pub assets_dir: PathBuf,
    pub instances_dir: PathBuf,
    pub servers_dir: PathBuf,
    pub profiles_file: PathBuf,
    pub servers_file: PathBuf,
    pub accounts_file: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemePreset {
    #[default]
    Sunset,
    Emerald,
    Cyan,
    Crimson,
    Nord,
    Amber,
    Amethyst,
    Midnight,
    Light,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomPalette {
    pub bg_void: [u8; 3],
    pub bg_sidebar: [u8; 3],
    pub bg_elevated: [u8; 3],
    pub bg_card: [u8; 3],
    pub bg_card_hover: [u8; 3],
    pub bg_input: [u8; 3],
    pub border_subtle: [u8; 3],
    pub border_strong: [u8; 3],
    pub text_primary: [u8; 3],
    pub text_secondary: [u8; 3],
    pub text_muted: [u8; 3],
    pub accent: [u8; 3],
}

impl Default for CustomPalette {
    fn default() -> Self {
        Self {
            bg_void: [21, 20, 19],
            bg_sidebar: [16, 15, 14],
            bg_elevated: [32, 30, 28],
            bg_card: [27, 26, 24],
            bg_card_hover: [38, 36, 33],
            bg_input: [16, 15, 14],
            border_subtle: [42, 40, 37],
            border_strong: [58, 55, 51],
            text_primary: [239, 233, 222],
            text_secondary: [170, 163, 152],
            text_muted: [138, 132, 122],
            accent: [255, 128, 66],
        }
    }
}

impl ThemePreset {
    pub const ALL: [ThemePreset; 10] = [
        ThemePreset::Sunset,
        ThemePreset::Emerald,
        ThemePreset::Cyan,
        ThemePreset::Crimson,
        ThemePreset::Nord,
        ThemePreset::Amber,
        ThemePreset::Amethyst,
        ThemePreset::Midnight,
        ThemePreset::Light,
        ThemePreset::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ThemePreset::Sunset => "Sunset",
            ThemePreset::Emerald => "Emerald",
            ThemePreset::Cyan => "Obsidian Cyan",
            ThemePreset::Crimson => "Crimson",
            ThemePreset::Nord => "Nord Blue",
            ThemePreset::Amber => "Amber Gold",
            ThemePreset::Amethyst => "Amethyst",
            ThemePreset::Midnight => "Midnight",
            ThemePreset::Light => "Light",
            ThemePreset::Custom => "Custom",
        }
    }

    pub fn palette(self) -> CustomPalette {
        match self {
            ThemePreset::Sunset | ThemePreset::Custom => CustomPalette::default(),
            ThemePreset::Emerald => CustomPalette {
                bg_void: [18, 20, 24],
                bg_sidebar: [24, 26, 32],
                bg_elevated: [30, 33, 40],
                bg_card: [36, 40, 48],
                bg_card_hover: [44, 49, 59],
                bg_input: [15, 17, 21],
                border_subtle: [46, 51, 61],
                border_strong: [69, 76, 90],
                text_primary: [238, 242, 247],
                text_secondary: [155, 163, 180],
                text_muted: [103, 112, 130],
                accent: [27, 217, 106],
            },
            ThemePreset::Cyan => CustomPalette {
                bg_void: [15, 19, 24],
                bg_sidebar: [20, 25, 32],
                bg_elevated: [26, 32, 41],
                bg_card: [31, 39, 49],
                bg_card_hover: [39, 49, 61],
                bg_input: [12, 15, 20],
                border_subtle: [42, 53, 66],
                border_strong: [62, 78, 96],
                accent: [34, 211, 238],
                ..CustomPalette::default()
            },
            ThemePreset::Crimson => CustomPalette {
                bg_void: [22, 17, 19],
                bg_sidebar: [29, 22, 25],
                bg_elevated: [36, 28, 31],
                bg_card: [43, 33, 37],
                bg_card_hover: [54, 41, 46],
                bg_input: [17, 13, 15],
                border_subtle: [58, 44, 49],
                border_strong: [84, 62, 70],
                accent: [239, 68, 68],
                ..CustomPalette::default()
            },
            ThemePreset::Nord => CustomPalette {
                bg_void: [46, 52, 64],
                bg_sidebar: [36, 41, 51],
                bg_elevated: [59, 66, 82],
                bg_card: [67, 76, 94],
                bg_card_hover: [76, 86, 106],
                bg_input: [30, 34, 42],
                border_subtle: [76, 86, 106],
                border_strong: [94, 129, 172],
                text_primary: [236, 239, 244],
                text_secondary: [216, 222, 233],
                text_muted: [143, 155, 179],
                accent: [136, 192, 208],
            },
            ThemePreset::Amber => CustomPalette {
                bg_void: [22, 19, 16],
                bg_sidebar: [29, 25, 21],
                bg_elevated: [37, 32, 27],
                bg_card: [45, 39, 32],
                bg_card_hover: [56, 48, 40],
                bg_input: [17, 15, 12],
                border_subtle: [60, 52, 42],
                border_strong: [88, 75, 60],
                accent: [245, 158, 11],
                ..CustomPalette::default()
            },
            ThemePreset::Amethyst => CustomPalette {
                bg_void: [19, 16, 28],
                bg_sidebar: [25, 21, 37],
                bg_elevated: [32, 27, 47],
                bg_card: [39, 33, 57],
                bg_card_hover: [49, 41, 71],
                bg_input: [15, 12, 22],
                border_subtle: [52, 44, 76],
                border_strong: [78, 66, 112],
                accent: [168, 85, 247],
                ..CustomPalette::default()
            },
            ThemePreset::Midnight => CustomPalette {
                bg_void: [11, 16, 30],
                bg_sidebar: [15, 22, 40],
                bg_elevated: [20, 29, 52],
                bg_card: [25, 36, 64],
                bg_card_hover: [33, 47, 82],
                bg_input: [8, 12, 22],
                border_subtle: [39, 54, 92],
                border_strong: [58, 80, 132],
                text_primary: [235, 241, 255],
                text_secondary: [154, 171, 206],
                text_muted: [102, 118, 156],
                accent: [56, 189, 248],
            },
            ThemePreset::Light => CustomPalette {
                bg_void: [241, 244, 249],
                bg_sidebar: [255, 255, 255],
                bg_elevated: [255, 255, 255],
                bg_card: [255, 255, 255],
                bg_card_hover: [232, 237, 245],
                bg_input: [229, 234, 242],
                border_subtle: [208, 215, 226],
                border_strong: [170, 180, 196],
                text_primary: [22, 27, 38],
                text_secondary: [78, 89, 108],
                text_muted: [120, 132, 152],
                accent: [37, 99, 235],
            },
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_custom_accent() -> [u8; 3] {
    [255, 128, 66]
}

fn default_min_ram() -> u32 {
    512
}

fn default_max_ram() -> u32 {
    2048
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherSettings {
    pub language: crate::i18n::Language,
    pub theme_preset: ThemePreset,
    #[serde(default = "default_custom_accent")]
    pub custom_accent: [u8; 3],
    #[serde(default)]
    pub custom_palette: CustomPalette,
    #[serde(default = "default_true")]
    pub discord_rpc_enabled: bool,
    #[serde(default)]
    pub discord_client_id: String,
    #[serde(default)]
    pub ui_font_path: String,
    #[serde(default)]
    pub hide_on_launch: bool,
    #[serde(default = "default_true")]
    pub open_console_on_crash: bool,
    #[serde(default = "default_true")]
    pub check_updates_on_startup: bool,
    #[serde(default)]
    pub auto_update: bool,
    #[serde(default)]
    pub curseforge_api_key: String,
    #[serde(default)]
    pub offline_mode: bool,
    #[serde(default = "default_min_ram")]
    pub default_min_ram_mb: u32,
    #[serde(default = "default_max_ram")]
    pub default_max_ram_mb: u32,
    #[serde(default)]
    pub default_jvm_args: String,
    #[serde(default)]
    pub default_java_path: String,
    #[serde(default)]
    pub default_window_width: Option<u32>,
    #[serde(default)]
    pub default_window_height: Option<u32>,
    #[serde(default)]
    pub default_fullscreen: bool,
    #[serde(default)]
    pub default_prefer_discrete_gpu: bool,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            language: crate::i18n::Language::En,
            theme_preset: ThemePreset::Sunset,
            custom_accent: default_custom_accent(),
            custom_palette: CustomPalette::default(),
            discord_rpc_enabled: true,
            discord_client_id: String::new(),
            ui_font_path: String::new(),
            hide_on_launch: false,
            open_console_on_crash: true,
            check_updates_on_startup: true,
            auto_update: false,
            curseforge_api_key: String::new(),
            offline_mode: false,
            default_min_ram_mb: default_min_ram(),
            default_max_ram_mb: default_max_ram(),
            default_jvm_args: String::new(),
            default_java_path: String::new(),
            default_window_width: None,
            default_window_height: None,
            default_fullscreen: false,
            default_prefer_discrete_gpu: false,
        }
    }
}

impl LauncherSettings {
    pub fn effective_palette(&self) -> CustomPalette {
        if self.theme_preset == ThemePreset::Custom {
            self.custom_palette
        } else {
            self.theme_preset.palette()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sunset_theme_is_first_and_default() {
        assert_eq!(ThemePreset::ALL[0], ThemePreset::Sunset);
        assert_eq!(ThemePreset::default(), ThemePreset::Sunset);
        assert_eq!(
            LauncherSettings::default().theme_preset,
            ThemePreset::Sunset
        );
        assert_eq!(ThemePreset::Sunset.palette().accent, [255, 128, 66]);
        assert_eq!(ThemePreset::Emerald.palette().accent, [27, 217, 106]);
    }

    #[test]
    fn profile_new_with_defaults_applies_settings() {
        let mut s = LauncherSettings::default();
        s.default_min_ram_mb = 2048;
        s.default_max_ram_mb = 6144;
        s.default_jvm_args = "-XX:+UseG1GC".to_string();
        s.default_java_path = "/opt/jdk21/bin/java".to_string();
        s.default_window_width = Some(1920);
        s.default_window_height = Some(1080);
        s.default_fullscreen = true;
        s.default_prefer_discrete_gpu = true;

        let p = Profile::new_with_defaults(
            "Pack".into(),
            "1.21.4".into(),
            ModLoader::Vanilla,
            "Alex".into(),
            &s,
        );
        assert_eq!(p.min_ram_mb, 2048);
        assert_eq!(p.max_ram_mb, 6144);
        assert_eq!(p.jvm_args, "-XX:+UseG1GC");
        assert_eq!(p.java_path, "/opt/jdk21/bin/java");
        assert_eq!(p.window_width, Some(1920));
        assert_eq!(p.window_height, Some(1080));
        assert!(p.fullscreen);
        assert!(p.prefer_discrete_gpu);
    }
}
