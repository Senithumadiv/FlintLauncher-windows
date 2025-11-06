@echo off
chcp 65001 >nul
title Flint Launcher Uninstaller

echo 🗑️ Flint Launcher Uninstaller
echo =============================
echo.

net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [!] Please run as Administrator
    echo Right-click -> Run as Administrator
    pause
    exit /b 1
)

set INSTALL_DIR=%PROGRAMFILES%\FlintLauncher

echo [1/4] Stopping Flint Launcher...
taskkill /F /IM flint.exe >nul 2>&1
wscript //B //Nologo //T:10 "%INSTALL_DIR%\StartFlint.vbs" >nul 2>&1

echo [2/4] Removing startup entry...
reg delete "HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run" /v "FlintLauncher" /f >nul 2>&1

echo [3/4] Removing shortcuts...
set START_MENU=%APPDATA%\Microsoft\Windows\Start Menu\Programs
del "%START_MENU%\Flint Launcher.lnk" >nul 2>&1

echo [4/4] Removing files...
if exist "%INSTALL_DIR%" (
    rmdir /S /Q "%INSTALL_DIR%" >nul 2>&1
    if exist "%INSTALL_DIR%" (
        echo [!] Could not remove all files. Please delete manually: %INSTALL_DIR%
    ) else (
        echo ✅ Files removed successfully.
    )
)

echo.
echo ✅ Uninstallation complete!
echo.
echo Note: Your config files in %APPDATA%\Flint were kept.
echo To remove them completely, delete that folder manually.
echo.
pause