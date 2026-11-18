use crate::models::{Account, AccountStore, LauncherPaths, LauncherSettings, ModLoader, Profile};
use std::fs;
use std::path::{Path, PathBuf};

/// Returns standard launcher data directories (e.g. ~/.local/share/sunset-launcher on Linux).
pub fn get_launcher_paths() -> Result<LauncherPaths, String> {
    let base = dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "Could not determine local data directory".to_string())?;
    let root = base.join("sunset-launcher");
    let legacy_root = base.join("mc-launcher");
    if !root.exists() && legacy_root.is_dir() {
        let _ = fs::rename(&legacy_root, &root);
    }
    let paths = LauncherPaths {
        versions_dir: root.join("versions"),
        libraries_dir: root.join("libraries"),
        assets_dir: root.join("assets"),
        instances_dir: root.join("instances"),
        profiles_file: root.join("profiles.json"),
        accounts_file: root.join("accounts.json"),
        root,
    };

    for dir in [
        &paths.root,
        &paths.versions_dir,
        &paths.libraries_dir,
        &paths.assets_dir,
        &paths.assets_dir.join("indexes"),
        &paths.assets_dir.join("objects"),
        &paths.instances_dir,
    ] {
        fs::create_dir_all(dir)
            .map_err(|e| format!("Failed to create directory {}: {e}", dir.display()))?;
    }

    Ok(paths)
}

/// Converts an instance display name into a safe, human-readable directory name.
pub fn sanitize_instance_dir_name(name: &str) -> String {
    let mut cleaned = String::with_capacity(name.len());
    let mut prev_space = false;
    for ch in name.trim().chars() {
        let mapped = if ch.is_control()
            || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
        {
            '-'
        } else {
            ch
        };
        if mapped.is_whitespace() {
            if !prev_space {
                cleaned.push(' ');
                prev_space = true;
            }
        } else {
            cleaned.push(mapped);
            prev_space = false;
        }
    }

    let trimmed = cleaned.trim_matches(|c: char| c.is_whitespace() || c == '.');
    let mut truncated = String::new();
    for ch in trimmed.chars().take(80) {
        truncated.push(ch);
    }
    let final_name = truncated
        .trim_matches(|c: char| c.is_whitespace() || c == '.')
        .to_string();

    if final_name.is_empty() || final_name == "." || final_name == ".." {
        return "instance".to_string();
    }

    let upper = final_name.to_ascii_uppercase();
    let is_windows_reserved = matches!(
        upper.as_str(),
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
        format!("{final_name}_instance")
    } else {
        final_name
    }
}

fn is_dir_name_taken(
    paths: &LauncherPaths,
    candidate: &str,
    existing: &[Profile],
    ignore_profile_id: Option<&str>,
) -> bool {
    let used_by_profile = existing.iter().any(|p| {
        ignore_profile_id != Some(p.id.as_str())
            && !p.dir_name.is_empty()
            && p.dir_name.eq_ignore_ascii_case(candidate)
    });
    used_by_profile || paths.instances_dir.join(candidate).exists()
}

/// Picks a unique, human-readable folder name inside `instances/` for a profile.
pub fn unique_instance_dir_name(
    paths: &LauncherPaths,
    name: &str,
    existing: &[Profile],
) -> String {
    unique_instance_dir_name_excluding(paths, name, existing, None)
}

fn unique_instance_dir_name_excluding(
    paths: &LauncherPaths,
    name: &str,
    existing: &[Profile],
    ignore_profile_id: Option<&str>,
) -> String {
    let base = sanitize_instance_dir_name(name);
    if !is_dir_name_taken(paths, &base, existing, ignore_profile_id) {
        return base;
    }
    for n in 2.. {
        let candidate = format!("{base} ({n})");
        if !is_dir_name_taken(paths, &candidate, existing, ignore_profile_id) {
            return candidate;
        }
    }
    unreachable!()
}

fn rewrite_path_prefix(path_str: &mut String, old_dir: &Path, new_dir: &Path) {
    let trimmed = path_str.trim();
    if trimmed.is_empty() {
        return;
    }
    if let Ok(rel) = Path::new(trimmed).strip_prefix(old_dir) {
        *path_str = new_dir.join(rel).to_string_lossy().to_string();
    }
}

