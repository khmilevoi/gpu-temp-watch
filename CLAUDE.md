# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

GpuTempWatch is a lightweight Rust application for monitoring GPU temperatures using NVIDIA Management Library (NVML). The application provides real-time temperature monitoring with system tray integration, Windows toast notifications, web-based configuration interface, and comprehensive file logging.

## Architecture

### Core Components

- **NVML Integration**: Direct communication with NVIDIA drivers via NVML for real-time GPU temperature readings
- **System Tray**: Native Windows system tray icon with dynamic color-coded temperature icons and context menu
- **Web Interface**: Modern web-based configuration and monitoring interface on localhost:18235
- **Toast Notifications**: Windows native toast notifications with smart exponential backoff cooldown
- **Unified Logging**: New LoggerService providing both console and file logging with JSON structured format, correlation IDs, and automatic rotation to `./Logs/gpu-temp-watch.log`
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

### Configuration

- `temperature_threshold_c`: Temperature threshold in Celsius (default: 60°C)
- `poll_interval_sec`: Polling interval in seconds (default: 20)
- `base_cooldown_sec`: Base cooldown between notifications (default: 20)
- `enable_logging`: Enable/disable file logging (default: true)
- `log_file_path`: Path to log file (default: "./Logs/gpu-temp-watch.log")

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
Get-Content ".\Logs\gpu-temp-watch.log" -Wait -Tail 10

# View recent log entries
Get-Content ".\Logs\gpu-temp-watch.log" -Tail 20

# Open logs directory
explorer Logs\
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
- 🟢🟡🔴 Color-coded temperature indication (planned)
- Right-click context menu with controls
- Pause/resume monitoring
- Settings access
- Quick log access
- Exit option

## Performance Benefits

### Rust vs PowerShell Implementation
- ✅ **Memory Usage**: <2MB vs 20MB+ (PowerShell)
- ✅ **Startup Time**: Instant vs several seconds
- ✅ **Resource Efficiency**: Minimal CPU usage
- ✅ **No Dependencies**: Self-contained executable
- ✅ **Native Performance**: Direct NVML integration
- ✅ **Reliability**: No execution policy issues
- ✅ **Size**: 1.2MB executable vs multiple script files

## Claude Code Environment
This project was developed and migrated from PowerShell to Rust with Claude Code:
- **Model**: Sonnet 4 (claude-sonnet-4-20250514)
- **Platform**: Windows (win32)
- **Working Directory**: C:\Users\Khmil\Scripts\GpuTempWatch
- **Date**: September 2025
- **Migration**: Complete replacement of PowerShell implementation with native Rust