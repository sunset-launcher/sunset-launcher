use crate::models::{
    Account, AccountKind, DeviceCodeInfo, IconImage, LauncherPaths, LauncherUpdateInfo, WorkerEvent,
};
use crate::mojang::{build_http_client, download_file};
use serde::Deserialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

const AUTHLIB_INJECTOR_LATEST: &str = "https://authlib-injector.yushi.moe/artifact/latest.json";

#[derive(Deserialize)]
struct AuthlibLatest {
    download_url: String,
}

/// Ensures authlib-injector is available (needed for Ely.by skins and Ely.by servers).
/// Uses the cached jar if present, so launching works offline after the first time.
pub fn ensure_authlib_injector(paths: &LauncherPaths) -> Result<PathBuf, String> {
    let dest = paths.root.join("authlib-injector.jar");
    if dest.is_file() {
        return Ok(dest);
    }
    let client = build_http_client()?;
    let latest: AuthlibLatest = client
        .get(AUTHLIB_INJECTOR_LATEST)
        .send()
        .map_err(|e| format!("Failed to fetch authlib-injector: {e}"))?
        .error_for_status()
        .map_err(|e| format!("authlib-injector HTTP error: {e}"))?
        .json()
        .map_err(|e| format!("authlib-injector parse error: {e}"))?;
    download_file(&client, &latest.download_url, &dest, None)?;
    Ok(dest)
}

/// Default public Azure App client ID for Minecraft Device Code Flow (XboxLive.signin).
/// Users can also override it in the Microsoft login form if they have their own Azure App.
pub const DEFAULT_MS_CLIENT_ID: &str = "c36a9fb6-4f2a-41ff-90bd-ae7cc92031eb";

const MS_DEVICE_CODE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const MS_TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_XBOX_URL: &str =
    "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const ELY_AUTH_URL: &str = "https://authserver.ely.by/auth/authenticate";
const ELY_VALIDATE_URL: &str = "https://authserver.ely.by/auth/validate";
const ELY_REFRESH_URL: &str = "https://authserver.ely.by/auth/refresh";

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: Option<u64>,
    message: String,
}

#[derive(Deserialize)]
struct MsTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

#[derive(Deserialize)]
struct MsTokenError {
    error: String,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct XblResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: XblDisplayClaims,
}

#[derive(Deserialize)]
struct XblDisplayClaims {
    xui: Vec<XblXui>,
}

#[derive(Deserialize)]
struct XblXui {
    uhs: String,
}

#[derive(Deserialize)]
struct McLoginResponse {
    access_token: String,
    expires_in: i64,
}

#[derive(Deserialize)]
struct McProfileResponse {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct ElyAuthResponse {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "clientToken")]
    client_token: String,
    #[serde(rename = "selectedProfile")]
    selected_profile: Option<ElyProfile>,
}

#[derive(Deserialize)]
struct ElyProfile {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct ElyErrorResponse {
    #[serde(rename = "errorMessage")]
    error_message: Option<String>,
    error: Option<String>,
}

/// Completes Xbox Live -> XSTS -> Minecraft Services authentication from a Microsoft OAuth token.
fn exchange_ms_token_for_minecraft(
    ms_access_token: &str,
    ms_refresh_token: &str,
) -> Result<Account, String> {
    let client = build_http_client()?;

    // 1. Xbox Live
    let xbl_body = serde_json::json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={ms_access_token}")
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });
    let xbl: XblResponse = client
        .post(XBL_AUTH_URL)
        .json(&xbl_body)
        .send()
        .map_err(|e| format!("Xbox Live request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Xbox Live HTTP error: {e}"))?
        .json()
        .map_err(|e| format!("Xbox Live parse error: {e}"))?;

    let uhs = xbl
        .display_claims
        .xui
        .first()
        .map(|x| x.uhs.clone())
        .ok_or_else(|| "Xbox Live response missing uhs claim".to_string())?;

    // 2. XSTS
    let xsts_body = serde_json::json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbl.token]
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    });
    let xsts_resp = client
        .post(XSTS_AUTH_URL)
        .json(&xsts_body)
        .send()
        .map_err(|e| format!("XSTS request failed: {e}"))?;
    if !xsts_resp.status().is_success() {
        let status = xsts_resp.status();
        let body = xsts_resp.text().unwrap_or_default();
        return Err(format!("XSTS auth failed ({status}): {body}"));
    }
    let xsts: XblResponse = xsts_resp
        .json()
        .map_err(|e| format!("XSTS parse error: {e}"))?;

    // 3. Minecraft login_with_xbox
    let mc_body = serde_json::json!({
        "identityToken": format!("XBL3.0 x={uhs};{}", xsts.token)
    });
    let mc_login: McLoginResponse = client
        .post(MC_LOGIN_XBOX_URL)
        .json(&mc_body)
        .send()
        .map_err(|e| format!("Minecraft login_with_xbox failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Minecraft login HTTP error: {e}"))?
        .json()
        .map_err(|e| format!("Minecraft login parse error: {e}"))?;

    // 4. Minecraft profile
    let profile_resp = client
        .get(MC_PROFILE_URL)
        .bearer_auth(&mc_login.access_token)
        .send()
        .map_err(|e| format!("Minecraft profile request failed: {e}"))?;
    if !profile_resp.status().is_success() {
        return Err("This Microsoft account does not own Minecraft Java Edition (or no profile exists yet).".to_string());
    }
    let profile: McProfileResponse = profile_resp
        .json()
        .map_err(|e| format!("Minecraft profile parse error: {e}"))?;

    let expires_at = chrono::Utc::now().timestamp() + mc_login.expires_in;

    Ok(Account {
        id: uuid::Uuid::new_v4().to_string(),
        username: profile.name,
        kind: AccountKind::Microsoft {
            uuid: profile.id,
            access_token: mc_login.access_token,
            refresh_token: ms_refresh_token.to_string(),
            expires_at,
        },
    })
}

