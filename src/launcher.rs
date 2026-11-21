use crate::models::{
    Account, AccountKind, AssetIndex, GameHandle, JavaInstall, LauncherPaths, Profile, Rule,
    WorkerEvent,
};
use crate::mojang::{
    check_not_cancelled, is_download_cancelled, rules_allow_with_features,
    sync_mapped_assets_to_dir, PreparedLaunch, RuleFeatures, CANCELLED_ERR_MSG,
};
use crate::storage::instance_dir;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
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

/// Splits a JVM argument string into tokens while respecting `'...'` and `"..."` quotes
/// (including inline flags like `-XX:HeapDumpPath="C:\My Games\dump.hprof"`), preserving
/// Windows backslashes unless escaping a quote (`\"`).
pub fn split_jvm_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut quote: Option<char> = None;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == '\\' && q == '"' && chars.peek() == Some(&'"') {
                    current.push(chars.next().unwrap());
                } else if c == q {
                    quote = None;
                } else {
                    current.push(c);
                }
            }
            None => {
                if c.is_whitespace() {
                    if in_token {
                        args.push(std::mem::take(&mut current));
                        in_token = false;
                    }
                } else if c == '"' || c == '\'' {
                    quote = Some(c);
                    in_token = true;
                } else {
                    current.push(c);
                    in_token = true;
                }
            }
        }
    }
    if in_token {
        args.push(current);
    }
    args
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

/// Finds Java runtimes in the launcher's managed `java/` folder, JAVA_HOME, standard OS vendor locations, and PATH.
pub fn detect_java_installations() -> Vec<JavaInstall> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(base) = dirs::data_local_dir().or_else(dirs::home_dir) {
        push_java_vendor_dirs(&base.join("sunset-launcher").join("java"), &mut candidates);
    }
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

fn adoptium_jre_url_for(
    major: u32,
    os: &str,
    arch: &str,
) -> Result<(&'static str, String), String> {
    let api_os = match os {
        "windows" => "windows",
        "macos" => "mac",
        "linux" => "linux",
        other => {
            return Err(format!(
                "Auto-downloading Java is not supported on OS '{other}'."
            ))
        }
    };
    let api_arch = match arch {
        "x86_64" => "x64",
        "aarch64" => "aarch64",
        "x86" => "x32",
        other => {
            return Err(format!(
                "Auto-downloading Java is not supported on CPU '{other}'."
            ))
        }
    };
    let ext = if api_os == "windows" { "zip" } else { "tar.gz" };
    let url = format!(
        "https://api.adoptium.net/v3/binary/latest/{major}/ga/{api_os}/{api_arch}/jre/hotspot/normal/eclipse"
    );
    Ok((ext, url))
}

fn find_java_in_dir(dir: &Path) -> Option<PathBuf> {
    let direct = [
        dir.join("bin").join(JAVA_BIN),
        dir.join("jre").join("bin").join(JAVA_BIN),
        dir.join("Contents").join("Home").join("bin").join(JAVA_BIN),
    ];
    for p in direct {
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let sub = entry.path();
            if sub.is_dir() {
                for p in [
                    sub.join("bin").join(JAVA_BIN),
                    sub.join("jre").join("bin").join(JAVA_BIN),
                    sub.join("Contents").join("Home").join("bin").join(JAVA_BIN),
                ] {
                    if p.is_file() {
                        return Some(p);
                    }
                }
            }
        }
    }
    None
}

fn extract_zip_archive(archive_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive_path)
        .map_err(|e| format!("Failed to open Java archive: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Invalid Java zip archive: {e}"))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry {i}: {e}"))?;
        let Some(rel_path) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };
        let out_path = dest_dir.join(rel_path);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| format!("Failed to create dir {}: {e}", out_path.display()))?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create parent dir {}: {e}", parent.display()))?;
        }
        let mut out_file = std::fs::File::create(&out_path)
            .map_err(|e| format!("Failed to create {}: {e}", out_path.display()))?;
        std::io::copy(&mut entry, &mut out_file)
            .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&out_path, std::fs::Permissions::from_mode(mode));
        }
    }
    Ok(())
}

