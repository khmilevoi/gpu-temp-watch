use tracing::{info, warn};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows::{
    Win32::UI::Shell::ShellExecuteW,
    Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_ICONERROR, MB_ICONINFORMATION, MB_ICONQUESTION, MB_ICONWARNING,
        MB_OK, MB_YESNO, MESSAGEBOX_RESULT, MESSAGEBOX_STYLE, SW_SHOWNORMAL
    },
    Win32::Foundation::HWND,
    core::PCWSTR,
};

pub struct GuiDialogs;

impl GuiDialogs {
    /// Show an information message box
    pub fn show_info(title: &str, message: &str) {
        #[cfg(windows)]
        Self::show_message_box(title, message, MB_ICONINFORMATION);

        #[cfg(not(windows))]
        {
            println!("INFO: {}: {}", title, message);
        }
    }

    /// Show a warning message box
    pub fn show_warning(title: &str, message: &str) {
        #[cfg(windows)]
        Self::show_message_box(title, message, MB_ICONWARNING);

        #[cfg(not(windows))]
        {
            println!("WARNING: {}: {}", title, message);
        }
    }

    /// Show an error message box
    pub fn show_error(title: &str, message: &str) {
        #[cfg(windows)]
        Self::show_message_box(title, message, MB_ICONERROR);

        #[cfg(not(windows))]
        {
            println!("ERROR: {}: {}", title, message);
        }
    }

    /// Show a yes/no question dialog
    pub fn show_question(title: &str, message: &str) -> bool {
        #[cfg(windows)]
        {
            let result =
                Self::show_message_box_with_result(title, message, MB_ICONQUESTION | MB_YESNO);
            result.0 == IDYES.0
        }

        #[cfg(not(windows))]
        {
            println!("QUESTION: {}: {} (assuming YES)", title, message);
            true
        }
    }

    /// Show a simple input dialog for text input
    pub fn show_input_dialog(_title: &str, prompt: &str, default_value: &str) -> Option<String> {
        // For now, we'll use a simple approach with multiple input boxes
        // In a full implementation, you might want to create a custom dialog
        Self::show_info(
            "Input Required",
            &format!(
                "{}\n\nDefault: {}\n\nPlease use the settings file to change this value for now.",
                prompt, default_value
            ),
        );
        None // Placeholder - returning None means user cancelled
    }

    /// Open a file in the default application
    pub fn open_file(file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(windows)]
        {
            let file_path_wide = Self::to_wide_string(file_path);
            let operation = Self::to_wide_string("open");

            unsafe {
                let result = ShellExecuteW(
                    HWND::default(),
                    PCWSTR(operation.as_ptr()),
                    PCWSTR(file_path_wide.as_ptr()),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                );

                if result.0 as isize <= 32 {
                    return Err(format!("Failed to open file: {}", file_path).into());
                }
            }
        }

        #[cfg(not(windows))]
        {
            std::process::Command::new("xdg-open")
                .arg(file_path)
                .spawn()?;
        }

