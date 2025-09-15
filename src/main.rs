#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod monitor;
mod notifications;
mod config;
mod tray;
mod logging;
mod autostart;
mod gui;
mod web_server;

use monitor::{TempMonitor, GpuTempReading};
use notifications::NotificationManager;
use config::Config;
use tray::{SystemTray, TrayMessage};
use logging::FileLogger;
use autostart::AutoStart;
use gui::{GuiDialogs, GuiManager};
use web_server::{WebServer, open_browser};

use std::time::Duration;
use std::env;
use tokio::time::sleep;
use log::warn;

fn debug_print(msg: &str) {
    #[cfg(debug_assertions)]
    println!("{}", msg);

    #[cfg(not(debug_assertions))]
    log::info!("{}", msg);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Handle command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--install" => {
                debug_print("🚀 GPU Temperature Monitor v0.1.0");
                debug_print("📥 Installing autostart...");
                match AutoStart::new() {
                    Ok(autostart) => {
                        match autostart.install() {
                            Ok(_) => autostart.print_status(),
                            Err(e) => debug_print(&format!("❌ Failed to install autostart: {:?}", e)),
                        }
                    }
                    Err(e) => debug_print(&format!("❌ Failed to create autostart: {:?}", e)),
                }
                return Ok(());
            }
            "--uninstall" => {
                println!("🚀 GPU Temperature Monitor v0.1.0");
                println!("📤 Removing autostart...");
                match AutoStart::new() {
                    Ok(autostart) => {
                        match autostart.uninstall() {
                            Ok(_) => autostart.print_status(),
                            Err(e) => eprintln!("❌ Failed to remove autostart: {:?}", e),
                        }
                    }
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

    // Load configuration
    let config = Config::load_or_create()?;
    config.validate()?;

    // Initialize components
    let temp_monitor = TempMonitor::new();
    let mut notification_manager = NotificationManager::new();
    let file_logger = FileLogger::new(&config)?;
    let mut system_tray = match SystemTray::new() {
        Ok(tray) => {
            println!("✅ System tray initialized successfully");
            Some(tray)
        }
        Err(e) => {
            eprintln!("❌ Failed to create system tray: {}", e);
            let _ = notification_manager.send_status_notification(&format!("❌ System Tray Error: {}. Continuing without tray integration.", e));
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
            let _ = notification_manager.send_status_notification(&format!("❌ NVML Connection Error: {}", e));
            return Err(e);
        }
    }

    // Send startup notification
    notification_manager.send_status_notification("GPU Temperature Monitor started")?;

    // Send startup toast notification
    #[cfg(debug_assertions)]
    {
        let _ = notification_manager.send_status_notification("🚀 GPU Temperature Monitor started successfully! Right-click tray icon for options.");
    }

    println!("🌡️  Temperature threshold: {:.1}°C", config.temperature_threshold_c);
    println!("⏱️  Poll interval: {}s", config.poll_interval_sec);
    println!("🔄 Starting monitoring loop...");

    let mut monitoring_paused = false;
    let mut gui_manager = GuiManager::new();

    // Initialize and start web server
    let web_server = WebServer::new(config.clone(), 18235);
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
        if let Some(ref tray) = system_tray {
            if let Some(message) = tray.get_message() {
                println!("📬 Received tray message: {:?}", message);
                match message {
                    TrayMessage::Exit => {
                        println!("🚪 Exiting via system tray");
                        return Ok(());
                    }
                    TrayMessage::Pause => {
                        println!("⏸️ Monitoring paused");
                        monitoring_paused = true;
                        gui_manager.set_monitoring_paused(true);
                        // Send toast notification instead of modal dialog
                        let _ = notification_manager.send_status_notification("⏸️ GPU monitoring paused");
                    }
                    TrayMessage::Resume => {
                        println!("▶️ Monitoring resumed");
                        monitoring_paused = false;
                        gui_manager.set_monitoring_paused(false);
                        // Send toast notification instead of modal dialog
                        let _ = notification_manager.send_status_notification("▶️ GPU monitoring resumed");
                    }
                    TrayMessage::Settings => {
                        println!("⚙️ Settings clicked");
                        let settings_msg = format!("🔧 Current Settings:\n\n🌡️ Temperature Threshold: {:.1}°C\n⏱️ Poll Interval: {}s\n\n💡 Edit config.json to change settings", config.temperature_threshold_c, config.poll_interval_sec);
                        let _ = notification_manager.send_status_notification(&settings_msg);
                    }
                    TrayMessage::OpenWebInterface => {
                        println!("🌐 Opening web interface...");
                        if let Err(e) = open_browser("http://localhost:18235") {
                            let _ = notification_manager.send_status_notification(&format!("❌ Failed to open web interface: {}", e));
                        } else {
                            let _ = notification_manager.send_status_notification("🌐 Web interface opened in browser");
                        }
                    }
                    TrayMessage::ShowLogs => {
                        println!("📋 Show logs clicked");
                        if let Some(log_path) = &config.log_file_path {
                            if let Err(e) = GuiDialogs::open_file(log_path) {
                                let _ = notification_manager.send_status_notification(&format!("❌ Failed to open log file: {}", e));
                            }
                        } else {
                            let _ = notification_manager.send_status_notification("⚠️ Log file path not configured");
                        }
                    }
                    TrayMessage::About => {
                        println!("ℹ️ About clicked");
                        let about_msg = "🚀 GPU Temperature Monitor v0.1.0\n\nReal-time GPU temperature monitoring with notifications and system tray integration.\n\n💡 Right-click tray icon for options";
                        let _ = notification_manager.send_status_notification(about_msg);
                    }
                    TrayMessage::OpenConfig => {
                        println!("📂 Open config clicked");
                        if let Err(e) = GuiDialogs::open_file("./config.json") {
                            let _ = notification_manager.send_status_notification(&format!("❌ Failed to open config file: {}", e));
                        }
                    }
                    TrayMessage::OpenLogsFolder => {
                        println!("📁 Open logs folder clicked");
                        if let Err(e) = GuiDialogs::open_folder("./Logs") {
                            let _ = notification_manager.send_status_notification(&format!("❌ Failed to open logs folder: {}", e));
                        }
                    }
                    TrayMessage::InstallAutostart => {
                        println!("⚙️ Installing autostart...");
                        match AutoStart::new() {
                            Ok(autostart) => {
                                match autostart.install() {
                                    Ok(_) => {
                                        let _ = notification_manager.send_status_notification("✅ Autostart installed! Application will start automatically with Windows.");
                                    }
                                    Err(e) => {
                                        let _ = notification_manager.send_status_notification(&format!("❌ Failed to install autostart: {:?}", e));
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = notification_manager.send_status_notification(&format!("❌ Failed to create autostart: {:?}", e));
                            }
                        }
                    }
                    TrayMessage::UninstallAutostart => {
                        println!("🗑️ Removing autostart...");
                        match AutoStart::new() {
                            Ok(autostart) => {
                                match autostart.uninstall() {
                                    Ok(_) => {
                                        let _ = notification_manager.send_status_notification("✅ Autostart removed! Application will no longer start with Windows.");
                                    }
                                    Err(e) => {
                                        let _ = notification_manager.send_status_notification(&format!("❌ Failed to remove autostart: {:?}", e));
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = notification_manager.send_status_notification(&format!("❌ Failed to create autostart: {:?}", e));
                            }
                        }
                    }
                }
            }
        }

        // Monitor temperatures if not paused
        if !monitoring_paused {
            match monitor_temperatures(&temp_monitor, &mut notification_manager, &config, &mut system_tray, &file_logger, &mut gui_manager, &web_state).await {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("❌ Monitoring error: {}", e);
                    let _ = file_logger.log_error(&format!("Monitoring error: {}", e));

                    // Show GUI error dialog for critical errors
                    #[cfg(debug_assertions)]
                    GuiDialogs::show_warning("Monitoring Warning", &format!("⚠️ Monitoring error occurred:\n\n{}\n\nMonitoring will continue...", e));

                    // Continue monitoring despite errors
                }
            }
        }

        // Update web state
        {
            let mut state = web_state.write().unwrap();
            state.monitoring_paused = monitoring_paused;
            state.uptime_seconds += config.poll_interval_sec;
        }

        sleep(Duration::from_secs(config.poll_interval_sec)).await;
    }
}

async fn monitor_temperatures(
    temp_monitor: &TempMonitor,
    notification_manager: &mut NotificationManager,
    config: &Config,
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

    for reading in &gpu_temps {
        let temp = reading.temperature;
        let exceeds_threshold = temp > config.temperature_threshold_c;

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
        let _ = file_logger.log_temperature_reading(&reading.sensor_name, temp, config.temperature_threshold_c);
    }

    // Update GUI manager with current temperature
    gui_manager.update_temperature(max_temp);

    // Update web state with current temperature and add log entries
    {
        let mut state = web_state.write().unwrap();
        state.current_temperature = max_temp;
        state.config = config.clone();

        // Add temperature reading to web logs
        for reading in &gpu_temps {
            let level = if reading.temperature > config.temperature_threshold_c { "WARN" } else { "INFO" };
            let message = format!("{}: {:.1}°C", reading.sensor_name, reading.temperature);
            state.add_log(level, &message);
        }
    }

    // Update system tray icon based on temperature
    if let Some(ref mut tray) = system_tray {
        if let Err(e) = tray.update_icon_for_temperature(max_temp, config.temperature_threshold_c) {
            warn!("⚠️  Failed to update tray icon: {}", e);
        }
    }

    // Check if we should send notification
    if notification_manager.should_notify(any_over_threshold) {
        let cooldown_level = notification_manager.cooldown_level;

        // Log alert to file
        let _ = file_logger.log_alert(&hottest_sensor, max_temp, config.temperature_threshold_c, cooldown_level);

        notification_manager.send_temperature_alert(
            &hottest_sensor,
            max_temp,
            config.temperature_threshold_c,
        )?;
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

fn print_gpu_status(readings: &[GpuTempReading], threshold: f32) {
    println!("\n📊 GPU Temperature Status:");
    for reading in readings {
        let status = if reading.temperature > threshold {
            "🔥 HOT"
        } else if reading.temperature > threshold - 10.0 {
            "🟡 WARM"
        } else {
            "🟢 COOL"
        };

        println!(
            "  {} {}: {:.1}°C (Min: {:.1}°C, Max: {:.1}°C)",
            status,
            reading.sensor_name,
            reading.temperature,
            reading.min_temp.unwrap_or(0.0),
            reading.max_temp.unwrap_or(0.0)
        );
    }
    println!();
}
