use tray_icon::{TrayIcon, TrayIconBuilder, menu::{Menu, MenuItem, MenuEvent, PredefinedMenuItem}};
use tray_icon::Icon;
use std::error::Error;
use std::sync::mpsc;
use log::{info, warn};
use image::{ImageFormat, RgbaImage};

#[derive(Debug, Clone, PartialEq)]
pub enum TrayMessage {
    Exit,
    Pause,
    Resume,
    Settings,
    ShowLogs,
    InstallAutostart,
    UninstallAutostart,
    About,
    OpenConfig,
    OpenLogsFolder,
}

pub struct SystemTray {
    _tray_icon: TrayIcon,
    _icon_data: Vec<u8>,
    receiver: mpsc::Receiver<TrayMessage>,
}

impl SystemTray {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        println!("🔧 Creating new tray with proper event handler architecture...");

        // Create channel for tray events
        let (sender, receiver) = mpsc::channel::<TrayMessage>();

        // Set up menu event handler using the recommended approach
        let event_sender = sender.clone();
        MenuEvent::set_event_handler(Some(Box::new(move |event: MenuEvent| {
            println!("🖱️ Menu event received: {:?}", event);

            // Convert menu event to TrayMessage without blocking operations
            let tray_message = match event.id().0.as_str() {
                "🌡️ GPU Temperature Monitor" => Some(TrayMessage::About),
                "⏸️ Pause Monitoring" => Some(TrayMessage::Pause),
                "▶️ Resume Monitoring" => Some(TrayMessage::Resume),
                "⚙️ Settings" => Some(TrayMessage::Settings),
                "📋 Show Logs" => Some(TrayMessage::ShowLogs),
                "📂 Open Config File" => Some(TrayMessage::OpenConfig),
                "📁 Open Logs Folder" => Some(TrayMessage::OpenLogsFolder),
                "🔧 Install Autostart" => Some(TrayMessage::InstallAutostart),
                "🗑️ Remove Autostart" => Some(TrayMessage::UninstallAutostart),
                "❌ Exit" => Some(TrayMessage::Exit),
                _ => {
                    println!("🤷 Unknown menu item: {}", event.id().0);
                    None
                }
            };

            // Send message to main thread without any blocking operations
            if let Some(msg) = tray_message {
                if let Err(e) = event_sender.send(msg) {
                    eprintln!("❌ Failed to send tray message: {:?}", e);
                }
            }
        })));

        // Create menu
        let menu = Menu::new();

        let about_item = MenuItem::new("🌡️ GPU Temperature Monitor", true, None);
        let separator1 = PredefinedMenuItem::separator();
        let pause_item = MenuItem::new("⏸️ Pause Monitoring", true, None);
        let resume_item = MenuItem::new("▶️ Resume Monitoring", true, None);
        let separator2 = PredefinedMenuItem::separator();
        let settings_item = MenuItem::new("⚙️ Settings", true, None);
        let logs_item = MenuItem::new("📋 Show Logs", true, None);
        let config_item = MenuItem::new("📂 Open Config File", true, None);
        let logs_folder_item = MenuItem::new("📁 Open Logs Folder", true, None);
        let separator3 = PredefinedMenuItem::separator();
        let autostart_install_item = MenuItem::new("🔧 Install Autostart", true, None);
        let autostart_remove_item = MenuItem::new("🗑️ Remove Autostart", true, None);
        let separator4 = PredefinedMenuItem::separator();
        let exit_item = MenuItem::new("❌ Exit", true, None);

        menu.append_items(&[
            &about_item,
            &separator1,
            &pause_item,
            &resume_item,
            &separator2,
            &settings_item,
            &logs_item,
            &config_item,
            &logs_folder_item,
            &separator3,
            &autostart_install_item,
            &autostart_remove_item,
            &separator4,
            &exit_item,
        ])?;

        // Load icon
        let icon_data = Self::load_icon_data();
        let icon = Self::create_icon_from_data(&icon_data)?;

        // Create tray icon
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("GPU Temperature Monitor - Right click for menu")
            .with_icon(icon)
            .build()?;

        println!("🔧 Tray icon created with proper event handler");

        info!("✅ System tray initialized with non-blocking event handler");

