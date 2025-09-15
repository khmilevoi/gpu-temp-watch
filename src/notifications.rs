use winrt_notification::{Duration, Sound, Toast};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct NotificationManager {
    last_notification_time: Option<u64>,
    cooldown_level: u32,
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
        let title = "🔥 GPU Temperature Alert";
        let message = format!(
            "{}: {:.1}°C (Threshold: {:.1}°C)",
            sensor_name, temperature, threshold
        );

        // Try to send Windows toast notification
        match self.send_toast_notification(title, &message) {
            Ok(_) => {
                println!("🔔 Toast notification sent: {}", message);
                self.cooldown_level += 1;
                Ok(())
            }
            Err(e) => {
                // Fallback to console notification
                println!("⚠️  TEMPERATURE ALERT: {}", message);
                println!("📱 Toast notification failed: {}", e);
                self.cooldown_level += 1;
                Ok(())
            }
        }
    }

    fn send_toast_notification(&self, title: &str, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        Toast::new(Toast::POWERSHELL_APP_ID)
            .title(title)
            .text1(message)
            .sound(Some(Sound::SMS))
            .duration(Duration::Short)
            .show()?;
        Ok(())
    }

    pub fn send_status_notification(&self, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        match self.send_toast_notification("GPU Temperature Monitor", message) {
            Ok(_) => println!("ℹ️  Status: {}", message),
            Err(_) => println!("ℹ️  Status: {}", message),
        }
        Ok(())
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}