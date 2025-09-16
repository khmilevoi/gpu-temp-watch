# Remediation Plan

## Overview
The current build exposes multiple regressions:
- Tray actions are delayed or mis-triggered because we only poll the tray channel once per monitoring loop and the menu still contains deprecated entries.
- The web dashboard reports stale temperatures and thresholds because the runtime keeps configuration in a local copy and we only push values when the monitoring loop observes threshold violations.
- The UI relies solely on REST polling, so even when a WebSocket client is connected, values can arrive as zero due to timing gaps.
- Windows toast notifications silently fail; we never verify AppUserModelID registration nor run the toast call on a COM STA thread.

This plan restructures the tray runtime, configuration flow, and telemetry streaming to restore responsiveness while simplifying the menu to the four supported actions.

## 1. Tray Responsiveness & Menu Simplification
1. Replace the current `try_recv` polling inside `main` with an async channel:
   - Spawn a dedicated task that blocks on the tray thread’s `mpsc::Receiver` and forwards each `TrayMessage` through a `tokio::sync::mpsc::UnboundedSender`.
   - In the main monitoring loop, use `tokio::select!` to process tray messages as soon as they arrive (no dependency on the poll interval).
   - This eliminates the existing delay between a click and the next monitoring cycle.
2. Update `SystemTray::new` to expose only the required commands:
   - “GPU Temperature Monitor” → open the web UI.
   - “Открыть логи” → open log file.
   - “Открыть конфиг” → open `config.json`.
   - “Убить приложение” → dispatch `TrayMessage::Exit`.
   - Remove pause/resume/autostart entries; those will remain available through the web interface.
3. Ensure right-click is the only trigger for the context menu:
   - Call `.with_menu_on_left_click(false)` when building the tray icon so left-click does not show the menu.
   - Keep the double-click handler for opening the web UI; remove extraneous logging once verified.
4. Tweak the tray thread loop:
   - Replace the busy `recv_timeout`/spin with a blocking `recv` paired with `pump_windows_messages()` to avoid unnecessary latency.
   - Maintain the message pump so Windows continues dispatching tooltip and menu notifications.

## 2. Accurate Temperature & Config Propagation
1. Share configuration between all components:
   - Store `Config` inside an `Arc<RwLock<Config>>` and pass clones to the monitoring loop, the web server, and the tray.
   - Always read the latest values inside `monitor_temperatures` instead of relying on a captured copy.
2. Fix maximum temperature tracking:
   - Track `Option<f32>` and call `max()` for every reading so the reported max is always the latest, even below threshold.
   - Persist the hottest sensor name alongside the value for logging and UI display.
3. After a `/api/config` update:
   - Write the new config to the shared `Arc<RwLock<Config>>`.
   - Publish a “config updated” message to the WebSocket broadcast (see section 3) so clients refresh without polling.

## 3. Real-Time Web Updates
1. Implement a WebSocket endpoint `/ws` in `WebServer::start` using `tokio_tungstenite`:
   - Maintain a `Vec<UnboundedSender<serde_json::Value>>` of active clients in a `Mutex` or `RwLock`.
   - On connection, stream the latest status, config, and recent logs immediately.
2. Push updates when new data arrives:
   - From `monitor_temperatures`, call a helper on `WebServer` (via `SharedState` or a broadcast channel) with the latest temperature snapshot.
   - From `update_config`, broadcast the new configuration so dashboards update thresholds instantly.
   - Optionally forward new log entries when `FileLogger::log_temperature_reading` runs to keep the UI in sync.
3. Update `web/index.html`:
   - Attempt to open a WebSocket (`ws://` or `wss://` depending on origin) after `DOMContentLoaded`.
   - On socket messages, update `currentStatus`, `currentConfig`, and the log view; fall back to the existing `setInterval` polling if the socket closes.
   - Remove duplicate polling once the socket is stable (retain a long-interval fallback if desired).

## 4. Windows Toast Notification Reliability
1. Register or reuse a valid AppUserModelID before sending any toasts:
   - Use `Toast::POWERSHELL_APP_ID` as the default or create a static AUMID via `Shell_NotifyIconGetRect` registry if branding is required.
2. Execute toast delivery on a blocking STA task:
   - Wrap `Toast::show()` inside `tokio::task::spawn_blocking` and call `CoInitializeEx` with `COINIT_APARTMENTTHREADED` for the task scope.
   - Log both success and error paths so we can diagnose failures.
3. Keep the existing fallback path (message box in debug, console logging) so notifications continue to surface if the toast API is unavailable.

## 5. Verification Steps
1. Unit & integration checks:
   - `cargo test`
   - `cargo fmt`
   - `cargo clippy --all-targets --all-features` (optional but recommended).
2. Manual QA:
   - Launch the app, verify the tray menu shows only the four items and responds instantly.
   - Right-click vs left-click behaviour (menu only on right click).
   - Double-click opens the web UI without delay.
   - Adjust thresholds in the web UI; confirm the change updates the file and propagates to live readings (no stale WebSocket data).
   - Monitor temperature changes to ensure the web dashboard and logs reflect non-zero values continuously.
   - Trigger alerts to confirm toasts appear; inspect logs if they fall back to message boxes.

Implementing these steps restores interactive parity between the tray, monitoring loop, and UI while simplifying the configuration surface to the officially supported entry points.
