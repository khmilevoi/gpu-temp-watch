mod monitor;
mod notifications;
mod config;
mod tray;
mod logging;

use monitor::{TempMonitor, GpuTempReading};
use notifications::NotificationManager;
use config::Config;
use tray::{SystemTray, TrayMessage};
use logging::FileLogger;

use std::time::Duration;
use tokio::time::sleep;
use log::{info, error, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    println!("🚀 GPU Temperature Monitor v0.1.0");
    println!("🔧 Initializing...");

    // Load configuration
    let config = Config::load_or_create()?;
    config.validate()?;

    // Initialize components
    let temp_monitor = TempMonitor::new();
    let mut notification_manager = NotificationManager::new();
    let file_logger = FileLogger::new(&config)?;
    let mut system_tray = SystemTray::new().map_err(|e| {
        warn!("⚠️  Failed to create system tray: {}", e);
        e
    }).ok();

    // Test NVML connection
    println!("🔌 Testing NVML connection...");
    match temp_monitor.test_connection().await {
        Ok(_) => println!("✅ Connected to NVML"),
        Err(e) => {
            eprintln!("❌ Failed to connect to NVML: {}", e);
            eprintln!("💡 Make sure NVIDIA drivers are installed and GPU is available");
            return Err(e);
        }
    }

    // Send startup notification
    notification_manager.send_status_notification("GPU Temperature Monitor started")?;

    println!("🌡️  Temperature threshold: {:.1}°C", config.temperature_threshold_c);
    println!("⏱️  Poll interval: {}s", config.poll_interval_sec);
    println!("🔄 Starting monitoring loop...");

    let mut monitoring_paused = false;

    // Main monitoring loop
    loop {
        // Handle system tray messages
        if let Some(ref tray) = system_tray {
            if let Some(message) = tray.get_message() {
                match message {
                    TrayMessage::Exit => {
                        println!("🚪 Exiting via system tray");
                        return Ok(());
                    }
                    TrayMessage::Pause => {
                        println!("⏸️ Monitoring paused");
                        monitoring_paused = true;
                    }
                    TrayMessage::Resume => {
                        println!("▶️ Monitoring resumed");
                        monitoring_paused = false;
                    }
                    TrayMessage::Settings => {
                        println!("⚙️ Settings clicked - opening config...");
                        // TODO: Open config dialog
                    }
                    TrayMessage::ShowLogs => {
                        println!("📋 Show logs clicked");
                        // TODO: Open log file
                    }
                }
            }
        }

        // Monitor temperatures if not paused
        if !monitoring_paused {
            match monitor_temperatures(&temp_monitor, &mut notification_manager, &config, &mut system_tray, &file_logger).await {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("❌ Monitoring error: {}", e);
                    let _ = file_logger.log_error(&format!("Monitoring error: {}", e));
                    // Continue monitoring despite errors
                }
            }
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
