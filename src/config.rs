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
        use crate::log_both;

        let config_path = Self::get_config_path();

        log_both!(
            info,
            "💾 Starting configuration save process",
            Some(serde_json::json!({
                "config_path": config_path.to_string_lossy(),
                "config_data": {
                    "temperature_threshold_c": self.temperature_threshold_c,
                    "poll_interval_sec": self.poll_interval_sec,
                    "base_cooldown_sec": self.base_cooldown_sec,
                    "enable_logging": self.enable_logging,
                    "log_file_path": self.log_file_path
                }
            }))
        );

        // Validate configuration before saving
        if let Err(e) = self.validate() {
            log_both!(
                error,
                "❌ Configuration validation failed before saving",
                Some(serde_json::json!({
                    "error": e.to_string(),
                    "config_path": config_path.to_string_lossy()
                }))
            );
            return Err(e);
        }

        // Check if parent directory exists and create if necessary
        if let Some(parent) = config_path.parent() {
            if !parent.exists() {
                log_both!(
                    info,
                    "📁 Creating parent directory for config",
                    Some(serde_json::json!({
                        "parent_dir": parent.to_string_lossy()
                    }))
                );

                if let Err(e) = fs::create_dir_all(parent) {
                    log_both!(
                        error,
                        "❌ Failed to create parent directory",
                        Some(serde_json::json!({
                            "parent_dir": parent.to_string_lossy(),
                            "error": e.to_string()
                        }))
                    );
                    return Err(e.into());
                }
            }
        }

        // Check file permissions before writing
        if config_path.exists() {
            let metadata = fs::metadata(&config_path)?;
            log_both!(
                debug,
                "📄 Existing config file metadata",
                Some(serde_json::json!({
                    "file_size": metadata.len(),
                    "readonly": metadata.permissions().readonly(),
                    "modified": metadata.modified().map(|t| format!("{:?}", t)).unwrap_or_else(|_| "unknown".to_string())
                }))
            );

            if metadata.permissions().readonly() {
                log_both!(
                    error,
                    "❌ Config file is read-only",
                    Some(serde_json::json!({
                        "config_path": config_path.to_string_lossy()
                    }))
                );
                return Err("Configuration file is read-only".into());
            }
        }

        // Serialize configuration to JSON
        let config_str = match serde_json::to_string_pretty(self) {
            Ok(json_str) => {
                log_both!(
                    debug,
                    "✅ Configuration serialized to JSON",
                    Some(serde_json::json!({
                        "json_length": json_str.len()
                    }))
                );
                json_str
            }
            Err(e) => {
                log_both!(
                    error,
                    "❌ Failed to serialize configuration to JSON",
                    Some(serde_json::json!({
                        "error": e.to_string()
                    }))
                );
                return Err(e.into());
            }
        };

        // Write to file
        match fs::write(&config_path, &config_str) {
            Ok(_) => {
                log_both!(
                    info,
                    "✅ Configuration saved successfully",
                    Some(serde_json::json!({
                        "config_path": config_path.to_string_lossy(),
                        "file_size": config_str.len()
                    }))
                );

                // Verify the file was written correctly
                match fs::read_to_string(&config_path) {
                    Ok(read_back) => {
                        if read_back == config_str {
                            log_both!(debug, "✅ File verification successful", None);
                        } else {
                            log_both!(
                                warn,
                                "⚠️ File verification failed - content mismatch",
                                Some(serde_json::json!({
                                    "expected_length": config_str.len(),
                                    "actual_length": read_back.len()
                                }))
                            );
                        }
                    }
                    Err(e) => {
                        log_both!(
                            warn,
                            "⚠️ Could not verify saved file",
                            Some(serde_json::json!({
                                "error": e.to_string()
                            }))
                        );
                    }
                }

                Ok(())
            }
            Err(e) => {
                log_both!(
                    error,
                    "❌ Failed to write configuration file",
                    Some(serde_json::json!({
                        "config_path": config_path.to_string_lossy(),
                        "error": e.to_string(),
                        "error_kind": format!("{:?}", e.kind())
                    }))
                );
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
        println!(
            "🌡️  Temperature threshold updated to: {:.1}°C",
            new_threshold
        );
        Ok(())
    }

    pub fn update_poll_interval(
        &mut self,
        new_interval: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
