use std::error::Error;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;
use tracing::{error, info, warn};
use tray_icon::Icon;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
    MouseButton, TrayIconBuilder, TrayIconEvent,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
};

use image::RgbaImage;

#[derive(Debug, Clone, PartialEq)]
pub enum TrayMessage {
    OpenDashboard,
    ViewLogs,
    EditSettings,
    QuitMonitor,
}

fn menu_id_to_message(id: &str) -> Option<TrayMessage> {
    match id {
        "open_dashboard" => Some(TrayMessage::OpenDashboard),
        "view_logs" => Some(TrayMessage::ViewLogs),
        "edit_settings" => Some(TrayMessage::EditSettings),
        "quit_monitor" => Some(TrayMessage::QuitMonitor),
        _ => None,
    }
}

#[derive(Debug, Clone)]
enum TrayCommand {
    UpdateIcon(IconState),
    Shutdown,
}

#[derive(Debug, Clone)]
struct TrayThreadInitError(String);

impl std::fmt::Display for TrayThreadInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for TrayThreadInitError {}

pub struct SystemTray {
    command_sender: Sender<TrayCommand>,
    receiver: mpsc::Receiver<TrayMessage>,
    current_icon_state: IconState,
    thread_handle: Option<JoinHandle<()>>,
    last_message: Option<(TrayMessage, std::time::Instant)>,
}

#[derive(Debug, Clone, PartialEq)]
enum IconState {
    Cool,
    Warm,
    Hot,
}

impl SystemTray {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        println!("🔧 Creating new tray with dedicated event loop thread...");

        let (event_sender, event_receiver) = mpsc::channel::<TrayMessage>();
        let (command_sender, command_receiver) = mpsc::channel::<TrayCommand>();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), TrayThreadInitError>>();

        let thread_event_sender = event_sender.clone();
        let tray_thread = std::thread::Builder::new()
            .name("gpu-temp-tray".to_string())
            .spawn(move || {
                if let Err(err) =
                    run_tray_event_loop(command_receiver, thread_event_sender, ready_tx)
                {
                    error!("Tray thread terminated: {}", err);
                }
            })?;

        match ready_rx.recv() {
            Ok(Ok(())) => {
                println!("✅ Tray thread initialized");
            }
            Ok(Err(err)) => {
                return Err(Box::new(err));
            }
            Err(err) => {
                return Err(Box::new(err));
            }
        }

        Ok(SystemTray {
            command_sender,
            receiver: event_receiver,
            current_icon_state: IconState::Cool,
            thread_handle: Some(tray_thread),
            last_message: None,
        })
    }

    pub fn get_message(&mut self) -> Option<TrayMessage> {
        match self.receiver.try_recv() {
            Ok(message) => {
                let now = std::time::Instant::now();

                // Check for duplicate message within 500ms
                if let Some((last_msg, last_time)) = &self.last_message {
                    if *last_msg == message && now.duration_since(*last_time).as_millis() < 500 {
                        // Skip duplicate message
                        return None;
                    }
                }

                self.last_message = Some((message.clone(), now));
                println!("📬 Received tray message: {:?}", message);
                Some(message)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                warn!("❌ Tray message channel disconnected");
                None
            }
        }
    }

    pub fn update_icon_for_temperature(
        &mut self,
        temperature: f32,
        threshold: f32,
    ) -> Result<(), Box<dyn Error>> {
        let new_state = if temperature > threshold {
            IconState::Hot
        } else if temperature > threshold - 10.0 {
            IconState::Warm
        } else {
            IconState::Cool
        };

        if new_state != self.current_icon_state {
            self.command_sender
                .send(TrayCommand::UpdateIcon(new_state.clone()))
                .map_err(|e| Box::new(e) as Box<dyn Error>)?;

            let state_str = match new_state {
                IconState::Hot => "🔴 HOT",
                IconState::Warm => "🟡 WARM",
                IconState::Cool => "🟢 COOL",
            };

            println!(
                "🌡️ Temperature: {:.1}°C - State: {} (icon update requested)",
                temperature, state_str
            );
            info!("Tray icon update requested for state: {:?}", new_state);

            self.current_icon_state = new_state;
        }

        Ok(())
    }

    fn load_icon_for_state(state: &IconState) -> Result<Icon, Box<dyn Error>> {
        let icon_filename = match state {
            IconState::Cool => "icons/thermometer-cool.ico",
            IconState::Warm => "icons/thermometer-warm.ico",
            IconState::Hot => "icons/thermometer-hot.ico",
        };

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

        warn!("⚠️ No icon files found, creating programmatic icon");
        let rgba_image = Self::create_simple_rgba_image_for_state(state);
        let icon = Icon::from_rgba(
            rgba_image.as_raw().clone(),
            rgba_image.width(),
            rgba_image.height(),
        )?;
        Ok(icon)
    }

    fn create_simple_rgba_image_for_state(state: &IconState) -> RgbaImage {
        let mut img = RgbaImage::new(16, 16);

        let fill_color = match state {
            IconState::Cool => [0, 255, 0, 255],
            IconState::Warm => [255, 165, 0, 255],
            IconState::Hot => [255, 0, 0, 255],
        };

        for y in 0..16 {
            for x in 0..16 {
                let pixel = if x == 8 && y < 12 {
                    fill_color
                } else if (x == 7 || x == 9) && y < 12 {
                    [0, 0, 0, 255]
                } else if (6..=10).contains(&x) && (12..=14).contains(&y) {
                    fill_color
                } else if (5..=11).contains(&x) && (11..=15).contains(&y) {
                    [0, 0, 0, 255]
                } else {
                    [0, 0, 0, 0]
                };

                img.put_pixel(x, y, image::Rgba(pixel));
            }
        }

        img
    }
}

