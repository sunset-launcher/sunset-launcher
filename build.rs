use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn parse_version_tuple(version: &str) -> (u16, u16, u16) {
    let clean = version.split(['-', '+']).next().unwrap_or(version);
    let mut parts = clean.split('.');
    let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

fn find_windows_rc() -> Option<PathBuf> {
    if let Some(path_var) = env::var_os("PATH") {
        for dir in env::split_paths(&path_var) {
            let candidate = dir.join("rc.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let arch = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86") => "x86",
        Ok("aarch64") => "arm64",
        _ => "x64",
    };
    let mut bases = Vec::new();
    if let Ok(sdk) = env::var("WindowsSdkDir") {
        bases.push(PathBuf::from(sdk).join("bin"));
    }
    for env_key in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Ok(pf) = env::var(env_key) {
            bases.push(
                PathBuf::from(pf)
                    .join("Windows Kits")
                    .join("10")
                    .join("bin"),
            );
        }
    }
    for base in bases {
        let Ok(entries) = fs::read_dir(&base) else {
            continue;
        };
        let mut versions: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        versions.sort();
        for dir in versions.into_iter().rev() {
            let candidate = dir.join(arch).join("rc.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=logo.ico");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let ico_path = manifest_dir.join("logo.ico");
    let ico_escaped = ico_path.to_string_lossy().replace('\\', "\\\\");

    let pkg_ver = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.3".to_string());
    let (maj, min, pat) = parse_version_tuple(&pkg_ver);
    let ver_str = format!("{maj}.{min}.{pat}.0");

    let rc_content = format!(
        r#"1 ICON "{ico_escaped}"

1 VERSIONINFO
FILEVERSION {maj},{min},{pat},0
PRODUCTVERSION {maj},{min},{pat},0
FILEFLAGSMASK 0x3FL
FILEFLAGS 0x0L
FILEOS 0x00040004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "CompanyName", "sunset-launcher"
            VALUE "FileDescription", "Sunset Launcher"
            VALUE "FileVersion", "{ver_str}"
            VALUE "InternalName", "mc-launcher"
            VALUE "LegalCopyright", "Copyright (C) 2026 svarka (@sunset-launcher) (MIT)"
            VALUE "OriginalFilename", "sunset-launcher-windows-x64.exe"
            VALUE "ProductName", "Sunset Launcher"
            VALUE "ProductVersion", "{ver_str}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
"#
    );

    let rc_path = out_dir.join("app.rc");
    let res_path = out_dir.join("app.res");
    fs::write(&rc_path, rc_content).expect("Failed to write generated app.rc");

    if let Some(rc_bin) = find_windows_rc() {
        let status = Command::new(&rc_bin)
            .args([
                Path::new("/nologo"),
                Path::new("/fo"),
                res_path.as_path(),
                rc_path.as_path(),
            ])
            .status()
            .expect("Failed to execute Windows Resource Compiler (rc.exe)");
        assert!(status.success(), "rc.exe failed with status {status}");
        println!("cargo:rustc-link-arg-bins={}", res_path.display());
    } else {
        println!(
            "cargo:warning=Windows rc.exe not found; skipping embedded .exe icon and VersionInfo"
        );
    }
}