/// Starts Microsoft Device Code Flow in a background thread and reports progress via `tx`.
pub fn start_microsoft_device_login(client_id: String, tx: Sender<WorkerEvent>) {
    let effective_client_id = if client_id.trim().is_empty() {
        DEFAULT_MS_CLIENT_ID.to_string()
    } else {
        client_id.trim().to_string()
    };

    thread::spawn(move || {
        let client = match build_http_client() {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(WorkerEvent::MsDeviceCodeReady(Err(e)));
                return;
            }
        };

        let resp = client
            .post(MS_DEVICE_CODE_URL)
            .form(&[
                ("client_id", effective_client_id.as_str()),
                ("scope", "XboxLive.signin offline_access"),
            ])
            .send();

        let dc: DeviceCodeResponse = match resp {
            Ok(r) if r.status().is_success() => match r.json() {
                Ok(parsed) => parsed,
                Err(e) => {
                    let _ = tx.send(WorkerEvent::MsDeviceCodeReady(Err(format!(
                        "Device code parse error: {e}"
                    ))));
                    return;
                }
            },
            Ok(r) => {
                let body = r.text().unwrap_or_default();
                let _ = tx.send(WorkerEvent::MsDeviceCodeReady(Err(format!(
                    "Microsoft OAuth error: {body}"
                ))));
                return;
            }
            Err(e) => {
                let _ = tx.send(WorkerEvent::MsDeviceCodeReady(Err(format!(
                    "Microsoft OAuth request failed: {e}"
                ))));
                return;
            }
        };

        let _ = tx.send(WorkerEvent::MsDeviceCodeReady(Ok(DeviceCodeInfo {
            user_code: dc.user_code,
            verification_uri: dc.verification_uri,
            message: dc.message,
        })));

        let interval = Duration::from_secs(dc.interval.unwrap_or(5).max(3));
        let deadline = std::time::Instant::now() + Duration::from_secs(dc.expires_in);

        while std::time::Instant::now() < deadline {
            thread::sleep(interval);

            let token_resp = client
                .post(MS_TOKEN_URL)
                .form(&[
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ("client_id", effective_client_id.as_str()),
                    ("device_code", dc.device_code.as_str()),
                ])
                .send();

            let Ok(r) = token_resp else {
                continue;
            };

            if r.status().is_success() {
                match r.json::<MsTokenResponse>() {
                    Ok(tok) => {
                        let refresh = tok.refresh_token.unwrap_or_default();
                        let res = exchange_ms_token_for_minecraft(&tok.access_token, &refresh);
                        let _ = tx.send(WorkerEvent::MsAuthFinished(res));
                        return;
                    }
                    Err(e) => {
                        let _ = tx.send(WorkerEvent::MsAuthFinished(Err(format!(
                            "Token parse error: {e}"
                        ))));
                        return;
                    }
                }
            } else {
                let err_body = r.text().unwrap_or_default();
                if let Ok(err) = serde_json::from_str::<MsTokenError>(&err_body) {
                    if err.error == "authorization_pending" || err.error == "slow_down" {
                        continue;
                    }
                    let msg = err.error_description.unwrap_or(err.error);
                    let _ = tx.send(WorkerEvent::MsAuthFinished(Err(msg)));
                    return;
                }
            }
        }

        let _ = tx.send(WorkerEvent::MsAuthFinished(Err(
            "Microsoft authentication timed out.".to_string(),
        )));
    });
}

/// Refreshes a Microsoft account's Minecraft access token if it is expired or close to expiring.
pub fn ensure_valid_account(account: &mut Account, ms_client_id: &str) -> Result<(), String> {
    match &mut account.kind {
        AccountKind::Offline => Ok(()),
        AccountKind::Microsoft {
            refresh_token,
            expires_at,
            ..
        } => {
            let now = chrono::Utc::now().timestamp();
            if now + 120 < *expires_at {
                return Ok(());
            }
            if refresh_token.is_empty() {
                return Err("Microsoft session expired. Please sign in again.".to_string());
            }
            let cid = if ms_client_id.trim().is_empty() {
                DEFAULT_MS_CLIENT_ID
            } else {
                ms_client_id.trim()
            };
            let client = build_http_client()?;
            let tok: MsTokenResponse = client
                .post(MS_TOKEN_URL)
                .form(&[
                    ("client_id", cid),
                    ("refresh_token", refresh_token.as_str()),
                    ("grant_type", "refresh_token"),
                    ("scope", "XboxLive.signin offline_access"),
                ])
                .send()
                .map_err(|e| format!("Microsoft token refresh failed: {e}"))?
                .error_for_status()
                .map_err(|e| format!("Microsoft token refresh HTTP error: {e}"))?
                .json()
                .map_err(|e| format!("Microsoft token refresh parse error: {e}"))?;

            let new_refresh = tok.refresh_token.unwrap_or_else(|| refresh_token.clone());
            let _ = tok.expires_in;
            let refreshed = exchange_ms_token_for_minecraft(&tok.access_token, &new_refresh)?;
            account.username = refreshed.username;
            account.kind = refreshed.kind;
            Ok(())
        }
        AccountKind::ElyBy {
            access_token,
            client_token,
            ..
        } => {
            let client = build_http_client()?;
            let validate_body = serde_json::json!({
                "accessToken": access_token.as_str(),
                "clientToken": client_token.as_str(),
            });
            let ok = client
                .post(ELY_VALIDATE_URL)
                .json(&validate_body)
                .send()
                .is_ok_and(|r| r.status().is_success());
            if ok {
                return Ok(());
            }

            let refresh_resp = client
                .post(ELY_REFRESH_URL)
                .json(&serde_json::json!({
                    "accessToken": access_token.as_str(),
                    "clientToken": client_token.as_str(),
                    "requestUser": true
                }))
                .send()
                .map_err(|e| format!("Ely.by refresh failed: {e}"))?;

            if !refresh_resp.status().is_success() {
                return Err("Ely.by session expired. Please sign in again.".to_string());
            }
            let parsed: ElyAuthResponse = refresh_resp
                .json()
                .map_err(|e| format!("Ely.by refresh parse error: {e}"))?;
            *access_token = parsed.access_token;
            *client_token = parsed.client_token;
            if let Some(p) = parsed.selected_profile {
                account.username = p.name;
            }
            Ok(())
        }
    }
}

