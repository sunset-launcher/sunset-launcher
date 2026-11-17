use crate::models::{Account, AccountStore, LauncherPaths, LauncherSettings, ModLoader, Profile};
use std::fs;
use std::path::PathBuf;

/// Returns standard launcher data directories (e.g. ~/.local/share/mc-launcher on Linux).
pub fn get_launcher_paths() -> Result<LauncherPaths, String> {
    let base = dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "Could not determine local data directory".to_string())?;
    let root = base.join("mc-launcher");
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

/// Loads saved profiles from profiles.json, or creates a default profile if none exist.
pub fn load_profiles(paths: &LauncherPaths) -> Vec<Profile> {
    if !paths.profiles_file.exists() {
        let default_profile = Profile::new(
            "Default Vanilla".to_string(),
            "1.21.4".to_string(),
            ModLoader::Vanilla,
            "Player".to_string(),
        );
        let profiles = vec![default_profile];
        let _ = save_profiles(paths, &profiles);
        return profiles;
    }

    match fs::read_to_string(&paths.profiles_file) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
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
    let dir = paths.instances_dir.join(&profile.id);
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

/// Installs the Svarka Launcher icon and `.desktop` menu entry on Linux so the binary
/// appears in desktop application launchers with its icon.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn ensure_linux_desktop_entry(paths: &LauncherPaths) -> Result<PathBuf, String> {
    const LOGO_PNG: &[u8] = include_bytes!("../logo.png");
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to locate current executable: {e}"))?;
    let exe_escaped = exe_path.to_string_lossy().replace('"', "\\\"");

    let icon_path = paths.root.join("svarka-launcher.png");
    fs::write(&icon_path, LOGO_PNG)
        .map_err(|e| format!("Failed to write icon {}: {e}", icon_path.display()))?;

    let data_dir = dirs::data_local_dir()
        .ok_or_else(|| "Could not determine XDG data directory".to_string())?;
    let hicolor_dir = data_dir.join("icons/hicolor/512x512/apps");
    if fs::create_dir_all(&hicolor_dir).is_ok() {
        let _ = fs::write(hicolor_dir.join("svarka-launcher.png"), LOGO_PNG);
    }

    let apps_dir = data_dir.join("applications");
    fs::create_dir_all(&apps_dir)
        .map_err(|e| format!("Failed to create {}: {e}", apps_dir.display()))?;
    let desktop_path = apps_dir.join("svarka-launcher.desktop");
    let desktop_contents = format!(
        "[Desktop Entry]\n\
Type=Application\n\
Version=1.0\n\
Name=Svarka Launcher\n\
Comment=Native Minecraft launcher in Rust with Modrinth & Multi-Loader support\n\
Exec=\"{exe_escaped}\"\n\
Icon={}\n\
Terminal=false\n\
Categories=Game;\n\
StartupNotify=true\n\
StartupWMClass=Svarka Launcher\n",
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
    let ico_path = paths.root.join("svarka-launcher.ico");
    let ico_bytes: &[u8] = include_bytes!("../logo.ico");
    let needs_write = match fs::read(&ico_path) {
        Ok(existing) => existing != ico_bytes,
        Err(_) => true,
    };
    if needs_write {
        fs::write(&ico_path, ico_bytes)
            .map_err(|e| format!("Failed to write {}: {e}", ico_path.display()))?;
    }
    Ok(ico_path)
}

/// Ensures a Start Menu shortcut (`Svarka Launcher.lnk`) with a crisp `.ico` exists on Windows.
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
    let lnk_path = programs_dir.join("Svarka Launcher.lnk");

    let stamp_path = paths.root.join(".windows-shortcut-target");
    let prev_target = fs::read_to_string(&stamp_path).unwrap_or_default();
    let expected_stamp = format!("{exe_str}\n{}", ico_path.display());
    if lnk_path.exists() && prev_target == expected_stamp {
        return Ok(lnk_path);
    }

    let script = "$ws=New-Object -ComObject WScript.Shell;$s=$ws.CreateShortcut($env:SVARKA_LNK);$s.TargetPath=$env:SVARKA_EXE;$s.WorkingDirectory=$env:SVARKA_DIR;$s.IconLocation=$env:SVARKA_ICO;$s.Description='Svarka Launcher';$s.Save()";
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .env("SVARKA_LNK", &lnk_path)
        .env("SVARKA_EXE", &exe_str)
        .env("SVARKA_DIR", &work_dir)
        .env("SVARKA_ICO", &ico_path)
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
}

