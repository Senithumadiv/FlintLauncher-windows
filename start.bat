@echo off
chcp 65001 >nul
title Flint Launcher

echo Starting Flint Launcher in system tray...
echo Config location: %APPDATA%\Flint
echo.
echo Flint will run in the background. Right-click the system tray icon for options.
echo Press Ctrl+C to stop Flint.

:: Run Flint in tray mode (hidden)
start /B flint.exe --tray

echo Flint is now running in system tray.
echo You can access it by right-clicking the tray icon.
pause