/// Migrates legacy UUID-named instance folders (`instances/<id>`) to readable names
/// and updates `dir_name` on profiles that don't have one yet.
pub fn migrate_profile_instance_dirs(paths: &LauncherPaths, profiles: &mut [Profile]) -> bool {
    let mut changed = false;
    if let Some(parent) = paths.root.parent() {
        let legacy_root = parent.join("mc-launcher");
        for p in profiles.iter_mut() {
            let old_icon = p.icon_path.clone();
            let old_java = p.java_path.clone();
            rewrite_path_prefix(&mut p.icon_path, &legacy_root, &paths.root);
            rewrite_path_prefix(&mut p.java_path, &legacy_root, &paths.root);
            if p.icon_path != old_icon || p.java_path != old_java {
                changed = true;
            }
        }
    }

    for idx in 0..profiles.len() {
        if !profiles[idx].dir_name.is_empty() {
            continue;
        }
        let pid = profiles[idx].id.clone();
        let pname = profiles[idx].name.clone();
        let target = unique_instance_dir_name_excluding(paths, &pname, profiles, Some(&pid));
        let legacy_dir = paths.instances_dir.join(&pid);
        let new_dir = paths.instances_dir.join(&target);

        let assigned = if legacy_dir.is_dir() {
            if !new_dir.exists() && fs::rename(&legacy_dir, &new_dir).is_ok() {
                rewrite_path_prefix(&mut profiles[idx].icon_path, &legacy_dir, &new_dir);
                target
            } else {
                pid
            }
        } else {
            target
        };

        profiles[idx].dir_name = assigned;
        changed = true;
    }

    changed
}

/// Renames a profile's folder in `instances/` to match its updated `name` when safe.
pub fn rename_instance_dir_for_profile(
    paths: &LauncherPaths,
    profile: &mut Profile,
    all_profiles: &[Profile],
) -> bool {
    let target = unique_instance_dir_name_excluding(
        paths,
        &profile.name,
        all_profiles,
        Some(&profile.id),
    );
    if profile.dir_name == target {
        return false;
    }
    let old_key = if profile.dir_name.is_empty() {
        profile.id.clone()
    } else {
        profile.dir_name.clone()
    };
    let old_dir = paths.instances_dir.join(&old_key);
    let new_dir = paths.instances_dir.join(&target);

    if old_dir.is_dir() {
        if new_dir.exists() || fs::rename(&old_dir, &new_dir).is_err() {
            return false;
        }
        rewrite_path_prefix(&mut profile.icon_path, &old_dir, &new_dir);
    }
    profile.dir_name = target;
    true
}

/// Loads saved profiles from profiles.json, or creates a default profile if none exist.
pub fn load_profiles(paths: &LauncherPaths) -> Vec<Profile> {
    if !paths.profiles_file.exists() {
        let mut default_profile = Profile::new(
            "Default Vanilla".to_string(),
            "1.21.4".to_string(),
            ModLoader::Vanilla,
            "Player".to_string(),
        );
        default_profile.dir_name = unique_instance_dir_name(paths, &default_profile.name, &[]);
        let profiles = vec![default_profile];
        let _ = save_profiles(paths, &profiles);
        return profiles;
    }

    let mut profiles: Vec<Profile> = match fs::read_to_string(&paths.profiles_file) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    if migrate_profile_instance_dirs(paths, &mut profiles) {
        let _ = save_profiles(paths, &profiles);
    }
    profiles
}

/// Persists profiles to profiles.json.
pub fn save_profiles(paths: &LauncherPaths, profiles: &[Profile]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(profiles)
        .map_err(|e| format!("Failed to serialize profiles: {e}"))?;
    fs::write(&paths.profiles_file, json)
        .map_err(|e| format!("Failed to write {}: {e}", paths.profiles_file.display()))
}

