use crate::autostart::AutoStart;
use crate::config::Config;
use axum::{
    extract::{Request, State},
    middleware::{self, Next},
    response::{Html, IntoResponse},
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tower_http::trace::TraceLayer;
use tracing::info;
use crate::{log_info, log_error, log_warn};

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
    pub temperature: Option<f32>,
    pub threshold: f32,
    pub monitoring_paused: bool,
    pub autostart_enabled: bool,
    pub uptime_seconds: u64,
    pub last_update: String,
    pub gpu_connection_status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

pub type SharedState = Arc<RwLock<AppState>>;

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Config,
    pub config_handle: Arc<RwLock<Config>>,
    pub current_temperature: Option<f32>,
    pub monitoring_paused: bool,
    pub autostart_enabled: bool,
    pub uptime_seconds: u64,
    pub recent_logs: Vec<LogEntry>,
    pub last_update: chrono::DateTime<chrono::Local>,
    pub gpu_connection_status: String,
}

impl AppState {
    pub fn new(shared_config: Arc<RwLock<Config>>) -> Self {
        let autostart_enabled = AutoStart::new().map(|a| a.is_installed()).unwrap_or(false);
        let config = shared_config.read().unwrap().clone();

        Self {
            config,
            config_handle: Arc::clone(&shared_config),
            current_temperature: None,
            monitoring_paused: false,
            autostart_enabled,
            uptime_seconds: 0,
            recent_logs: Vec::new(),
            last_update: chrono::Local::now(),
            gpu_connection_status: "Unknown".to_string(),
        }
    }

    pub fn add_log(&mut self, level: &str, message: &str) {
        let entry = LogEntry {
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            level: level.to_string(),
            message: message.to_string(),
        };

        self.recent_logs.push(entry);

        // Keep only last 100 entries
        if self.recent_logs.len() > 100 {
            self.recent_logs.remove(0);
        }
    }

    pub fn update_temperature(&mut self, temperature: Option<f32>, status: &str) {
        self.current_temperature = temperature;
        self.last_update = chrono::Local::now();
        self.gpu_connection_status = status.to_string();
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
    pub fn new(shared_config: Arc<RwLock<Config>>, port: u16) -> Self {
        let app_state = AppState::new(Arc::clone(&shared_config));
        let shared_state = Arc::new(RwLock::new(app_state));

        Self { shared_state, port }
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
    let method = req.method().clone();
    let uri = req.uri().clone();
    let headers = req.headers().clone();


    let start_time = std::time::Instant::now();
    let response = next.run(req).await;
    let duration = start_time.elapsed();


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
        last_update: app_state.last_update.format("%Y-%m-%d %H:%M:%S").to_string(),
        gpu_connection_status: app_state.gpu_connection_status.clone(),
    };
    Json(status)
}

async fn get_logs(State(state): State<SharedState>) -> Json<Vec<LogEntry>> {
    let app_state = state.read().unwrap();
    Json(app_state.recent_logs.clone())
}

async fn get_config(State(state): State<SharedState>) -> Json<WebConfig> {

    let app_state = match state.read() {
        Ok(state) => state,
        Err(e) => {
            log_error!("Failed to acquire read lock on app state");
            // Return default config in case of lock failure
            return Json(WebConfig::from(crate::config::Config::default()));
        }
    };

    let web_config = WebConfig::from(app_state.config.clone());

    Json(web_config)
}

#[tracing::instrument(skip(state))]
async fn update_config(
    State(state): State<SharedState>,
    Json(web_config): Json<WebConfig>,
) -> impl IntoResponse {
    log_info!("Configuration update request received");

    // Get write lock on shared state
    let mut app_state = match state.write() {
        Ok(state) => state,
        Err(e) => {
            log_error!("Failed to acquire write lock on app state");
            return Json(serde_json::json!({
                "success": false,
                "error": "Internal server error: Failed to acquire state lock"
            }));
        }
    };

    // Log the current config for comparison

    // Convert web config to internal config
    let updated_config: crate::config::Config = web_config.into();

    // Validate new configuration
    if let Err(e) = updated_config.validate() {
        log_warn!("Configuration validation failed", serde_json::json!({"error": format!("{}", e)}));

        return Json(serde_json::json!({
            "success": false,
            "error": format!("Configuration validation failed: {}", e)
        }));
    }

    let previous_snapshot = app_state.config.clone();

    {
        let mut config_guard = match app_state.config_handle.write() {
            Ok(guard) => guard,
            Err(e) => {
                log_error!("Failed to update shared configuration state");

                return Json(serde_json::json!({
                    "success": false,
                    "error": format!("Failed to update shared configuration: {}", e)
                }));
            }
        };

        *config_guard = updated_config.clone();
    }

    app_state.config = updated_config.clone();
    log_info!("Configuration updated in memory");

    // Save to file with detailed logging
    if let Err(e) = app_state.config.save() {
        if let Ok(mut config_guard) = app_state.config_handle.write() {
            *config_guard = previous_snapshot.clone();
        }
        app_state.config = previous_snapshot;
        log_error!("Failed to persist configuration to file", serde_json::json!({"error": format!("{}", e)}));

        return Json(serde_json::json!({
            "success": false,
            "error": format!("Failed to save config: {}", e)
        }));
    }

    log_info!("Configuration successfully persisted to file");


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
        "toggle_autostart" => match AutoStart::new() {
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
        },
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


pub fn open_browser(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()?;
    }

    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }

    Ok(())
}