        Ok(())
    }

    /// Open a folder in file explorer
    pub fn open_folder(folder_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(windows)]
        {
            let folder_path_wide = Self::to_wide_string(folder_path);
            let operation = Self::to_wide_string("explore");

            unsafe {
                let result = ShellExecuteW(
                    HWND::default(),
                    PCWSTR(operation.as_ptr()),
                    PCWSTR(folder_path_wide.as_ptr()),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                );

                if result.0 as isize <= 32 {
                    return Err(format!("Failed to open folder: {}", folder_path).into());
                }
            }
        }

        #[cfg(not(windows))]
        {
            std::process::Command::new("xdg-open")
                .arg(folder_path)
                .spawn()?;
        }

        Ok(())
    }

    /// Show an about dialog with application information
    pub fn show_about() {
        let about_text = format!(
            "GPU Temperature Monitor v0.1.0\n\n\
            A lightweight GPU temperature monitoring tool.\n\n\
            Features:\n\
            • Real-time NVIDIA GPU temperature monitoring\n\
            • System tray integration\n\
            • Windows toast notifications\n\
            • Automatic startup support\n\
            • Smart notification cooldown\n\n\
            Built with Rust 🦀\n\
            Created with Claude Code"
        );

        Self::show_info("About GPU Temperature Monitor", &about_text);
    }

    /// Show settings dialog (simplified version)
    pub fn show_settings_info(current_threshold: f32, current_interval: u64) {
        let settings_text = format!(
            "⚙️ GPU Temperature Monitor Settings\n\n\
            📊 Current Configuration:\n\
            • 🌡️ Temperature Threshold: {:.1}°C\n\
            • ⏱️ Poll Interval: {} seconds\n\
            • 📂 Config File: ./config.json\n\
            • 📋 Log File: ./Logs/GpuTempWatch.log\n\n\
            🔧 How to Change Settings:\n\
            1. Click 'Open Config File' from the tray menu\n\
            2. Edit the values in config.json\n\
            3. Save the file and restart the application\n\n\
            📝 Available Settings:\n\
            • temperature_threshold_c: Alert temperature (°C)\n\
            • poll_interval_sec: Check interval (5-3600s)\n\
            • base_cooldown_sec: Notification cooldown (1-600s)\n\
            • enable_logging: Enable/disable file logging\n\
            • log_file_path: Log file location",
            current_threshold, current_interval
        );

        Self::show_info("Settings", &settings_text);
    }

    #[cfg(windows)]
    fn show_message_box(title: &str, message: &str, icon_type: MESSAGEBOX_STYLE) {
        let _ = Self::show_message_box_with_result(title, message, icon_type);
    }

    #[cfg(windows)]
    fn show_message_box_with_result(title: &str, message: &str, icon_type: MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT {
        unsafe {
            let title_wide = Self::to_wide_string(title);
            let message_wide = Self::to_wide_string(message);

            MessageBoxW(
                HWND::default(),
                PCWSTR(message_wide.as_ptr()),
                PCWSTR(title_wide.as_ptr()),
                MB_OK | icon_type,
            )
        }
    }

    #[cfg(windows)]
    fn to_wide_string(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
}

/// Helper struct for managing GUI state and operations
pub struct GuiManager {
    pub last_temperature: f32,
    pub monitoring_paused: bool,
    pub autostart_enabled: bool,
}

impl GuiManager {
    pub fn new() -> Self {
        Self {
            last_temperature: 0.0,
            monitoring_paused: false,
            autostart_enabled: false,
        }
    }

    pub fn update_temperature(&mut self, temperature: f32) {
        self.last_temperature = temperature;
    }

    pub fn set_monitoring_paused(&mut self, paused: bool) {
        self.monitoring_paused = paused;
    }

    pub fn set_autostart_enabled(&mut self, enabled: bool) {
        self.autostart_enabled = enabled;
    }

    pub fn get_status_tooltip(&self) -> String {
        let status = if self.monitoring_paused {
            "PAUSED"
        } else {
            "ACTIVE"
        };
        let autostart = if self.autostart_enabled { "ON" } else { "OFF" };

        format!(
            "GPU Temperature Monitor\n\
            Status: {}\n\
            Temperature: {:.1}°C\n\
            Autostart: {}",
            status, self.last_temperature, autostart
        )
    }

    pub fn handle_operation_result(&self, operation: &str, success: bool, error_msg: Option<&str>) {
        if success {
            GuiDialogs::show_info(
                "Operation Successful",
                &format!("✅ {} completed successfully", operation),
            );
            info!("GUI operation successful: {}", operation);
        } else {
            let error = error_msg.unwrap_or("Unknown error");
            GuiDialogs::show_error(
                "Operation Failed",
                &format!("❌ {} failed:\n\n{}", operation, error),
            );
            warn!("GUI operation failed: {}: {}", operation, error);
        }
    }
}

impl Default for GuiManager {
    fn default() -> Self {
        Self::new()
    }
}