/// Authenticates an Ely.by account via Yggdrasil-compatible authserver.ely.by.
pub fn login_ely_by(login: &str, password: &str, totp_code: &str) -> Result<Account, String> {
    let client = build_http_client()?;
    let client_token = uuid::Uuid::new_v4().to_string();
    let effective_password = if totp_code.trim().is_empty() {
        password.to_string()
    } else {
        format!("{password}:{}", totp_code.trim())
    };

    let body = serde_json::json!({
        "username": login.trim(),
        "password": effective_password,
        "clientToken": client_token,
        "requestUser": true
    });

    let resp = client
        .post(ELY_AUTH_URL)
        .json(&body)
        .send()
        .map_err(|e| format!("Ely.by auth request failed: {e}"))?;

    if !resp.status().is_success() {
        let text = resp.text().unwrap_or_default();
        if let Ok(err) = serde_json::from_str::<ElyErrorResponse>(&text) {
            let msg = err
                .error_message
                .or(err.error)
                .unwrap_or_else(|| "Ely.by login failed".to_string());
            return Err(msg);
        }
        return Err(format!("Ely.by auth error: {text}"));
    }

    let parsed: ElyAuthResponse = resp
        .json()
        .map_err(|e| format!("Ely.by response parse error: {e}"))?;
    let profile = parsed
        .selected_profile
        .ok_or_else(|| "Ely.by did not return a game profile".to_string())?;

    Ok(Account {
        id: uuid::Uuid::new_v4().to_string(),
        username: profile.name,
        kind: AccountKind::ElyBy {
            uuid: profile.id,
            access_token: parsed.access_token,
            client_token: parsed.client_token,
        },
    })
}

