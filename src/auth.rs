use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use rand::Rng;
use rand::distributions::Alphanumeric;
use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::cli::AuthLoginCommand;
use crate::config::{AppConfig, AuthMode, ConfigManager, SecretStore};
use crate::error::{AppError, AppResult};

const AUTHORIZE_URL: &str = "https://apis.roblox.com/oauth/v1/authorize";
const TOKEN_URL: &str = "https://apis.roblox.com/oauth/v1/token";
const REVOKE_URL: &str = "https://apis.roblox.com/oauth/v1/token/revoke";
const USERINFO_URL: &str = "https://apis.roblox.com/oauth/v1/userinfo";
const DEFAULT_REDIRECT_PORT: u16 = 9785;
const OAUTH_CALLBACK_TIMEOUT_SECONDS: u64 = 300;
const ACCESS_TOKEN_REFRESH_SKEW_SECONDS: u64 = 30;
const DEFAULT_SCOPES: &[&str] = &["openid", "profile", "asset:write"];

pub trait AuthProvider {
    fn apply(&self, builder: RequestBuilder) -> RequestBuilder;
}

#[derive(Debug, Clone)]
pub struct ApiKeyAuth {
    api_key: String,
}

impl ApiKeyAuth {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

impl AuthProvider for ApiKeyAuth {
    fn apply(&self, builder: RequestBuilder) -> RequestBuilder {
        builder.header("x-api-key", &self.api_key)
    }
}

#[derive(Debug, Clone)]
pub struct OAuthBearerAuth {
    access_token: String,
}

impl OAuthBearerAuth {
    pub fn new(access_token: String) -> Self {
        Self { access_token }
    }
}

impl AuthProvider for OAuthBearerAuth {
    fn apply(&self, builder: RequestBuilder) -> RequestBuilder {
        builder.bearer_auth(&self.access_token)
    }
}

pub type SharedAuthProvider = Arc<dyn AuthProvider + Send + Sync>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredOAuthSession {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub token_type: String,
    pub scope: Vec<String>,
    pub expires_at_unix: u64,
    #[serde(default)]
    pub user_info: Option<OAuthUserInfo>,
    pub client_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthUserInfo {
    pub sub: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(rename = "preferred_username", default)]
    pub preferred_username: Option<String>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
    #[serde(default)]
    pub created_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthStatus {
    #[serde(rename = "authMode")]
    pub auth_mode: String,
    #[serde(rename = "clientId", skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(rename = "userId", skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(rename = "displayName", skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(rename = "username", skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(rename = "expiresAtUnix", skip_serializing_if = "Option::is_none")]
    pub expires_at_unix: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogoutStatus {
    #[serde(rename = "loggedOut")]
    pub logged_out: bool,
    #[serde(rename = "revoked")]
    pub revoked: bool,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    token_type: String,
    expires_in: u64,
    #[serde(default)]
    scope: String,
}

#[derive(Debug)]
struct OAuthCallback {
    code: String,
}

pub async fn resolve_auth_provider<S: SecretStore>(
    config_manager: &ConfigManager<S>,
) -> AppResult<SharedAuthProvider> {
    let config = config_manager.load()?;

    match config_manager.resolve_auth_mode(&config)? {
        AuthMode::ApiKey => config_manager
            .get_api_key()?
            .map(|api_key| Arc::new(ApiKeyAuth::new(api_key)) as SharedAuthProvider)
            .ok_or_else(|| {
                AppError::auth("no API key configured. Run `rbxup config set api-key <key>`")
            }),
        AuthMode::OAuth => {
            let session = load_active_oauth_session(config_manager, &config).await?;
            Ok(Arc::new(OAuthBearerAuth::new(session.access_token)))
        }
        AuthMode::Auto => {
            if let Some(api_key) = config_manager.get_api_key()? {
                return Ok(Arc::new(ApiKeyAuth::new(api_key)));
            }

            let session = load_active_oauth_session(config_manager, &config).await?;
            Ok(Arc::new(OAuthBearerAuth::new(session.access_token)))
        }
    }
}

pub async fn login<S: SecretStore>(
    config_manager: &ConfigManager<S>,
    args: AuthLoginCommand,
) -> AppResult<AuthStatus> {
    let mut config = config_manager.load()?;
    let client_id = args
        .client_id
        .clone()
        .or(config_manager.resolve_oauth_client_id(&config)?)
        .ok_or_else(|| {
            AppError::invalid_args(
                "OAuth login requires --client-id or RBXUP_OAUTH_CLIENT_ID the first time",
            )
        })?;
    let redirect_port = args
        .redirect_port
        .or(config_manager.resolve_oauth_redirect_port(&config)?)
        .unwrap_or(DEFAULT_REDIRECT_PORT);
    let scopes = if args.scopes.is_empty() {
        DEFAULT_SCOPES
            .iter()
            .map(|value| (*value).to_string())
            .collect()
    } else {
        args.scopes.clone()
    };
    let redirect_uri = format!("http://127.0.0.1:{redirect_port}/callback");
    let state = random_urlsafe(32);
    let nonce = random_urlsafe(32);
    let code_verifier = random_urlsafe(64);
    let code_challenge = pkce_challenge(&code_verifier);
    let timeout = Duration::from_secs(OAUTH_CALLBACK_TIMEOUT_SECONDS);
    let callback_listener = std::thread::spawn({
        let state = state.clone();
        let redirect_uri = redirect_uri.clone();
        move || wait_for_oauth_callback(redirect_port, &state, &redirect_uri, timeout)
    });

    let authorization_url = build_authorization_url(
        &client_id,
        &redirect_uri,
        &scopes,
        &state,
        &nonce,
        &code_challenge,
    )?;
    if let Err(error) = open::that(authorization_url.as_str()) {
        eprintln!("Could not open a browser automatically: {error}");
        eprintln!("Open this URL to continue OAuth login:");
        eprintln!("{authorization_url}");
    }

    let callback = callback_listener
        .join()
        .map_err(|_| AppError::general("OAuth callback listener crashed"))??;
    let token_response =
        exchange_authorization_code(&client_id, &redirect_uri, &callback.code, &code_verifier)
            .await?;
    let user_info = fetch_user_info(&token_response.access_token).await.ok();
    let session = build_session(&client_id, token_response, user_info.clone());
    config_manager.set_oauth_session(&serialize_session(&session)?)?;

    config.auth.mode = AuthMode::OAuth;
    config.auth.oauth_client_id = Some(client_id.clone());
    config.auth.oauth_redirect_port = Some(redirect_port);
    config_manager.save(&config)?;

    Ok(build_auth_status(
        "oauth",
        Some(&session),
        user_info.as_ref(),
    ))
}

pub async fn logout<S: SecretStore>(config_manager: &ConfigManager<S>) -> AppResult<LogoutStatus> {
    let mut config = config_manager.load()?;
    let mut revoked = false;

    if let Some(session_json) = config_manager.get_oauth_session()? {
        let session = parse_session(&session_json)?;
        if let Some(refresh_token) = &session.refresh_token {
            revoked = revoke_refresh_token(&session.client_id, refresh_token)
                .await
                .is_ok();
        }
    }

    config_manager.clear_oauth_session()?;
    if config.auth.mode == AuthMode::OAuth {
        config.auth.mode = AuthMode::Auto;
        config_manager.save(&config)?;
    }

    Ok(LogoutStatus {
        logged_out: true,
        revoked,
    })
}

pub async fn whoami<S: SecretStore>(config_manager: &ConfigManager<S>) -> AppResult<AuthStatus> {
    let config = config_manager.load()?;
    match config_manager.resolve_auth_mode(&config)? {
        AuthMode::ApiKey => Ok(AuthStatus {
            auth_mode: "api_key".to_string(),
            client_id: None,
            user_id: None,
            display_name: None,
            username: None,
            expires_at_unix: None,
            scopes: None,
        }),
        AuthMode::OAuth => {
            let session = load_active_oauth_session(config_manager, &config).await?;
            Ok(build_auth_status(
                "oauth",
                Some(&session),
                session.user_info.as_ref(),
            ))
        }
        AuthMode::Auto => {
            if config_manager.get_api_key()?.is_some() {
                return Ok(AuthStatus {
                    auth_mode: "api_key".to_string(),
                    client_id: None,
                    user_id: None,
                    display_name: None,
                    username: None,
                    expires_at_unix: None,
                    scopes: None,
                });
            }

            let session = load_active_oauth_session(config_manager, &config).await?;
            Ok(build_auth_status(
                "oauth",
                Some(&session),
                session.user_info.as_ref(),
            ))
        }
    }
}

async fn load_active_oauth_session<S: SecretStore>(
    config_manager: &ConfigManager<S>,
    config: &AppConfig,
) -> AppResult<StoredOAuthSession> {
    let session_json = config_manager.get_oauth_session()?.ok_or_else(|| {
        AppError::auth("no OAuth session found. Run `rbxup auth login --client-id <id>`")
    })?;
    let mut session = parse_session(&session_json)?;

    if session.expires_at_unix <= now_unix_seconds() + ACCESS_TOKEN_REFRESH_SKEW_SECONDS {
        session = refresh_oauth_session(&session, config_manager, config).await?;
        config_manager.set_oauth_session(&serialize_session(&session)?)?;
    }

    Ok(session)
}

async fn refresh_oauth_session<S: SecretStore>(
    session: &StoredOAuthSession,
    _config_manager: &ConfigManager<S>,
    _config: &AppConfig,
) -> AppResult<StoredOAuthSession> {
    let refresh_token = session.refresh_token.clone().ok_or_else(|| {
        AppError::auth("stored OAuth session cannot be refreshed. Run `rbxup auth login` again")
    })?;
    let client = reqwest::Client::new();
    let response = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.as_str()),
            ("client_id", session.client_id.as_str()),
        ])
        .send()
        .await
        .map_err(|error| AppError::auth(format!("failed to refresh OAuth token: {error}")))?;
    let status = response.status();

    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<response body unavailable>".to_string());
        return Err(AppError::auth(format!(
            "OAuth token refresh failed. Run `rbxup auth login` again. Roblox returned HTTP {}: {}",
            status.as_u16(),
            body
        )));
    }

    let token_response = response
        .json::<TokenResponse>()
        .await
        .map_err(|error| AppError::auth(format!("invalid OAuth refresh response: {error}")))?;
    let user_info = fetch_user_info(&token_response.access_token)
        .await
        .ok()
        .or_else(|| session.user_info.clone());

    Ok(build_session(&session.client_id, token_response, user_info))
}

