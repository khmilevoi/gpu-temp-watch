#!/usr/bin/env python3
"""
Script to create all icons for GPU Temperature Monitor
- Regular icons: thermometer with transparent background
- Dev icons: same thermometer with black background
Requires: pip install Pillow
"""

from PIL import Image, ImageDraw
import os

def create_thermometer_icon(size, color, background_color, output_path):
    """Create a thermometer icon with specified colors and size"""
    img = Image.new('RGBA', (size, size), background_color)
    draw = ImageDraw.Draw(img)

    # Scale factors based on icon size
    scale = size / 32

    # Use WHITE outline for BOTH versions for consistency
    # This ensures the icons look identical regardless of background
    outline_color = 'white'

    # Thermometer tube (vertical rectangle)
    tube_x = int(size // 2)
    tube_top = int(3 * scale)
    tube_bottom = int(20 * scale)
    tube_width = int(3 * scale)

    # Draw thermometer tube with white outline (consistent for both versions)
    draw.rectangle([
        tube_x - tube_width, tube_top,
        tube_x + tube_width, tube_bottom
    ], outline=outline_color, width=2, fill=color)

    # Thermometer bulb (larger circle at bottom)
    bulb_radius = int(5 * scale)
    bulb_center_x = tube_x
    bulb_center_y = int(25 * scale)

    # Draw bulb with white outline (consistent for both versions)
    draw.ellipse([
        bulb_center_x - bulb_radius, bulb_center_y - bulb_radius,
        bulb_center_x + bulb_radius, bulb_center_y + bulb_radius
    ], outline=outline_color, width=2, fill=color)

    # Temperature markings (scale marks on the right side) - white for both versions
    mark_length = int(2 * scale)
    for i in range(4):
        mark_y = tube_top + int((i + 1) * 3 * scale)
        draw.line([
            tube_x + tube_width + 1, mark_y,
            tube_x + tube_width + 1 + mark_length, mark_y
        ], fill=outline_color, width=1)

    # Add temperature level indicator inside tube
    level_height = int(12 * scale)  # How much of the tube is "filled"
    level_bottom = tube_bottom - 1
    level_top = level_bottom - level_height

    # Draw the "mercury" level with slightly different shade
    mercury_color = tuple(min(255, c + 20) if i < 3 else c for i, c in enumerate(color))
    draw.rectangle([
        tube_x - tube_width + 2, level_top,
        tube_x + tube_width - 2, level_bottom - 1
    ], fill=mercury_color)

    return img

def create_ico_file(png_path, ico_path):
    """Convert PNG to ICO format with multiple sizes"""
    try:
        img = Image.open(png_path)
        # Create ICO with multiple sizes (16x16, 32x32, 48x48)
        sizes = [(16, 16), (32, 32), (48, 48)]

        images = []
        for size in sizes:
            resized = img.resize(size, Image.Resampling.LANCZOS)
            images.append(resized)

        # Save as ICO
        images[0].save(ico_path, format='ICO', sizes=[(img.width, img.height) for img in images])
        print(f"Created ICO: {ico_path}")

    except Exception as e:
        print(f"Error creating ICO file {ico_path}: {e}")

def create_icon_set(icons_dir, base_name, color, description):
    """Create both regular and dev versions of an icon with the same thermometer design"""
    print(f"\nCreating {description} icons...")

    # Generate both regular and dev versions using the same function
    versions = [
        (base_name, (0, 0, 0, 0), "Regular"),  # Transparent background
        (f"{base_name}-dev", (0, 0, 0, 255), "Dev")  # Black background
    ]

    for icon_name, bg_color, version_type in versions:
        print(f"  {version_type}: {icon_name}")

        # Create icon using the same function, just different background
        img = create_thermometer_icon(32, color, bg_color, None)

        # Save as PNG
        png_path = os.path.join(icons_dir, f"{icon_name}.png")
        img.save(png_path, 'PNG')
        print(f"    PNG: {png_path}")

        # Convert to ICO
        ico_path = os.path.join(icons_dir, f"{icon_name}.ico")
        create_ico_file(png_path, ico_path)

def main():
    # Create icons directory if it doesn't exist
    icons_dir = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'icons')
    os.makedirs(icons_dir, exist_ok=True)

    # Icon configurations: (base_name, color, description)
    icon_configs = [
        ('thermometer-cool', (0, 255, 0, 255), 'Green (Cool)'),
        ('thermometer-warm', (255, 165, 0, 255), 'Orange (Warm)'),
        ('thermometer-hot', (255, 0, 0, 255), 'Red (Hot)')
    ]

    print("Creating all thermometer icons...")

    # Create icon sets using parameterized function
    for base_name, color, description in icon_configs:
        create_icon_set(icons_dir, base_name, color, description)

    print("\n✅ All icons created successfully!")
    print("\nIcon usage:")
    print("• Debug builds (`cargo build`): Uses -dev icons with black background")
    print("• Release builds (`cargo build --release`): Uses regular icons with transparent background")
    print("• All icons use the same thermometer design for consistency")

if __name__ == "__main__":
    main()