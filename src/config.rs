use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use crate::{log_info, log_error, log_warn};

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
            log_file_path: Some("./Logs/gpu-temp-watch.log".to_string()),
        }
    }
}

impl Config {
    pub fn load_or_create() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = Self::get_config_path();

        if config_path.exists() {
            let config_str = fs::read_to_string(&config_path)?;
            let config: Config = serde_json::from_str(&config_str)?;
            log_info!("Config loaded", serde_json::json!({"path": config_path.display().to_string()}));
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            log_info!("Created default config", serde_json::json!({"path": config_path.display().to_string()}));
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::get_config_path();


        // Validate configuration before saving
        if let Err(e) = self.validate() {
            log_error!("Configuration validation failed", serde_json::json!({"error": format!("{}", e)}));
            return Err(e);
        }

        // Check if parent directory exists and create if necessary
        if let Some(parent) = config_path.parent() {
            if !parent.exists() {

                if let Err(e) = fs::create_dir_all(parent) {
                    log_error!("Failed to create directory", serde_json::json!({"error": format!("{}", e)}));
                    return Err(e.into());
                }
            }
        }

        // Check file permissions before writing
        if config_path.exists() {
            let metadata = fs::metadata(&config_path)?;

            if metadata.permissions().readonly() {
                log_error!("Config file is read-only");
                return Err("Configuration file is read-only".into());
            }
        }

        // Serialize configuration to JSON
        let config_str = match serde_json::to_string_pretty(self) {
            Ok(json_str) => {
                json_str
            }
            Err(e) => {
                log_error!("Failed to serialize config", serde_json::json!({"error": format!("{}", e)}));
                return Err(e.into());
            }
        };

        // Write to file
        match fs::write(&config_path, &config_str) {
            Ok(_) => {
                log_info!("Configuration saved", serde_json::json!({"path": config_path.display().to_string()}));

                // Verify the file was written correctly
                match fs::read_to_string(&config_path) {
                    Ok(read_back) => {
                        if read_back == config_str {
                        } else {
                            log_warn!("File verification failed");
                        }
                    }
                    Err(e) => {
                        log_warn!("Could not verify saved file", serde_json::json!({"error": format!("{}", e)}));
                    }
                }

                Ok(())
            }
            Err(e) => {
                log_error!("Failed to write config file", serde_json::json!({"error": format!("{}", e)}));
                Err(e.into())
            }
        }
    }

    fn get_config_path() -> PathBuf {
        // Use absolute path to avoid working directory issues
        if let Ok(current_dir) = std::env::current_dir() {
            current_dir.join("config.json")
        } else {
            PathBuf::from("./config.json")
        }
    }

    pub fn update_threshold(
        &mut self,
        new_threshold: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.temperature_threshold_c = new_threshold;
        self.save()?;
        log_info!("Temperature threshold updated", serde_json::json!({"threshold": new_threshold}));
        Ok(())
    }

    pub fn update_poll_interval(
        &mut self,
        new_interval: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.poll_interval_sec = new_interval;
        self.save()?;
        log_info!("Poll interval updated", serde_json::json!({"interval": new_interval}));
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