async fn exchange_authorization_code(
    client_id: &str,
    redirect_uri: &str,
    code: &str,
    code_verifier: &str,
) -> AppResult<TokenResponse> {
    let client = reqwest::Client::new();
    let response = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("code", code),
            ("code_verifier", code_verifier),
            ("redirect_uri", redirect_uri),
        ])
        .send()
        .await
        .map_err(|error| AppError::auth(format!("failed to exchange OAuth code: {error}")))?;
    let status = response.status();

    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<response body unavailable>".to_string());
        return Err(AppError::auth(format!(
            "OAuth login failed with HTTP {}: {}",
            status.as_u16(),
            body
        )));
    }

    response
        .json::<TokenResponse>()
        .await
        .map_err(|error| AppError::auth(format!("invalid OAuth token response: {error}")))
}

async fn fetch_user_info(access_token: &str) -> AppResult<OAuthUserInfo> {
    let client = reqwest::Client::new();
    let response = client
        .get(USERINFO_URL)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|error| AppError::auth(format!("failed to fetch OAuth user info: {error}")))?;
    let status = response.status();

    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<response body unavailable>".to_string());
        return Err(AppError::auth(format!(
            "OAuth user info failed with HTTP {}: {}",
            status.as_u16(),
            body
        )));
    }

    response
        .json::<OAuthUserInfo>()
        .await
        .map_err(|error| AppError::auth(format!("invalid OAuth user info response: {error}")))
}

