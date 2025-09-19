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
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};

fn debug_print(msg: &str) {
    #[cfg(debug_assertions)]
    println!("{}", msg);

    #[cfg(not(debug_assertions))]
    info!("{}", msg);
}

// Fast tray event handler - runs every 100ms
async fn handle_tray_events(
    mut system_tray: SystemTray,
    _notification_manager: Arc<Mutex<NotificationManager>>,
) {
    println!("🚀 Starting fast tray event handler (100ms interval)");
    
    loop {
        if let Some(message) = system_tray.get_message() {
            println!("📬 Received tray message: {:?}", message);
            
            match message {
                TrayMessage::QuitMonitor => {
                    println!("🚪 Quitting monitor via system tray");
                    std::process::exit(0);
                }
                TrayMessage::OpenDashboard => {
                    println!("🌐 Opening dashboard...");
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

                    tokio::spawn(async move {
                        if let Err(e) = crate::gui::GuiDialogs::open_file(&log_path.to_string_lossy()) {
                            eprintln!("❌ Failed to open log file: {}", e);
                        }
                    });
                }
                TrayMessage::EditSettings => {
                    println!("⚙️ Edit settings clicked");
                    let config_path = std::env::current_dir()
                        .unwrap_or_default()
                        .join("config.json");

                    tokio::spawn(async move {
                        if let Err(e) = crate::gui::GuiDialogs::open_file(&config_path.to_string_lossy()) {
                            eprintln!("❌ Failed to open config file: {}", e);
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
    file_logger: FileLogger,
    mut gui_manager: GuiManager,
    web_state: web_server::SharedState,
    tray_sender: Option<mpsc::Sender<TrayIconUpdate>>,
) {
    println!("🚀 Starting temperature monitoring handler");
    let monitoring_paused = false;
    
    loop {
        if !monitoring_paused {
            let poll_interval = match monitor_temperatures_cycle(
                &temp_monitor,
                &notification_manager,
                &shared_config,
                &file_logger,
                &mut gui_manager,
                &web_state,
                &tray_sender,
            ).await {
                Ok(interval) => interval,
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
    file_logger: &FileLogger,
    gui_manager: &mut GuiManager,
    web_state: &web_server::SharedState,
    tray_sender: &Option<mpsc::Sender<TrayIconUpdate>>,
) -> Result<u64, Box<dyn std::error::Error>> {
    let gpu_temps = temp_monitor.get_gpu_temperatures().await?;

    if gpu_temps.is_empty() {
        println!("⚠️  No GPU temperature sensors found");
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
        println!("{} {}: {:.1}°C", status_icon, reading.sensor_name, temp);

        // Log to file
        let _ = file_logger.log_temperature_reading(&reading.sensor_name, temp, threshold);
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

            // Log alert to file
            let _ = file_logger.log_alert(&hottest_sensor, max_temp, threshold, cooldown_level);

            // Send notification directly (already in async context)
            let _ = nm.send_temperature_alert(&hottest_sensor, max_temp, threshold).await;
        }
    }

    // Return poll interval for next cycle
    Ok(shared_config.read().unwrap().poll_interval_sec)
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
        file_logger,
        gui_manager,
        web_state.clone(),
        Some(tray_tx),
    ));

    // Handle tray icon updates in a separate task
    let tray_icon_handle = tokio::spawn(async move {
        // This would need access to the tray's command sender
        // For now, we'll just consume the messages
        while let Some(update) = tray_rx.recv().await {
            println!("🎨 Tray icon update: {:.1}°C (threshold: {:.1}°C)", 
                     update.temperature, update.threshold);
            // In a full implementation, we'd send this to the tray thread
        }
    });

    println!("🚀 All handlers started - fast tray polling (100ms), temperature monitoring ({}s)", 
             shared_config.read().unwrap().poll_interval_sec);

    // Wait for any task to complete (which means an error or shutdown)
    tokio::select! {
        result = monitor_handle => {
            match result {
                Ok(()) => println!("✅ Temperature monitoring completed successfully"),
                Err(e) => eprintln!("❌ Temperature monitoring task panicked: {}", e),
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
                Ok(()) => println!("✅ Tray handler completed successfully"),
                Err(e) => eprintln!("❌ Tray handler task panicked: {}", e),
            }
        }
        _ = tray_icon_handle => {
            println!("✅ Tray icon handler completed");
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
