use crate::models::{
    Account, AccountKind, GameHandle, JavaInstall, LauncherPaths, Profile, Rule, WorkerEvent,
};
use crate::mojang::{rules_allow, PreparedLaunch};
use crate::storage::instance_dir;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const JAVA_BIN: &str = if cfg!(windows) { "java.exe" } else { "java" };

/// Configures a background/piped child process so Windows does not flash a console window.
pub(crate) fn configure_background_command(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// Computes Minecraft's standard offline-mode UUID v3 (`OfflinePlayer:<username>`).
pub fn offline_uuid(username: &str) -> String {
    let input = format!("OfflinePlayer:{username}");
    let digest = md5::compute(input.as_bytes());
    let mut bytes = digest.0;
    // Set version to 3 (0011) and variant to RFC 4122 (10xx)
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).simple().to_string()
}

/// Parses the major version from `java -version` output
/// (`"1.8.0_402"` -> 8, `"21.0.8"` -> 21, `"25"` -> 25).
pub fn parse_java_major(version_output: &str) -> Option<u32> {
    let start = version_output.find('"')?;
    let rest = &version_output[start + 1..];
    let ver = &rest[..rest.find('"')?];
    let mut parts = ver.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

fn java_major_version(bin: &Path) -> Option<u32> {
    let mut cmd = Command::new(bin);
    cmd.arg("-version");
    configure_background_command(&mut cmd);
    let out = cmd.output().ok()?;
    let mut text = String::from_utf8_lossy(&out.stderr).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stdout));
    parse_java_major(&text)
}

fn push_java_vendor_dirs(root: &Path, candidates: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(root) {
        let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        dirs.sort();
        for dir in dirs {
            candidates.push(dir.join("bin").join(JAVA_BIN));
            candidates.push(dir.join("jre").join("bin").join(JAVA_BIN));
            candidates.push(dir.join("Contents").join("Home").join("bin").join(JAVA_BIN));
        }
    }
}

/// Finds Java runtimes in JAVA_HOME, standard OS vendor locations, and PATH.
pub fn detect_java_installations() -> Vec<JavaInstall> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(home) = std::env::var("JAVA_HOME") {
        candidates.push(PathBuf::from(home).join("bin").join(JAVA_BIN));
    }

    if cfg!(windows) {
        let vendors = [
            "Java",
            "Eclipse Adoptium",
            "Microsoft",
            "BellSoft",
            "Zulu",
            "Amazon Corretto",
            "AdoptOpenJDK",
        ];
        for env_key in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(base) = std::env::var_os(env_key) {
                let base_path = PathBuf::from(base);
                for vendor in vendors {
                    push_java_vendor_dirs(&base_path.join(vendor), &mut candidates);
                }
            }
        }
        if let Some(local_app) = std::env::var_os("LOCALAPPDATA") {
            let programs = PathBuf::from(local_app).join("Programs");
            for vendor in vendors {
                push_java_vendor_dirs(&programs.join(vendor), &mut candidates);
            }
        }
    } else if cfg!(target_os = "macos") {
        push_java_vendor_dirs(Path::new("/Library/Java/JavaVirtualMachines"), &mut candidates);
    } else {
        for root in ["/usr/lib/jvm", "/usr/lib64/jvm", "/opt/java"] {
            push_java_vendor_dirs(Path::new(root), &mut candidates);
        }
    }

    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            candidates.push(dir.join(JAVA_BIN));
        }
    }

    let mut found: Vec<JavaInstall> = Vec::new();
    let mut seen_real: Vec<PathBuf> = Vec::new();
    for cand in candidates {
        if !cand.is_file() {
            continue;
        }
        // Skip symlinks that point to an already-detected runtime (e.g. /usr/bin/java).
        let real = std::fs::canonicalize(&cand).unwrap_or_else(|_| cand.clone());
        if seen_real.contains(&real) {
            continue;
        }
        seen_real.push(real);
        if let Some(major) = java_major_version(&cand) {
            found.push(JavaInstall { path: cand, major });
        }
    }
    found
}