        Ok(SystemTray {
            _tray_icon: tray_icon,
            _icon_data: icon_data,
            receiver,
        })
    }

    pub fn get_message(&self) -> Option<TrayMessage> {
        // Use our own channel instead of MenuEvent::receiver()
        match self.receiver.try_recv() {
            Ok(message) => {
                println!("📬 Received tray message: {:?}", message);
                Some(message)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                eprintln!("❌ Tray message channel disconnected");
                None
            }
        }
    }

    pub fn update_icon_for_temperature(&mut self, temperature: f32, threshold: f32) -> Result<(), Box<dyn Error>> {
        // Determine which icon to use based on temperature
        let icon_filename = if temperature > threshold {
            "icons/thermometer-hot.ico"
        } else if temperature > threshold - 10.0 {
            "icons/thermometer-warm.ico"
        } else {
            "icons/thermometer-cool.ico"
        };

        // Log the temperature state
        let state = if temperature > threshold {
            "🔴 HOT"
        } else if temperature > threshold - 10.0 {
            "🟡 WARM"
        } else {
            "🟢 COOL"
        };

        println!("🌡️  Temperature: {:.1}°C - State: {} (should use {})", temperature, state, icon_filename);

        // TODO: Implement dynamic icon changing with tray-icon crate
        // For now, just log the desired state
        info!("Icon should be: {}", icon_filename);
        Ok(())
    }

    fn load_icon_data() -> Vec<u8> {
        // Try to load the thermometer icon from icons folder
        if std::path::Path::new("icons/thermometer.ico").exists() {
            match std::fs::read("icons/thermometer.ico") {
                Ok(data) => {
                    info!("✅ Loaded icons/thermometer.ico file");
                    return data;
                }
                Err(e) => {
                    warn!("⚠️ Failed to read icons/thermometer.ico: {}, using embedded icon", e);
                }
            }
        }

        if std::path::Path::new("icons/icon.ico").exists() {
            match std::fs::read("icons/icon.ico") {
                Ok(data) => {
                    info!("✅ Loaded icons/icon.ico file");
                    return data;
                }
                Err(e) => {
                    warn!("⚠️ Failed to read icons/icon.ico: {}, using embedded icon", e);
                }
            }
        }

        info!("ℹ️ No external icon file found in icons/ folder, creating simple icon");
        Self::create_simple_icon_data()
    }

    fn create_icon_from_data(data: &[u8]) -> Result<Icon, Box<dyn Error>> {
        // Try to load as ICO by writing to temp file first
        let temp_path = std::env::temp_dir().join("gpu_temp_icon.ico");
        std::fs::write(&temp_path, data)?;

        if let Ok(icon) = Icon::from_path(&temp_path, None) {
            let _ = std::fs::remove_file(&temp_path); // Clean up
            return Ok(icon);
        }

        // If ICO fails, create a simple programmatic icon
        warn!("⚠️ Failed to load icon from bytes, creating simple programmatic icon");
        let rgba_image = Self::create_simple_rgba_image();
        let icon = Icon::from_rgba(rgba_image.as_raw().clone(), rgba_image.width(), rgba_image.height())?;
        Ok(icon)
    }

    fn create_simple_rgba_image() -> RgbaImage {
        let mut img = RgbaImage::new(16, 16);

        // Create a simple thermometer pattern
        for y in 0..16 {
            for x in 0..16 {
                let pixel = if x == 8 && y < 12 {
                    // Vertical line (thermometer tube) - red
                    [255, 0, 0, 255]
                } else if (x == 7 || x == 9) && y < 12 {
                    // Thermometer outline - black
                    [0, 0, 0, 255]
                } else if (6..=10).contains(&x) && (12..=14).contains(&y) {
                    // Thermometer bulb - red
                    [255, 0, 0, 255]
                } else if (5..=11).contains(&x) && (11..=15).contains(&y) {
                    // Bulb outline - black
                    [0, 0, 0, 255]
                } else {
                    // Transparent background
                    [0, 0, 0, 0]
                };

                img.put_pixel(x, y, image::Rgba(pixel));
            }
        }

        img
    }

    fn create_simple_icon_data() -> Vec<u8> {
        let img = Self::create_simple_rgba_image();
        let mut buffer = Vec::new();

        // Convert to PNG format
        if let Err(e) = img.write_to(&mut std::io::Cursor::new(&mut buffer), ImageFormat::Png) {
            warn!("Failed to create PNG icon data: {}", e);
            // Return minimal data as fallback
            return vec![0x89, 0x50, 0x4E, 0x47]; // PNG header
        }

        buffer
    }
}