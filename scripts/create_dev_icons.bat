@echo off
echo Installing Python dependencies...
pip install -r requirements.txt

echo.
echo Creating all icons (regular + dev)...
python create_icons.py

echo.
echo Done! Press any key to exit...
pause >nul