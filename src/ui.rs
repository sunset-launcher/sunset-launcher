use crate::auth::{
    ensure_authlib_injector, ensure_valid_account, login_ely_by, start_microsoft_device_login,
    DEFAULT_MS_CLIENT_ID,
};
use crate::discord::{
    format_presence, unix_now_secs, DiscordActivityKind, DiscordRpc, PresenceState,
};
use crate::i18n::{tr, Language};
use crate::icons::{draw_brand_logo, draw_icon, draw_player_pixel_avatar, Icon};
use crate::launcher::{
    detect_java_installations, download_java_major, ensure_java_runtime, is_managed_java_path,
    launch_game, open_in_file_manager, remove_managed_java,
};
use crate::models::{
    Account, AccountKind, AccountStore, DeviceCodeInfo, GameHandle, InstalledProject, JavaInstall,
    LauncherPaths, LauncherSettings, LoaderKind, LoaderVersionEntry, ModLoader,
    ModrinthInstallOutcome, ModrinthProjectType, ModrinthSearchHit, ModrinthSort, ModrinthVersion,
    Profile, ThemePreset, VersionManifest, VersionType, WorkerEvent,
};
use crate::modrinth::{
    delete_content_file, disable_incompatible_mods_for_loader, export_instance_mrpack, fetch_icon,
    fetch_project_versions_for_profile, import_local_file, install_project,
    install_specific_version, list_content_items, list_screenshots, list_worlds,
    load_installed_index, search_projects, set_all_content_enabled, toggle_content_file,
    update_all_installed_projects, SearchParams,
};
use crate::mojang::{fetch_loader_versions, fetch_version_manifest, prepare_version};
use crate::storage::{
    instance_dir, is_version_installed, load_selected_profile_id, resolve_selected_profile_index,
    save_accounts, save_launcher_settings, save_profiles, save_selected_profile_id,
};
use crate::theme::{
    self, animated_card, animated_progress_bar, apply_modrinth_theme, badge, badge_sized,
    card_frame, draw_icon_placeholder, draw_instance_avatar, draw_mini_gauge,
    draw_ram_allocation_bar, draw_skeleton_card, hero_spotlight_card, lerp_color, loader_badge,
    loader_badge_sized, loader_colors, mod_toggle_switch, mono_spinner, primary_action_button,
    section_header, set_accent_rgb, solid_accent_badge, ui_alert_banner, ui_button,
    ui_button_icon, ui_button_sized, ui_checkbox, ui_icon_button, ui_loader_pill, ui_pill_tab,
    ui_pill_tab_icon, ui_ram_slider, ui_search_input, ui_text_input, ui_toolbar_row, ui_wrap_row,
    with_alpha, AlertKind,
    ButtonVariant, accent, accent_dim,
    BG_CARD, bg_card_active, BG_CARD_HOVER, BG_INPUT, BG_SIDEBAR, BORDER_STRONG, BORDER_SUBTLE,
    DANGER, INFO,
    PURE_WHITE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY, WARNING,
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
    Console,
    Accounts,
    Settings,
}