fn emit_progress(tx: Option<&Sender<WorkerEvent>>, stage: impl Into<String>, fraction: f32) {
    if let Some(tx) = tx {
        let _ = tx.send(WorkerEvent::Progress {
            stage: stage.into(),
            fraction,
        });
    }
}

fn emit_log(tx: Option<&Sender<WorkerEvent>>, msg: impl Into<String>) {
    if let Some(tx) = tx {
        let _ = tx.send(WorkerEvent::Log(msg.into()));
    }
}

fn download_adoptium_jre(
    paths: &LauncherPaths,
    major: u32,
    tx: Option<&Sender<WorkerEvent>>,
) -> Result<PathBuf, String> {
    let java_root = paths.root.join("java");
    let target_dir = java_root.join(format!("temurin-{major}"));
    if let Some(existing) = find_java_in_dir(&target_dir) {
        if java_major_version(&existing) == Some(major) {
            return Ok(existing);
        }
    }

    let (ext, url) = adoptium_jre_url_for(major, std::env::consts::OS, std::env::consts::ARCH)?;
    std::fs::create_dir_all(&java_root)
        .map_err(|e| format!("Failed to create launcher java directory: {e}"))?;

    emit_log(
        tx,
        format!(
            "[Java] Required Java {major} not found locally; downloading Eclipse Temurin {major} into {}...",
            target_dir.display()
        ),
    );
    emit_progress(tx, format!("Downloading Java {major} (Temurin)..."), 0.02);

    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("sunset-launcher/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| format!("Failed to build HTTP client for Java download: {e}"))?;

    let mut resp = client
        .get(&url)
        .send()
        .map_err(|e| format!("Failed to download Java {major}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "Adoptium API returned HTTP {} while downloading Java {major}",
            resp.status()
        ));
    }

    let total_bytes = resp.content_length();
    let archive_part = java_root.join(format!("temurin-{major}.{ext}.part"));
    {
        let mut out = std::fs::File::create(&archive_part)
            .map_err(|e| format!("Failed to create temp Java archive: {e}"))?;
        let mut buf = [0u8; 64 * 1024];
        let mut downloaded: u64 = 0;
        let mut last_pct: u32 = 0;
        loop {
            if is_download_cancelled() {
                drop(out);
                let _ = std::fs::remove_file(&archive_part);
                return Err(CANCELLED_ERR_MSG.to_string());
            }
            let n = resp
                .read(&mut buf)
                .map_err(|e| format!("Error reading Java {major} download stream: {e}"))?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])
                .map_err(|e| format!("Failed to write Java {major} archive: {e}"))?;
            downloaded += n as u64;
            if let Some(total) = total_bytes {
                if total > 0 {
                    let pct = ((downloaded * 100) / total) as u32;
                    if pct >= last_pct + 5 || pct == 100 {
                        last_pct = pct;
                        let frac = 0.02 + 0.14 * (downloaded as f32 / total as f32);
                        emit_progress(
                            tx,
                            format!(
                                "Downloading Java {major} ({:.1} / {:.1} MB)...",
                                downloaded as f64 / (1024.0 * 1024.0),
                                total as f64 / (1024.0 * 1024.0)
                            ),
                            frac,
                        );
                    }
                }
            }
        }
    }

    emit_progress(tx, format!("Extracting Java {major}..."), 0.17);
    let stage_dir = java_root.join(format!(".tmp-temurin-{major}"));
    if stage_dir.exists() {
        let _ = std::fs::remove_dir_all(&stage_dir);
    }
    std::fs::create_dir_all(&stage_dir)
        .map_err(|e| format!("Failed to create Java staging directory: {e}"))?;

    let extract_res = if ext == "zip" {
        extract_zip_archive(&archive_part, &stage_dir)
    } else {
        let mut cmd = Command::new("tar");
        cmd.args([
            "-xzf",
            &archive_part.to_string_lossy(),
            "-C",
            &stage_dir.to_string_lossy(),
        ]);
        configure_background_command(&mut cmd);
        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run tar to extract Java {major}: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Failed to extract Java {major} archive: {stderr}"))
        }
    };
    let _ = std::fs::remove_file(&archive_part);
    if let Err(err) = extract_res {
        let _ = std::fs::remove_dir_all(&stage_dir);
        return Err(err);
    }

    // Flatten a single top-level directory (e.g. `jdk-21.0.5+11-jre/`) into `target_dir`.
    let mut top_dirs = Vec::new();
    let mut top_files = 0usize;
    if let Ok(entries) = std::fs::read_dir(&stage_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                top_dirs.push(entry.path());
            } else {
                top_files += 1;
            }
        }
    }
    let source_dir = if top_dirs.len() == 1 && top_files == 0 {
        top_dirs.remove(0)
    } else {
        stage_dir.clone()
    };

    if target_dir.exists() {
        let _ = std::fs::remove_dir_all(&target_dir);
    }
    if let Err(e) = std::fs::rename(&source_dir, &target_dir) {
        let _ = std::fs::remove_dir_all(&stage_dir);
        return Err(format!(
            "Failed to move extracted Java {major} into {}: {e}",
            target_dir.display()
        ));
    }
    if stage_dir.exists() {
        let _ = std::fs::remove_dir_all(&stage_dir);
    }

    let bin = find_java_in_dir(&target_dir).ok_or_else(|| {
        format!(
            "Extracted Java {major} in {}, but {} binary was not found.",
            target_dir.display(),
            JAVA_BIN
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&bin) {
            let mut perms = meta.permissions();
            if perms.mode() & 0o111 == 0 {
                perms.set_mode(perms.mode() | 0o755);
                let _ = std::fs::set_permissions(&bin, perms);
            }
        }
    }

    if let Some(actual) = java_major_version(&bin) {
        emit_log(
            tx,
            format!(
                "[Java] Installed Eclipse Temurin {actual} at {}",
                bin.display()
            ),
        );
    }
    Ok(bin)
}