/// Returns a stable filesystem-safe cache key for an account's Minecraft skin.
pub fn account_skin_cache_key(account: &Account) -> String {
    let raw = match &account.kind {
        AccountKind::Microsoft { uuid, .. } => format!("ms_{}", uuid.replace('-', "")),
        AccountKind::ElyBy { .. } => format!("ely_{}", account.username.to_lowercase()),
        AccountKind::Offline => format!("off_{}", account.username.to_lowercase()),
    };
    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

fn decode_base64_str(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u8 = 0;
    for b in input.bytes() {
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        };
        buf = (buf << 6) | u32::from(val);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

#[derive(Deserialize)]
struct SessionProperty {
    name: String,
    value: String,
}

#[derive(Deserialize)]
struct SessionProfileResp {
    #[serde(default)]
    properties: Vec<SessionProperty>,
}

#[derive(Deserialize)]
struct TexturesPayload {
    textures: Option<TexturesMap>,
}

#[derive(Deserialize)]
struct TexturesMap {
    #[serde(rename = "SKIN")]
    skin: Option<SkinRef>,
}

#[derive(Deserialize)]
struct SkinRef {
    url: String,
}

fn fetch_mojang_skin_url(client: &reqwest::blocking::Client, uuid: &str) -> Option<String> {
    let clean = uuid.replace('-', "");
    let url = format!("https://sessionserver.mojang.com/session/minecraft/profile/{clean}");
    let resp: SessionProfileResp = client.get(&url).send().ok()?.error_for_status().ok()?.json().ok()?;
    let prop = resp.properties.into_iter().find(|p| p.name == "textures")?;
    let decoded = decode_base64_str(&prop.value)?;
    let parsed: TexturesPayload = serde_json::from_slice(&decoded).ok()?;
    parsed.textures?.skin.map(|s| s.url)
}

fn download_skin_bytes(client: &reqwest::blocking::Client, url: &str) -> Option<Vec<u8>> {
    let bytes = client
        .get(url)
        .send()
        .ok()?
        .error_for_status()
        .ok()?
        .bytes()
        .ok()?;
    if bytes.is_empty() {
        None
    } else {
        Some(bytes.to_vec())
    }
}

fn blend_rgba(dst: &mut [u8], src: [u8; 4]) {
    let sa = src[3] as f32 / 255.0;
    if sa <= 0.0 {
        return;
    }
    if sa >= 1.0 || dst[3] == 0 {
        dst.copy_from_slice(&src);
        return;
    }
    let inv = 1.0 - sa;
    dst[0] = (src[0] as f32 * sa + dst[0] as f32 * inv).round() as u8;
    dst[1] = (src[1] as f32 * sa + dst[1] as f32 * inv).round() as u8;
    dst[2] = (src[2] as f32 * sa + dst[2] as f32 * inv).round() as u8;
    dst[3] = ((sa + (dst[3] as f32 / 255.0) * inv) * 255.0).round() as u8;
}

fn blit_region(
    skin: &image::RgbaImage,
    src_x: u32,
    src_y: u32,
    w: u32,
    h: u32,
    dst: &mut [u8],
    dst_stride: usize,
    dst_x: usize,
    dst_y: usize,
    mirror_x: bool,
    overlay: bool,
) {
    if src_x + w > skin.width() || src_y + h > skin.height() {
        return;
    }
    for dy in 0..h {
        for dx in 0..w {
            let sx = src_x + if mirror_x { w - 1 - dx } else { dx };
            let sy = src_y + dy;
            let px = skin.get_pixel(sx, sy).0;
            let off = ((dst_y + dy as usize) * dst_stride + (dst_x + dx as usize)) * 4;
            if overlay {
                blend_rgba(&mut dst[off..off + 4], px);
            } else {
                dst[off..off + 4].copy_from_slice(&px);
            }
        }
    }
}

fn upscale_nearest(src: &[u8], w: usize, h: usize, scale: usize) -> IconImage {
    let out_w = w * scale;
    let out_h = h * scale;
    let mut out = vec![0_u8; out_w * out_h * 4];
    for y in 0..out_h {
        let sy = y / scale;
        for x in 0..out_w {
            let sx = x / scale;
            let s_off = (sy * w + sx) * 4;
            let d_off = (y * out_w + x) * 4;
            out[d_off..d_off + 4].copy_from_slice(&src[s_off..s_off + 4]);
        }
    }
    IconImage {
        width: out_w,
        height: out_h,
        rgba: out,
    }
}

/// Extracts an upscaled 32x32 head avatar and 64x128 front paperdoll from a raw Minecraft skin PNG.
pub fn decode_skin_head_and_body(png_bytes: &[u8]) -> Result<(IconImage, IconImage), String> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| format!("Invalid skin image: {e}"))?
        .to_rgba8();
    if img.width() < 64 || img.height() < 32 {
        return Err(format!("Unsupported skin dimensions {}x{}", img.width(), img.height()));
    }
    let modern = img.height() >= 64;

    let mut head = vec![0_u8; 8 * 8 * 4];
    blit_region(&img, 8, 8, 8, 8, &mut head, 8, 0, 0, false, false);
    blit_region(&img, 40, 8, 8, 8, &mut head, 8, 0, 0, false, true);
    let head_scaled = upscale_nearest(&head, 8, 8, 4);

    let mut body = vec![0_u8; 16 * 32 * 4];
    // Head
    blit_region(&img, 8, 8, 8, 8, &mut body, 16, 4, 0, false, false);
    blit_region(&img, 40, 8, 8, 8, &mut body, 16, 4, 0, false, true);
    // Torso
    blit_region(&img, 20, 20, 8, 12, &mut body, 16, 4, 8, false, false);
    if modern {
        blit_region(&img, 20, 36, 8, 12, &mut body, 16, 4, 8, false, true);
    }
    // Right arm (viewer left)
    blit_region(&img, 44, 20, 4, 12, &mut body, 16, 0, 8, false, false);
    if modern {
        blit_region(&img, 44, 36, 4, 12, &mut body, 16, 0, 8, false, true);
    }
    // Left arm (viewer right)
    if modern {
        blit_region(&img, 36, 52, 4, 12, &mut body, 16, 12, 8, false, false);
        blit_region(&img, 52, 52, 4, 12, &mut body, 16, 12, 8, false, true);
    } else {
        blit_region(&img, 44, 20, 4, 12, &mut body, 16, 12, 8, true, false);
    }
    // Right leg (viewer left)
    blit_region(&img, 4, 20, 4, 12, &mut body, 16, 4, 20, false, false);
    if modern {
        blit_region(&img, 4, 36, 4, 12, &mut body, 16, 4, 20, false, true);
    }
    // Left leg (viewer right)
    if modern {
        blit_region(&img, 20, 52, 4, 12, &mut body, 16, 8, 20, false, false);
        blit_region(&img, 4, 52, 4, 12, &mut body, 16, 8, 20, false, true);
    } else {
        blit_region(&img, 4, 20, 4, 12, &mut body, 16, 8, 20, true, false);
    }
    let body_scaled = upscale_nearest(&body, 16, 32, 4);

    Ok((head_scaled, body_scaled))
}

/// Fetches and caches the account's skin PNG, returning upscaled head and paperdoll RGBA images.
pub fn fetch_account_skin(
    paths: &LauncherPaths,
    account: &Account,
    force_refresh: bool,
) -> Result<(IconImage, IconImage), String> {
    let key = account_skin_cache_key(account);
    let skins_dir = paths.root.join("skins");
    let cache_path = skins_dir.join(format!("{key}.png"));

    if !force_refresh && cache_path.is_file() {
        if let Ok(bytes) = std::fs::read(&cache_path) {
            if let Ok(decoded) = decode_skin_head_and_body(&bytes) {
                return Ok(decoded);
            }
        }
    }

    let client = build_http_client()?;
    let png_bytes = match &account.kind {
        AccountKind::Microsoft { uuid, .. } => {
            let mojang_url = fetch_mojang_skin_url(&client, uuid);
            mojang_url
                .as_deref()
                .and_then(|u| download_skin_bytes(&client, u))
                .or_else(|| {
                    download_skin_bytes(
                        &client,
                        &format!("https://minotar.net/skin/{}", account.username),
                    )
                })
        }
        AccountKind::ElyBy { .. } => download_skin_bytes(
            &client,
            &format!("https://skinsystem.ely.by/skins/{}.png", account.username),
        )
        .or_else(|| {
            download_skin_bytes(
                &client,
                &format!("http://skinsystem.ely.by/skins/{}.png", account.username),
            )
        }),
        AccountKind::Offline => download_skin_bytes(
            &client,
            &format!("https://minotar.net/skin/{}", account.username),
        ),
    }
    .ok_or_else(|| format!("No skin found for {}", account.username))?;

    let decoded = decode_skin_head_and_body(&png_bytes)?;
    let _ = std::fs::create_dir_all(&skins_dir);
    let _ = std::fs::write(&cache_path, &png_bytes);
    Ok(decoded)
}

