# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

GpuTempWatch is a production-ready lightweight Rust application for monitoring GPU temperatures using NVIDIA Management Library (NVML). The application provides real-time temperature monitoring with system tray integration, Windows toast notifications, web-based configuration interface, and comprehensive logging.

**Current Status**: ✅ **Production Ready** - Fully functional with complete feature set, comprehensive documentation, and testing.

## Architecture

### Core Components

- **NVML Integration**: Direct communication with NVIDIA drivers via NVML for real-time GPU temperature readings
- **System Tray**: Native Windows system tray icon with dynamic color-coded temperature icons and context menu
- **Web Interface**: Modern web-based configuration and monitoring interface on localhost:18235
- **Toast Notifications**: Windows native toast notifications with smart exponential backoff cooldown
- **Unified Logging**: New LoggerService providing both console and file logging with JSON structured format, correlation IDs, and automatic rotation to `%LOCALAPPDATA%\GpuTempWatch\Logs\gpu-temp-watch.log`
- **Autostart Management**: Automatic Windows startup integration via registry
- **Configuration**: JSON-based configuration with real-time web updates

### Key Modules

- `monitor.rs`: NVML wrapper for GPU temperature monitoring with error handling
- `tray.rs`: System tray integration with dynamic icon updates (cool/warm/hot states) and double-click support
- `notifications.rs`: Windows toast notification system with exponential backoff (20s → 40s → 80s → 160s → 320s)
- `logger_service.rs`: Unified logging service with console and file output, JSON structured logging, and automatic rotation
- `config.rs`: JSON configuration management with validation and live updates
- `web_server.rs`: HTTP server with REST API and WebSocket support for real-time monitoring
- `autostart.rs`: Windows registry integration for startup management
- `gui.rs`: Native Windows dialogs and file operations
- `app_paths.rs`: Centralized application path management using %LOCALAPPDATA%

### Configuration

- `temperature_threshold_c`: Temperature threshold in Celsius (default: 80°C)
- `poll_interval_sec`: Polling interval in seconds (default: 20)
- `base_cooldown_sec`: Base cooldown between notifications (default: 20)
- `enable_logging`: Enable/disable file logging (default: true)
- `log_file_path`: Path to log file (default: "%LOCALAPPDATA%\\GpuTempWatch\\Logs\\gpu-temp-watch.log")

### Project Structure

```
GpuTempWatch/
├── src/
│   ├── main.rs                 # Application entry point and main loop
│   ├── app_paths.rs           # Centralized path management (%LOCALAPPDATA%)
│   ├── config.rs              # JSON configuration management
│   ├── monitor.rs             # NVML GPU temperature monitoring
│   ├── tray.rs                # System tray integration with color icons
│   ├── notifications.rs       # Windows toast notifications with backoff
│   ├── logger_service.rs      # Unified console/file logging service
│   ├── web_server.rs          # HTTP/WebSocket server for web interface
│   ├── autostart.rs           # Windows registry autostart management
│   └── gui.rs                 # Native Windows dialogs
├── web/
│   └── index.html             # Modern web dashboard interface
├── icons/
│   ├── icon.ico               # Main application icon
│   ├── thermometer-cool.ico   # Green tray icon (below threshold)
│   ├── thermometer-warm.ico   # Yellow tray icon (approaching threshold)
│   └── thermometer-hot.ico    # Red tray icon (above threshold)
├── docs/
│   ├── web-interface.png      # Web dashboard screenshot
│   └── tray-menu.png          # System tray menu screenshot
├── Cargo.toml                 # Rust dependencies and build configuration
├── README.md                  # Complete user documentation
├── CLAUDE.md                  # Development guidance (this file)
├── config.json               # Runtime configuration file
└── Logs/                      # Application logs directory
```

### Application Data Storage

The application follows Windows standards for data storage:

- **Configuration**: `%LOCALAPPDATA%\GpuTempWatch\config.json`
- **Logs**: `%LOCALAPPDATA%\GpuTempWatch\Logs\gpu-temp-watch.log`
- **Fallback**: Current working directory (for compatibility)

All paths are managed centrally through the `AppPaths` module, ensuring consistent behavior across the application.

## Development Commands

### Building the Application

#### Development Build
```bash
# Build debug version
cargo build

# Check for compilation errors
cargo check
```

#### Release Build
```bash
# Build optimized release version (~1.2MB executable)
cargo build --release

# The executable will be located at:
# target/release/gpu-temp-watch.exe
```

### Running the Application

#### Direct Execution
```bash
# Run development version
cargo run

# Run release version
./target/release/gpu-temp-watch.exe
```

#### System Integration
- The application runs as a console application with system tray integration
- No external dependencies required (all features built-in)
- Automatic configuration file creation on first run
- Works with any NVIDIA GPU with NVML support