/// Resolves the Java binary for `profile`, automatically downloading the required
/// Finds a usable Java binary (configured -> detected matching major -> managed Adoptium JRE).
/// When `offline_mode` is false, downloads an Eclipse Temurin JRE into `paths.root/java` if needed.
#[allow(dead_code)]
pub fn ensure_java_runtime(
    paths: &LauncherPaths,
    profile: &Profile,
    required: Option<u32>,
    tx: Option<&Sender<WorkerEvent>>,
) -> Result<PathBuf, String> {
    ensure_java_runtime_with_mode(paths, profile, required, false, tx)
}

pub fn ensure_java_runtime_with_mode(
    paths: &LauncherPaths,
    profile: &Profile,
    required: Option<u32>,
    offline_mode: bool,
    tx: Option<&Sender<WorkerEvent>>,
) -> Result<PathBuf, String> {
    if !profile.java_path.trim().is_empty() {
        return resolve_java_binary(profile, required);
    }

    let installs = detect_java_installations();
    if let Some(req) = required {
        // Prefer an exact major version match so older MC versions (e.g. 1.12.2 -> Java 8,
        // 1.18-1.20.4 -> Java 17) don't break on newer runtimes like Java 21/25.
        if let Some(exact) = installs.iter().find(|j| j.major == req) {
            return Ok(exact.path.clone());
        }
        if offline_mode {
            if let Ok(bin) = resolve_java_binary(profile, Some(req)) {
                emit_log(
                    tx,
                    format!(
                        "[Java] Offline mode: using fallback Java at {} (required {req})",
                        bin.display()
                    ),
                );
                return Ok(bin);
            }
            return Err(format!(
                "Offline mode: Java {req} is not installed locally and cannot be downloaded offline."
            ));
        }
        match download_adoptium_jre(paths, req, tx) {
            Ok(bin) => return Ok(bin),
            Err(dl_err) if dl_err == CANCELLED_ERR_MSG => return Err(dl_err),
            Err(dl_err) => {
                // Offline/fallback: try a newer installed Java if one exists.
                if let Ok(bin) = resolve_java_binary(profile, Some(req)) {
                    emit_log(
                        tx,
                        format!(
                            "[Java] Could not download Java {req} ({dl_err}); falling back to {}",
                            bin.display()
                        ),
                    );
                    return Ok(bin);
                }
                return Err(dl_err);
            }
        }
    }

    if let Ok(bin) = resolve_java_binary(profile, None) {
        return Ok(bin);
    }
    if offline_mode {
        return Err(
            "Offline mode: no Java runtime found locally. Install Java or disable offline mode."
                .to_string(),
        );
    }
    download_adoptium_jre(paths, 21, tx)
}

