#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_paths;
mod autostart;
mod config;
mod gui;
#[macro_use]
mod logger_service;
mod monitor;
mod notifications;
mod tray;
mod web_server;

use app_paths::AppPaths;
use autostart::{AutoStart, AutoStartStatus};
use config::Config;
use gui::GuiManager;
use logger_service::{init_logger, LoggerConfig, LogLevel, LogOutput, LogFormat};

use monitor::TempMonitor;
use notifications::NotificationManager;
use tray::{SystemTray, TrayMessage};
use web_server::{open_browser, WebServer};

use std::env;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};

fn debug_print(msg: &str) {
    #[cfg(debug_assertions)]
    log_info!(msg);

    #[cfg(not(debug_assertions))]
    log_info!(msg);
}

// Fast tray event handler - runs every 100ms
async fn handle_tray_events(
    mut system_tray: SystemTray,
    _notification_manager: Arc<Mutex<NotificationManager>>,
) {
    log_info!("🚀 Starting fast tray event handler (100ms interval)");
    
    loop {
        if let Some(message) = system_tray.get_message() {
            log_info!(&format!("📬 Received tray message: {:?}", message));
            
            match message {
                TrayMessage::QuitMonitor => {
                    log_info!("🚪 Quitting monitor via system tray");
                    std::process::exit(0);
                }
                TrayMessage::OpenDashboard => {
                    log_info!("🌐 Opening dashboard...");
                    info!("Tray request: open dashboard");
                    
                    // Launch browser in separate task to avoid blocking
                    tokio::spawn(async {
                        if let Err(e) = open_browser("http://localhost:18235") {
                            warn!("Failed to open dashboard: {}", e);
                        } else {
                            info!("Dashboard launched in default browser");
                        }
                    });
                }
                TrayMessage::ViewLogs => {
                    log_info!("📋 View logs clicked");
                    let log_path = AppPaths::get_log_file_path()
                        .unwrap_or_else(|_| AppPaths::get_fallback_log_path());

                    // Ensure log file exists before trying to open
                    if !log_path.exists() {
                        if let Some(parent) = log_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let _ = std::fs::write(&log_path, "Log file created\n");
                    }

                    tokio::spawn(async move {
                        if let Err(e) = crate::gui::GuiDialogs::open_file(&log_path.to_string_lossy()) {
                            log_error!(&format!("❌ Failed to open log file: {}", e));
                        }
                    });
                }
                TrayMessage::EditSettings => {
                    log_info!("⚙️ Edit settings clicked");
                    let config_path = AppPaths::get_config_path()
                        .unwrap_or_else(|_| AppPaths::get_fallback_config_path());

                    tokio::spawn(async move {
                        if let Err(e) = crate::gui::GuiDialogs::open_file(&config_path.to_string_lossy()) {
                            log_error!(&format!("❌ Failed to open config file: {}", e));
                        }
                    });
                }
            }
        }
        
        // Fast polling - check tray every 100ms
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

// Temperature monitoring handler - runs at configured poll interval
async fn handle_temperature_monitoring(
    temp_monitor: TempMonitor,
    shared_config: Arc<RwLock<Config>>,
    notification_manager: Arc<Mutex<NotificationManager>>,
    mut gui_manager: GuiManager,
    web_state: web_server::SharedState,
    tray_sender: Option<mpsc::Sender<TrayIconUpdate>>,
) {
    log_info!("🚀 Starting temperature monitoring handler");
    let monitoring_paused = false;
    
    loop {
        if !monitoring_paused {
            let poll_interval = match monitor_temperatures_cycle(
                &temp_monitor,
                &notification_manager,
                &shared_config,
                &mut gui_manager,
                &web_state,
                &tray_sender,
            ).await {
                Ok(interval) => interval,
                Err(e) => {
                    log_error!(&format!("❌ Monitoring error: {}", e));
                    
                    // Show GUI error dialog for critical errors
                    #[cfg(debug_assertions)]
                    crate::gui::GuiDialogs::show_warning(
                        "Monitoring Warning",
                        &format!(
                            "⚠️ Monitoring error occurred:\n\n{}\n\nMonitoring will continue...",
                            e
                        ),
                    );
                    
                    // Default interval on error
                    shared_config.read().unwrap().poll_interval_sec
                }
            };
            
            tokio::time::sleep(Duration::from_secs(poll_interval)).await;
        } else {
            // If paused, check every second
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}

// Tray icon update message
#[derive(Debug)]
struct TrayIconUpdate {
    temperature: f32,
    threshold: f32,
}

// Refactored temperature monitoring cycle - returns poll interval
async fn monitor_temperatures_cycle(
    temp_monitor: &TempMonitor,
    notification_manager: &Arc<Mutex<NotificationManager>>,
    shared_config: &Arc<RwLock<Config>>,
    gui_manager: &mut GuiManager,
    web_state: &web_server::SharedState,
    tray_sender: &Option<mpsc::Sender<TrayIconUpdate>>,
) -> Result<u64, Box<dyn std::error::Error>> {
    let gpu_temps = temp_monitor.get_gpu_temperatures().await?;

    if gpu_temps.is_empty() {
        log_warn!("⚠️  No GPU temperature sensors found");
        return Ok(shared_config.read().unwrap().poll_interval_sec);
    }

    let mut any_over_threshold = false;
    let mut max_temp = 0.0f32;
    let mut hottest_sensor = String::new();

    let threshold = shared_config.read().unwrap().temperature_threshold_c;

    for reading in &gpu_temps {
        let temp = reading.temperature;
        let exceeds_threshold = temp > threshold;

        if exceeds_threshold {
            any_over_threshold = true;
            if temp > max_temp {
                max_temp = temp;
                hottest_sensor = reading.sensor_name.clone();
            }
        }

        // Log temperature reading
        let status_icon = if exceeds_threshold { "🔥" } else { "🟢" };
        log_temperature!(&reading.sensor_name, temp, threshold);

        // Temperature already logged by log_temperature! macro
    }

    // Update GUI manager with current temperature
    gui_manager.update_temperature(max_temp);

    // Update web state with current temperature and add log entries
    {
        let mut state = web_state.write().unwrap();
        let (temperature, gpu_status) = if gpu_temps.is_empty() {
            (None, "No GPU detected")
        } else {
            // Find the actual maximum temperature from readings
            let actual_max = gpu_temps.iter()
                .map(|reading| reading.temperature)
                .fold(0.0f32, f32::max);
            
            if actual_max > 0.0 {
                (Some(actual_max), "Connected")
            } else {
                (None, "Error reading temperature")
            }
        };
        
        state.update_temperature(temperature, gpu_status);
        state.config = shared_config.read().unwrap().clone();

        // Add temperature reading to web logs
        for reading in &gpu_temps {
            let level = if reading.temperature > threshold {
                "WARN"
            } else {
                "INFO"
            };
            let message = format!("{}: {:.1}°C", reading.sensor_name, reading.temperature);
            state.add_log(level, &message);
        }
    }

    // Send tray icon update via channel (non-blocking)
    if let Some(sender) = tray_sender {
        // Use the actual maximum temperature from readings
        let actual_max = if gpu_temps.is_empty() {
            0.0
        } else {
            gpu_temps.iter()
                .map(|reading| reading.temperature)
                .fold(0.0f32, f32::max)
        };
        
        let update = TrayIconUpdate {
            temperature: actual_max,
            threshold,
        };
        let _ = sender.try_send(update); // Non-blocking send
    }

    // Check if we should send notification
    if let Ok(mut nm) = notification_manager.try_lock() {
        if nm.should_notify(any_over_threshold) {
            let cooldown_level = nm.cooldown_level;

            // Alert logged by notification system

            // Send notification directly (already in async context)
            let _ = nm.send_temperature_alert(&hottest_sensor, max_temp, threshold);
        }
    }

    // Return poll interval for next cycle
    Ok(shared_config.read().unwrap().poll_interval_sec)
}

#[tracing::instrument]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // Initialize new logger service
    let logger_config = LoggerConfig {
        min_level: LogLevel::Info,
        output: LogOutput::Both,
        console_format: LogFormat::Human,
        file_format: LogFormat::Json,
        file_path: Some(
            AppPaths::get_log_file_path()
                .unwrap_or_else(|_| AppPaths::get_fallback_log_path())
        ),
        max_file_size: Some(10 * 1024 * 1024), // 10MB
        max_files: Some(5),
        colored_output: true,
        enabled: true,
    };

    if let Err(e) = init_logger(logger_config) {
        eprintln!("Failed to initialize logger: {}", e);
        return Err(e);
    }


    // Log startup information with environment details
    let startup_time = chrono::Local::now();
    let args: Vec<String> = env::args().collect();
    let is_autostart = std::env::var("SESSIONNAME").is_ok() && args.len() == 1;

    log_startup!("0.1.0", &args);

    // Additional startup diagnostics for autostart detection
    if is_autostart {
        log_info!("🔧 Detected autostart environment",
                 serde_json::json!({
                     "detection_method": "session_name_env_var",
                     "session_name": std::env::var("SESSIONNAME").unwrap_or_default(),
                     "args_count": args.len()
                 }));
    }

    // Handle command line arguments
    if args.len() > 1 {
        match args[1].as_str() {
            "--install" => {
                debug_print("🚀 GPU Temperature Monitor v0.1.0");
                debug_print("� Installing autostart...");
                match AutoStart::new() {
                    Ok(autostart) => match autostart.install() {
                        Ok(_) => autostart.print_status(),
                        Err(e) => debug_print(&format!("❌ Failed to install autostart: {:?}", e)),
                    },
                    Err(e) => debug_print(&format!("❌ Failed to create autostart: {:?}", e)),
                }
                return Ok(());
            }
            "--uninstall" => {
                log_info!("🚀 GPU Temperature Monitor v0.1.0");
                log_info!("📤 Removing autostart...");
                match AutoStart::new() {
                    Ok(autostart) => match autostart.uninstall() {
                        Ok(_) => autostart.print_status(),
                        Err(e) => log_error!(&format!("❌ Failed to remove autostart: {:?}", e)),
                    },
                    Err(e) => log_error!(&format!("❌ Failed to create autostart: {:?}", e)),
                }
                return Ok(());
            }
            "--status" => {
                log_info!("🚀 GPU Temperature Monitor v0.1.0");
                log_info!("📋 Autostart status:");
                match AutoStart::new() {
                    Ok(autostart) => {
                        autostart.print_status();
                        let status = autostart.get_detailed_status();
                        log_info!("\n🔧 Detailed diagnostics:");
                        log_info!(&format!("   Current executable: {}", status.current_exe.display()));
                        log_info!(&format!("   Registry path: {}", status.registry_path.display()));
                        log_info!(&format!("   Paths match: {}", if status.paths_match { "✅" } else { "❌" }));
                        log_info!(&format!("   File exists: {}", if status.file_exists { "✅" } else { "❌" }));

                        // Log detailed status for file logging
                        log_info!("Autostart status check performed",
                                 serde_json::json!({
                                     "is_installed": status.is_installed,
                                     "registry_path": status.registry_path.display().to_string(),
                                     "current_exe": status.current_exe.display().to_string(),
                                     "paths_match": status.paths_match,
                                     "file_exists": status.file_exists,
                                     "app_name": status.app_name
                                 }));
                    },
                    Err(e) => {
                        log_error!(&format!("❌ Failed to check autostart: {:?}", e));
                        log_error!("Failed to check autostart status",
                                  serde_json::json!({
                                      "error": format!("{:?}", e)
                                  }));
                    },
                }
                return Ok(());
            }
            "--startup-test" => {
                log_info!("🚀 GPU Temperature Monitor v0.1.0");
                log_info!("🧪 Running startup diagnostics...");

                // Test NVML availability early
                let temp_monitor = TempMonitor::new();
                log_info!("🔌 Testing NVML connection...");
                match temp_monitor.test_connection().await {
                    Ok(_) => {
                        log_info!("✅ NVML connection successful");
                        log_info!("Startup test: NVML connection successful",
                                 serde_json::json!({"component": "nvml", "status": "ok"}));
                    },
                    Err(e) => {
                        log_error!(&format!("❌ NVML connection failed: {}", e));
                        log_error!("Startup test: NVML connection failed",
                                  serde_json::json!({
                                      "component": "nvml",
                                      "status": "error",
                                      "error": format!("{}", e)
                                  }));
                    }
                }

                // Test autostart configuration
                match AutoStart::new() {
                    Ok(autostart) => {
                        let status = autostart.get_detailed_status();
                        log_info!("🔧 Autostart diagnostics:");
                        log_info!(&format!("   Installed: {}", if status.is_installed { "✅" } else { "❌" }));
                        log_info!(&format!("   Paths match: {}", if status.paths_match { "✅" } else { "❌" }));
                        log_info!(&format!("   File exists: {}", if status.file_exists { "✅" } else { "❌" }));

                        if !status.paths_match && status.is_installed {
                            log_warn!("⚠️  Path mismatch detected:");
                            log_warn!(&format!("     Registry: {}", status.registry_path.display()));
                            log_warn!(&format!("     Current:  {}", status.current_exe.display()));
                        }

                        log_info!("Startup test: Autostart diagnostics completed",
                                 serde_json::to_value(&status).unwrap_or_default());
                    },
                    Err(e) => {
                        log_error!(&format!("❌ Autostart diagnostics failed: {:?}", e));
                        log_error!("Startup test: Autostart diagnostics failed",
                                  serde_json::json!({"error": format!("{:?}", e)}));
                    }
                }

                log_info!("✅ Startup diagnostics completed. Check logs for details.");
                return Ok(());
            }
            "--help" => {
                print_help();
                return Ok(());
            }
            _ => {
                log_error!(&format!("❌ Unknown argument: {}", args[1]));
                print_help();
                return Ok(());
            }
        }
    }

    log_info!("🚀 GPU Temperature Monitor v0.1.0");
    log_info!("🔧 Initializing...");

    // Check and install autostart on first run if not already installed
    match AutoStart::new() {
        Ok(autostart) => {
            let status = autostart.get_detailed_status();
            log_info!("Autostart status check on startup",
                     serde_json::json!({
                         "is_installed": status.is_installed,
                         "paths_match": status.paths_match,
                         "file_exists": status.file_exists,
                         "current_exe": status.current_exe.display().to_string(),
                         "registry_path": status.registry_path.display().to_string()
                     }));

            if !status.is_installed {
                log_info!("📦 First run detected, installing autostart...");
                log_info!("First run detected, installing autostart",
                         serde_json::json!({
                             "current_exe": status.current_exe.display().to_string()
                         }));

                match autostart.install() {
                    Ok(_) => {
                        log_info!("✅ Autostart installed successfully");
                        log_info!("Autostart installed successfully on first run",
                                 serde_json::json!({"status": "success"}));
                    },
                    Err(e) => {
                        log_error!(&format!("⚠️ Failed to install autostart: {:?}", e));
                        log_error!("Failed to install autostart on first run",
                                  serde_json::json!({"error": format!("{:?}", e)}));
                    }
                }
            } else if !status.paths_match {
                log_warn!("⚠️ Autostart path mismatch detected");
                log_warn!(&format!("   Registry: {}", status.registry_path.display()));
                log_warn!(&format!("   Current:  {}", status.current_exe.display()));
                log_warn!("   Updating autostart entry...");

                log_warn!("Autostart path mismatch detected, updating",
                         serde_json::json!({
                             "registry_path": status.registry_path.display().to_string(),
                             "current_exe": status.current_exe.display().to_string()
                         }));

                match autostart.install() {
                    Ok(_) => {
                        log_info!("✅ Autostart entry updated successfully");
                        log_info!("Autostart entry updated successfully",
                                 serde_json::json!({"status": "updated"}));
                    },
                    Err(e) => {
                        log_error!(&format!("⚠️ Failed to update autostart: {:?}", e));
                        log_error!("Failed to update autostart entry",
                                  serde_json::json!({"error": format!("{:?}", e)}));
                    }
                }
            } else {
                log_debug!("Autostart is properly configured",
                          serde_json::json!({"status": "ok"}));
            }
        }
        Err(e) => {
            log_error!(&format!("⚠️ Failed to create autostart manager: {:?}", e));
            log_error!("Failed to create autostart manager",
                      serde_json::json!({"error": format!("{:?}", e)}));
        }
    }

    // Load configuration and wrap in Arc<RwLock> for shared access
    let config = Config::load_or_create()?;
    config.validate()?;
    let shared_config = Arc::new(RwLock::new(config));

    // Initialize components
    let temp_monitor = TempMonitor::new();
    let mut notification_manager = NotificationManager::new();
    let mut system_tray = match SystemTray::new() {
        Ok(tray) => {
            log_info!("✅ System tray initialized successfully");
            Some(tray)
        }
        Err(e) => {
            log_error!(&format!("❌ Failed to create system tray: {}", e));
            let _ = notification_manager.send_status_notification_sync(&format!(
                "❌ System Tray Error: {}. Continuing without tray integration.",
                e
            ));
            None
        }
    };

    // Test NVML connection with enhanced error handling
    log_info!("🔌 Testing NVML connection...");
    log_info!("Testing NVML connection on startup",
             serde_json::json!({"component": "nvml"}));

    match temp_monitor.test_connection().await {
        Ok(_) => {
            log_info!("✅ Connected to NVML");
            log_info!("NVML connection successful",
                     serde_json::json!({"component": "nvml", "status": "connected"}));
        },
        Err(e) => {
            let error_msg = format!("❌ Failed to connect to NVML: {}\n\n💡 Make sure NVIDIA drivers are installed and GPU is available", e);
            log_error!(&error_msg);

            log_error!("NVML connection failed on startup",
                      serde_json::json!({
                          "component": "nvml",
                          "status": "error",
                          "error": format!("{}", e),
                          "suggestion": "Make sure NVIDIA drivers are installed and GPU is available"
                      }));

            // If this was an autostart, create a detailed startup failure log
            if is_autostart {
                log_error!("Autostart failed due to NVML connection error",
                          serde_json::json!({
                              "startup_source": "autostart",
                              "failure_reason": "nvml_connection",
                              "error": format!("{}", e),
                              "session": std::env::var("SESSIONNAME").unwrap_or_default()
                          }));

                // For autostart failures, delay and retry once
                log_info!("⏳ Autostart detected, waiting 10 seconds and retrying NVML connection...");
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;

                match temp_monitor.test_connection().await {
                    Ok(_) => {
                        log_info!("✅ NVML connection successful on retry");
                        log_info!("NVML connection successful on autostart retry",
                                 serde_json::json!({"component": "nvml", "status": "connected_retry"}));
                    },
                    Err(retry_error) => {
                        log_error!("NVML connection failed on autostart retry, exiting",
                                  serde_json::json!({
                                      "error": format!("{}", retry_error),
                                      "startup_source": "autostart"
                                  }));

                        // Send error notification and exit
                        let _ = notification_manager
                            .send_status_notification_sync(&format!("❌ NVML Connection Error (Autostart): {}", retry_error));
                        return Err(retry_error);
                    }
                }
            } else {
                // Send error notification and exit for manual starts
                let _ = notification_manager
                    .send_status_notification_sync(&format!("❌ NVML Connection Error: {}", e));
                return Err(e);
            }
        }
    }

    // Send startup notification
    notification_manager
        .send_status_notification("GPU Temperature Monitor started")?;

    // Send startup toast notification
    #[cfg(debug_assertions)]
    {
        let _ = notification_manager.send_status_notification_sync(
            "🚀 GPU Temperature Monitor started successfully! Right-click tray icon for options.",
        );
    }

    {
        let config = shared_config.read().unwrap();
        log_info!(&format!(
            "🌡️  Temperature threshold: {:.1}°C",
            config.temperature_threshold_c
        ));
        log_info!(&format!("⏱️  Poll interval: {}s", config.poll_interval_sec));
    }
    log_info!("🔄 Starting monitoring loop...");

    let monitoring_paused = false;
    let mut gui_manager = GuiManager::new();

    // Initialize and start web server
    let web_server = WebServer::new(shared_config.clone(), 18235);
    let web_state = web_server.get_state();

    // Start web server in background
    let _web_server_handle = {
        let web_server = web_server;
        tokio::spawn(async move {
            if let Err(e) = web_server.start().await {
                log_error!(&format!("❌ Web server error: {}", e));
            }
        })
    };

    log_info!("🌐 Web interface available at http://localhost:18235");

    // Create channel for tray icon updates
    let (tray_tx, mut tray_rx) = mpsc::channel::<TrayIconUpdate>(100);

    // Wrap notification manager in Arc<Mutex> for sharing between tasks
    let shared_notification_manager = Arc::new(Mutex::new(notification_manager));

    // Start fast tray event handler (100ms polling)
    let tray_handle = if let Some(tray) = system_tray {
        Some(tokio::spawn(handle_tray_events(
            tray,
            shared_notification_manager.clone(),
        )))
    } else {
        None
    };

    // Start temperature monitoring handler (poll_interval polling)
    let monitor_handle = tokio::spawn(handle_temperature_monitoring(
        temp_monitor,
        shared_config.clone(),
        shared_notification_manager.clone(),
        gui_manager,
        web_state.clone(),
        Some(tray_tx),
    ));

    // Handle tray icon updates in a separate task
    let tray_icon_handle = tokio::spawn(async move {
        // This would need access to the tray's command sender
        // For now, we'll just consume the messages
        while let Some(update) = tray_rx.recv().await {
            log_debug!(&format!("🎨 Tray icon update: {:.1}°C (threshold: {:.1}°C)",
                     update.temperature, update.threshold));
            // In a full implementation, we'd send this to the tray thread
        }
    });

    log_info!(&format!("🚀 All handlers started - fast tray polling (100ms), temperature monitoring ({}s)",
             shared_config.read().unwrap().poll_interval_sec));

    // Wait for any task to complete (which means an error or shutdown)
    tokio::select! {
        result = monitor_handle => {
            match result {
                Ok(()) => log_info!("✅ Temperature monitoring completed successfully"),
                Err(e) => log_error!(&format!("❌ Temperature monitoring task panicked: {}", e)),
            }
        }
        result = async {
            if let Some(handle) = tray_handle {
                handle.await
            } else {
                // If no tray, wait indefinitely
                std::future::pending::<Result<(), tokio::task::JoinError>>().await
            }
        } => {
            match result {
                Ok(()) => log_info!("✅ Tray handler completed successfully"),
                Err(e) => log_error!(&format!("❌ Tray handler task panicked: {}", e)),
            }
        }
        _ = tray_icon_handle => {
            log_info!("✅ Tray icon handler completed");
        }
    }

    Ok(())
}

#[tracing::instrument(skip_all)]
async fn monitor_temperatures(
    temp_monitor: &TempMonitor,
    notification_manager: &mut NotificationManager,
    shared_config: &Arc<RwLock<Config>>,
    system_tray: &mut Option<SystemTray>,
    gui_manager: &mut GuiManager,
    web_state: &web_server::SharedState,
) -> Result<(), Box<dyn std::error::Error>> {
    let gpu_temps = temp_monitor.get_gpu_temperatures().await?;

    if gpu_temps.is_empty() {
        log_warn!("⚠️  No GPU temperature sensors found");
        return Ok(());
    }

    let mut any_over_threshold = false;
    let mut max_temp = 0.0f32;
    let mut hottest_sensor = String::new();

    let threshold = shared_config.read().unwrap().temperature_threshold_c;

    for reading in &gpu_temps {
        let temp = reading.temperature;
        let exceeds_threshold = temp > threshold;

        if exceeds_threshold {
            any_over_threshold = true;
            if temp > max_temp {
                max_temp = temp;
                hottest_sensor = reading.sensor_name.clone();
            }
        }

        // Log temperature reading
        let status_icon = if exceeds_threshold { "🔥" } else { "🟢" };
        log_temperature!(&reading.sensor_name, temp, threshold);

        // Temperature already logged by log_temperature! macro
    }

    // Update GUI manager with current temperature
    gui_manager.update_temperature(max_temp);

    // Update web state with current temperature and add log entries
    {
        let mut state = web_state.write().unwrap();
        let gpu_status = if gpu_temps.is_empty() {
            "No GPU detected"
        } else if max_temp > 0.0 {
            "Connected"
        } else {
            "Error reading temperature"
        };
        
        state.update_temperature(
            if max_temp > 0.0 { Some(max_temp) } else { None },
            gpu_status
        );
        state.config = shared_config.read().unwrap().clone();

        // Add temperature reading to web logs
        for reading in &gpu_temps {
            let level = if reading.temperature > threshold {
                "WARN"
            } else {
                "INFO"
            };
            let message = format!("{}: {:.1}°C", reading.sensor_name, reading.temperature);
            state.add_log(level, &message);
        }
    }

    // Update system tray icon based on temperature
    if let Some(ref mut tray) = system_tray {
        if let Err(e) = tray.update_icon_for_temperature(max_temp, threshold) {
            warn!("⚠️  Failed to update tray icon: {}", e);
        }
    }

    // Check if we should send notification
    if notification_manager.should_notify(any_over_threshold) {
        let cooldown_level = notification_manager.cooldown_level;

        // Log temperature alert
        log_temperature!(&hottest_sensor, max_temp, threshold);

        notification_manager
            .send_temperature_alert(&hottest_sensor, max_temp, threshold)?;
    }

    Ok(())
}

fn print_help() {
    log_info!("🚀 GPU Temperature Monitor v0.1.0");
    log_info!("");
    log_info!("USAGE:");
    log_info!("    gpu-temp-watch.exe [OPTIONS]");
    log_info!("");
    log_info!("OPTIONS:");
    log_info!("    --install      Install autostart (add to Windows startup)");
    log_info!("    --uninstall    Remove autostart");
    log_info!("    --status       Show autostart status with diagnostics");
    log_info!("    --startup-test Run comprehensive startup diagnostics");
    log_info!("    --help         Show this help message");
    log_info!("");
    log_info!("EXAMPLES:");
    log_info!("    gpu-temp-watch.exe                    # Run temperature monitor");
    log_info!("    gpu-temp-watch.exe --install          # Install autostart");
    log_info!("    gpu-temp-watch.exe --status           # Check autostart status");
    log_info!("    gpu-temp-watch.exe --startup-test     # Test startup components");
    log_info!("");
    log_info!("The application will run in system tray and monitor GPU temperatures.");
    let config_path = AppPaths::get_config_path()
        .unwrap_or_else(|_| AppPaths::get_fallback_config_path());
    let log_path = AppPaths::get_log_file_path()
        .unwrap_or_else(|_| AppPaths::get_fallback_log_path());
    log_info!(&format!("Configuration file: {}", config_path.display()));
    log_info!(&format!("Log files: {}", log_path.display()));
}
