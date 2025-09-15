# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

GpuTempWatch is a lightweight Rust application for monitoring GPU temperatures using NVIDIA Management Library (NVML). The application provides real-time temperature monitoring with system tray integration, Windows toast notifications, and file logging.

## Architecture

### Core Components

- **NVML Integration**: Direct communication with NVIDIA drivers via NVML for real-time GPU temperature readings
- **System Tray**: Native Windows system tray icon with color-coded temperature status and context menu
- **Toast Notifications**: Windows native toast notifications with smart cooldown logic
- **File Logging**: Structured logging to `./Logs/GpuTempWatch.log` with timestamped entries
- **Configuration**: JSON-based configuration with automatic defaults

### Key Modules

- `monitor.rs`: NVML wrapper for GPU temperature monitoring
- `tray.rs`: System tray integration with temperature-based icon updates
- `notifications.rs`: Windows toast notification system with exponential backoff
- `logging.rs`: File logging with automatic directory creation
- `config.rs`: JSON configuration management with validation

### Configuration

- `temperature_threshold_c`: Temperature threshold in Celsius (default: 60°C)
- `poll_interval_sec`: Polling interval in seconds (default: 20)
- `base_cooldown_sec`: Base cooldown between notifications (default: 20)
- `enable_logging`: Enable/disable file logging (default: true)
- `log_file_path`: Path to log file (default: "./Logs/GpuTempWatch.log")

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

### Log Monitoring
```bash
# Monitor log in real-time (Windows)
Get-Content ".\Logs\GpuTempWatch.log" -Wait -Tail 10

# View recent log entries
Get-Content ".\Logs\GpuTempWatch.log" -Tail 20

# Open logs directory
explorer Logs\
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