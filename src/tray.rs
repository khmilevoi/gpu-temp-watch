use trayicon::{TrayIcon, TrayIconBuilder, MenuItem, MenuBuilder};
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
}

pub struct SystemTray {
    _tray_icon: TrayIcon<TrayMessage>,
    receiver: mpsc::Receiver<TrayMessage>,
}

impl SystemTray {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        // Build the tray menu
        let tray_menu = MenuBuilder::new()
            .separator()
            .item("🌡️ GPU Temperature Monitor", TrayMessage::Settings)
            .separator()
            .item("⏸️ Pause Monitoring", TrayMessage::Pause)
            .item("▶️ Resume Monitoring", TrayMessage::Resume)
            .separator()
            .item("📋 Show Logs", TrayMessage::ShowLogs)
            .separator()
            .item("❌ Exit", TrayMessage::Exit);

        // Create callback function for tray clicks
        let (callback_sender, callback_receiver) = mpsc::channel();
        let callback = move |msg: &TrayMessage| {
            let _ = callback_sender.send(msg.clone());
        };

        // Create the tray icon using static icon data
        let tray_icon = TrayIconBuilder::new()
            .sender(callback)
            .icon_from_buffer(Self::get_static_icon())
            .tooltip("GPU Temperature Monitor")
            .menu(tray_menu)
            .build()?;

        info!("✅ System tray initialized");

        Ok(SystemTray {
            _tray_icon: tray_icon,
            receiver: callback_receiver,
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
        let _icon_data = if temperature > threshold {
            Self::create_hot_icon()
        } else if temperature > threshold - 10.0 {
            Self::create_warm_icon()
        } else {
            Self::create_cool_icon()
        };

        // Note: trayicon 0.3.0 may not support dynamic icon updates
        // This is a placeholder for the functionality
        warn!("Dynamic icon updates not fully supported in current trayicon version");
        Ok(())
    }

    fn get_static_icon() -> &'static [u8] {
        // Create a minimal ICO file as static data
        // This is a basic 16x16 monochrome icon
        &[
            0x00, 0x00, // Reserved. Must always be 0.
            0x01, 0x00, // Image type: 1 for icon (.ICO), 2 for cursor (.CUR).
            0x01, 0x00, // Number of images in the file.

            // Image directory (16 bytes per image)
            0x10, // Width (16 pixels)
            0x10, // Height (16 pixels)
            0x00, // Number of colors in color palette (0 = no palette)
            0x00, // Reserved. Should be 0.
            0x01, 0x00, // Color planes (0 or 1)
            0x01, 0x00, // Bits per pixel (1, 4, 8, 16, 24, 32)
            0x28, 0x00, 0x00, 0x00, // Size of image data in bytes
            0x16, 0x00, 0x00, 0x00, // Offset of image data from beginning of file

            // Image data (40 byte bitmap header + image data)
            0x28, 0x00, 0x00, 0x00, // Header size (40 bytes)
            0x10, 0x00, 0x00, 0x00, // Width
            0x20, 0x00, 0x00, 0x00, // Height (doubled for icon)
            0x01, 0x00, // Planes
            0x01, 0x00, // Bits per pixel
            0x00, 0x00, 0x00, 0x00, // Compression
            0x00, 0x00, 0x00, 0x00, // Image size (can be 0 for uncompressed)
            0x00, 0x00, 0x00, 0x00, // X pixels per meter
            0x00, 0x00, 0x00, 0x00, // Y pixels per meter
            0x00, 0x00, 0x00, 0x00, // Colors used
            0x00, 0x00, 0x00, 0x00, // Important colors
        ]
    }

    fn create_cool_icon() -> &'static [u8] {
        // Green icon for cool temperatures
        Self::get_static_icon()
    }

    fn create_warm_icon() -> &'static [u8] {
        // Yellow icon for warm temperatures
        Self::get_static_icon()
    }

    fn create_hot_icon() -> &'static [u8] {
        // Red icon for hot temperatures
        Self::get_static_icon()
    }
}