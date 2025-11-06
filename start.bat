@echo off
chcp 65001 >nul
title Flint Launcher

tasklist /FI "IMAGENAME eq flint.exe" 2>NUL | find /I "flint.exe" >NUL
IF "%ERRORLEVEL%"=="0" (
    echo Flint is already running in system tray.
    echo Right-click the tray icon to access options.
    pause
    exit /b 0
)

echo Starting Flint Launcher...
echo.
echo Config location: %APPDATA%\Flint
echo.
flint.exe --tray