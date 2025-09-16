# Remediation Plan (Windows 11 Target)

## 0. Dependency & Runtime Migration
- **Goal:** Modernize the stack for Windows 11 by consolidating platform support and observability.
- **Actions:**
  1. Swap `warp` + `tokio-tungstenite` for `axum` (with `ws`, `macros`) to serve HTTP and WebSocket traffic in one crate.
  2. Replace `log`/`env_logger` with `tracing` + `tracing-subscriber` to capture structured, contextual logs.
  3. Remove `winapi`; rely exclusively on the `windows` crate. Enable features `Win32_UI_Shell`, `Win32_UI_WindowsAndMessaging`, `Win32_System_Power`, `UI_Notifications`, and related WinRT namespaces.
  4. Optional: migrate `chrono` to the `time` crate if downstream JSON consumers tolerate new formats.
  5. Prefer direct WinRT toast integrations via `windows::UI::Notifications`; keep `winrt-notification` only while porting.

## 1. Tray Experience & Menu Simplification
- **Observations:**
  - Logs show duplicated tray messages (`📬 Received tray message: ...` twice) because the current bridge reads the same event multiple times.
  - The context menu still exposes deprecated entries; only four options are required.
  - Menu actions fire slowly due to polling.
- **Actions:**
  1. Build a single async bridge: the tray thread pushes events into a `tokio::sync::mpsc::UnboundedSender`; `main` consumes with `tokio::select!` to remove the poll-interval delay and prevent duplicate dispatches. Ensure the tray thread closes the channel before shutdown.
  2. Reconstruct the context menu with four items and improved labels:
     - `Open Dashboard` — launches the web interface in the default browser.
     - `View Logs` — opens `GpuTempWatch.log` in the system shell.
     - `Edit Settings` — opens `config.json` for editing.
     - `Quit Monitor` — terminates the process cleanly.
     Map these to new `TrayMessage` variants and delete legacy pause/autostart entries.
  3. Call `.with_menu_on_left_click(false)` so only right-click opens the menu; keep double-click for “Open Dashboard”.
  4. In the tray worker, replace `recv_timeout` with a blocking `recv` plus `pump_windows_messages()` to guarantee prompt delivery without event duplication.

## 2. Configuration & Temperature Consistency
- **Observations:** Temperatures occasionally report zero over WebSocket because the monitor owns a stale Config copy and only updates when thresholds are exceeded.
- **Actions:**
  1. Store `Config` in an `Arc<RwLock<_>>` shared by the monitor, tray, and web layer. Always read the latest thresholds/intervals before evaluating temperatures.
  2. Track the hottest reading using `max()` (or `Option<f32>`) and persist the sensor name so dashboards never fall back to 0.0 between alerts.
  3. After `/api/config` updates, write to the shared config, flush to disk, and publish a “config updated” event (via WebSocket broadcast) so all clients refresh thresholds immediately.

## 3. Real-Time Web Delivery with `axum`
- **Observations:** The SPA relies on `setInterval` polling; WebSocket clients still see stale data.
- **Actions:**
  1. Provide `/ws` via `axum::extract::ws`. Maintain a list of active senders protected by `Mutex`/`RwLock`.
  2. Broadcast JSON snapshots on every temperature sample, config change, and log append. Include current temperature, thresholds, monitoring status, and recent log entries.
  3. Update `web/index.html` to prioritize the WebSocket feed. On socket messages, update UI state instantly; use REST polling only as a fallback if the socket closes.

## 4. Reliable Windows 11 Toast Notifications
- **Observations:** Logs show `"WinRT toast not yet fully implemented"` warnings. The toast helper is a stub and never contacts WinRT.
- **Actions:**
  1. Implement real WinRT toast delivery using `windows` APIs (`ToastNotificationManager`, `XmlDocument`, etc.). Register a stable AppUserModelID once during startup.
  2. Execute toast creation/sending inside `tokio::task::spawn_blocking`, wrapping the call in `CoInitializeEx(COINIT_APARTMENTTHREADED)` to satisfy COM STA requirements.
  3. Log success/failure for each toast and keep the fallback (message box/console) when WinRT is unavailable.

## 5. Verification Checklist
- **Automated:** `cargo fmt`, `cargo clippy --all-targets --all-features`, `cargo test`.
- **Manual QA:**
  - Tray menu shows only the four approved items, opens solely on right-click, and actions execute immediately without duplication.
  - Double-click launches the dashboard; WebSocket telemetry updates temperature/thresholds live without reverting to zero.
  - Editing settings through the UI updates `config.json`, the shared config state, and connected clients instantly.
  - Toast notifications appear in Action Center; failure logs should disappear once the WinRT path is implemented.

## 6. Tray.rs Refactoring (Critical Issues Found)
- **Observations from logs:**
  - Duplicate tray messages: `📬 Received tray message: About` appears twice per action
  - Right-click detection fires multiple times causing menu flicker
  - File operations fail: `❌ Failed to open log file: ./Logs/GpuTempWatch.log`
- **Actions:**
  1. **Remove tray-icon dependency** — Replace with direct `Shell_NotifyIconW` via `windows` crate
  2. **Fix event duplication** — Implement proper message filtering in Windows event loop
  3. **Async bridge** — Use `tokio::sync::mpsc::UnboundedSender` instead of polling approach
  4. **Simplified menu** — Only 4 items: Open Dashboard, View Logs, Edit Settings, Quit Monitor
  5. **File path fixes** — Correct absolute vs relative path handling for log/config access

## 7. Complete WinRT Toast Implementation
- **Current state:** Stub returns `"WinRT toast not yet fully implemented"` error
- **Actions:**
  1. Real `ToastNotificationManager` integration with proper XML templates
  2. AppUserModelID registration for Action Center persistence
  3. COM STA thread initialization via `CoInitializeEx(COINIT_APARTMENTTHREADED)`
  4. Structured error handling with fallback chain: WinRT → MessageBox → Console

## 8. Configuration Consistency Fixes
- **Actions:**
  1. Shared `Arc<RwLock<Config>>` across monitor/tray/web components
  2. Immediate WebSocket broadcast on config changes
  3. File watcher for automatic config reload

Implementing these steps modernizes the Windows 11 build, removes redundant dependencies, and restores a responsive tray, accurate telemetry, and reliable notifications.
