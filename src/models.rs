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
pub enum ModLoader {
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

impl Default for ModLoader {
    fn default() -> Self {
        Self::Vanilla
    }
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
}

impl Profile {
    pub fn new(name: String, version_id: String, loader: ModLoader, username: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            version_id,
            loader,
            username,
            min_ram_mb: 512,
            max_ram_mb: 2048,
            java_path: String::new(),
            jvm_args: String::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
            last_played: None,
            pinned: false,
            playtime_secs: 0,
            window_width: None,
            window_height: None,
            fullscreen: false,
            server_address: String::new(),
            prefer_discrete_gpu: false,
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

impl VersionType {
    pub fn label(&self) -> &'static str {
        match self {
            VersionType::Release => "Release",
            VersionType::Snapshot => "Snapshot",
            VersionType::OldBeta => "Old Beta",
            VersionType::OldAlpha => "Old Alpha",
        }
    }
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaVersionInfo {
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionDownloads {
    pub client: Option<DownloadArtifact>,
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
    LaunchFinished(Result<GameHandle, String>),
    GameExited(Option<i32>),
    /// Account tokens were refreshed in the background and must be persisted.
    AccountRefreshed(Account),
    MsDeviceCodeReady(Result<DeviceCodeInfo, String>),
    MsAuthFinished(Result<Account, String>),
    ElyAuthFinished(Result<Account, String>),
    ModrinthSearchFinished {
        generation: u64,
        append: bool,
        result: Result<ModrinthSearchPage, String>,
    },
    ModrinthInstallFinished(Result<ModrinthInstallOutcome, String>),
    ModUpdatesFinished(Result<Vec<String>, String>),
    ProjectVersionsLoaded {
        project_id: String,
        result: Result<Vec<ModrinthVersion>, String>,
    },
    IconLoaded {
        url: String,
        image: Result<IconImage, String>,
    },
    JavaDetected(Vec<JavaInstall>),
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

    pub fn label(self) -> &'static str {
        match self {
            ModrinthSort::Relevance => "Relevance",
            ModrinthSort::Downloads => "Downloads",
            ModrinthSort::Newest => "Newest",
            ModrinthSort::Updated => "Updated",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModrinthProjectType {
    Mod,
    Resourcepack,
    Modpack,
}

impl ModrinthProjectType {
    pub fn api_str(self) -> &'static str {
        match self {
            ModrinthProjectType::Mod => "mod",
            ModrinthProjectType::Resourcepack => "resourcepack",
            ModrinthProjectType::Modpack => "modpack",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ModrinthProjectType::Mod => "Mods",
            ModrinthProjectType::Resourcepack => "Resourcepacks",
            ModrinthProjectType::Modpack => "Modpacks",
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

/// Record of a Modrinth project installed into an instance (stored per instance).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledProject {
    pub title: String,
    pub filename: String,
    pub folder: String,
    pub version_number: String,
    #[serde(default)]
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthHashes {
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MrpackIndex {
    pub name: String,
    pub files: Vec<MrpackFile>,
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MrpackFile {
    pub path: String,
    pub hashes: ModrinthHashes,
    pub env: Option<MrpackEnv>,
    pub downloads: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MrpackEnv {
    pub client: Option<String>,
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
    pub profiles_file: PathBuf,
    pub accounts_file: PathBuf,
}

