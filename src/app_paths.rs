use std::path::PathBuf;
use std::fs;

/// Centralized application path management for GpuTempWatch
///
/// This module provides functions to get consistent paths for application data,
/// configuration, and logs using Windows %LOCALAPPDATA% directory.
pub struct AppPaths;

impl AppPaths {
    /// Get the main application data directory: %LOCALAPPDATA%\GpuTempWatch
    pub fn get_app_data_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let localappdata = std::env::var("LOCALAPPDATA")
            .map_err(|_| "LOCALAPPDATA environment variable not found")?;

        let app_dir = PathBuf::from(localappdata).join("GpuTempWatch");

        // Create directory if it doesn't exist
        if !app_dir.exists() {
            fs::create_dir_all(&app_dir)?;
        }

        Ok(app_dir)
    }

    /// Get the configuration file path: %LOCALAPPDATA%\GpuTempWatch\config.json
    pub fn get_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let app_dir = Self::get_app_data_dir()?;
        Ok(app_dir.join("config.json"))
    }

    /// Get the log file path: %LOCALAPPDATA%\GpuTempWatch\gpu-temp-watch.log
    pub fn get_log_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let app_dir = Self::get_app_data_dir()?;
        Ok(app_dir.join("gpu-temp-watch.log"))
    }

    /// Get the logs directory path: %LOCALAPPDATA%\GpuTempWatch\Logs
    /// (For backward compatibility with existing log structure)
    pub fn get_logs_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let app_dir = Self::get_app_data_dir()?;
        let logs_dir = app_dir.join("Logs");

        // Create logs directory if it doesn't exist
        if !logs_dir.exists() {
            fs::create_dir_all(&logs_dir)?;
        }

        Ok(logs_dir)
    }

    /// Get the log file path in logs directory: %LOCALAPPDATA%\GpuTempWatch\Logs\gpu-temp-watch.log
    pub fn get_log_file_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let logs_dir = Self::get_logs_dir()?;
        Ok(logs_dir.join("gpu-temp-watch.log"))
    }

    /// Get fallback paths for backward compatibility
    /// Returns the current working directory based paths as fallback
    pub fn get_fallback_config_path() -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("config.json")
    }

    pub fn get_fallback_log_path() -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("Logs")
            .join("gpu-temp-watch.log")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_functions() {
        // These tests will work on Windows systems with LOCALAPPDATA set
        if std::env::var("LOCALAPPDATA").is_ok() {
            assert!(AppPaths::get_app_data_dir().is_ok());
            assert!(AppPaths::get_config_path().is_ok());
            assert!(AppPaths::get_log_path().is_ok());
            assert!(AppPaths::get_logs_dir().is_ok());
            assert!(AppPaths::get_log_file_path().is_ok());
        }

        // Fallback paths should always work
        assert!(AppPaths::get_fallback_config_path().to_string_lossy().contains("config.json"));
        assert!(AppPaths::get_fallback_log_path().to_string_lossy().contains("gpu-temp-watch.log"));
    }

    #[test]
    fn test_paths_consistency() {
        if std::env::var("LOCALAPPDATA").is_ok() {
            let app_dir = AppPaths::get_app_data_dir().unwrap();
            let config_path = AppPaths::get_config_path().unwrap();
            let log_path = AppPaths::get_log_path().unwrap();
            let logs_dir = AppPaths::get_logs_dir().unwrap();
            let log_file_path = AppPaths::get_log_file_path().unwrap();

            assert_eq!(config_path.parent().unwrap(), app_dir);
            assert_eq!(log_path.parent().unwrap(), app_dir);
            assert_eq!(logs_dir.parent().unwrap(), app_dir);
            assert_eq!(log_file_path.parent().unwrap(), logs_dir);
        }
    }
}