/// Returns the game directory for a specific profile instance.
pub fn instance_dir(paths: &LauncherPaths, profile: &Profile) -> PathBuf {
    let dir = if !profile.dir_name.is_empty() {
        paths.instances_dir.join(&profile.dir_name)
    } else {
        let legacy = paths.instances_dir.join(&profile.id);
        if legacy.exists() {
            legacy
        } else {
            paths
                .instances_dir
                .join(sanitize_instance_dir_name(&profile.name))
        }
    };
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Checks whether a version's client JAR and JSON are already present locally.
pub fn is_version_installed(paths: &LauncherPaths, version_id: &str) -> bool {
    let vdir = paths.versions_dir.join(version_id);
    vdir.join(format!("{version_id}.json")).exists()
        && vdir.join(format!("{version_id}.jar")).exists()
}

/// Loads saved accounts from accounts.json, creating a default offline account if empty.
pub fn load_accounts(paths: &LauncherPaths) -> AccountStore {
    if !paths.accounts_file.exists() {
        let default_acc = Account::offline("Player".to_string());
        let store = AccountStore {
            active_id: Some(default_acc.id.clone()),
            accounts: vec![default_acc],
        };
        let _ = save_accounts(paths, &store);
        return store;
    }

    match fs::read_to_string(&paths.accounts_file) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => AccountStore::default(),
    }
}