### Requirements
- NVIDIA GPU with compatible drivers
- Windows 10/11 (for toast notifications)
- No additional software required (self-contained executable)

### Logging System

The application uses a unified LoggerService that provides:

#### Features
- **Dual Output**: Both console (human-readable) and file (JSON structured) logging
- **JSON Format**: Structured file logs with correlation IDs, timestamps, and contextual data
- **Log Rotation**: Automatic rotation based on file size (10MB) and count (5 files)
- **Thread-Safe**: Concurrent access using Arc<Mutex<>>
- **Multiple Levels**: Trace, Debug, Info, Warn, Error with configurable minimum levels
- **Specialized Functions**: Built-in support for startup, temperature alerts, and shutdown events

#### Log Monitoring
```bash
# Monitor log in real-time (Windows)
Get-Content "$env:LOCALAPPDATA\GpuTempWatch\Logs\gpu-temp-watch.log" -Wait -Tail 10

# View recent log entries
Get-Content "$env:LOCALAPPDATA\GpuTempWatch\Logs\gpu-temp-watch.log" -Tail 20

# Open logs directory
explorer "$env:LOCALAPPDATA\GpuTempWatch\Logs"
```

#### Usage Examples
```rust
// Basic logging
log_info!("Application started");
log_error!("Failed to connect", json!({"error": "Connection timeout"}));

// Temperature logging
log_temperature!("GPU-0", 75.5, 80.0);

// Startup logging
log_startup!("1.0.0", &["--help"]);
```

### Notification System
The application uses native Windows toast notifications with smart cooldown:

1. **Primary**: Windows native toast notifications
2. **Fallback**: Console output with emoji indicators

#### Smart Cooldown Logic
- **First alert**: Immediate notification when temperature exceeds threshold
- **Subsequent alerts**: Exponential backoff (20s → 40s → 80s → 160s → 320s max)
- **Reset condition**: When temperature drops below threshold, cooldown resets to base interval
- **Continuous monitoring**: Always logs temperature status every polling cycle

### System Tray Features
- 🟢🟡🔴 **Color-coded temperature indication** (✅ Implemented)
  - Green: Below threshold (cool)
  - Yellow: Approaching threshold (warm)
  - Red: Above threshold (hot)
- **Right-click context menu** with controls:
  - Open Dashboard (web interface)
  - View Logs (file explorer)
  - Edit Settings (quick config)
  - Quit Monitor
- **Double-click**: Opens web dashboard
- **Hover tooltip**: Shows current temperature

## Production Status & Features

### ✅ Completed Features
- **Real-time GPU monitoring** with NVML integration
- **System tray integration** with color-coded temperature icons
- **Web dashboard** on localhost:18235 with real-time updates
- **Smart toast notifications** with exponential backoff (20s→40s→80s→160s→320s)
- **Comprehensive logging** with JSON structured format and rotation
- **Live configuration updates** via web interface
- **Windows autostart integration** via registry
- **Complete documentation** with screenshots and usage guide
- **Professional file structure** with docs/ directory

### Performance Benefits (Rust vs PowerShell)
- ✅ **Memory Usage**: <2MB vs 20MB+ (PowerShell)
- ✅ **Startup Time**: Instant vs several seconds
- ✅ **Resource Efficiency**: Minimal CPU usage
- ✅ **No Dependencies**: Self-contained executable
- ✅ **Native Performance**: Direct NVML integration
- ✅ **Reliability**: No execution policy issues
- ✅ **Size**: 1.2MB executable vs multiple script files

## Documentation & Support

### Available Documentation
- **README.md**: Complete user guide with installation, usage, and troubleshooting
- **docs/web-interface.png**: Screenshot of web dashboard showing all features
- **docs/tray-menu.png**: Screenshot of system tray context menu
- **CLAUDE.md**: This development guidance file

### Web Interface Features
The web dashboard (localhost:18235) provides:
- **Real-time temperature display** with threshold indicator
- **Status badges**: Active, Temperature State, Autostart, GPU Connection
- **Control panel**: Pause/Resume, Toggle Autostart, Manual Refresh
- **Live configuration**: Temperature threshold, polling interval, cooldown, logging
- **Recent logs viewer** with automatic updates
- **Responsive design** with modern UI/UX

## Development Environment
This project was developed and migrated from PowerShell to Rust with Claude Code:
- **Model**: Sonnet 4 (claude-sonnet-4-20250514)
- **Platform**: Windows (win32)
- **Working Directory**: C:\Users\Khmil\Scripts\GpuTempWatch
- **Date**: September 2025
- **Status**: ✅ **Production Ready** - Migration complete with full feature parity and enhancements