/// Picks the Java binary for a profile: explicit path, else a runtime matching the
/// version's required major (exact match preferred, otherwise the closest newer one).
pub fn resolve_java_binary(profile: &Profile, required: Option<u32>) -> Result<PathBuf, String> {
    let install_hint = |n: u32| {
        if cfg!(windows) {
            format!("Install it (e.g. `winget install EclipseAdoptium.Temurin.{n}.JRE`) or specify javaw.exe/java.exe in Instance settings.")
        } else {
            format!("Install it (e.g. `sudo xbps-install -S openjdk{n}-jre`) or specify the java path in Instance settings.")
        }
    };

    if !profile.java_path.trim().is_empty() {
        let p = PathBuf::from(profile.java_path.trim());
        if !p.exists() {
            return Err(format!("Configured Java path not found: {}", p.display()));
        }
        if let (Some(req), Some(major)) = (required, java_major_version(&p)) {
            if major < req {
                return Err(format!(
                    "This Minecraft version requires Java {req}+, but the instance is configured with Java {major}. {}",
                    install_hint(req)
                ));
            }
        }
        return Ok(p);
    }

    let installs = detect_java_installations();
    if installs.is_empty() {
        return Err(format!("Java runtime not found. {}", install_hint(required.unwrap_or(21))));
    }

    let chosen = match required {
        Some(req) => installs
            .iter()
            .find(|j| j.major == req)
            .or_else(|| {
                installs
                    .iter()
                    .filter(|j| j.major > req)
                    .min_by_key(|j| j.major)
            }),
        None => installs.iter().max_by_key(|j| j.major),
    };

    match chosen {
        Some(j) => Ok(j.path.clone()),
        None => {
            let have: Vec<String> = installs.iter().map(|j| j.major.to_string()).collect();
            let req = required.unwrap_or(21);
            Err(format!(
                "Java {req}+ is required, only found: {}. {}",
                have.join(", "),
                install_hint(req)
            ))
        }
    }
}

fn replace_placeholders(arg: &str, vars: &HashMap<&str, String>) -> String {
    let mut result = arg.to_string();
    for (key, val) in vars {
        let token = format!("${{{key}}}");
        if result.contains(&token) {
            result = result.replace(&token, val);
        }
    }
    result
}

