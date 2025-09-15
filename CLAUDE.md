# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

GpuTempWatch is a PowerShell script for monitoring GPU temperatures using LibreHardwareMonitor (LHM) data. The script polls temperature sensors, detects overheating conditions, and sends desktop notifications when temperature thresholds are exceeded.

## Architecture

### Core Components

- **Temperature Monitoring**: Polls LibreHardwareMonitor's JSON API at `http://127.0.0.1:8085/data.json`
- **Sensor Detection**: Uses configurable patterns (`$GpuTempNamePatterns`) to identify GPU temperature sensors
- **Temperature Parsing**: Extracts numeric values from various temperature string formats (e.g., "61,0 °C")
- **Notification System**: Prefers BurntToast module for toast notifications, with fallback to standard Windows MessageBox
- **Logging**: Writes activity to `.\Logs\GpuTempWatch.log` (local directory)

### Key Functions

- `Collect-GpuTemps()`: Recursively traverses LHM JSON structure to find GPU temperature sensors
- `Parse-Temp()`: Parses temperature strings with different formats and locales
- `Notify()`: Sends toast notifications with cooldown logic
- `Write-Log()`: Timestamped logging functionality

### Configuration Variables

- `$ThresholdC`: Temperature threshold in Celsius (default: 60°C)
- `$PollSeconds`: Polling interval (default: 20 seconds)
- `$BaseCooldownSec`: Base cooldown between notifications (default: 20 seconds)
- `$GpuTempNamePatterns`: Array of wildcard patterns to match GPU temperature sensors

## Development Commands

### Running the Script
```powershell
# Run directly
.\GpuTempWatch.ps1

# Run in background
Start-Process powershell -ArgumentList "-File .\GpuTempWatch.ps1" -WindowStyle Hidden
```

### Requirements
- LibreHardwareMonitor must be running with web server enabled on port 8085
- BurntToast PowerShell module for notifications (optional - script includes fallback notifications)

### Prerequisites Check
```powershell
# Test LHM connectivity
Invoke-RestMethod -Uri "http://127.0.0.1:8085/data.json" -TimeoutSec 2

# Check BurntToast module
Get-Module -ListAvailable BurntToast
```

### Log Monitoring
```powershell
# Monitor log in real-time
Get-Content ".\Logs\GpuTempWatch.log" -Wait -Tail 10

# View recent log entries
Get-Content ".\Logs\GpuTempWatch.log" -Tail 20
```

### Notification System
The script uses a multi-tier notification approach with smart cooldown:

1. **Primary**: BurntToast module (Windows 10+ toast notifications)
2. **Fallback**: Console output with colored text and critical log entry

#### Smart Cooldown Logic
- **First alert**: Immediate notification when temperature exceeds threshold
- **Subsequent alerts**: Exponential backoff (20s → 40s → 80s → 160s → 320s max)
- **Reset condition**: When temperature drops below threshold, cooldown resets to base interval
- **Continuous monitoring**: Always logs temperature status every polling cycle

### Installing BurntToast (Optional)
```powershell
# Install from PowerShell Gallery
Install-Module -Name BurntToast -Scope CurrentUser

# Import manually if needed
Import-Module BurntToast
```