/// Persists accounts to accounts.json.
pub fn save_accounts(paths: &LauncherPaths, store: &AccountStore) -> Result<(), String> {
    let json = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Failed to serialize accounts: {e}"))?;
    fs::write(&paths.accounts_file, json)
        .map_err(|e| format!("Failed to write {}: {e}", paths.accounts_file.display()))
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct LauncherState {
    #[serde(default)]
    selected_profile_id: Option<String>,
}

fn state_file_path(paths: &LauncherPaths) -> PathBuf {
    paths.root.join("state.json")
}

/// Loads the previously selected profile ID from state.json, if present.
pub fn load_selected_profile_id(paths: &LauncherPaths) -> Option<String> {
    let content = fs::read_to_string(state_file_path(paths)).ok()?;
    serde_json::from_str::<LauncherState>(&content)
        .ok()
        .and_then(|s| s.selected_profile_id)
}

/// Persists the currently selected profile ID to state.json.
pub fn save_selected_profile_id(
    paths: &LauncherPaths,
    profile_id: Option<&str>,
) -> Result<(), String> {
    let path = state_file_path(paths);
    let state = LauncherState {
        selected_profile_id: profile_id.map(str::to_string),
    };
    let json = serde_json::to_string_pretty(&state)
        .map_err(|e| format!("Failed to serialize launcher state: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

/// Resolves the initial profile index from a persisted profile ID, falling back to 0.
pub fn resolve_selected_profile_index(profiles: &[Profile], saved_id: Option<&str>) -> usize {
    saved_id
        .and_then(|id| profiles.iter().position(|p| p.id == id))
        .unwrap_or(0)
}

/// Installs the Sunset Launcher icon and `.desktop` menu entry on Linux so the binary
/// appears in desktop application launchers with its icon.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn ensure_linux_desktop_entry(paths: &LauncherPaths) -> Result<PathBuf, String> {
    const LOGO_PNG: &[u8] = include_bytes!("../logo.png");
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to locate current executable: {e}"))?;
    let exe_escaped = exe_path.to_string_lossy().replace('"', "\\\"");

    let icon_path = paths.root.join("sunset-launcher.png");
    fs::write(&icon_path, LOGO_PNG)
        .map_err(|e| format!("Failed to write icon {}: {e}", icon_path.display()))?;
    let _ = fs::remove_file(paths.root.join("svarka-launcher.png"));

    let data_dir = dirs::data_local_dir()
        .ok_or_else(|| "Could not determine XDG data directory".to_string())?;
    let hicolor_dir = data_dir.join("icons/hicolor/512x512/apps");
    if fs::create_dir_all(&hicolor_dir).is_ok() {
        let _ = fs::write(hicolor_dir.join("sunset-launcher.png"), LOGO_PNG);
        let _ = fs::remove_file(hicolor_dir.join("svarka-launcher.png"));
    }

    let apps_dir = data_dir.join("applications");
    fs::create_dir_all(&apps_dir)
        .map_err(|e| format!("Failed to create {}: {e}", apps_dir.display()))?;
    let _ = fs::remove_file(apps_dir.join("svarka-launcher.desktop"));
    let desktop_path = apps_dir.join("sunset-launcher.desktop");
    let desktop_contents = format!(
        "[Desktop Entry]\n\
Type=Application\n\
Version=1.0\n\
Name=Sunset Launcher\n\
Comment=Native Minecraft launcher in Rust with Modrinth & Multi-Loader support\n\
Exec=\"{exe_escaped}\"\n\
Icon={}\n\
Terminal=false\n\
Categories=Game;\n\
StartupNotify=true\n\
StartupWMClass=Sunset Launcher\n",
        icon_path.display()
    );
    fs::write(&desktop_path, desktop_contents)
        .map_err(|e| format!("Failed to write {}: {e}", desktop_path.display()))?;
    Ok(desktop_path)
}

pub fn load_launcher_settings(paths: &LauncherPaths) -> LauncherSettings {
    let path = paths.root.join("settings.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save_launcher_settings(
    paths: &LauncherPaths,
    settings: &LauncherSettings,
) -> Result<(), String> {
    let path = paths.root.join("settings.json");
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

/// Ensures the multi-size Windows `.ico` icon file exists in the launcher root directory.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub fn ensure_windows_icon(paths: &LauncherPaths) -> Result<PathBuf, String> {
    let ico_path = paths.root.join("sunset-launcher.ico");
    let ico_bytes: &[u8] = include_bytes!("../logo.ico");
    let needs_write = match fs::read(&ico_path) {
        Ok(existing) => existing != ico_bytes,
        Err(_) => true,
    };
    if needs_write {
        fs::write(&ico_path, ico_bytes)
            .map_err(|e| format!("Failed to write {}: {e}", ico_path.display()))?;
    }
    let _ = fs::remove_file(paths.root.join("svarka-launcher.ico"));
    Ok(ico_path)
}

/// Ensures a Start Menu shortcut (`Sunset Launcher.lnk`) with a crisp `.ico` exists on Windows.
#[cfg(target_os = "windows")]
pub fn ensure_windows_shortcut(paths: &LauncherPaths) -> Result<PathBuf, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let exe_path =
        std::env::current_exe().map_err(|e| format!("Failed to locate executable: {e}"))?;
    let exe_str = exe_path.to_string_lossy().to_string();
    let work_dir = exe_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| paths.root.to_string_lossy().to_string());

    let ico_path = ensure_windows_icon(paths)?;
    let data_dir = dirs::data_dir()
        .or_else(dirs::data_local_dir)
        .ok_or_else(|| "Could not resolve AppData directory".to_string())?;
    let programs_dir = data_dir
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs");
    fs::create_dir_all(&programs_dir)
        .map_err(|e| format!("Failed to create {}: {e}", programs_dir.display()))?;
    let _ = fs::remove_file(programs_dir.join("Svarka Launcher.lnk"));
    let lnk_path = programs_dir.join("Sunset Launcher.lnk");

    let stamp_path = paths.root.join(".windows-shortcut-target");
    let prev_target = fs::read_to_string(&stamp_path).unwrap_or_default();
    let expected_stamp = format!("{exe_str}\n{}", ico_path.display());
    if lnk_path.exists() && prev_target == expected_stamp {
        return Ok(lnk_path);
    }

    let script = "$ws=New-Object -ComObject WScript.Shell;$s=$ws.CreateShortcut($env:SUNSET_LNK);$s.TargetPath=$env:SUNSET_EXE;$s.WorkingDirectory=$env:SUNSET_DIR;$s.IconLocation=$env:SUNSET_ICO;$s.Description='Sunset Launcher';$s.Save()";
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .env("SUNSET_LNK", &lnk_path)
        .env("SUNSET_EXE", &exe_str)
        .env("SUNSET_DIR", &work_dir)
        .env("SUNSET_ICO", &ico_path)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("Failed to invoke powershell for Start Menu shortcut: {e}"))?;
    if !status.success() {
        return Err(format!("PowerShell shortcut creation exited with {status}"));
    }

    let _ = fs::write(&stamp_path, expected_stamp);
    Ok(lnk_path)
}

/// Process-lifetime guard ensuring only one launcher instance uses the data directory at once.
#[cfg(unix)]
pub struct SingleInstanceGuard {
    _listener: std::os::unix::net::UnixListener,
    sock_path: PathBuf,
}

#[cfg(unix)]
impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.sock_path);
    }
}

