mod accounts_discover;
pub mod anim;
mod instance;
mod library;
mod modals;
mod servers;
mod settings_console;
mod widgets;

use self::widgets::*;
use self::anim::{
    compute_icon_anim, ease_out_back, ease_out_cubic, paint_click_ripple, paint_fade_veil,
    paint_shimmer_sweep, pulse_01, track_click_anim,
};
use crate::auth::{
    account_skin_cache_key, check_github_latest_release, download_and_apply_update,
    ensure_authlib_injector, ensure_valid_account, fetch_account_skin, login_ely_by,
    start_microsoft_device_login, DEFAULT_MS_CLIENT_ID,
};
use crate::discord::{
    format_presence, unix_now_secs, DiscordActivityKind, DiscordRpc, PresenceState,
};
use crate::i18n::{tr, Language};
use crate::icons::{draw_brand_logo_animated, draw_icon, draw_icon_animated, draw_player_pixel_avatar, Icon};
use crate::launcher::{
    detect_java_installations, download_java_major, ensure_java_runtime, is_managed_java_path,
    launch_game, open_in_file_manager, remove_managed_java,
};
use crate::models::{
    Account, AccountKind, AccountStore, DedicatedServer, DeviceCodeInfo, FilePickPurpose,
    GameHandle, InstalledProject, JavaInstall, LauncherPaths, LauncherSettings, LauncherUpdateInfo,
    LoaderKind, LoaderVersionEntry, ModLoader, ModrinthInstallOutcome, ModrinthProjectType,
    ModrinthSearchHit, ModrinthSort, ModrinthVersion, Profile, ServerHandle, ServerSoftware,
    ThemePreset, VersionManifest, VersionType, WorkerEvent,
};
use crate::modrinth::{
    backfill_server_installed_index, backup_world, decode_icon_bytes, decode_screenshot_thumbnail,
    delete_content_file, delete_screenshot, delete_world, delete_world_backup,
    delete_world_datapack, disable_incompatible_mods_for_loader, export_instance_mrpack,
    fetch_icon, fetch_project_versions_for_profile, import_local_file, import_modpack_archive,
    import_world_datapack, install_project, install_server_project,
    install_server_specific_version, install_specific_version, list_content_items,
    list_screenshots, list_world_backups, list_world_datapacks, list_worlds, load_installed_index,
    load_server_installed_index, restore_world_backup, search_projects, set_all_content_enabled,
    toggle_content_file, toggle_world_datapack, update_all_installed_projects,
    InstalledContentItem, ScreenshotEntry, SearchParams, WorldBackupEntry, WorldEntry,
};
use crate::mojang::{
    fetch_loader_versions, fetch_version_manifest, prepare_version, request_download_cancel,
    reset_download_cancel, CANCELLED_ERR_MSG,
};
use crate::servers::{
    delete_server_property_entry, detect_local_lan_ip, is_server_provisioned,
    kill_dedicated_server, parse_mc_motd_segments, provision_server_jar,
    read_server_properties_entries, required_java_major_for_mc, save_server_icon_png,
    send_server_command, server_supports_plugins_live, start_dedicated_server,
    stop_dedicated_server, sync_server_properties_from_model, write_eula_and_properties,
    write_server_properties_entries,
};
use crate::storage::{
    delete_account_secret, instance_dir, load_selected_profile_id, load_servers,
    rename_instance_dir_for_profile, resolve_selected_profile_index, save_accounts,
    save_launcher_settings, save_profiles, save_selected_profile_id, save_servers, server_dir,
    unique_instance_dir_name, unique_server_dir_name,
};
use crate::theme::{
    self, animated_card, animated_progress_bar, apply_modrinth_theme, apply_ui_font, badge,
    badge_sized, card_frame, discover_system_fonts, draw_icon_placeholder, draw_instance_avatar,
    draw_instance_image_avatar, draw_ram_allocation_bar, draw_running_pulse_dot,
    draw_server_avatar, draw_skeleton_card, lerp_color, loader_badge, loader_badge_sized,
    mod_toggle_switch, mono_spinner, primary_action_button, section_header,
    server_software_badge, set_theme_palette, solid_accent_badge, tick_theme_transition,
    transition_theme_palette, ui_alert_banner, ui_button, ui_button_icon, ui_button_sized,
    ui_checkbox, ui_flow_row, ui_icon_button, ui_int_slider, ui_loader_pill, ui_pill_tab,
    ui_pill_tab_icon, ui_ram_slider, ui_search_input, ui_text_input, ui_toolbar_row, ui_wrap_row,
    with_alpha, AlertKind, ButtonVariant, SystemFont, accent, accent_text, bg_card,
    bg_card_active, bg_card_hover, bg_elevated, bg_input, bg_sidebar, bg_void, border_subtle,
    DANGER, INFO, text_muted, text_primary, text_secondary, WARNING,
};
use eframe::egui::{self, Color32, Margin, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

const MAX_LOG_LINES: usize = 1500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavPage {
    Library,
    Discover,
    Instance,
    Servers,
    Console,
    Accounts,
    Settings,
}

impl NavPage {
    const ALL: [NavPage; 7] = [
        NavPage::Library,
        NavPage::Servers,
        NavPage::Discover,
        NavPage::Instance,
        NavPage::Console,
        NavPage::Accounts,
        NavPage::Settings,
    ];

    fn label(self) -> &'static str {
        match self {
            NavPage::Library => "Instances",
            NavPage::Discover => "Discover",
            NavPage::Instance => "Instance",
            NavPage::Servers => "Servers",
            NavPage::Console => "Console",
            NavPage::Accounts => "Accounts",
            NavPage::Settings => "Settings",
        }
    }

    fn translated_label(self, lang: Language) -> &'static str {
        match self {
            NavPage::Library => tr(lang, "nav.library"),
            NavPage::Discover => tr(lang, "nav.discover"),
            NavPage::Instance => tr(lang, "nav.instance"),
            NavPage::Servers => tr(lang, "nav.servers"),
            NavPage::Console => tr(lang, "nav.console"),
            NavPage::Accounts => tr(lang, "nav.accounts"),
            NavPage::Settings => tr(lang, "nav.settings"),
        }
    }

    fn icon(self) -> Icon {
        match self {
            NavPage::Library => Icon::Grid,
            NavPage::Discover => Icon::Compass,
            NavPage::Instance => Icon::Sliders,
            NavPage::Servers => Icon::Server,
            NavPage::Console => Icon::Terminal,
            NavPage::Accounts => Icon::User,
            NavPage::Settings => Icon::Sparkle,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerSubTab {
    Console,
    Content,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstanceSubTab {
    Content,
    Worlds,
    Screenshots,
    Settings,
}

impl InstanceSubTab {
    const ALL: [InstanceSubTab; 4] = [
        InstanceSubTab::Content,
        InstanceSubTab::Worlds,
        InstanceSubTab::Screenshots,
        InstanceSubTab::Settings,
    ];

    fn label(self) -> &'static str {
        match self {
            InstanceSubTab::Content => "Content",
            InstanceSubTab::Worlds => "Worlds",
            InstanceSubTab::Screenshots => "Screenshots",
            InstanceSubTab::Settings => "Settings",
        }
    }

    fn translated_label(self, lang: Language) -> &'static str {
        match self {
            InstanceSubTab::Content => tr(lang, "inst.content"),
            InstanceSubTab::Worlds => tr(lang, "inst.worlds"),
            InstanceSubTab::Screenshots => tr(lang, "inst.screenshots"),
            InstanceSubTab::Settings => tr(lang, "inst.settings"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContentFolder {
    Mods,
    Resourcepacks,
    Shaderpacks,
}

impl ContentFolder {
    const ALL: [ContentFolder; 3] = [
        ContentFolder::Mods,
        ContentFolder::Resourcepacks,
        ContentFolder::Shaderpacks,
    ];

    fn dir_name(self) -> &'static str {
        match self {
            ContentFolder::Mods => "mods",
            ContentFolder::Resourcepacks => "resourcepacks",
            ContentFolder::Shaderpacks => "shaderpacks",
        }
    }

    fn label(self) -> &'static str {
        match self {
            ContentFolder::Mods => "Mods",
            ContentFolder::Resourcepacks => "Resourcepacks",
            ContentFolder::Shaderpacks => "Shaderpacks",
        }
    }

    fn translated_label(self, lang: Language) -> &'static str {
        match self {
            ContentFolder::Mods => tr(lang, "content.mods"),
            ContentFolder::Resourcepacks => tr(lang, "content.resourcepacks"),
            ContentFolder::Shaderpacks => tr(lang, "content.shaderpacks"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContentStateFilter {
    All,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LibraryFilter {
    All,
    Modded,
    Vanilla,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LibrarySort {
    Recent,
    Name,
    Playtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LibraryViewMode {
    Grid,
    List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerFilter {
    All,
    Plugins,
    Mods,
    Vanilla,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerSort {
    Recent,
    Name,
    Port,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaletteCategory {
    All,
    Menus,
    Instances,
    Actions,
}

impl PaletteCategory {
    const ALL: [PaletteCategory; 4] = [
        PaletteCategory::All,
        PaletteCategory::Menus,
        PaletteCategory::Instances,
        PaletteCategory::Actions,
    ];

    fn translated_label(self, lang: Language) -> &'static str {
        match self {
            PaletteCategory::All => tr(lang, "lib.all"),
            PaletteCategory::Menus => tr(lang, "palette.cat_menus"),
            PaletteCategory::Instances => tr(lang, "palette.cat_instances"),
            PaletteCategory::Actions => tr(lang, "palette.cat_actions"),
        }
    }

    fn translated_badge(self, lang: Language) -> &'static str {
        match self {
            PaletteCategory::All => tr(lang, "lib.all"),
            PaletteCategory::Menus => tr(lang, "palette.badge_menu"),
            PaletteCategory::Instances => tr(lang, "palette.badge_instance"),
            PaletteCategory::Actions => tr(lang, "palette.badge_action"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogLevel {
    Info,
    Warn,
    Error,
}

struct LogEntry {
    timestamp: String,
    level: LogLevel,
    text: String,
}

struct ToastMessage {
    text: String,
    created_at: f64,
}

#[derive(Default)]
struct ConsoleLayoutCache {
    revision: u64,
    errors_only: bool,
    query: String,
    wrap_width: f32,
    warn_err_count: usize,
    indices: Vec<usize>,
    y_offsets: Vec<f32>,
    diag_revision: u64,
    diag_summary: Option<String>,
}

const CONSOLE_ROW_GAP: f32 = 2.0;

fn visible_console_range(y_offsets: &[f32], viewport_min_y: f32, viewport_max_y: f32) -> (usize, usize) {
    let count = y_offsets.len().saturating_sub(1);
    if count == 0 {
        return (0, 0);
    }
    let first = y_offsets
        .partition_point(|&y| y <= viewport_min_y)
        .saturating_sub(1)
        .min(count.saturating_sub(1));
    let last = y_offsets
        .partition_point(|&y| y < viewport_max_y)
        .clamp(first, count);
    (first, last)
}

const FS_CACHE_TTL: std::time::Duration = std::time::Duration::from_millis(1500);

#[derive(Default)]
struct FsUiCache {
    mod_counts: HashMap<String, (usize, usize, std::time::Instant)>,
    server_counts: HashMap<String, (usize, usize, std::time::Instant)>,
    folder_counts: HashMap<(String, &'static str), (usize, std::time::Instant)>,
    content_items: Option<(String, &'static str, Vec<InstalledContentItem>, std::time::Instant)>,
    worlds: Option<(String, Vec<WorldEntry>, std::time::Instant)>,
    world_backups: Option<(String, Vec<WorldBackupEntry>, std::time::Instant)>,
    screenshots: Option<(String, Vec<ScreenshotEntry>, std::time::Instant)>,
}

impl FsUiCache {
    fn invalidate(&mut self) {
        self.mod_counts.clear();
        self.server_counts.clear();
        self.folder_counts.clear();
        self.content_items = None;
        self.worlds = None;
        self.world_backups = None;
        self.screenshots = None;
    }
}

pub struct McLauncherApp {
    paths: LauncherPaths,
    profiles: Vec<Profile>,
    selected_idx: usize,
    nav_page: NavPage,
    instance_tab: InstanceSubTab,
    page_switch_time: f64,
    subtab_switch_time: f64,

    library_search: String,
    library_filter: LibraryFilter,
    library_sort: LibrarySort,
    library_view: LibraryViewMode,
    server_search: String,
    server_filter: ServerFilter,
    server_sort: ServerSort,
    server_view: LibraryViewMode,
    content_folder: ContentFolder,
    content_state_filter: ContentStateFilter,
    content_search: String,

    manifest: Option<VersionManifest>,
    manifest_loading: bool,
    manifest_error: Option<String>,

    loader_cache: HashMap<(LoaderKind, String), Vec<LoaderVersionEntry>>,
    loader_loading: HashSet<(LoaderKind, String)>,

    java_installs: Vec<JavaInstall>,
    java_scanning: bool,

    show_snapshots: bool,

    is_busy: bool,
    game_handle: Option<GameHandle>,
    running_profile_id: Option<String>,
    launch_started_at: Option<std::time::Instant>,
    progress: f32,
    status_text: String,

    // Live Console state
    logs: VecDeque<LogEntry>,
    log_search: String,
    log_errors_only: bool,
    log_auto_scroll: bool,
    log_error_count: usize,
    log_revision: u64,
    log_cache: ConsoleLayoutCache,
    fs_cache: FsUiCache,

    // New profile modal state
    show_new_modal: bool,
    new_name: String,
    new_version: String,
    new_loader: LoaderKind,
    new_ram_mb: u32,
    new_show_snapshots: bool,
    new_username: String,

    // Delete confirmation modal state
    confirm_delete_idx: Option<usize>,

    // Accounts & Auth state
    account_store: AccountStore,
    offline_input: String,
    ely_login: String,
    ely_password: String,
    ely_totp: String,
    ely_show_password: bool,
    ely_busy: bool,
    ms_client_id: String,
    ms_show_custom_id: bool,
    ms_device_code: Option<DeviceCodeInfo>,
    ms_busy: bool,
    auth_error: Option<String>,
    auth_status: Option<String>,

    // Modrinth Discover state
    mr_query: String,
    mr_type: ModrinthProjectType,
    mr_sort: ModrinthSort,
    mr_search_gen: u64,
    mr_loaded_filter_key: String,
    mr_results: Vec<ModrinthSearchHit>,
    mr_total_hits: u64,
    mr_next_offset: usize,
    mr_has_more: bool,
    mr_searching: bool,
    mr_installing: bool,
    mr_installing_id: Option<String>,
    mr_error: Option<String>,
    mr_status: Option<String>,
    mr_icons: HashMap<String, Option<egui::TextureHandle>>,
    mr_icons_pending: HashSet<String>,
    mr_installed_cache: HashMap<String, InstalledProject>,
    mr_installed_profile_id: String,
    mr_detail_hit: Option<ModrinthSearchHit>,
    mr_detail_versions: Option<Result<Vec<ModrinthVersion>, String>>,
    mr_detail_loading: bool,

    show_cmd_palette: bool,
    cmd_palette_query: String,
    cmd_palette_selected: usize,
    cmd_palette_category: PaletteCategory,
    cmd_palette_just_opened: bool,

    toast: Option<ToastMessage>,

    settings: LauncherSettings,
    system_fonts: Vec<SystemFont>,
    brand_logo_tex: Option<Option<egui::TextureHandle>>,
    profile_icons: HashMap<String, Option<egui::TextureHandle>>,
    skin_heads: HashMap<String, Option<egui::TextureHandle>>,
    skin_bodies: HashMap<String, Option<egui::TextureHandle>>,
    skin_pending: HashSet<String>,
    expanded_world_datapacks: Option<String>,
    screenshot_thumbs: HashMap<String, Option<egui::TextureHandle>>,
    file_picker_open: bool,
    pending_world_delete: Option<String>,
    pending_backup_delete: Option<String>,
    world_backup_busy: bool,
    last_crash_code: Option<i32>,
    update_checking: bool,
    update_installing: bool,
    update_available: Option<LauncherUpdateInfo>,
    update_checked_up_to_date: bool,
    update_error: Option<String>,
    discord_rpc: DiscordRpc,
    rpc_started_at: i64,
    java_downloading_major: Option<u32>,

    // Dedicated servers state
    servers: Vec<DedicatedServer>,
    selected_server_idx: usize,
    server_detail_open: bool,
    server_tab: ServerSubTab,
    server_content_is_plugins: bool,
    discover_target_server_id: Option<String>,
    server_index_backfilled: HashSet<String>,
    server_icons: HashMap<String, Option<egui::TextureHandle>>,
    server_props_cache: Option<(String, Vec<(String, String)>)>,
    server_props_expanded: bool,
    server_props_filter: String,
    server_props_new_key: String,
    server_props_new_val: String,
    server_port_buf: Option<(String, String)>,
    local_lan_ip: Option<Option<String>>,
    server_handle: Option<ServerHandle>,
    running_server_id: Option<String>,
    server_tunnel_addr: Option<String>,
    server_tunnel_error: Option<String>,
    server_busy: bool,
    server_logs: VecDeque<String>,
    server_cmd_input: String,
    show_create_server_modal: bool,
    new_srv_name: String,
    new_srv_software: ServerSoftware,
    new_srv_version: String,
    new_srv_port: String,
    new_srv_max_ram: u32,
    new_srv_online_mode: bool,
    new_srv_upnp: bool,
    new_srv_tunnel: bool,
    new_srv_motd: String,
    new_srv_eula: bool,
    confirm_delete_server_idx: Option<usize>,

    tx: Sender<WorkerEvent>,
    rx: Receiver<WorkerEvent>,
    icon_tx: Sender<String>,
}

impl McLauncherApp {
    pub fn new(
        paths: LauncherPaths,
        profiles: Vec<Profile>,
        account_store: AccountStore,
        settings: LauncherSettings,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let (icon_tx, icon_rx) = mpsc::channel::<String>();
        let icon_rx = Arc::new(Mutex::new(icon_rx));
        for _ in 0..4 {
            let rx_shared = Arc::clone(&icon_rx);
            let ev_tx = tx.clone();
            thread::spawn(move || loop {
                let Ok(url) = rx_shared.lock().map(|r| r.recv()) else {
                    break;
                };
                let Ok(url) = url else {
                    break;
                };
                let image = fetch_icon(&url);
                let _ = ev_tx.send(WorkerEvent::IconLoaded { url, image });
            });
        }
        let servers = load_servers(&paths);
        let saved_profile_id = load_selected_profile_id(&paths);
        let selected_idx =
            resolve_selected_profile_index(&profiles, saved_profile_id.as_deref());
        let mut app = Self {
            paths,
            profiles,
            selected_idx,
            nav_page: NavPage::Library,
            instance_tab: InstanceSubTab::Content,
            page_switch_time: 0.0,
            subtab_switch_time: 0.0,
            library_search: String::new(),
            library_filter: LibraryFilter::All,
            library_sort: LibrarySort::Recent,
            library_view: LibraryViewMode::Grid,
            server_search: String::new(),
            server_filter: ServerFilter::All,
            server_sort: ServerSort::Recent,
            server_view: LibraryViewMode::Grid,
            content_folder: ContentFolder::Mods,
            content_state_filter: ContentStateFilter::All,
            content_search: String::new(),
            manifest: None,
            manifest_loading: false,
            manifest_error: None,
            loader_cache: HashMap::new(),
            loader_loading: HashSet::new(),
            java_installs: Vec::new(),
            java_scanning: true,
            show_snapshots: false,
            is_busy: false,
            game_handle: None,
            running_profile_id: None,
            launch_started_at: None,
            progress: 0.0,
            status_text: String::new(),
            logs: VecDeque::with_capacity(MAX_LOG_LINES),
            log_search: String::new(),
            log_errors_only: false,
            log_auto_scroll: true,
            log_error_count: 0,
            log_revision: 1,
            log_cache: ConsoleLayoutCache::default(),
            fs_cache: FsUiCache::default(),
            show_new_modal: false,
            new_name: "New Instance".to_string(),
            new_version: "1.21.4".to_string(),
            new_loader: LoaderKind::Fabric,
            new_ram_mb: 4096,
            new_show_snapshots: false,
            new_username: "Player".to_string(),
            confirm_delete_idx: None,
            account_store,
            offline_input: String::new(),
            ely_login: String::new(),
            ely_password: String::new(),
            ely_totp: String::new(),
            ely_show_password: false,
            ely_busy: false,
            ms_client_id: DEFAULT_MS_CLIENT_ID.to_string(),
            ms_show_custom_id: false,
            ms_device_code: None,
            ms_busy: false,
            auth_error: None,
            auth_status: None,
            mr_query: String::new(),
            mr_type: ModrinthProjectType::Mod,
            mr_sort: ModrinthSort::Downloads,
            mr_search_gen: 0,
            mr_loaded_filter_key: String::new(),
            mr_results: Vec::new(),
            mr_total_hits: 0,
            mr_next_offset: 0,
            mr_has_more: false,
            mr_searching: false,
            mr_installing: false,
            mr_installing_id: None,
            mr_error: None,
            mr_status: None,
            mr_icons: HashMap::new(),
            mr_icons_pending: HashSet::new(),
            mr_installed_cache: HashMap::new(),
            mr_installed_profile_id: String::new(),
            mr_detail_hit: None,
            mr_detail_versions: None,
            mr_detail_loading: false,
            show_cmd_palette: false,
            cmd_palette_query: String::new(),
            cmd_palette_selected: 0,
            cmd_palette_category: PaletteCategory::All,
            cmd_palette_just_opened: false,
            toast: None,
            settings,
            system_fonts: discover_system_fonts(),
            brand_logo_tex: None,
            profile_icons: HashMap::new(),
            skin_heads: HashMap::new(),
            skin_bodies: HashMap::new(),
            skin_pending: HashSet::new(),
            expanded_world_datapacks: None,
            screenshot_thumbs: HashMap::new(),
            file_picker_open: false,
            pending_world_delete: None,
            pending_backup_delete: None,
            world_backup_busy: false,
            last_crash_code: None,
            update_checking: false,
            update_installing: false,
            update_available: None,
            update_checked_up_to_date: false,
            update_error: None,
            discord_rpc: DiscordRpc::spawn(),
            rpc_started_at: unix_now_secs(),
            java_downloading_major: None,
            servers,
            selected_server_idx: 0,
            server_detail_open: false,
            server_tab: ServerSubTab::Console,
            server_content_is_plugins: true,
            discover_target_server_id: None,
            server_index_backfilled: HashSet::new(),
            server_icons: HashMap::new(),
            server_props_cache: None,
            server_props_expanded: false,
            server_props_filter: String::new(),
            server_props_new_key: String::new(),
            server_props_new_val: String::new(),
            server_port_buf: None,
            local_lan_ip: None,
            server_handle: None,
            running_server_id: None,
            server_tunnel_addr: None,
            server_tunnel_error: None,
            server_busy: false,
            server_logs: VecDeque::with_capacity(MAX_LOG_LINES),
            server_cmd_input: String::new(),
            show_create_server_modal: false,
            new_srv_name: "Survival Server".to_string(),
            new_srv_software: ServerSoftware::Paper,
            new_srv_version: "1.21.4".to_string(),
            new_srv_port: "25565".to_string(),
            new_srv_max_ram: 2048,
            new_srv_online_mode: true,
            new_srv_upnp: true,
            new_srv_tunnel: false,
            new_srv_motd: "A Sunset Launcher Server".to_string(),
            new_srv_eula: false,
            confirm_delete_server_idx: None,
            tx,
            rx,
            icon_tx,
        };
        app.refresh_manifest();
        app.scan_java();
        app.sync_installed_index();
        app.trigger_modrinth_search(false);
        app.sync_discord_presence();
        if app.settings.check_updates_on_startup {
            app.check_launcher_updates();
        }
        app
    }

    fn show_toast(&mut self, ctx: &egui::Context, text: impl Into<String>) {
        let now = ctx.input(|i| i.time);
        self.toast = Some(ToastMessage {
            text: text.into(),
            created_at: now,
        });
    }

    fn current_discord_kind(&self) -> DiscordActivityKind {
        if let Some(rid) = &self.running_profile_id {
            if let Some(p) = self.profiles.iter().find(|p| &p.id == rid) {
                return DiscordActivityKind::Playing {
                    instance: p.name.clone(),
                    version: p.version_id.clone(),
                    loader: p.loader.kind().label().to_string(),
                };
            }
        }
        match self.nav_page {
            NavPage::Library | NavPage::Servers | NavPage::Settings => {
                DiscordActivityKind::InLauncher
            }
            NavPage::Discover => DiscordActivityKind::BrowsingModrinth,
            NavPage::Instance => {
                let name = self
                    .profiles
                    .get(self.selected_idx)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "Instance".to_string());
                DiscordActivityKind::ConfiguringInstance { name }
            }
            NavPage::Console => DiscordActivityKind::Console,
            NavPage::Accounts => DiscordActivityKind::Accounts,
        }
    }

    fn sync_discord_presence(&self) {
        self.discord_rpc.update(PresenceState {
            enabled: self.settings.discord_rpc_enabled,
            language: self.settings.language,
            kind: self.current_discord_kind(),
            started_at: self.rpc_started_at,
            client_id: self.settings.discord_client_id.trim().to_string(),
        });
    }

    fn download_java_version(&mut self, major: u32) {
        if self.java_downloading_major.is_some() {
            return;
        }
        self.java_downloading_major = Some(major);
        let paths = self.paths.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            match download_java_major(&paths, major, Some(&tx)) {
                Ok(bin) => {
                    let _ = tx.send(WorkerEvent::Log(format!(
                        "[Java] Ready: Java {major} at {}",
                        bin.display()
                    )));
                }
                Err(e) => {
                    let _ = tx.send(WorkerEvent::Log(format!(
                        "[Java ERROR] Failed to download Java {major}: {e}"
                    )));
                }
            }
            let _ = tx.send(WorkerEvent::JavaDetected(detect_java_installations()));
        });
    }

    fn switch_page(&mut self, ctx: &egui::Context, page: NavPage) {
        if self.nav_page != page {
            self.nav_page = page;
            self.page_switch_time = ctx.input(|i| i.time);
            ctx.request_repaint();
            if page == NavPage::Console {
                self.log_error_count = 0;
            } else if page == NavPage::Discover {
                self.sync_discover_for_profile();
            }
            self.sync_discord_presence();
        }
    }

    fn mark_subtab_switch(&mut self, ctx: &egui::Context) {
        self.subtab_switch_time = ctx.input(|i| i.time);
        ctx.request_repaint();
    }

    fn apply_subtab_transition(
        ui: &mut egui::Ui,
        switch_time: f64,
        add_contents: impl FnOnce(&mut egui::Ui),
    ) {
        let now = ui.input(|i| i.time);
        let raw_t = ((now - switch_time) / 0.16).clamp(0.0, 1.0) as f32;
        let eased = ease_out_cubic(raw_t);
        if raw_t < 1.0 {
            ui.ctx().request_repaint();
        }
        let resp = ui.vertical(|ui| {
            if raw_t < 1.0 {
                ui.add_space((1.0 - eased) * 8.0);
            }
            add_contents(ui);
        });
        if raw_t < 0.99 {
            paint_fade_veil(ui.painter(), resp.response.rect, bg_void(), raw_t, 125.0);
        }
    }

    fn push_log(&mut self, line: String) {
        let lower = line.to_lowercase();
        let level = if lower.contains("error")
            || lower.contains("exception")
            || lower.contains("fatal")
            || lower.contains("[stderr]")
        {
            LogLevel::Error
        } else if lower.contains("warn") {
            LogLevel::Warn
        } else {
            LogLevel::Info
        };
        if level == LogLevel::Error && self.nav_page != NavPage::Console {
            self.log_error_count += 1;
        }
        if self.logs.len() >= MAX_LOG_LINES {
            self.logs.pop_front();
        }
        self.logs.push_back(LogEntry {
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            level,
            text: line,
        });
        self.log_revision = self.log_revision.wrapping_add(1);
    }

    fn clear_logs(&mut self) {
        self.logs.clear();
        self.log_error_count = 0;
        self.last_crash_code = None;
        self.log_revision = self.log_revision.wrapping_add(1);
    }

    fn scan_java(&mut self) {
        self.java_scanning = true;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let installs = detect_java_installations();
            let _ = tx.send(WorkerEvent::JavaDetected(installs));
        });
    }

    fn persist_selected_profile(&self) {
        let id = self.profiles.get(self.selected_idx).map(|p| p.id.as_str());
        let _ = save_selected_profile_id(&self.paths, id);
    }

    fn select_profile(&mut self, idx: usize) {
        if idx < self.profiles.len() {
            self.selected_idx = idx;
            self.persist_selected_profile();
            self.sync_installed_index();
            self.sync_discover_for_profile();
            if self.nav_page == NavPage::Instance {
                self.sync_discord_presence();
            }
        }
    }

    fn current_profile_filter_key(&self) -> String {
        self.profiles
            .get(self.selected_idx)
            .map(|p| format!("{}|{:?}", p.version_id, p.loader.kind()))
            .unwrap_or_default()
    }

    fn sync_discover_for_profile(&mut self) {
        let key = self.current_profile_filter_key();
        if self.mr_loaded_filter_key != key {
            self.mr_results.clear();
            self.mr_total_hits = 0;
            self.mr_next_offset = 0;
            self.mr_has_more = false;
            self.mr_error = None;
            if self.nav_page == NavPage::Discover {
                self.trigger_modrinth_search(false);
            }
        }
    }

    fn sync_installed_index(&mut self) {
        if let Some(p) = self.profiles.get(self.selected_idx) {
            if self.mr_installed_profile_id != p.id {
                self.mr_installed_cache = load_installed_index(&self.paths, p);
                self.mr_installed_profile_id = p.id.clone();
                self.fs_cache.invalidate();
            }
        }
    }

    fn duplicate_profile(&mut self, ctx: &egui::Context, idx: usize) {
        let Some(orig) = self.profiles.get(idx).cloned() else {
            return;
        };
        let mut copy = orig.clone();
        copy.id = uuid::Uuid::new_v4().to_string();
        copy.name = format!("{} (Copy)", orig.name);
        copy.dir_name = unique_instance_dir_name(&self.paths, &copy.name, &self.profiles);
        copy.created_at = chrono::Utc::now().to_rfc3339();
        copy.last_played = None;
        self.profiles.push(copy);
        let new_idx = self.profiles.len() - 1;
        let _ = save_profiles(&self.paths, &self.profiles);
        self.select_profile(new_idx);
        let msg = format!("{}: '{}'", tr(self.settings.language, "toast.duplicated"), orig.name);
        self.show_toast(ctx, msg);
    }

    fn active_account(&self) -> Option<&Account> {
        if let Some(id) = &self.account_store.active_id {
            if let Some(acc) = self.account_store.accounts.iter().find(|a| &a.id == id) {
                return Some(acc);
            }
        }
        self.account_store.accounts.first()
    }

    fn refresh_manifest(&mut self) {
        if self.manifest_loading {
            return;
        }
        self.manifest_loading = true;
        self.manifest_error = None;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = fetch_version_manifest();
            let _ = tx.send(WorkerEvent::ManifestLoaded(res));
        });
    }

    fn request_loader_versions(&mut self, loader: LoaderKind, game_version: &str) {
        if !loader.is_modded() {
            return;
        }
        let key = (loader, game_version.to_string());
        if self.loader_cache.contains_key(&key) || self.loader_loading.contains(&key) {
            return;
        }
        self.loader_loading.insert(key);
        let gv = game_version.to_string();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = fetch_loader_versions(loader, &gv);
            let _ = tx.send(WorkerEvent::LoaderVersionsLoaded {
                loader,
                game_version: gv,
                result: res,
            });
        });
    }

    fn trigger_modrinth_search(&mut self, append: bool) {
        if append && self.mr_searching {
            return;
        }
        self.mr_search_gen = self.mr_search_gen.wrapping_add(1);
        let generation = self.mr_search_gen;
        self.mr_searching = true;
        self.mr_error = None;
        self.mr_loaded_filter_key = self.current_profile_filter_key();
        if !append {
            self.mr_results.clear();
            self.mr_total_hits = 0;
            self.mr_next_offset = 0;
            self.mr_has_more = false;
        }
        let offset = if append { self.mr_next_offset } else { 0 };
        let (game_version, loader_filter) = if let Some(srv_id) = &self.discover_target_server_id {
            if let Some(srv) = self.servers.iter().find(|s| &s.id == srv_id) {
                let lf = if self.mr_type == ModrinthProjectType::Mod {
                    Some(
                        srv.software
                            .modrinth_mod_loader()
                            .unwrap_or(LoaderKind::Vanilla),
                    )
                } else {
                    None
                };
                (Some(srv.mc_version.clone()), lf)
            } else {
                (None, None)
            }
        } else {
            self.profiles
                .get(self.selected_idx)
                .map(|p| (Some(p.version_id.clone()), Some(p.loader.kind())))
                .unwrap_or((None, None))
        };

        let server_software = self
            .discover_target_server_id
            .as_ref()
            .and_then(|id| self.servers.iter().find(|s| &s.id == id))
            .map(|s| s.software);
        let params = SearchParams {
            query: self.mr_query.clone(),
            project_type: self.mr_type,
            sort: self.mr_sort,
            game_version,
            loader_filter,
            server_software,
            offset,
        };
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = search_projects(&params);
            let _ = tx.send(WorkerEvent::ModrinthSearchFinished {
                generation,
                append,
                result: res,
            });
        });
    }

    fn request_icon(&mut self, url: &str) {
        if url.is_empty() || self.mr_icons.contains_key(url) || self.mr_icons_pending.contains(url) {
            return;
        }
        self.mr_icons_pending.insert(url.to_string());
        let _ = self.icon_tx.send(url.to_string());
    }

    fn start_launch(&mut self) {
        self.start_launch_with_world(None);
    }

    fn start_launch_with_world(&mut self, quick_world: Option<String>) {
        if self.is_busy || self.game_handle.is_some() {
            return;
        }
        let Some(profile) = self.profiles.get_mut(self.selected_idx) else {
            return;
        };
        profile.last_played = Some(chrono::Utc::now().to_rfc3339());
        let profile_clone = profile.clone();
        self.running_profile_id = Some(profile_clone.id.clone());
        let _ = save_profiles(&self.paths, &self.profiles);

        let mut account = self
            .active_account()
            .cloned()
            .unwrap_or_else(|| Account::offline(profile_clone.username.clone()));

        reset_download_cancel();
        self.is_busy = true;
        self.last_crash_code = None;
        self.progress = 0.03;
        self.status_text = format!(
            "{} {}...",
            tr(self.settings.language, "status.preparing"),
            profile_clone.version_id
        );
        self.push_log(format!(
            "[Launcher] Preparing '{}' ({} • {}) as {}",
            profile_clone.name,
            profile_clone.version_id,
            profile_clone.loader.label(),
            account.username
        ));

        let paths = self.paths.clone();
        let manifest = self.manifest.clone();
        let ms_client_id = self.ms_client_id.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = (|| -> Result<GameHandle, String> {
                let before_token_check = serde_json::to_string(&account).ok();
                ensure_valid_account(&mut account, &ms_client_id)?;
                if serde_json::to_string(&account).ok() != before_token_check {
                    let _ = tx.send(WorkerEvent::AccountRefreshed(account.clone()));
                }

                let injector_path = if matches!(account.kind, AccountKind::ElyBy { .. }) {
                    let _ = tx.send(WorkerEvent::Progress {
                        stage: "Checking authlib-injector for Ely.by...".to_string(),
                        fraction: 0.04,
                    });
                    Some(ensure_authlib_injector(&paths)?)
                } else {
                    None
                };

                let prepared = prepare_version(
                    &paths,
                    &profile_clone.version_id,
                    &profile_clone.loader,
                    manifest.as_ref(),
                    &|req| ensure_java_runtime(&paths, &profile_clone, req, Some(&tx)),
                    &tx,
                )?;
                launch_game(
                    &paths,
                    &profile_clone,
                    &account,
                    &prepared,
                    injector_path.as_deref(),
                    quick_world.as_deref(),
                    tx.clone(),
                )
            })();
            let _ = tx.send(WorkerEvent::LaunchFinished(res));
        });
    }

    fn check_launcher_updates(&mut self) {
        if self.update_checking || self.update_installing {
            return;
        }
        self.update_checking = true;
        self.update_error = None;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = check_github_latest_release(env!("CARGO_PKG_VERSION"));
            let _ = tx.send(WorkerEvent::LauncherUpdateChecked(res));
        });
    }

    fn start_launcher_update(&mut self) {
        if self.update_installing || self.is_busy || self.game_handle.is_some() {
            return;
        }
        let Some(info) = self.update_available.clone() else {
            return;
        };
        if info.download_url.is_none() {
            self.update_error = Some("No downloadable binary asset found for this platform".to_string());
            return;
        }
        self.update_installing = true;
        self.is_busy = true;
        self.progress = 0.05;
        self.update_error = None;
        self.status_text = format!(
            "{} {}...",
            tr(self.settings.language, "status.downloading_update"),
            info.version
        );
        let paths = self.paths.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = download_and_apply_update(&paths, &info, &tx);
            let _ = tx.send(WorkerEvent::LauncherUpdateInstalled(res));
        });
    }

    fn request_account_skin(&mut self, acc: &Account, force_refresh: bool) {
        let key = account_skin_cache_key(acc);
        if !force_refresh && (self.skin_heads.contains_key(&key) || self.skin_pending.contains(&key))
        {
            return;
        }
        self.skin_pending.insert(key.clone());
        let paths = self.paths.clone();
        let acc = acc.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = fetch_account_skin(&paths, &acc, force_refresh);
            let (head, body) = match res {
                Ok((h, b)) => (Ok(h), Ok(b)),
                Err(e) => (Err(e.clone()), Err(e)),
            };
            let _ = tx.send(WorkerEvent::SkinLoaded { key, head, body });
        });
    }

    fn ensure_account_skins_requested(&mut self) {
        let missing: Vec<Account> = self
            .account_store
            .accounts
            .iter()
            .filter(|a| {
                let key = account_skin_cache_key(a);
                !self.skin_heads.contains_key(&key) && !self.skin_pending.contains(&key)
            })
            .cloned()
            .collect();
        for acc in missing {
            self.request_account_skin(&acc, false);
        }
    }

    fn refresh_all_skins(&mut self) {
        self.skin_heads.clear();
        self.skin_bodies.clear();
        self.skin_pending.clear();
        let all = self.account_store.accounts.clone();
        for acc in &all {
            self.request_account_skin(acc, true);
        }
    }

    fn request_file_pick(&mut self, purpose: FilePickPurpose) {
        if self.file_picker_open {
            return;
        }
        self.file_picker_open = true;
        let tx = self.tx.clone();
        let (title, filter_label, exts): (&'static str, &'static str, &'static [&'static str]) =
            match &purpose {
                FilePickPurpose::ModpackImport => (
                    "Import Modpack (.mrpack / .zip)",
                    "Modpacks (.mrpack, .zip)",
                    &["mrpack", "zip"],
                ),
                FilePickPurpose::InstanceContentImport => (
                    "Import Mod / Resourcepack / Shader",
                    "Minecraft Content",
                    &["jar", "zip", "mrpack"],
                ),
                FilePickPurpose::WorldDatapackImport { .. } => {
                    ("Import World Datapack (.zip)", "Datapacks", &["zip"])
                }
                FilePickPurpose::InstanceIcon | FilePickPurpose::ServerIcon => (
                    "Select Image",
                    "Images",
                    &["png", "jpg", "jpeg", "webp", "gif", "bmp", "ico"],
                ),
                FilePickPurpose::JavaBinary | FilePickPurpose::ServerJavaBinary => {
                    #[cfg(windows)]
                    {
                        ("Select Java Executable", "Java", &["exe"])
                    }
                    #[cfg(not(windows))]
                    {
                        ("Select Java Executable", "All Files", &["*"])
                    }
                }
                FilePickPurpose::CustomUiFont => {
                    ("Select Font (.ttf / .otf)", "Fonts", &["ttf", "otf"])
                }
            };
        thread::spawn(move || {
            let path = pick_file(title, filter_label, exts);
            let _ = tx.send(WorkerEvent::FilePicked { purpose, path });
        });
    }

    fn import_mrpack_from_path(&mut self, mrpack_path: std::path::PathBuf) {
        if self.mr_installing {
            return;
        }
        reset_download_cancel();
        let username = self
            .active_account()
            .map(|a| a.username.clone())
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| "Player".to_string());
        let paths = self.paths.clone();
        let cf_key = self.settings.curseforge_api_key.clone();
        let tx = self.tx.clone();
        self.mr_installing = true;
        self.progress = 0.08;
        self.status_text = format!(
            "{} {}...",
            tr(self.settings.language, "status.importing"),
            mrpack_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "modpack".to_string())
        );
        thread::spawn(move || {
            let res = import_modpack_archive(&paths, &mrpack_path, username, &cf_key, &tx);
            let _ = tx.send(WorkerEvent::ModrinthInstallFinished(res));
        });
    }

    fn stop_game(&mut self) {
        let mut stopped = false;
        if let Some(handle) = &self.game_handle {
            if let Ok(mut child) = handle.lock() {
                let _ = child.kill();
                stopped = true;
            }
        }
        if stopped {
            self.status_text = tr(self.settings.language, "status.stopping").to_string();
            self.push_log("[Launcher] Sent stop signal to game process".to_string());
        }
    }

    fn poll_events(&mut self, ctx: &egui::Context) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                WorkerEvent::ManifestLoaded(res) => {
                    self.manifest_loading = false;
                    match res {
                        Ok(m) => {
                            self.new_version = m.latest.release.clone();
                            self.manifest = Some(m);
                        }
                        Err(e) => {
                            self.manifest_error = Some(e);
                        }
                    }
                }
                WorkerEvent::LoaderVersionsLoaded {
                    loader,
                    game_version,
                    result,
                } => {
                    self.loader_loading.remove(&(loader, game_version.clone()));
                    if let Ok(list) = result {
                        if let Some(profile) = self.profiles.get_mut(self.selected_idx) {
                            if profile.version_id == game_version && profile.loader.kind() == loader
                            {
                                if let Some(ver_slot) = profile.loader.loader_version_mut() {
                                    if ver_slot.is_empty() {
                                        if let Some(first) = list
                                            .iter()
                                            .find(|v| v.stable)
                                            .or_else(|| list.first())
                                        {
                                            *ver_slot = first.version.clone();
                                            let _ = save_profiles(&self.paths, &self.profiles);
                                        }
                                    }
                                }
                            }
                        }
                        self.loader_cache.insert((loader, game_version), list);
                    }
                }
                WorkerEvent::JavaDetected(installs) => {
                    self.java_scanning = false;
                    self.java_downloading_major = None;
                    self.java_installs = installs;
                }
                WorkerEvent::Progress { stage, fraction } => {
                    self.progress = fraction;
                    self.status_text = stage;
                }
                WorkerEvent::Log(line) => {
                    println!("{line}");
                    self.push_log(line);
                }
                WorkerEvent::AccountRefreshed(updated) => {
                    if let Some(slot) = self
                        .account_store
                        .accounts
                        .iter_mut()
                        .find(|a| a.id == updated.id)
                    {
                        *slot = updated;
                        let _ = save_accounts(&self.paths, &self.account_store);
                    }
                }
                WorkerEvent::LaunchFinished(res) => {
                    self.is_busy = false;
                    match res {
                        Ok(handle) => {
                            let lang = self.settings.language;
                            self.game_handle = Some(handle);
                            self.launch_started_at = Some(std::time::Instant::now());
                            self.last_crash_code = None;
                            self.status_text = tr(lang, "status.running").to_string();
                            self.push_log("[Launcher] Minecraft process started".to_string());
                            let launched_msg = tr(lang, "toast.game_launched").to_string();
                            self.show_toast(ctx, launched_msg);
                            anim::emit_edge_pulse(ctx, accent());
                            self.scan_java();
                            self.sync_discord_presence();
                            if self.settings.hide_on_launch {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                            }
                        }
                        Err(e) if e == CANCELLED_ERR_MSG => {
                            let lang = self.settings.language;
                            self.running_profile_id = None;
                            self.launch_started_at = None;
                            self.progress = 0.0;
                            let msg = tr(lang, "status.cancelled").to_string();
                            self.status_text = msg.clone();
                            self.push_log("[Launcher] Cancelled by user".to_string());
                            self.show_toast(ctx, msg);
                            self.sync_discord_presence();
                        }
                        Err(e) => {
                            let lang = self.settings.language;
                            self.running_profile_id = None;
                            self.launch_started_at = None;
                            self.progress = 0.0;
                            self.status_text = format!("{}: {e}", tr(lang, "status.error"));
                            self.push_log(format!("[Launcher ERROR] {e}"));
                            self.show_toast(
                                ctx,
                                format!("{}: {e}", tr(lang, "status.launch_failed")),
                            );
                            anim::emit_edge_pulse(ctx, DANGER);
                            self.sync_discord_presence();
                        }
                    }
                }
                WorkerEvent::GameExited(code) => {
                    if let (Some(started), Some(rid)) =
                        (self.launch_started_at.take(), self.running_profile_id.as_deref())
                    {
                        let elapsed = started.elapsed().as_secs().max(1);
                        if let Some(p) = self.profiles.iter_mut().find(|p| p.id == rid) {
                            p.playtime_secs = p.playtime_secs.saturating_add(elapsed);
                            let _ = save_profiles(&self.paths, &self.profiles);
                        }
                    }
                    self.game_handle = None;
                    self.running_profile_id = None;
                    self.progress = 0.0;
                    let msg = match code {
                        Some(0) => "Game exited (code 0)".to_string(),
                        Some(c) => format!("Game exited (code {c})"),
                        None => "Game exited".to_string(),
                    };
                    self.status_text = msg.clone();
                    self.push_log(format!("[Launcher] {msg}"));
                    self.sync_discord_presence();
                    if self.settings.hide_on_launch {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    if matches!(code, Some(c) if c != 0) {
                        self.last_crash_code = code;
                        anim::emit_edge_pulse(ctx, DANGER);
                        if self.settings.open_console_on_crash {
                            self.switch_page(ctx, NavPage::Console);
                        }
                    }
                }
                WorkerEvent::MsDeviceCodeReady(res) => match res {
                    Ok(info) => {
                        self.ms_device_code = Some(info);
                        self.auth_error = None;
                        self.auth_status =
                            Some("Waiting for browser confirmation...".to_string());
                    }
                    Err(e) => {
                        self.ms_busy = false;
                        self.ms_device_code = None;
                        self.auth_error = Some(e);
                    }
                },
                WorkerEvent::MsAuthFinished(res) => {
                    self.ms_busy = false;
                    self.ms_device_code = None;
                    match res {
                        Ok(acc) => {
                            let name = acc.username.clone();
                            self.account_store.active_id = Some(acc.id.clone());
                            self.account_store.accounts.push(acc);
                            let _ = save_accounts(&self.paths, &self.account_store);
                            self.auth_error = None;
                            let msg = format!("Signed in with Microsoft: {name}");
                            self.auth_status = Some(msg.clone());
                            self.show_toast(ctx, msg);
                            anim::emit_burst(ctx, anim::pointer_or_center(ctx), accent(), 10);
                            anim::emit_edge_pulse(ctx, accent());
                        }
                        Err(e) => {
                            self.auth_error = Some(e);
                            anim::emit_edge_pulse(ctx, DANGER);
                        }
                    }
                }
                WorkerEvent::ElyAuthFinished(res) => {
                    self.ely_busy = false;
                    match res {
                        Ok(acc) => {
                            let name = acc.username.clone();
                            self.account_store.active_id = Some(acc.id.clone());
                            self.account_store.accounts.push(acc);
                            let _ = save_accounts(&self.paths, &self.account_store);
                            self.ely_password.clear();
                            self.ely_totp.clear();
                            self.auth_error = None;
                            let msg = format!("Signed in with Ely.by: {name}");
                            self.auth_status = Some(msg.clone());
                            self.show_toast(ctx, msg);
                            anim::emit_burst(ctx, anim::pointer_or_center(ctx), INFO, 10);
                            anim::emit_edge_pulse(ctx, INFO);
                        }
                        Err(e) => {
                            self.auth_error = Some(e);
                            anim::emit_edge_pulse(ctx, DANGER);
                        }
                    }
                }
                WorkerEvent::ModrinthSearchFinished {
                    generation,
                    append,
                    result,
                } => {
                    if generation == self.mr_search_gen {
                        self.mr_searching = false;
                        match result {
                            Ok(page) => {
                                self.mr_total_hits = page.total_hits;
                                self.mr_next_offset = page.next_offset;
                                self.mr_has_more = page.has_more;
                                if append {
                                    self.mr_results.extend(page.hits);
                                } else {
                                    self.mr_results = page.hits;
                                }
                                self.mr_error = None;
                                let urls: Vec<String> = self
                                    .mr_results
                                    .iter()
                                    .filter_map(|h| h.icon_url.clone())
                                    .collect();
                                for u in urls {
                                    self.request_icon(&u);
                                }
                            }
                            Err(e) => {
                                self.mr_error = Some(e);
                            }
                        }
                    }
                }
                WorkerEvent::ModrinthInstallFinished(res) => {
                    self.mr_installing = false;
                    self.mr_installing_id = None;
                    self.progress = 0.0;
                    if let Some(p) = self.profiles.get(self.selected_idx) {
                        self.mr_installed_cache = load_installed_index(&self.paths, p);
                        self.fs_cache.invalidate();
                        self.mr_installed_profile_id = p.id.clone();
                    }
                    match res {
                        Ok(ModrinthInstallOutcome::FileInstalled {
                            title,
                            filename,
                            dest_folder,
                            dependencies_installed,
                        }) => {
                            let lang = self.settings.language;
                            let inst_word = tr(lang, "toast.installed");
                            let msg = if dependencies_installed.is_empty() {
                                format!("{inst_word}: '{title}' ({filename}) -> {dest_folder}/")
                            } else {
                                format!(
                                    "{inst_word}: '{title}' (+ {})",
                                    dependencies_installed.join(", ")
                                )
                            };
                            self.mr_status = Some(msg.clone());
                            self.status_text = msg.clone();
                            self.show_toast(ctx, msg);
                            anim::emit_burst(ctx, anim::pointer_or_center(ctx), accent(), 10);
                            anim::emit_edge_pulse(ctx, accent());
                        }
                        Ok(ModrinthInstallOutcome::ModpackProfileCreated {
                            profile,
                            files_count,
                        }) => {
                            let lang = self.settings.language;
                            let name = profile.name.clone();
                            self.profiles.push(profile);
                            let new_idx = self.profiles.len() - 1;
                            let _ = save_profiles(&self.paths, &self.profiles);
                            self.select_profile(new_idx);
                            let msg = format!(
                                "{}: '{name}' ({files_count})",
                                tr(lang, "toast.installed")
                            );
                            self.mr_status = Some(msg.clone());
                            self.status_text = msg.clone();
                            self.show_toast(ctx, msg);
                            anim::emit_burst(ctx, anim::pointer_or_center(ctx), accent(), 12);
                            anim::emit_edge_pulse(ctx, accent());
                        }
                        Err(e) if e == CANCELLED_ERR_MSG => {
                            let msg = tr(self.settings.language, "status.cancelled").to_string();
                            self.mr_error = None;
                            self.status_text = msg.clone();
                            self.show_toast(ctx, msg);
                        }
                        Err(e) => {
                            let lang = self.settings.language;
                            self.mr_error = Some(e.clone());
                            self.status_text = format!("{}: {e}", tr(lang, "status.error"));
                            self.show_toast(ctx, format!("{}: {e}", tr(lang, "status.error")));
                            anim::emit_edge_pulse(ctx, DANGER);
                        }
                    }
                }
                WorkerEvent::ModUpdatesFinished(res) => {
                    self.mr_installing = false;
                    self.progress = 0.0;
                    if let Some(p) = self.profiles.get(self.selected_idx) {
                        self.mr_installed_cache = load_installed_index(&self.paths, p);
                        self.fs_cache.invalidate();
                        self.mr_installed_profile_id = p.id.clone();
                    }
                    let lang = self.settings.language;
                    match res {
                        Ok(updated) if updated.is_empty() => {
                            let msg = tr(lang, "toast.mods_up_to_date").to_string();
                            self.status_text = msg.clone();
                            self.show_toast(ctx, msg);
                            anim::emit_ring(ctx, anim::pointer_or_center(ctx), accent(), 34.0);
                        }
                        Ok(updated) => {
                            let msg = format!(
                                "{}: {}",
                                tr(lang, "toast.installed"),
                                updated.join(", ")
                            );
                            self.status_text = msg.clone();
                            self.show_toast(ctx, msg);
                            anim::emit_burst(ctx, anim::pointer_or_center(ctx), accent(), 10);
                            anim::emit_edge_pulse(ctx, accent());
                        }
                        Err(e) => {
                            self.status_text = format!("{}: {e}", tr(lang, "status.update_failed"));
                            self.show_toast(ctx, format!("{}: {e}", tr(lang, "status.error")));
                            anim::emit_edge_pulse(ctx, DANGER);
                        }
                    }
                }
                WorkerEvent::ProjectVersionsLoaded { project_id, result } => {
                    if self
                        .mr_detail_hit
                        .as_ref()
                        .is_some_and(|h| h.project_id == project_id)
                    {
                        self.mr_detail_loading = false;
                        self.mr_detail_versions = Some(result);
                    }
                }
                WorkerEvent::IconLoaded { url, image } => {
                    self.mr_icons_pending.remove(&url);
                    let tex = image.ok().map(|img| {
                        let color_img = egui::ColorImage::from_rgba_unmultiplied(
                            [img.width, img.height],
                            &img.rgba,
                        );
                        ctx.load_texture(
                            format!("mr_icon_{url}"),
                            color_img,
                            egui::TextureOptions::LINEAR,
                        )
                    });
                    self.mr_icons.insert(url, tex);
                }
                WorkerEvent::SkinLoaded { key, head, body } => {
                    self.skin_pending.remove(&key);
                    let head_tex = head.ok().map(|img| {
                        ctx.load_texture(
                            format!("skin_head_{key}"),
                            egui::ColorImage::from_rgba_unmultiplied(
                                [img.width, img.height],
                                &img.rgba,
                            ),
                            egui::TextureOptions::NEAREST,
                        )
                    });
                    let body_tex = body.ok().map(|img| {
                        ctx.load_texture(
                            format!("skin_body_{key}"),
                            egui::ColorImage::from_rgba_unmultiplied(
                                [img.width, img.height],
                                &img.rgba,
                            ),
                            egui::TextureOptions::NEAREST,
                        )
                    });
                    self.skin_heads.insert(key.clone(), head_tex);
                    self.skin_bodies.insert(key, body_tex);
                }
                WorkerEvent::FilePicked { purpose, path } => {
                    self.file_picker_open = false;
                    if let Some(chosen) = path {
                        match purpose {
                            FilePickPurpose::ModpackImport => {
                                self.show_new_modal = false;
                                self.import_mrpack_from_path(chosen);
                            }
                            FilePickPurpose::InstanceContentImport => {
                                if let Some(prof) = self.profiles.get(self.selected_idx).cloned() {
                                    reset_download_cancel();
                                    let active_folder = self.content_folder.dir_name().to_string();
                                    let paths = self.paths.clone();
                                    let cf_key = self.settings.curseforge_api_key.clone();
                                    let tx = self.tx.clone();
                                    self.mr_installing = true;
                                    self.progress = 0.08;
                                    thread::spawn(move || {
                                        let res = import_local_file(
                                            &paths,
                                            &prof,
                                            &chosen,
                                            &active_folder,
                                            &cf_key,
                                            &tx,
                                        );
                                        let _ = tx.send(WorkerEvent::ModrinthInstallFinished(res));
                                    });
                                }
                            }
                            FilePickPurpose::WorldDatapackImport { world_folder } => {
                                if let Some(prof) = self.profiles.get(self.selected_idx) {
                                    let inst = instance_dir(&self.paths, prof);
                                    match import_world_datapack(&inst, &world_folder, &chosen) {
                                        Ok(fname) => {
                                            self.fs_cache.invalidate();
                                            self.show_toast(
                                                ctx,
                                                format!("Datapack imported: {fname}"),
                                            );
                                        }
                                        Err(e) => self.show_toast(ctx, e),
                                    }
                                }
                            }
                            FilePickPurpose::InstanceIcon => {
                                if let Some(target) = self.profiles.get_mut(self.selected_idx) {
                                    match apply_profile_icon_from_file(
                                        ctx,
                                        &self.paths,
                                        &mut self.profile_icons,
                                        target,
                                        &chosen,
                                    ) {
                                        Ok(()) => {
                                            let _ = save_profiles(&self.paths, &self.profiles);
                                            self.show_toast(ctx, "Instance icon updated");
                                        }
                                        Err(e) => self.show_toast(ctx, e),
                                    }
                                }
                            }
                            FilePickPurpose::ServerIcon => {
                                if let Some(srv) =
                                    self.servers.get(self.selected_server_idx).cloned()
                                {
                                    let dir = server_dir(&self.paths, &srv);
                                    match save_server_icon_png(&dir, &chosen) {
                                        Ok(_) => {
                                            self.server_icons.remove(&srv.id);
                                            self.show_toast(ctx, "server-icon.png (64×64)");
                                        }
                                        Err(e) => self.show_toast(ctx, e),
                                    }
                                }
                            }
                            FilePickPurpose::JavaBinary => {
                                if let Some(prof) = self.profiles.get_mut(self.selected_idx) {
                                    prof.java_path = chosen.to_string_lossy().to_string();
                                    let _ = save_profiles(&self.paths, &self.profiles);
                                }
                            }
                            FilePickPurpose::ServerJavaBinary => {
                                if let Some(srv) = self.servers.get_mut(self.selected_server_idx) {
                                    srv.java_path = chosen.to_string_lossy().to_string();
                                    let _ = save_servers(&self.paths, &self.servers);
                                }
                            }
                            FilePickPurpose::CustomUiFont => {
                                self.settings.ui_font_path = chosen.to_string_lossy().to_string();
                                apply_ui_font(ctx, &self.settings.ui_font_path);
                                let _ = save_launcher_settings(&self.paths, &self.settings);
                            }
                        }
                    }
                }
                WorkerEvent::WorldBackupFinished(res) => {
                    self.world_backup_busy = false;
                    let lang = self.settings.language;
                    match res {
                        Ok(zip_path) => {
                            let fname = zip_path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "backup.zip".to_string());
                            let msg = format!("{}: {fname}", tr(lang, "world.backed_up"));
                            self.status_text = msg.clone();
                            self.show_toast(ctx, msg);
                            anim::emit_burst(ctx, anim::pointer_or_center(ctx), accent(), 10);
                            anim::emit_edge_pulse(ctx, accent());
                        }
                        Err(err) => {
                            self.status_text = err.clone();
                            self.show_toast(ctx, format!("{}: {err}", tr(lang, "status.error")));
                            anim::emit_edge_pulse(ctx, DANGER);
                        }
                    }
                }
                WorkerEvent::WorldRestoreFinished(res) => {
                    self.world_backup_busy = false;
                    let lang = self.settings.language;
                    match res {
                        Ok(world_name) => {
                            let msg = format!("{}: {world_name}", tr(lang, "world.restored"));
                            self.status_text = msg.clone();
                            self.show_toast(ctx, msg);
                            anim::emit_burst(ctx, anim::pointer_or_center(ctx), accent(), 10);
                            anim::emit_edge_pulse(ctx, accent());
                        }
                        Err(err) => {
                            self.status_text = err.clone();
                            self.show_toast(ctx, format!("{}: {err}", tr(lang, "status.error")));
                            anim::emit_edge_pulse(ctx, DANGER);
                        }
                    }
                }
                WorkerEvent::LauncherUpdateChecked(res) => {
                    self.update_checking = false;
                    match res {
                        Ok(Some(info)) => {
                            let can_auto = self.settings.auto_update
                                && info.download_url.is_some()
                                && !self.is_busy
                                && self.game_handle.is_none()
                                && !self.update_installing;
                            self.update_available = Some(info);
                            self.update_checked_up_to_date = false;
                            self.update_error = None;
                            if can_auto {
                                self.start_launcher_update();
                            }
                        }
                        Ok(None) => {
                            self.update_available = None;
                            self.update_checked_up_to_date = true;
                            self.update_error = None;
                        }
                        Err(e) => {
                            self.update_checked_up_to_date = false;
                            self.update_error = Some(e);
                        }
                    }
                }
                WorkerEvent::LauncherUpdateInstalled(res) => {
                    self.update_installing = false;
                    self.is_busy = false;
                    self.progress = 0.0;
                    match res {
                        Ok(exe_path) => {
                            let mut cmd = std::process::Command::new(&exe_path);
                            cmd.arg("--relaunch-wait");
                            #[cfg(windows)]
                            {
                                use std::os::windows::process::CommandExt;
                                // Detach console inheritance so the updated instance relaunches cleanly on exit.
                                const DETACHED_PROCESS: u32 = 0x0000_0008;
                                cmd.creation_flags(DETACHED_PROCESS);
                            }
                            match cmd.spawn() {
                                Ok(_) => {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                                Err(e) => {
                                    let msg = format!("Update installed, restart failed: {e}");
                                    self.update_error = Some(msg.clone());
                                    self.status_text = msg.clone();
                                    self.show_toast(ctx, msg);
                                }
                            }
                        }
                        Err(e) => {
                            let lang = self.settings.language;
                            self.update_error = Some(e.clone());
                            self.status_text = e.clone();
                            self.show_toast(
                                ctx,
                                format!("{}: {e}", tr(lang, "status.update_failed")),
                            );
                        }
                    }
                }
                WorkerEvent::ServerProvisioned(res) => {
                    self.server_busy = false;
                    self.is_busy = false;
                    self.progress = 0.0;
                    match res {
                        Ok(updated) => {
                            if let Some(slot) = self.servers.iter_mut().find(|s| s.id == updated.id)
                            {
                                *slot = updated.clone();
                            } else {
                                self.servers.push(updated.clone());
                                self.selected_server_idx = self.servers.len().saturating_sub(1);
                            }
                            let _ = save_servers(&self.paths, &self.servers);
                            let msg = format!("Server '{}' ready", updated.name);
                            self.status_text = msg.clone();
                            self.show_toast(ctx, msg);
                            anim::emit_burst(ctx, anim::pointer_or_center(ctx), accent(), 10);
                            anim::emit_edge_pulse(ctx, accent());
                        }
                        Err(e) => {
                            self.status_text = format!("Server setup failed: {e}");
                            self.server_logs.push_back(format!("[ERR] {e}"));
                            self.show_toast(ctx, format!("Server setup failed: {e}"));
                            anim::emit_edge_pulse(ctx, DANGER);
                        }
                    }
                }
                WorkerEvent::ServerStarted(res) => {
                    self.server_busy = false;
                    self.is_busy = false;
                    self.progress = 0.0;
                    match res {
                        Ok(handle) => {
                            self.running_server_id = Some(handle.server_id.clone());
                            if let Some(srv) =
                                self.servers.iter_mut().find(|s| s.id == handle.server_id)
                            {
                                srv.last_started = Some(chrono::Utc::now().to_rfc3339());
                            }
                            let _ = save_servers(&self.paths, &self.servers);
                            self.server_handle = Some(handle);
                            self.status_text = "Server running".to_string();
                            self.show_toast(ctx, "Dedicated server started");
                            anim::emit_edge_pulse(ctx, accent());
                        }
                        Err(e) => {
                            self.running_server_id = None;
                            self.server_handle = None;
                            self.status_text = format!("Server start failed: {e}");
                            self.server_logs.push_back(format!("[ERR] {e}"));
                            self.show_toast(ctx, format!("Server start failed: {e}"));
                            anim::emit_edge_pulse(ctx, DANGER);
                        }
                    }
                }
                WorkerEvent::ServerLog(line) => {
                    if self.server_logs.len() >= MAX_LOG_LINES {
                        self.server_logs.pop_front();
                    }
                    self.server_logs.push_back(line);
                }
                WorkerEvent::ServerTunnelAddress(addr) => {
                    self.server_tunnel_error = None;
                    self.server_tunnel_addr = Some(addr.clone());
                    self.show_toast(ctx, format!("Tunnel ready: {addr}"));
                    anim::emit_edge_pulse(ctx, accent());
                }
                WorkerEvent::ServerTunnelFailed(err) => {
                    self.server_tunnel_error = Some(err.clone());
                    self.show_toast(ctx, format!("Tunnel failed: {err}"));
                    anim::emit_edge_pulse(ctx, DANGER);
                }
                WorkerEvent::ServerProjectInstalled(res) => {
                    self.mr_installing = false;
                    self.mr_installing_id = None;
                    self.progress = 0.0;
                    match res {
                        Ok(ModrinthInstallOutcome::FileInstalled {
                            title,
                            filename,
                            dest_folder,
                            ..
                        }) => {
                            let msg = format!("Installed {title} ({filename}) -> {dest_folder}/");
                            self.mr_status = Some(msg.clone());
                            self.status_text = msg.clone();
                            self.show_toast(ctx, msg);
                            anim::emit_burst(ctx, anim::pointer_or_center(ctx), accent(), 10);
                            anim::emit_edge_pulse(ctx, accent());
                        }
                        Ok(_) => {}
                        Err(e) => {
                            self.mr_error = Some(e.clone());
                            self.status_text = e.clone();
                            self.show_toast(ctx, e);
                            anim::emit_edge_pulse(ctx, DANGER);
                        }
                    }
                }
                WorkerEvent::ServerExited(code) => {
                    self.server_busy = false;
                    self.running_server_id = None;
                    self.server_tunnel_addr = None;
                    self.server_tunnel_error = None;
                    self.server_handle = None;
                    let msg = match code {
                        Some(0) | None => "[Server] Stopped.".to_string(),
                        Some(c) => format!("[Server] Exited with code {c}."),
                    };
                    self.server_logs.push_back(msg);
                    self.status_text = "Server stopped".to_string();
                }
            }
            ctx.request_repaint();
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let k_pressed = ctx.input(|i| (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(egui::Key::K));
        if k_pressed {
            self.show_cmd_palette = !self.show_cmd_palette;
            if self.show_cmd_palette {
                self.cmd_palette_query.clear();
                self.cmd_palette_selected = 0;
                self.cmd_palette_category = PaletteCategory::All;
                self.cmd_palette_just_opened = true;
            }
            return;
        }
        if ctx.wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.show_new_modal = false;
                self.confirm_delete_idx = None;
                self.show_create_server_modal = false;
                self.confirm_delete_server_idx = None;
                self.show_cmd_palette = false;
                self.mr_detail_hit = None;
            }
            return;
        }
        let (ctrl, n_pressed, esc, num_key) = ctx.input(|i| {
            let ctrl = i.modifiers.ctrl || i.modifiers.command;
            let n = ctrl && i.key_pressed(egui::Key::N);
            let esc = i.key_pressed(egui::Key::Escape);
            let num = if ctrl {
                if i.key_pressed(egui::Key::Num1) {
                    Some(0)
                } else if i.key_pressed(egui::Key::Num2) {
                    Some(1)
                } else if i.key_pressed(egui::Key::Num3) {
                    Some(2)
                } else if i.key_pressed(egui::Key::Num4) {
                    Some(3)
                } else if i.key_pressed(egui::Key::Num5) {
                    Some(4)
                } else {
                    None
                }
            } else {
                None
            };
            (ctrl, n, esc, num)
        });
        let _ = ctrl;
        if esc {
            self.show_new_modal = false;
            self.confirm_delete_idx = None;
            self.show_create_server_modal = false;
            self.confirm_delete_server_idx = None;
            self.show_cmd_palette = false;
            self.mr_detail_hit = None;
        }
        if n_pressed {
            self.show_new_modal = true;
            anim::emit_edge_pulse(ctx, accent());
        }
        if let Some(idx) = num_key {
            if let Some(page) = NavPage::ALL.get(idx).copied() {
                self.switch_page(ctx, page);
            }
        }
    }

    fn profile_mod_counts(&mut self, profile: &Profile) -> (usize, usize) {
        let now = std::time::Instant::now();
        if let Some((total, enabled, ts)) = self.fs_cache.mod_counts.get(&profile.id) {
            if now.duration_since(*ts) < FS_CACHE_TTL {
                return (*total, *enabled);
            }
        }
        let items = list_content_items(&instance_dir(&self.paths, profile).join("mods"));
        let enabled = items.iter().filter(|m| m.enabled).count();
        let total = items.len();
        self.fs_cache
            .mod_counts
            .insert(profile.id.clone(), (total, enabled, now));
        (total, enabled)
    }

    fn server_content_counts(&mut self, srv: &DedicatedServer) -> (usize, usize) {
        let now = std::time::Instant::now();
        if let Some((plugins, mods, ts)) = self.fs_cache.server_counts.get(&srv.id) {
            if now.duration_since(*ts) < FS_CACHE_TTL {
                return (*plugins, *mods);
            }
        }
        let dir = server_dir(&self.paths, srv);
        let plugins = if srv.software.supports_plugins() {
            list_content_items(&dir.join("plugins")).len()
        } else {
            0
        };
        let mods = if srv.software.supports_mods() {
            list_content_items(&dir.join("mods")).len()
        } else {
            0
        };
        self.fs_cache
            .server_counts
            .insert(srv.id.clone(), (plugins, mods, now));
        (plugins, mods)
    }

    fn folder_content_count(&mut self, profile: &Profile, folder: &'static str) -> usize {
        let now = std::time::Instant::now();
        if let Some((cnt, ts)) = self
            .fs_cache
            .folder_counts
            .get(&(profile.id.clone(), folder))
        {
            if now.duration_since(*ts) < FS_CACHE_TTL {
                return *cnt;
            }
        }
        let cnt = list_content_items(&instance_dir(&self.paths, profile).join(folder)).len();
        self.fs_cache
            .folder_counts
            .insert((profile.id.clone(), folder), (cnt, now));
        cnt
    }

    fn cached_content_items(
        &mut self,
        profile: &Profile,
        folder: &'static str,
    ) -> Vec<InstalledContentItem> {
        let now = std::time::Instant::now();
        if let Some((pid, f, items, ts)) = &self.fs_cache.content_items {
            if pid == &profile.id && *f == folder && now.duration_since(*ts) < FS_CACHE_TTL {
                return items.clone();
            }
        }
        let items = list_content_items(&instance_dir(&self.paths, profile).join(folder));
        let total = items.len();
        self.fs_cache
            .folder_counts
            .insert((profile.id.clone(), folder), (total, now));
        if folder == "mods" {
            let enabled = items.iter().filter(|m| m.enabled).count();
            self.fs_cache
                .mod_counts
                .insert(profile.id.clone(), (total, enabled, now));
        }
        self.fs_cache.content_items = Some((profile.id.clone(), folder, items.clone(), now));
        items
    }

    fn cached_worlds(&mut self, profile: &Profile) -> Vec<WorldEntry> {
        let now = std::time::Instant::now();
        if let Some((pid, worlds, ts)) = &self.fs_cache.worlds {
            if pid == &profile.id && now.duration_since(*ts) < FS_CACHE_TTL {
                return worlds.clone();
            }
        }
        let worlds = list_worlds(&instance_dir(&self.paths, profile));
        self.fs_cache.worlds = Some((profile.id.clone(), worlds.clone(), now));
        worlds
    }

    fn cached_world_backups(&mut self, profile: &Profile) -> Vec<WorldBackupEntry> {
        let now = std::time::Instant::now();
        if let Some((pid, backups, ts)) = &self.fs_cache.world_backups {
            if pid == &profile.id && now.duration_since(*ts) < FS_CACHE_TTL {
                return backups.clone();
            }
        }
        let backups = list_world_backups(&instance_dir(&self.paths, profile));
        self.fs_cache.world_backups = Some((profile.id.clone(), backups.clone(), now));
        backups
    }

    fn cached_screenshots(&mut self, profile: &Profile) -> Vec<ScreenshotEntry> {
        let now = std::time::Instant::now();
        if let Some((pid, shots, ts)) = &self.fs_cache.screenshots {
            if pid == &profile.id && now.duration_since(*ts) < FS_CACHE_TTL {
                return shots.clone();
            }
        }
        let shots = list_screenshots(&instance_dir(&self.paths, profile));
        self.fs_cache.screenshots = Some((profile.id.clone(), shots.clone(), now));
        shots
    }

    fn render_nav_sidebar(&mut self, ui: &mut egui::Ui, compact: bool) {
        // Pinned account footer at the bottom of the sidebar
        egui::TopBottomPanel::bottom("nav_sidebar_footer")
            .resizable(false)
            .frame(egui::Frame::none().inner_margin(Margin::symmetric(2.0, 2.0)))
            .show_inside(ui, |ui| {
                ui.separator();
                ui.add_space(5.0);
                let (acc_name, acc_kind, acc_col) = self
                    .active_account()
                    .map(|a| {
                        let col = match a.kind {
                            AccountKind::Microsoft { .. } => accent(),
                            AccountKind::ElyBy { .. } => INFO,
                            AccountKind::Offline => text_secondary(),
                        };
                        (a.username.clone(), a.kind_label(), col)
                    })
                    .unwrap_or_else(|| ("None".to_string(), "—", text_muted()));
                let foot_pad_x = 6.0;
                let foot_inner_w = (ui.available_width() - foot_pad_x * 2.0 - 4.0).max(28.0);
                let foot_id = ui.make_persistent_id("nav_acc_footer_card");
                let prev_hov = ui.ctx().data(|d| d.get_temp::<bool>(foot_id)).unwrap_or(false);
                let hov_t = ui.ctx().animate_bool_with_time(foot_id, prev_hov, 0.12);
                let prev_prs = ui
                    .ctx()
                    .data(|d| d.get_temp::<bool>(foot_id.with("prs_state")))
                    .unwrap_or(false);
                let prs_t = ui
                    .ctx()
                    .animate_bool_with_time(foot_id.with("prs_anim"), prev_prs, 0.07);
                if (hov_t > 0.01 && hov_t < 0.99) || (prs_t > 0.01 && prs_t < 0.99) {
                    ui.ctx().request_repaint();
                }

                let frame_resp = card_frame(self.nav_page == NavPage::Accounts, hov_t)
                    .inner_margin(Margin::symmetric(foot_pad_x, 7.0))
                    .show(ui, |ui| {
                        ui.set_min_width(foot_inner_w);
                        ui.set_max_width(foot_inner_w);
                        let (slot_r, _) =
                            ui.allocate_exact_size(Vec2::new(foot_inner_w, 30.0), Sense::hover());
                        let av_r = Rect::from_min_size(
                            Pos2::new(slot_r.left(), slot_r.center().y - 14.0),
                            Vec2::splat(28.0),
                        );
                        draw_account_avatar(
                            ui.painter(),
                            av_r,
                            self.active_account(),
                            &acc_name,
                            acc_col,
                            &self.skin_heads,
                        );
                        let dot_c = Pos2::new(av_r.right() - 1.5, av_r.bottom() - 1.5);
                        if hov_t > 0.05 {
                            let ring_r = 3.6 + hov_t * 2.2;
                            ui.painter().circle_stroke(
                                dot_c,
                                ring_r,
                                Stroke::new(1.0_f32, with_alpha(acc_col, (hov_t * 150.0) as u8)),
                            );
                        }
                        ui.painter().circle_filled(dot_c, 4.2, bg_sidebar());
                        ui.painter().circle_filled(dot_c, 3.0, acc_col);

                        if slot_r.width() > 36.0 {
                            let clipped = ui.painter().with_clip_rect(slot_r);
                            let text_x = av_r.right() + 9.0;
                            clipped.text(
                                Pos2::new(text_x, slot_r.top() + 8.5),
                                egui::Align2::LEFT_CENTER,
                                &acc_name,
                                egui::FontId::proportional(13.0),
                                text_primary(),
                            );
                            clipped.text(
                                Pos2::new(text_x, slot_r.top() + 22.0),
                                egui::Align2::LEFT_CENTER,
                                acc_kind,
                                egui::FontId::proportional(10.5),
                                acc_col,
                            );
                        }
                    });
                let foot_rect = frame_resp.response.rect;
                let hovered_now = frame_resp.response.hovered()
                    || ui.rect_contains_pointer(foot_rect);
                ui.ctx().data_mut(|d| d.insert_temp(foot_id, hovered_now));
                let resp = frame_resp
                    .response
                    .interact(Sense::click())
                    .on_hover_text(format!("Account: {acc_name} ({acc_kind})"));
                ui.ctx()
                    .data_mut(|d| d.insert_temp(foot_id.with("prs_state"), resp.is_pointer_button_down_on()));
                if let Some((ct, origin)) = track_click_anim(
                    ui.ctx(),
                    foot_id,
                    foot_rect,
                    resp.clicked(),
                    resp.interact_pointer_pos(),
                    0.38,
                ) {
                    paint_click_ripple(
                        ui.painter(),
                        foot_rect,
                        Rounding::same(10.0),
                        origin,
                        ct,
                        acc_col,
                        true,
                    );
                }
                if prs_t > 0.02 {
                    ui.painter().rect_stroke(
                        foot_rect.shrink(prs_t * 0.8),
                        Rounding::same(10.0),
                        Stroke::new(1.2_f32, with_alpha(accent(), (prs_t * 140.0) as u8)),
                    );
                }
                if resp.hovered() || hovered_now {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    anim::emit_ring(ui.ctx(), foot_rect.center(), acc_col, 32.0);
                    self.switch_page(ui.ctx(), NavPage::Accounts);
                }
                let credit_w = (ui.available_width() - 4.0).max(20.0);
                let (credit_r, _) =
                    ui.allocate_exact_size(Vec2::new(credit_w, 16.0), Sense::hover());
                if credit_r.width() > 90.0 {
                    let clipped = ui.painter().with_clip_rect(credit_r);
                    clipped.text(
                        Pos2::new(credit_r.left() + 6.0, credit_r.center().y),
                        egui::Align2::LEFT_CENTER,
                        "by svarka (@sunset-launcher) • MIT",
                        egui::FontId::proportional(10.0),
                        text_muted(),
                    );
                }
            });

        // Brand + vertical navigation
        ui.vertical(|ui| {
            let brand_id = ui.make_persistent_id("nav_brand_lockup");
            let brand_hov_prev = ui.ctx().data_mut(|d| d.get_temp::<bool>(brand_id).unwrap_or(false));
            let brand_prs_prev = ui.ctx().data_mut(|d| d.get_temp::<bool>(brand_id.with("prs")).unwrap_or(false));
            let brand_ht = ui.ctx().animate_bool_with_time(brand_id, brand_hov_prev, 0.14);
            let brand_pt = ui.ctx().animate_bool_with_time(brand_id.with("prs_anim"), brand_prs_prev, 0.07);
            if (brand_ht > 0.01 && brand_ht < 0.99) || (brand_pt > 0.01 && brand_pt < 0.99) {
                ui.ctx().request_repaint();
            }
            let brand_click_info = track_click_anim(
                ui.ctx(),
                brand_id,
                Rect::NOTHING,
                false,
                None,
                0.42,
            );
            let brand_click_t = brand_click_info.map(|(t, _)| t);
            let brand_fill = if brand_ht > 0.01 {
                with_alpha(accent(), (brand_ht * 18.0) as u8)
            } else {
                Color32::TRANSPARENT
            };
            let brand_stroke = if brand_ht > 0.01 {
                Stroke::new(1.0_f32, with_alpha(accent(), (brand_ht * 120.0) as u8))
            } else {
                Stroke::NONE
            };
            if self.brand_logo_tex.is_none() {
                let loaded = decode_icon_bytes(include_bytes!("../../logo.png")).ok().map(|img| {
                    ui.ctx().load_texture(
                        "brand_logo_sunset",
                        egui::ColorImage::from_rgba_unmultiplied(
                            [img.width, img.height],
                            &img.rgba,
                        ),
                        egui::TextureOptions::LINEAR,
                    )
                });
                self.brand_logo_tex = Some(loaded);
            }
            let brand_resp = egui::Frame::none()
                .fill(brand_fill)
                .stroke(brand_stroke)
                .rounding(Rounding::same(10.0))
                .inner_margin(Margin::symmetric(4.0, 5.0))
                .show(ui, |ui| {
                    let row_w = ui.available_width().max(36.0);
                    let (row_rect, _) =
                        ui.allocate_exact_size(Vec2::new(row_w, 36.0), Sense::hover());
                    let pop = brand_click_t.map_or(0.0, |t| (t * std::f32::consts::PI).sin() * 0.08);
                    let scale = (1.0 + brand_ht * 0.04 - brand_pt * 0.06 + pop).max(0.85);
                    let base_icon_rect = Rect::from_min_size(row_rect.min, Vec2::splat(36.0));
                    let icon_rect = Rect::from_center_size(base_icon_rect.center(), base_icon_rect.size() * scale);
                    if let Some(Some(tex)) = &self.brand_logo_tex {
                        if brand_ht > 0.02 {
                            ui.painter().rect_filled(
                                icon_rect.expand(2.0 + brand_ht * 2.0),
                                Rounding::same(10.0),
                                with_alpha(accent(), (brand_ht * 42.0) as u8),
                            );
                        }
                        ui.painter().image(
                            tex.id(),
                            icon_rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        if let Some(t) = brand_click_t {
                            let inv = 1.0 - t;
                            let expand = self::anim::ease_out_quint(t) * 6.0;
                            ui.painter().rect_stroke(
                                icon_rect.expand(expand),
                                Rounding::same(9.0 + expand * 0.4),
                                Stroke::new(1.5_f32, with_alpha(accent(), (inv * inv * 180.0) as u8)),
                            );
                        }
                    } else {
                        draw_brand_logo_animated(ui.painter(), base_icon_rect, brand_ht, brand_click_t);
                    }
                    if row_rect.width() > 42.0 {
                        let clipped = ui.painter().with_clip_rect(row_rect);
                        let text_x = row_rect.left() + 45.0;
                        clipped.text(
                            Pos2::new(text_x, row_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            "Sunset Launcher",
                            egui::FontId::proportional(15.0),
                            lerp_color(text_primary(), accent_text(), brand_ht * 0.35),
                        );
                    }
                });
            let brand_rect = brand_resp.response.rect;
            let brand_hov_now = brand_resp.response.hovered()
                || ui.rect_contains_pointer(brand_rect);
            ui.ctx().data_mut(|d| d.insert_temp(brand_id, brand_hov_now));
            let brand_click = brand_resp
                .response
                .interact(Sense::click())
                .on_hover_text(concat!(
                    "Sunset Launcher v",
                    env!("CARGO_PKG_VERSION"),
                    " • by svarka (@sunset-launcher)"
                ));
            ui.ctx().data_mut(|d| d.insert_temp(brand_id.with("prs"), brand_click.is_pointer_button_down_on()));
            if let Some((ct, origin)) = track_click_anim(
                ui.ctx(),
                brand_id,
                brand_rect,
                brand_click.clicked(),
                brand_click.interact_pointer_pos(),
                0.42,
            ) {
                paint_click_ripple(
                    ui.painter(),
                    brand_rect,
                    Rounding::same(10.0),
                    origin,
                    ct,
                    accent(),
                    true,
                );
            }
            if brand_click.hovered() || brand_hov_now {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if brand_click.clicked() {
                anim::emit_ring(ui.ctx(), brand_rect.center(), accent(), 34.0);
                self.switch_page(ui.ctx(), NavPage::Library);
            }

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            let is_game_running = self.game_handle.is_some();
            let is_server_running = self.running_server_id.is_some();

            bounded_scroll(ui, "nav_sidebar_scroll", |ui| {
                egui::Frame::none()
                    .inner_margin(Margin::symmetric(2.0, 1.0))
                    .show(ui, |ui| {
                        let nav_w = (ui.available_width() - 4.0).max(24.0);
                        let mut active_nav_rect = Rect::NOTHING;
                        for (page_idx, page) in NavPage::ALL.iter().copied().enumerate() {
                            let is_active = self.nav_page == page;
                            let nav_id = ui.make_persistent_id(("nav_btn_base", page.label()));
                            let active_t = ui.ctx().animate_bool_with_time(
                                ui.make_persistent_id(("nav_active", page.label())),
                                is_active,
                                0.16,
                            );
                            if active_t > 0.01 && active_t < 0.99 {
                                ui.ctx().request_repaint();
                            }

                            let desired = Vec2::new(nav_w, 35.0);
                            let (rect, mut resp) = ui.allocate_exact_size(desired, Sense::click());
                            if compact {
                                resp = resp.on_hover_text(format!(
                                    "{} (Ctrl+{})",
                                    page.label(),
                                    page_idx + 1
                                ));
                            }
                            if is_active {
                                active_nav_rect = rect;
                            }
                            let hover_t = ui.ctx().animate_bool_with_time(
                                ui.make_persistent_id(("nav_hover", page.label())),
                                resp.hovered(),
                                0.12,
                            );
                            let press_t = ui.ctx().animate_bool_with_time(
                                ui.make_persistent_id(("nav_press", page.label())),
                                resp.is_pointer_button_down_on(),
                                0.07,
                            );
                            if (hover_t > 0.01 && hover_t < 0.99)
                                || (press_t > 0.01 && press_t < 0.99)
                            {
                                ui.ctx().request_repaint();
                            }
                            let click_info = track_click_anim(
                                ui.ctx(),
                                nav_id,
                                rect,
                                resp.clicked(),
                                resp.interact_pointer_pos(),
                                0.36,
                            );
                            let click_t = click_info.map(|(t, _)| t);

                            let draw_rect = rect.shrink(press_t * 0.8);
                            let bg = lerp_color(
                                lerp_color(Color32::TRANSPARENT, bg_card(), hover_t),
                                bg_card_active(),
                                active_t,
                            );
                            let painter = ui.painter();
                            painter.rect_filled(draw_rect, Rounding::same(8.0), bg);
                            if active_t > 0.05 {
                                painter.rect_stroke(
                                    draw_rect,
                                    Rounding::same(8.0),
                                    Stroke::new(1.0_f32, with_alpha(accent(), (active_t * 95.0) as u8)),
                                );
                            }
                            if let Some((ct, origin)) = click_info {
                                paint_click_ripple(
                                    painter,
                                    draw_rect,
                                    Rounding::same(8.0),
                                    origin,
                                    ct,
                                    accent(),
                                    true,
                                );
                            }

                            let fg =
                                lerp_color(text_secondary(), text_primary(), active_t.max(hover_t * 0.7));
                            let icon_col = lerp_color(
                                lerp_color(text_muted(), text_secondary(), hover_t),
                                accent(),
                                active_t,
                            );
                            let slide_x = if compact || rect.width() < 64.0 {
                                0.0
                            } else {
                                hover_t * 2.2
                            };
                            let ic_rect = Rect::from_center_size(
                                Pos2::new(draw_rect.left() + 20.0 + slide_x * 0.5, draw_rect.center().y),
                                Vec2::splat(16.0),
                            );
                            let ic_anim = compute_icon_anim(
                                page.icon(),
                                hover_t.max(active_t * 0.25),
                                press_t,
                                click_t,
                            );
                            draw_icon_animated(painter, ic_rect, page.icon(), icon_col, ic_anim);

                            if compact && rect.width() < 64.0 {
                                if page == NavPage::Console
                                    && (self.log_error_count > 0 || is_game_running)
                                {
                                    let dot_col = if self.log_error_count > 0 {
                                        DANGER
                                    } else {
                                        accent()
                                    };
                                    draw_running_pulse_dot(
                                        ui,
                                        Pos2::new(rect.right() - 7.0, rect.top() + 7.0),
                                        3.2,
                                        dot_col,
                                        bg_sidebar(),
                                    );
                                } else if page == NavPage::Servers && is_server_running {
                                    draw_running_pulse_dot(
                                        ui,
                                        Pos2::new(rect.right() - 7.0, rect.top() + 7.0),
                                        3.2,
                                        accent(),
                                        bg_sidebar(),
                                    );
                                }
                            } else if rect.width() > 48.0 {
                                let clipped = painter.with_clip_rect(rect.shrink2(Vec2::new(6.0, 0.0)));
                                clipped.text(
                                    Pos2::new(rect.left() + 38.0 + slide_x, rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    page.translated_label(self.settings.language),
                                    egui::FontId::proportional(13.5),
                                    fg,
                                );

                                // Live running / error indicators only
                                if rect.width() >= 140.0 {
                                    match page {
                                        NavPage::Servers if is_server_running => {
                                            draw_running_pulse_dot(
                                                ui,
                                                Pos2::new(rect.right() - 13.0, rect.center().y),
                                                3.0,
                                                accent(),
                                                bg_sidebar(),
                                            );
                                        }
                                        NavPage::Console => {
                                            if self.log_error_count > 0 {
                                                let pulse = pulse_01(ui.input(|i| i.time), 4.0);
                                                let pill = Rect::from_center_size(
                                                    Pos2::new(rect.right() - 16.0, rect.center().y),
                                                    Vec2::new(20.0, 16.0),
                                                );
                                                painter.rect_filled(
                                                    pill,
                                                    Rounding::same(8.0),
                                                    with_alpha(DANGER, (185.0 + pulse * 65.0) as u8),
                                                );
                                                painter.rect_stroke(
                                                    pill.expand(pulse * 1.5),
                                                    Rounding::same(9.0),
                                                    Stroke::new(1.0_f32, with_alpha(DANGER, ((1.0 - pulse) * 140.0) as u8)),
                                                );
                                                painter.text(
                                                    pill.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    format!("{}", self.log_error_count.min(99)),
                                                    egui::FontId::proportional(10.0),
                                                    text_primary(),
                                                );
                                                ui.ctx().request_repaint();
                                            } else if is_game_running {
                                                draw_running_pulse_dot(
                                                    ui,
                                                    Pos2::new(rect.right() - 13.0, rect.center().y),
                                                    3.2,
                                                    accent(),
                                                    bg_sidebar(),
                                                );
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }

                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if resp.clicked() {
                                self.switch_page(ui.ctx(), page);
                            }
                            ui.add_space(3.0);
                        }

                        // Gliding accent indicator along the left rail
                        if active_nav_rect != Rect::NOTHING {
                            let target_cy = active_nav_rect.center().y;
                            let rail_y = ui.ctx().animate_value_with_time(
                                ui.make_persistent_id("nav_rail_cy"),
                                target_cy,
                                0.16,
                            );
                            let delta = (rail_y - target_cy).abs();
                            if delta > 0.4 {
                                ui.ctx().request_repaint();
                            }
                            let bar_h = 18.0 + (delta * 0.45).min(12.0);
                            let bar = Rect::from_center_size(
                                Pos2::new(active_nav_rect.left() + 2.5, rail_y),
                                Vec2::new(3.2, bar_h),
                            );
                            ui.painter().rect_filled(bar, Rounding::same(1.6), accent());
                        }
                    });
            });
        });
    }

    fn render_top_bar(&mut self, ui: &mut egui::Ui) {
        let avail_w = ui.available_width();
        let btn_w = if avail_w < 480.0 { 112.0 } else { 142.0 };
        let left_w = (avail_w - btn_w - 20.0).max(80.0);

        ui_toolbar_row(ui, 34.0, |ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(left_w, 34.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_height(34.0);
                    ui.set_max_height(34.0);
                    ui.spacing_mut().interact_size.y = 34.0;
                    ui.spacing_mut().button_padding = Vec2::new(11.0, 9.5);
                    ui.spacing_mut().item_spacing.x = 6.0;

                    if let Some(cur) = self.profiles.get(self.selected_idx).cloned() {
                        let show_badge = left_w >= 300.0;
                        let reserve = (if show_badge { 88.0 } else { 0.0 }) + 12.0;
                        let combo_w = (left_w - reserve).clamp(80.0, 260.0);

                        let combo_resp = egui::ComboBox::from_id_salt("topbar_instance_combo")
                            .selected_text(format!("{} ({})", cur.name, cur.version_id))
                            .width(combo_w)
                            .truncate()
                            .show_ui(ui, |ui| {
                                let mut switched = false;
                                for (idx, p) in self.profiles.iter().enumerate() {
                                    if ui
                                        .selectable_value(
                                            &mut self.selected_idx,
                                            idx,
                                            format!(
                                                "{} ({} • {})",
                                                p.name,
                                                p.version_id,
                                                p.loader.label()
                                            ),
                                        )
                                        .changed()
                                    {
                                        switched = true;
                                    }
                                }
                                switched
                            });
                        if combo_resp.inner == Some(true) {
                            self.persist_selected_profile();
                            self.sync_installed_index();
                            self.sync_discover_for_profile();
                            self.mark_subtab_switch(ui.ctx());
                            anim::emit_ring(
                                ui.ctx(),
                                combo_resp.response.rect.center(),
                                accent(),
                                32.0,
                            );
                        }

                        if show_badge {
                            loader_badge_sized(
                                ui,
                                cur.loader.kind(),
                                cur.loader.short_name(),
                                34.0,
                            );
                        }
                    }
                },
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let is_running = self.game_handle.is_some();
                let (btn_text, icon, enabled, stop_style, pulsing) = if self.is_busy {
                    (
                        tr(self.settings.language, "top.loading"),
                        None,
                        false,
                        false,
                        true,
                    )
                } else if is_running {
                    (tr(self.settings.language, "top.stop"), Some(Icon::Stop), true, true, true)
                } else {
                    (tr(self.settings.language, "top.play"), Some(Icon::Play), true, false, false)
                };

                let play_btn_resp = primary_action_button(
                    ui,
                    "main_play_btn",
                    btn_text,
                    icon,
                    enabled,
                    stop_style,
                    pulsing,
                    Vec2::new(btn_w, 34.0),
                );
                if play_btn_resp.clicked() {
                    if is_running {
                        anim::emit_ring(ui.ctx(), play_btn_resp.rect.center(), DANGER, 38.0);
                        anim::emit_edge_pulse(ui.ctx(), DANGER);
                        self.stop_game();
                    } else {
                        anim::emit_burst(ui.ctx(), play_btn_resp.rect.center(), accent(), 10);
                        anim::emit_edge_pulse(ui.ctx(), accent());
                        self.start_launch();
                    }
                }

                if let Some(upd) = self.update_available.clone() {
                    if ui.available_width() > 96.0 {
                        let ver = upd.version.trim_start_matches(['v', 'V']);
                        let resp = ui_button_icon(
                            ui,
                            "top_update_badge",
                            Some(Icon::Download),
                            &format!("v{ver}"),
                            ButtonVariant::Secondary,
                            !self.update_installing,
                            30.0,
                        )
                        .on_hover_text(format!(
                            "{}: v{ver}",
                            tr(self.settings.language, "settings.updates.available")
                        ));
                        let pulse = pulse_01(ui.input(|i| i.time), 3.2);
                        ui.painter().rect_stroke(
                            resp.rect.expand(pulse * 1.8),
                            Rounding::same(8.5),
                            Stroke::new(1.1_f32, with_alpha(accent(), (90.0 + (1.0 - pulse) * 110.0) as u8)),
                        );
                        paint_shimmer_sweep(
                            ui.painter(),
                            resp.rect,
                            Rounding::same(7.5),
                            (ui.input(|i| i.time) as f32 * 0.75).fract(),
                            accent(),
                            30,
                        );
                        ui.ctx().request_repaint();
                        if resp.clicked() {
                            anim::emit_ring(ui.ctx(), resp.rect.center(), accent(), 28.0);
                            self.switch_page(ui.ctx(), NavPage::Settings);
                        }
                    }
                }
            });
        });
    }

    fn render_bottom_bar(&mut self, ui: &mut egui::Ui) {
        let can_cancel = (self.is_busy && self.game_handle.is_none() && !self.update_installing)
            || self.mr_installing
            || self.server_busy
            || self.java_downloading_major.is_some();
        if can_cancel {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let bar_w = (ui.available_width() - 96.0).max(80.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(bar_w, 26.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        animated_progress_bar(
                            ui,
                            "main_progress",
                            self.progress,
                            &self.status_text,
                            true,
                        );
                    },
                );
                if ui_button_icon(
                    ui,
                    "cancel_active_dl_btn",
                    Some(Icon::Close),
                    tr(self.settings.language, "btn.cancel"),
                    ButtonVariant::Danger,
                    true,
                    24.0,
                )
                .clicked()
                {
                    request_download_cancel();
                    anim::emit_ring(ui.ctx(), anim::pointer_or_center(ui.ctx()), DANGER, 26.0);
                }
            });
        } else {
            animated_progress_bar(
                ui,
                "main_progress",
                self.progress,
                &self.status_text,
                self.is_busy || self.mr_installing,
            );
        }
    }
}

impl McLauncherApp {
    fn ensure_profile_icons_loaded(&mut self, ctx: &egui::Context) {
        for profile in &self.profiles {
            let path = profile.icon_path.trim();
            if path.is_empty() || self.profile_icons.contains_key(path) {
                continue;
            }
            let loaded = std::fs::read(path)
                .ok()
                .and_then(|bytes| decode_icon_bytes(&bytes).ok())
                .map(|img| {
                    ctx.load_texture(
                        format!("profile_icon:{path}"),
                        egui::ColorImage::from_rgba_unmultiplied([img.width, img.height], &img.rgba),
                        egui::TextureOptions::LINEAR,
                    )
                });
            self.profile_icons.insert(path.to_string(), loaded);
        }
    }

    fn ensure_server_icon_loaded(
        &mut self,
        ctx: &egui::Context,
        srv_id: &str,
        dir: &std::path::Path,
    ) -> Option<egui::TextureHandle> {
        if let Some(cached) = self.server_icons.get(srv_id) {
            return cached.clone();
        }
        let icon_path = dir.join("server-icon.png");
        let loaded = std::fs::read(&icon_path)
            .ok()
            .and_then(|bytes| decode_icon_bytes(&bytes).ok())
            .map(|img| {
                ctx.load_texture(
                    format!("server_icon:{srv_id}"),
                    egui::ColorImage::from_rgba_unmultiplied([img.width, img.height], &img.rgba),
                    egui::TextureOptions::LINEAR,
                )
            });
        self.server_icons.insert(srv_id.to_string(), loaded.clone());
        loaded
    }
}

impl Drop for McLauncherApp {
    fn drop(&mut self) {
        if let Some(handle) = self.server_handle.take() {
            kill_dedicated_server(&handle);
        }
    }
}

impl eframe::App for McLauncherApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(handle) = self.server_handle.take() {
            kill_dedicated_server(&handle);
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events(ctx);
        self.handle_shortcuts(ctx);
        tick_theme_transition(ctx);
        self.ensure_profile_icons_loaded(ctx);
        self.ensure_account_skins_requested();

        let screen_w = ctx.screen_rect().width();
        let compact_sidebar = screen_w < 680.0;
        let target_sidebar_w = if compact_sidebar { 60.0 } else { 208.0 };
        let sidebar_w = ctx.animate_value_with_time(
            egui::Id::new("sidebar_width_anim"),
            target_sidebar_w,
            0.16,
        );
        if (sidebar_w - target_sidebar_w).abs() > 0.3 {
            ctx.request_repaint();
        }
        let compact_pad = screen_w < 600.0;

        // 1. Top Play Bar spanning full window width
        egui::TopBottomPanel::top("top_play_bar")
            .exact_height(54.0)
            .frame(
                egui::Frame::none()
                    .fill(bg_sidebar())
                    .stroke(Stroke::new(1.0_f32, border_subtle()))
                    .inner_margin(Margin::symmetric(if compact_pad { 10.0 } else { 14.0 }, 9.0)),
            )
            .show(ctx, |ui| {
                self.render_top_bar(ui);
            });

        // 2. Bottom Progress Bar (smooth animated height slide-in/out)
        let show_bottom_bar = self.is_busy || self.mr_installing || self.progress > 0.0;
        let bar_t = ctx.animate_bool_with_time(
            egui::Id::new("bottom_bar_open_anim"),
            show_bottom_bar,
            0.20,
        );
        if bar_t > 0.01 && bar_t < 0.99 {
            ctx.request_repaint();
        }
        let bar_h = (34.0 * ease_out_cubic(bar_t)).round();
        if bar_t > 0.02 && bar_h >= 6.0 {
            let v_pad = ((bar_h - 22.0) * 0.5).clamp(0.0, 6.0);
            egui::TopBottomPanel::bottom("bottom_status_bar")
                .exact_height(bar_h)
                .frame(
                    egui::Frame::none()
                        .fill(bg_sidebar())
                        .stroke(Stroke::new(1.0_f32, border_subtle()))
                        .inner_margin(Margin::symmetric(
                            if compact_pad { 10.0 } else { 14.0 },
                            v_pad,
                        )),
                )
                .show(ctx, |ui| {
                    self.render_bottom_bar(ui);
                });
        }

        // 3. Left navigation rail
        egui::SidePanel::left("nav_sidebar")
            .frame(
                egui::Frame::none()
                    .fill(bg_sidebar())
                    .stroke(Stroke::new(1.0_f32, border_subtle()))
                    .inner_margin(Margin::symmetric(8.0, 9.0)),
            )
            .resizable(false)
            .exact_width(sidebar_w)
            .show(ctx, |ui| {
                self.render_nav_sidebar(ui, compact_sidebar);
            });

        // 4. Main scrollable page area
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(theme::bg_void())
                    .inner_margin(Margin::same(if compact_pad { 10.0 } else { 16.0 })),
            )
            .show(ctx, |ui| {
                let now = ctx.input(|i| i.time);
                let raw_t = ((now - self.page_switch_time) / 0.18).clamp(0.0, 1.0) as f32;
                let eased = ease_out_cubic(raw_t);
                if raw_t < 1.0 {
                    ctx.request_repaint();
                }
                let panel_rect = ui.max_rect();
                ui.add_space((1.0 - eased) * 10.0);

                match self.nav_page {
                    NavPage::Library => self.render_library_page(ui),
                    NavPage::Discover => self.render_discover_page(ui),
                    NavPage::Instance => self.render_instance_page(ui),
                    NavPage::Servers => self.render_servers_page(ui, ctx),
                    NavPage::Console => self.render_console_page(ui),
                    NavPage::Accounts => self.render_accounts_page(ui),
                    NavPage::Settings => self.render_settings_page(ui, ctx),
                }

                if raw_t < 0.99 {
                    paint_fade_veil(ui.painter(), panel_rect, bg_void(), raw_t, 135.0);
                }
            });

        self.render_new_profile_modal(ctx);
        self.render_delete_confirm_modal(ctx);
        self.render_create_server_modal(ctx);
        self.render_delete_server_modal(ctx);
        self.render_project_details_modal(ctx);
        self.render_command_palette_modal(ctx);
        self.handle_file_drop(ctx);
        self.render_toast(ctx);
        anim::render_action_fx(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_playtime() {
        assert_eq!(format_playtime(0, Language::En), "0m");
        assert_eq!(format_playtime(45, Language::En), "<1m");
        assert_eq!(format_playtime(125, Language::En), "2m");
        assert_eq!(format_playtime(3600, Language::En), "1h");
        assert_eq!(format_playtime(3600 + 27 * 60, Language::En), "1h 27m");
        assert_eq!(format_playtime(3600 + 27 * 60, Language::Ru), "1ч 27м");
    }

    #[test]
    fn test_diagnose_console_logs_patterns() {
        let mut logs = VecDeque::new();
        logs.push_back(LogEntry {
            timestamp: "12:00:00".to_string(),
            level: LogLevel::Info,
            text: "Starting game...".to_string(),
        });
        assert!(diagnose_console_logs(&logs).is_none());

        logs.push_back(LogEntry {
            timestamp: "12:00:05".to_string(),
            level: LogLevel::Error,
            text: "java.lang.OutOfMemoryError: Java heap space".to_string(),
        });
        assert!(diagnose_console_logs(&logs)
            .unwrap()
            .contains("Out of Memory"));

        let mut logs_java = VecDeque::new();
        logs_java.push_back(LogEntry {
            timestamp: "12:01:00".to_string(),
            level: LogLevel::Error,
            text: "java.lang.UnsupportedClassVersionError: class file version 65.0".to_string(),
        });
        assert!(diagnose_console_logs(&logs_java)
            .unwrap()
            .contains("Java Version Mismatch"));

        let mut logs_dep = VecDeque::new();
        logs_dep.push_back(LogEntry {
            timestamp: "12:02:00".to_string(),
            level: LogLevel::Error,
            text: "ModResolutionException: Could not find required mod: fabric-api".to_string(),
        });
        assert!(diagnose_console_logs(&logs_dep)
            .unwrap()
            .contains("Missing Mod Dependency"));
    }

    #[test]
    fn test_score_palette_match_ranking_and_bilingual_keywords() {
        let prefix = score_palette_match(
            "inst",
            "Instance — Settings",
            "Configure RAM",
            "настройки опра",
        )
        .expect("prefix match");
        let keyword = score_palette_match(
            "опра",
            "Instance — Settings",
            "Configure RAM",
            "настройки опра",
        )
        .expect("russian keyword match");
        assert!(prefix > keyword);

        // Multi-token match across title + keywords
        assert!(score_palette_match(
            "настройки опра",
            "Instance — Settings",
            "Configure RAM",
            "настройки опра",
        )
        .is_some());

        // Non-matching token fails
        assert!(score_palette_match(
            "shaders nonexistent",
            "Instance — Shaderpacks",
            "Manage shaders",
            "шейдеры",
        )
        .is_none());
    }

    #[test]
    fn test_win32_file_filter_double_nul_utf16() {
        let utf16 = build_win32_file_filter_utf16("Modpacks", &["mrpack", "zip"]);
        assert!(utf16.ends_with(&[0, 0]));
        let decoded = String::from_utf16(&utf16).unwrap();
        assert_eq!(decoded, "Modpacks (*.mrpack;*.zip)\0*.mrpack;*.zip\0\0");
    }

    #[test]
    fn test_visible_console_range_binary_search() {
        assert_eq!(visible_console_range(&[0.0], 0.0, 200.0), (0, 0));
        let offsets = [0.0, 22.0, 44.0, 80.0, 102.0, 124.0];
        assert_eq!(visible_console_range(&offsets, 0.0, 50.0), (0, 3));
        assert_eq!(visible_console_range(&offsets, 22.0, 80.0), (1, 3));
        assert_eq!(visible_console_range(&offsets, 45.0, 124.0), (2, 5));
    }
}
