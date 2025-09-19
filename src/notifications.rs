use std::time::{SystemTime, UNIX_EPOCH};
use windows::{
    core::{HSTRING, PCWSTR},
    Data::Xml::Dom,
    Win32::Foundation::HWND,
    Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED},
    Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MESSAGEBOX_STYLE,
    },
    UI::Notifications::{ToastNotification, ToastNotificationManager},
};
use crate::{log_info, log_error, log_warn, log_debug};

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
        unsafe {
            let title_wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
            let message_wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();

            let _ = MessageBoxW(
                HWND::default(),
                PCWSTR(message_wide.as_ptr()),
                PCWSTR(title_wide.as_ptr()),
                MB_OK | icon_type,
            );
        }
    }

    fn create_toast_notification(
        title: &str,
        message: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        unsafe {
            // Initialize COM for this thread
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            // Create toast notification manager
            let toast_manager = ToastNotificationManager::CreateToastNotifier()?;

            // Create XML template for the toast
            let xml_template = format!(
                r#"<toast>
                    <visual>
                        <binding template="ToastGeneric">
                            <text>{}</text>
                            <text>{}</text>
                        </binding>
                    </visual>
                    <audio silent="false" />
                </toast>"#,
                title, message
            );

            // Create XML document from template
            let xml_doc = Dom::XmlDocument::new()?;
            xml_doc.LoadXml(&HSTRING::from(xml_template))?;

            // Create toast notification from XML
            let toast = ToastNotification::CreateToastNotification(&xml_doc)?;

            // Show the toast
            toast_manager.Show(&toast)?;

            log_debug!("WinRT toast notification sent successfully", serde_json::json!({
                "title": title,
                "message": message
            }));
            Ok(())
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

    pub async fn send_temperature_alert(
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

        // Try to send Windows toast notification using tokio::task::spawn_blocking for COM/WinRT
        let toast_title = title.to_string();
        let toast_message = message.clone();

        match tokio::task::spawn_blocking(move || {
            Self::create_toast_notification(&toast_title, &toast_message)
        })
        .await
        {
            Ok(Ok(_)) => {
                log_info!("Toast notification sent", serde_json::json!({"message": message}));
            }
            Ok(Err(e)) => {
                log_error!("Failed to send WinRT toast notification, falling back", serde_json::json!({
                    "error": format!("{}", e),
                    "fallback": "message_box"
                }));

                // Always fallback to message box for alerts
                log_info!("Showing message box fallback");
                Self::show_message_box(title, &message, MB_ICONWARNING);
            }
            Err(e) => {
                log_error!("Tokio spawn_blocking error", serde_json::json!({"error": format!("{}", e)}));
                Self::show_message_box(title, &message, MB_ICONWARNING);
            }
        }

        // For debugging: log to console instead of showing modal dialogs that block tray
        #[cfg(debug_assertions)]
        {
            log_debug!("Would show GUI dialog - Temperature Alert", serde_json::json!({
                "message": message
            }));
        }

        self.cooldown_level += 1;
        Ok(())
    }

    pub async fn send_status_notification(
        &self,
        message: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log_info!("Status notification (async)", serde_json::json!({"message": message}));

        // Try to send Windows toast notification for status updates
        let toast_message = message.to_string();

        match tokio::task::spawn_blocking(move || {
            Self::create_toast_notification("GPU Temperature Monitor", &toast_message)
        })
        .await
        {
            Ok(Ok(_)) => {
                log_debug!("Status WinRT toast notification sent");
            }
            Ok(Err(e)) => {
                log_error!("Failed to send status WinRT toast notification", serde_json::json!({"error": format!("{}", e)}));

                // For startup notifications, show message box as well
                if message.contains("started") {
                    Self::show_message_box("GPU Temperature Monitor", message, MB_ICONINFORMATION);
                }
            }
            Err(e) => {
                log_error!("Tokio spawn_blocking error for status notification", serde_json::json!({
                    "error": format!("{}", e)
                }));
                if message.contains("started") {
                    Self::show_message_box("GPU Temperature Monitor", message, MB_ICONINFORMATION);
                }
            }
        }

        Ok(())
    }

    // Temporary sync wrapper for backward compatibility
    pub fn send_status_notification_sync(&self, message: &str) {
        log_info!("Status notification (async)", serde_json::json!({"message": message}));

        // For now, just log to console and use fallback message box for critical notifications
        if message.contains("started") || message.contains("Error") {
            Self::show_message_box("GPU Temperature Monitor", message, MB_ICONINFORMATION);
        }
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
