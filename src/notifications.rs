use std::time::{SystemTime, UNIX_EPOCH};
use windows::{
    core::PCWSTR,
    Win32::Foundation::HWND,
    Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MESSAGEBOX_STYLE,
    },
};
use crate::{log_info, log_error, log_warn};

pub struct NotificationManager {
    last_notification_time: Option<u64>,
    pub cooldown_level: u32,
    base_cooldown_sec: u64,
    max_cooldown_sec: u64,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            last_notification_time: None,
            cooldown_level: 0,
            base_cooldown_sec: 20,
            max_cooldown_sec: 320,
        }
    }

    fn show_message_box(title: &str, message: &str, icon_type: MESSAGEBOX_STYLE) {
        // Validate input strings to prevent potential issues
        if title.len() > 1024 || message.len() > 4096 {
            log_error!("Message box strings too long, truncating", serde_json::json!({
                "title_len": title.len(),
                "message_len": message.len()
            }));
            return;
        }

        unsafe {
            let title_wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
            let message_wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();

            // Validate that we have null-terminated strings
            if title_wide.is_empty() || message_wide.is_empty() {
                log_error!("Failed to create wide strings for message box");
                return;
            }

            let _ = MessageBoxW(
                HWND::default(),
                PCWSTR(message_wide.as_ptr()),
                PCWSTR(title_wide.as_ptr()),
                MB_OK | icon_type,
            );
        }
    }




    pub fn should_notify(&mut self, temp_exceeds_threshold: bool) -> bool {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if !temp_exceeds_threshold {
            // Temperature is normal, reset cooldown
            self.cooldown_level = 0;
            self.last_notification_time = None;
            return false;
        }

        // Check if we're in cooldown period
        if let Some(last_time) = self.last_notification_time {
            let cooldown_duration = self.calculate_cooldown_duration();
            if current_time - last_time < cooldown_duration {
                return false;
            }
        }

        // Time to notify
        self.last_notification_time = Some(current_time);
        true
    }

    fn calculate_cooldown_duration(&self) -> u64 {
        let cooldown = self.base_cooldown_sec * (2_u64.pow(self.cooldown_level));
        cooldown.min(self.max_cooldown_sec)
    }

    pub fn send_temperature_alert(
        &mut self,
        sensor_name: &str,
        temperature: f32,
        threshold: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let title = "GPU Temperature Alert!";
        let message = format!(
            "{}: {:.1}°C (Threshold: {:.1}°C)",
            sensor_name, temperature, threshold
        );

        // Console notification with visual alert
        log_warn!("TEMPERATURE ALERT", serde_json::json!({"message": message}));

        // Show MessageBox for temperature alerts
        log_info!("Showing temperature alert MessageBox", serde_json::json!({"message": message}));
        Self::show_message_box(title, &message, MB_ICONWARNING);

        // Always increase cooldown for MessageBox (blocking behavior)
        self.cooldown_level += 1;
        log_info!("Cooldown increased due to MessageBox", serde_json::json!({
            "cooldown_level": self.cooldown_level
        }));
        Ok(())
    }

    pub fn send_status_notification(
        &self,
        message: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log_info!("Status notification", serde_json::json!({"message": message}));

        // Show MessageBox only for errors, not for startup messages
        if message.contains("Error") || message.contains("Failed") {
            log_info!("Showing status MessageBox", serde_json::json!({"message": message}));
            Self::show_message_box("GPU Temperature Monitor", message, MB_ICONINFORMATION);
        } else {
            log_info!("Status notification logged only (not critical)", serde_json::json!({"message": message}));
        }

        Ok(())
    }

    // Sync wrapper for backward compatibility (now all are sync)
    pub fn send_status_notification_sync(&self, message: &str) {
        // Just delegate to the main function since everything is sync now
        let _ = self.send_status_notification(message);
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl NotificationManager {
    fn test_set_last_notification_time(&mut self, value: Option<u64>) {
        self.last_notification_time = value;
    }

    fn test_last_notification_time(&self) -> Option<u64> {
        self.last_notification_time
    }

    fn test_set_cooldown_level(&mut self, level: u32) {
        self.cooldown_level = level;
    }

    fn test_cooldown_level(&self) -> u32 {
        self.cooldown_level
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn cooldown_prevents_immediate_repeat() {
        let mut manager = NotificationManager::new();
        assert!(manager.should_notify(true));

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        manager.test_set_last_notification_time(Some(now));
        manager.test_set_cooldown_level(0);

        assert!(!manager.should_notify(true));

        manager.test_set_last_notification_time(Some(0));
        assert!(manager.should_notify(true));
    }

    #[test]
    fn reset_on_normal_temperature() {
        let mut manager = NotificationManager::new();
        manager.test_set_last_notification_time(Some(123));
        manager.test_set_cooldown_level(3);

        assert!(!manager.should_notify(false));
        assert_eq!(manager.test_cooldown_level(), 0);
        assert_eq!(manager.test_last_notification_time(), None);
    }
}