async fn revoke_refresh_token(client_id: &str, refresh_token: &str) -> AppResult<()> {
    let client = reqwest::Client::new();
    let response = client
        .post(REVOKE_URL)
        .form(&[("token", refresh_token), ("client_id", client_id)])
        .send()
        .await
        .map_err(|error| AppError::auth(format!("failed to revoke OAuth session: {error}")))?;
    let status = response.status();

    if status.is_success() {
        Ok(())
    } else {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<response body unavailable>".to_string());
        Err(AppError::auth(format!(
            "OAuth revoke failed with HTTP {}: {}",
            status.as_u16(),
            body
        )))
    }
}

fn build_authorization_url(
    client_id: &str,
    redirect_uri: &str,
    scopes: &[String],
    state: &str,
    nonce: &str,
    code_challenge: &str,
) -> AppResult<reqwest::Url> {
    let mut url = reqwest::Url::parse(AUTHORIZE_URL)
        .map_err(|error| AppError::general(format!("failed to construct OAuth URL: {error}")))?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", &scopes.join(" "))
        .append_pair("response_type", "code")
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state)
        .append_pair("nonce", nonce)
        .append_pair("prompt", "consent");
    Ok(url)
}

fn wait_for_oauth_callback(
    redirect_port: u16,
    expected_state: &str,
    redirect_uri: &str,
    timeout: Duration,
) -> AppResult<OAuthCallback> {
    let listener = TcpListener::bind(("127.0.0.1", redirect_port)).map_err(|error| {
        AppError::auth(format!(
            "failed to bind OAuth callback server on {}: {error}",
            redirect_uri
        ))
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|error| AppError::auth(format!("failed to configure callback server: {error}")))?;
    listener.set_ttl(64).map_err(|error| {
        AppError::auth(format!("failed to configure callback server TTL: {error}"))
    })?;
    let started_at = Instant::now();
    let (mut stream, _) = loop {
        match listener.accept() {
            Ok(pair) => break pair,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if started_at.elapsed() >= timeout {
                    return Err(AppError::auth(
                        "timed out waiting for the Roblox OAuth callback",
                    ));
                }

                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => {
                return Err(AppError::auth(format!(
                    "failed to receive OAuth callback: {error}"
                )));
            }
        }
    };
    let mut buffer = [0u8; 8192];
    let bytes_read = stream
        .read(&mut buffer)
        .map_err(|error| AppError::auth(format!("failed to read OAuth callback: {error}")))?;
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or_else(|| AppError::auth("received an invalid OAuth callback request"))?;
    let url = reqwest::Url::parse(&format!("http://127.0.0.1:{redirect_port}{path}"))
        .map_err(|error| AppError::auth(format!("failed to parse OAuth callback URL: {error}")))?;
    let query: HashMap<String, String> = url.query_pairs().into_owned().collect();

    let callback = if let Some(error) = query.get("error") {
        let description = query
            .get("error_description")
            .cloned()
            .unwrap_or_else(|| "authorization failed".to_string());
        write_callback_response(
            &mut stream,
            "OAuth login failed. You can close this tab and return to the terminal.",
        )?;
        return Err(AppError::auth(format!("{error}: {description}")));
    } else {
        let state = query
            .get("state")
            .ok_or_else(|| AppError::auth("OAuth callback did not include a state value"))?;
        if state != expected_state {
            write_callback_response(
                &mut stream,
                "OAuth login failed because the state value did not match.",
            )?;
            return Err(AppError::auth(
                "OAuth callback state did not match the original request",
            ));
        }

        let code = query.get("code").cloned().ok_or_else(|| {
            AppError::auth("OAuth callback did not include an authorization code")
        })?;
        write_callback_response(
            &mut stream,
            "OAuth login completed. You can close this tab and return to the terminal.",
        )?;
        OAuthCallback { code }
    };

    Ok(callback)
}

fn write_callback_response(stream: &mut std::net::TcpStream, message: &str) -> AppResult<()> {
    let body = format!("<html><body><h1>rbxup</h1><p>{message}</p></body></html>");
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).map_err(|error| {
        AppError::auth(format!("failed to write OAuth callback response: {error}"))
    })
}

