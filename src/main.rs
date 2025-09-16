#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod config;
mod gui;
mod logging;
mod monitor;
mod notifications;
mod tray;
mod universal_logger;
mod web_server;

use autostart::AutoStart;
use config::Config;
use gui::GuiManager;
use logging::FileLogger;
use monitor::TempMonitor;
use notifications::NotificationManager;
use tray::{SystemTray, TrayMessage};
use web_server::{open_browser, WebServer};

use std::env;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

fn debug_print(msg: &str) {
    #[cfg(debug_assertions)]
    println!("{}", msg);

    #[cfg(not(debug_assertions))]
    info!("{}", msg);
}

#[tracing::instrument]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .json()
        .init();

    // Initialize universal logger for dual output (console + file)
    universal_logger::init_logger(Some("./Logs/GpuTempWatch_detailed.log"), true);

    // Test universal logger
    log_both!(
        info,
        "🚀 GPU Temperature Monitor v0.1.0 starting up",
        Some(serde_json::json!({
            "version": "0.1.0",
            "startup_time": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
        }))
    );

    // Handle command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--install" => {
                debug_print("🚀 GPU Temperature Monitor v0.1.0");
                debug_print("📥 Installing autostart...");
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
                println!("🚀 GPU Temperature Monitor v0.1.0");
                println!("📤 Removing autostart...");
                match AutoStart::new() {
                    Ok(autostart) => match autostart.uninstall() {
                        Ok(_) => autostart.print_status(),
                        Err(e) => eprintln!("❌ Failed to remove autostart: {:?}", e),
                    },
                    Err(e) => eprintln!("❌ Failed to create autostart: {:?}", e),
                }
                return Ok(());
            }
            "--status" => {
                println!("🚀 GPU Temperature Monitor v0.1.0");
                println!("📋 Autostart status:");
                match AutoStart::new() {
                    Ok(autostart) => autostart.print_status(),
                    Err(e) => eprintln!("❌ Failed to check autostart: {:?}", e),
                }
                return Ok(());
            }
            "--help" => {
                print_help();
                return Ok(());
            }
            _ => {
                println!("❌ Unknown argument: {}", args[1]);
                print_help();
                return Ok(());
            }
        }
    }

    println!("🚀 GPU Temperature Monitor v0.1.0");
    println!("🔧 Initializing...");

    // Check and install autostart on first run if not already installed
    match AutoStart::new() {
        Ok(autostart) => {
            if !autostart.is_installed() {
                println!("📦 First run detected, installing autostart...");
                match autostart.install() {
                    Ok(_) => println!("✅ Autostart installed successfully"),
                    Err(e) => eprintln!("⚠️ Failed to install autostart: {:?}", e),
                }
            }
        }
        Err(e) => eprintln!("⚠️ Failed to create autostart manager: {:?}", e),
    }

    // Load configuration and wrap in Arc<RwLock> for shared access
    let config = Config::load_or_create()?;
    config.validate()?;
    let shared_config = Arc::new(RwLock::new(config));

    // Initialize components
    let temp_monitor = TempMonitor::new();
    let mut notification_manager = NotificationManager::new();
    let file_logger = FileLogger::new(&shared_config.read().unwrap())?;
    let mut system_tray = match SystemTray::new() {
        Ok(tray) => {
            println!("✅ System tray initialized successfully");
            Some(tray)
        }
        Err(e) => {
            eprintln!("❌ Failed to create system tray: {}", e);
            let _ = notification_manager.send_status_notification_sync(&format!(
                "❌ System Tray Error: {}. Continuing without tray integration.",
                e
            ));
            None
        }
    };

    // Test NVML connection
    println!("🔌 Testing NVML connection...");
    match temp_monitor.test_connection().await {
        Ok(_) => println!("✅ Connected to NVML"),
        Err(e) => {
            let error_msg = format!("❌ Failed to connect to NVML: {}\n\n💡 Make sure NVIDIA drivers are installed and GPU is available", e);
            eprintln!("{}", error_msg);

            // Send error notification and exit
            let _ = notification_manager
                .send_status_notification_sync(&format!("❌ NVML Connection Error: {}", e));
            return Err(e);
        }
    }

    // Send startup notification
    notification_manager
        .send_status_notification("GPU Temperature Monitor started")
        .await?;

    // Send startup toast notification
    #[cfg(debug_assertions)]
    {
        let _ = notification_manager.send_status_notification_sync(
            "🚀 GPU Temperature Monitor started successfully! Right-click tray icon for options.",
        );
    }

    {
        let config = shared_config.read().unwrap();
        println!(
            "🌡️  Temperature threshold: {:.1}°C",
            config.temperature_threshold_c
        );
        println!("⏱️  Poll interval: {}s", config.poll_interval_sec);
    }
    println!("🔄 Starting monitoring loop...");

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
                eprintln!("❌ Web server error: {}", e);
            }
        })
    };

    println!("🌐 Web interface available at http://localhost:18235");

    // Main monitoring loop
    loop {
        // Handle system tray messages
        if let Some(ref mut tray) = system_tray {
            if let Some(message) = tray.get_message() {
                println!("📬 Received tray message: {:?}", message);
                match message {
                    TrayMessage::QuitMonitor => {
                        println!("🚪 Quitting monitor via system tray");
                        return Ok(());
                    }
                    TrayMessage::OpenDashboard => {
                        println!("🌐 Opening dashboard...");
                        info!("Tray request: open dashboard");
                        if let Err(e) = open_browser("http://localhost:18235") {
                            warn!("Failed to open dashboard: {}", e);
                            let _ = notification_manager.send_status_notification_sync(&format!(
                                "❌ Failed to open dashboard: {}",
                                e
                            ));
                        } else {
                            info!("Dashboard launched in default browser");
                            let _ = notification_manager
                                .send_status_notification_sync("🌐 Dashboard opened in browser");
                        }
                    }
                    TrayMessage::ViewLogs => {
                        println!("📋 View logs clicked");
                        let log_path = std::env::current_dir()
                            .unwrap_or_default()
                            .join("Logs")
                            .join("GpuTempWatch.log");

                        // Ensure log file exists before trying to open
                        if !log_path.exists() {
                            if let Some(parent) = log_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            let _ = std::fs::write(&log_path, "Log file created\n");
                        }

                        if let Err(e) =
                            crate::gui::GuiDialogs::open_file(&log_path.to_string_lossy())
                        {
                            let _ = notification_manager.send_status_notification_sync(&format!(
                                "❌ Failed to open log file: {}",
                                e
                            ));
                        }
                    }
                    TrayMessage::EditSettings => {
                        println!("⚙️ Edit settings clicked");
                        let config_path = std::env::current_dir()
                            .unwrap_or_default()
                            .join("config.json");

                        if let Err(e) =
                            crate::gui::GuiDialogs::open_file(&config_path.to_string_lossy())
                        {
                            let _ = notification_manager.send_status_notification_sync(&format!(
                                "❌ Failed to open config file: {}",
                                e
                            ));
                        }
                    }
                }
            }
        }

        // Monitor temperatures if not paused
        if !monitoring_paused {
            match monitor_temperatures(
                &temp_monitor,
                &mut notification_manager,
                &shared_config,
                &mut system_tray,
                &file_logger,
                &mut gui_manager,
                &web_state,
            )
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("❌ Monitoring error: {}", e);
                    let _ = file_logger.log_error(&format!("Monitoring error: {}", e));

                    // Show GUI error dialog for critical errors
                    #[cfg(debug_assertions)]
                    crate::gui::GuiDialogs::show_warning(
                        "Monitoring Warning",
                        &format!(
                            "⚠️ Monitoring error occurred:\n\n{}\n\nMonitoring will continue...",
                            e
                        ),
                    );

                    // Continue monitoring despite errors
                }
            }
        }

        // Update web state
        let poll_interval = {
            let config = shared_config.read().unwrap();
            let mut state = web_state.write().unwrap();
            state.monitoring_paused = monitoring_paused;
            state.uptime_seconds += config.poll_interval_sec;
            config.poll_interval_sec
        };

        sleep(Duration::from_secs(poll_interval)).await;
    }
}

