use tracing::info;
use std::env;
use std::path::PathBuf;
use windows::{core::*, Win32::Foundation::*, Win32::System::Registry::*};

pub struct AutoStart {
    app_name: String,
    app_path: PathBuf,
}

impl AutoStart {
    pub fn new() -> windows::core::Result<Self> {
        let app_name = "GpuTempWatch".to_string();
        let app_path = env::current_exe().map_err(|_e| windows::core::Error::from_win32())?;

        Ok(AutoStart { app_name, app_path })
    }

    pub fn install(&self) -> windows::core::Result<()> {
        self.add_to_registry()?;
        info!("✅ Autostart installed successfully");
        Ok(())
    }

    pub fn uninstall(&self) -> windows::core::Result<()> {
        self.remove_from_registry()?;
        info!("✅ Autostart removed successfully");
        Ok(())
    }

    pub fn is_installed(&self) -> bool {
        self.check_registry().unwrap_or(false)
    }

    fn add_to_registry(&self) -> windows::core::Result<()> {
        unsafe {
            let mut key: HKEY = HKEY::default();

            // Open the Run registry key
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
                0,
                KEY_WRITE,
                &mut key,
            )
            .ok()?;

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
            result.ok()
        }
    }

    fn remove_from_registry(&self) -> windows::core::Result<()> {
        unsafe {
            let mut key: HKEY = HKEY::default();

            // Open the Run registry key
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
                0,
                KEY_WRITE,
                &mut key,
            )
            .ok()?;

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
                result.ok()
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

            Ok(result.is_ok())
        }
    }

    pub fn print_status(&self) {
        if self.is_installed() {
            println!("✅ Autostart is enabled");
            println!(
                "   Registry: HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Run"
            );
            println!("   Key: {}", self.app_name);
            println!("   Path: {}", self.app_path.display());
        } else {
            println!("❌ Autostart is disabled");
        }
    }
}
