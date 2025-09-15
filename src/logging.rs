use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use chrono::Local;
use log::{error, info};

pub struct FileLogger {
    log_file_path: Option<String>,
    enabled: bool,
}

impl FileLogger {
    pub fn new(config: &crate::config::Config) -> Result<Self, Box<dyn std::error::Error>> {
        let logger = Self {
            log_file_path: config.log_file_path.clone(),
            enabled: config.enable_logging,
        };

        if logger.enabled {
            logger.ensure_log_directory()?;
            logger.log_startup()?;
        }

        Ok(logger)
    }

    fn ensure_log_directory(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref log_path) = self.log_file_path {
            if let Some(parent) = Path::new(log_path).parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)?;
                    info!("📁 Created log directory: {:?}", parent);
                }
            }
        }
        Ok(())
    }

    fn log_startup(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.write_log_entry(&format!("🚀 GPU Temperature Monitor v0.1.0 started"))?;
        self.write_log_entry(&format!("📋 Logging enabled, file: {:?}", self.log_file_path))?;
        Ok(())
    }

    pub fn log_temperature_reading(&self, sensor_name: &str, temperature: f32, threshold: f32) -> Result<(), Box<dyn std::error::Error>> {
        if !self.enabled {
            return Ok(());
        }

        let status = if temperature > threshold {
            "HOT 🔥"
        } else if temperature > threshold - 10.0 {
            "WARM 🟡"
        } else {
            "COOL 🟢"
        };

        let message = format!(
            "{} {}: {:.1}°C (Threshold: {:.1}°C)",
            status, sensor_name, temperature, threshold
        );

        self.write_log_entry(&message)
    }

    pub fn log_alert(&self, sensor_name: &str, temperature: f32, threshold: f32, cooldown_level: u32) -> Result<(), Box<dyn std::error::Error>> {
        if !self.enabled {
            return Ok(());
        }

        let message = format!(
            "🚨 ALERT #{}: {} reached {:.1}°C (Threshold: {:.1}°C)",
            cooldown_level + 1, sensor_name, temperature, threshold
        );

        self.write_log_entry(&message)
    }

    pub fn log_status(&self, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        if !self.enabled {
            return Ok(());
        }

        self.write_log_entry(&format!("ℹ️ {}", message))
    }

    pub fn log_error(&self, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        if !self.enabled {
            return Ok(());
        }

        self.write_log_entry(&format!("❌ ERROR: {}", message))
    }

    fn write_log_entry(&self, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref log_path) = self.log_file_path {
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let log_line = format!("[{}] {}\n", timestamp, message);

            match OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
            {
                Ok(mut file) => {
                    if let Err(e) = file.write_all(log_line.as_bytes()) {
                        error!("Failed to write to log file: {}", e);
                        return Err(e.into());
                    }
                }
                Err(e) => {
                    error!("Failed to open log file {}: {}", log_path, e);
                    return Err(e.into());
                }
            }
        }

        Ok(())
    }

    pub fn cleanup_old_logs(&self, max_age_days: u32) -> Result<(), Box<dyn std::error::Error>> {
        if !self.enabled {
            return Ok(());
        }

        if let Some(ref log_path) = self.log_file_path {
            if let Some(_parent) = Path::new(log_path).parent() {
                // This is a simplified cleanup - in a real implementation,
                // you'd parse log file dates and remove old ones
                println!("🧹 Log cleanup functionality (max age: {} days) - placeholder", max_age_days);
            }
        }

        Ok(())
    }
}