impl Drop for SystemTray {
    fn drop(&mut self) {
        if let Err(err) = self.command_sender.send(TrayCommand::Shutdown) {
            warn!("Tray command channel closed during shutdown: {}", err);
        }

        if let Some(handle) = self.thread_handle.take() {
            if let Err(e) = handle.join() {
                warn!("Failed to join tray thread: {:?}", e);
            }
        }

        clear_tray_handlers();
    }
}

fn run_tray_event_loop(
    command_receiver: mpsc::Receiver<TrayCommand>,
    event_sender: Sender<TrayMessage>,
    ready_sender: std::sync::mpsc::Sender<Result<(), TrayThreadInitError>>,
) -> Result<(), Box<dyn Error>> {
    let menu_sender = event_sender.clone();
    MenuEvent::set_event_handler(Some(Box::new(move |event: MenuEvent| {
        println!("🖱️ Menu event received: {:?}", event);
        info!("Tray menu event: {}", event.id().0);

        if let Some(msg) = menu_id_to_message(event.id().0.as_str()) {
            if let Err(e) = menu_sender.send(msg) {
                warn!("❌ Failed to send tray message: {}", e);
            }
        } else {
            println!("🤷 Unknown menu item: {}", event.id().0);
        }
    })));

    let click_sender = event_sender.clone();
    TrayIconEvent::set_event_handler(Some(Box::new(move |event: TrayIconEvent| match event {
        TrayIconEvent::Click {
            button: MouseButton::Right,
            ..
        } => {
            println!("🖱️ Right click detected on tray icon, showing menu");
            info!("Tray context menu requested (right click)");
        }
        TrayIconEvent::DoubleClick { .. } => {
            println!("🖱️ Double click detected on tray icon");
            info!("Tray icon double-clicked, opening dashboard");
            if let Err(e) = click_sender.send(TrayMessage::OpenDashboard) {
                warn!("❌ Failed to send double-click message: {}", e);
            }
        }
        _ => {}
    })));

    let menu = Menu::new();

    // Simplified menu with only 4 essential items
    let dashboard_item = MenuItem::with_id(
        MenuId::new("open_dashboard"),
        "🌐 Open Dashboard",
        true,
        None,
    );
    let logs_item = MenuItem::with_id(MenuId::new("view_logs"), "📋 View Logs", true, None);
    let settings_item =
        MenuItem::with_id(MenuId::new("edit_settings"), "⚙️ Edit Settings", true, None);
    let separator = PredefinedMenuItem::separator();
    let quit_item = MenuItem::with_id(MenuId::new("quit_monitor"), "❌ Quit Monitor", true, None);

    menu.append_items(&[
        &dashboard_item,
        &logs_item,
        &settings_item,
        &separator,
        &quit_item,
    ])?;

    // Keep menu handles alive for the lifetime of the tray thread
    let _menu_handles = vec![
        dashboard_item.clone(),
        logs_item.clone(),
        settings_item.clone(),
        quit_item.clone(),
    ];

    let icon = match SystemTray::load_icon_for_state(&IconState::Cool) {
        Ok(icon) => icon,
        Err(e) => {
            let err = TrayThreadInitError(format!("Failed to load initial tray icon: {}", e));
            let _ = ready_sender.send(Err(err.clone()));
            clear_tray_handlers();
            return Err(Box::new(err));
        }
    };

    let tray_icon = match TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("GPU Temperature Monitor - Right click for menu, double click for settings")
        .with_menu_on_left_click(false)
        .with_icon(icon)
        .build()
    {
        Ok(icon) => icon,
        Err(e) => {
            let err = TrayThreadInitError(format!("Failed to create tray icon: {}", e));
            let _ = ready_sender.send(Err(err.clone()));
            clear_tray_handlers();
            return Err(Box::new(err));
        }
    };

    info!("✅ System tray initialized with non-blocking event handler");
    let _ = ready_sender.send(Ok(()));

    let mut current_state = IconState::Cool;
    let tray_icon = tray_icon;

    loop {
        pump_windows_messages();

        match command_receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(TrayCommand::UpdateIcon(state)) => {
                if state != current_state {
                    match SystemTray::load_icon_for_state(&state) {
                        Ok(icon) => {
                            if let Err(e) = tray_icon.set_icon(Some(icon)) {
                                warn!("⚠️ Failed to set tray icon: {}", e);
                            } else {
                                current_state = state;
                            }
                        }
                        Err(e) => warn!("⚠️ Failed to load tray icon: {}", e),
                    }
                }
            }
            Ok(TrayCommand::Shutdown) => {
                info!("Tray thread received shutdown signal");
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                warn!("Tray command channel disconnected");
                break;
            }
        }
    }

    clear_tray_handlers();

    Ok(())
}

fn clear_tray_handlers() {
    MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
    TrayIconEvent::set_event_handler(None::<fn(TrayIconEvent)>);
}

fn pump_windows_messages() {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_menu_ids() {
        assert_eq!(
            menu_id_to_message("open_dashboard"),
            Some(TrayMessage::OpenDashboard)
        );
        assert_eq!(menu_id_to_message("view_logs"), Some(TrayMessage::ViewLogs));
        assert_eq!(
            menu_id_to_message("edit_settings"),
            Some(TrayMessage::EditSettings)
        );
        assert_eq!(
            menu_id_to_message("quit_monitor"),
            Some(TrayMessage::QuitMonitor)
        );
    }

    #[test]
    fn unknown_menu_id_returns_none() {
        assert_eq!(menu_id_to_message("nope"), None);
    }
}
