use crate::autostart::AutoStart;
use crate::config::Config;
use tracing::info;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use axum::{
    extract::{ws::{WebSocket, WebSocketUpgrade}, State, Request},
    response::{Html, Response, IntoResponse},
    routing::{get, post, put},
    middleware::{self, Next},
    Router, Json,
};
use tower_http::trace::TraceLayer;
use tokio::sync::broadcast;
use futures_util::{sink::SinkExt, stream::StreamExt};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    pub temperature_threshold_c: f32,
    pub poll_interval_sec: u64,
    pub base_cooldown_sec: u64,
    pub enable_logging: bool,
    pub log_file_path: Option<String>,
}

impl From<Config> for WebConfig {
    fn from(config: Config) -> Self {
        Self {
            temperature_threshold_c: config.temperature_threshold_c,
            poll_interval_sec: config.poll_interval_sec,
            base_cooldown_sec: config.base_cooldown_sec,
            enable_logging: config.enable_logging,
            log_file_path: config.log_file_path,
        }
    }
}

impl Into<Config> for WebConfig {
    fn into(self) -> Config {
        Config {
            temperature_threshold_c: self.temperature_threshold_c,
            poll_interval_sec: self.poll_interval_sec,
            base_cooldown_sec: self.base_cooldown_sec,
            enable_logging: self.enable_logging,
            log_file_path: self.log_file_path,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusResponse {
    pub temperature: f32,
    pub threshold: f32,
    pub monitoring_paused: bool,
    pub autostart_enabled: bool,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebSocketMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub data: serde_json::Value,
}

pub type SharedState = Arc<RwLock<AppState>>;

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Config,
    pub current_temperature: f32,
    pub monitoring_paused: bool,
    pub autostart_enabled: bool,
    pub uptime_seconds: u64,
    pub recent_logs: Vec<LogEntry>,
    pub broadcast_tx: broadcast::Sender<WebSocketMessage>,
}

impl AppState {
    pub fn new(config: Config) -> (Self, broadcast::Receiver<WebSocketMessage>) {
        let autostart_enabled = AutoStart::new().map(|a| a.is_installed()).unwrap_or(false);
        let (broadcast_tx, broadcast_rx) = broadcast::channel(100);

        let state = Self {
            config,
            current_temperature: 0.0,
            monitoring_paused: false,
            autostart_enabled,
            uptime_seconds: 0,
            recent_logs: Vec::new(),
            broadcast_tx,
        };

        (state, broadcast_rx)
    }

    pub fn add_log(&mut self, level: &str, message: &str) {
        let entry = LogEntry {
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            level: level.to_string(),
            message: message.to_string(),
        };

        self.recent_logs.push(entry.clone());

        // Keep only last 100 entries
        if self.recent_logs.len() > 100 {
            self.recent_logs.remove(0);
        }

        // Broadcast log update to WebSocket clients
        let ws_message = WebSocketMessage {
            msg_type: "log".to_string(),
            data: serde_json::to_value(&entry).unwrap_or_default(),
        };
        let _ = self.broadcast_tx.send(ws_message);
    }

    pub fn broadcast_temperature_update(&self) {
        let status = StatusResponse {
            temperature: self.current_temperature,
            threshold: self.config.temperature_threshold_c,
            monitoring_paused: self.monitoring_paused,
            autostart_enabled: self.autostart_enabled,
            uptime_seconds: self.uptime_seconds,
        };

        let ws_message = WebSocketMessage {
            msg_type: "temperature".to_string(),
            data: serde_json::to_value(&status).unwrap_or_default(),
        };
        let _ = self.broadcast_tx.send(ws_message);
    }

    pub fn broadcast_config_update(&self) {
        let web_config = WebConfig::from(self.config.clone());
        let ws_message = WebSocketMessage {
            msg_type: "config".to_string(),
            data: serde_json::to_value(&web_config).unwrap_or_default(),
        };
        let _ = self.broadcast_tx.send(ws_message);
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ActionRequest {
    action: String,
}

pub struct WebServer {
    shared_state: SharedState,
    port: u16,
}

impl WebServer {
    pub fn new(config: Config, port: u16) -> Self {
        let (app_state, _) = AppState::new(config);
        let shared_state = Arc::new(RwLock::new(app_state));

        Self {
            shared_state,
            port,
        }
    }

    pub fn get_state(&self) -> SharedState {
        self.shared_state.clone()
    }

    #[tracing::instrument(skip(self))]
    pub async fn start(self) -> Result<(), Box<dyn std::error::Error>> {
        let app = Router::new()
            .route("/", get(get_index))
            .route("/api/status", get(get_status))
            .route("/api/logs", get(get_logs))
            .route("/api/config", get(get_config))
            .route("/api/config", post(update_config))
            .route("/api/config", put(update_config)) // Добавляем PUT для REST стандарта
            .route("/api/action", post(handle_action))
            .route("/health", get(health_check))
            .route("/ws", get(websocket_handler))
            .layer(middleware::from_fn(log_requests))
            .layer(TraceLayer::new_for_http())
            .with_state(self.shared_state);

        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", self.port)).await?;
        info!("Web server starting on http://127.0.0.1:{}", self.port);

        axum::serve(listener, app).await?;
        Ok(())
    }
}

// HTTP Request Logging Middleware
async fn log_requests(req: Request, next: Next) -> impl IntoResponse {
    use crate::log_both;

    let method = req.method().clone();
    let uri = req.uri().clone();
    let headers = req.headers().clone();

    // Log incoming request
    log_both!(info, "🌐 HTTP Request", Some(serde_json::json!({
        "method": method.to_string(),
        "uri": uri.to_string(),
        "path": uri.path(),
        "query": uri.query().unwrap_or(""),
        "user_agent": headers.get("user-agent").and_then(|v| v.to_str().ok()).unwrap_or(""),
        "content_type": headers.get("content-type").and_then(|v| v.to_str().ok()).unwrap_or(""),
        "content_length": headers.get("content-length").and_then(|v| v.to_str().ok()).unwrap_or("0")
    })));

    let start_time = std::time::Instant::now();
    let response = next.run(req).await;
    let duration = start_time.elapsed();

    // Log response
    log_both!(info, "📤 HTTP Response", Some(serde_json::json!({
        "method": method.to_string(),
        "uri": uri.to_string(),
        "status": response.status().as_u16(),
        "duration_ms": duration.as_millis()
    })));

    response
}

// Handlers
async fn get_index() -> Html<&'static str> {
    Html(include_str!("../web/index.html"))
}

async fn get_status(State(state): State<SharedState>) -> Json<StatusResponse> {
    let app_state = state.read().unwrap();
    let status = StatusResponse {
        temperature: app_state.current_temperature,
        threshold: app_state.config.temperature_threshold_c,
        monitoring_paused: app_state.monitoring_paused,
        autostart_enabled: app_state.autostart_enabled,
        uptime_seconds: app_state.uptime_seconds,
    };
    Json(status)
}

async fn get_logs(State(state): State<SharedState>) -> Json<Vec<LogEntry>> {
    let app_state = state.read().unwrap();
    Json(app_state.recent_logs.clone())
}

async fn get_config(State(state): State<SharedState>) -> Json<WebConfig> {
    use crate::log_both;

    log_both!(debug, "🌐 Configuration requested via web API", None);

    let app_state = match state.read() {
        Ok(state) => state,
        Err(e) => {
            log_both!(error, "❌ Failed to acquire read lock on app state for config request", Some(serde_json::json!({
                "error": e.to_string()
            })));
            // Return default config in case of lock failure
            return Json(WebConfig::from(crate::config::Config::default()));
        }
    };

    let web_config = WebConfig::from(app_state.config.clone());
    log_both!(debug, "📋 Returning current configuration", Some(serde_json::json!({
        "config": {
            "temperature_threshold_c": web_config.temperature_threshold_c,
            "poll_interval_sec": web_config.poll_interval_sec,
            "base_cooldown_sec": web_config.base_cooldown_sec,
            "enable_logging": web_config.enable_logging,
            "log_file_path": web_config.log_file_path
        }
    })));

    Json(web_config)
}

#[tracing::instrument(skip(state))]
async fn update_config(
    State(state): State<SharedState>,
    Json(web_config): Json<WebConfig>,
) -> impl IntoResponse {
    use crate::log_both;

    log_both!(info, "🌐 Received configuration update request via web API", Some(serde_json::json!({
        "new_config": {
            "temperature_threshold_c": web_config.temperature_threshold_c,
            "poll_interval_sec": web_config.poll_interval_sec,
            "base_cooldown_sec": web_config.base_cooldown_sec,
            "enable_logging": web_config.enable_logging,
            "log_file_path": web_config.log_file_path
        }
    })));

    // Get write lock on shared state
    let mut app_state = match state.write() {
        Ok(state) => state,
        Err(e) => {
            log_both!(error, "❌ Failed to acquire write lock on app state", Some(serde_json::json!({
                "error": e.to_string()
            })));
            return Json(serde_json::json!({
                "success": false,
                "error": "Internal server error: Failed to acquire state lock"
            }));
        }
    };

    // Log the current config for comparison
    log_both!(debug, "📋 Current configuration before update", Some(serde_json::json!({
        "current_config": {
            "temperature_threshold_c": app_state.config.temperature_threshold_c,
            "poll_interval_sec": app_state.config.poll_interval_sec,
            "base_cooldown_sec": app_state.config.base_cooldown_sec,
            "enable_logging": app_state.config.enable_logging,
            "log_file_path": app_state.config.log_file_path
        }
    })));

    // Convert web config to internal config
    let new_config: crate::config::Config = web_config.into();

    // Validate new configuration
    if let Err(e) = new_config.validate() {
        log_both!(warn, "⚠️ Configuration validation failed", Some(serde_json::json!({
            "validation_error": e.to_string(),
            "rejected_config": {
                "temperature_threshold_c": new_config.temperature_threshold_c,
                "poll_interval_sec": new_config.poll_interval_sec,
                "base_cooldown_sec": new_config.base_cooldown_sec
            }
        })));

        return Json(serde_json::json!({
            "success": false,
            "error": format!("Configuration validation failed: {}", e)
        }));
    }

    // Update the config in memory
    app_state.config = new_config;

    log_both!(info, "✅ Configuration updated in memory", None);

    // Save to file with detailed logging
    if let Err(e) = app_state.config.save() {
        log_both!(error, "❌ Failed to persist configuration to file", Some(serde_json::json!({
            "error": e.to_string(),
            "attempted_config": {
                "temperature_threshold_c": app_state.config.temperature_threshold_c,
                "poll_interval_sec": app_state.config.poll_interval_sec,
                "base_cooldown_sec": app_state.config.base_cooldown_sec,
                "enable_logging": app_state.config.enable_logging,
                "log_file_path": app_state.config.log_file_path
            }
        })));

        return Json(serde_json::json!({
            "success": false,
            "error": format!("Failed to save config: {}", e)
        }));
    }

    log_both!(info, "💾 Configuration successfully persisted to file", None);

    // Broadcast config update to WebSocket clients
    app_state.broadcast_config_update();
    log_both!(info, "📡 Configuration update broadcasted to WebSocket clients", None);

    Json(serde_json::json!({
        "success": true,
        "message": "Configuration updated successfully"
    }))
}

async fn handle_action(
    State(_state): State<SharedState>,
    Json(action): Json<ActionRequest>,
) -> Json<serde_json::Value> {
    match action.action.as_str() {
        "toggle_autostart" => {
            match AutoStart::new() {
                Ok(autostart) => {
                    if autostart.is_installed() {
                        match autostart.uninstall() {
                            Ok(_) => Json(serde_json::json!({
                                "success": true,
                                "message": "Autostart disabled"
                            })),
                            Err(e) => Json(serde_json::json!({
                                "success": false,
                                "error": format!("Failed to disable autostart: {}", e)
                            })),
                        }
                    } else {
                        match autostart.install() {
                            Ok(_) => Json(serde_json::json!({
                                "success": true,
                                "message": "Autostart enabled"
                            })),
                            Err(e) => Json(serde_json::json!({
                                "success": false,
                                "error": format!("Failed to enable autostart: {}", e)
                            })),
                        }
                    }
                }
                Err(e) => Json(serde_json::json!({
                    "success": false,
                    "error": format!("Failed to access autostart: {}", e)
                })),
            }
        }
        _ => Json(serde_json::json!({
            "success": false,
            "error": "Unknown action"
        })),
    }
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "GPU Temperature Monitor",
        "timestamp": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
    }))
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> Response {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

#[tracing::instrument(skip_all)]
async fn handle_websocket(socket: WebSocket, shared_state: SharedState) {
    let mut rx = {
        let state = shared_state.read().unwrap();
        state.broadcast_tx.subscribe()
    };

    let (mut sender, mut receiver) = socket.split();

    // Send initial state
    let initial_message = {
        let state = shared_state.read().unwrap();
        let status = StatusResponse {
            temperature: state.current_temperature,
            threshold: state.config.temperature_threshold_c,
            monitoring_paused: state.monitoring_paused,
            autostart_enabled: state.autostart_enabled,
            uptime_seconds: state.uptime_seconds,
        };

        WebSocketMessage {
            msg_type: "initial".to_string(),
            data: serde_json::to_value(&status).unwrap_or_default(),
        }
    };

    if let Ok(msg_text) = serde_json::to_string(&initial_message) {
        let _ = sender.send(axum::extract::ws::Message::Text(msg_text)).await;
    }

    // Handle incoming and outgoing messages
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(msg_text) = serde_json::to_string(&msg) {
                if sender.send(axum::extract::ws::Message::Text(msg_text)).await.is_err() {
                    break;
                }
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            if let Ok(msg) = msg {
                match msg {
                    axum::extract::ws::Message::Text(_) => {
                        // Handle incoming text messages if needed
                    }
                    axum::extract::ws::Message::Close(_) => {
                        break;
                    }
                    _ => {}
                }
            } else {
                break;
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

pub fn open_browser(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()?;
    }

    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()?;
    }

    Ok(())
}