fn collect_arg_values(
    entries: &[serde_json::Value],
    vars: &HashMap<&str, String>,
    out: &mut Vec<String>,
) {
    for entry in entries {
        match entry {
            serde_json::Value::String(s) => {
                out.push(replace_placeholders(s, vars));
            }
            serde_json::Value::Object(obj) => {
                let allowed = obj
                    .get("rules")
                    .and_then(|r| serde_json::from_value::<Vec<Rule>>(r.clone()).ok())
                    .map_or(true, |rules| rules_allow(Some(&rules)));
                if !allowed {
                    continue;
                }
                match obj.get("value") {
                    Some(serde_json::Value::String(s)) => out.push(replace_placeholders(s, vars)),
                    Some(serde_json::Value::Array(arr)) => {
                        for item in arr.iter().filter_map(|i| i.as_str()) {
                            out.push(replace_placeholders(item, vars));
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

/// Launches Minecraft in a child process and streams stdout/stderr to the UI.
/// Returns a handle that lets the UI stop the game.
pub fn launch_game(
    paths: &LauncherPaths,
    profile: &Profile,
    account: &Account,
    prepared: &PreparedLaunch,
    authlib_injector: Option<&Path>,
    tx: Sender<WorkerEvent>,
) -> Result<GameHandle, String> {
    let required_java = prepared
        .version_details
        .java_version
        .as_ref()
        .map(|j| j.major_version);
    let java_bin = resolve_java_binary(profile, required_java)?;
    let game_dir = instance_dir(paths, profile);

    let sep = if cfg!(windows) { ";" } else { ":" };
    let classpath_str = prepared
        .classpath
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(sep);

    let (username, uuid, access_token, user_type) = match &account.kind {
        AccountKind::Offline => {
            let name = if account.username.trim().is_empty() {
                "Player"
            } else {
                account.username.trim()
            };
            (name.to_string(), offline_uuid(name), "0".to_string(), "legacy")
        }
        AccountKind::Microsoft {
            uuid, access_token, ..
        } => (account.username.clone(), uuid.clone(), access_token.clone(), "msa"),
        AccountKind::ElyBy {
            uuid, access_token, ..
        } => (account.username.clone(), uuid.clone(), access_token.clone(), "mojang"),
    };

    let assets_index_name = prepared
        .version_details
        .asset_index
        .as_ref()
        .map(|a| a.id.clone())
        .or_else(|| prepared.version_details.assets.clone())
        .unwrap_or_else(|| "legacy".to_string());
    let version_type = prepared
        .version_details
        .version_type
        .clone()
        .unwrap_or_else(|| "release".to_string());
    let assets_root = paths.assets_dir.to_string_lossy().to_string();

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", username);
    vars.insert("version_name", prepared.version_details.id.clone());
    vars.insert("game_directory", game_dir.to_string_lossy().to_string());
    vars.insert("assets_root", assets_root.clone());
    vars.insert("game_assets", assets_root);
    vars.insert("assets_index_name", assets_index_name);
    vars.insert("auth_uuid", uuid);
    vars.insert("auth_session", access_token.clone());
    vars.insert("auth_access_token", access_token);
    vars.insert("user_properties", "{}".to_string());
    vars.insert("clientid", "0".to_string());
    vars.insert("auth_xuid", "0".to_string());
    vars.insert("user_type", user_type.to_string());
    vars.insert("version_type", version_type);
    vars.insert(
        "natives_directory",
        prepared.natives_dir.to_string_lossy().to_string(),
    );
    vars.insert("launcher_name", "Svarka Launcher".to_string());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").to_string());
    vars.insert("classpath", classpath_str.clone());
    vars.insert("classpath_separator", sep.to_string());
    vars.insert("library_directory", paths.libraries_dir.to_string_lossy().to_string());

    let mut cmd_args: Vec<String> = Vec::new();
    let max_ram = profile.max_ram_mb.max(512);
    cmd_args.push(format!("-Xms{}M", profile.min_ram_mb.clamp(256, max_ram)));
    cmd_args.push(format!("-Xmx{max_ram}M"));

    if let Some(agent) = authlib_injector {
        // Ely.by skins + joining Ely.by servers.
        cmd_args.push(format!("-javaagent:{}=ely.by", agent.display()));
    }

    cmd_args.extend(profile.jvm_args.split_whitespace().map(str::to_string));

    // JVM args from Mojang version JSON (or fallback for legacy versions)
    if let Some(jvm_entries) = prepared
        .version_details
        .arguments
        .as_ref()
        .and_then(|a| a.jvm.as_ref())
    {
        collect_arg_values(jvm_entries, &vars, &mut cmd_args);
    } else {
        cmd_args.push(format!(
            "-Djava.library.path={}",
            prepared.natives_dir.to_string_lossy()
        ));
        cmd_args.push("-cp".to_string());
        cmd_args.push(classpath_str);
    }

    if let Some(jvm_entries) = prepared
        .loader_overlay
        .as_ref()
        .and_then(|ov| ov.arguments.as_ref())
        .and_then(|a| a.jvm.as_ref())
    {
        collect_arg_values(jvm_entries, &vars, &mut cmd_args);
    }

    // Main class (mod loader overrides Vanilla)
    let main_class = prepared
        .loader_overlay
        .as_ref()
        .map(|ov| ov.main_class.clone())
        .unwrap_or_else(|| prepared.version_details.main_class.clone());
    cmd_args.push(main_class);

    // Game args
    if let Some(legacy_override) = prepared
        .loader_overlay
        .as_ref()
        .and_then(|ov| ov.minecraft_arguments.as_ref())
    {
        for token in legacy_override.split_whitespace() {
            cmd_args.push(replace_placeholders(token, &vars));
        }
    } else if let Some(game_entries) = prepared
        .version_details
        .arguments
        .as_ref()
        .and_then(|a| a.game.as_ref())
    {
        collect_arg_values(game_entries, &vars, &mut cmd_args);
    } else if let Some(legacy_args) = &prepared.version_details.minecraft_arguments {
        for token in legacy_args.split_whitespace() {
            cmd_args.push(replace_placeholders(token, &vars));
        }
    }

    if let Some(game_entries) = prepared
        .loader_overlay
        .as_ref()
        .and_then(|ov| ov.arguments.as_ref())
        .and_then(|a| a.game.as_ref())
    {
        collect_arg_values(game_entries, &vars, &mut cmd_args);
    }

    append_profile_launch_args(profile, &mut cmd_args);

    let _ = tx.send(WorkerEvent::Log(format!(
        "[Launcher] Starting {} ({}) with {} in {}",
        profile.name,
        profile.version_id,
        java_bin.display(),
        game_dir.display()
    )));

    let mut command = Command::new(&java_bin);
    command
        .args(&cmd_args)
        .current_dir(&game_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_background_command(&mut command);
    if profile.prefer_discrete_gpu {
        command
            .env("DRI_PRIME", "1")
            .env("__NV_PRIME_RENDER_OFFLOAD", "1")
            .env("__GLX_VENDOR_LIBRARY_NAME", "nvidia");
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to start Java ({}): {e}", java_bin.display()))?;

    if let Some(stdout) = child.stdout.take() {
        let tx_out = tx.clone();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let _ = tx_out.send(WorkerEvent::Log(line));
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let tx_err = tx.clone();
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let _ = tx_err.send(WorkerEvent::Log(line));
            }
        });
    }

    let handle: GameHandle = Arc::new(Mutex::new(child));
    let watcher = Arc::clone(&handle);
    thread::spawn(move || loop {
        // Poll instead of wait() so the UI can lock the child and kill it.
        let status = match watcher.lock() {
            Ok(mut child) => child.try_wait(),
            Err(_) => break,
        };
        match status {
            Ok(Some(s)) => {
                let _ = tx.send(WorkerEvent::GameExited(s.code()));
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(400)),
            Err(_) => {
                let _ = tx.send(WorkerEvent::GameExited(None));
                break;
            }
        }
    });

    Ok(handle)
}

/// Appends instance window resolution, fullscreen, and quick-play server flags to `cmd_args`.
pub fn append_profile_launch_args(profile: &Profile, cmd_args: &mut Vec<String>) {
    if profile.fullscreen {
        cmd_args.push("--fullscreen".to_string());
    }
    if let (Some(w), Some(h)) = (
        profile.window_width.filter(|&w| w >= 320),
        profile.window_height.filter(|&h| h >= 240),
    ) {
        cmd_args.push("--width".to_string());
        cmd_args.push(w.to_string());
        cmd_args.push("--height".to_string());
        cmd_args.push(h.to_string());
    }
    let addr = profile.server_address.trim();
    if !addr.is_empty() {
        cmd_args.push("--quickPlayMultiplayer".to_string());
        cmd_args.push(addr.to_string());
    }
}

/// Opens a folder or file in the system file manager / viewer.
pub fn open_in_file_manager(dir: &Path) {
    if !dir.is_file() {
        let _ = std::fs::create_dir_all(dir);
    }
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = Command::new(opener).arg(dir).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ModLoader, Profile};

    #[test]
    fn test_append_profile_launch_args_flags() {
        let mut profile = Profile::new(
            "Test".to_string(),
            "1.21.4".to_string(),
            ModLoader::Vanilla,
            "Player".to_string(),
        );
        profile.window_width = Some(1280);
        profile.window_height = Some(720);
        profile.fullscreen = true;
        profile.server_address = " play.example.com ".to_string();
        let mut args = Vec::new();
        append_profile_launch_args(&profile, &mut args);
        assert_eq!(
            args,
            vec![
                "--fullscreen",
                "--width",
                "1280",
                "--height",
                "720",
                "--quickPlayMultiplayer",
                "play.example.com",
            ]
        );
    }

    #[test]
    fn test_append_profile_launch_args_ignores_too_small_window() {
        let mut profile = Profile::new(
            "Test".to_string(),
            "1.21.4".to_string(),
            ModLoader::Vanilla,
            "Player".to_string(),
        );
        profile.window_width = Some(100);
        profile.window_height = Some(100);
        let mut args = Vec::new();
        append_profile_launch_args(&profile, &mut args);
        assert!(args.is_empty());
    }
}
