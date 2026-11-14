//! Lightweight Discord Rich Presence client over local IPC (no extra crates).
//! Activity text automatically follows the active launcher UI language.

use crate::i18n::{tr, Language};
use std::io::{Read, Write};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_DISCORD_CLIENT_ID: &str = "1571137394923667579";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscordActivityKind {
    InLauncher,
    BrowsingModrinth,
    ConfiguringInstance {
        name: String,
    },
    Console,
    Accounts,
    Playing {
        instance: String,
        version: String,
        loader: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceState {
    pub enabled: bool,
    pub language: Language,
    pub kind: DiscordActivityKind,
    pub started_at: i64,
    pub client_id: String,
}

impl Default for PresenceState {
    fn default() -> Self {
        Self {
            enabled: true,
            language: Language::En,
            kind: DiscordActivityKind::InLauncher,
            started_at: unix_now_secs(),
            client_id: String::new(),
        }
    }
}

pub fn unix_now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Formats `(details, state)` in the currently selected launcher language.
pub fn format_presence(lang: Language, kind: &DiscordActivityKind) -> (String, String) {
    match kind {
        DiscordActivityKind::InLauncher => (
            tr(lang, "rpc.in_launcher").to_string(),
            "Svarka Launcher".to_string(),
        ),
        DiscordActivityKind::BrowsingModrinth => (
            tr(lang, "rpc.browsing_mods").to_string(),
            "Modrinth".to_string(),
        ),
        DiscordActivityKind::ConfiguringInstance { name } => (
            tr(lang, "rpc.configuring").to_string(),
            format!("{}: {name}", tr(lang, "rpc.instance")),
        ),
        DiscordActivityKind::Console => (
            tr(lang, "rpc.console").to_string(),
            "Svarka Launcher".to_string(),
        ),
        DiscordActivityKind::Accounts => (
            tr(lang, "rpc.accounts").to_string(),
            "Svarka Launcher".to_string(),
        ),
        DiscordActivityKind::Playing {
            instance,
            version,
            loader,
        } => (
            format!("{} {version} ({loader})", tr(lang, "rpc.playing")),
            format!("{}: {instance}", tr(lang, "rpc.instance")),
        ),
    }
}

pub struct DiscordRpc {
    tx: Sender<PresenceState>,
}

impl DiscordRpc {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<PresenceState>();
        thread::spawn(move || {
            let mut conn: Option<IpcConn> = None;
            let mut last_state: Option<PresenceState> = None;
            let mut last_client_id = String::new();

            while let Ok(mut state) = rx.recv() {
                // Coalesce bursty updates so we only send the latest state.
                while let Ok(newer) = rx.try_recv() {
                    state = newer;
                }
                if last_state.as_ref() == Some(&state) {
                    continue;
                }
                last_state = Some(state.clone());

                let effective_cid = if state.client_id.trim().is_empty() {
                    DEFAULT_DISCORD_CLIENT_ID.to_string()
                } else {
                    state.client_id.trim().to_string()
                };
                if last_client_id != effective_cid {
                    conn = None;
                    last_client_id = effective_cid.clone();
                }

                if !state.enabled {
                    if let Some(c) = conn.as_mut() {
                        if send_clear_activity(c).is_err() {
                            conn = None;
                        }
                    }
                    continue;
                }

                if conn.is_none() {
                    conn = connect_and_handshake(&effective_cid).ok();
                }
                if let Some(c) = conn.as_mut() {
                    if send_set_activity(c, &state).is_err() {
                        conn = None;
                        if let Ok(mut fresh) = connect_and_handshake(&effective_cid) {
                            if send_set_activity(&mut fresh, &state).is_ok() {
                                conn = Some(fresh);
                            }
                        }
                    }
                }
            }
        });
        Self { tx }
    }

    pub fn update(&self, state: PresenceState) {
        let _ = self.tx.send(state);
    }
}

#[cfg(unix)]
type IpcConn = std::os::unix::net::UnixStream;
#[cfg(windows)]
type IpcConn = std::fs::File;

fn connect_and_handshake(client_id: &str) -> Result<IpcConn, String> {
    let mut conn = open_ipc_socket()?;
    let handshake = serde_json::json!({
        "v": 1,
        "client_id": client_id
    });
    write_frame(&mut conn, 0, &handshake.to_string())?;
    let (op, body) = read_frame_with_op(&mut conn)?;
    if op == 2 {
        return Err(format!("Discord IPC rejected handshake: {body}"));
    }
    Ok(conn)
}

#[cfg(unix)]
fn open_ipc_socket() -> Result<IpcConn, String> {
    let mut dirs_to_check: Vec<std::path::PathBuf> = Vec::new();
    for key in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"] {
        if let Ok(val) = std::env::var(key) {
            if !val.trim().is_empty() {
                let base = std::path::PathBuf::from(val);
                dirs_to_check.push(base.clone());
                dirs_to_check.push(base.join("app/com.discordapp.Discord"));
                dirs_to_check.push(base.join("snap.discord"));
            }
        }
    }
    dirs_to_check.push(std::path::PathBuf::from("/tmp"));

    for dir in dirs_to_check {
        for i in 0..10 {
            let path = dir.join(format!("discord-ipc-{i}"));
            if path.exists() {
                if let Ok(stream) = std::os::unix::net::UnixStream::connect(&path) {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                    return Ok(stream);
                }
            }
        }
    }
    Err("Discord IPC socket not found".to_string())
}

