use trayicon::{TrayIcon, TrayIconBuilder, MenuBuilder};
use std::sync::mpsc;
use std::error::Error;
use log::{info, error, warn};

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
    _tray_icon: TrayIcon<TrayMessage>,
    receiver: mpsc::Receiver<TrayMessage>,
    _icon_data: Vec<u8>, // Store icon data to ensure it lives long enough
}

impl SystemTray {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        // Build the tray menu with explicit activation
        let tray_menu = MenuBuilder::new()
            .item("🌡️ GPU Temperature Monitor", TrayMessage::About)
            .separator()
            .item("⏸️ Pause Monitoring", TrayMessage::Pause)
            .item("▶️ Resume Monitoring", TrayMessage::Resume)
            .separator()
            .item("⚙️ Settings", TrayMessage::Settings)
            .item("📋 Show Logs", TrayMessage::ShowLogs)
            .item("📂 Open Config File", TrayMessage::OpenConfig)
            .item("📁 Open Logs Folder", TrayMessage::OpenLogsFolder)
            .separator()
            .item("🔧 Install Autostart", TrayMessage::InstallAutostart)
            .item("🗑️ Remove Autostart", TrayMessage::UninstallAutostart)
            .separator()
            .item("❌ Exit", TrayMessage::Exit);

        // Create callback function for tray clicks
        let (callback_sender, callback_receiver) = mpsc::channel();
        let callback = move |msg: &TrayMessage| {
            println!("🖱️ Tray menu item clicked: {:?}", msg);
            if let Err(e) = callback_sender.send(msg.clone()) {
                eprintln!("❌ Failed to send tray message: {:?}", e);
            }
        };

        // Try to load the thermometer icon from icons folder
        let icon_data = if std::path::Path::new("icons/thermometer.ico").exists() {
            match std::fs::read("icons/thermometer.ico") {
                Ok(data) => {
                    info!("✅ Loaded icons/thermometer.ico file");
                    data
                }
                Err(e) => {
                    warn!("⚠️ Failed to read icons/thermometer.ico: {}, using embedded icon", e);
                    Self::create_minimal_icon().to_vec()
                }
            }
        } else if std::path::Path::new("icons/icon.ico").exists() {
            match std::fs::read("icons/icon.ico") {
                Ok(data) => {
                    info!("✅ Loaded icons/icon.ico file");
                    data
                }
                Err(e) => {
                    warn!("⚠️ Failed to read icons/icon.ico: {}, using embedded icon", e);
                    Self::create_minimal_icon().to_vec()
                }
            }
        } else {
            info!("ℹ️ No external icon file found in icons/ folder, using embedded icon");
            Self::create_minimal_icon().to_vec()
        };

        // Store icon data in a static location to satisfy lifetime requirements
        let icon_bytes: &'static [u8] = Box::leak(icon_data.into_boxed_slice());

        // Create the tray icon with explicit settings
        println!("🔧 Creating tray icon with menu...");
        let tray_icon = TrayIconBuilder::new()
            .sender(callback)
            .icon_from_buffer(icon_bytes)
            .tooltip("GPU Temperature Monitor - Right click for menu")
            .menu(tray_menu)
            .build()?;

        println!("🔧 Tray icon created successfully");

        info!("✅ System tray initialized");

        Ok(SystemTray {
            _tray_icon: tray_icon,
            receiver: callback_receiver,
            _icon_data: vec![], // We don't need to store it since it's leaked
        })
    }

    pub fn get_message(&self) -> Option<TrayMessage> {
        match self.receiver.try_recv() {
            Ok(message) => Some(message),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                error!("❌ Tray message channel disconnected");
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

        println!("🌡️  Temperature: {:.1}°C - State: {} (using {})", temperature, state, icon_filename);

        // Note: trayicon 0.3.0 may not support dynamic icon updates during runtime
        // We would need to recreate the tray icon to change it, which is complex
        // For now, we just log the desired state
        info!("Icon should be: {}", icon_filename);
        Ok(())
    }

    fn create_minimal_icon() -> &'static [u8] {
        // Very simple 16x16 ICO data - just a few bytes for testing
        static MINIMAL_ICON: &[u8] = &[
            0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x10, 0x10, 0x00, 0x00, 0x01, 0x00,
            0x20, 0x00, 0x68, 0x04, 0x00, 0x00, 0x16, 0x00, 0x00, 0x00
        ];
        MINIMAL_ICON
    }

    fn create_cool_icon() -> &'static [u8] {
        // Green icon for cool temperatures
        Self::create_minimal_icon()
    }

    fn create_warm_icon() -> &'static [u8] {
        // Yellow icon for warm temperatures
        Self::create_minimal_icon()
    }

    fn create_hot_icon() -> &'static [u8] {
        // Red icon for hot temperatures
        Self::create_minimal_icon()
    }
}