/// Downloads a specific Eclipse Temurin JRE major version into `paths.root/java`.
pub fn download_java_major(
    paths: &LauncherPaths,
    major: u32,
    tx: Option<&Sender<WorkerEvent>>,
) -> Result<PathBuf, String> {
    download_adoptium_jre(paths, major, tx)
}

/// Returns true if `java_bin` lives inside the launcher's managed `java/` directory.
pub fn is_managed_java_path(paths: &LauncherPaths, java_bin: &Path) -> bool {
    let managed_root = paths.root.join("java");
    java_bin.starts_with(&managed_root)
}

/// Removes a launcher-managed Java runtime directory safely.
pub fn remove_managed_java(paths: &LauncherPaths, java_bin: &Path) -> Result<(), String> {
    let managed_root = paths.root.join("java");
    if !java_bin.starts_with(&managed_root) {
        return Err("Refusing to remove non-launcher Java runtime".to_string());
    }
    let rel = java_bin
        .strip_prefix(&managed_root)
        .map_err(|e| e.to_string())?;
    let Some(first_comp) = rel.components().next() else {
        return Err("Invalid managed Java path".to_string());
    };
    let runtime_dir = managed_root.join(first_comp.as_os_str());
    if runtime_dir.exists() {
        std::fs::remove_dir_all(&runtime_dir)
            .map_err(|e| format!("Failed to remove {}: {e}", runtime_dir.display()))?;
    }
    Ok(())
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
    features: &RuleFeatures,
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
                    .map_or(true, |rules| rules_allow_with_features(Some(&rules), features));
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

/// Parses newline-separated `KEY=VALUE` environment variables (ignoring `#` comments and blanks).
pub fn parse_env_vars(input: &str) -> Vec<(String, String)> {
    let mut vars = Vec::new();
    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim();
            if key.is_empty() {
                continue;
            }
            let mut val = v.trim();
            if val.len() >= 2
                && ((val.starts_with('"') && val.ends_with('"'))
                    || (val.starts_with('\'') && val.ends_with('\'')))
            {
                val = &val[1..val.len() - 1];
            }
            vars.push((key.to_string(), val.to_string()));
        }
    }
    vars
}

fn run_shell_hook(
    cmd_str: &str,
    cwd: &Path,
    envs: &[(String, String)],
    label: &str,
    tx: &Sender<WorkerEvent>,
    profile_id: &str,
) -> Result<(), String> {
    let trimmed = cmd_str.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let _ = tx.send(WorkerEvent::InstanceLog {
        profile_id: profile_id.to_string(),
        line: format!("[Hook:{label}] Running: {trimmed}"),
    });

    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", trimmed]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", trimmed]);
        c
    };
    cmd.current_dir(cwd);
    configure_background_command(&mut cmd);
    for (k, v) in envs {
        cmd.env(k, v);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("{label} command failed to start: {e}"))?;

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if !line.trim().is_empty() {
            let _ = tx.send(WorkerEvent::InstanceLog {
                profile_id: profile_id.to_string(),
                line: format!("[Hook:{label}] {line}"),
            });
        }
    }
    for line in String::from_utf8_lossy(&output.stderr).lines() {
        if !line.trim().is_empty() {
            let _ = tx.send(WorkerEvent::InstanceLog {
                profile_id: profile_id.to_string(),
                line: format!("[Hook:{label} ERR] {line}"),
            });
        }
    }

    if !output.status.success() {
        return Err(format!(
            "{label} command exited with code {:?}",
            output.status.code()
        ));
    }
    Ok(())
}

