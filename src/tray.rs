use tray_icon::{TrayIcon, TrayIconBuilder, menu::{Menu, MenuItem, MenuEvent, PredefinedMenuItem}};
use tray_icon::Icon;
use std::error::Error;
use std::sync::mpsc;
use log::{info, warn};
use image::RgbaImage;

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
    OpenWebInterface,
}

pub struct SystemTray {
    tray_icon: TrayIcon,
    receiver: mpsc::Receiver<TrayMessage>,
    current_icon_state: IconState,
}

#[derive(Debug, Clone, PartialEq)]
enum IconState {
    Cool,
    Warm,
    Hot,
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
                "⚙️ Settings" => Some(TrayMessage::OpenWebInterface),
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

        // Set up tray icon click handler for double-click
        let click_sender = sender.clone();
        use tray_icon::TrayIconEvent;
        TrayIconEvent::set_event_handler(Some(Box::new(move |event: TrayIconEvent| {
            match event {
                TrayIconEvent::DoubleClick { .. } => {
                    println!("🖱️ Double click detected on tray icon");
                    if let Err(e) = click_sender.send(TrayMessage::OpenWebInterface) {
                        eprintln!("❌ Failed to send double-click message: {:?}", e);
                    }
                }
                _ => {}
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

        // Load initial icon (cool state)
        let icon = Self::load_icon_for_state(&IconState::Cool)?;

        // Create tray icon
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("GPU Temperature Monitor - Right click for menu, double click for settings")
            .with_icon(icon)
            .build()?;

        println!("🔧 Tray icon created with proper event handler");

        info!("✅ System tray initialized with non-blocking event handler");

        Ok(SystemTray {
            tray_icon,
            receiver,
            current_icon_state: IconState::Cool,
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
        let new_state = if temperature > threshold {
            IconState::Hot
        } else if temperature > threshold - 10.0 {
            IconState::Warm
        } else {
            IconState::Cool
        };

        // Only update icon if state changed
        if new_state != self.current_icon_state {
            let icon = Self::load_icon_for_state(&new_state)?;
            self.tray_icon.set_icon(Some(icon))?;
            self.current_icon_state = new_state.clone();

            let state_str = match new_state {
                IconState::Hot => "🔴 HOT",
                IconState::Warm => "🟡 WARM",
                IconState::Cool => "🟢 COOL",
            };

            println!("🌡️ Temperature: {:.1}°C - State: {} (icon updated)", temperature, state_str);
            info!("Tray icon updated to: {:?}", new_state);
        }

        Ok(())
    }

    fn load_icon_for_state(state: &IconState) -> Result<Icon, Box<dyn Error>> {
        let icon_filename = match state {
            IconState::Cool => "icons/thermometer-cool.ico",
            IconState::Warm => "icons/thermometer-warm.ico",
            IconState::Hot => "icons/thermometer-hot.ico",
        };

        // Try to load the specific temperature icon
        if std::path::Path::new(icon_filename).exists() {
            match Icon::from_path(icon_filename, None) {
                Ok(icon) => {
                    info!("✅ Loaded {} icon", icon_filename);
                    return Ok(icon);
                }
                Err(e) => {
                    warn!("⚠️ Failed to load {}: {}", icon_filename, e);
                }
            }
        }

        // Fallback to general icon.ico
        if std::path::Path::new("icons/icon.ico").exists() {
            match Icon::from_path("icons/icon.ico", None) {
                Ok(icon) => {
                    info!("✅ Loaded fallback icons/icon.ico");
                    return Ok(icon);
                }
                Err(e) => {
                    warn!("⚠️ Failed to load icons/icon.ico: {}", e);
                }
            }
        }

        // Create programmatic icon as last resort
        warn!("⚠️ No icon files found, creating programmatic icon");
        let rgba_image = Self::create_simple_rgba_image_for_state(state);
        let icon = Icon::from_rgba(rgba_image.as_raw().clone(), rgba_image.width(), rgba_image.height())?;
        Ok(icon)
    }


    fn create_simple_rgba_image_for_state(state: &IconState) -> RgbaImage {
        let mut img = RgbaImage::new(16, 16);

        // Choose color based on state
        let fill_color = match state {
            IconState::Cool => [0, 255, 0, 255],   // Green
            IconState::Warm => [255, 165, 0, 255], // Orange
            IconState::Hot => [255, 0, 0, 255],    // Red
        };

        // Create a simple thermometer pattern
        for y in 0..16 {
            for x in 0..16 {
                let pixel = if x == 8 && y < 12 {
                    // Vertical line (thermometer tube) - state color
                    fill_color
                } else if (x == 7 || x == 9) && y < 12 {
                    // Thermometer outline - black
                    [0, 0, 0, 255]
                } else if (6..=10).contains(&x) && (12..=14).contains(&y) {
                    // Thermometer bulb - state color
                    fill_color
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

}