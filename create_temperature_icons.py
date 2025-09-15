#!/usr/bin/env python3
"""
Script to create different colored thermometer icons for different temperature states
"""
import struct

def create_colored_ico(color_bgra, filename):
    """Create a colored 16x16 ICO file with thermometer pattern"""
    # ICO header
    ico_header = struct.pack('<HHH', 0, 1, 1)  # Reserved, Type, Count

    # Image directory entry
    width = 16
    height = 16
    colors = 0  # 0 = no color palette
    reserved = 0
    planes = 1
    bpp = 32  # 32 bits per pixel
    size = 40 + (width * height * 4)  # Header + image data
    offset = 22  # Start of image data

    dir_entry = struct.pack('<BBBBHHLL', width, height, colors, reserved, planes, bpp, size, offset)

    # BITMAPINFOHEADER
    header_size = 40
    bmp_header = struct.pack('<LLLHHLLLLLL',
        header_size,    # biSize
        width,          # biWidth
        height * 2,     # biHeight (doubled for ICO)
        1,              # biPlanes
        bpp,            # biBitCount
        0,              # biCompression
        width * height * 4,  # biSizeImage
        0,              # biXPelsPerMeter
        0,              # biYPelsPerMeter
        0,              # biClrUsed
        0               # biClrImportant
    )

    # Create thermometer pattern in RGBA
    image_data = bytearray()
    for y in range(height):
        for x in range(width):
            # Create a simple thermometer shape
            if x == 8 and y < 12:  # Vertical line (thermometer tube)
                # Main thermometer color
                image_data.extend(color_bgra)
            elif x in [7, 9] and y < 12:  # Thermometer outline
                # Dark outline
                image_data.extend([0, 0, 0, 255])
            elif 6 <= x <= 10 and 12 <= y <= 14:  # Thermometer bulb
                # Bulb color (same as tube)
                image_data.extend(color_bgra)
            elif 5 <= x <= 11 and 11 <= y <= 15:  # Bulb outline
                # Dark outline around bulb
                image_data.extend([0, 0, 0, 255])
            else:
                # Transparent background
                image_data.extend([0, 0, 0, 0])

    # Combine all parts
    ico_data = ico_header + dir_entry + bmp_header + image_data

    with open(filename, "wb") as f:
        f.write(ico_data)
    print(f"Created {filename}")

if __name__ == "__main__":
    # Create different temperature state icons
    # BGRA format (Blue, Green, Red, Alpha)

    # Green for cool temperatures
    create_colored_ico([0, 255, 0, 255], "thermometer-cool.ico")

    # Yellow/Orange for warm temperatures
    create_colored_ico([0, 165, 255, 255], "thermometer-warm.ico")

    # Red for hot temperatures
    create_colored_ico([0, 0, 255, 255], "thermometer-hot.ico")

    print("Created temperature state icons!")
    print("- thermometer-cool.ico (green) for cool temperatures")
    print("- thermometer-warm.ico (orange) for warm temperatures")
    print("- thermometer-hot.ico (red) for hot temperatures")