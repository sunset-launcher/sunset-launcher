use crate::models::{Account, AccountKind, DeviceCodeInfo, LauncherPaths, WorkerEvent};
use crate::mojang::{build_http_client, download_file};
use serde::Deserialize;
use std::path::PathBuf;
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