impl NavPage {
    const ALL: [NavPage; 6] = [
        NavPage::Library,
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
            NavPage::Console => Icon::Terminal,
            NavPage::Accounts => Icon::User,
            NavPage::Settings => Icon::Sparkle,
        }
    }
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

    fn label(self) -> &'static str {
        match self {
            PaletteCategory::All => "All",
            PaletteCategory::Menus => "Menus & Tabs",
            PaletteCategory::Instances => "Instances",
            PaletteCategory::Actions => "Actions",
        }
    }

    fn short_badge(self) -> &'static str {
        match self {
            PaletteCategory::All => "All",
            PaletteCategory::Menus => "Menu",
            PaletteCategory::Instances => "Instance",
            PaletteCategory::Actions => "Action",
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

pub struct McLauncherApp {
    paths: LauncherPaths,
    profiles: Vec<Profile>,
    selected_idx: usize,
    nav_page: NavPage,
    instance_tab: InstanceSubTab,
    page_switch_time: f64,
    sidebar_collapsed_manual: Option<bool>,

    library_search: String,
    library_filter: LibraryFilter,
    library_sort: LibrarySort,
    library_view: LibraryViewMode,
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
    discord_rpc: DiscordRpc,
    rpc_started_at: i64,
    java_downloading_major: Option<u32>,

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
            sidebar_collapsed_manual: None,
            library_search: String::new(),
            library_filter: LibraryFilter::All,
            library_sort: LibrarySort::Recent,
            library_view: LibraryViewMode::Grid,
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
            discord_rpc: DiscordRpc::spawn(),
            rpc_started_at: unix_now_secs(),
            java_downloading_major: None,
            tx,
            rx,
            icon_tx,
        };
        app.refresh_manifest();
        app.scan_java();
        app.sync_installed_index();
        app.trigger_modrinth_search(false);
        app.sync_discord_presence();
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
            NavPage::Library | NavPage::Settings => DiscordActivityKind::InLauncher,
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
            if page == NavPage::Console {
                self.log_error_count = 0;
            } else if page == NavPage::Discover {
                self.sync_discover_for_profile();
            }
            self.sync_discord_presence();
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
        copy.created_at = chrono::Utc::now().to_rfc3339();
        copy.last_played = None;
        self.profiles.push(copy);
        let new_idx = self.profiles.len() - 1;
        let _ = save_profiles(&self.paths, &self.profiles);
        self.select_profile(new_idx);
        self.show_toast(ctx, format!("Duplicated '{}'", orig.name));
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
        let (game_version, loader_filter) = self
            .profiles
            .get(self.selected_idx)
            .map(|p| (Some(p.version_id.clone()), Some(p.loader.kind())))
            .unwrap_or((None, None));

        let params = SearchParams {
            query: self.mr_query.clone(),
            project_type: self.mr_type,
            sort: self.mr_sort,
            game_version,
            loader_filter,
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

        self.is_busy = true;
        self.progress = 0.03;
        self.status_text = format!("Preparing {}...", profile_clone.version_id);
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
                    tx.clone(),
                )
            })();
            let _ = tx.send(WorkerEvent::LaunchFinished(res));
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
            self.status_text = "Stopping game...".to_string();
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
                            self.game_handle = Some(handle);
                            self.launch_started_at = Some(std::time::Instant::now());
                            self.status_text = "Minecraft running".to_string();
                            self.push_log("[Launcher] Minecraft process started".to_string());
                            self.show_toast(ctx, "Game launched");
                            self.scan_java();
                            self.sync_discord_presence();
                        }
                        Err(e) => {
                            self.running_profile_id = None;
                            self.launch_started_at = None;
                            self.progress = 0.0;
                            self.status_text = format!("Error: {e}");
                            self.push_log(format!("[Launcher ERROR] {e}"));
                            self.show_toast(ctx, format!("Launch failed: {e}"));
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
                        }
                        Err(e) => {
                            self.auth_error = Some(e);
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
                        }
                        Err(e) => {
                            self.auth_error = Some(e);
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
                        self.mr_installed_profile_id = p.id.clone();
                    }
                    match res {
                        Ok(ModrinthInstallOutcome::FileInstalled {
                            title,
                            filename,
                            dest_folder,
                            dependencies_installed,
                        }) => {
                            let msg = if dependencies_installed.is_empty() {
                                format!("Installed '{title}' ({filename}) to {dest_folder}/")
                            } else {
                                format!(
                                    "Installed '{title}' + dependencies: {}",
                                    dependencies_installed.join(", ")
                                )
                            };
                            self.mr_status = Some(msg.clone());
                            self.status_text = msg.clone();
                            self.show_toast(ctx, msg);
                        }
                        Ok(ModrinthInstallOutcome::ModpackProfileCreated {
                            profile,
                            files_count,
                        }) => {
                            let name = profile.name.clone();
                            self.profiles.push(profile);
                            let new_idx = self.profiles.len() - 1;
                            let _ = save_profiles(&self.paths, &self.profiles);
                            self.select_profile(new_idx);
                            let msg = format!(
                                "Modpack '{name}' installed ({files_count} files), instance created!"
                            );
                            self.mr_status = Some(msg.clone());
                            self.status_text = msg.clone();
                            self.show_toast(ctx, msg);
                        }
                        Err(e) => {
                            self.mr_error = Some(e.clone());
                            self.status_text = format!("Modrinth error: {e}");
                            self.show_toast(ctx, format!("Error: {e}"));
                        }
                    }
                }
                WorkerEvent::ModUpdatesFinished(res) => {
                    self.mr_installing = false;
                    self.progress = 0.0;
                    if let Some(p) = self.profiles.get(self.selected_idx) {
                        self.mr_installed_cache = load_installed_index(&self.paths, p);
                        self.mr_installed_profile_id = p.id.clone();
                    }
                    match res {
                        Ok(updated) if updated.is_empty() => {
                            self.status_text = "All tracked mods are up to date".to_string();
                            self.show_toast(ctx, "All tracked mods are up to date");
                        }
                        Ok(updated) => {
                            let msg = format!(
                                "Updated {} project(s): {}",
                                updated.len(),
                                updated.join(", ")
                            );
                            self.status_text = msg.clone();
                            self.show_toast(ctx, msg);
                        }
                        Err(e) => {
                            self.status_text = format!("Update error: {e}");
                            self.show_toast(ctx, format!("Error: {e}"));
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
            self.show_cmd_palette = false;
            self.mr_detail_hit = None;
        }
        if n_pressed {
            self.show_new_modal = true;
        }
        if let Some(idx) = num_key {
            if let Some(page) = NavPage::ALL.get(idx).copied() {
                self.switch_page(ctx, page);
            }
        }
    }

    fn profile_mod_counts(&self, profile: &Profile) -> (usize, usize) {
        let items = list_content_items(&instance_dir(&self.paths, profile).join("mods"));
        let enabled = items.iter().filter(|m| m.enabled).count();
        (items.len(), enabled)
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
                            AccountKind::Offline => TEXT_SECONDARY,
                        };
                        (a.username.clone(), a.kind_label(), col)
                    })
                    .unwrap_or_else(|| ("None".to_string(), "—", TEXT_MUTED));
                let foot_pad_x = 6.0;
                let foot_inner_w = (ui.available_width() - foot_pad_x * 2.0 - 4.0).max(28.0);
                let foot_id = ui.make_persistent_id("nav_acc_footer_card");
                let prev_hov = ui.ctx().data(|d| d.get_temp::<bool>(foot_id)).unwrap_or(false);
                let hov_t = ui.ctx().animate_bool_with_time(foot_id, prev_hov, 0.12);
                if hov_t > 0.01 && hov_t < 0.99 {
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
                        draw_player_pixel_avatar(ui.painter(), av_r, &acc_name, acc_col);
                        let dot_c = Pos2::new(av_r.right() - 1.5, av_r.bottom() - 1.5);
                        ui.painter().circle_filled(dot_c, 4.2, BG_SIDEBAR);
                        ui.painter().circle_filled(dot_c, 3.0, acc_col);

                        if slot_r.width() > 36.0 {
                            let clipped = ui.painter().with_clip_rect(slot_r);
                            let text_x = av_r.right() + 9.0;
                            clipped.text(
                                Pos2::new(text_x, slot_r.top() + 8.5),
                                egui::Align2::LEFT_CENTER,
                                &acc_name,
                                egui::FontId::proportional(13.0),
                                PURE_WHITE,
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
                let hovered_now = frame_resp.response.hovered()
                    || ui.rect_contains_pointer(frame_resp.response.rect);
                ui.ctx().data_mut(|d| d.insert_temp(foot_id, hovered_now));
                let resp = frame_resp
                    .response
                    .interact(Sense::click())
                    .on_hover_text(format!("Account: {acc_name} ({acc_kind})"));
                if resp.hovered() || hovered_now {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
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
                        "by svarka (@svarkawm) • MIT",
                        egui::FontId::proportional(10.0),
                        TEXT_MUTED,
                    );
                }
            });

        // Brand + vertical navigation
        ui.vertical(|ui| {
            let brand_id = ui.make_persistent_id("nav_brand_lockup");
            let brand_hov_prev = ui.ctx().data_mut(|d| d.get_temp::<bool>(brand_id).unwrap_or(false));
            let brand_ht = ui.ctx().animate_bool_with_time(brand_id, brand_hov_prev, 0.14);
            let brand_fill = if brand_ht > 0.01 {
                with_alpha(accent(), (brand_ht * 16.0) as u8)
            } else {
                Color32::TRANSPARENT
            };
            let brand_stroke = if brand_ht > 0.01 {
                Stroke::new(1.0_f32, with_alpha(accent(), (brand_ht * 110.0) as u8))
            } else {
                Stroke::NONE
            };
            let brand_resp = egui::Frame::none()
                .fill(brand_fill)
                .stroke(brand_stroke)
                .rounding(Rounding::same(10.0))
                .inner_margin(Margin::symmetric(4.0, 5.0))
                .show(ui, |ui| {
                    let row_w = ui.available_width().max(36.0);
                    let (row_rect, _) =
                        ui.allocate_exact_size(Vec2::new(row_w, 36.0), Sense::hover());
                    let icon_rect = Rect::from_min_size(row_rect.min, Vec2::splat(36.0));
                    draw_brand_logo(ui.painter(), icon_rect);
                    if row_rect.width() > 42.0 {
                        let clipped = ui.painter().with_clip_rect(row_rect);
                        let text_x = row_rect.left() + 45.0;
                        clipped.text(
                            Pos2::new(text_x, row_rect.top() + 10.5),
                            egui::Align2::LEFT_CENTER,
                            "Svarka Launcher",
                            egui::FontId::proportional(15.0),
                            PURE_WHITE,
                        );
                        clipped.circle_filled(
                            Pos2::new(text_x + 3.0, row_rect.top() + 26.5),
                            2.5,
                            accent(),
                        );
                        clipped.text(
                            Pos2::new(text_x + 10.0, row_rect.top() + 26.5),
                            egui::Align2::LEFT_CENTER,
                            "Modrinth • Multi-Loader",
                            egui::FontId::proportional(10.5),
                            TEXT_MUTED,
                        );
                    }
                });
            let brand_hov_now = brand_resp.response.hovered()
                || ui.rect_contains_pointer(brand_resp.response.rect);
            ui.ctx().data_mut(|d| d.insert_temp(brand_id, brand_hov_now));
            let brand_click = brand_resp
                .response
                .interact(Sense::click())
                .on_hover_text("Svarka Launcher v0.1.1 • by svarka (@svarkawm) — Go to Library");
            if brand_click.hovered() || brand_hov_now {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if brand_click.clicked() {
                self.switch_page(ui.ctx(), NavPage::Library);
            }

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            let is_game_running = self.game_handle.is_some();

            bounded_scroll(ui, "nav_sidebar_scroll", |ui| {
                egui::Frame::none()
                    .inner_margin(Margin::symmetric(2.0, 1.0))
                    .show(ui, |ui| {
                        let nav_w = (ui.available_width() - 4.0).max(24.0);
                        let mut active_nav_rect = Rect::NOTHING;
                        for (page_idx, page) in NavPage::ALL.iter().copied().enumerate() {
                            let is_active = self.nav_page == page;
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
                            if hover_t > 0.01 && hover_t < 0.99 {
                                ui.ctx().request_repaint();
                            }

                            let bg = lerp_color(
                                lerp_color(Color32::TRANSPARENT, BG_CARD, hover_t),
                                bg_card_active(),
                                active_t,
                            );
                            let painter = ui.painter();
                            painter.rect_filled(rect, Rounding::same(8.0), bg);
                            if active_t > 0.05 {
                                painter.rect_stroke(
                                    rect,
                                    Rounding::same(8.0),
                                    Stroke::new(1.0_f32, with_alpha(accent(), (active_t * 95.0) as u8)),
                                );
                            }

                            let fg =
                                lerp_color(TEXT_SECONDARY, PURE_WHITE, active_t.max(hover_t * 0.7));
                            let icon_col = lerp_color(
                                lerp_color(TEXT_MUTED, TEXT_SECONDARY, hover_t),
                                accent(),
                                active_t,
                            );
                            let slide_x = if compact || rect.width() < 64.0 {
                                0.0
                            } else {
                                hover_t * 2.2
                            };
                            let ic_rect = Rect::from_center_size(
                                Pos2::new(rect.left() + 20.0 + slide_x * 0.5, rect.center().y),
                                Vec2::splat(16.0),
                            );
                            draw_icon(painter, ic_rect, page.icon(), icon_col);

                            if compact && rect.width() < 64.0 {
                                if page == NavPage::Console
                                    && (self.log_error_count > 0 || is_game_running)
                                {
                                    let dot_col = if self.log_error_count > 0 {
                                        DANGER
                                    } else {
                                        accent()
                                    };
                                    painter.circle_filled(
                                        Pos2::new(rect.right() - 7.0, rect.top() + 7.0),
                                        3.2,
                                        dot_col,
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

                                // Live contextual pill/dot on right side of nav row
                                if rect.width() >= 140.0 {
                                    match page {
                                    NavPage::Library => {
                                        let label = format!("{}", self.profiles.len());
                                        let pill = Rect::from_center_size(
                                            Pos2::new(rect.right() - 17.0, rect.center().y),
                                            Vec2::new((label.len() as f32 * 7.0 + 12.0).max(20.0), 16.0),
                                        );
                                        painter.rect_filled(
                                            pill,
                                            Rounding::same(6.0),
                                            if is_active {
                                                accent_dim()
                                            } else {
                                                Color32::from_rgb(30, 34, 42)
                                            },
                                        );
                                        painter.text(
                                            pill.center(),
                                            egui::Align2::CENTER_CENTER,
                                            label,
                                            egui::FontId::proportional(10.5),
                                            if is_active { accent() } else { TEXT_MUTED },
                                        );
                                    }
                                    NavPage::Instance => {
                                        if let Some(cur) = self.profiles.get(self.selected_idx) {
                                            let (_, en) = self.profile_mod_counts(cur);
                                            let text = if en > 0 {
                                                format!("{en}")
                                            } else {
                                                cur.loader.short_name().to_string()
                                            };
                                            let pw = (text.len() as f32 * 6.2 + 12.0).clamp(20.0, 56.0);
                                            let pill = Rect::from_center_size(
                                                Pos2::new(rect.right() - pw * 0.5 - 7.0, rect.center().y),
                                                Vec2::new(pw, 16.0),
                                            );
                                            painter.rect_filled(
                                                pill,
                                                Rounding::same(6.0),
                                                if is_active {
                                                    accent_dim()
                                                } else {
                                                    Color32::from_rgb(30, 34, 42)
                                                },
                                            );
                                            painter.text(
                                                pill.center(),
                                                egui::Align2::CENTER_CENTER,
                                                text,
                                                egui::FontId::proportional(10.5),
                                                if is_active { accent() } else { TEXT_MUTED },
                                            );
                                        }
                                    }
                                    NavPage::Console => {
                                        if self.log_error_count > 0 {
                                            let pill = Rect::from_center_size(
                                                Pos2::new(rect.right() - 16.0, rect.center().y),
                                                Vec2::new(20.0, 16.0),
                                            );
                                            painter.rect_filled(
                                                pill,
                                                Rounding::same(8.0),
                                                with_alpha(DANGER, 210),
                                            );
                                            painter.text(
                                                pill.center(),
                                                egui::Align2::CENTER_CENTER,
                                                format!("{}", self.log_error_count.min(99)),
                                                egui::FontId::proportional(10.0),
                                                PURE_WHITE,
                                            );
                                        } else if is_game_running {
                                            let pulse =
                                                (ui.input(|i| i.time) as f32 * 3.8).sin() * 0.5
                                                    + 0.5;
                                            let dot_c =
                                                Pos2::new(rect.right() - 13.0, rect.center().y);
                                            painter.circle_stroke(
                                                dot_c,
                                                4.0 + pulse * 1.5,
                                                Stroke::new(
                                                    1.0_f32,
                                                    with_alpha(accent(), (120.0 * (1.0 - pulse * 0.5)) as u8),
                                                ),
                                            );
                                            painter.circle_filled(dot_c, 3.2, accent());
                                            ui.ctx().request_repaint();
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

                        // Smoothly sliding emerald indicator along the left rail
                        if active_nav_rect != Rect::NOTHING {
                            let anim_y = ui.ctx().animate_value_with_time(
                                ui.make_persistent_id("nav_rail_indicator_y"),
                                active_nav_rect.center().y,
                                0.16,
                            );
                            if (anim_y - active_nav_rect.center().y).abs() > 0.3 {
                                ui.ctx().request_repaint();
                            }
                            let bar = Rect::from_center_size(
                                Pos2::new(active_nav_rect.left() + 2.5, anim_y),
                                Vec2::new(3.2, 18.0),
                            );
                            ui.painter().rect_filled(bar, Rounding::same(1.6), accent());
                        }

                        let expand_t = ui.ctx().animate_bool_with_time(
                            ui.make_persistent_id("quick_switch_expand_t"),
                            !compact,
                            0.16,
                        );
                        if expand_t > 0.01 && expand_t < 0.99 {
                            ui.ctx().request_repaint();
                        }
                        if expand_t > 0.01 {
                            let full_w = nav_w.max(180.0);
                            let slide_x = -(1.0 - expand_t) * (full_w + 8.0);
                            let scale_alpha = |col: Color32, t: f32| -> Color32 {
                                Color32::from_rgba_unmultiplied(
                                    col.r(),
                                    col.g(),
                                    col.b(),
                                    ((col.a() as f32) * t).clamp(0.0, 255.0) as u8,
                                )
                            };

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(6.0);

                            let (hdr_rect, _) =
                                ui.allocate_exact_size(Vec2::new(nav_w, 22.0), Sense::hover());
                            let hdr_slide = Rect::from_min_size(
                                Pos2::new(hdr_rect.left() + slide_x, hdr_rect.top()),
                                Vec2::new(full_w, 22.0),
                            );
                            let plus_rect = Rect::from_center_size(
                                Pos2::new(hdr_slide.right() - 10.0, hdr_slide.center().y),
                                Vec2::splat(20.0),
                            );
                            let plus_id = ui.make_persistent_id("sidebar_quick_new_slide");
                            let plus_resp = if expand_t > 0.5 && hdr_rect.contains(plus_rect.center())
                            {
                                Some(
                                    ui.interact(plus_rect, plus_id, Sense::click())
                                        .on_hover_text("New Instance (Ctrl+N)"),
                                )
                            } else {
                                None
                            };
                            let plus_hov = plus_resp.as_ref().is_some_and(|r| r.hovered());
                            if plus_hov {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if plus_resp.as_ref().is_some_and(|r| r.clicked()) {
                                self.show_new_modal = true;
                            }

                            let hdr_clip =
                                ui.painter().with_clip_rect(hdr_rect.expand2(Vec2::new(0.0, 2.0)));
                            hdr_clip.text(
                                Pos2::new(hdr_slide.left() + 4.0, hdr_slide.center().y),
                                egui::Align2::LEFT_CENTER,
                                tr(self.settings.language, "side.quick_switch"),
                                egui::FontId::proportional(11.0),
                                scale_alpha(TEXT_MUTED, expand_t),
                            );
                            hdr_clip.rect(
                                plus_rect,
                                Rounding::same(6.0),
                                scale_alpha(
                                    if plus_hov { BG_CARD_HOVER } else { BG_CARD },
                                    expand_t,
                                ),
                                Stroke::new(
                                    1.0_f32,
                                    scale_alpha(
                                        if plus_hov { BORDER_STRONG } else { BORDER_SUBTLE },
                                        expand_t,
                                    ),
                                ),
                            );
                            draw_icon(
                                &hdr_clip,
                                plus_rect.shrink(4.0),
                                Icon::Plus,
                                scale_alpha(if plus_hov { PURE_WHITE } else { TEXT_SECONDARY }, expand_t),
                            );
                            ui.add_space(4.0);

                            let mut picked: Option<usize> = None;
                            let mut quick_order: Vec<usize> = (0..self.profiles.len()).collect();
                            quick_order.sort_by(|&a, &b| {
                                self.profiles[b].pinned.cmp(&self.profiles[a].pinned)
                            });
                            for idx in quick_order {
                                let p = &self.profiles[idx];
                                let sel = idx == self.selected_idx;
                                let is_running =
                                    self.running_profile_id.as_deref() == Some(p.id.as_str())
                                        && self.game_handle.is_some();
                                let hov_id = ui.make_persistent_id(("quick_inst_hov", &p.id));
                                let (row_rect, resp) =
                                    ui.allocate_exact_size(Vec2::new(nav_w, 30.0), Sense::click());
                                let hov_t = ui.ctx().animate_bool_with_time(
                                    hov_id,
                                    resp.hovered() && expand_t > 0.5,
                                    0.11,
                                );
                                if hov_t > 0.01 && hov_t < 0.99 {
                                    ui.ctx().request_repaint();
                                }
                                let bg = if sel {
                                    accent_dim()
                                } else {
                                    lerp_color(Color32::TRANSPARENT, BG_CARD, hov_t)
                                };
                                let border = if sel { accent() } else { Color32::TRANSPARENT };
                                let (_, loader_dot_col, _) = loader_colors(p.loader.kind());
                                let dot_col = if is_running { accent() } else { loader_dot_col };
                                let fg = if is_running {
                                    accent()
                                } else if sel {
                                    Color32::from_rgb(168, 250, 202)
                                } else {
                                    lerp_color(TEXT_SECONDARY, PURE_WHITE, hov_t)
                                };

                                let draw_rect = Rect::from_min_size(
                                    Pos2::new(row_rect.left() + slide_x + 1.0, row_rect.top() + 1.0),
                                    Vec2::new((full_w - 2.0).max(20.0), 28.0),
                                );
                                let row_clip_rect = Rect::from_min_max(
                                    Pos2::new(row_rect.left(), row_rect.top() - 2.0),
                                    Pos2::new(row_rect.right(), row_rect.bottom() + 2.0),
                                );
                                let clipped = ui.painter().with_clip_rect(row_clip_rect);
                                clipped.rect(
                                    draw_rect,
                                    Rounding::same(6.5),
                                    scale_alpha(bg, expand_t),
                                    Stroke::new(
                                        if sel { 1.0_f32 } else { 0.0_f32 },
                                        scale_alpha(border, expand_t),
                                    ),
                                );
                                clipped.circle_filled(
                                    Pos2::new(draw_rect.left() + 11.0, draw_rect.center().y),
                                    3.0,
                                    scale_alpha(dot_col, expand_t),
                                );
                                let ver_galley = clipped.layout_no_wrap(
                                    p.version_id.clone(),
                                    egui::FontId::proportional(10.0),
                                    scale_alpha(TEXT_MUTED, expand_t),
                                );
                                let ver_w = ver_galley.size().x;
                                clipped.galley(
                                    Pos2::new(
                                        draw_rect.right() - 8.0 - ver_w,
                                        draw_rect.center().y - ver_galley.size().y * 0.5,
                                    ),
                                    ver_galley,
                                    scale_alpha(TEXT_MUTED, expand_t),
                                );
                                let name_bounds = Rect::from_min_max(
                                    Pos2::new(draw_rect.left() + 20.0, draw_rect.top()),
                                    Pos2::new(
                                        (draw_rect.right() - 14.0 - ver_w)
                                            .max(draw_rect.left() + 20.0),
                                        draw_rect.bottom(),
                                    ),
                                )
                                .intersect(row_clip_rect);
                                if name_bounds.width() > 2.0 {
                                    let name_clip = ui.painter().with_clip_rect(name_bounds);
                                    name_clip.text(
                                        Pos2::new(draw_rect.left() + 20.0, draw_rect.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        &p.name,
                                        egui::FontId::proportional(12.5),
                                        scale_alpha(fg, expand_t),
                                    );
                                }
                                if resp.hovered() && expand_t > 0.5 {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if resp.clicked() && expand_t > 0.3 {
                                    picked = Some(idx);
                                }
                                ui.add_space(2.0);
                            }
                            if let Some(idx) = picked {
                                self.select_profile(idx);
                            }
                        }
                    });
            });
        });
    }

    fn render_top_bar(&mut self, ui: &mut egui::Ui, compact_sidebar: bool) {
        let avail_w = ui.available_width();
        let btn_w = if avail_w < 480.0 { 112.0 } else { 142.0 };
        let show_cmd_btn = avail_w >= 540.0;
        let wide_cmd_btn = avail_w >= 760.0;
        let show_account_chip = avail_w >= 680.0;
        let acc_chip_w = if show_account_chip { 132.0 } else { 0.0 };
        let cmd_btn_w = if wide_cmd_btn {
            156.0
        } else if show_cmd_btn {
            38.0
        } else {
            0.0
        };
        let left_w = (avail_w - btn_w - acc_chip_w - cmd_btn_w - 26.0).max(80.0);

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
                    if ui_icon_button(
                        ui,
                        "sidebar_collapse_btn",
                        if compact_sidebar {
                            Icon::ChevronRight
                        } else {
                            Icon::ChevronLeft
                        },
                        "Collapse / expand sidebar",
                        ButtonVariant::Subtle,
                        true,
                        34.0,
                    )
                    .clicked()
                    {
                        self.sidebar_collapsed_manual = Some(!compact_sidebar);
                    }

                    if let Some(cur) = self.profiles.get(self.selected_idx).cloned() {
                        let show_folder = left_w >= 430.0;
                        let show_badge = left_w >= 340.0;
                        let show_avatar = left_w >= 250.0;
                        let reserve = 34.0
                            + 8.0
                            + (if show_avatar { 40.0 } else { 0.0 })
                            + (if show_badge { 88.0 } else { 0.0 })
                            + (if show_folder { 42.0 } else { 0.0 })
                            + 18.0;
                        let combo_w = (left_w - reserve).clamp(80.0, 230.0);

                        if show_avatar {
                            let (av_rect, av_resp) =
                                ui.allocate_exact_size(Vec2::splat(34.0), Sense::click());
                            draw_instance_avatar(
                                ui.painter(),
                                av_rect,
                                &cur.name,
                                cur.loader.kind(),
                                true,
                                self.game_handle.is_some(),
                            );
                            if av_resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if av_resp
                                .on_hover_text("Open instance details (Ctrl+2)")
                                .clicked()
                            {
                                self.switch_page(ui.ctx(), NavPage::Instance);
                            }
                        }

                        egui::ComboBox::from_id_salt("topbar_instance_combo")
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
                                if switched {
                                    self.persist_selected_profile();
                                    self.sync_installed_index();
                                    self.sync_discover_for_profile();
                                }
                            });

                        if show_badge {
                            loader_badge_sized(
                                ui,
                                cur.loader.kind(),
                                cur.loader.short_name(),
                                34.0,
                            );
                        }
                        if show_folder
                            && ui_icon_button(
                                ui,
                                "topbar_folder_btn",
                                Icon::Folder,
                                "Open instance folder",
                                ButtonVariant::Subtle,
                                true,
                                34.0,
                            )
                            .clicked()
                        {
                            open_in_file_manager(&instance_dir(&self.paths, &cur));
                        }
                    }
                },
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let is_running = self.game_handle.is_some();
                let _compact_btn = btn_w < 130.0;
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

                if primary_action_button(
                    ui,
                    "main_play_btn",
                    btn_text,
                    icon,
                    enabled,
                    stop_style,
                    pulsing,
                    Vec2::new(btn_w, 34.0),
                )
                .clicked()
                {
                    if is_running {
                        self.stop_game();
                    } else {
                        self.start_launch();
                    }
                }

                if show_account_chip {
                    let (acc_name, acc_col) = self
                        .active_account()
                        .map(|a| {
                            let col = match a.kind {
                                AccountKind::Microsoft { .. } => accent(),
                                AccountKind::ElyBy { .. } => INFO,
                                AccountKind::Offline => TEXT_SECONDARY,
                            };
                            (a.username.clone(), col)
                        })
                        .unwrap_or_else(|| ("Offline".to_string(), TEXT_MUTED));
                    let name_galley = ui.painter().layout_no_wrap(
                        acc_name.clone(),
                        egui::FontId::proportional(12.5),
                        TEXT_PRIMARY,
                    );
                    let chip_w = (name_galley.size().x + 42.0).clamp(76.0, 128.0);
                    let (chip_rect, chip_resp) =
                        ui.allocate_exact_size(Vec2::new(chip_w, 34.0), Sense::click());
                    if ui.is_rect_visible(chip_rect) {
                        let fill = if chip_resp.hovered() {
                            BG_CARD_HOVER
                        } else {
                            BG_CARD
                        };
                        ui.painter().rect(
                            chip_rect,
                            Rounding::same(7.5),
                            fill,
                            Stroke::new(1.0_f32, BORDER_SUBTLE),
                        );
                        let av_rect = Rect::from_center_size(
                            Pos2::new(chip_rect.left() + 17.0, chip_rect.center().y),
                            Vec2::splat(20.0),
                        );
                        draw_player_pixel_avatar(ui.painter(), av_rect, &acc_name, acc_col);
                        let text_max_w = (chip_w - 38.0).max(20.0);
                        let draw_galley = if name_galley.size().x <= text_max_w {
                            name_galley
                        } else {
                            ui.painter().layout(
                                acc_name.clone(),
                                egui::FontId::proportional(12.5),
                                TEXT_PRIMARY,
                                text_max_w,
                            )
                        };
                        ui.painter().galley(
                            Pos2::new(
                                chip_rect.left() + 32.0,
                                chip_rect.center().y - draw_galley.size().y * 0.5,
                            ),
                            draw_galley,
                            TEXT_PRIMARY,
                        );
                    }
                    let chip_resp = chip_resp.on_hover_text("Switch or manage accounts");
                    if chip_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if chip_resp.clicked() {
                        self.switch_page(ui.ctx(), NavPage::Accounts);
                    }
                }

                if show_cmd_btn {
                    let clicked = if wide_cmd_btn {
                        ui_button_icon(
                            ui,
                            "topbar_cmd_palette_btn",
                            Some(Icon::Search),
                            tr(self.settings.language, "top.search"),
                            ButtonVariant::Subtle,
                            true,
                            34.0,
                        )
                        .on_hover_text("Search menus, tabs, instances, and actions (Ctrl+K)")
                        .clicked()
                    } else {
                        ui_icon_button(
                            ui,
                            "topbar_cmd_palette_btn",
                            Icon::Search,
                            "Search menu (Ctrl+K)",
                            ButtonVariant::Subtle,
                            true,
                            34.0,
                        )
                        .clicked()
                    };
                    if clicked {
                        self.show_cmd_palette = true;
                        self.cmd_palette_query.clear();
                        self.cmd_palette_selected = 0;
                        self.cmd_palette_category = PaletteCategory::All;
                        self.cmd_palette_just_opened = true;
                    }
                }
            });
        });
    }

    fn render_bottom_bar(&mut self, ui: &mut egui::Ui) {
        let busy_progress = self.is_busy || self.mr_installing || self.progress > 0.0;
        if busy_progress {
            animated_progress_bar(
                ui,
                "main_progress",
                self.progress,
                &self.status_text,
                self.is_busy || self.mr_installing,
            );
        } else {
            let avail_w = ui.available_width();
            let show_console_btn = avail_w >= 360.0;
            let right_w = if avail_w >= 540.0 {
                240.0
            } else if show_console_btn {
                110.0
            } else {
                0.0
            };
            let left_w = (avail_w - right_w - 8.0).max(60.0);

            ui_toolbar_row(ui, 22.0, |ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(left_w, 22.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let (dot, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                        let is_running = self.game_handle.is_some();
                        if is_running {
                            let pulse = (ui.input(|i| i.time) as f32 * 3.8).sin() * 0.5 + 0.5;
                            ui.painter().circle_stroke(
                                dot.center(),
                                3.8 + pulse * 1.4,
                                Stroke::new(
                                    1.0_f32,
                                    with_alpha(accent(), (120.0 * (1.0 - pulse * 0.5)) as u8),
                                ),
                            );
                            ui.painter().circle_filled(dot.center(), 3.5, accent());
                            ui.ctx().request_repaint();
                        } else {
                            ui.painter()
                                .circle_filled(dot.center(), 3.5, Color32::from_rgb(82, 92, 108));
                        }
                        let status_display = if self.status_text == "Ready" {
                            tr(self.settings.language, "status.ready")
                        } else {
                            self.status_text.as_str()
                        };
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(status_display)
                                    .small()
                                    .color(TEXT_SECONDARY),
                            )
                            .truncate(),
                        );
                    },
                );

                if show_console_btn {
                    let mut open_active_inst = false;
                    let inst_summary = (avail_w >= 540.0)
                        .then(|| {
                            self.profiles
                                .get(self.selected_idx)
                                .map(|p| format!("{} • {}", p.name, p.version_id))
                        })
                        .flatten();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let log_lbl = if self.log_error_count > 0 {
                            format!(
                                "{} ({}!)",
                                tr(self.settings.language, "bar.logs"),
                                self.log_error_count
                            )
                        } else {
                            format!(
                                "{} ({})",
                                tr(self.settings.language, "bar.logs"),
                                self.logs.len()
                            )
                        };
                        let log_variant = if self.log_error_count > 0 {
                            ButtonVariant::Danger
                        } else {
                            ButtonVariant::Subtle
                        };
                        if ui_button_icon(
                            ui,
                            "bottom_console_btn",
                            Some(Icon::Terminal),
                            &log_lbl,
                            log_variant,
                            true,
                            20.0,
                        )
                        .clicked()
                        {
                            self.switch_page(ui.ctx(), NavPage::Console);
                        }
                        if let Some(summary) = inst_summary {
                            if ui_button(
                                ui,
                                "bottom_inst_pill",
                                &summary,
                                ButtonVariant::Subtle,
                                true,
                                20.0,
                            )
                            .on_hover_text("Open active instance")
                            .clicked()
                            {
                                open_active_inst = true;
                            }
                        }
                    });
                    if open_active_inst {
                        self.switch_page(ui.ctx(), NavPage::Instance);
                    }
                }
            });
        }
    }

    fn render_library_page(&mut self, ui: &mut egui::Ui) {
        let lang = self.settings.language;
        let avail_w = ui.available_width();
        let wide_hdr = avail_w >= 540.0;
        ui_toolbar_row(ui, 30.0, |ui| {
            ui.heading(egui::RichText::new(tr(lang, "lib.title")).size(21.0).color(PURE_WHITE));
            badge(ui, &format!("{}", self.profiles.len()), false);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if ui_button_icon(
                    ui,
                    "new_instance_btn",
                    Some(Icon::Plus),
                    tr(lang, "btn.create"),
                    ButtonVariant::Primary,
                    true,
                    30.0,
                )
                .clicked()
                {
                    self.show_new_modal = true;
                }
                if wide_hdr {
                    ui_search_input(
                        ui,
                        "lib_search_w",
                        &mut self.library_search,
                        tr(lang, "lib.search"),
                        210.0,
                    );
                }
            });
        });

        ui.add_space(6.0);
        if !wide_hdr {
            ui_toolbar_row(ui, 30.0, |ui| {
                ui_search_input(
                    ui,
                    "lib_search_n",
                    &mut self.library_search,
                    tr(lang, "lib.search"),
                    (avail_w - 10.0).max(120.0),
                );
            });
            ui.add_space(6.0);
        }

        // Filter, Sort & View Mode bar
        {
            let total_cnt = self.profiles.len();
            let modded_cnt = self.profiles.iter().filter(|p| p.loader.is_modded()).count();
            let vanilla_cnt = total_cnt.saturating_sub(modded_cnt);
            ui_wrap_row(ui, 30.0, |ui| {
                for (f, key, lbl) in [
                    (
                        LibraryFilter::All,
                        "all",
                        format!("{} ({total_cnt})", tr(lang, "lib.all")),
                    ),
                    (
                        LibraryFilter::Modded,
                        "modded",
                        format!("{} ({modded_cnt})", tr(lang, "lib.modded")),
                    ),
                    (
                        LibraryFilter::Vanilla,
                        "vanilla",
                        format!("{} ({vanilla_cnt})", tr(lang, "lib.vanilla")),
                    ),
                ] {
                    if ui_pill_tab(ui, ("lib_filter", key), &lbl, self.library_filter == f)
                        .clicked()
                    {
                        self.library_filter = f;
                    }
                }
                egui::ComboBox::from_id_salt("lib_sort_combo")
                    .selected_text(match self.library_sort {
                        LibrarySort::Recent => tr(lang, "lib.sort_recent"),
                        LibrarySort::Name => tr(lang, "lib.sort_name"),
                        LibrarySort::Playtime => tr(lang, "lib.sort_playtime"),
                    })
                    .width(136.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.library_sort,
                            LibrarySort::Recent,
                            tr(lang, "lib.sort_recent"),
                        );
                        ui.selectable_value(
                            &mut self.library_sort,
                            LibrarySort::Name,
                            tr(lang, "lib.sort_name"),
                        );
                        ui.selectable_value(
                            &mut self.library_sort,
                            LibrarySort::Playtime,
                            tr(lang, "lib.sort_playtime"),
                        );
                    });
                if ui_icon_button(
                    ui,
                    "lib_view_grid",
                    Icon::Grid,
                    "Grid view",
                    if self.library_view == LibraryViewMode::Grid {
                        ButtonVariant::Secondary
                    } else {
                        ButtonVariant::Subtle
                    },
                    true,
                    28.0,
                )
                .clicked()
                {
                    self.library_view = LibraryViewMode::Grid;
                }
                if ui_icon_button(
                    ui,
                    "lib_view_list",
                    Icon::List,
                    "List view",
                    if self.library_view == LibraryViewMode::List {
                        ButtonVariant::Secondary
                    } else {
                        ButtonVariant::Subtle
                    },
                    true,
                    28.0,
                )
                .clicked()
                {
                    self.library_view = LibraryViewMode::List;
                }
            });
            ui.add_space(8.0);
        }

        let query = self.library_search.trim().to_lowercase();
        let mut action_select: Option<usize> = None;
        let mut action_play: Option<usize> = None;
        let mut action_open: Option<usize> = None;
        let mut action_mods: Option<usize> = None;
        let mut action_discover: Option<usize> = None;
        let mut action_console = false;
        let mut action_duplicate: Option<usize> = None;
        let mut action_delete: Option<usize> = None;
        let mut action_toggle_pin: Option<usize> = None;
        let can_delete = self.profiles.len() > 1;
        let running_id = self.running_profile_id.clone();

        bounded_scroll(ui, "library_grid_scroll", |ui| {
            let usable_w = (ui.available_width() - 12.0).max(160.0);

            // Active Instance Spotlight Hero Banner
            if query.is_empty() && self.library_filter == LibraryFilter::All {
                if let Some(cur) = self.profiles.get(self.selected_idx).cloned() {
                    let is_running = running_id.as_deref() == Some(cur.id.as_str())
                        && self.game_handle.is_some();
                    let mod_items =
                        list_content_items(&instance_dir(&self.paths, &cur).join("mods"));
                    let enabled_mods = mod_items.iter().filter(|m| m.enabled).count();
                    let total_mods = mod_items.len();
                    let cached = is_version_installed(&self.paths, &cur.version_id);
                    let (_, loader_accent, _) = loader_colors(cur.loader.kind());
                    let hero_inner_w = (usable_w - 28.0).max(130.0);
                    let hero_id = ui.make_persistent_id(("lib_hero_card", &cur.id));
                    hero_spotlight_card(ui, hero_id, loader_accent, |ui| {
                        ui.set_min_width(hero_inner_w);
                        ui.set_max_width(hero_inner_w);
                        ui.horizontal(|ui| {
                            let (av_rect, _) =
                                ui.allocate_exact_size(Vec2::splat(54.0), Sense::hover());
                            draw_instance_avatar(
                                ui.painter(),
                                av_rect,
                                &cur.name,
                                cur.loader.kind(),
                                true,
                                is_running,
                            );

                            let text_w = (hero_inner_w - 66.0).max(80.0);
                            ui.vertical(|ui| {
                                ui.set_max_width(text_w);
                                ui_wrap_row(ui, 20.0, |ui| {
                                    ui.label(
                                        egui::RichText::new(tr(lang, "lib.active_instance"))
                                            .size(9.5)
                                            .strong()
                                            .color(accent()),
                                    );
                                    if is_running {
                                        badge(ui, tr(lang, "lib.running"), true);
                                    } else if cached {
                                        badge(ui, tr(lang, "lib.ready"), true);
                                    }
                                    if let Some(rel) = format_relative_time(cur.last_played.as_deref()) {
                                        ui.label(
                                            egui::RichText::new(format!("• {} {rel}", tr(lang, "lib.played_rel")))
                                                .size(10.5)
                                                .color(TEXT_MUTED),
                                        );
                                    }
                                });
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(&cur.name)
                                            .strong()
                                            .size(17.5)
                                            .color(PURE_WHITE),
                                    )
                                    .truncate(),
                                );
                                ui_wrap_row(ui, 22.0, |ui| {
                                    badge(ui, &cur.version_id, false);
                                    loader_badge(ui, cur.loader.kind(), &cur.loader.label());
                                    badge(ui, &format!("{enabled_mods}/{total_mods} {}", tr(lang, "lib.mods_count")), false);
                                    badge(ui, &format!("{} {}", cur.max_ram_mb, tr(lang, "lib.mb_ram")), false);
                                    if cur.playtime_secs > 0 {
                                        badge(
                                            ui,
                                            &format!("{} {}", format_playtime(cur.playtime_secs), tr(lang, "lib.played")),
                                            false,
                                        );
                                    }
                                    if let Some(acc) = self.active_account() {
                                        badge(
                                            ui,
                                            &format!("{}: {}", tr(lang, "lib.player"), acc.username),
                                            false,
                                        );
                                    }
                                });
                            });
                        });

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(5.0);
                        ui_wrap_row(ui, 30.0, |ui| {
                            if ui_button_icon(
                                ui,
                                "hero_play_btn",
                                Some(if is_running { Icon::Stop } else { Icon::Play }),
                                if is_running {
                                    tr(lang, "btn.stop")
                                } else {
                                    tr(lang, "btn.launch")
                                },
                                if is_running {
                                    ButtonVariant::Danger
                                } else {
                                    ButtonVariant::Primary
                                },
                                !self.is_busy,
                                30.0,
                            )
                            .clicked()
                            {
                                if is_running {
                                    self.stop_game();
                                } else {
                                    action_play = Some(self.selected_idx);
                                }
                            }
                            if ui_button_icon(
                                ui,
                                "hero_discover_btn",
                                Some(Icon::Compass),
                                tr(lang, "btn.add_content"),
                                ButtonVariant::Secondary,
                                true,
                                30.0,
                            )
                            .clicked()
                            {
                                action_discover = Some(self.selected_idx);
                            }
                            if ui_button_icon(
                                ui,
                                "hero_mods_btn",
                                Some(Icon::Layers),
                                tr(lang, "inst.content"),
                                ButtonVariant::Subtle,
                                true,
                                30.0,
                            )
                            .clicked()
                            {
                                action_mods = Some(self.selected_idx);
                            }
                            if ui_button_icon(
                                ui,
                                "hero_cfg_btn",
                                Some(Icon::Sliders),
                                tr(lang, "inst.settings_btn"),
                                ButtonVariant::Subtle,
                                true,
                                30.0,
                            )
                            .clicked()
                            {
                                action_open = Some(self.selected_idx);
                            }
                            if ui_button_icon(
                                ui,
                                "hero_console_btn",
                                Some(Icon::Terminal),
                                tr(lang, "nav.console"),
                                ButtonVariant::Subtle,
                                true,
                                30.0,
                            )
                            .clicked()
                            {
                                action_console = true;
                            }
                            if ui_icon_button(
                                ui,
                                "hero_folder_btn",
                                Icon::Folder,
                                "Open instance folder",
                                ButtonVariant::Subtle,
                                true,
                                30.0,
                            )
                            .clicked()
                            {
                                open_in_file_manager(&instance_dir(&self.paths, &cur));
                            }
                            if ui_icon_button(
                                ui,
                                "hero_dup_btn",
                                Icon::Duplicate,
                                "Duplicate instance",
                                ButtonVariant::Subtle,
                                true,
                                30.0,
                            )
                            .clicked()
                            {
                                action_duplicate = Some(self.selected_idx);
                            }
                        });
                    });
                    ui.add_space(12.0);
                }
            }

            let cols = if usable_w >= 820.0 {
                3
            } else if usable_w >= 480.0 {
                2
            } else {
                1
            };
            let gap = 10.0;
            let card_w = ((usable_w - gap * (cols as f32 - 1.0)) / cols as f32)
                .floor()
                .max(150.0);

            let mut visible: Vec<(usize, Profile)> = self
                .profiles
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    query.is_empty()
                        || p.name.to_lowercase().contains(&query)
                        || p.version_id.to_lowercase().contains(&query)
                })
                .filter(|(_, p)| match self.library_filter {
                    LibraryFilter::All => true,
                    LibraryFilter::Modded => p.loader.is_modded(),
                    LibraryFilter::Vanilla => !p.loader.is_modded(),
                })
                .map(|(i, p)| (i, p.clone()))
                .collect();

            match self.library_sort {
                LibrarySort::Recent => {
                    visible.sort_by(|(_, a), (_, b)| {
                        b.pinned
                            .cmp(&a.pinned)
                            .then_with(|| b.last_played.cmp(&a.last_played))
                    });
                }
                LibrarySort::Name => {
                    visible.sort_by(|(_, a), (_, b)| {
                        b.pinned
                            .cmp(&a.pinned)
                            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                    });
                }
                LibrarySort::Playtime => {
                    visible.sort_by(|(_, a), (_, b)| {
                        b.pinned
                            .cmp(&a.pinned)
                            .then_with(|| b.playtime_secs.cmp(&a.playtime_secs))
                    });
                }
            }

            if visible.is_empty() {
                card_frame(false, 0.0).show(ui, |ui| {
                    let inner_w = (usable_w - 22.0).max(110.0);
                    ui.set_min_width(inner_w);
                    ui.set_max_width(inner_w);
                    ui_wrap_row(ui, 30.0, |ui| {
                        ui.label(
                            egui::RichText::new(tr(lang, "lib.no_match"))
                                .color(TEXT_SECONDARY),
                        );
                        if ui_button(
                            ui,
                            "lib_clear_filters",
                            tr(self.settings.language, "btn.reset_filters"),
                            ButtonVariant::Secondary,
                            true,
                            30.0,
                        )
                        .clicked()
                        {
                            self.library_search.clear();
                            self.library_filter = LibraryFilter::All;
                        }
                    });
                });
                return;
            }

            if self.library_view == LibraryViewMode::List {
                let list_inner_w = (usable_w - 22.0).max(150.0);
                for (idx, p) in &visible {
                    let selected = *idx == self.selected_idx;
                    let is_running = running_id.as_deref() == Some(p.id.as_str())
                        && self.game_handle.is_some();
                    let mod_items =
                        list_content_items(&instance_dir(&self.paths, p).join("mods"));
                    let enabled_mods = mod_items.iter().filter(|m| m.enabled).count();
                    let total_mods = mod_items.len();

                    let row_id = ui.make_persistent_id(("lib_list_hover", &p.id));
                    animated_card(ui, row_id, selected, |ui| {
                        ui.set_min_width(list_inner_w);
                        ui.set_max_width(list_inner_w);
                        ui_toolbar_row(ui, 38.0, |ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            let (av_r, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                            draw_instance_avatar(
                                ui.painter(),
                                av_r,
                                &p.name,
                                p.loader.kind(),
                                selected,
                                is_running,
                            );
                            let mid_w = (list_inner_w - 290.0).max(90.0);
                            let info_resp = ui
                                .allocate_ui_with_layout(
                                    Vec2::new(mid_w, 36.0),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        ui_toolbar_row(ui, 18.0, |ui| {
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(&p.name)
                                                        .strong()
                                                        .size(14.0)
                                                        .color(PURE_WHITE),
                                                )
                                                .truncate(),
                                            );
                                            badge(ui, &p.version_id, false);
                                            loader_badge(ui, p.loader.kind(), p.loader.short_name());
                                            if selected {
                                                badge(ui, tr(lang, "acc.active"), true);
                                            }
                                        });
                                        let mut meta = format!("{enabled_mods}/{total_mods} {}", tr(lang, "lib.mods_count"));
                                        if p.playtime_secs > 0 {
                                            meta.push_str(&format!(
                                                " • {} {}",
                                                format_playtime(p.playtime_secs),
                                                tr(lang, "lib.played")
                                            ));
                                        }
                                        if let Some(rel) = format_relative_time(p.last_played.as_deref()) {
                                            meta.push_str(&format!(" • {rel}"));
                                        }
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(meta).small().color(TEXT_MUTED),
                                            )
                                            .truncate(),
                                        );
                                    },
                                )
                                .response
                                .interact(Sense::click());
                            if info_resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if info_resp.clicked() {
                                action_select = Some(*idx);
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                if can_delete
                                    && ui_icon_button(
                                        ui,
                                        ("list_del", &p.id),
                                        Icon::Trash,
                                        "Delete instance",
                                        ButtonVariant::Danger,
                                        true,
                                        28.0,
                                    )
                                    .clicked()
                                {
                                    action_delete = Some(*idx);
                                }
                                if ui_icon_button(
                                    ui,
                                    ("list_cfg", &p.id),
                                    Icon::Sliders,
                                    "Instance settings",
                                    ButtonVariant::Secondary,
                                    true,
                                    28.0,
                                )
                                .clicked()
                                {
                                    action_open = Some(*idx);
                                }
                                if ui_icon_button(
                                    ui,
                                    ("list_mods", &p.id),
                                    Icon::Layers,
                                    "Manage content",
                                    ButtonVariant::Subtle,
                                    true,
                                    28.0,
                                )
                                .clicked()
                                {
                                    action_mods = Some(*idx);
                                }
                                if ui_icon_button(
                                    ui,
                                    ("list_pin", &p.id),
                                    Icon::Star,
                                    if p.pinned { "Unpin instance" } else { "Pin instance" },
                                    if p.pinned {
                                        ButtonVariant::Primary
                                    } else {
                                        ButtonVariant::Subtle
                                    },
                                    true,
                                    28.0,
                                )
                                .clicked()
                                {
                                    action_toggle_pin = Some(*idx);
                                }
                                if ui_button_icon(
                                    ui,
                                    ("list_play", &p.id),
                                    Some(Icon::Play),
                                    tr(lang, "btn.play"),
                                    if selected {
                                        ButtonVariant::Primary
                                    } else {
                                        ButtonVariant::Secondary
                                    },
                                    !self.is_busy && self.game_handle.is_none(),
                                    28.0,
                                )
                                .clicked()
                                {
                                    action_play = Some(*idx);
                                }
                            });
                        });
                    });
                    ui.add_space(6.0);
                }
            } else {
                for chunk in visible.chunks(cols) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (idx, p) in chunk {
                        let selected = *idx == self.selected_idx;
                        let is_running = running_id.as_deref() == Some(p.id.as_str())
                            && self.game_handle.is_some();
                        let mod_items =
                            list_content_items(&instance_dir(&self.paths, p).join("mods"));
                        let enabled_mods = mod_items.iter().filter(|m| m.enabled).count();
                        let total_mods = mod_items.len();

                        let inner_w = (card_w - 22.0).max(110.0);
                        ui.allocate_ui_with_layout(
                            Vec2::new(card_w, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                let card_id = ui.make_persistent_id(("lib_card_hover", &p.id));
                                animated_card(ui, card_id, selected, |ui| {
                                    ui.set_min_width(inner_w);
                                    ui.set_max_width(inner_w);

                                    let hdr_resp = ui
                                        .horizontal(|ui| {
                                            let (icon_r, _) = ui.allocate_exact_size(
                                                Vec2::splat(44.0),
                                                Sense::hover(),
                                            );
                                            draw_instance_avatar(
                                                ui.painter(),
                                                icon_r,
                                                &p.name,
                                                p.loader.kind(),
                                                selected,
                                                is_running,
                                            );

                                            let meta_w = (inner_w - 56.0).max(60.0);
                                            ui.vertical(|ui| {
                                                ui.set_max_width(meta_w);
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(&p.name)
                                                            .strong()
                                                            .size(14.5)
                                                            .color(PURE_WHITE),
                                                    )
                                                    .truncate(),
                                                );
                                                ui_toolbar_row(ui, 20.0, |ui| {
                                                    ui.spacing_mut().item_spacing.x = 5.0;
                                                    badge(ui, &p.version_id, false);
                                                    loader_badge(
                                                        ui,
                                                        p.loader.kind(),
                                                        p.loader.short_name(),
                                                    );
                                                    if selected {
                                                        badge(ui, tr(lang, "acc.active"), true);
                                                    }
                                                });
                                                let mut sub = if is_running {
                                                    tr(lang, "lib.running").to_string()
                                                } else if total_mods == 0 {
                                                    format!("0 {}", tr(lang, "lib.mods_count"))
                                                } else if enabled_mods == total_mods {
                                                    format!("{total_mods} {}", tr(lang, "lib.mods_count"))
                                                } else {
                                                    format!("{enabled_mods}/{total_mods} {}", tr(lang, "lib.mods_count"))
                                                };
                                                if !is_running {
                                                    if p.playtime_secs > 0 {
                                                        sub.push_str(&format!(
                                                            " • {}",
                                                            format_playtime(p.playtime_secs)
                                                        ));
                                                    }
                                                    if let Some(rel) =
                                                        format_relative_time(p.last_played.as_deref())
                                                    {
                                                        sub.push_str(&format!(" • {rel}"));
                                                    }
                                                }
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(sub)
                                                            .small()
                                                            .color(if is_running {
                                                                accent()
                                                            } else {
                                                                TEXT_MUTED
                                                            }),
                                                    )
                                                    .truncate(),
                                                );
                                            });
                                        })
                                        .response
                                        .interact(Sense::click());
                                    if hdr_resp.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if hdr_resp.clicked() {
                                        action_select = Some(*idx);
                                    }

                                    ui.add_space(6.0);
                                    ui.separator();
                                    ui.add_space(4.0);

                                    ui_toolbar_row(ui, 28.0, |ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;
                                        let play_variant = if selected {
                                            ButtonVariant::Primary
                                        } else {
                                            ButtonVariant::Secondary
                                        };
                                        let play_lbl = if inner_w < 240.0 {
                                            ""
                                        } else {
                                            tr(lang, "btn.play")
                                        };
                                        let mods_lbl = if inner_w < 200.0 { "" } else { tr(lang, "content.mods") };
                                        if ui_button_icon(
                                            ui,
                                            ("card_play", &p.id),
                                            Some(Icon::Play),
                                            play_lbl,
                                            play_variant,
                                            !self.is_busy && self.game_handle.is_none(),
                                            28.0,
                                        )
                                        .clicked()
                                        {
                                            action_play = Some(*idx);
                                        }
                                        if ui_button_icon(
                                            ui,
                                            ("card_mods", &p.id),
                                            Some(Icon::Layers),
                                            mods_lbl,
                                            ButtonVariant::Subtle,
                                            true,
                                            28.0,
                                        )
                                        .on_hover_text("Manage mods")
                                        .clicked()
                                        {
                                            action_mods = Some(*idx);
                                        }

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.spacing_mut().item_spacing.x = 5.0;
                                                if can_delete
                                                    && ui_icon_button(
                                                        ui,
                                                        ("card_del", &p.id),
                                                        Icon::Trash,
                                                        "Delete instance",
                                                        ButtonVariant::Danger,
                                                        true,
                                                        28.0,
                                                    )
                                                    .clicked()
                                                {
                                                    action_delete = Some(*idx);
                                                }
                                                if ui_icon_button(
                                                    ui,
                                                    ("card_folder", &p.id),
                                                    Icon::Folder,
                                                    "Open instance folder",
                                                    ButtonVariant::Subtle,
                                                    true,
                                                    28.0,
                                                )
                                                .clicked()
                                                {
                                                    open_in_file_manager(&instance_dir(
                                                        &self.paths,
                                                        p,
                                                    ));
                                                }
                                                if ui_icon_button(
                                                    ui,
                                                    ("card_dup", &p.id),
                                                    Icon::Duplicate,
                                                    "Duplicate instance",
                                                    ButtonVariant::Subtle,
                                                    true,
                                                    28.0,
                                                )
                                                .clicked()
                                                {
                                                    action_duplicate = Some(*idx);
                                                }
                                                if ui_icon_button(
                                                    ui,
                                                    ("card_cfg", &p.id),
                                                    Icon::Sliders,
                                                    "Instance settings",
                                                    ButtonVariant::Secondary,
                                                    true,
                                                    28.0,
                                                )
                                                .clicked()
                                                {
                                                    action_open = Some(*idx);
                                                }
                                                if ui_icon_button(
                                                    ui,
                                                    ("card_pin", &p.id),
                                                    Icon::Star,
                                                    if p.pinned {
                                                        "Unpin instance"
                                                    } else {
                                                        "Pin instance"
                                                    },
                                                    if p.pinned {
                                                        ButtonVariant::Primary
                                                    } else {
                                                        ButtonVariant::Subtle
                                                    },
                                                    true,
                                                    28.0,
                                                )
                                                .clicked()
                                                {
                                                    action_toggle_pin = Some(*idx);
                                                }
                                            },
                                        );
                                    });
                                });
                            },
                        );
                    }
                });
                ui.add_space(gap);
                }
            }
        });

        if let Some(idx) = action_toggle_pin {
            if let Some(p) = self.profiles.get_mut(idx) {
                p.pinned = !p.pinned;
                let now_pinned = p.pinned;
                let name = p.name.clone();
                let _ = save_profiles(&self.paths, &self.profiles);
                self.show_toast(
                    ui.ctx(),
                    if now_pinned {
                        format!("Pinned '{name}'")
                    } else {
                        format!("Unpinned '{name}'")
                    },
                );
            }
        }

        if let Some(idx) = action_select {
            self.select_profile(idx);
        }
        if let Some(idx) = action_open {
            self.select_profile(idx);
            self.instance_tab = InstanceSubTab::Settings;
            self.switch_page(ui.ctx(), NavPage::Instance);
        }
        if let Some(idx) = action_mods {
            self.select_profile(idx);
            self.instance_tab = InstanceSubTab::Content;
            self.switch_page(ui.ctx(), NavPage::Instance);
        }
        if let Some(idx) = action_discover {
            self.select_profile(idx);
            self.switch_page(ui.ctx(), NavPage::Discover);
            self.trigger_modrinth_search(false);
        }
        if action_console {
            self.switch_page(ui.ctx(), NavPage::Console);
        }
        if let Some(idx) = action_duplicate {
            self.duplicate_profile(ui.ctx(), idx);
        }
        if let Some(idx) = action_play {
            self.select_profile(idx);
            self.start_launch();
        }
        if let Some(idx) = action_delete {
            self.confirm_delete_idx = Some(idx);
        }
    }

    fn render_instance_page(&mut self, ui: &mut egui::Ui) {
        if self.profiles.is_empty() {
            ui.label("No instances found.");
            return;
        }

        let profile = self.profiles[self.selected_idx].clone();
        let wide = ui.available_width() >= 760.0;
        if wide {
            ui_toolbar_row(ui, 30.0, |ui| {
                let (av_r, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                draw_instance_avatar(
                    ui.painter(),
                    av_r,
                    &profile.name,
                    profile.loader.kind(),
                    true,
                    self.running_profile_id.as_deref() == Some(profile.id.as_str())
                        && self.game_handle.is_some(),
                );
                ui.heading(egui::RichText::new(&profile.name).size(21.0).color(PURE_WHITE));
                badge_sized(ui, &profile.version_id, false, 30.0, None);
                loader_badge_sized(ui, profile.loader.kind(), &profile.loader.label(), 30.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if ui_button_icon(
                        ui,
                        "inst_mr_btn",
                        Some(Icon::Compass),
                        tr(self.settings.language, "nav.discover"),
                        ButtonVariant::Primary,
                        true,
                        30.0,
                    )
                    .clicked()
                    {
                        self.switch_page(ui.ctx(), NavPage::Discover);
                    }
                    if ui_button_icon(
                        ui,
                        "inst_dir_btn",
                        Some(Icon::Folder),
                        tr(self.settings.language, "btn.open_folder"),
                        ButtonVariant::Secondary,
                        true,
                        30.0,
                    )
                    .clicked()
                    {
                        open_in_file_manager(&instance_dir(&self.paths, &profile));
                    }
                    if ui_button_icon(
                        ui,
                        "inst_export_mrpack_btn",
                        Some(Icon::Download),
                        tr(self.settings.language, "btn.export_mrpack"),
                        ButtonVariant::Secondary,
                        true,
                        30.0,
                    )
                    .clicked()
                    {
                        match export_instance_mrpack(&self.paths, &profile) {
                            Ok(out_path) => {
                                self.show_toast(
                                    ui.ctx(),
                                    format!("Exported: {}", out_path.display()),
                                );
                                if let Some(parent) = out_path.parent() {
                                    open_in_file_manager(parent);
                                }
                            }
                            Err(e) => {
                                self.show_toast(ui.ctx(), format!("Export failed: {e}"));
                            }
                        }
                    }
                });
            });
        } else {
            ui_wrap_row(ui, 28.0, |ui| {
                ui.heading(egui::RichText::new(&profile.name).size(21.0).color(PURE_WHITE));
                badge(ui, &profile.version_id, false);
                loader_badge(ui, profile.loader.kind(), &profile.loader.label());
            });
            ui.add_space(4.0);
            ui_wrap_row(ui, 30.0, |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if ui_button_icon(
                    ui,
                    "inst_mr_btn_narrow",
                    Some(Icon::Compass),
                    tr(self.settings.language, "nav.discover"),
                    ButtonVariant::Primary,
                    true,
                    30.0,
                )
                .clicked()
                {
                    self.switch_page(ui.ctx(), NavPage::Discover);
                }
                if ui_button_icon(
                    ui,
                    "inst_dir_btn_narrow",
                    Some(Icon::Folder),
                    tr(self.settings.language, "btn.folder"),
                    ButtonVariant::Secondary,
                    true,
                    30.0,
                )
                .clicked()
                {
                    open_in_file_manager(&instance_dir(&self.paths, &profile));
                }
                if ui_button_icon(
                    ui,
                    "inst_export_mrpack_btn_narrow",
                    Some(Icon::Download),
                    tr(self.settings.language, "btn.export_mrpack"),
                    ButtonVariant::Secondary,
                    true,
                    30.0,
                )
                .clicked()
                {
                    match export_instance_mrpack(&self.paths, &profile) {
                        Ok(out_path) => {
                            self.show_toast(
                                ui.ctx(),
                                format!("Exported: {}", out_path.display()),
                            );
                            if let Some(parent) = out_path.parent() {
                                open_in_file_manager(parent);
                            }
                        }
                        Err(e) => {
                            self.show_toast(ui.ctx(), format!("Export failed: {e}"));
                        }
                    }
                }
            });
        }

        ui.add_space(6.0);

        let (total_mods, enabled_mods) = self.profile_mod_counts(&profile);
        let inst_path = instance_dir(&self.paths, &profile);
        let worlds_cnt = list_worlds(&inst_path).len();
        let shots_cnt = list_screenshots(&inst_path).len();
        let mut active_rect = Rect::NOTHING;
        ui_wrap_row(ui, 32.0, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 5.0);
            for tab in InstanceSubTab::ALL {
                let is_active = self.instance_tab == tab;
                let hover_id = ui.make_persistent_id(("inst_subtab_hov", tab.label()));
                let prev_hov = ui.ctx().data(|d| d.get_temp::<bool>(hover_id)).unwrap_or(false);
                let hov_t = ui.ctx().animate_bool_with_time(hover_id, prev_hov, 0.11);
                if hov_t > 0.01 && hov_t < 0.99 {
                    ui.ctx().request_repaint();
                }
                let bg = if is_active {
                    bg_card_active()
                } else {
                    lerp_color(Color32::TRANSPARENT, BG_CARD_HOVER, hov_t)
                };
                let border = if is_active {
                    with_alpha(accent(), 130)
                } else if hov_t > 0.05 {
                    with_alpha(BORDER_STRONG, (hov_t * 140.0) as u8)
                } else {
                    Color32::TRANSPARENT
                };
                let tab_icon = match tab {
                    InstanceSubTab::Content => Icon::Layers,
                    InstanceSubTab::Worlds => Icon::Grid,
                    InstanceSubTab::Screenshots => Icon::External,
                    InstanceSubTab::Settings => Icon::Sliders,
                };
                let count_tag: Option<String> = match tab {
                    InstanceSubTab::Content if profile.loader.is_modded() => {
                        if total_mods > 0 {
                            Some(format!("{enabled_mods}/{total_mods}"))
                        } else {
                            Some("0".to_string())
                        }
                    }
                    InstanceSubTab::Worlds if worlds_cnt > 0 => Some(format!("{worlds_cnt}")),
                    InstanceSubTab::Screenshots if shots_cnt > 0 => Some(format!("{shots_cnt}")),
                    _ => None,
                };
                let icon_col = if is_active {
                    accent()
                } else {
                    lerp_color(TEXT_MUTED, TEXT_PRIMARY, hov_t)
                };
                let text_col = if is_active {
                    PURE_WHITE
                } else {
                    lerp_color(TEXT_SECONDARY, PURE_WHITE, hov_t)
                };

                let frame_resp = egui::Frame::none()
                    .fill(bg)
                    .stroke(Stroke::new(1.0_f32, border))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::symmetric(10.0, 5.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let (ic_r, _) =
                                ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                            draw_icon(ui.painter(), ic_r, tab_icon, icon_col);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(tab.translated_label(self.settings.language))
                                        .strong()
                                        .color(text_col),
                                )
                                .selectable(false),
                            );
                            if let Some(tag) = count_tag {
                                egui::Frame::none()
                                    .fill(if is_active { accent_dim() } else { BG_CARD })
                                    .rounding(Rounding::same(6.0))
                                    .inner_margin(Margin::symmetric(5.0, 1.0))
                                    .show(ui, |ui| {
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(tag).size(10.5).color(
                                                    if is_active {
                                                        Color32::from_rgb(162, 248, 198)
                                                    } else {
                                                        TEXT_MUTED
                                                    },
                                                ),
                                            )
                                            .selectable(false),
                                        );
                                    });
                            }
                        });
                    });
                let hov_now = frame_resp.response.hovered()
                    || ui.rect_contains_pointer(frame_resp.response.rect);
                ui.ctx().data_mut(|d| d.insert_temp(hover_id, hov_now));
                let resp = frame_resp.response.interact(Sense::click());
                if resp.hovered() || hov_now {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if is_active {
                    active_rect = resp.rect;
                }
                if resp.clicked() {
                    self.instance_tab = tab;
                }
            }
        });
        if active_rect != Rect::NOTHING {
            let ctx = ui.ctx();
            let anim_x = ctx.animate_value_with_time(
                ui.make_persistent_id("inst_subtab_x"),
                active_rect.left(),
                0.15,
            );
            let anim_w = ctx.animate_value_with_time(
                ui.make_persistent_id("inst_subtab_w"),
                active_rect.width(),
                0.15,
            );
            if (anim_x - active_rect.left()).abs() > 0.4
                || (anim_w - active_rect.width()).abs() > 0.4
            {
                ctx.request_repaint();
            }
            ui.painter().rect_filled(
                Rect::from_min_size(
                    Pos2::new(anim_x + 8.0, active_rect.bottom() + 1.0),
                    Vec2::new((anim_w - 16.0).max(12.0), 2.0),
                ),
                Rounding::same(1.0),
                accent(),
            );
        }
        ui.add_space(2.0);
        ui.separator();
        ui.add_space(6.0);

        match self.instance_tab {
            InstanceSubTab::Content => self.render_content_manager(ui, &profile),
            InstanceSubTab::Worlds => self.render_worlds_tab(ui, &profile),
            InstanceSubTab::Screenshots => self.render_screenshots_tab(ui, &profile),
            InstanceSubTab::Settings => self.render_instance_settings_tab(ui),
        }
    }

    fn render_worlds_tab(&mut self, ui: &mut egui::Ui, profile: &Profile) {
        let inst = instance_dir(&self.paths, profile);
        let saves_dir = inst.join("saves");
        let worlds = list_worlds(&inst);
        ui_toolbar_row(ui, 30.0, |ui| {
            ui.label(
                egui::RichText::new(format!("{} {}", worlds.len(), tr(self.settings.language, "inst.saved_worlds")))
                    .small()
                    .color(TEXT_SECONDARY),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui_button_icon(
                    ui,
                    "worlds_open_dir",
                    Some(Icon::Folder),
                    tr(self.settings.language, "btn.open_saves"),
                    ButtonVariant::Secondary,
                    true,
                    28.0,
                )
                .clicked()
                {
                    let _ = std::fs::create_dir_all(&saves_dir);
                    open_in_file_manager(&saves_dir);
                }
            });
        });
        ui.add_space(6.0);
        bounded_scroll(ui, "instance_worlds_scroll", |ui| {
            let card_inner_w = (ui.available_width() - 32.0).max(140.0);
            if worlds.is_empty() {
                card_frame(false, 0.0).show(ui, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);
                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "inst.no_worlds"))
                            .color(TEXT_SECONDARY),
                    );
                });
                return;
            }
            for w in worlds {
                let card_id = ui.make_persistent_id(("world_card", &profile.id, &w.folder_name));
                animated_card(ui, card_id, false, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);
                    ui_toolbar_row(ui, 32.0, |ui| {
                        ui.vertical(|ui| {
                            ui.strong(egui::RichText::new(&w.display_name).color(PURE_WHITE));
                            ui.label(
                                egui::RichText::new(format!(
                                    "saves/{} • {}",
                                    w.folder_name,
                                    format_size(w.size_bytes)
                                ))
                                .small()
                                .color(TEXT_MUTED),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui_button_icon(
                                ui,
                                ("world_open", &w.folder_name),
                                Some(Icon::Folder),
                                tr(self.settings.language, "btn.open"),
                                ButtonVariant::Subtle,
                               true,
                                28.0,
                            )
                            .clicked()
                            {
                                open_in_file_manager(&w.path);
                            }
                        });
                    });
                });
                ui.add_space(5.0);
            }
        });
    }

    fn render_screenshots_tab(&mut self, ui: &mut egui::Ui, profile: &Profile) {
        let inst = instance_dir(&self.paths, profile);
        let shots_dir = inst.join("screenshots");
        let shots = list_screenshots(&inst);
        ui_toolbar_row(ui, 30.0, |ui| {
            ui.label(
                egui::RichText::new(format!("{} screenshot(s)", shots.len()))
                    .small()
                    .color(TEXT_SECONDARY),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui_button_icon(
                    ui,
                    "shots_open_dir",
                    Some(Icon::Folder),
                    tr(self.settings.language, "btn.open_shots"),
                    ButtonVariant::Secondary,
                    true,
                    28.0,
                )
                .clicked()
                {
                    let _ = std::fs::create_dir_all(&shots_dir);
                    open_in_file_manager(&shots_dir);
                }
            });
        });
        ui.add_space(6.0);
        bounded_scroll(ui, "instance_shots_scroll", |ui| {
            let card_inner_w = (ui.available_width() - 32.0).max(140.0);
            if shots.is_empty() {
                card_frame(false, 0.0).show(ui, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);
                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "inst.no_screenshots"))
                            .color(TEXT_SECONDARY),
                    );
                });
                return;
            }
            for shot in shots {
                let card_id = ui.make_persistent_id(("shot_card", &profile.id, &shot.filename));
                animated_card(ui, card_id, false, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);
                    ui_toolbar_row(ui, 30.0, |ui| {
                        ui.vertical(|ui| {
                            ui.strong(egui::RichText::new(&shot.filename).color(PURE_WHITE));
                            ui.label(
                                egui::RichText::new(format_size(shot.size_bytes))
                                    .small()
                                    .color(TEXT_MUTED),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui_button_icon(
                                ui,
                                ("shot_open", &shot.filename),
                                Some(Icon::External),
                                tr(self.settings.language, "btn.open_image"),
                                ButtonVariant::Subtle,
                                true,
                                28.0,
                            )
                            .clicked()
                            {
                                open_in_file_manager(&shot.path);
                            }
                        });
                    });
                });
                ui.add_space(4.0);
            }
        });
    }

    fn render_content_manager(&mut self, ui: &mut egui::Ui, profile: &Profile) {
        let inst = instance_dir(&self.paths, profile);
        let folder_name = self.content_folder.dir_name();
        let target_dir = inst.join(folder_name);
        let items = list_content_items(&target_dir);
        let total_count = items.len();
        let enabled_count = items.iter().filter(|i| i.enabled).count();
        let disabled_count = total_count.saturating_sub(enabled_count);

        // Vanilla mods notice banner
        if !profile.loader.is_modded() && self.content_folder == ContentFolder::Mods {
            ui_alert_banner(
                ui,
                AlertKind::Info,
                "This instance is using Vanilla. Switch to Fabric, Quilt, Forge, or NeoForge in Instance Settings to load mods.",
                ui.available_width(),
            );
            ui.add_space(6.0);
        }

        let mut go_discover = false;

        // Folder switcher
        ui_wrap_row(ui, 30.0, |ui| {
            for f in ContentFolder::ALL {
                let count = list_content_items(&inst.join(f.dir_name())).len();
                let lbl = format!("{} ({count})", f.translated_label(self.settings.language));
                let ic = match f {
                    ContentFolder::Mods => Icon::Layers,
                    ContentFolder::Resourcepacks => Icon::Grid,
                    ContentFolder::Shaderpacks => Icon::Sparkle,
                };
                if ui_pill_tab_icon(
                    ui,
                    ("folder_tab", f.dir_name()),
                    Some(ic),
                    &lbl,
                    self.content_folder == f,
                )
                .clicked()
                {
                    self.content_folder = f;
                }
            }
            if ui_button_icon(
                ui,
                "content_open_dir_btn",
                Some(Icon::Folder),
                tr(self.settings.language, "btn.open_folder"),
                ButtonVariant::Subtle,
                true,
                28.0,
            )
            .clicked()
            {
                let _ = std::fs::create_dir_all(&target_dir);
                open_in_file_manager(&target_dir);
            }
            if ui_button_icon(
                ui,
                "content_get_more_btn",
                Some(Icon::Compass),
                tr(self.settings.language, "content.get_more"),
                ButtonVariant::Secondary,
                true,
                28.0,
            )
            .clicked()
            {
                go_discover = true;
            }
            if !self.mr_installed_cache.is_empty()
                && ui_button_icon(
                    ui,
                    "content_check_updates_btn",
                    Some(Icon::Refresh),
                    if self.mr_installing {
                        tr(self.settings.language, "content.checking")
                    } else {
                        tr(self.settings.language, "btn.check_updates")
                    },
                    ButtonVariant::Secondary,
                    !self.mr_installing,
                    28.0,
                )
                .clicked()
            {
                self.mr_installing = true;
                self.progress = 0.08;
                self.status_text = "Checking mod updates...".to_string();
                let paths = self.paths.clone();
                let prof = profile.clone();
                let tx = self.tx.clone();
                thread::spawn(move || {
                    let res = update_all_installed_projects(&paths, &prof, &tx);
                    let _ = tx.send(WorkerEvent::ModUpdatesFinished(res));
                });
            }
        });

        ui.add_space(6.0);
        let avail_w = ui.available_width();
        ui_wrap_row(ui, 30.0, |ui| {
            ui_search_input(
                ui,
                "content_search_input",
                &mut self.content_search,
                tr(self.settings.language, "content.filter"),
                (avail_w * 0.36).clamp(115.0, 230.0),
            );
            if ui_pill_tab(
                ui,
                "state_all",
                &format!("{} ({total_count})", tr(self.settings.language, "lib.all")),
                self.content_state_filter == ContentStateFilter::All,
            )
            .clicked()
            {
                self.content_state_filter = ContentStateFilter::All;
            }
            if ui_pill_tab(
                ui,
                "state_on",
                &format!("{} ({enabled_count})", tr(self.settings.language, "content.enabled")),
                self.content_state_filter == ContentStateFilter::Enabled,
            )
            .clicked()
            {
                self.content_state_filter = ContentStateFilter::Enabled;
            }
            if ui_pill_tab(
                ui,
                "state_off",
                &format!("{} ({disabled_count})", tr(self.settings.language, "content.disabled")),
                self.content_state_filter == ContentStateFilter::Disabled,
            )
            .clicked()
            {
                self.content_state_filter = ContentStateFilter::Disabled;
            }
            if disabled_count > 0
                && ui_button(
                    ui,
                    "bulk_enable_btn",
                    tr(self.settings.language, "btn.enable_all"),
                    ButtonVariant::Secondary,
                    true,
                    30.0,
                )
                .clicked()
            {
                match set_all_content_enabled(&target_dir, true) {
                    Ok(n) => self.show_toast(ui.ctx(), format!("Enabled: {n}")),
                    Err(e) => self.show_toast(ui.ctx(), e),
                }
            }
            if enabled_count > 0
                && ui_button(
                    ui,
                    "bulk_disable_btn",
                    tr(self.settings.language, "btn.disable_all"),
                    ButtonVariant::Subtle,
                    true,
                    30.0,
                )
                .clicked()
            {
                match set_all_content_enabled(&target_dir, false) {
                    Ok(n) => self.show_toast(ui.ctx(), format!("Disabled: {n}")),
                    Err(e) => self.show_toast(ui.ctx(), e),
                }
            }
        });

        if total_count > 0 {
            let total_bytes: u64 = items.iter().map(|i| i.size_bytes).sum();
            let ratio = enabled_count as f32 / total_count as f32;
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                draw_mini_gauge(ui, "mods_enabled_ratio", ratio, 84.0, 6.0);
                ui.label(
                    egui::RichText::new(format!(
                        "{enabled_count}/{total_count} {} • {} {}",
                        tr(self.settings.language, "content.active"),
                        format_size(total_bytes),
                        tr(self.settings.language, "content.total")
                    ))
                    .small()
                    .color(TEXT_MUTED),
                );
            });
        }

        ui.add_space(6.0);
        let q = self.content_search.trim().to_lowercase();
        let indexed_by_file: HashMap<String, InstalledProject> = self
            .mr_installed_cache
            .values()
            .filter(|p| p.folder == folder_name)
            .map(|p| (p.filename.clone(), p.clone()))
            .collect();
        let pending_icons: Vec<String> = indexed_by_file
            .values()
            .filter_map(|p| p.icon_url.clone())
            .collect();
        for icon_url in pending_icons {
            self.request_icon(&icon_url);
        }

        let filtered: Vec<_> = items
            .into_iter()
            .filter(|item| match self.content_state_filter {
                ContentStateFilter::All => true,
                ContentStateFilter::Enabled => item.enabled,
                ContentStateFilter::Disabled => !item.enabled,
            })
            .filter(|item| {
                if q.is_empty() {
                    return true;
                }
                let title_match = indexed_by_file
                    .get(&item.base_name)
                    .is_some_and(|info| info.title.to_lowercase().contains(&q));
                title_match || item.base_name.to_lowercase().contains(&q)
            })
            .collect();

        let mut toggle_disk_name: Option<String> = None;
        let mut delete_disk_name: Option<(String, String)> = None;

        bounded_scroll(ui, "instance_content_scroll", |ui| {
            let card_outer_w = (ui.available_width() - 10.0).max(150.0);
            let card_inner_w = (card_outer_w - 24.0).max(110.0);

            if filtered.is_empty() {
                card_frame(false, 0.0).show(ui, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);
                    ui_wrap_row(ui, 30.0, |ui| {
                        let msg = if total_count == 0 {
                            format!("{folder_name}/ folder is currently empty.")
                        } else {
                            "No matching files found.".to_string()
                        };
                        ui.label(egui::RichText::new(msg).color(TEXT_SECONDARY));
                        if ui_button_icon(
                            ui,
                            "empty_find_mr_btn",
                            Some(Icon::Compass),
                            tr(self.settings.language, "btn.browse_modrinth"),
                            ButtonVariant::Primary,
                            true,
                            30.0,
                        )
                        .clicked()
                        {
                            go_discover = true;
                        }
                    });
                });
                return;
            }

            for item in &filtered {
                let info = indexed_by_file.get(&item.base_name);
                let display_title = info
                    .map(|i| i.title.clone())
                    .unwrap_or_else(|| item.base_name.clone());

                let card_id =
                    ui.make_persistent_id(("mod_card_hover", folder_name, &item.base_name));
                animated_card(ui, card_id, false, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);
                    let compact_row = card_inner_w < 380.0;
                    let right_pill: Option<(&str, bool)> = if !item.enabled {
                        Some(("Disabled", false))
                    } else {
                        info.map(|inf| (inf.version_number.as_str(), true))
                    };
                    let show_right_pill = !compact_row && right_pill.is_some();
                    let del_reserve = if compact_row {
                        36.0
                    } else if show_right_pill {
                        214.0
                    } else {
                        96.0
                    };
                    let switch_w = 38.0;
                    let icon_px = 30.0;
                    let mid_w = (card_inner_w - switch_w - icon_px - del_reserve - 26.0).max(60.0);

                    ui_toolbar_row(ui, 36.0, |ui| {
                        if mod_toggle_switch(
                            ui,
                            &format!("{}_{}_{}", profile.id, folder_name, item.base_name),
                            item.enabled,
                        )
                        .on_hover_text(if item.enabled {
                            "Disable temporarily (.disabled)"
                        } else {
                            "Enable"
                        })
                        .clicked()
                        {
                            toggle_disk_name = Some(item.disk_name.clone());
                        }

                        let icon_url_opt = info.and_then(|i| i.icon_url.as_ref());
                        let tex_opt = icon_url_opt
                            .and_then(|u| self.mr_icons.get(u))
                            .and_then(|o| o.as_ref());
                        let is_icon_loading = icon_url_opt
                            .is_some_and(|u| self.mr_icons_pending.contains(u.as_str()));
                        let icon_size = Vec2::splat(icon_px);
                        if let Some(tex) = tex_opt {
                            ui.add(
                                egui::Image::new(tex)
                                    .fit_to_exact_size(icon_size)
                                    .rounding(Rounding::same(6.5)),
                            );
                        } else {
                            let (rect, _) = ui.allocate_exact_size(icon_size, Sense::hover());
                            draw_icon_placeholder(
                                ui,
                                rect,
                                6.5,
                                &display_title,
                                is_icon_loading,
                            );
                        }

                        ui.allocate_ui_with_layout(
                            Vec2::new(mid_w, 34.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_min_width(mid_w);
                                ui.set_max_width(mid_w);
                                let mut title_rt = egui::RichText::new(&display_title).strong();
                                if !item.enabled {
                                    title_rt = title_rt.strikethrough().color(TEXT_MUTED);
                                } else {
                                    title_rt = title_rt.color(PURE_WHITE);
                                }
                                ui.add(egui::Label::new(title_rt).truncate());
                                let src_tag = if info.is_some() { "Modrinth" } else { "Local" };
                                let meta = format!(
                                    "{src_tag} • {} • {}",
                                    item.disk_name,
                                    format_size(item.size_bytes)
                                );
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(meta)
                                            .monospace()
                                            .small()
                                            .color(TEXT_MUTED),
                                    )
                                    .truncate(),
                                );
                            },
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let del_clicked = if compact_row {
                                ui_icon_button(
                                    ui,
                                    ("mod_del", &item.disk_name),
                                    Icon::Trash,
                                    &format!("Remove {}", item.disk_name),
                                    ButtonVariant::Danger,
                                    true,
                                    28.0,
                                )
                                .clicked()
                            } else {
                                ui_button_icon(
                                    ui,
                                    ("mod_del", &item.disk_name),
                                    Some(Icon::Trash),
                                    tr(self.settings.language, "btn.remove"),
                                    ButtonVariant::Danger,
                                    true,
                                    28.0,
                                )
                                .on_hover_text(format!("Remove {}", item.disk_name))
                                .clicked()
                            };
                            if del_clicked {
                                delete_disk_name =
                                    Some((item.disk_name.clone(), display_title.clone()));
                            }
                            if show_right_pill {
                                if let Some((pill_txt, highlighted)) = right_pill {
                                    badge_sized(ui, pill_txt, highlighted, 28.0, Some(112.0))
                                        .on_hover_text(pill_txt);
                                }
                            }
                        });
                    });
                });
                ui.add_space(4.0);
            }
        });

        if let Some(name) = toggle_disk_name {
            match toggle_content_file(&target_dir, &name) {
                Ok(now_on) => {
                    let msg = if now_on {
                        format!("Enabled: {}", name.trim_end_matches(".disabled"))
                    } else {
                        format!("Disabled: {name}")
                    };
                    self.show_toast(ui.ctx(), msg);
                }
                Err(e) => self.show_toast(ui.ctx(), e),
            }
        }
        if let Some((disk_name, title)) = delete_disk_name {
            match delete_content_file(&self.paths, profile, folder_name, &disk_name) {
                Ok(()) => {
                    self.mr_installed_cache = load_installed_index(&self.paths, profile);
                    self.show_toast(ui.ctx(), format!("Removed: {title}"));
                }
                Err(e) => self.show_toast(ui.ctx(), e),
            }
        }
        if go_discover {
            self.mr_type = match self.content_folder {
                ContentFolder::Mods | ContentFolder::Shaderpacks => ModrinthProjectType::Mod,
                ContentFolder::Resourcepacks => ModrinthProjectType::Resourcepack,
            };
            self.switch_page(ui.ctx(), NavPage::Discover);
            self.trigger_modrinth_search(false);
        }
    }

    fn render_instance_settings_tab(&mut self, ui: &mut egui::Ui) {
        if self.profiles.is_empty() {
            ui.label("No instances found.");
            return;
        }
        if self.selected_idx >= self.profiles.len() {
            self.selected_idx = 0;
        }
        let profile_ver = self.profiles[self.selected_idx].version_id.clone();
        let loader_kind = self.profiles[self.selected_idx].loader.kind();
        if loader_kind.is_modded() {
            self.request_loader_versions(loader_kind, &profile_ver);
        }

        let mut changed = false;
        let filtered_versions: Vec<String> = self
            .manifest
            .as_ref()
            .map(|m| {
                m.versions
                    .iter()
                    .filter(|v| self.show_snapshots || v.version_type == VersionType::Release)
                    .map(|v| v.id.clone())
                    .collect()
            })
            .unwrap_or_default();

        let loader_list = loader_kind
            .is_modded()
            .then(|| self.loader_cache.get(&(loader_kind, profile_ver)).cloned())
            .flatten();
        let java_installs = self.java_installs.clone();
        let java_scanning = self.java_scanning;
        let mut rescan_java = false;
        let mut trigger_dup = false;
        let mut trigger_del = false;
        let can_delete = self.profiles.len() > 1;
        let paths = self.paths.clone();
        let selected_idx = self.selected_idx;

        bounded_scroll(ui, "profile_tab_scroll", |ui| {
            let profile = &mut self.profiles[selected_idx];
            let card_inner_w = (ui.available_width() - 32.0).max(140.0);
            let field_w = (card_inner_w - 20.0).min(360.0);
            let wide_grid = card_inner_w >= 480.0;

            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_inner_w);
                ui.set_max_width(card_inner_w);
                ui_toolbar_row(ui, 24.0, |ui| {
                    section_header(ui, tr(self.settings.language, "inst.sec_loader"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if can_delete
                            && ui_button_icon(
                                ui,
                                "cfg_del_inst",
                                Some(Icon::Trash),
                                tr(self.settings.language, "btn.delete"),
                                ButtonVariant::Danger,
                                true,
                                24.0,
                            )
                            .clicked()
                        {
                            trigger_del = true;
                        }
                        if ui_button_icon(
                            ui,
                            "cfg_dup_inst",
                            Some(Icon::Duplicate),
                            tr(self.settings.language, "btn.duplicate"),
                            ButtonVariant::Subtle,
                            true,
                            24.0,
                        )
                        .clicked()
                        {
                            trigger_dup = true;
                        }
                        if ui_button_icon(
                            ui,
                            "cfg_open_inst",
                            Some(Icon::Folder),
                            tr(self.settings.language, "btn.folder"),
                            ButtonVariant::Subtle,
                            true,
                            24.0,
                        )
                        .clicked()
                        {
                            open_in_file_manager(&instance_dir(&paths, profile));
                        }
                    });
                });
                ui.add_space(6.0);

                if wide_grid {
                    egui::Grid::new("profile_main_grid")
                        .num_columns(2)
                        .spacing([16.0, 10.0])
                        .show(ui, |ui| {
                            ui.label(tr(self.settings.language, "inst.name"));
                            if ui_text_input(
                                ui,
                                "cfg_inst_name_w",
                                &mut profile.name,
                                "My Instance",
                                (field_w - 90.0).max(160.0),
                                false,
                            )
                            .changed()
                            {
                                changed = true;
                            }
                            ui.end_row();

                            ui.label(tr(self.settings.language, "inst.mc_version"));
                            ui_wrap_row(ui, 30.0, |ui| {
                                egui::ComboBox::from_id_salt("version_combo")
                                    .selected_text(&profile.version_id)
                                    .width(160.0)
                                    .show_ui(ui, |ui| {
                                        if filtered_versions.is_empty() {
                                            ui.label(tr(self.settings.language, "inst.loading_versions"));
                                        } else {
                                            for vid in &filtered_versions {
                                                if ui
                                                    .selectable_value(
                                                        &mut profile.version_id,
                                                        vid.clone(),
                                                        vid,
                                                    )
                                                    .changed()
                                                {
                                                    if let Some(slot) =
                                                        profile.loader.loader_version_mut()
                                                    {
                                                        slot.clear();
                                                    }
                                                    changed = true;
                                                }
                                            }
                                        }
                                    });
                                ui_checkbox(
                                    ui,
                                    "cfg_snapshots_w",
                                    &mut self.show_snapshots,
                                    tr(self.settings.language, "inst.snapshots"),
                                );
                            });
                            ui.end_row();

                            ui.label(tr(self.settings.language, "inst.mod_loader"));
                            if render_loader_kind_pills(ui, "inst_cfg_w", &mut profile.loader) {
                                changed = true;
                            }
                            ui.end_row();

                            let kind = profile.loader.kind();
                            if let Some(loader_version) = profile.loader.loader_version_mut() {
                                ui.label(format!("{} Version:", kind.label()));
                                if render_loader_version_picker(
                                    ui,
                                    "loader_ver_combo",
                                    210.0,
                                    kind,
                                    loader_version,
                                    loader_list.as_deref(),
                                ) {
                                    changed = true;
                                }
                                ui.end_row();
                            }
                        });
                } else {
                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "inst.name"))
                            .small()
                            .color(TEXT_SECONDARY),
                    );
                    if ui_text_input(ui, "cfg_inst_name_n", &mut profile.name, "My Instance", field_w, false)
                        .changed()
                    {
                        changed = true;
                    }
                    ui.add_space(6.0);

                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "inst.mc_version"))
                            .small()
                            .color(TEXT_SECONDARY),
                    );
                    ui_wrap_row(ui, 30.0, |ui| {
                        egui::ComboBox::from_id_salt("version_combo")
                            .selected_text(&profile.version_id)
                            .width(140.0)
                            .show_ui(ui, |ui| {
                                if filtered_versions.is_empty() {
                                    ui.label(tr(self.settings.language, "inst.loading_versions"));
                                } else {
                                    for vid in &filtered_versions {
                                        if ui
                                            .selectable_value(
                                                &mut profile.version_id,
                                                vid.clone(),
                                                vid,
                                            )
                                            .changed()
                                        {
                                            if let Some(slot) = profile.loader.loader_version_mut()
                                            {
                                                slot.clear();
                                            }
                                            changed = true;
                                        }
                                    }
                                }
                            });
                        ui_checkbox(
                            ui,
                            "cfg_snapshots_n",
                            &mut self.show_snapshots,
                            tr(self.settings.language, "inst.snapshots"),
                        );
                    });
                    ui.add_space(6.0);

                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "inst.mod_loader"))
                            .small()
                            .color(TEXT_SECONDARY),
                    );
                    if render_loader_kind_pills(ui, "inst_cfg_n", &mut profile.loader) {
                        changed = true;
                    }

                    let kind = profile.loader.kind();
                    if let Some(loader_version) = profile.loader.loader_version_mut() {
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(format!("{} Version:", kind.label()))
                                .small()
                                .color(TEXT_SECONDARY),
                        );
                        if render_loader_version_picker(
                            ui,
                            "loader_ver_combo",
                            (field_w - 28.0).max(120.0),
                            kind,
                            loader_version,
                            loader_list.as_deref(),
                        ) {
                            changed = true;
                        }
                    }
                }
            });

            ui.add_space(10.0);

            // Memory Allocation Card
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_inner_w);
                ui.set_max_width(card_inner_w);
                ui_toolbar_row(ui, 24.0, |ui| {
                    section_header(ui, tr(self.settings.language, "inst.sec_ram"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        badge(
                            ui,
                            &format!("-Xms{}M  -Xmx{}M", profile.min_ram_mb, profile.max_ram_mb),
                            true,
                        );
                    });
                });
                ui.add_space(6.0);
                draw_ram_allocation_bar(
                    ui,
                    profile.min_ram_mb,
                    profile.max_ram_mb,
                    (card_inner_w - 16.0).clamp(120.0, 520.0),
                );
                ui.add_space(8.0);

                let slider_w = (field_w - 90.0).max(100.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(tr(self.settings.language, "inst.max_ram")).small().color(TEXT_SECONDARY));
                    if ui_ram_slider(ui, &mut profile.max_ram_mb, 512..=16384, slider_w).changed()
                    {
                        if profile.min_ram_mb > profile.max_ram_mb {
                            profile.min_ram_mb = profile.max_ram_mb;
                        }
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(tr(self.settings.language, "inst.min_ram")).small().color(TEXT_SECONDARY));
                    let max_cap = profile.max_ram_mb.max(256);
                    if ui_ram_slider(ui, &mut profile.min_ram_mb, 256..=max_cap, slider_w).changed()
                    {
                        changed = true;
                    }
                });

                ui.add_space(4.0);
                ui_wrap_row(ui, 28.0, |ui| {
                    for preset in [2048_u32, 4096, 6144, 8192, 12288] {
                        if ui_pill_tab(
                            ui,
                            ("ram_preset", preset),
                            &format!("{} GB", preset / 1024),
                            profile.max_ram_mb == preset,
                        )
                        .clicked()
                        {
                            profile.max_ram_mb = preset;
                            if profile.min_ram_mb > preset {
                                profile.min_ram_mb = preset;
                            }
                            changed = true;
                        }
                    }
                });
                if profile.max_ram_mb >= 12288 {
                    ui.add_space(6.0);
                    ui_alert_banner(
                        ui,
                        AlertKind::Warning,
                        "Allocating 12+ GB RAM can cause garbage collection pauses unless using ZGC or tuned G1GC flags.",
                        card_inner_w,
                    );
                }
            });

            ui.add_space(10.0);

            // Java & JVM Card
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_inner_w);
                ui.set_max_width(card_inner_w);
                ui_toolbar_row(ui, 26.0, |ui| {
                    section_header(ui, tr(self.settings.language, "inst.sec_java"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if java_scanning {
                            mono_spinner(ui, 14.0);
                        } else if ui_button_icon(
                            ui,
                            "refresh_java_btn",
                            Some(Icon::Refresh),
                            tr(self.settings.language, "inst.rescan_java"),
                            ButtonVariant::Secondary,
                            true,
                            24.0,
                        )
                        .clicked()
                        {
                            rescan_java = true;
                        }
                    });
                });
                ui.add_space(6.0);

                if !java_scanning && java_installs.is_empty() && profile.java_path.trim().is_empty() {
                    ui_alert_banner(
                        ui,
                        AlertKind::Warning,
                        "No Java runtime was detected automatically. Install OpenJDK or enter a custom java binary path below.",
                        card_inner_w,
                    );
                    ui.add_space(6.0);
                }

                ui.label(
                    egui::RichText::new(tr(self.settings.language, "inst.detected_runtimes"))
                        .small()
                        .color(TEXT_SECONDARY),
                );
                ui_wrap_row(ui, 28.0, |ui| {
                    let is_auto = profile.java_path.trim().is_empty();
                    if ui_pill_tab(ui, "java_auto_pill", tr(self.settings.language, "inst.auto"), is_auto).clicked() {
                        profile.java_path.clear();
                        changed = true;
                    }
                    for j in &java_installs {
                        let j_str = j.path.to_string_lossy().to_string();
                        let active = profile.java_path == j_str;
                        if ui_pill_tab(
                            ui,
                            ("java_pill", j_str.as_str()),
                            &format!("Java {}", j.major),
                            active,
                        )
                        .on_hover_text(&j_str)
                        .clicked()
                        {
                            profile.java_path = j_str;
                            changed = true;
                        }
                    }
                });
                ui.add_space(4.0);
                let java_placeholder = if cfg!(windows) {
                    r"Auto (or C:\path\to\javaw.exe)"
                } else {
                    "Auto (or /usr/bin/java)"
                };
                if ui_text_input(
                    ui,
                    "cfg_java_path",
                    &mut profile.java_path,
                    java_placeholder,
                    field_w,
                    false,
                )
                .changed()
                {
                    changed = true;
                }

                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(tr(self.settings.language, "inst.jvm_args"))
                        .small()
                        .color(TEXT_SECONDARY),
                );
                if ui_text_input(
                    ui,
                    "cfg_jvm_args",
                    &mut profile.jvm_args,
                    "-XX:+UseG1GC",
                    field_w,
                    false,
                )
                .changed()
                {
                    changed = true;
                }
                ui.add_space(4.0);
                ui_wrap_row(ui, 26.0, |ui| {
                    let presets = [
                        ("G1GC", "-XX:+UseG1GC -XX:+UnlockExperimentalVMOptions -XX:G1NewSizePercent=20"),
                        ("ZGC", "-XX:+UseZGC -XX:+ZGenerational"),
                        ("Aikar's", "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions"),
                    ];
                    for (lbl, flags) in presets {
                        if ui_button(
                            ui,
                            ("jvm_preset", lbl),
                            lbl,
                            ButtonVariant::Subtle,
                            true,
                            26.0,
                        )
                        .on_hover_text(flags)
                        .clicked()
                        {
                            profile.jvm_args = flags.to_string();
                            changed = true;
                        }
                    }
                    if !profile.jvm_args.is_empty()
                        && ui_button(
                            ui,
                            "jvm_preset_clear",
                            tr(self.settings.language, "btn.clear"),
                            ButtonVariant::Subtle,
                            true,
                            26.0,
                        )
                        .clicked()
                    {
                        profile.jvm_args.clear();
                        changed = true;
                    }
                });
            });

            ui.add_space(10.0);

            // Window, Quick Play & GPU Card
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_inner_w);
                ui.set_max_width(card_inner_w);
                section_header(ui, tr(self.settings.language, "inst.sec_window"));
                ui.add_space(6.0);

                ui.label(egui::RichText::new(tr(self.settings.language, "inst.game_resolution")).small().color(TEXT_SECONDARY));
                let cur_res = (profile.window_width, profile.window_height);
                ui_wrap_row(ui, 28.0, |ui| {
                    for (lbl, dims) in [
                        ("Default", None),
                        ("1280×720", Some((1280_u32, 720_u32))),
                        ("1600×900", Some((1600_u32, 900_u32))),
                        ("1920×1080", Some((1920_u32, 1080_u32))),
                        ("2560×1440", Some((2560_u32, 1440_u32))),
                    ] {
                        let active = match dims {
                            None => cur_res.0.is_none() || cur_res.1.is_none(),
                            Some((w, h)) => cur_res == (Some(w), Some(h)),
                        };
                        if ui_pill_tab(ui, ("res_preset", lbl), lbl, active).clicked() {
                            match dims {
                                None => {
                                    profile.window_width = None;
                                    profile.window_height = None;
                                }
                                Some((w, h)) => {
                                    profile.window_width = Some(w);
                                    profile.window_height = Some(h);
                                }
                            }
                            changed = true;
                        }
                    }
                });
                ui.add_space(6.0);
                if ui_checkbox(
                    ui,
                    "cfg_fullscreen_chk",
                    &mut profile.fullscreen,
                    tr(self.settings.language, "inst.fullscreen"),
                ) {
                    changed = true;
                }
                if ui_checkbox(
                    ui,
                    "cfg_discrete_gpu_chk",
                    &mut profile.prefer_discrete_gpu,
                    tr(self.settings.language, "inst.discrete_gpu"),
                ) {
                    changed = true;
                }
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(tr(self.settings.language, "inst.quick_play"))
                        .small()
                        .color(TEXT_SECONDARY),
                );
                if ui_text_input(
                    ui,
                    "cfg_quickplay_server",
                    &mut profile.server_address,
                    "play.example.net (optional auto-join)",
                    field_w,
                    false,
                )
                .changed()
                {
                    changed = true;
                }
            });

            ui.add_space(10.0);

            // About Card
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_inner_w);
                ui.set_max_width(card_inner_w);
                ui_toolbar_row(ui, 22.0, |ui| {
                    section_header(ui, tr(self.settings.language, "inst.sec_about"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        badge(ui, "MIT License", false);
                    });
                });
                ui.add_space(4.0);
                ui_wrap_row(ui, 22.0, |ui| {
                    ui.strong(egui::RichText::new("Svarka Launcher v0.1.1").color(PURE_WHITE));
                    ui.label(
                        egui::RichText::new("• Author: svarka (@svarkawm) • Co-Author: Antigravity (Gemini)")
                            .small()
                            .color(TEXT_SECONDARY),
                    );
                });
            });
        });

        if rescan_java {
            self.scan_java();
        }
        if trigger_dup {
            self.duplicate_profile(ui.ctx(), selected_idx);
        }
        if trigger_del {
            self.confirm_delete_idx = Some(selected_idx);
        }
        if changed {
            let new_loader = self.profiles[selected_idx].loader.kind();
            if new_loader != loader_kind {
                let disabled =
                    disable_incompatible_mods_for_loader(&self.paths, &self.profiles[selected_idx]);
                if disabled > 0 {
                    self.show_toast(
                        ui.ctx(),
                        format!(
                            "Switched to {} — disabled {disabled} incompatible mod(s)",
                            new_loader.label()
                        ),
                    );
                }
            }
            let _ = save_profiles(&self.paths, &self.profiles);
            self.sync_discover_for_profile();
        }
    }

    fn render_settings_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let lang = self.settings.language;
        ui.heading(
            egui::RichText::new(tr(lang, "settings.title"))
                .size(21.0)
                .color(PURE_WHITE),
        );
        ui.label(
            egui::RichText::new(tr(lang, "settings.subtitle"))
                .small()
                .color(TEXT_SECONDARY),
        );
        ui.add_space(10.0);

        bounded_scroll(ui, "launcher_settings_scroll", |ui| {
            let card_w = (ui.available_width() - 36.0).max(160.0);

            // 1. Language
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_w);
                ui.set_max_width(card_w);
                section_header(ui, tr(lang, "settings.lang.title"));
                ui.label(
                    egui::RichText::new(tr(lang, "settings.lang.desc"))
                        .small()
                        .color(TEXT_SECONDARY),
                );
                ui.add_space(6.0);
                ui_wrap_row(ui, 30.0, |ui| {
                    for option in Language::ALL {
                        let active = self.settings.language == option;
                        if ui_pill_tab(
                            ui,
                            ("lang_pill", option.code()),
                            option.label(),
                            active,
                        )
                        .clicked()
                        {
                            self.settings.language = option;
                            let _ = save_launcher_settings(&self.paths, &self.settings);
                            self.sync_discord_presence();
                        }
                    }
                });
            });
            ui.add_space(10.0);

            // 2. Color Palette
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_w);
                ui.set_max_width(card_w);
                section_header(ui, tr(lang, "settings.theme.title"));
                ui.label(
                    egui::RichText::new(tr(lang, "settings.theme.preset"))
                        .small()
                        .color(TEXT_SECONDARY),
                );
                ui.add_space(6.0);
                ui_wrap_row(ui, 30.0, |ui| {
                    for preset in ThemePreset::ALL {
                        let active = self.settings.theme_preset == preset;
                        if ui_pill_tab(
                            ui,
                            ("theme_preset_pill", preset.label()),
                            preset.label(),
                            active,
                        )
                        .clicked()
                        {
                            self.settings.theme_preset = preset;
                            set_accent_rgb(self.settings.effective_accent_rgb());
                            apply_modrinth_theme(ctx);
                            let _ = save_launcher_settings(&self.paths, &self.settings);
                        }
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(tr(lang, "settings.theme.custom"))
                            .small()
                            .color(TEXT_SECONDARY),
                    );
                    if ui
                        .color_edit_button_srgb(&mut self.settings.custom_accent)
                        .changed()
                    {
                        self.settings.theme_preset = ThemePreset::Custom;
                        set_accent_rgb(self.settings.effective_accent_rgb());
                        apply_modrinth_theme(ctx);
                        let _ = save_launcher_settings(&self.paths, &self.settings);
                    }
                });
            });
            ui.add_space(10.0);

            // 3. Discord Rich Presence
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_w);
                ui.set_max_width(card_w);
                section_header(ui, tr(lang, "settings.discord.title"));
                if ui_checkbox(
                    ui,
                    "discord_rpc_toggle",
                    &mut self.settings.discord_rpc_enabled,
                    tr(lang, "settings.discord.enable"),
                ) {
                    let _ = save_launcher_settings(&self.paths, &self.settings);
                    self.sync_discord_presence();
                }
                ui.add_space(4.0);
                let kind = self.current_discord_kind();
                let (details, state_text) = format_presence(self.settings.language, &kind);
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(tr(lang, "settings.discord.preview"))
                            .small()
                            .color(TEXT_MUTED),
                    );
                    badge(ui, &format!("{details} • {state_text}"), self.settings.discord_rpc_enabled);
                });
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(tr(lang, "settings.discord.app_id"))
                        .small()
                        .color(TEXT_SECONDARY),
                );
                if ui_text_input(
                    ui,
                    "discord_client_id_in",
                    &mut self.settings.discord_client_id,
                    "1571137394923667579 (default)",
                    (card_w - 28.0).clamp(180.0, 360.0),
                    false,
                )
                .changed()
                {
                    let _ = save_launcher_settings(&self.paths, &self.settings);
                    self.sync_discord_presence();
                }
                ui.label(
                    egui::RichText::new(tr(lang, "settings.discord.app_id_hint"))
                        .small()
                        .color(TEXT_MUTED),
                );
            });
            ui.add_space(10.0);

            // 4. Java Runtimes Manager
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_w);
                ui.set_max_width(card_w);
                section_header(ui, tr(lang, "settings.java.title"));
                ui.label(
                    egui::RichText::new(format!(
                        "{} ({})",
                        tr(lang, "settings.java.desc"),
                        self.paths.root.join("java").display()
                    ))
                    .small()
                    .color(TEXT_SECONDARY),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for major in [8u32, 16, 17, 21, 25] {
                        let busy_this = self.java_downloading_major == Some(major);
                        let label = if busy_this {
                            format!("Java {major}...")
                        } else {
                            format!("Java {major}")
                        };
                        if ui_button_icon(
                            ui,
                            ("dl_java_btn", major),
                            Some(Icon::Download),
                            &label,
                            ButtonVariant::Secondary,
                            self.java_downloading_major.is_none(),
                            28.0,
                        )
                        .on_hover_text(format!("{} Java {major}", tr(lang, "btn.download")))
                        .clicked()
                        {
                            self.download_java_version(major);
                        }
                    }
                    if ui_button_icon(
                        ui,
                        "settings_java_dir",
                        Some(Icon::Folder),
                        tr(lang, "btn.open_folder"),
                        ButtonVariant::Secondary,
                        true,
                        28.0,
                    )
                    .clicked()
                    {
                        open_in_file_manager(&self.paths.root.join("java"));
                    }
                });
                ui.add_space(8.0);

                let mut to_remove: Option<std::path::PathBuf> = None;
                for (idx, j) in self.java_installs.iter().enumerate() {
                    let managed = is_managed_java_path(&self.paths, &j.path);
                    ui.horizontal(|ui| {
                        badge(ui, &format!("Java {}", j.major), managed);
                        badge(
                            ui,
                            if managed {
                                tr(lang, "settings.java.managed")
                            } else {
                                tr(lang, "settings.java.system")
                            },
                            false,
                        );
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(j.path.to_string_lossy().as_ref())
                                    .small()
                                    .color(TEXT_MUTED),
                            )
                            .truncate(),
                        );
                        if managed {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui_button(
                                    ui,
                                    ("rm_java_btn", idx),
                                    tr(lang, "btn.remove"),
                                    ButtonVariant::Danger,
                                    true,
                                    26.0,
                                )
                                .clicked()
                                {
                                    to_remove = Some(j.path.clone());
                                }
                            });
                        }
                    });
                    ui.add_space(3.0);
                }
                if let Some(path) = to_remove {
                    match remove_managed_java(&self.paths, &path) {
                        Ok(()) => {
                            self.show_toast(ctx, "Removed managed Java runtime");
                            self.scan_java();
                        }
                        Err(e) => {
                            self.show_toast(ctx, format!("Failed to remove Java: {e}"));
                        }
                    }
                }
            });
        });
    }

    fn render_console_page(&mut self, ui: &mut egui::Ui) {
        let lang = self.settings.language;
        let is_running = self.game_handle.is_some();
        ui_toolbar_row(ui, 32.0, |ui| {
            ui.heading(egui::RichText::new(tr(lang, "nav.console")).size(21.0).color(PURE_WHITE));
            if is_running {
                badge(ui, tr(lang, "console.live"), true);
            } else {
                badge(ui, &format!("{} {}", self.logs.len(), tr(lang, "console.lines")), false);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if ui_button_icon(
                    ui,
                    "console_clear_btn",
                    Some(Icon::Trash),
                    tr(lang, "btn.clear"),
                    ButtonVariant::Subtle,
                    !self.logs.is_empty(),
                    28.0,
                )
                .clicked()
                {
                    self.logs.clear();
                    self.log_error_count = 0;
                }
                if ui_button_icon(
                    ui,
                    "console_copy_btn",
                    Some(Icon::Copy),
                    tr(lang, "btn.copy_all"),
                    ButtonVariant::Secondary,
                    !self.logs.is_empty(),
                    28.0,
                )
                .clicked()
                {
                    let all_text: String = self
                        .logs
                        .iter()
                        .map(|l| format!("[{}] {}", l.timestamp, l.text))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ui.ctx().copy_text(all_text);
                    self.show_toast(ui.ctx(), "Console logs copied");
                }
            });
        });

        ui.add_space(6.0);
        let avail_w = ui.available_width();
        let total_logs = self.logs.len();
        let warn_err_count = self
            .logs
            .iter()
            .filter(|e| e.level != LogLevel::Info)
            .count();
        ui_wrap_row(ui, 30.0, |ui| {
            ui_search_input(
                ui,
                "console_search",
                &mut self.log_search,
                tr(lang, "console.filter"),
                (avail_w * 0.42).clamp(130.0, 280.0),
            );
            if ui_pill_tab(
                ui,
                "log_all",
                &format!("{} ({total_logs})", tr(lang, "lib.all")),
                !self.log_errors_only,
            )
            .clicked()
            {
                self.log_errors_only = false;
            }
            if ui_pill_tab(
                ui,
                "log_err",
                &format!("{} ({warn_err_count})", tr(lang, "console.warn_err")),
                self.log_errors_only,
            )
            .clicked()
            {
                self.log_errors_only = true;
            }
            ui_checkbox(
                ui,
                "console_autoscroll",
                &mut self.log_auto_scroll,
                tr(lang, "console.autoscroll"),
            );
        });

        ui.add_space(8.0);
        if let Some(diag) = diagnose_console_logs(&self.logs) {
            ui_alert_banner(ui, AlertKind::Warning, &diag, ui.available_width());
            ui.add_space(6.0);
        }
        let q = self.log_search.trim().to_lowercase();
        let scroll_h = ui.available_height().max(80.0);
        let mut copied_line: Option<String> = None;

        egui::Frame::none()
            .fill(BG_INPUT)
            .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
            .rounding(Rounding::same(10.0))
            .inner_margin(Margin::same(10.0))
            .show(ui, |ui| {
                ui.set_min_height((scroll_h - 22.0).max(60.0));
                ui.set_min_width((ui.available_width() - 4.0).max(120.0));

                if self.logs.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(28.0);
                        let (ic_r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                        draw_icon(ui.painter(), ic_r, Icon::Terminal, TEXT_MUTED);
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(tr(lang, "console.empty"))
                                .color(TEXT_MUTED),
                        );
                    });
                    return;
                }

                let mut scroll = egui::ScrollArea::vertical()
                    .id_salt("console_log_scroll")
                    .auto_shrink([false, false])
                    .max_height((scroll_h - 24.0).max(60.0));
                if self.log_auto_scroll {
                    scroll = scroll.stick_to_bottom(true);
                }
                scroll.show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let row_w = (ui.available_width() - 4.0).max(100.0);
                    for (row_idx, entry) in self.logs.iter().enumerate() {
                        if self.log_errors_only && entry.level == LogLevel::Info {
                            continue;
                        }
                        if !q.is_empty() && !entry.text.to_lowercase().contains(&q) {
                            continue;
                        }
                        let (tag, tag_col, tag_bg, msg_col, row_tint) = match entry.level {
                            LogLevel::Error => (
                                "ERR",
                                Color32::from_rgb(255, 146, 150),
                                Color32::from_rgb(68, 28, 32),
                                Color32::from_rgb(248, 138, 142),
                                Color32::from_rgba_unmultiplied(229, 72, 77, 18),
                            ),
                            LogLevel::Warn => (
                                "WARN",
                                WARNING,
                                Color32::from_rgb(56, 42, 20),
                                Color32::from_rgb(250, 198, 102),
                                Color32::from_rgba_unmultiplied(245, 166, 35, 14),
                            ),
                            LogLevel::Info => (
                                "INFO",
                                TEXT_MUTED,
                                Color32::from_rgb(27, 31, 39),
                                TEXT_SECONDARY,
                                if row_idx % 2 == 1 {
                                    Color32::from_rgba_unmultiplied(255, 255, 255, 4)
                                } else {
                                    Color32::TRANSPARENT
                                },
                            ),
                        };
                        let row_resp = egui::Frame::none()
                            .fill(row_tint)
                            .rounding(Rounding::same(5.0))
                            .inner_margin(Margin::symmetric(6.0, 2.5))
                            .show(ui, |ui| {
                                ui.set_min_width((row_w - 12.0).max(80.0));
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = 7.0;
                                    ui.label(
                                        egui::RichText::new(&entry.timestamp)
                                            .monospace()
                                            .size(11.0)
                                            .color(TEXT_MUTED),
                                    );
                                    egui::Frame::none()
                                        .fill(tag_bg)
                                        .rounding(Rounding::same(4.0))
                                        .inner_margin(Margin::symmetric(5.0, 0.5))
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(tag)
                                                    .monospace()
                                                    .strong()
                                                    .size(10.0)
                                                    .color(tag_col),
                                            );
                                        });
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&entry.text)
                                                .monospace()
                                                .size(11.5)
                                                .color(msg_col),
                                        )
                                        .wrap(),
                                    );
                                });
                            })
                            .response
                            .interact(Sense::click())
                            .on_hover_text("Click to copy line");
                        if row_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if row_resp.clicked() {
                            copied_line = Some(format!("[{}] {}", entry.timestamp, entry.text));
                        }
                    }
                });
            });
        if let Some(line) = copied_line {
            ui.ctx().copy_text(line);
            self.show_toast(ui.ctx(), "Log line copied");
        }
    }

    fn render_accounts_page(&mut self, ui: &mut egui::Ui) {
        ui_toolbar_row(ui, 32.0, |ui| {
            ui.heading(
                egui::RichText::new(tr(self.settings.language, "nav.accounts"))
                    .size(21.0)
                    .color(PURE_WHITE),
            );
            badge(ui, &format!("{}", self.account_store.accounts.len()), false);
        });
        ui.add_space(8.0);

        bounded_scroll(ui, "accounts_scroll", |ui| {
            let card_inner_w = (ui.available_width() - 32.0).max(130.0);
            let inner_w = card_inner_w.clamp(120.0, 340.0);

            if let Some(err) = &self.auth_error {
                ui_alert_banner(ui, AlertKind::Error, &format!("Error: {err}"), card_inner_w + 22.0);
                ui.add_space(6.0);
            }
            if let Some(msg) = &self.auth_status {
                ui_alert_banner(ui, AlertKind::Success, msg, card_inner_w + 22.0);
                ui.add_space(6.0);
            }

            let active_id = self.account_store.active_id.clone();
            let mut select_id: Option<String> = None;
            let mut delete_id: Option<String> = None;

            for acc in &self.account_store.accounts {
                let is_active = active_id.as_deref() == Some(acc.id.as_str());
                let acc_col = match acc.kind {
                    AccountKind::Microsoft { .. } => accent(),
                    AccountKind::ElyBy { .. } => INFO,
                    AccountKind::Offline => TEXT_SECONDARY,
                };
                let uuid_short = match &acc.kind {
                    AccountKind::Microsoft { uuid, .. } | AccountKind::ElyBy { uuid, .. } => {
                        Some(uuid.get(..8).unwrap_or(uuid.as_str()).to_string())
                    }
                    AccountKind::Offline => None,
                };

                let acc_card_id = ui.make_persistent_id(("acc_card_hover", &acc.id));
                animated_card(ui, acc_card_id, is_active, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);
                    if card_inner_w >= 340.0 {
                        ui_toolbar_row(ui, 34.0, |ui| {
                            let (av_r, _) =
                                ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                            draw_player_pixel_avatar(ui.painter(), av_r, &acc.username, acc_col);
                            ui.strong(egui::RichText::new(&acc.username).size(14.5).color(PURE_WHITE));
                            badge(ui, acc.kind_label(), is_active);
                            if is_active {
                                badge(ui, tr(self.settings.language, "acc.active"), true);
                            }
                            if let Some(u) = &uuid_short {
                                if card_inner_w >= 480.0 {
                                    ui.label(
                                        egui::RichText::new(format!("{u}..."))
                                            .monospace()
                                            .small()
                                            .color(TEXT_MUTED),
                                    );
                                }
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    if self.account_store.accounts.len() > 1
                                        && ui_icon_button(
                                            ui,
                                            ("acc_del", &acc.id),
                                            Icon::Trash,
                                            "Delete account",
                                            ButtonVariant::Danger,
                                            true,
                                            26.0,
                                        )
                                        .clicked()
                                    {
                                        delete_id = Some(acc.id.clone());
                                    }
                                    if !is_active
                                        && ui_button(
                                            ui,
                                            ("acc_sel", &acc.id),
                                            tr(self.settings.language, "btn.select"),
                                            ButtonVariant::Secondary,
                                            true,
                                            26.0,
                                        )
                                        .clicked()
                                    {
                                        select_id = Some(acc.id.clone());
                                    }
                                },
                            );
                        });
                    } else {
                        ui_wrap_row(ui, 28.0, |ui| {
                            let (av_r, _) =
                                ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                            draw_player_pixel_avatar(ui.painter(), av_r, &acc.username, acc_col);
                            ui.strong(&acc.username);
                            badge(ui, acc.kind_label(), is_active);
                            if is_active {
                                badge(ui, tr(self.settings.language, "acc.active"), true);
                            }
                            if !is_active
                                && ui_button(
                                    ui,
                                    ("acc_sel", &acc.id),
                                    tr(self.settings.language, "btn.select"),
                                    ButtonVariant::Secondary,
                                    true,
                                    28.0,
                                )
                                .clicked()
                            {
                                select_id = Some(acc.id.clone());
                            }
                            if self.account_store.accounts.len() > 1
                                && ui_icon_button(
                                    ui,
                                    ("acc_del", &acc.id),
                                    Icon::Trash,
                                    "Delete account",
                                    ButtonVariant::Danger,
                                    true,
                                    28.0,
                                )
                                .clicked()
                            {
                                delete_id = Some(acc.id.clone());
                            }
                        });
                    }
                });
                ui.add_space(4.0);
            }

            if let Some(id) = select_id {
                self.account_store.active_id = Some(id);
                let _ = save_accounts(&self.paths, &self.account_store);
            }
            if let Some(id) = delete_id {
                self.account_store.accounts.retain(|a| a.id != id);
                if self.account_store.active_id.as_deref() == Some(id.as_str()) {
                    self.account_store.active_id =
                        self.account_store.accounts.first().map(|a| a.id.clone());
                }
                let _ = save_accounts(&self.paths, &self.account_store);
            }

            ui.add_space(10.0);

            // Microsoft OAuth card
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_inner_w);
                ui.set_max_width(card_inner_w);
                ui_toolbar_row(ui, 24.0, |ui| {
                    ui.strong(egui::RichText::new(tr(self.settings.language, "acc.ms_title")).color(PURE_WHITE));
                    badge(ui, "OAuth Device Code", false);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let toggle_lbl = if self.ms_show_custom_id {
                            tr(self.settings.language, "acc.hide")
                        } else {
                            tr(self.settings.language, "acc.custom_id")
                        };
                        if ui_button(
                            ui,
                            "ms_toggle_cid",
                            toggle_lbl,
                            ButtonVariant::Subtle,
                            true,
                            22.0,
                        )
                        .clicked()
                        {
                            self.ms_show_custom_id = !self.ms_show_custom_id;
                        }
                    });
                });
                ui.label(
                    egui::RichText::new(tr(self.settings.language, "acc.ms_desc"))
                        .small()
                        .color(TEXT_SECONDARY),
                );
                if self.ms_show_custom_id {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(tr(self.settings.language, "acc.client_id")).small().color(TEXT_SECONDARY));
                    ui_text_input(ui, "ms_cid_input", &mut self.ms_client_id, DEFAULT_MS_CLIENT_ID, inner_w, false);
                }
                ui.add_space(8.0);

                if let Some(dc) = self.ms_device_code.clone() {
                    ui.label(&dc.message);
                    ui.add_space(6.0);
                    ui_wrap_row(ui, 30.0, |ui| {
                        ui.label(tr(self.settings.language, "acc.code"));
                        solid_accent_badge(ui, &dc.user_code);
                        if ui_button_icon(
                            ui,
                            "ms_copy_code_btn",
                            Some(Icon::Copy),
                            tr(self.settings.language, "btn.copy"),
                            ButtonVariant::Secondary,
                            true,
                            30.0,
                        )
                        .clicked()
                        {
                            ui.ctx().copy_text(dc.user_code.clone());
                            self.show_toast(ui.ctx(), "Code copied to clipboard");
                        }
                        if ui_button_icon(
                            ui,
                            "ms_open_browser_btn",
                            Some(Icon::External),
                            tr(self.settings.language, "acc.open_browser"),
                            ButtonVariant::Primary,
                            true,
                            30.0,
                        )
                        .clicked()
                        {
                            ui.ctx()
                                .open_url(egui::OpenUrl::new_tab(&dc.verification_uri));
                        }
                        mono_spinner(ui, 16.0);
                    });
                } else {
                    ui_toolbar_row(ui, 30.0, |ui| {
                        if ui_button_icon(
                            ui,
                            "ms_login_btn",
                            Some(Icon::User),
                            tr(self.settings.language, "acc.ms_signin"),
                            ButtonVariant::Primary,
                            !self.ms_busy,
                            30.0,
                        )
                        .clicked()
                        {
                            self.ms_busy = true;
                            self.auth_error = None;
                            self.auth_status =
                                Some("Requesting Microsoft device code...".to_string());
                            start_microsoft_device_login(
                                self.ms_client_id.clone(),
                                self.tx.clone(),
                            );
                        }
                        if self.ms_busy {
                            mono_spinner(ui, 16.0);
                        }
                    });
                }
            });

            ui.add_space(8.0);

            // Ely.by card
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_inner_w);
                ui.set_max_width(card_inner_w);
                ui_wrap_row(ui, 22.0, |ui| {
                    ui.strong(egui::RichText::new("Ely.by").color(PURE_WHITE));
                    badge(ui, "authlib-injector", false);
                });
                ui.label(
                    egui::RichText::new(
                        tr(self.settings.language, "acc.ely_desc"),
                    )
                    .small()
                    .color(TEXT_SECONDARY),
                );
                ui.add_space(6.0);
                ui.label(egui::RichText::new(tr(self.settings.language, "acc.login_label")).small().color(TEXT_SECONDARY));
                ui_toolbar_row(ui, 30.0, |ui| {
                    ui_text_input(ui, "ely_login_in", &mut self.ely_login, tr(self.settings.language, "acc.login_hint"), inner_w, false);
                });
                ui.add_space(5.0);
                ui.label(egui::RichText::new(tr(self.settings.language, "acc.password")).small().color(TEXT_SECONDARY));
                ui_toolbar_row(ui, 30.0, |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui_text_input(
                        ui,
                        "ely_pass_in",
                        &mut self.ely_password,
                        tr(self.settings.language, "acc.pass_hint"),
                        (inner_w - 64.0).max(80.0),
                        !self.ely_show_password,
                    );
                    if ui_button(
                        ui,
                        "ely_show_pw",
                        if self.ely_show_password { tr(self.settings.language, "acc.hide") } else { tr(self.settings.language, "acc.show") },
                        ButtonVariant::Subtle,
                        true,
                        30.0,
                    )
                    .clicked()
                    {
                        self.ely_show_password = !self.ely_show_password;
                    }
                });
                ui.add_space(5.0);
                ui.label(egui::RichText::new(tr(self.settings.language, "acc.2fa_label")).small().color(TEXT_SECONDARY));
                ui_text_input(ui, "ely_totp_in", &mut self.ely_totp, tr(self.settings.language, "acc.2fa_hint"), inner_w, false);
                ui.add_space(8.0);
                ui_toolbar_row(ui, 28.0, |ui| {
                    let ely_valid = !self.ely_busy
                        && !self.ely_login.trim().is_empty()
                        && !self.ely_password.is_empty();
                    if ui_button_icon(
                        ui,
                        "ely_login_btn",
                        Some(Icon::User),
                        tr(self.settings.language, "acc.ely_signin"),
                        ButtonVariant::Secondary,
                        ely_valid,
                        28.0,
                    )
                    .clicked()
                    {
                        self.ely_busy = true;
                        self.auth_error = None;
                        self.auth_status = Some("Signing in to Ely.by...".to_string());
                        let login = self.ely_login.clone();
                        let password = self.ely_password.clone();
                        let totp = self.ely_totp.clone();
                        let tx = self.tx.clone();
                        thread::spawn(move || {
                            let res = login_ely_by(&login, &password, &totp);
                            let _ = tx.send(WorkerEvent::ElyAuthFinished(res));
                        });
                    }
                    if self.ely_busy {
                        mono_spinner(ui, 16.0);
                    }
                });
            });

            ui.add_space(8.0);

            // Offline card
            card_frame(false, 0.0).show(ui, |ui| {
                ui.set_min_width(card_inner_w);
                ui.set_max_width(card_inner_w);
                ui.strong(egui::RichText::new(tr(self.settings.language, "acc.offline_title")).color(PURE_WHITE));
                ui.add_space(6.0);
                ui_wrap_row(ui, 30.0, |ui| {
                    ui.label(tr(self.settings.language, "acc.username"));
                    let edit_resp = ui_text_input(
                        ui,
                        "offline_user_in",
                        &mut self.offline_input,
                        "Steve",
                        (inner_w - 130.0).max(110.0),
                        false,
                    );
                    let enter = edit_resp.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let valid = !self.offline_input.trim().is_empty();
                    if (ui_button_icon(
                        ui,
                        "offline_add_btn",
                        Some(Icon::Plus),
                        tr(self.settings.language, "btn.add"),
                        ButtonVariant::Primary,
                        valid,
                        30.0,
                    )
                    .clicked()
                        || enter)
                        && valid
                    {
                        let name = self.offline_input.trim().to_string();
                        let acc = Account::offline(name.clone());
                        self.account_store.active_id = Some(acc.id.clone());
                        self.account_store.accounts.push(acc);
                        let _ = save_accounts(&self.paths, &self.account_store);
                        self.offline_input.clear();
                        self.auth_error = None;
                        self.auth_status = Some(format!("Added Offline account: {name}"));
                    }
                });
            });
        });
    }

    fn render_discover_page(&mut self, ui: &mut egui::Ui) {
        let cur_profile = self.profiles.get(self.selected_idx).cloned();
        let cur_loader = cur_profile.as_ref().map(|p| p.loader.kind());
        let is_vanilla_mods =
            cur_loader == Some(LoaderKind::Vanilla) && self.mr_type == ModrinthProjectType::Mod;
        let target_label = cur_profile
            .as_ref()
            .map(|p| {
                if is_vanilla_mods {
                    format!("Vanilla {} (no mod loader)", p.version_id)
                } else {
                    format!("{} {} • {}", tr(self.settings.language, "mr.only_for"), p.version_id, p.loader.short_name())
                }
            })
            .unwrap_or_else(|| "No instance selected".to_string());

        ui_toolbar_row(ui, 30.0, |ui| {
            ui.heading(egui::RichText::new("Modrinth").size(21.0).color(PURE_WHITE));
            if let Some(p) = self.profiles.get(self.selected_idx) {
                badge(ui, &p.name, false);
                loader_badge(ui, p.loader.kind(), p.loader.short_name());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui_button_icon(
                    ui,
                    "mr_inst_content_btn",
                    Some(Icon::Layers),
                    tr(self.settings.language, "btn.instance_content"),
                    ButtonVariant::Secondary,
                    true,
                    30.0,
                )
                .clicked()
                {
                    self.instance_tab = InstanceSubTab::Content;
                    self.switch_page(ui.ctx(), NavPage::Instance);
                }
            });
        });

        ui.add_space(6.0);
        ui_wrap_row(ui, 30.0, |ui| {
            for (pt, ic) in [
                (ModrinthProjectType::Mod, Icon::Layers),
                (ModrinthProjectType::Modpack, Icon::Grid),
                (ModrinthProjectType::Resourcepack, Icon::Sparkle),
            ] {
                let pt_lbl = match pt {
                    ModrinthProjectType::Mod => tr(self.settings.language, "content.mods"),
                    ModrinthProjectType::Modpack => tr(self.settings.language, "mr.modpacks"),
                    ModrinthProjectType::Resourcepack => {
                        tr(self.settings.language, "content.resourcepacks")
                    }
                };
                if ui_pill_tab_icon(
                    ui,
                    ("mr_type_pill", pt.label()),
                    Some(ic),
                    pt_lbl,
                    self.mr_type == pt,
                )
                .clicked()
                    && self.mr_type != pt
                {
                    self.mr_type = pt;
                    self.trigger_modrinth_search(false);
                }
            }
            let lang = self.settings.language;
            let sort_label = |s: ModrinthSort| match s {
                ModrinthSort::Relevance => tr(lang, "mr.sort_relevance"),
                ModrinthSort::Downloads => tr(lang, "mr.sort_downloads"),
                ModrinthSort::Newest => tr(lang, "mr.sort_newest"),
                ModrinthSort::Updated => tr(lang, "mr.sort_updated"),
            };
            egui::ComboBox::from_id_salt("mr_sort_combo")
                .selected_text(sort_label(self.mr_sort))
                .width(136.0)
                .show_ui(ui, |ui| {
                    for s in [
                        ModrinthSort::Relevance,
                        ModrinthSort::Downloads,
                        ModrinthSort::Newest,
                        ModrinthSort::Updated,
                    ] {
                        if ui.selectable_value(&mut self.mr_sort, s, sort_label(s)).changed() {
                            self.trigger_modrinth_search(false);
                        }
                    }
                });
            badge_sized(ui, &target_label, !is_vanilla_mods, 28.0, None);
        });

        ui.add_space(5.0);
        let search_w = (ui.available_width() - 126.0).clamp(110.0, 460.0);
        ui_toolbar_row(ui, 30.0, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let enter_or_cleared = ui_search_input(
                ui,
                "mr_search_input",
                &mut self.mr_query,
                tr(self.settings.language, "mr.search_placeholder"),
                search_w,
            );
            if (ui_button_icon(
                ui,
                "mr_search_btn",
                Some(Icon::Search),
                tr(self.settings.language, "btn.search"),
                ButtonVariant::Primary,
                !self.mr_searching,
                30.0,
            )
            .clicked()
                || enter_or_cleared)
                && !self.mr_searching
            {
                self.trigger_modrinth_search(false);
            }
            if self.mr_searching {
                mono_spinner(ui, 16.0);
            } else if !self.mr_results.is_empty() {
                ui.label(
                    egui::RichText::new(format!("{} {}", self.mr_results.len(), tr(self.settings.language, "mr.compatible")))
                        .small()
                        .color(TEXT_MUTED),
                );
            }
        });

        ui.add_space(5.0);
        let topic_chips: &[&str] = match self.mr_type {
            ModrinthProjectType::Mod => &[
                "All",
                "Optimization",
                "Sodium",
                "Iris",
                "JEI",
                "Worldgen",
                "Minimap",
            ],
            ModrinthProjectType::Modpack => {
                &["All", "Optimization", "FPS", "RPG", "Tech", "Kitchen Sink"]
            }
            ModrinthProjectType::Resourcepack => {
                &["All", "Faithful", "GUI", "PvP", "3D", "Dark Mode"]
            }
        };
        ui_wrap_row(ui, 26.0, |ui| {
            for &topic in topic_chips {
                let active = if topic == "All" {
                    self.mr_query.trim().is_empty()
                } else {
                    self.mr_query.trim().eq_ignore_ascii_case(topic)
                };
                let topic_lbl = if topic == "All" {
                    tr(self.settings.language, "lib.all")
                } else {
                    topic
                };
                if ui_pill_tab(ui, ("mr_topic", self.mr_type.api_str(), topic), topic_lbl, active)
                    .clicked()
                {
                    if topic == "All" || active {
                        self.mr_query.clear();
                    } else {
                        self.mr_query = topic.to_string();
                    }
                    self.trigger_modrinth_search(false);
                }
            }
        });

        if let Some(err) = &self.mr_error {
            ui.add_space(6.0);
            ui_alert_banner(
                ui,
                AlertKind::Error,
                &format!("Error: {err}"),
                ui.available_width(),
            );
        }
        if let Some(msg) = &self.mr_status {
            ui.add_space(6.0);
            ui_alert_banner(ui, AlertKind::Success, msg, ui.available_width());
        }

        ui.add_space(6.0);
        let mut install_req: Option<(String, String, Option<String>)> = None;
        let mut open_detail: Option<ModrinthSearchHit> = None;
        let mut load_more = false;
        let mr_type = self.mr_type;
        let mr_installing = self.mr_installing;
        let mr_installing_id = self.mr_installing_id.clone();

        bounded_scroll(ui, "modrinth_scroll", |ui| {
            let usable_w = (ui.available_width() - 12.0).max(160.0);
            let card_inner_w = (usable_w - 22.0).max(140.0);

            if self.mr_results.is_empty() && self.mr_searching {
                for _ in 0..4 {
                    draw_skeleton_card(ui, card_inner_w);
                    ui.add_space(6.0);
                }
                return;
            } else if self.mr_results.is_empty() && !self.mr_searching {
                let mut open_inst_settings = false;
                card_frame(false, 0.0).show(ui, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);
                    if is_vanilla_mods {
                        ui_wrap_row(ui, 30.0, |ui| {
                            ui.label(
                                egui::RichText::new(
                                    "Vanilla instances do not support mods. Switch loader to Fabric, Quilt, Forge, or NeoForge.",
                                )
                                .color(TEXT_MUTED),
                            );
                            if ui_button_icon(
                                ui,
                                "mr_open_inst_settings",
                                Some(Icon::Sliders),
                                tr(self.settings.language, "inst.settings_btn"),
                                ButtonVariant::Primary,
                                true,
                                28.0,
                            )
                            .clicked()
                            {
                                open_inst_settings = true;
                            }
                        });
                    } else {
                        ui.label(
                            egui::RichText::new(
                                tr(self.settings.language, "mr.no_results"),
                            )
                            .color(TEXT_MUTED),
                        );
                    }
                });
                if open_inst_settings {
                    self.instance_tab = InstanceSubTab::Settings;
                    self.switch_page(ui.ctx(), NavPage::Instance);
                }
                return;
            }

            let results = self.mr_results.clone();
            let active_loader_slug = cur_loader.and_then(|l| l.modrinth_slug());
            let wide_card = card_inner_w >= 430.0;
            for hit in &results {
                let is_installed = self.mr_installed_cache.contains_key(&hit.project_id);
                let is_this_installing = mr_installing
                    && mr_installing_id.as_deref() == Some(hit.project_id.as_str());
                let btn_lbl = if is_this_installing {
                    "Installing..."
                } else if mr_type == ModrinthProjectType::Modpack {
                    tr(self.settings.language, "modal.create_instance")
                } else if is_installed {
                    tr(self.settings.language, "mr.update")
                } else {
                    tr(self.settings.language, "mr.install")
                };
                let btn_variant = if is_installed {
                    ButtonVariant::Secondary
                } else {
                    ButtonVariant::Primary
                };
                let btn_icon = if is_installed {
                    Icon::Refresh
                } else {
                    Icon::Download
                };

                let card_id = ui.make_persistent_id(("mr_card_hover", &hit.project_id));
                animated_card(ui, card_id, is_installed, |ui| {
                    ui.set_min_width(card_inner_w);
                    ui.set_max_width(card_inner_w);

                    let tex_opt = hit
                        .icon_url
                        .as_ref()
                        .and_then(|u| self.mr_icons.get(u))
                        .and_then(|o| o.as_ref());
                    let is_icon_loading = hit
                        .icon_url
                        .as_ref()
                        .is_some_and(|u| self.mr_icons_pending.contains(u.as_str()));

                    if wide_card {
                        let icon_px = 52.0;
                        let action_w = 206.0;
                        let btn_w = 132.0;
                        let gap_x = 12.0;
                        let info_w = (card_inner_w - icon_px - action_w - gap_x * 2.0).max(120.0);
                        let max_desc_chars = ((info_w / 6.3) * 1.85).clamp(48.0, 145.0) as usize;
                        let desc = clamp_summary(&hit.description, max_desc_chars);

                        ui.allocate_ui_with_layout(
                            Vec2::new(card_inner_w, 72.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.set_min_size(Vec2::new(card_inner_w, 72.0));
                                ui.set_max_size(Vec2::new(card_inner_w, 72.0));
                                ui.spacing_mut().item_spacing.x = gap_x;

                                let icon_size = Vec2::splat(icon_px);
                                if let Some(tex) = tex_opt {
                                    ui.add(
                                        egui::Image::new(tex)
                                            .fit_to_exact_size(icon_size)
                                            .rounding(Rounding::same(10.0)),
                                    );
                                } else {
                                    let (rect, _) =
                                        ui.allocate_exact_size(icon_size, Sense::hover());
                                    draw_icon_placeholder(
                                        ui,
                                        rect,
                                        10.0,
                                        &hit.title,
                                        is_icon_loading,
                                    );
                                }

                                ui.allocate_ui_with_layout(
                                    Vec2::new(info_w, 72.0),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        ui.set_min_size(Vec2::new(info_w, 72.0));
                                        ui.set_max_size(Vec2::new(info_w, 72.0));
                                        ui.spacing_mut().item_spacing.y = 2.0;

                                        // Line 1: Title + Installed badge
                                        ui_toolbar_row(ui, 20.0, |ui| {
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            let badge_w = if is_installed { 76.0 } else { 0.0 };
                                            ui.allocate_ui(
                                                Vec2::new((info_w - badge_w).max(60.0), 20.0),
                                                |ui| {
                                                    let title_resp = ui
                                                        .add(
                                                            egui::Label::new(
                                                                egui::RichText::new(&hit.title)
                                                                    .strong()
                                                                    .size(14.5)
                                                                    .color(PURE_WHITE),
                                                            )
                                                            .truncate()
                                                            .sense(Sense::click()),
                                                        )
                                                        .on_hover_cursor(
                                                            egui::CursorIcon::PointingHand,
                                                        )
                                                        .on_hover_text(
                                                            "Click to view compatible versions",
                                                        );
                                                    if title_resp.clicked() {
                                                        open_detail = Some(hit.clone());
                                                    }
                                                },
                                            );
                                            if is_installed {
                                                badge(ui, tr(self.settings.language, "mr.installed"), true);
                                            }
                                        });

                                        // Line 2: Uniform 28px summary slot so all rows have equal height
                                        ui.allocate_ui_with_layout(
                                            Vec2::new(info_w, 28.0),
                                            egui::Layout::top_down(egui::Align::Min),
                                            |ui| {
                                                ui.set_min_size(Vec2::new(info_w, 28.0));
                                                ui.set_max_size(Vec2::new(info_w, 28.0));
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(&desc)
                                                            .small()
                                                            .color(TEXT_SECONDARY),
                                                    )
                                                    .wrap(),
                                                );
                                            },
                                        );

                                        // Line 3: Single-line stats + category chips
                                        ui_toolbar_row(ui, 20.0, |ui| {
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            let display_cats: Vec<&str> = hit
                                                .categories
                                                .iter()
                                                .map(String::as_str)
                                                .filter(|c| match *c {
                                                    "fabric" | "quilt" | "forge" | "neoforge" => {
                                                        active_loader_slug == Some(*c)
                                                    }
                                                    _ => true,
                                                })
                                                .collect();
                                            let cats_count = if info_w >= 380.0 {
                                                display_cats.len().min(3)
                                            } else if info_w >= 300.0 {
                                                display_cats.len().min(2)
                                            } else {
                                                0
                                            };
                                            let cats_reserve = if cats_count > 0 {
                                                cats_count as f32 * 76.0
                                            } else {
                                                0.0
                                            };
                                            let meta_w = (info_w - cats_reserve - 18.0).max(80.0);
                                            let (dl_r, _) = ui.allocate_exact_size(
                                                Vec2::splat(11.0),
                                                Sense::hover(),
                                            );
                                            draw_icon(
                                                ui.painter(),
                                                dl_r,
                                                Icon::Download,
                                                TEXT_MUTED,
                                            );
                                            ui.allocate_ui(Vec2::new(meta_w, 20.0), |ui| {
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(format!(
                                                            "{} • by {}",
                                                            format_downloads(hit.downloads),
                                                            hit.author
                                                        ))
                                                        .small()
                                                        .color(TEXT_MUTED),
                                                    )
                                                    .truncate(),
                                                );
                                            });
                                            for cat in display_cats.iter().take(cats_count) {
                                                badge(ui, cat, false);
                                            }
                                        });
                                    },
                                );

                                ui.allocate_ui_with_layout(
                                    Vec2::new(action_w, 72.0),
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;
                                        if ui_icon_button(
                                            ui,
                                            ("mr_ext_w", &hit.project_id),
                                            Icon::External,
                                            "View on Modrinth",
                                            ButtonVariant::Subtle,
                                            true,
                                            30.0,
                                        )
                                        .clicked()
                                        {
                                            let url = format!(
                                                "https://modrinth.com/{}/{}",
                                                mr_type.api_str(),
                                                hit.project_id
                                            );
                                            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                                        }
                                        if ui_icon_button(
                                            ui,
                                            ("mr_vers_w", &hit.project_id),
                                            Icon::List,
                                            "Browse compatible versions",
                                            ButtonVariant::Subtle,
                                            true,
                                            30.0,
                                        )
                                        .clicked()
                                        {
                                            open_detail = Some(hit.clone());
                                        }
                                        if is_this_installing {
                                            ui.allocate_ui_with_layout(
                                                Vec2::new(btn_w, 30.0),
                                                egui::Layout::centered_and_justified(
                                                    egui::Direction::LeftToRight,
                                                ),
                                                |ui| mono_spinner(ui, 18.0),
                                            );
                                        } else if ui_button_sized(
                                            ui,
                                            ("mr_inst_w", &hit.project_id),
                                            Some(btn_icon),
                                            btn_lbl,
                                            btn_variant,
                                            !mr_installing,
                                            Vec2::new(btn_w, 30.0),
                                        )
                                        .clicked()
                                        {
                                            install_req = Some((
                                                hit.project_id.clone(),
                                                hit.title.clone(),
                                                hit.icon_url.clone(),
                                            ));
                                        }
                                    },
                                );
                            },
                        );
                    } else {
                        let icon_px = 42.0;
                        let info_w = (card_inner_w - icon_px - 10.0).max(80.0);
                        ui_toolbar_row(ui, 44.0, |ui| {
                            let icon_size = Vec2::splat(icon_px);
                            if let Some(tex) = tex_opt {
                                ui.add(
                                    egui::Image::new(tex)
                                        .fit_to_exact_size(icon_size)
                                        .rounding(Rounding::same(9.0)),
                                );
                            } else {
                                let (rect, _) = ui.allocate_exact_size(icon_size, Sense::hover());
                                draw_icon_placeholder(
                                    ui,
                                    rect,
                                    9.0,
                                    &hit.title,
                                    is_icon_loading,
                                );
                            }
                            ui.allocate_ui_with_layout(
                                Vec2::new(info_w, 42.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui_toolbar_row(ui, 20.0, |ui| {
                                        let badge_w = if is_installed { 72.0 } else { 0.0 };
                                        ui.allocate_ui(
                                            Vec2::new((info_w - badge_w).max(40.0), 20.0),
                                            |ui| {
                                                let title_resp = ui
                                                    .add(
                                                        egui::Label::new(
                                                            egui::RichText::new(&hit.title)
                                                                .strong()
                                                                .color(PURE_WHITE),
                                                        )
                                                        .truncate()
                                                        .sense(Sense::click()),
                                                    )
                                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                                    .on_hover_text(
                                                        "Click to view compatible versions",
                                                    );
                                                if title_resp.clicked() {
                                                    open_detail = Some(hit.clone());
                                                }
                                            },
                                        );
                                        if is_installed {
                                            badge(ui, tr(self.settings.language, "mr.installed"), true);
                                        }
                                    });
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(format!(
                                                "{} • {}",
                                                format_downloads(hit.downloads),
                                                hit.author
                                            ))
                                            .small()
                                            .color(TEXT_MUTED),
                                        )
                                        .truncate(),
                                    );
                                },
                            );
                        });
                        ui.add_space(5.0);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(clamp_summary(&hit.description, 90))
                                    .small()
                                    .color(TEXT_SECONDARY),
                            )
                            .wrap(),
                        );
                        ui.add_space(6.0);
                        ui_toolbar_row(ui, 30.0, |ui| {
                            if is_this_installing {
                                mono_spinner(ui, 16.0);
                            } else if ui_button_sized(
                                ui,
                                ("mr_inst_n", &hit.project_id),
                                Some(btn_icon),
                                btn_lbl,
                                btn_variant,
                                !mr_installing,
                                Vec2::new((card_inner_w - 38.0).max(80.0), 30.0),
                            )
                            .clicked()
                            {
                                install_req = Some((
                                    hit.project_id.clone(),
                                    hit.title.clone(),
                                    hit.icon_url.clone(),
                                ));
                            }
                            if ui_icon_button(
                                ui,
                                ("mr_vers_n", &hit.project_id),
                                Icon::List,
                                "Browse compatible versions",
                                ButtonVariant::Subtle,
                                true,
                                30.0,
                            )
                            .clicked()
                            {
                                open_detail = Some(hit.clone());
                            }
                        });
                    }
                });
                ui.add_space(6.0);
            }

            if !self.mr_results.is_empty() && self.mr_has_more {
                ui.add_space(4.0);
                ui_wrap_row(ui, 30.0, |ui| {
                    if ui_button_icon(
                        ui,
                        "mr_load_more_btn",
                        Some(Icon::Plus),
                        tr(self.settings.language, "btn.load_more"),
                        ButtonVariant::Secondary,
                        !self.mr_searching,
                        30.0,
                    )
                    .clicked()
                    {
                        load_more = true;
                    }
                    ui.label(
                        egui::RichText::new(format!(
                            "Showing {} compatible",
                            self.mr_results.len(),
                        ))
                        .small()
                        .color(TEXT_MUTED),
                    );
                });
            }
        });

        if let Some(hit) = open_detail {
            self.open_project_details(&hit);
        }

        if load_more {
            self.trigger_modrinth_search(true);
        }

        if let Some((project_id, title, icon_url)) = install_req {
            if let Some(profile) = self.profiles.get(self.selected_idx).cloned() {
                self.mr_installing = true;
                self.mr_installing_id = Some(project_id.clone());
                self.mr_error = None;
                self.mr_status = Some(format!("Installing {title}..."));
                self.progress = 0.05;
                let paths = self.paths.clone();
                let tx = self.tx.clone();
                thread::spawn(move || {
                    let res = install_project(
                        &paths,
                        &profile,
                        &project_id,
                        mr_type,
                        &title,
                        icon_url.as_deref(),
                        &tx,
                    );
                    let _ = tx.send(WorkerEvent::ModrinthInstallFinished(res));
                });
            }
        }
    }

    fn render_new_profile_modal(&mut self, ctx: &egui::Context) {
        let modal_t = ctx.animate_bool_with_time(
            egui::Id::new("new_profile_modal_anim"),
            self.show_new_modal,
            0.16,
        );
        if modal_t > 0.01 && modal_t < 0.99 {
            ctx.request_repaint();
        }
        if !self.show_new_modal && modal_t <= 0.01 {
            return;
        }

        let screen = ctx.screen_rect();
        if self.show_new_modal && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_new_modal = false;
            return;
        }
        egui::Area::new(egui::Id::new("modal_backdrop"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    Rounding::ZERO,
                    Color32::from_rgba_unmultiplied(0, 0, 0, (modal_t * 170.0) as u8),
                );
                if resp.clicked() {
                    self.show_new_modal = false;
                }
            });

        if !self.show_new_modal {
            return;
        }

        let mut open = self.show_new_modal;
        let max_modal_w = (screen.width() - 32.0).clamp(260.0, 480.0);
        let win_resp = egui::Window::new(tr(self.settings.language, "modal.create_title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .max_width(max_modal_w)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                let field_w = (ui.available_width() - 96.0).clamp(120.0, 290.0);
                let mut enter_create = false;
                egui::Grid::new("new_profile_grid")
                    .num_columns(2)
                    .spacing([12.0, 10.0])
                    .show(ui, |ui| {
                        ui.label(tr(self.settings.language, "modal.name"));
                        let r = ui_text_input(
                            ui,
                            "modal_inst_name",
                            &mut self.new_name,
                            "My Modded World",
                            field_w,
                            false,
                        );
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            enter_create = true;
                        }
                        ui.end_row();

                        ui.label(tr(self.settings.language, "modal.version"));
                        ui_wrap_row(ui, 30.0, |ui| {
                            egui::ComboBox::from_id_salt("new_ver_combo")
                                .selected_text(&self.new_version)
                                .width((field_w - 108.0).max(110.0))
                                .show_ui(ui, |ui| {
                                    if let Some(m) = &self.manifest {
                                        for v in m
                                            .versions
                                            .iter()
                                            .filter(|v| {
                                                self.new_show_snapshots
                                                    || v.version_type == VersionType::Release
                                            })
                                            .take(80)
                                        {
                                            ui.selectable_value(
                                                &mut self.new_version,
                                                v.id.clone(),
                                                &v.id,
                                            );
                                        }
                                    }
                                });
                            ui_checkbox(
                                ui,
                                "modal_snapshots_chk",
                                &mut self.new_show_snapshots,
                                tr(self.settings.language, "inst.snapshots"),
                            );
                        });
                        ui.end_row();

                        ui.label(tr(self.settings.language, "modal.loader"));
                        ui_wrap_row(ui, 28.0, |ui| {
                            for kind in LoaderKind::ALL {
                                if ui_loader_pill(
                                    ui,
                                    ("modal_loader", kind.label()),
                                    kind,
                                    self.new_loader == kind,
                                )
                                .clicked()
                                {
                                    self.new_loader = kind;
                                }
                            }
                        });
                        ui.end_row();

                        ui.label(tr(self.settings.language, "modal.memory"));
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 6.0;
                            ui_wrap_row(ui, 28.0, |ui| {
                                for preset in [2048_u32, 4096, 6144, 8192, 12288] {
                                    if ui_pill_tab(
                                        ui,
                                        ("modal_ram", preset),
                                        &format!("{} GB", preset / 1024),
                                        self.new_ram_mb == preset,
                                    )
                                    .clicked()
                                    {
                                        self.new_ram_mb = preset;
                                    }
                                }
                            });
                            ui_ram_slider(
                                ui,
                                &mut self.new_ram_mb,
                                512..=16384,
                                (field_w - 86.0).clamp(110.0, 220.0),
                            );
                        });
                        ui.end_row();
                    });

                ui.add_space(12.0);
                let name_valid = !self.new_name.trim().is_empty();
                ui_toolbar_row(ui, 30.0, |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if (ui_button_icon(
                        ui,
                        "modal_create_btn",
                        Some(Icon::Plus),
                        tr(self.settings.language, "modal.create_instance"),
                        ButtonVariant::Primary,
                        name_valid,
                        30.0,
                    )
                    .clicked()
                        || enter_create)
                        && name_valid
                    {
                        let loader = ModLoader::from_kind(self.new_loader, String::new());
                        let mut profile = Profile::new(
                            self.new_name.trim().to_string(),
                            self.new_version.clone(),
                            loader,
                            self.new_username.trim().to_string(),
                        );
                        profile.max_ram_mb = self.new_ram_mb;
                        self.profiles.push(profile);
                        let new_idx = self.profiles.len() - 1;
                        let _ = save_profiles(&self.paths, &self.profiles);
                        self.select_profile(new_idx);
                        self.show_new_modal = false;
                        self.show_toast(ctx, "Instance created");
                    }
                    if ui_button(
                        ui,
                        "modal_cancel_btn",
                        tr(self.settings.language, "btn.cancel"),
                        ButtonVariant::Secondary,
                        true,
                        30.0,
                    )
                    .clicked()
                    {
                        self.show_new_modal = false;
                    }
                });
            });
        if let Some(ir) = win_resp {
            ctx.move_to_top(ir.response.layer_id);
        }
        if !open {
            self.show_new_modal = false;
        }
    }

    fn render_delete_confirm_modal(&mut self, ctx: &egui::Context) {
        let Some(del_idx) = self.confirm_delete_idx else {
            return;
        };
        let Some(target) = self.profiles.get(del_idx).cloned() else {
            self.confirm_delete_idx = None;
            return;
        };

        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("del_modal_backdrop"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    Rounding::ZERO,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 165),
                );
                if resp.clicked() {
                    self.confirm_delete_idx = None;
                }
            });

        let mut open = true;
        let max_w = (screen.width() - 32.0).clamp(240.0, 400.0);
        egui::Window::new(tr(self.settings.language, "modal.delete_title"))
            .order(egui::Order::Foreground)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .max_width(max_w)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!("Remove '{}' from your library?", target.name))
                        .strong()
                        .color(PURE_WHITE),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!(
                        "{} • {}",
                        target.version_id,
                        target.loader.label()
                    ))
                    .small()
                    .color(TEXT_SECONDARY),
                );
                ui.add_space(12.0);
                ui_toolbar_row(ui, 30.0, |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if ui_button_icon(
                        ui,
                        "confirm_del_yes",
                        Some(Icon::Trash),
                        tr(self.settings.language, "btn.delete"),
                        ButtonVariant::Danger,
                        true,
                        30.0,
                    )
                    .clicked()
                    {
                        if del_idx < self.profiles.len() && self.profiles.len() > 1 {
                            let removed = self.profiles.remove(del_idx);
                            if del_idx < self.selected_idx {
                                self.selected_idx -= 1;
                            } else if self.selected_idx >= self.profiles.len() {
                                self.selected_idx = self.profiles.len().saturating_sub(1);
                            }
                            let _ = save_profiles(&self.paths, &self.profiles);
                            self.persist_selected_profile();
                            self.sync_installed_index();
                            self.sync_discover_for_profile();
                            self.show_toast(ctx, format!("Removed '{}'", removed.name));
                        }
                        self.confirm_delete_idx = None;
                    }
                    if ui_button(
                        ui,
                        "confirm_del_cancel",
                        tr(self.settings.language, "btn.cancel"),
                        ButtonVariant::Secondary,
                        true,
                        30.0,
                    )
                    .clicked()
                    {
                        self.confirm_delete_idx = None;
                    }
                });
            });
        if !open {
            self.confirm_delete_idx = None;
        }
    }

    fn render_toast(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let active = match &self.toast {
            Some(t) if now - t.created_at < 3.8 => Some((t.text.clone(), now - t.created_at)),
            _ => {
                self.toast = None;
                None
            }
        };

        if let Some((text, age)) = active {
            let enter = (age / 0.2).clamp(0.0, 1.0) as f32;
            let exit = ((3.8 - age) / 0.3).clamp(0.0, 1.0) as f32;
            let alpha = enter.min(exit);
            let slide_y = (1.0 - (1.0 - enter).powi(3)) * 14.0;
            let rem_frac = (1.0 - (age / 3.8)).clamp(0.0, 1.0) as f32;
            let is_err = text.to_lowercase().contains("error")
                || text.to_lowercase().contains("failed");
            let accent_col = if is_err { DANGER } else { accent() };
            let bg_col = if is_err {
                Color32::from_rgb(46, 24, 28)
            } else {
                bg_card_active()
            };
            let max_w = (ctx.screen_rect().width() - 36.0).clamp(160.0, 420.0);
            ctx.request_repaint();

            egui::Area::new(egui::Id::new("toast_overlay"))
                .order(egui::Order::Tooltip)
                .anchor(egui::Align2::RIGHT_BOTTOM, Vec2::new(-16.0, -44.0 + slide_y))
                .show(ctx, |ui| {
                    ui.set_max_width(max_w);
                    let a = (alpha * 255.0) as u8;
                    let f_resp = egui::Frame::none()
                        .fill(with_alpha(bg_col, a))
                        .stroke(Stroke::new(1.2_f32, with_alpha(accent_col, a)))
                        .rounding(Rounding::same(9.5))
                        .inner_margin(Margin::symmetric(14.0, 10.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                let (ic_r, _) =
                                    ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                                draw_icon(
                                    ui.painter(),
                                    ic_r,
                                    if is_err { Icon::Alert } else { Icon::Check },
                                    with_alpha(accent_col, a),
                                );
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(text).color(with_alpha(PURE_WHITE, a)),
                                    )
                                    .wrap(),
                                );
                            });
                        });
                    let rect = f_resp.response.rect;
                    let bar_w = ((rect.width() - 12.0) * rem_frac).max(0.0);
                    if bar_w > 1.0 {
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                Pos2::new(rect.left() + 6.0, rect.bottom() - 2.5),
                                Vec2::new(bar_w, 2.0),
                            ),
                            Rounding::same(1.0),
                            with_alpha(accent_col, (alpha * 190.0) as u8),
                        );
                    }
                });
        }
    }

    fn open_project_details(&mut self, hit: &ModrinthSearchHit) {
        self.mr_detail_hit = Some(hit.clone());
        self.mr_detail_loading = true;
        self.mr_detail_versions = None;

        let (game_version, loader) = self
            .profiles
            .get(self.selected_idx)
            .map(|p| (p.version_id.clone(), p.loader.clone()))
            .unwrap_or_else(|| ("1.21.4".to_string(), ModLoader::Vanilla));
        let project_id = hit.project_id.clone();
        let mr_type = self.mr_type;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result =
                fetch_project_versions_for_profile(&project_id, mr_type, &game_version, &loader);
            let _ = tx.send(WorkerEvent::ProjectVersionsLoaded { project_id, result });
        });
    }

    fn handle_file_drop(&mut self, ctx: &egui::Context) {
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if hovering {
            let screen = ctx.screen_rect();
            let target_hint = self
                .profiles
                .get(self.selected_idx)
                .map(|p| format!("Into \"{}\" ({})", p.name, self.content_folder.label()))
                .unwrap_or_else(|| "No active instance selected".to_string());
            egui::Area::new(egui::Id::new("file_drop_overlay"))
                .order(egui::Order::Foreground)
                .fixed_pos(screen.min)
                .show(ctx, |ui| {
                    let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::hover());
                    ui.painter().rect_filled(
                        rect,
                        Rounding::ZERO,
                        Color32::from_rgba_unmultiplied(8, 12, 18, 200),
                    );
                    let box_w = (screen.width() - 48.0).clamp(260.0, 440.0);
                    let box_rect = Rect::from_center_size(rect.center(), Vec2::new(box_w, 128.0));
                    ui.painter().rect(
                        box_rect,
                        Rounding::same(14.0),
                        BG_CARD,
                        Stroke::new(2.0_f32, accent()),
                    );
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(box_rect.shrink(16.0))
                            .layout(egui::Layout::top_down(egui::Align::Center)),
                    );
                    let (ic_r, _) = child.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                    draw_icon(child.painter(), ic_r, Icon::Download, accent());
                    child.add_space(6.0);
                    child.label(
                        egui::RichText::new(tr(self.settings.language, "content.drop_hint"))
                            .strong()
                            .size(15.0)
                            .color(PURE_WHITE),
                    );
                    child.label(
                        egui::RichText::new(target_hint)
                            .small()
                            .color(TEXT_SECONDARY),
                    );
                });
        }

        let dropped: Vec<std::path::PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if dropped.is_empty() {
            return;
        }
        let Some(profile) = self.profiles.get(self.selected_idx).cloned() else {
            self.show_toast(ctx, "Create or select an instance before importing files");
            return;
        };
        let active_folder = self.content_folder.dir_name().to_string();
        let paths = self.paths.clone();
        let tx = self.tx.clone();
        self.mr_installing = true;
        self.progress = 0.08;
        self.status_text = format!("Importing {} file(s)...", dropped.len());
        thread::spawn(move || {
            for path in dropped {
                let res = import_local_file(&paths, &profile, &path, &active_folder, &tx);
                let _ = tx.send(WorkerEvent::ModrinthInstallFinished(res));
            }
        });
    }

    fn render_project_details_modal(&mut self, ctx: &egui::Context) {
        let Some(hit) = self.mr_detail_hit.clone() else {
            return;
        };
        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("mr_detail_backdrop"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    Rounding::ZERO,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 175),
                );
                if resp.clicked() {
                    self.mr_detail_hit = None;
                }
            });
        if self.mr_detail_hit.is_none() {
            return;
        }

        let max_w = (screen.width() - 40.0).clamp(320.0, 680.0);
        let max_h = (screen.height() - 56.0).clamp(280.0, 560.0);
        let mr_type = self.mr_type;
        let mr_installing = self.mr_installing;
        let target_label = self
            .profiles
            .get(self.selected_idx)
            .map(|p| format!("{} • {}", p.version_id, p.loader.kind().label()))
            .unwrap_or_else(|| "Default".to_string());

        let mut open = true;
        let mut install_version_req: Option<ModrinthVersion> = None;

        egui::Window::new(&hit.title)
            .id(egui::Id::new("mr_project_details_modal"))
            .order(egui::Order::Foreground)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .fixed_size(Vec2::new(max_w, max_h))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                let tex_opt = hit
                    .icon_url
                    .as_ref()
                    .and_then(|u| self.mr_icons.get(u))
                    .and_then(|o| o.as_ref());

                ui_toolbar_row(ui, 52.0, |ui| {
                    let icon_size = Vec2::splat(48.0);
                    if let Some(tex) = tex_opt {
                        ui.add(
                            egui::Image::new(tex)
                                .fit_to_exact_size(icon_size)
                                .rounding(Rounding::same(10.0)),
                        );
                    } else {
                        let (r, _) = ui.allocate_exact_size(icon_size, Sense::hover());
                        draw_icon_placeholder(ui, r, 10.0, &hit.title, false);
                    }
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(&hit.title)
                                .strong()
                                .size(15.5)
                                .color(PURE_WHITE),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "by {} • {} downloads",
                                hit.author,
                                format_downloads(hit.downloads)
                            ))
                            .small()
                            .color(TEXT_MUTED),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui_button_icon(
                            ui,
                            "mr_detail_ext_btn",
                            Some(Icon::External),
                            "Modrinth",
                            ButtonVariant::Subtle,
                            true,
                            28.0,
                        )
                        .clicked()
                        {
                            let url = format!(
                                "https://modrinth.com/{}/{}",
                                mr_type.api_str(),
                                hit.project_id
                            );
                            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                        }
                    });
                });

                ui.add_space(6.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&hit.description)
                            .small()
                            .color(TEXT_SECONDARY),
                    )
                    .wrap(),
                );
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                ui_toolbar_row(ui, 26.0, |ui| {
                    ui.label(
                        egui::RichText::new(tr(self.settings.language, "mr.compat_versions"))
                            .strong()
                            .small()
                            .color(TEXT_SECONDARY),
                    );
                    badge(ui, &target_label, true);
                });
                ui.add_space(6.0);

                let list_h = (ui.available_height() - 8.0).max(120.0);
                if self.mr_detail_loading {
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), list_h),
                        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                        |ui| {
                            ui.horizontal(|ui| {
                                mono_spinner(ui, 20.0);
                                ui.label(
                                    egui::RichText::new(tr(self.settings.language, "mr.loading_versions"))
                                        .color(TEXT_SECONDARY),
                                );
                            });
                        },
                    );
                } else if let Some(Err(err)) = &self.mr_detail_versions {
                    ui_alert_banner(ui, AlertKind::Error, err, ui.available_width());
                } else if let Some(Ok(versions)) = &self.mr_detail_versions {
                    if versions.is_empty() {
                        ui_alert_banner(
                            ui,
                            AlertKind::Info,
                            "No compatible versions matched this instance's Minecraft version and loader.",
                            ui.available_width(),
                        );
                    } else {
                        egui::ScrollArea::vertical()
                            .id_salt("mr_detail_versions_scroll")
                            .max_height(list_h)
                            .show(ui, |ui| {
                                let inner_w = (ui.available_width() - 8.0).max(200.0);
                                for ver in versions.iter().take(45) {
                                    card_frame(false, 0.0).show(ui, |ui| {
                                        ui.set_min_width(inner_w);
                                        ui_toolbar_row(ui, 30.0, |ui| {
                                            let title_str = if ver.name.trim().is_empty() {
                                                &ver.version_number
                                            } else {
                                                &ver.name
                                            };
                                            let ch_accent = ver.version_type == "release";
                                            let ch_lbl = if ver.version_type.is_empty() {
                                                "release"
                                            } else {
                                                ver.version_type.as_str()
                                            };
                                            let file_lbl = ver
                                                .files
                                                .first()
                                                .map(|f| f.filename.as_str())
                                                .unwrap_or_default();
                                            let date_lbl =
                                                format_relative_time(Some(&ver.date_published))
                                                    .unwrap_or_default();

                                            ui.vertical(|ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(title_str)
                                                            .strong()
                                                            .color(PURE_WHITE),
                                                    );
                                                    badge(ui, ch_lbl, ch_accent);
                                                });
                                                let meta = [
                                                    ver.version_number.as_str(),
                                                    date_lbl.as_str(),
                                                    file_lbl,
                                                ]
                                                .into_iter()
                                                .filter(|s| !s.is_empty())
                                                .collect::<Vec<_>>()
                                                .join(" • ");
                                                ui.label(
                                                    egui::RichText::new(meta)
                                                        .small()
                                                        .color(TEXT_MUTED),
                                                );
                                            });

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if ui_button_sized(
                                                        ui,
                                                        ("mr_ver_inst", &ver.id),
                                                        Some(Icon::Download),
                                                        if mr_type == ModrinthProjectType::Modpack
                                                        {
                                                            "Create"
                                                        } else {
                                                            tr(self.settings.language, "mr.install")
                                                        },
                                                        if ch_accent {
                                                            ButtonVariant::Primary
                                                        } else {
                                                            ButtonVariant::Secondary
                                                        },
                                                        !mr_installing,
                                                        Vec2::new(88.0, 28.0),
                                                    )
                                                    .clicked()
                                                    {
                                                        install_version_req = Some(ver.clone());
                                                    }
                                                },
                                            );
                                        });
                                    });
                                    ui.add_space(4.0);
                                }
                            });
                    }
                }
            });

        if !open {
            self.mr_detail_hit = None;
        }
        if let Some(version) = install_version_req {
            if let Some(profile) = self.profiles.get(self.selected_idx).cloned() {
                self.mr_installing = true;
                self.mr_installing_id = Some(hit.project_id.clone());
                self.mr_error = None;
                self.mr_status =
                    Some(format!("Installing {} ({})...", hit.title, version.version_number));
                self.progress = 0.05;
                self.mr_detail_hit = None;
                let paths = self.paths.clone();
                let tx = self.tx.clone();
                let pid = hit.project_id.clone();
                let title = hit.title.clone();
                let icon_url = hit.icon_url.clone();
                thread::spawn(move || {
                    let res = install_specific_version(
                        &paths,
                        &profile,
                        &pid,
                        mr_type,
                        &title,
                        icon_url.as_deref(),
                        &version,
                        &tx,
                    );
                    let _ = tx.send(WorkerEvent::ModrinthInstallFinished(res));
                });
            }
        }
    }

    fn render_command_palette_modal(&mut self, ctx: &egui::Context) {
        let modal_t = ctx.animate_bool_with_time(
            egui::Id::new("cmd_palette_anim"),
            self.show_cmd_palette,
            0.13,
        );
        if modal_t > 0.01 && modal_t < 0.99 {
            ctx.request_repaint();
        }
        if !self.show_cmd_palette && modal_t <= 0.01 {
            return;
        }

        #[derive(Clone, Copy)]
        enum PaletteAction {
            Page(NavPage),
            DiscoverType(ModrinthProjectType),
            ContentFolder(ContentFolder),
            InstanceTab(InstanceSubTab),
            LaunchSelected,
            NewInstance,
            CheckUpdates,
            OpenInstanceFolder,
            CopyLogs,
            ClearLogs,
            SelectInstance(usize),
            SelectAndLaunch(usize),
        }

        struct PaletteItem {
            category: PaletteCategory,
            icon: Icon,
            title: String,
            subtitle: String,
            keywords: &'static str,
            shortcut: &'static str,
            action: PaletteAction,
        }

        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("cmd_palette_backdrop"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    Rounding::ZERO,
                    Color32::from_rgba_unmultiplied(0, 0, 0, (modal_t * 170.0) as u8),
                );
                if resp.clicked() {
                    self.show_cmd_palette = false;
                }
            });
        if !self.show_cmd_palette {
            return;
        }

        let active_prof_summary = self
            .profiles
            .get(self.selected_idx)
            .map(|p| format!("{} ({})", p.name, p.version_id))
            .unwrap_or_else(|| "No instance selected".to_string());

        let mut items: Vec<PaletteItem> = vec![
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Grid,
                title: "Library — All Instances".to_string(),
                subtitle: "Browse, pin, sort, and spotlight instances".to_string(),
                keywords: "library home instances profiles библиотека главная инстансы профили",
                shortcut: "Ctrl+1",
                action: PaletteAction::Page(NavPage::Library),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Compass,
                title: "Discover — Browse Mods".to_string(),
                subtitle: "Search & install compatible Modrinth mods".to_string(),
                keywords: "discover search modrinth mods каталог поиск моды модрич",
                shortcut: "Ctrl+2",
                action: PaletteAction::DiscoverType(ModrinthProjectType::Mod),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Sparkle,
                title: "Discover — Browse Resourcepacks".to_string(),
                subtitle: "Search & install Modrinth resource packs".to_string(),
                keywords: "discover resourcepacks textures ресурспаки текстуры",
                shortcut: "",
                action: PaletteAction::DiscoverType(ModrinthProjectType::Resourcepack),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Download,
                title: "Discover — Browse Modpacks".to_string(),
                subtitle: "Search & create instances from Modrinth .mrpack".to_string(),
                keywords: "discover modpacks mrpack сборки модпаки",
                shortcut: "",
                action: PaletteAction::DiscoverType(ModrinthProjectType::Modpack),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Layers,
                title: "Instance — Mods".to_string(),
                subtitle: "Toggle, update, or remove installed mods".to_string(),
                keywords: "instance content mods jar моды инстанс список модов",
                shortcut: "Ctrl+3",
                action: PaletteAction::ContentFolder(ContentFolder::Mods),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Grid,
                title: "Instance — Resourcepacks".to_string(),
                subtitle: "Manage resource packs in active instance".to_string(),
                keywords: "instance content resourcepacks textures ресурспаки текстуры",
                shortcut: "",
                action: PaletteAction::ContentFolder(ContentFolder::Resourcepacks),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Sparkle,
                title: "Instance — Shaderpacks".to_string(),
                subtitle: "Manage shader packs in active instance".to_string(),
                keywords: "instance content shaderpacks shaders шейдеры",
                shortcut: "",
                action: PaletteAction::ContentFolder(ContentFolder::Shaderpacks),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Folder,
                title: "Instance — Saved Worlds".to_string(),
                subtitle: "View worlds & open save directories".to_string(),
                keywords: "worlds saves singleplayer миры сохранения",
                shortcut: "",
                action: PaletteAction::InstanceTab(InstanceSubTab::Worlds),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Sparkle,
                title: "Instance — Screenshots Gallery".to_string(),
                subtitle: "Browse captured in-game screenshots".to_string(),
                keywords: "screenshots photos gallery скриншоты фото",
                shortcut: "",
                action: PaletteAction::InstanceTab(InstanceSubTab::Screenshots),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Sliders,
                title: "Instance — Settings (Version, Loader, Java, RAM)".to_string(),
                subtitle: "Configure MC version, loader, memory slider, JVM args, resolution, Quick Play".to_string(),
                keywords: "settings version loader fabric forge quilt neoforge vanilla ram java jvm gpu window server настройки опра память джава версии лоадер",
                shortcut: "",
                action: PaletteAction::InstanceTab(InstanceSubTab::Settings),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::Terminal,
                title: "Live Console & Crash Diagnostics".to_string(),
                subtitle: "Inspect stdout/stderr and automatic crash diagnostics".to_string(),
                keywords: "console logs crash terminal диагностика консоль логи краш",
                shortcut: "Ctrl+4",
                action: PaletteAction::Page(NavPage::Console),
            },
            PaletteItem {
                category: PaletteCategory::Menus,
                icon: Icon::User,
                title: "Accounts — Microsoft, Ely.by, Offline".to_string(),
                subtitle: "Switch or add player accounts".to_string(),
                keywords: "accounts profile microsoft ely offline login аккаунты вход ник",
                shortcut: "Ctrl+5",
                action: PaletteAction::Page(NavPage::Accounts),
            },
            PaletteItem {
                category: PaletteCategory::Actions,
                icon: Icon::Play,
                title: "Launch Active Instance".to_string(),
                subtitle: active_prof_summary,
                keywords: "play launch start run game играть запуск старт",
                shortcut: "",
                action: PaletteAction::LaunchSelected,
            },
            PaletteItem {
                category: PaletteCategory::Actions,
                icon: Icon::Plus,
                title: "Create New Instance".to_string(),
                subtitle: "Open instance creation dialog".to_string(),
                keywords: "new create profile instance создать новый профиль инстанс",
                shortcut: "Ctrl+N",
                action: PaletteAction::NewInstance,
            },
            PaletteItem {
                category: PaletteCategory::Actions,
                icon: Icon::Refresh,
                title: "Check Mod Updates".to_string(),
                subtitle: "Update all tracked Modrinth content in active instance".to_string(),
                keywords: "update check mods refresh обновить моды апдейт",
                shortcut: "",
                action: PaletteAction::CheckUpdates,
            },
            PaletteItem {
                category: PaletteCategory::Actions,
                icon: Icon::Folder,
                title: "Open Active Instance Folder".to_string(),
                subtitle: "Reveal .minecraft / instance directory in file manager".to_string(),
                keywords: "folder directory open files папка директория файлы",
                shortcut: "",
                action: PaletteAction::OpenInstanceFolder,
            },
            PaletteItem {
                category: PaletteCategory::Actions,
                icon: Icon::Copy,
                title: "Copy Console Logs".to_string(),
                subtitle: "Copy all buffered console log lines to clipboard".to_string(),
                keywords: "copy clipboard logs console скопировать логи буфер",
                shortcut: "",
                action: PaletteAction::CopyLogs,
            },
            PaletteItem {
                category: PaletteCategory::Actions,
                icon: Icon::Trash,
                title: "Clear Console Logs".to_string(),
                subtitle: "Clear all live launcher & game log entries".to_string(),
                keywords: "clear reset logs console очистить логи консоль",
                shortcut: "",
                action: PaletteAction::ClearLogs,
            },
        ];

        for (idx, p) in self.profiles.iter().enumerate() {
            items.push(PaletteItem {
                category: PaletteCategory::Instances,
                icon: Icon::Layers,
                title: format!("Open Instance: {}", p.name),
                subtitle: format!("{} • {}", p.version_id, p.loader.kind().label()),
                keywords: "instance profile switch открыть переключить профиль",
                shortcut: "",
                action: PaletteAction::SelectInstance(idx),
            });
            items.push(PaletteItem {
                category: PaletteCategory::Instances,
                icon: Icon::Play,
                title: format!("Play: {}", p.name),
                subtitle: format!(
                    "Switch & launch {} ({})",
                    p.version_id,
                    p.loader.kind().label()
                ),
                keywords: "play launch instance profile играть запустить",
                shortcut: "",
                action: PaletteAction::SelectAndLaunch(idx),
            });
        }

        let (key_down, key_up, key_enter, key_tab, key_esc) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Tab),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if key_esc {
            self.show_cmd_palette = false;
            return;
        }
        if key_tab {
            let next_idx = (PaletteCategory::ALL
                .iter()
                .position(|&c| c == self.cmd_palette_category)
                .unwrap_or(0)
                + 1)
                % PaletteCategory::ALL.len();
            self.cmd_palette_category = PaletteCategory::ALL[next_idx];
            self.cmd_palette_selected = 0;
        }

        let mut scored: Vec<(i32, usize, PaletteItem)> = items
            .into_iter()
            .enumerate()
            .filter(|(_, it)| {
                self.cmd_palette_category == PaletteCategory::All
                    || it.category == self.cmd_palette_category
            })
            .filter_map(|(order_idx, it)| {
                score_palette_match(
                    &self.cmd_palette_query,
                    &it.title,
                    &it.subtitle,
                    it.keywords,
                )
                .map(|sc| (sc, order_idx, it))
            })
            .collect();
        if !self.cmd_palette_query.trim().is_empty() {
            scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        }

        let total_matches = scored.len();
        if total_matches == 0 {
            self.cmd_palette_selected = 0;
        } else {
            if self.cmd_palette_selected >= total_matches {
                self.cmd_palette_selected = total_matches - 1;
            }
            if key_down {
                self.cmd_palette_selected = (self.cmd_palette_selected + 1) % total_matches;
            } else if key_up {
                self.cmd_palette_selected = if self.cmd_palette_selected == 0 {
                    total_matches - 1
                } else {
                    self.cmd_palette_selected - 1
                };
            }
        }
        let scroll_to_selection = key_down || key_up;

        let max_w = (screen.width() - 36.0).clamp(320.0, 580.0);
        let max_h = (screen.height() - 100.0).clamp(280.0, 440.0);
        let mut chosen: Option<PaletteAction> = None;

        egui::Window::new("Command Palette")
            .id(egui::Id::new("cmd_palette_modal"))
            .order(egui::Order::Foreground)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(Vec2::new(max_w, max_h))
            .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 58.0))
            .show(ctx, |ui| {
                // Top styled search input bar
                let bar_w = ui.available_width();
                let mut clear_clicked = false;
                let mut refocus_input = self.cmd_palette_just_opened || key_tab || key_up || key_down;
                egui::Frame::none()
                    .fill(BG_INPUT)
                    .stroke(Stroke::new(1.2_f32, with_alpha(accent(), 185)))
                    .rounding(Rounding::same(9.0))
                    .inner_margin(Margin::symmetric(10.0, 6.0))
                    .show(ui, |ui| {
                        ui.set_min_width((bar_w - 22.0).max(120.0));
                        ui_toolbar_row(ui, 26.0, |ui| {
                            let (ic_r, _) =
                                ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
                            draw_icon(ui.painter(), ic_r, Icon::Search, accent());

                            let right_reserve = if self.cmd_palette_query.is_empty() {
                                46.0
                            } else {
                                74.0
                            };
                            let edit_w = (ui.available_width() - right_reserve).max(90.0);
                            let edit_resp = ui.add(
                                egui::TextEdit::singleline(&mut self.cmd_palette_query)
                                    .id_salt("cmd_palette_search_edit")
                                    .frame(false)
                                    .hint_text(tr(self.settings.language, "palette.hint"))
                                    .desired_width(edit_w),
                            );
                            if edit_resp.changed() {
                                self.cmd_palette_selected = 0;
                            }
                            if refocus_input {
                                edit_resp.request_focus();
                                self.cmd_palette_just_opened = false;
                                refocus_input = false;
                            }
                            let _ = refocus_input;

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    badge(ui, "Esc", false);
                                    if !self.cmd_palette_query.is_empty()
                                        && ui_icon_button(
                                            ui,
                                            "cmd_palette_clear_btn",
                                            Icon::Close,
                                            "Clear search",
                                            ButtonVariant::Subtle,
                                            true,
                                            24.0,
                                        )
                                        .clicked()
                                    {
                                        clear_clicked = true;
                                    }
                                },
                            );
                        });
                    });
                if clear_clicked {
                    self.cmd_palette_query.clear();
                    self.cmd_palette_selected = 0;
                    self.cmd_palette_just_opened = true;
                }

                ui.add_space(6.0);
                // Category filter pills
                ui_wrap_row(ui, 26.0, |ui| {
                    for cat in PaletteCategory::ALL {
                        let active = self.cmd_palette_category == cat;
                        if ui_pill_tab(ui, ("cmd_cat", cat.label()), cat.label(), active).clicked()
                            && !active
                        {
                            self.cmd_palette_category = cat;
                            self.cmd_palette_selected = 0;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!("{total_matches} found"))
                                .small()
                                .color(TEXT_MUTED),
                        );
                    });
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                if key_enter {
                    if let Some((_, _, it)) = scored.get(self.cmd_palette_selected) {
                        chosen = Some(it.action);
                    }
                }

                let list_h = (ui.available_height() - 28.0).max(120.0);
                egui::ScrollArea::vertical()
                    .id_salt("cmd_palette_scroll")
                    .max_height(list_h)
                    .show(ui, |ui| {
                        if scored.is_empty() {
                            ui.add_space(16.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new(tr(self.settings.language, "palette.empty"))
                                        .color(TEXT_SECONDARY),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(
                                        tr(self.settings.language, "palette.empty_sub"),
                                    )
                                    .small()
                                    .color(TEXT_MUTED),
                                );
                            });
                            return;
                        }

                        for (idx, (_, _, it)) in scored.into_iter().enumerate() {
                            let is_sel = idx == self.cmd_palette_selected;
                            let row_id = ui.make_persistent_id(("cmd_row", idx));
                            let card_r = animated_card(ui, row_id, is_sel, |ui| {
                                ui.set_min_width((ui.available_width() - 8.0).max(180.0));
                                ui_toolbar_row(ui, 34.0, |ui| {
                                    let (ic_box, _) =
                                        ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                                    ui.painter().rect(
                                        ic_box,
                                        Rounding::same(7.0),
                                        if is_sel {
                                            with_alpha(accent(), 34)
                                        } else {
                                            BG_INPUT
                                        },
                                        Stroke::new(
                                            1.0_f32,
                                            if is_sel {
                                                with_alpha(accent(), 130)
                                            } else {
                                                BORDER_SUBTLE
                                            },
                                        ),
                                    );
                                    draw_icon(
                                        ui.painter(),
                                        ic_box.shrink(6.0),
                                        it.icon,
                                        if is_sel { accent() } else { TEXT_SECONDARY },
                                    );

                                    let shortcut_reserve = if it.shortcut.is_empty() { 70.0 } else { 130.0 };
                                    let text_w = (ui.available_width() - shortcut_reserve).max(100.0);
                                    ui.allocate_ui(Vec2::new(text_w, 34.0), |ui| {
                                        ui.vertical(|ui| {
                                            ui.spacing_mut().item_spacing.y = 0.0;
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(&it.title)
                                                        .strong()
                                                        .color(PURE_WHITE),
                                                )
                                                .truncate(),
                                            );
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(&it.subtitle)
                                                        .small()
                                                        .color(TEXT_MUTED),
                                                )
                                                .truncate(),
                                            );
                                        });
                                    });

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if !it.shortcut.is_empty() {
                                                badge(ui, it.shortcut, is_sel);
                                            }
                                            badge(ui, it.category.short_badge(), false);
                                        },
                                    );
                                });
                            });
                            let row_resp = card_r.response.interact(Sense::click());
                            if row_resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                self.cmd_palette_selected = idx;
                            }
                            if is_sel && scroll_to_selection {
                                row_resp.scroll_to_me(Some(egui::Align::Center));
                            }
                            if row_resp.clicked() {
                                chosen = Some(it.action);
                            }
                            ui.add_space(3.0);
                        }
                    });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
                ui_toolbar_row(ui, 20.0, |ui| {
                    ui.label(
                        egui::RichText::new("↑↓ Navigate   •   ↵ Open / Run   •   Tab Category   •   Esc Close")
                            .small()
                            .color(TEXT_MUTED),
                    );
                });
            });

        if let Some(act) = chosen {
            self.show_cmd_palette = false;
            match act {
                PaletteAction::Page(page) => self.switch_page(ctx, page),
                PaletteAction::DiscoverType(mr_type) => {
                    if self.mr_type != mr_type {
                        self.mr_type = mr_type;
                        self.mr_query.clear();
                        self.mr_results.clear();
                        self.trigger_modrinth_search(false);
                    }
                    self.switch_page(ctx, NavPage::Discover);
                }
                PaletteAction::ContentFolder(folder) => {
                    self.content_folder = folder;
                    self.instance_tab = InstanceSubTab::Content;
                    self.switch_page(ctx, NavPage::Instance);
                }
                PaletteAction::InstanceTab(tab) => {
                    self.instance_tab = tab;
                    self.switch_page(ctx, NavPage::Instance);
                }
                PaletteAction::LaunchSelected => self.start_launch(),
                PaletteAction::NewInstance => self.show_new_modal = true,
                PaletteAction::CheckUpdates => {
                    if let Some(profile) = self.profiles.get(self.selected_idx).cloned() {
                        self.mr_installing = true;
                        self.progress = 0.08;
                        self.status_text = "Checking mod updates...".to_string();
                        let paths = self.paths.clone();
                        let tx = self.tx.clone();
                        thread::spawn(move || {
                            let res = update_all_installed_projects(&paths, &profile, &tx);
                            let _ = tx.send(WorkerEvent::ModUpdatesFinished(res));
                        });
                    }
                }
                PaletteAction::OpenInstanceFolder => {
                    if let Some(profile) = self.profiles.get(self.selected_idx) {
                        open_in_file_manager(&instance_dir(&self.paths, profile));
                    }
                }
                PaletteAction::CopyLogs => {
                    let text: String = self
                        .logs
                        .iter()
                        .map(|e| format!("[{}] {}", e.timestamp, e.text))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ctx.copy_text(text);
                    self.show_toast(ctx, "Console logs copied to clipboard");
                }
                PaletteAction::ClearLogs => {
                    self.logs.clear();
                    self.show_toast(ctx, "Console logs cleared");
                }
                PaletteAction::SelectInstance(idx) => {
                    if idx < self.profiles.len() {
                        self.select_profile(idx);
                        self.switch_page(ctx, NavPage::Instance);
                    }
                }
                PaletteAction::SelectAndLaunch(idx) => {
                    if idx < self.profiles.len() {
                        self.select_profile(idx);
                        self.start_launch();
                    }
                }
            }
        }
    }
}

