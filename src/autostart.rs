use std::env;
use std::path::PathBuf;
use windows::{core::*, Win32::Foundation::*, Win32::System::Registry::*};
use crate::{log_info, log_error, log_debug};

pub struct AutoStart {
    app_name: String,
    app_path: PathBuf,
}

impl AutoStart {
    pub fn new() -> windows::core::Result<Self> {
        let app_name = "GpuTempWatch".to_string();
        let app_path = env::current_exe().map_err(|e| {
            log_error!("Failed to get current executable path", serde_json::json!({"error": format!("{}", e)}));
            windows::core::Error::from_win32()
        })?;

        Ok(AutoStart { app_name, app_path })
    }

    pub fn install(&self) -> windows::core::Result<()> {
        log_info!("Installing autostart", serde_json::json!({
            "app_name": self.app_name,
            "app_path": self.app_path.display().to_string()
        }));

        match self.add_to_registry() {
            Ok(_) => {
                log_info!("Autostart installed successfully");
                Ok(())
            }
            Err(e) => {
                log_error!("Failed to install autostart", serde_json::json!({"error": format!("{:?}", e)}));
                Err(e)
            }
        }
    }

    pub fn uninstall(&self) -> windows::core::Result<()> {
        log_info!("Removing autostart", serde_json::json!({"app_name": self.app_name}));

        match self.remove_from_registry() {
            Ok(_) => {
                log_info!("Autostart removed successfully");
                Ok(())
            }
            Err(e) => {
                log_error!("Failed to remove autostart", serde_json::json!({"error": format!("{:?}", e)}));
                Err(e)
            }
        }
    }

    pub fn is_installed(&self) -> bool {
        match self.check_registry() {
            Ok(installed) => {
                installed
            }
            Err(e) => {
                log_error!("Failed to check autostart status", serde_json::json!({"error": format!("{:?}", e)}));
                false
            }
        }
    }

    fn add_to_registry(&self) -> windows::core::Result<()> {
        unsafe {
            let mut key: HKEY = HKEY::default();

            // Open the Run registry key
            let open_result = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
                0,
                KEY_WRITE,
                &mut key,
            );

            if let Err(e) = open_result.ok() {
                log_error!("Failed to open registry key for write", serde_json::json!({"error": format!("{:?}", e)}));
                return Err(e);
            }

            // Convert app name to wide string
            let app_name_wide: Vec<u16> = self
                .app_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            // Convert app path to wide string
            let app_path_str = format!("\"{}\"", self.app_path.display());
            let app_path_wide: Vec<u16> = app_path_str
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            // Set the registry value
            let data_bytes: &[u8] = std::slice::from_raw_parts(
                app_path_wide.as_ptr() as *const u8,
                app_path_wide.len() * 2,
            );

            let result = RegSetValueExW(
                key,
                PCWSTR(app_name_wide.as_ptr()),
                0,
                REG_SZ,
                Some(data_bytes),
            );

            let _ = RegCloseKey(key);

            match result.ok() {
                Ok(_) => {
                    Ok(())
                }
                Err(e) => {
                    log_error!("Failed to set registry value", serde_json::json!({"error": format!("{:?}", e)}));
                    Err(e)
                }
            }
        }
    }

    fn remove_from_registry(&self) -> windows::core::Result<()> {
        unsafe {
            let mut key: HKEY = HKEY::default();

            // Open the Run registry key
            let open_result = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
                0,
                KEY_WRITE,
                &mut key,
            );

            if let Err(e) = open_result.ok() {
                log_error!("Failed to open registry key for removal", serde_json::json!({"error": format!("{:?}", e)}));
                return Err(e);
            }

            // Convert app name to wide string
            let app_name_wide: Vec<u16> = self
                .app_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();


            // Delete the registry value
            let result = RegDeleteValueW(key, PCWSTR(app_name_wide.as_ptr()));

            let _ = RegCloseKey(key);

            // Ignore ERROR_FILE_NOT_FOUND as it means the value doesn't exist (which is what we want)
            if result == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                match result.ok() {
                    Ok(_) => {
                        Ok(())
                    }
                    Err(e) => {
                        log_error!("Failed to delete registry value", serde_json::json!({"error": format!("{:?}", e)}));
                        Err(e)
                    }
                }
            }
        }
    }

    fn check_registry(&self) -> windows::core::Result<bool> {
        unsafe {
            let mut key: HKEY = HKEY::default();

            // Open the Run registry key
            let result = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
                0,
                KEY_READ,
                &mut key,
            );

            if !result.is_ok() {
                log_error!("Failed to open registry key for read", serde_json::json!({"error": format!("{:?}", result)}));
                return Ok(false);
            }

            // Convert app name to wide string
            let app_name_wide: Vec<u16> = self
                .app_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let mut data_type = REG_NONE;
            let mut data_size = 0u32;

            // Check if the value exists
            let result = RegQueryValueExW(
                key,
                PCWSTR(app_name_wide.as_ptr()),
                None,
                Some(&mut data_type),
                None,
                Some(&mut data_size),
            );

            let _ = RegCloseKey(key);

            let is_installed = result.is_ok();

            Ok(is_installed)
        }
    }

    pub fn print_status(&self) {
        if self.is_installed() {
            log_info!("Autostart is enabled", serde_json::json!({
                "registry": "HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "key": self.app_name,
                "path": self.app_path.display().to_string()
            }));
        } else {
            log_info!("Autostart is disabled");
        }
    }

    /// Get detailed autostart information for diagnostics
    pub fn get_detailed_status(&self) -> AutoStartStatus {
        let current_exe = match env::current_exe() {
            Ok(path) => path,
            Err(e) => {
                log_error!("Failed to get current executable path", serde_json::json!({"error": format!("{}", e)}));
                std::path::PathBuf::new()
            }
        };

        let is_installed = self.is_installed();
        let paths_match = is_installed && (current_exe == self.app_path);
        let file_exists = self.app_path.exists();

        AutoStartStatus {
            is_installed,
            registry_path: self.app_path.clone(),
            current_exe,
            paths_match,
            file_exists,
            app_name: self.app_name.clone(),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct AutoStartStatus {
    pub is_installed: bool,
    pub registry_path: PathBuf,
    pub current_exe: PathBuf,
    pub paths_match: bool,
    pub file_exists: bool,
    pub app_name: String,
}
