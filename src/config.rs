use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub temperature_threshold_c: f32,
    pub poll_interval_sec: u64,
    pub base_cooldown_sec: u64,
    pub enable_logging: bool,
    pub log_file_path: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            temperature_threshold_c: 60.0,
            poll_interval_sec: 20,
            base_cooldown_sec: 20,
            enable_logging: true,
            log_file_path: Some("./Logs/GpuTempWatch.log".to_string()),
        }
    }
}

impl Config {
    pub fn load_or_create() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = Self::get_config_path();

        if config_path.exists() {
            let config_str = fs::read_to_string(&config_path)?;
            let config: Config = serde_json::from_str(&config_str)?;
            println!("📋 Config loaded from: {:?}", config_path);
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            println!("📋 Created default config at: {:?}", config_path);
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::get_config_path();

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let config_str = serde_json::to_string_pretty(self)?;
        fs::write(&config_path, config_str)?;
        println!("💾 Config saved to: {:?}", config_path);
        Ok(())
    }

    fn get_config_path() -> PathBuf {
        PathBuf::from("./config.json")
    }

    pub fn update_threshold(&mut self, new_threshold: f32) -> Result<(), Box<dyn std::error::Error>> {
        self.temperature_threshold_c = new_threshold;
        self.save()?;
        println!("🌡️  Temperature threshold updated to: {:.1}°C", new_threshold);
        Ok(())
    }

    pub fn update_poll_interval(&mut self, new_interval: u64) -> Result<(), Box<dyn std::error::Error>> {
        self.poll_interval_sec = new_interval;
        self.save()?;
        println!("⏱️  Poll interval updated to: {}s", new_interval);
        Ok(())
    }

    pub fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.temperature_threshold_c < 0.0 || self.temperature_threshold_c > 150.0 {
            return Err("Temperature threshold must be between 0 and 150°C".into());
        }

        if self.poll_interval_sec < 5 || self.poll_interval_sec > 3600 {
            return Err("Poll interval must be between 5 and 3600 seconds".into());
        }

        if self.base_cooldown_sec < 1 || self.base_cooldown_sec > 600 {
            return Err("Base cooldown must be between 1 and 600 seconds".into());
        }

        Ok(())
    }
}