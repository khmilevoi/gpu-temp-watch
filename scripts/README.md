# Icon Generation Scripts

This directory contains scripts for generating all icons for GPU Temperature Monitor (both regular and development versions).

## Files

- `create_icons.py` - Python script to generate all thermometer icons (regular + dev versions)
- `create_dev_icons.bat` - Batch file to install dependencies and run the Python script
- `requirements.txt` - Python dependencies

## Usage

### Option 1: Using the batch file (Windows)
```bash
scripts\create_dev_icons.bat
```

### Option 2: Manual execution
```bash
# Install dependencies
pip install -r scripts/requirements.txt

# Run the script
python scripts/create_dev_icons.py
```

## Generated Icons

The script creates both regular and dev versions of thermometer icons:

### Regular Icons (transparent background)
- `thermometer-cool.ico` - Green thermometer on transparent background
- `thermometer-warm.ico` - Orange thermometer on transparent background
- `thermometer-hot.ico` - Red thermometer on transparent background

### Dev Icons (black background)
- `thermometer-cool-dev.ico` - Green thermometer on black background
- `thermometer-warm-dev.ico` - Orange thermometer on black background
- `thermometer-hot-dev.ico` - Red thermometer on black background

## How It Works

1. **Debug builds** (`cargo build`) automatically use the `-dev` icons with black backgrounds
2. **Release builds** (`cargo build --release`) use the normal icons with transparent backgrounds
3. Icons are generated as both PNG and ICO formats for compatibility
4. The application automatically detects debug vs release mode using `cfg!(debug_assertions)`

## Development Mode Features

When running in debug mode, the application has these special behaviors:

- **Icons**: Uses dev icons with black background for easy identification
- **Data Storage**: Uses `./AppData/` directory in project instead of `%LOCALAPPDATA%`
- **Configuration**: Stored in `./AppData/config.json`
- **Logs**: Stored in `./AppData/Logs/gpu-temp-watch.log`

This makes development and testing easier by keeping all data local to the project.