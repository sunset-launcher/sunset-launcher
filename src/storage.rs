use crate::models::{Account, AccountStore, LauncherPaths, ModLoader, Profile};
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
}