fn score_palette_match(query: &str, title: &str, subtitle: &str, keywords: &str) -> Option<i32> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Some(0);
    }
    let title_l = title.to_lowercase();
    let sub_l = subtitle.to_lowercase();
    let kw_l = keywords.to_lowercase();
    let mut total = 0_i32;

    for token in trimmed.split_whitespace() {
        let tok = token.to_lowercase();
        if title_l.starts_with(&tok) {
            total += 120;
        } else if title_l
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| w.starts_with(&tok))
        {
            total += 80;
        } else if title_l.contains(&tok) {
            total += 50;
        } else if sub_l.contains(&tok) || kw_l.contains(&tok) {
            total += 25;
        } else if is_subsequence(&tok, &title_l) {
            total += 15;
        } else {
            return None;
        }
    }
    Some(total)
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut it = haystack.chars();
    for ch in needle.chars() {
        if !it.any(|h| h == ch) {
            return false;
        }
    }
    true
}

/// Renders a vertical scroll area strictly clamped to `ui.available_height()` so content
/// can never expand past the bottom status bar or clip outside the viewport.
fn bounded_scroll<R>(
    ui: &mut egui::Ui,
    id_salt: &str,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::scroll_area::ScrollAreaOutput<R> {
    let max_h = ui.available_height().max(0.0);
    egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .auto_shrink([false, false])
        .min_scrolled_height(0.0)
        .max_height(max_h)
        .show(ui, add_contents)
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{bytes} B")
    }
}

