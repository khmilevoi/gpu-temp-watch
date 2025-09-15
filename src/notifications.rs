use std::time::{SystemTime, UNIX_EPOCH};
use winrt_notification::{Toast, Duration as ToastDuration};
use log::{info, warn};

#[cfg(windows)]
use winapi::um::winuser::{MessageBoxW, MB_OK, MB_ICONWARNING, MB_ICONINFORMATION};
#[cfg(windows)]
use winapi::shared::ntdef::NULL;

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

    #[cfg(windows)]
    fn show_message_box(title: &str, message: &str, icon_type: u32) {
        unsafe {
            let title_wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
            let message_wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();

            MessageBoxW(
                NULL as *mut _,
                message_wide.as_ptr(),
                title_wide.as_ptr(),
                MB_OK | icon_type,
            );
        }
    }

    #[cfg(not(windows))]
    fn show_message_box(_title: &str, _message: &str, _icon_type: u32) {
        // No-op for non-Windows platforms
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

    pub fn send_temperature_alert(&mut self, sensor_name: &str, temperature: f32, threshold: f32) -> Result<(), Box<dyn std::error::Error>> {
        let title = "GPU Temperature Alert!";
        let message = format!(
            "{}: {:.1}°C (Threshold: {:.1}°C)",
            sensor_name, temperature, threshold
        );

        // Console notification with visual alert
        println!("🔥🔥🔥 TEMPERATURE ALERT 🔥🔥🔥");
        println!("⚠️  {}", message);
        println!("🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥");

        // Try to send Windows toast notification
        match Toast::new("GPU Temperature Monitor")
            .title(title)
            .text1(&message)
            .sound(None)
            .duration(ToastDuration::Short)
            .show()
        {
            Ok(_) => {
                info!("✅ Toast notification sent successfully");
                println!("📱 Toast notification: {}", message);
            }
            Err(e) => {
                warn!("⚠️  Failed to send toast notification: {}", e);
                println!("❌ Toast failed: {}", e);
                warn!("🔄 Falling back to message box notification");

                // Always fallback to message box for alerts
                #[cfg(windows)]
                {
                    println!("💬 Showing message box instead");
                    Self::show_message_box(title, &message, MB_ICONWARNING);
                }
            }
        }

        // For debugging: log to console instead of showing modal dialogs that block tray
        #[cfg(debug_assertions)]
        {
            println!("🔔 DEBUG: Would show GUI dialog - Temperature Alert: {}", message);
        }

        self.cooldown_level += 1;
        Ok(())
    }

    pub fn send_status_notification(&self, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        println!("ℹ️  Status: {}", message);

        // Try to send Windows toast notification for status updates
        match Toast::new("GPU Temperature Monitor")
            .title("Status Update")
            .text1(message)
            .sound(None)
            .duration(ToastDuration::Short)
            .show()
        {
            Ok(_) => {
                info!("✅ Status toast notification sent");
            }
            Err(e) => {
                warn!("⚠️  Failed to send status toast notification: {}", e);

                // For startup notifications, show message box as well
                if message.contains("started") {
                    #[cfg(windows)]
                    Self::show_message_box("GPU Temperature Monitor", message, MB_ICONINFORMATION);
                }
            }
        }

        Ok(())
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}