/// Compares two semver strings (ignoring optional `v` prefix and `-prerelease` suffix).
pub fn is_newer_semver(latest_tag: &str, current: &str) -> bool {
    let parse_triple = |s: &str| -> (u32, u32, u32) {
        let core = s
            .trim()
            .trim_start_matches(['v', 'V'])
            .split(['-', '+'])
            .next()
            .unwrap_or("");
        let mut it = core.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
        (
            it.next().unwrap_or(0),
            it.next().unwrap_or(0),
            it.next().unwrap_or(0),
        )
    };
    parse_triple(latest_tag) > parse_triple(current)
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

#[derive(Deserialize)]
struct GithubReleaseResp {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

fn select_release_asset<'a>(
    assets: &'a [GithubAsset],
    os: &str,
    arch: &str,
) -> Option<&'a GithubAsset> {
    let preferred_names: &[&str] = match (os, arch) {
        ("linux", "x86_64") => &["sunset-launcher-linux-x64", "svarka-launcher-linux-x64"],
        ("windows", "x86_64") => &[
            "sunset-launcher-windows-x64.exe",
            "svarka-launcher-windows-x64.exe",
        ],
        _ => &[],
    };
    for exact in preferred_names {
        if let Some(found) = assets.iter().find(|a| a.name.eq_ignore_ascii_case(exact)) {
            return Some(found);
        }
    }

    let os_tokens: &[&str] = match os {
        "linux" => &["linux"],
        "windows" => &["windows", "win"],
        "macos" => &["macos", "darwin", "osx"],
        _ => return None,
    };
    let arch_tokens: &[&str] = match arch {
        "x86_64" => &["x86_64", "x64", "amd64"],
        "aarch64" => &["aarch64", "arm64"],
        _ => &[],
    };

    let mut best: Option<(u8, &GithubAsset)> = None;
    for a in assets {
        let lower = a.name.to_ascii_lowercase();
        if [
            ".sha256", ".sha512", ".sig", ".asc", ".txt", ".json", ".deb", ".rpm", ".dmg",
            ".appimage",
        ]
        .iter()
        .any(|ext| lower.ends_with(ext))
        {
            continue;
        }
        if !os_tokens.iter().any(|t| lower.contains(t)) {
            continue;
        }
        if !arch_tokens.is_empty() && !arch_tokens.iter().any(|t| lower.contains(t)) {
            continue;
        }
        let is_zip = lower.ends_with(".zip");
        if os == "windows" && !lower.ends_with(".exe") && !is_zip {
            continue;
        }
        let score = if is_zip { 1 } else { 2 };
        if best.is_none_or(|(best_s, _)| score > best_s) {
            best = Some((score, a));
        }
    }
    best.map(|(_, a)| a)
}

fn verify_executable_magic(head: &[u8], is_zip: bool, os: &str) -> Result<(), String> {
    if is_zip {
        return if head.starts_with(b"PK\x03\x04") {
            Ok(())
        } else {
            Err("Downloaded update archive is invalid (bad ZIP header)".to_string())
        };
    }
    let valid = match os {
        "linux" => head.starts_with(b"\x7fELF"),
        "windows" => head.starts_with(b"MZ"),
        "macos" => {
            head.starts_with(b"\xcf\xfa\xed\xfe")
                || head.starts_with(b"\xce\xfa\xed\xfe")
                || head.starts_with(b"\xfe\xed\xfa\xcf")
                || head.starts_with(b"\xca\xfe\xba\xbe")
        }
        _ => !head.is_empty(),
    };
    if valid {
        Ok(())
    } else {
        Err("Downloaded file is not a valid executable for this platform".to_string())
    }
}

fn extract_binary_from_zip(zip_path: &Path, dest_path: &Path, os: &str) -> Result<(), String> {
    let file = std::fs::File::open(zip_path)
        .map_err(|e| format!("Failed to open update archive {}: {e}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Failed to read update archive: {e}"))?;

    let mut chosen_idx: Option<(u64, usize)> = None;
    for idx in 0..archive.len() {
        let entry = archive
            .by_index(idx)
            .map_err(|e| format!("Failed to inspect update archive entry: {e}"))?;
        if !entry.is_file() {
            continue;
        }
        let name = entry.name().to_ascii_lowercase();
        if os == "windows" {
            if !name.ends_with(".exe") {
                continue;
            }
        } else if [".txt", ".md", ".sha256", ".json", ".desktop", ".png", ".svg"]
            .iter()
            .any(|ext| name.ends_with(ext))
        {
            continue;
        }
        let sz = entry.size();
        if chosen_idx.is_none_or(|(best_sz, _)| sz > best_sz) {
            chosen_idx = Some((sz, idx));
        }
    }

    let (_, idx) = chosen_idx
        .ok_or_else(|| "Update archive does not contain a runnable binary".to_string())?;
    let mut entry = archive
        .by_index(idx)
        .map_err(|e| format!("Failed to extract update binary: {e}"))?;
    let mut out = std::fs::File::create(dest_path)
        .map_err(|e| format!("Failed to create {}: {e}", dest_path.display()))?;
    std::io::copy(&mut entry, &mut out)
        .map_err(|e| format!("Failed to write extracted binary: {e}"))?;
    Ok(())
}

