//! Lightweight Discord Rich Presence client over local IPC (no extra crates).
//! Activity text automatically follows the active launcher UI language.

use crate::i18n::{tr, Language};
use std::io::{Read, Write};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
            String::new(),
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
            String::new(),
        ),
        DiscordActivityKind::Accounts => (
            tr(lang, "rpc.accounts").to_string(),
            String::new(),
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

const DEBOUNCE_WINDOW: Duration = Duration::from_millis(320);
const MIN_SEND_SPACING: Duration = Duration::from_millis(1500);
const RETRY_DELAY: Duration = Duration::from_millis(3000);

fn next_send_deadline(last_send_at: Option<Instant>) -> Instant {
    let debounced = Instant::now() + DEBOUNCE_WINDOW;
    match last_send_at {
        Some(prev) => debounced.max(prev + MIN_SEND_SPACING),
        None => debounced,
    }
}

impl DiscordRpc {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<PresenceState>();
        thread::spawn(move || {
            let mut conn: Option<IpcConn> = None;
            let mut last_acked: Option<PresenceState> = None;
            let mut desired: Option<PresenceState> = None;
            let mut last_client_id = String::new();
            let mut last_send_at: Option<Instant> = None;
            let mut next_flush_at: Option<Instant> = None;

            loop {
                let maybe_msg = match next_flush_at {
                    None => match rx.recv() {
                        Ok(s) => Some(s),
                        Err(_) => break,
                    },
                    Some(deadline) => {
                        let wait = deadline.saturating_duration_since(Instant::now());
                        match rx.recv_timeout(wait) {
                            Ok(s) => Some(s),
                            Err(mpsc::RecvTimeoutError::Timeout) => None,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                };

                if let Some(mut state) = maybe_msg {
                    while let Ok(newer) = rx.try_recv() {
                        state = newer;
                    }
                    desired = Some(state);
                    if desired.as_ref() == last_acked.as_ref() {
                        next_flush_at = None;
                    } else {
                        next_flush_at = Some(next_send_deadline(last_send_at));
                    }
                    continue;
                }

                // Coalesce any last-moment updates arriving right as the debounce deadline fires.
                while let Ok(newer) = rx.try_recv() {
                    desired = Some(newer);
                }
                next_flush_at = None;

                let Some(state) = desired.clone() else {
                    continue;
                };
                if last_acked.as_ref() == Some(&state) {
                    continue;
                }

                let effective_cid = if state.client_id.trim().is_empty() {
                    DEFAULT_DISCORD_CLIENT_ID.to_string()
                } else {
                    state.client_id.trim().to_string()
                };
                if last_client_id != effective_cid {
                    conn = None;
                    last_acked = None;
                    last_client_id = effective_cid.clone();
                }

                if !state.enabled {
                    if let Some(c) = conn.as_mut() {
                        if send_clear_activity(c).is_err() {
                            conn = None;
                        }
                    }
                    last_acked = Some(state);
                    continue;
                }

                last_send_at = Some(Instant::now());
                if conn.is_none() {
                    conn = connect_and_handshake(&effective_cid).ok();
                }
                let mut sent_ok = false;
                if let Some(c) = conn.as_mut() {
                    match send_set_activity(c, &state) {
                        Ok(()) => sent_ok = true,
                        Err(IpcError::Protocol) => {}
                        Err(IpcError::Io) => {
                            conn = None;
                            if let Ok(mut fresh) = connect_and_handshake(&effective_cid) {
                                if send_set_activity(&mut fresh, &state).is_ok() {
                                    conn = Some(fresh);
                                    sent_ok = true;
                                }
                            }
                        }
                    }
                }

                if sent_ok {
                    last_acked = Some(state);
                } else {
                    // Leave `last_acked` unchanged so we retry the latest state after cooldown.
                    next_flush_at = Some(Instant::now() + RETRY_DELAY);
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
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(900)));
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(900)));
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

#[derive(Debug, PartialEq, Eq)]
enum IpcError {
    Io,
    Protocol,
}

#[derive(Debug, PartialEq, Eq)]
enum AckVerdict {
    MatchedOk,
    Error,
    IgnoreAndReadNext,
}

fn parse_ipc_ack_response(op: u32, body: &str, expected_nonce: &str) -> AckVerdict {
    if op == 2 {
        return AckVerdict::Error;
    }
    let Ok(val) = serde_json::from_str::<serde_json::Value>(body) else {
        return AckVerdict::IgnoreAndReadNext;
    };
    let nonce = val.get("nonce").and_then(|v| v.as_str());
    let is_error = val.get("evt").and_then(|v| v.as_str()) == Some("ERROR");
    if is_error {
        return match nonce {
            Some(n) if n == expected_nonce => AckVerdict::Error,
            None => AckVerdict::Error,
            _ => AckVerdict::IgnoreAndReadNext,
        };
    }
    if nonce == Some(expected_nonce) {
        AckVerdict::MatchedOk
    } else {
        AckVerdict::IgnoreAndReadNext
    }
}

fn send_command_and_wait_ack(
    conn: &mut IpcConn,
    payload: &serde_json::Value,
    nonce: &str,
) -> Result<(), IpcError> {
    write_frame(conn, 1, &payload.to_string()).map_err(|_| IpcError::Io)?;
    for _ in 0..4 {
        let (op, body) = read_frame_with_op(conn).map_err(|_| IpcError::Io)?;
        match parse_ipc_ack_response(op, &body, nonce) {
            AckVerdict::MatchedOk => return Ok(()),
            AckVerdict::Error => return Err(IpcError::Protocol),
            AckVerdict::IgnoreAndReadNext => continue,
        }
    }
    Err(IpcError::Io)
}

fn build_activity_json(state: &PresenceState) -> serde_json::Value {
    let (details, sub_state) = format_presence(state.language, &state.kind);
    let mut activity = serde_json::json!({
        "details": details,
        "timestamps": {
            "start": state.started_at
        },
        "assets": {
            "large_image": "minecraft"
        }
    });
    // Discord IPC rejects empty strings in `state`; omit it when there is no secondary line.
    if !sub_state.trim().is_empty() {
        activity["state"] = serde_json::Value::String(sub_state);
    }
    activity
}

fn send_set_activity(conn: &mut IpcConn, state: &PresenceState) -> Result<(), IpcError> {
    let nonce = uuid::Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "cmd": "SET_ACTIVITY",
        "args": {
            "pid": std::process::id(),
            "activity": build_activity_json(state)
        },
        "nonce": nonce
    });
    send_command_and_wait_ack(conn, &payload, &nonce)
}

fn send_clear_activity(conn: &mut IpcConn) -> Result<(), IpcError> {
    let nonce = uuid::Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "cmd": "SET_ACTIVITY",
        "args": {
            "pid": std::process::id(),
            "activity": serde_json::Value::Null
        },
        "nonce": nonce
    });
    send_command_and_wait_ack(conn, &payload, &nonce)
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

        for kind in [
            DiscordActivityKind::InLauncher,
            DiscordActivityKind::Console,
            DiscordActivityKind::Accounts,
        ] {
            let (_d, s) = format_presence(Language::Ru, &kind);
            assert!(s.is_empty());
            let activity = build_activity_json(&PresenceState {
                kind,
                language: Language::Ru,
                ..PresenceState::default()
            });
            assert!(activity.get("state").is_none());
            assert!(activity["assets"].get("large_text").is_none());
        }

        let playing_activity = build_activity_json(&PresenceState {
            kind: playing,
            language: Language::En,
            ..PresenceState::default()
        });
        assert_eq!(playing_activity["state"], "Instance: Survival");
        assert!(playing_activity["assets"].get("large_text").is_none());
    }

    #[test]
    fn test_ipc_ack_parsing_and_send_pacing() {
        assert_eq!(
            parse_ipc_ack_response(
                1,
                r#"{"cmd":"SET_ACTIVITY","evt":null,"nonce":"n-1"}"#,
                "n-1"
            ),
            AckVerdict::MatchedOk
        );
        assert_eq!(
            parse_ipc_ack_response(
                1,
                r#"{"cmd":"SET_ACTIVITY","evt":"ERROR","nonce":"n-1","data":{"code":4009}}"#,
                "n-1"
            ),
            AckVerdict::Error
        );
        assert_eq!(
            parse_ipc_ack_response(
                1,
                r#"{"cmd":"SET_ACTIVITY","evt":null,"nonce":"stale-0"}"#,
                "n-1"
            ),
            AckVerdict::IgnoreAndReadNext
        );
        assert_eq!(parse_ipc_ack_response(2, "{}", "n-1"), AckVerdict::Error);

        let now = Instant::now();
        let d_recent = next_send_deadline(Some(now));
        assert!(d_recent >= now + MIN_SEND_SPACING - Duration::from_millis(20));

        let d_idle = next_send_deadline(Some(now - Duration::from_secs(10)));
        assert!(d_idle >= Instant::now());
        assert!(d_idle < Instant::now() + DEBOUNCE_WINDOW + Duration::from_millis(50));
    }
}