/// Launches Minecraft in a child process and streams stdout/stderr to the UI.
/// Returns a handle that lets the UI stop the game.
pub fn launch_game(
    paths: &LauncherPaths,
    profile: &Profile,
    account: &Account,
    prepared: &PreparedLaunch,
    authlib_injector: Option<&Path>,
    quick_world: Option<&str>,
    offline_mode: bool,
    tx: Sender<WorkerEvent>,
) -> Result<GameHandle, String> {
    check_not_cancelled()?;
    let required_java = prepared
        .version_details
        .java_version
        .as_ref()
        .map(|j| j.major_version);
    let java_bin =
        ensure_java_runtime_with_mode(paths, profile, required_java, offline_mode, Some(&tx))?;
    let game_dir = instance_dir(paths, profile);
    let profile_id = profile.id.clone();

    let mut hook_envs = parse_env_vars(&profile.env_vars);
    hook_envs.push((
        "SUNSET_INSTANCE_DIR".to_string(),
        game_dir.to_string_lossy().to_string(),
    ));
    hook_envs.push(("SUNSET_PROFILE_NAME".to_string(), profile.name.clone()));
    hook_envs.push(("SUNSET_MC_VERSION".to_string(), profile.version_id.clone()));

    run_shell_hook(
        &profile.pre_launch_cmd,
        &game_dir,
        &hook_envs,
        "pre-launch",
        &tx,
        &profile_id,
    )?;

    // Sync pre-1.6 `map_to_resources` assets into `<instance>/resources/` when applicable.
    if let Some(ref idx_path) = prepared.map_to_resources_index {
        if let Ok(raw) = std::fs::read_to_string(idx_path) {
            if let Ok(asset_idx) = serde_json::from_str::<AssetIndex>(&raw) {
                let _ = sync_mapped_assets_to_dir(
                    &paths.assets_dir.join("objects"),
                    &asset_idx,
                    &game_dir.join("resources"),
                );
            }
        }
    }

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
    let game_assets = prepared.game_assets_dir.to_string_lossy().to_string();

    let custom_res = match (
        profile.window_width.filter(|&w| w >= 320),
        profile.window_height.filter(|&h| h >= 240),
    ) {
        (Some(w), Some(h)) => Some((w, h)),
        _ => None,
    };
    let single_world = quick_world
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .map(str::to_string);
    let multi_server = if single_world.is_none() {
        let addr = profile.server_address.trim();
        (!addr.is_empty()).then(|| addr.to_string())
    } else {
        None
    };

    let features = RuleFeatures {
        is_demo_user: false,
        has_custom_resolution: custom_res.is_some(),
        has_quick_plays_support: false,
        is_quick_play_singleplayer: single_world.is_some(),
        is_quick_play_multiplayer: multi_server.is_some(),
        is_quick_play_realms: false,
    };

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", username);
    vars.insert("version_name", prepared.version_details.id.clone());
    vars.insert("game_directory", game_dir.to_string_lossy().to_string());
    vars.insert("assets_root", assets_root);
    vars.insert("game_assets", game_assets);
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
    vars.insert("launcher_name", "Sunset Launcher".to_string());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").to_string());
    vars.insert("classpath", classpath_str.clone());
    vars.insert("classpath_separator", sep.to_string());
    vars.insert("library_directory", paths.libraries_dir.to_string_lossy().to_string());
    if let Some((w, h)) = custom_res {
        vars.insert("resolution_width", w.to_string());
        vars.insert("resolution_height", h.to_string());
    }
    if let Some(ref w) = single_world {
        vars.insert("quickPlaySingleplayer", w.clone());
    }
    if let Some(ref addr) = multi_server {
        vars.insert("quickPlayMultiplayer", addr.clone());
    }

    let mut cmd_args: Vec<String> = Vec::new();
    let max_ram = profile.max_ram_mb.max(512);
    cmd_args.push(format!("-Xms{}M", profile.min_ram_mb.clamp(256, max_ram)));
    cmd_args.push(format!("-Xmx{max_ram}M"));

    if let Some(agent) = authlib_injector {
        // Ely.by skins + joining Ely.by servers.
        cmd_args.push(format!("-javaagent:{}=ely.by", agent.display()));
    }

    let user_jvm_args = split_jvm_args(&profile.jvm_args);
    if !user_jvm_args
        .iter()
        .any(|a| a.starts_with("-Dlog4j2.formatMsgNoLookups="))
    {
        cmd_args.push("-Dlog4j2.formatMsgNoLookups=true".to_string());
    }
    if let Some(ref log_arg) = prepared.log_config_arg {
        if !user_jvm_args
            .iter()
            .any(|a| a.starts_with("-Dlog4j.configurationFile="))
        {
            cmd_args.push(log_arg.clone());
        }
    }
    cmd_args.extend(user_jvm_args);

    // JVM args from Mojang version JSON (or fallback for legacy versions)
    if let Some(jvm_entries) = prepared
        .version_details
        .arguments
        .as_ref()
        .and_then(|a| a.jvm.as_ref())
    {
        collect_arg_values(jvm_entries, &vars, &features, &mut cmd_args);
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
        collect_arg_values(jvm_entries, &vars, &features, &mut cmd_args);
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
        collect_arg_values(game_entries, &vars, &features, &mut cmd_args);
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
        collect_arg_values(game_entries, &vars, &features, &mut cmd_args);
    }

    append_profile_launch_args_with_world(profile, quick_world, &mut cmd_args);

    let _ = tx.send(WorkerEvent::InstanceLog {
        profile_id: profile_id.clone(),
        line: format!(
            "[Launcher] Starting {} ({}) with {} in {}",
            profile.name,
            profile.version_id,
            java_bin.display(),
            game_dir.display()
        ),
    });

    let wrapper_tokens = split_jvm_args(&profile.wrapper_cmd);
    let mut command = if let Some((wrapper_exe, wrapper_rest)) = wrapper_tokens.split_first() {
        let mut c = Command::new(wrapper_exe);
        c.args(wrapper_rest);
        c.arg(&java_bin);
        c
    } else {
        Command::new(&java_bin)
    };
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
    for (k, v) in parse_env_vars(&profile.env_vars) {
        command.env(k, v);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to start Java ({}): {e}", java_bin.display()))?;

    if let Some(stdout) = child.stdout.take() {
        let tx_out = tx.clone();
        let pid_out = profile_id.clone();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let _ = tx_out.send(WorkerEvent::InstanceLog {
                    profile_id: pid_out.clone(),
                    line,
                });
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let tx_err = tx.clone();
        let pid_err = profile_id.clone();
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let _ = tx_err.send(WorkerEvent::InstanceLog {
                    profile_id: pid_err.clone(),
                    line,
                });
            }
        });
    }

    let handle: GameHandle = Arc::new(Mutex::new(child));
    let watcher = Arc::clone(&handle);
    let post_exit_cmd = profile.post_exit_cmd.clone();
    thread::spawn(move || loop {
        // Poll instead of wait() so the UI can lock the child and kill it.
        let status = match watcher.lock() {
            Ok(mut child) => child.try_wait(),
            Err(_) => break,
        };
        match status {
            Ok(Some(s)) => {
                if let Err(err) = run_shell_hook(
                    &post_exit_cmd,
                    &game_dir,
                    &hook_envs,
                    "post-exit",
                    &tx,
                    &profile_id,
                ) {
                    let _ = tx.send(WorkerEvent::InstanceLog {
                        profile_id: profile_id.clone(),
                        line: format!("[Hook:post-exit WARN] {err}"),
                    });
                }
                let _ = tx.send(WorkerEvent::GameExited {
                    profile_id,
                    code: s.code(),
                });
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(400)),
            Err(_) => {
                let _ = tx.send(WorkerEvent::GameExited {
                    profile_id,
                    code: None,
                });
                break;
            }
        }
    });

    Ok(handle)
}