#[cfg_attr(not(windows), allow(dead_code))]
fn pick_windows_backup_path(target_exe: &Path, file_name: &str) -> PathBuf {
    let primary = target_exe.with_file_name(format!("{file_name}.old"));
    if primary.exists() {
        let _ = std::fs::remove_file(&primary);
    }
    if !primary.exists() {
        return primary;
    }
    let pid = std::process::id();
    for attempt in 0..32_u32 {
        let suffix = if attempt == 0 {
            format!("{file_name}.old.{pid}")
        } else {
            format!("{file_name}.old.{pid}.{attempt}")
        };
        let candidate = target_exe.with_file_name(suffix);
        if candidate.exists() {
            let _ = std::fs::remove_file(&candidate);
        }
        if !candidate.exists() {
            return candidate;
        }
    }
    primary
}

fn replace_executable_file(staged_in_target_dir: &Path, target_exe: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(staged_in_target_dir, perms)
            .map_err(|e| format!("Failed to mark update executable: {e}"))?;
        std::fs::rename(staged_in_target_dir, target_exe)
            .map_err(|e| format!("Failed to replace {}: {e}", target_exe.display()))?;
        Ok(())
    }

    #[cfg(windows)]
    {
        let file_name = target_exe
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "sunset-launcher.exe".to_string());
        let backup = pick_windows_backup_path(target_exe, &file_name);
        std::fs::rename(target_exe, &backup)
            .map_err(|e| format!("Failed to move old executable: {e}"))?;
        if let Err(e) = std::fs::rename(staged_in_target_dir, target_exe) {
            let _ = std::fs::rename(&backup, target_exe);
            return Err(format!("Failed to install new executable: {e}"));
        }
        Ok(())
    }
}

fn cleanup_update_artifacts_in_dir(dir: &Path, exe_name: &str) -> Vec<PathBuf> {
    let staged = format!(".{exe_name}.update.tmp");
    let old_exact = format!("{exe_name}.old");
    let old_prefix = format!("{exe_name}.old.");
    let mut remaining = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let entry_name = entry.file_name().to_string_lossy().to_string();
            let matches = if cfg!(windows) {
                let lower = entry_name.to_ascii_lowercase();
                lower == staged.to_ascii_lowercase()
                    || lower == old_exact.to_ascii_lowercase()
                    || lower.starts_with(&old_prefix.to_ascii_lowercase())
            } else {
                entry_name == staged
                    || entry_name == old_exact
                    || entry_name.starts_with(&old_prefix)
            };
            if matches {
                let _ = std::fs::remove_file(&path);
                if path.exists() {
                    remaining.push(path);
                }
            }
        }
    } else {
        for fallback in [dir.join(&old_exact), dir.join(&staged)] {
            if fallback.exists() {
                let _ = std::fs::remove_file(&fallback);
                if fallback.exists() {
                    remaining.push(fallback);
                }
            }
        }
    }

    remaining
}

/// Removes temporary backup or staging files left behind by an earlier self-update.
pub fn cleanup_stale_update_files() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else {
        return;
    };
    let name = exe
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if name.is_empty() {
        return;
    }
    let remaining = cleanup_update_artifacts_in_dir(dir, &name);
    #[cfg(windows)]
    if !remaining.is_empty() {
        std::thread::spawn(move || {
            let mut pending = remaining;
            for delay_ms in [400_u64, 1000, 2000] {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                pending.retain(|p| p.exists() && std::fs::remove_file(p).is_err());
                if pending.is_empty() {
                    break;
                }
            }
        });
    }
    #[cfg(not(windows))]
    let _ = remaining;
}

/// Checks GitHub Releases (`sunset-launcher/sunset-launcher`) for a newer version than `current_version`.
pub fn check_github_latest_release(current_version: &str) -> Result<Option<LauncherUpdateInfo>, String> {
    let client = build_http_client()?;
    let resp = client
        .get("https://api.github.com/repos/sunset-launcher/sunset-launcher/releases/latest")
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| format!("Update check failed: {e}"))?;
    if resp.status().as_u16() == 404 {
        return Ok(None);
    }
    let rel: GithubReleaseResp = resp
        .error_for_status()
        .map_err(|e| format!("GitHub API error: {e}"))?
        .json()
        .map_err(|e| format!("Invalid GitHub release payload: {e}"))?;
    if is_newer_semver(&rel.tag_name, current_version) {
        let asset = select_release_asset(
            &rel.assets,
            std::env::consts::OS,
            std::env::consts::ARCH,
        );
        Ok(Some(LauncherUpdateInfo {
            version: rel.tag_name,
            html_url: rel.html_url,
            notes: rel.body,
            download_url: asset.map(|a| a.browser_download_url.clone()),
            asset_name: asset.map(|a| a.name.clone()),
            asset_size: asset.and_then(|a| (a.size > 0).then_some(a.size)),
        }))
    } else {
        Ok(None)
    }
}