fn build_session(
    client_id: &str,
    token_response: TokenResponse,
    user_info: Option<OAuthUserInfo>,
) -> StoredOAuthSession {
    StoredOAuthSession {
        access_token: token_response.access_token,
        refresh_token: token_response.refresh_token,
        id_token: token_response.id_token,
        token_type: token_response.token_type,
        scope: token_response
            .scope
            .split_whitespace()
            .map(|value| value.to_string())
            .collect(),
        expires_at_unix: now_unix_seconds() + token_response.expires_in,
        user_info,
        client_id: client_id.to_string(),
    }
}

fn serialize_session(session: &StoredOAuthSession) -> AppResult<String> {
    serde_json::to_string(session)
        .map_err(|error| AppError::config(format!("failed to serialize OAuth session: {error}")))
}

fn parse_session(session_json: &str) -> AppResult<StoredOAuthSession> {
    serde_json::from_str(session_json)
        .map_err(|error| AppError::config(format!("failed to parse stored OAuth session: {error}")))
}

fn build_auth_status(
    auth_mode: &str,
    session: Option<&StoredOAuthSession>,
    user_info: Option<&OAuthUserInfo>,
) -> AuthStatus {
    AuthStatus {
        auth_mode: auth_mode.to_string(),
        client_id: session.map(|value| value.client_id.clone()),
        user_id: user_info.map(|value| value.sub.clone()),
        display_name: user_info.and_then(|value| value.name.clone()),
        username: user_info.and_then(|value| value.preferred_username.clone()),
        expires_at_unix: session.map(|value| value.expires_at_unix),
        scopes: session.map(|value| value.scope.clone()),
    }
}

fn random_urlsafe(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = sha2::Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