fn format_downloads(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

fn format_relative_time(iso_opt: Option<&str>) -> Option<String> {
    let iso = iso_opt?;
    let dt = chrono::DateTime::parse_from_rfc3339(iso).ok()?;
    let secs = (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_seconds();
    if secs < 0 {
        return Some("just now".to_string());
    }
    if secs < 60 {
        Some("just now".to_string())
    } else if secs < 3600 {
        Some(format!("{}m ago", secs / 60))
    } else if secs < 86400 {
        Some(format!("{}h ago", secs / 3600))
    } else {
        Some(format!("{}d ago", secs / 86400))
    }
}

fn format_playtime(secs: u64) -> String {
    if secs == 0 {
        "0m".to_string()
    } else if secs < 60 {
        "<1m".to_string()
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        let hours = secs / 3600;
        let rem_mins = (secs % 3600) / 60;
        if rem_mins == 0 {
            format!("{hours}h")
        } else {
            format!("{hours}h {rem_mins}m")
        }
    }
}

fn diagnose_console_logs(logs: &VecDeque<LogEntry>) -> Option<String> {
    for entry in logs.iter().rev().take(300) {
        let line = entry.text.as_str();
        if line.contains("OutOfMemoryError")
            || line.contains("Java heap space")
            || line.contains("GC overhead limit exceeded")
        {
            return Some(
                "Crash Diagnostic — Out of Memory: increase allocated RAM in Instance Settings."
                    .to_string(),
            );
        }
        if line.contains("UnsupportedClassVersionError")
            || line.contains("compiled by a more recent version of the Java Runtime")
        {
            return Some(
                "Crash Diagnostic — Java Version Mismatch: configure a newer Java runtime (e.g. Java 17 or 21) in Instance Settings."
                    .to_string(),
            );
        }
        if line.contains("ModResolutionException")
            || line.contains("Could not find required mod")
            || line.contains("Missing or unsupported mandatory dependencies")
        {
            return Some(
                "Crash Diagnostic — Missing Mod Dependency: check the Content tab and install required library mods (such as Fabric API)."
                    .to_string(),
            );
        }
        if line.contains("MixinApplyError")
            || line.contains("MixinTransformerError")
            || line.contains("critical injection failure")
        {
            return Some(
                "Crash Diagnostic — Mixin Conflict: a mod failed during bytecode patching. Try toggling off recently added or incompatible mods."
                    .to_string(),
            );
        }
    }
    None
}

impl eframe::App for McLauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events(ctx);
        self.handle_shortcuts(ctx);

        let screen_w = ctx.screen_rect().width();
        let compact_sidebar = self
            .sidebar_collapsed_manual
            .unwrap_or(screen_w < 680.0);
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
                    .fill(BG_SIDEBAR)
                    .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                    .inner_margin(Margin::symmetric(if compact_pad { 10.0 } else { 14.0 }, 9.0)),
            )
            .show(ctx, |ui| {
                self.render_top_bar(ui, compact_sidebar);
            });

        // 2. Bottom Status / Progress Bar
        egui::TopBottomPanel::bottom("bottom_status_bar")
            .exact_height(34.0)
            .frame(
                egui::Frame::none()
                    .fill(BG_SIDEBAR)
                    .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                    .inner_margin(Margin::symmetric(if compact_pad { 10.0 } else { 14.0 }, 6.0)),
            )
            .show(ctx, |ui| {
                self.render_bottom_bar(ui);
            });

        // 3. Left navigation rail
        egui::SidePanel::left("nav_sidebar")
            .frame(
                egui::Frame::none()
                    .fill(BG_SIDEBAR)
                    .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
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
                    .fill(theme::BG_VOID)
                    .inner_margin(Margin::same(if compact_pad { 10.0 } else { 16.0 })),
            )
            .show(ctx, |ui| {
                let now = ctx.input(|i| i.time);
                let raw_t = ((now - self.page_switch_time) / 0.16).clamp(0.0, 1.0) as f32;
                let eased = 1.0 - (1.0 - raw_t).powi(3);
                if raw_t < 1.0 {
                    ctx.request_repaint();
                }
                ui.add_space((1.0 - eased) * 6.0);

                match self.nav_page {
                    NavPage::Library => self.render_library_page(ui),
                    NavPage::Discover => self.render_discover_page(ui),
                    NavPage::Instance => self.render_instance_page(ui),
                    NavPage::Console => self.render_console_page(ui),
                    NavPage::Accounts => self.render_accounts_page(ui),
                    NavPage::Settings => self.render_settings_page(ui, ctx),
                }
            });

        self.render_new_profile_modal(ctx);
        self.render_delete_confirm_modal(ctx);
        self.render_project_details_modal(ctx);
        self.render_command_palette_modal(ctx);
        self.handle_file_drop(ctx);
        self.render_toast(ctx);
    }
}