/// Appends instance window resolution, fullscreen, and quick-play server flags to `cmd_args`.
#[cfg(test)]
pub fn append_profile_launch_args(profile: &Profile, cmd_args: &mut Vec<String>) {
    append_profile_launch_args_with_world(profile, None, cmd_args);
}

/// Appends instance launch flags, preferring `quick_world` (`--quickPlaySingleplayer`) over
/// `profile.server_address` when launching directly into a saved world.
pub fn append_profile_launch_args_with_world(
    profile: &Profile,
    quick_world: Option<&str>,
    cmd_args: &mut Vec<String>,
) {
    if profile.fullscreen && !cmd_args.iter().any(|a| a == "--fullscreen") {
        cmd_args.push("--fullscreen".to_string());
    }
    if let (Some(w), Some(h)) = (
        profile.window_width.filter(|&w| w >= 320),
        profile.window_height.filter(|&h| h >= 240),
    ) {
        if !cmd_args.iter().any(|a| a == "--width") {
            cmd_args.push("--width".to_string());
            cmd_args.push(w.to_string());
        }
        if !cmd_args.iter().any(|a| a == "--height") {
            cmd_args.push("--height".to_string());
            cmd_args.push(h.to_string());
        }
    }
    if let Some(world) = quick_world.map(str::trim).filter(|w| !w.is_empty()) {
        if !cmd_args.iter().any(|a| a == "--quickPlaySingleplayer") {
            cmd_args.push("--quickPlaySingleplayer".to_string());
            cmd_args.push(world.to_string());
        }
    } else {
        let addr = profile.server_address.trim();
        if !addr.is_empty() && !cmd_args.iter().any(|a| a == "--quickPlayMultiplayer") {
            cmd_args.push("--quickPlayMultiplayer".to_string());
            cmd_args.push(addr.to_string());
        }
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

    #[test]
    fn test_adoptium_jre_url_for_platforms() {
        let (ext_lin, url_lin) = adoptium_jre_url_for(21, "linux", "x86_64").unwrap();
        assert_eq!(ext_lin, "tar.gz");
        assert_eq!(
            url_lin,
            "https://api.adoptium.net/v3/binary/latest/21/ga/linux/x64/jre/hotspot/normal/eclipse"
        );

        let (ext_win, url_win) = adoptium_jre_url_for(17, "windows", "x86_64").unwrap();
        assert_eq!(ext_win, "zip");
        assert_eq!(
            url_win,
            "https://api.adoptium.net/v3/binary/latest/17/ga/windows/x64/jre/hotspot/normal/eclipse"
        );

        let (ext_mac, url_mac) = adoptium_jre_url_for(8, "macos", "aarch64").unwrap();
        assert_eq!(ext_mac, "tar.gz");
        assert_eq!(
            url_mac,
            "https://api.adoptium.net/v3/binary/latest/8/ga/mac/aarch64/jre/hotspot/normal/eclipse"
        );

        let (ext_25, url_25) = adoptium_jre_url_for(25, "windows", "x86_64").unwrap();
        assert_eq!(ext_25, "zip");
        assert_eq!(
            url_25,
            "https://api.adoptium.net/v3/binary/latest/25/ga/windows/x64/jre/hotspot/normal/eclipse"
        );
    }

    #[test]
    fn test_find_java_in_dir_direct_and_nested() {
        let tmp = std::env::temp_dir().join(format!(
            "mc_launcher_java_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let bin_dir = tmp.join("jdk-21.0.5+11-jre").join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let fake_java = bin_dir.join(JAVA_BIN);
        std::fs::write(&fake_java, b"").unwrap();

        let found = find_java_in_dir(&tmp).expect("should find nested java binary");
        assert_eq!(found, fake_java);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_append_world_quickplay_overrides_server() {
        let mut p = Profile::new("W".into(), "1.21.4".into(), ModLoader::Vanilla, "P".into());
        p.server_address = "play.example.com".to_string();
        let mut args = Vec::new();
        append_profile_launch_args_with_world(&p, Some("MyWorld"), &mut args);
        assert_eq!(args, vec!["--quickPlaySingleplayer", "MyWorld"]);
    }

    #[test]
    fn test_split_jvm_args_preserves_quoted_paths_and_backslashes() {
        let raw = r#"-XX:+UseG1GC -XX:HeapDumpPath="C:\My Games\dumps\mc.hprof" '-Dfoo.bar=hello world' -Xmx4G"#;
        assert_eq!(
            split_jvm_args(raw),
            vec![
                "-XX:+UseG1GC",
                r"-XX:HeapDumpPath=C:\My Games\dumps\mc.hprof",
                "-Dfoo.bar=hello world",
                "-Xmx4G",
            ]
        );
    }

    #[test]
    fn test_parse_env_vars_handles_comments_quotes_and_export() {
        let raw = "
# comment
FOO=bar
export BAZ = \"hello world\"
SINGLE='one=two'
EMPTY=
";
        assert_eq!(
            parse_env_vars(raw),
            vec![
                ("FOO".to_string(), "bar".to_string()),
                ("BAZ".to_string(), "hello world".to_string()),
                ("SINGLE".to_string(), "one=two".to_string()),
                ("EMPTY".to_string(), "".to_string()),
            ]
        );
    }
}