#[cfg(windows)]
fn open_ipc_socket() -> Result<IpcConn, String> {
    for i in 0..10 {
        let pipe = format!(r"\\.\pipe\discord-ipc-{i}");
        if let Ok(file) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&pipe)
        {
            return Ok(file);
        }
    }
    Err("Discord IPC pipe not found".to_string())
}

fn write_frame<W: Write>(stream: &mut W, opcode: u32, json_payload: &str) -> Result<(), String> {
    let bytes = json_payload.as_bytes();
    let len = bytes.len() as u32;
    let mut header = [0u8; 8];
    header[0..4].copy_from_slice(&opcode.to_le_bytes());
    header[4..8].copy_from_slice(&len.to_le_bytes());
    stream.write_all(&header).map_err(|e| e.to_string())?;
    stream.write_all(bytes).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())
}

fn read_frame_with_op<R: Read>(stream: &mut R) -> Result<(u32, String), String> {
    let mut header = [0u8; 8];
    stream.read_exact(&mut header).map_err(|e| e.to_string())?;
    let op = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
    if len > 1024 * 1024 {
        return Err("Discord IPC frame too large".to_string());
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).map_err(|e| e.to_string())?;
    Ok((op, String::from_utf8_lossy(&buf).to_string()))
}

fn read_frame<R: Read>(stream: &mut R) -> Result<String, String> {
    read_frame_with_op(stream).map(|(_, body)| body)
}

fn send_set_activity(conn: &mut IpcConn, state: &PresenceState) -> Result<(), String> {
    let (details, sub_state) = format_presence(state.language, &state.kind);
    let payload = serde_json::json!({
        "cmd": "SET_ACTIVITY",
        "args": {
            "pid": std::process::id(),
            "activity": {
                "details": details,
                "state": sub_state,
                "timestamps": {
                    "start": state.started_at
                },
                "assets": {
                    "large_image": "minecraft",
                    "large_text": "Svarka Launcher"
                }
            }
        },
        "nonce": uuid::Uuid::new_v4().to_string()
    });
    write_frame(conn, 1, &payload.to_string())?;
    let _ = read_frame(conn);
    Ok(())
}

fn send_clear_activity(conn: &mut IpcConn) -> Result<(), String> {
    let payload = serde_json::json!({
        "cmd": "SET_ACTIVITY",
        "args": {
            "pid": std::process::id(),
            "activity": serde_json::Value::Null
        },
        "nonce": uuid::Uuid::new_v4().to_string()
    });
    write_frame(conn, 1, &payload.to_string())?;
    let _ = read_frame(conn);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_presence_respects_language() {
        let playing = DiscordActivityKind::Playing {
            instance: "Survival".to_string(),
            version: "1.21.4".to_string(),
            loader: "Fabric".to_string(),
        };
        let (d_en, s_en) = format_presence(Language::En, &playing);
        assert_eq!(d_en, "Playing 1.21.4 (Fabric)");
        assert_eq!(s_en, "Instance: Survival");

        let (d_ru, s_ru) = format_presence(Language::Ru, &playing);
        assert_eq!(d_ru, "Играет в 1.21.4 (Fabric)");
        assert_eq!(s_ru, "Сборка: Survival");

        let (d_uk, s_uk) = format_presence(Language::Uk, &playing);
        assert_eq!(d_uk, "Грає в 1.21.4 (Fabric)");
        assert_eq!(s_uk, "Збірка: Survival");
    }
}