#[tracing::instrument(skip_all)]
async fn monitor_temperatures(
    temp_monitor: &TempMonitor,
    notification_manager: &mut NotificationManager,
    shared_config: &Arc<RwLock<Config>>,
    system_tray: &mut Option<SystemTray>,
    file_logger: &FileLogger,
    gui_manager: &mut GuiManager,
    web_state: &web_server::SharedState,
) -> Result<(), Box<dyn std::error::Error>> {
    let gpu_temps = temp_monitor.get_gpu_temperatures().await?;

    if gpu_temps.is_empty() {
        println!("⚠️  No GPU temperature sensors found");
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
        println!("{} {}: {:.1}°C", status_icon, reading.sensor_name, temp);

        // Log to file
        let _ = file_logger.log_temperature_reading(&reading.sensor_name, temp, threshold);
    }

    // Update GUI manager with current temperature
    gui_manager.update_temperature(max_temp);

    // Update web state with current temperature and add log entries
    {
        let mut state = web_state.write().unwrap();
        state.current_temperature = max_temp;
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

        // Broadcast temperature update to WebSocket clients
        state.broadcast_temperature_update();
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

        // Log alert to file
        let _ = file_logger.log_alert(&hottest_sensor, max_temp, threshold, cooldown_level);

        notification_manager
            .send_temperature_alert(&hottest_sensor, max_temp, threshold)
            .await?;
    }

    Ok(())
}

fn print_help() {
    println!("🚀 GPU Temperature Monitor v0.1.0");
    println!();
    println!("USAGE:");
    println!("    gpu-temp-watch.exe [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    --install      Install autostart (add to Windows startup)");
    println!("    --uninstall    Remove autostart");
    println!("    --status       Show autostart status");
    println!("    --help         Show this help message");
    println!();
    println!("EXAMPLES:");
    println!("    gpu-temp-watch.exe                    # Run temperature monitor");
    println!("    gpu-temp-watch.exe --install          # Install autostart");
    println!("    gpu-temp-watch.exe --status           # Check autostart status");
    println!();
    println!("The application will run in system tray and monitor GPU temperatures.");
    println!("Configuration file: ./config.json");
    println!("Log file: ./Logs/GpuTempWatch.log");
}