fn clamp_summary(s: &str, max_chars: usize) -> String {
    let normalized: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let count = normalized.chars().count();
    if count <= max_chars || max_chars <= 3 {
        return normalized;
    }
    let mut out: String = normalized.chars().take(max_chars - 1).collect();
    out.push('…');
    out
}

fn render_loader_kind_pills(ui: &mut egui::Ui, id_prefix: &str, loader: &mut ModLoader) -> bool {
    let mut changed = false;
    let current = loader.kind();
    ui_wrap_row(ui, 28.0, |ui| {
        for kind in LoaderKind::ALL {
            let active = current == kind;
            if ui_loader_pill(ui, (id_prefix, kind.label()), kind, active).clicked() && !active {
                *loader = ModLoader::from_kind(kind, String::new());
                changed = true;
            }
        }
    });
    changed
}

fn render_loader_version_picker(
    ui: &mut egui::Ui,
    combo_id: &str,
    width: f32,
    kind: LoaderKind,
    loader_version: &mut String,
    list: Option<&[LoaderVersionEntry]>,
) -> bool {
    let mut changed = false;
    ui_wrap_row(ui, 30.0, |ui| {
        let label = if loader_version.is_empty() {
            "Auto".to_string()
        } else {
            loader_version.clone()
        };
        egui::ComboBox::from_id_salt(combo_id)
            .selected_text(label)
            .width(width)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(loader_version, String::new(), "Auto")
                    .changed()
                {
                    changed = true;
                }
                if let Some(entries) = list {
                    if entries.is_empty() {
                        ui.label(format!("No {} builds for this MC version", kind.label()));
                    } else {
                        for item in entries.iter().take(40) {
                            let tag = if item.stable {
                                format!("{} (recommended)", item.version)
                            } else {
                                item.version.clone()
                            };
                            if ui
                                .selectable_value(loader_version, item.version.clone(), tag)
                                .changed()
                            {
                                changed = true;
                            }
                        }
                    }
                } else {
                    ui.label(format!("Loading {} versions...", kind.label()));
                }
            });
        if list.is_none() {
            mono_spinner(ui, 16.0);
        }
    });
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_playtime() {
        assert_eq!(format_playtime(0), "0m");
        assert_eq!(format_playtime(45), "<1m");
        assert_eq!(format_playtime(125), "2m");
        assert_eq!(format_playtime(3600), "1h");
        assert_eq!(format_playtime(3600 + 27 * 60), "1h 27m");
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
}
