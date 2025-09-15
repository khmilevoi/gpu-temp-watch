use warp::Filter;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use log::info;
use crate::config::Config;
use crate::autostart::AutoStart;

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

pub type SharedState = Arc<RwLock<AppState>>;

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Config,
    pub current_temperature: f32,
    pub monitoring_paused: bool,
    pub autostart_enabled: bool,
    pub uptime_seconds: u64,
    pub recent_logs: Vec<LogEntry>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let autostart_enabled = AutoStart::new()
            .map(|a| a.is_installed())
            .unwrap_or(false);

        Self {
            config,
            current_temperature: 0.0,
            monitoring_paused: false,
            autostart_enabled,
            uptime_seconds: 0,
            recent_logs: Vec::new(),
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
}

#[derive(Debug, Serialize, Deserialize)]
struct ActionRequest {
    action: String,
    value: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct ActionResponse {
    success: bool,
    message: String,
}

pub struct WebServer {
    state: SharedState,
    port: u16,
}

impl WebServer {
    pub fn new(config: Config, port: u16) -> Self {
        let state = Arc::new(RwLock::new(AppState::new(config)));
        Self { state, port }
    }

    pub fn get_state(&self) -> SharedState {
        self.state.clone()
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let state = self.state.clone();

        // CORS headers
        let cors = warp::cors()
            .allow_any_origin()
            .allow_headers(vec!["content-type"])
            .allow_methods(vec!["GET", "POST", "PUT"]);

        // Static files route
        let static_files = warp::path("static")
            .and(warp::fs::dir("web"));

        // Main page
        let index = warp::path::end()
            .map(|| {
                warp::reply::html(include_str!("../web/index.html"))
            });

        // API routes
        let api = warp::path("api");

        // Get current status
        let status = api
            .and(warp::path("status"))
            .and(warp::get())
            .and(with_state(state.clone()))
            .and_then(get_status);

        // Get current config
        let config = api
            .and(warp::path("config"))
            .and(warp::get())
            .and(with_state(state.clone()))
            .and_then(get_config);

        // Update config
        let update_config = api
            .and(warp::path("config"))
            .and(warp::put())
            .and(warp::body::json())
            .and(with_state(state.clone()))
            .and_then(update_config);

        // Get logs
        let logs = api
            .and(warp::path("logs"))
            .and(warp::get())
            .and(with_state(state.clone()))
            .and_then(get_logs);

        // Actions (pause, resume, autostart)
        let actions = api
            .and(warp::path("action"))
            .and(warp::post())
            .and(warp::body::json())
            .and(with_state(state.clone()))
            .and_then(handle_action);

        // Combine all routes
        let routes = index
            .or(static_files)
            .or(status)
            .or(config)
            .or(update_config)
            .or(logs)
            .or(actions)
            .with(cors);

        info!("🌐 Starting web server on http://localhost:{}", self.port);

        warp::serve(routes)
            .run(([127, 0, 0, 1], self.port))
            .await;

        Ok(())
    }
}

fn with_state(state: SharedState) -> impl Filter<Extract = (SharedState,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

async fn get_status(state: SharedState) -> Result<impl warp::Reply, warp::Rejection> {
    let state = state.read().unwrap();
    let response = StatusResponse {
        temperature: state.current_temperature,
        threshold: state.config.temperature_threshold_c,
        monitoring_paused: state.monitoring_paused,
        autostart_enabled: state.autostart_enabled,
        uptime_seconds: state.uptime_seconds,
    };
    Ok(warp::reply::json(&response))
}

async fn get_config(state: SharedState) -> Result<impl warp::Reply, warp::Rejection> {
    let state = state.read().unwrap();
    let web_config = WebConfig::from(state.config.clone());
    Ok(warp::reply::json(&web_config))
}

async fn update_config(new_config: WebConfig, state: SharedState) -> Result<impl warp::Reply, warp::Rejection> {
    let mut state = state.write().unwrap();

    // Convert and validate
    let config: Config = new_config.into();
    match config.validate() {
        Ok(_) => {
            // Save to file
            match config.save() {
                Ok(_) => {
                    state.config = config;
                    state.add_log("INFO", "Configuration updated via web interface");
                    let response = ActionResponse {
                        success: true,
                        message: "Configuration updated successfully".to_string(),
                    };
                    Ok(warp::reply::json(&response))
                }
                Err(e) => {
                    let response = ActionResponse {
                        success: false,
                        message: format!("Failed to save config: {}", e),
                    };
                    Ok(warp::reply::json(&response))
                }
            }
        }
        Err(e) => {
            let response = ActionResponse {
                success: false,
                message: format!("Invalid config: {}", e),
            };
            Ok(warp::reply::json(&response))
        }
    }
}

async fn get_logs(state: SharedState) -> Result<impl warp::Reply, warp::Rejection> {
    let state = state.read().unwrap();
    Ok(warp::reply::json(&state.recent_logs))
}

async fn handle_action(request: ActionRequest, state: SharedState) -> Result<impl warp::Reply, warp::Rejection> {
    match request.action.as_str() {
        "pause" => {
            let mut state = state.write().unwrap();
            state.monitoring_paused = true;
            state.add_log("INFO", "Monitoring paused via web interface");
            let response = ActionResponse {
                success: true,
                message: "Monitoring paused".to_string(),
            };
            Ok(warp::reply::json(&response))
        }
        "resume" => {
            let mut state = state.write().unwrap();
            state.monitoring_paused = false;
            state.add_log("INFO", "Monitoring resumed via web interface");
            let response = ActionResponse {
                success: true,
                message: "Monitoring resumed".to_string(),
            };
            Ok(warp::reply::json(&response))
        }
        "toggle_autostart" => {
            let mut response = ActionResponse {
                success: false,
                message: "Failed to toggle autostart".to_string(),
            };

            match AutoStart::new() {
                Ok(autostart) => {
                    let currently_enabled = autostart.is_installed();

                    let result = if currently_enabled {
                        autostart.uninstall()
                    } else {
                        autostart.install()
                    };

                    match result {
                        Ok(_) => {
                            let mut state = state.write().unwrap();
                            state.autostart_enabled = !currently_enabled;
                            let status = if !currently_enabled { "enabled" } else { "disabled" };
                            state.add_log("INFO", &format!("Autostart {} via web interface", status));

                            response.success = true;
                            response.message = format!("Autostart {}", status);
                        }
                        Err(e) => {
                            response.message = format!("Failed to toggle autostart: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    response.message = format!("Failed to create autostart manager: {:?}", e);
                }
            }

            Ok(warp::reply::json(&response))
        }
        _ => {
            let response = ActionResponse {
                success: false,
                message: format!("Unknown action: {}", request.action),
            };
            Ok(warp::reply::json(&response))
        }
    }
}

pub fn open_browser(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        use std::process::Command;
        Command::new("cmd")
            .args(&["/c", "start", url])
            .spawn()?;
    }

    #[cfg(not(windows))]
    {
        use std::process::Command;
        Command::new("xdg-open")
            .arg(url)
            .spawn()?;
    }

    Ok(())
}