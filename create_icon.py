#!/usr/bin/env python3
"""
Simple script to create a basic ICO file for the system tray
"""
import struct

def create_simple_ico():
    # Create a minimal valid 16x16 ICO file
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

    # Create a simple thermometer pattern in RGBA
    image_data = bytearray()
    for y in range(height):
        for x in range(width):
            # Create a simple thermometer shape
            if x == 8 and y < 12:  # Vertical line (thermometer tube)
                # Red thermometer
                image_data.extend([0, 0, 255, 255])  # BGRA format
            elif x in [7, 9] and y < 12:  # Thermometer outline
                # Dark outline
                image_data.extend([0, 0, 0, 255])
            elif 6 <= x <= 10 and 12 <= y <= 14:  # Thermometer bulb
                # Red bulb
                image_data.extend([0, 0, 255, 255])
            elif 5 <= x <= 11 and 11 <= y <= 15:  # Bulb outline
                # Dark outline around bulb
                image_data.extend([0, 0, 0, 255])
            else:
                # Transparent background
                image_data.extend([0, 0, 0, 0])

    # Combine all parts
    ico_data = ico_header + dir_entry + bmp_header + image_data

    return ico_data

if __name__ == "__main__":
    ico_data = create_simple_ico()
    with open("icon.ico", "wb") as f:
        f.write(ico_data)
    print("Created icon.ico file")