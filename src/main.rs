mod monitor;
mod notifications;
mod config;

use monitor::{TempMonitor, GpuTempReading};
use notifications::NotificationManager;
use config::Config;

use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 GPU Temperature Monitor v0.1.0");
    println!("🔧 Initializing...");

    // Load configuration
    let mut config = Config::load_or_create()?;
    config.validate()?;

    // Initialize components
    let temp_monitor = TempMonitor::new();
    let mut notification_manager = NotificationManager::new();

    // Test LHM connection
    println!("🔌 Testing LibreHardwareMonitor connection...");
    match temp_monitor.test_connection().await {
        Ok(_) => println!("✅ Connected to LibreHardwareMonitor"),
        Err(e) => {
            eprintln!("❌ Failed to connect to LibreHardwareMonitor: {}", e);
            eprintln!("💡 Make sure LibreHardwareMonitor is running with web server enabled on port 8085");
            return Err(e);
        }
    }

    // Send startup notification
    notification_manager.send_status_notification("GPU Temperature Monitor started")?;

    println!("🌡️  Temperature threshold: {:.1}°C", config.temperature_threshold_c);
    println!("⏱️  Poll interval: {}s", config.poll_interval_sec);
    println!("🔄 Starting monitoring loop...");

    // Main monitoring loop
    loop {
        match monitor_temperatures(&temp_monitor, &mut notification_manager, &config).await {
            Ok(_) => {},
            Err(e) => {
                eprintln!("❌ Monitoring error: {}", e);
                // Continue monitoring despite errors
            }
        }

        sleep(Duration::from_secs(config.poll_interval_sec)).await;
    }
}

async fn monitor_temperatures(
    temp_monitor: &TempMonitor,
    notification_manager: &mut NotificationManager,
    config: &Config,
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
    }

    // Check if we should send notification
    if notification_manager.should_notify(any_over_threshold) {
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