/// Downloads the release binary for the current OS/architecture, verifies it, and atomically replaces `current_exe`.
pub fn download_and_apply_update(
    paths: &LauncherPaths,
    info: &LauncherUpdateInfo,
    tx: &Sender<WorkerEvent>,
) -> Result<PathBuf, String> {
    let download_url = info
        .download_url
        .as_deref()
        .ok_or_else(|| "No downloadable binary asset found for this platform".to_string())?;
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to locate current executable: {e}"))?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "Cannot determine launcher directory".to_string())?;
    let exe_name = exe_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| "Cannot determine executable filename".to_string())?;

    let updates_dir = paths.root.join("updates");
    std::fs::create_dir_all(&updates_dir)
        .map_err(|e| format!("Failed to create updates folder: {e}"))?;

    let asset_name = info.asset_name.as_deref().unwrap_or("sunset-launcher-update");
    let is_zip = asset_name.to_ascii_lowercase().ends_with(".zip");
    let dl_path = updates_dir.join(format!("download-{asset_name}"));

    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Downloading update {}...", info.version),
        fraction: 0.05,
    });

    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("sunset-launcher/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;
    let mut resp = client
        .get(download_url)
        .send()
        .map_err(|e| format!("Update download failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Update download HTTP error: {e}"))?;

    let expected_total = resp.content_length().or(info.asset_size).unwrap_or(0);
    let mut out_file = std::fs::File::create(&dl_path)
        .map_err(|e| format!("Failed to create {}: {e}", dl_path.display()))?;
    let mut buf = [0_u8; 65_536];
    let mut downloaded: u64 = 0;
    let mut head = Vec::with_capacity(8);
    let mut last_pct: u8 = 0;

    loop {
        let n = resp
            .read(&mut buf)
            .map_err(|e| format!("Failed while downloading update: {e}"))?;
        if n == 0 {
            break;
        }
        if head.len() < 8 {
            let take = (8 - head.len()).min(n);
            head.extend_from_slice(&buf[..take]);
        }
        out_file
            .write_all(&buf[..n])
            .map_err(|e| format!("Failed to write update file: {e}"))?;
        downloaded += n as u64;
        if expected_total > 0 {
            let pct = ((downloaded * 80) / expected_total).min(80) as u8;
            if pct >= last_pct + 4 {
                last_pct = pct;
                let _ = tx.send(WorkerEvent::Progress {
                    stage: format!(
                        "Downloading update {} ({:.1} / {:.1} MB)...",
                        info.version,
                        downloaded as f64 / 1_048_576.0,
                        expected_total as f64 / 1_048_576.0
                    ),
                    fraction: 0.05 + (pct as f32 / 100.0),
                });
            }
        }
    }
    drop(out_file);

    if downloaded == 0 {
        let _ = std::fs::remove_file(&dl_path);
        return Err("Downloaded update file is empty".to_string());
    }
    if let Some(expected) = info.asset_size {
        if expected > 0 && downloaded != expected {
            let _ = std::fs::remove_file(&dl_path);
            return Err(format!(
                "Incomplete update download ({downloaded} of {expected} bytes)"
            ));
        }
    }
    if let Err(e) = verify_executable_magic(&head, is_zip, std::env::consts::OS) {
        let _ = std::fs::remove_file(&dl_path);
        return Err(e);
    }

    let _ = tx.send(WorkerEvent::Progress {
        stage: "Installing update...".to_string(),
        fraction: 0.9,
    });

    let extracted_path = updates_dir.join(format!("extracted-{exe_name}"));
    let payload_path = if is_zip {
        if let Err(e) = extract_binary_from_zip(&dl_path, &extracted_path, std::env::consts::OS) {
            let _ = std::fs::remove_file(&dl_path);
            let _ = std::fs::remove_file(&extracted_path);
            return Err(e);
        }
        let mut magic = [0_u8; 4];
        if let Ok(mut f) = std::fs::File::open(&extracted_path) {
            let n = f.read(&mut magic).unwrap_or(0);
            if let Err(e) = verify_executable_magic(&magic[..n], false, std::env::consts::OS) {
                let _ = std::fs::remove_file(&dl_path);
                let _ = std::fs::remove_file(&extracted_path);
                return Err(e);
            }
        }
        &extracted_path
    } else {
        &dl_path
    };

    let staged_path = exe_dir.join(format!(".{exe_name}.update.tmp"));
    if let Err(e) = std::fs::copy(payload_path, &staged_path) {
        let _ = std::fs::remove_file(&staged_path);
        let _ = std::fs::remove_file(&dl_path);
        let _ = std::fs::remove_file(&extracted_path);
        return Err(format!(
            "Cannot write update to {}: {e}",
            exe_dir.display()
        ));
    }

    if let Err(e) = replace_executable_file(&staged_path, &exe_path) {
        let _ = std::fs::remove_file(&staged_path);
        let _ = std::fs::remove_file(&dl_path);
        let _ = std::fs::remove_file(&extracted_path);
        return Err(e);
    }

    let _ = std::fs::remove_file(&dl_path);
    let _ = std::fs::remove_file(&extracted_path);
    let _ = tx.send(WorkerEvent::Progress {
        stage: format!("Updated to {}", info.version),
        fraction: 1.0,
    });
    Ok(exe_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_semver() {
        assert!(is_newer_semver("v0.2.0", "0.1.1"));
        assert!(is_newer_semver("v0.1.2", "0.1.1"));
        assert!(!is_newer_semver("v0.1.1", "0.1.1"));
        assert!(!is_newer_semver("0.1.0", "0.1.1"));
    }

    #[test]
    fn test_decode_base64_and_skin_regions() {
        assert_eq!(decode_base64_str("SGVsbG8=").unwrap(), b"Hello");
        let mut rgba = image::RgbaImage::new(64, 64);
        // Set head pixel (8, 8) red, hat pixel (40, 8) semi-transparent blue
        rgba.put_pixel(8, 8, image::Rgba([200, 20, 20, 255]));
        rgba.put_pixel(40, 8, image::Rgba([20, 20, 200, 128]));
        let mut png_bytes = Vec::new();
        image::DynamicImage::ImageRgba8(rgba)
            .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
            .unwrap();
        let (head, body) = decode_skin_head_and_body(&png_bytes).unwrap();
        assert_eq!((head.width, head.height), (32, 32));
        assert_eq!((body.width, body.height), (64, 128));
        assert!(head.rgba[0] > 50 && head.rgba[2] > 50);
    }

    #[test]
    fn test_select_release_asset_matches_os_and_skips_signatures() {
        let legacy_assets = vec![
            GithubAsset {
                name: "svarka-launcher-linux-x64.sha256".to_string(),
                browser_download_url: "https://example.invalid/linux.sha256".to_string(),
                size: 64,
            },
            GithubAsset {
                name: "svarka-launcher-linux-x64".to_string(),
                browser_download_url: "https://example.invalid/linux-bin".to_string(),
                size: 12_000_000,
            },
            GithubAsset {
                name: "svarka-launcher-windows-x64.exe".to_string(),
                browser_download_url: "https://example.invalid/win-bin".to_string(),
                size: 13_000_000,
            },
        ];
        assert_eq!(
            select_release_asset(&legacy_assets, "linux", "x86_64")
                .unwrap()
                .name,
            "svarka-launcher-linux-x64"
        );
        assert_eq!(
            select_release_asset(&legacy_assets, "windows", "x86_64")
                .unwrap()
                .name,
            "svarka-launcher-windows-x64.exe"
        );

        let sunset_assets = vec![
            GithubAsset {
                name: "sunset-launcher-linux-x64.sha256".to_string(),
                browser_download_url: "https://example.invalid/sunset-linux.sha256".to_string(),
                size: 64,
            },
            GithubAsset {
                name: "sunset-launcher-linux-x64".to_string(),
                browser_download_url: "https://example.invalid/sunset-linux".to_string(),
                size: 12_000_000,
            },
            GithubAsset {
                name: "sunset-launcher-windows-x64.exe".to_string(),
                browser_download_url: "https://example.invalid/sunset-win".to_string(),
                size: 13_000_000,
            },
        ];
        assert_eq!(
            select_release_asset(&sunset_assets, "linux", "x86_64")
                .unwrap()
                .name,
            "sunset-launcher-linux-x64"
        );
        assert_eq!(
            select_release_asset(&sunset_assets, "windows", "x86_64")
                .unwrap()
                .name,
            "sunset-launcher-windows-x64.exe"
        );
        assert!(select_release_asset(&sunset_assets, "macos", "aarch64").is_none());
    }

    #[test]
    fn test_verify_executable_magic_and_atomic_replace() {
        assert!(verify_executable_magic(b"\x7fELF\x02\x01\x01\x00", false, "linux").is_ok());
        assert!(verify_executable_magic(b"<html>not found", false, "linux").is_err());
        assert!(verify_executable_magic(b"MZ\x90\x00", false, "windows").is_ok());
        assert!(verify_executable_magic(b"PK\x03\x04\x14\x00", true, "linux").is_ok());

        let tmp_dir = std::env::temp_dir().join(format!("mc-updater-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();
        let target = tmp_dir.join("launcher");
        let staged = tmp_dir.join(".launcher.update.tmp");
        std::fs::write(&target, b"old-bin").unwrap();
        std::fs::write(&staged, b"\x7fELF-new-bin").unwrap();
        replace_executable_file(&staged, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"\x7fELF-new-bin");
        assert!(!staged.exists());
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_windows_backup_fallback_and_stale_cleanup() {
        let tmp_dir = std::env::temp_dir().join(format!("mc-win-updater-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let exe_name = "svarka-launcher-windows-x64.exe";
        let target = tmp_dir.join(exe_name);
        std::fs::write(&target, b"MZ-cur").unwrap();

        // Unlocked .old should be cleared and reused as primary backup path.
        let primary = tmp_dir.join(format!("{exe_name}.old"));
        std::fs::write(&primary, b"stale").unwrap();
        assert_eq!(pick_windows_backup_path(&target, exe_name), primary);
        assert!(!primary.exists());

        // When primary backup cannot be unlinked by remove_file (simulated with a directory),
        // pick_windows_backup_path falls back to a PID-suffixed .old.<pid> path.
        std::fs::create_dir_all(&primary).unwrap();
        let fallback = pick_windows_backup_path(&target, exe_name);
        assert_ne!(fallback, primary);
        assert!(fallback
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(&format!("{exe_name}.old.")));
        std::fs::remove_dir_all(&primary).unwrap();

        // Stale .old, .old.<pid>, and .<exe>.update.tmp files are cleaned, unrelated files kept.
        let old1 = tmp_dir.join(format!("{exe_name}.old"));
        let old2 = tmp_dir.join(format!("{exe_name}.old.99999"));
        let staged = tmp_dir.join(format!(".{exe_name}.update.tmp"));
        let keep = tmp_dir.join("other-app.exe.old");
        std::fs::write(&old1, b"1").unwrap();
        std::fs::write(&old2, b"2").unwrap();
        std::fs::write(&staged, b"3").unwrap();
        std::fs::write(&keep, b"4").unwrap();

        let remaining = cleanup_update_artifacts_in_dir(&tmp_dir, exe_name);
        assert!(remaining.is_empty());
        assert!(!old1.exists());
        assert!(!old2.exists());
        assert!(!staged.exists());
        assert!(target.exists());
        assert!(keep.exists());

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}