#[cfg(windows)]
pub struct SingleInstanceGuard {
    _file: fs::File,
}

fn try_acquire_single_instance_lock(paths: &LauncherPaths) -> Option<SingleInstanceGuard> {
    let _ = fs::create_dir_all(&paths.root);
    #[cfg(unix)]
    {
        let sock_path = paths.root.join("instance.sock");
        match std::os::unix::net::UnixListener::bind(&sock_path) {
            Ok(listener) => Some(SingleInstanceGuard {
                _listener: listener,
                sock_path,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                if std::os::unix::net::UnixStream::connect(&sock_path).is_ok() {
                    None
                } else {
                    let _ = fs::remove_file(&sock_path);
                    std::os::unix::net::UnixListener::bind(&sock_path)
                        .ok()
                        .map(|listener| SingleInstanceGuard {
                            _listener: listener,
                            sock_path,
                        })
                }
            }
            Err(_) => None,
        }
    }
    #[cfg(windows)]
    {
        use std::io::Write;
        use std::os::windows::fs::OpenOptionsExt;
        let lock_path = paths.root.join("instance.lock");
        match fs::OpenOptions::new()
            .create(true)
            .write(true)
            .share_mode(0)
            .open(&lock_path)
        {
            Ok(mut file) => {
                let _ = write!(file, "{}", std::process::id());
                Some(SingleInstanceGuard { _file: file })
            }
            Err(_) => None,
        }
    }
}

/// Acquires the single-instance lock, optionally waiting briefly for an exiting previous process during update relaunch.
pub fn acquire_single_instance_lock(
    paths: &LauncherPaths,
    wait_for_relaunch: bool,
) -> Option<SingleInstanceGuard> {
    if let Some(guard) = try_acquire_single_instance_lock(paths) {
        return Some(guard);
    }
    if wait_for_relaunch {
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if let Some(guard) = try_acquire_single_instance_lock(paths) {
                return Some(guard);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths(label: &str) -> LauncherPaths {
        let root = std::env::temp_dir().join(format!(
            "mc_launcher_storage_{label}_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        LauncherPaths {
            versions_dir: root.join("versions"),
            libraries_dir: root.join("libraries"),
            assets_dir: root.join("assets"),
            instances_dir: root.join("instances"),
            profiles_file: root.join("profiles.json"),
            accounts_file: root.join("accounts.json"),
            root,
        }
    }

    #[test]
    fn selected_profile_id_roundtrip_and_resolution() {
        let paths = test_paths("selected_profile");
        assert_eq!(load_selected_profile_id(&paths), None);

        let p1 = Profile::new("One".into(), "1.21.4".into(), ModLoader::Vanilla, "A".into());
        let p2 = Profile::new("Two".into(), "1.20.1".into(), ModLoader::Vanilla, "B".into());
        let profiles = vec![p1.clone(), p2.clone()];

        save_selected_profile_id(&paths, Some(&p2.id)).unwrap();
        let loaded = load_selected_profile_id(&paths);
        assert_eq!(loaded.as_deref(), Some(p2.id.as_str()));
        assert_eq!(
            resolve_selected_profile_index(&profiles, loaded.as_deref()),
            1
        );
        assert_eq!(
            resolve_selected_profile_index(&profiles, Some("missing-id")),
            0
        );

        save_selected_profile_id(&paths, None).unwrap();
        assert_eq!(load_selected_profile_id(&paths), None);

        let _ = fs::remove_dir_all(&paths.root);
    }

    #[test]
    fn windows_icon_generation_produces_valid_256_ico() {
        let paths = test_paths("win_ico");
        let ico_path = ensure_windows_icon(&paths).unwrap();
        assert!(ico_path.exists());

        let bytes = fs::read(&ico_path).unwrap();
        assert!(bytes.len() > 22);
        assert_eq!(&bytes[0..6], &[0, 0, 1, 0, 5, 0]);

        let decoded =
            image::load_from_memory_with_format(&bytes, image::ImageFormat::Ico).unwrap();
        assert_eq!(decoded.width(), 256);
        assert_eq!(decoded.height(), 256);

        // Second call should remain valid and idempotent.
        let ico_path_again = ensure_windows_icon(&paths).unwrap();
        assert_eq!(ico_path_again, ico_path);
        assert_eq!(fs::read(&ico_path_again).unwrap(), bytes);

        let _ = fs::remove_dir_all(&paths.root);
    }

    #[test]
    fn single_instance_lock_prevents_duplicates_and_releases_on_drop() {
        let paths = test_paths("single_lock");
        let guard = acquire_single_instance_lock(&paths, false)
            .expect("first lock acquisition should succeed");
        assert!(acquire_single_instance_lock(&paths, false).is_none());

        drop(guard);
        let guard2 = acquire_single_instance_lock(&paths, false)
            .expect("lock should be re-acquirable after drop");
        drop(guard2);

        let _ = fs::remove_dir_all(&paths.root);
    }

    #[test]
    fn sanitize_and_unique_instance_dir_names() {
        assert_eq!(sanitize_instance_dir_name("My Instance 1.21"), "My Instance 1.21");
        assert_eq!(sanitize_instance_dir_name("Ванилла / Тест: 1.21*"), "Ванилла - Тест- 1.21-");
        assert_eq!(sanitize_instance_dir_name("  ...  "), "instance");
        assert_eq!(sanitize_instance_dir_name("CON"), "CON_instance");

        let paths = test_paths("unique_names");
        fs::create_dir_all(paths.instances_dir.join("Survival")).unwrap();
        let mut p1 = Profile::new("Survival".into(), "1.21.4".into(), ModLoader::Vanilla, "P".into());
        p1.dir_name = "Survival".into();
        let p2 = Profile::new("Survival".into(), "1.21.4".into(), ModLoader::Vanilla, "P".into());

        assert_eq!(
            unique_instance_dir_name(&paths, &p2.name, &[p1]),
            "Survival (2)"
        );

        let _ = fs::remove_dir_all(&paths.root);
    }

    #[test]
    fn load_profiles_migrates_legacy_uuid_dir_and_renames_cleanly() {
        let paths = test_paths("migrate_uuid_dir");
        fs::create_dir_all(&paths.instances_dir).unwrap();

        let mut p = Profile::new("Fabric 1.21".into(), "1.21.4".into(), ModLoader::Vanilla, "P".into());
        let legacy_dir = paths.instances_dir.join(&p.id);
        fs::create_dir_all(legacy_dir.join("mods")).unwrap();
        fs::write(legacy_dir.join("mods").join("sodium.jar"), b"jar").unwrap();
        p.icon_path = legacy_dir.join("icon.png").to_string_lossy().to_string();
        save_profiles(&paths, &[p.clone()]).unwrap();

        let mut loaded = load_profiles(&paths);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].dir_name, "Fabric 1.21");
        assert!(!legacy_dir.exists());
        assert!(paths.instances_dir.join("Fabric 1.21").join("mods").join("sodium.jar").is_file());
        assert_eq!(
            loaded[0].icon_path,
            paths.instances_dir.join("Fabric 1.21").join("icon.png").to_string_lossy()
        );

        loaded[0].name = "Better Fabric".into();
        assert!(rename_instance_dir_for_profile(&paths, &mut loaded[0], &[]));
        assert_eq!(loaded[0].dir_name, "Better Fabric");
        assert!(paths.instances_dir.join("Better Fabric").join("mods").join("sodium.jar").is_file());
        assert_eq!(
            loaded[0].icon_path,
            paths.instances_dir.join("Better Fabric").join("icon.png").to_string_lossy()
        );

        let _ = fs::remove_dir_all(&paths.root);
    }
}

