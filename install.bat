@echo off
chcp 65001 >nul
title Flint Launcher Installer

echo ⚡ Flint Launcher Installer
echo ===========================
echo.

net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [!] Please run as Administrator
    echo Right-click -> Run as Administrator
    pause
    exit /b 1
)

set INSTALL_DIR=%PROGRAMFILES%\FlintLauncher
echo [1/4] Creating installation directory...
if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"

echo [2/4] Copying files...
copy "flint.exe" "%INSTALL_DIR%\flint.exe" >nul

echo [3/4] Creating shortcuts...
set START_MENU=%APPDATA%\Microsoft\Windows\Start Menu\Programs
powershell -Command "$WshShell = New-Object -comObject WScript.Shell; $Shortcut = $WshShell.CreateShortcut('%START_MENU%\Flint Launcher.lnk'); $Shortcut.TargetPath = '%INSTALL_DIR%\flint.exe'; $Shortcut.Save()"

echo [4/4] Setting up auto-start...
echo Set WshShell = CreateObject("WScript.Shell") > "%INSTALL_DIR%\StartFlint.vbs"
echo WshShell.Run "%INSTALL_DIR%\flint.exe --tray", 0, False >> "%INSTALL_DIR%\StartFlint.vbs"

reg add "HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run" /v "FlintLauncher" /t REG_SZ /d "wscript.exe \"%INSTALL_DIR%\StartFlint.vbs\"" /f

echo.
echo ✅ Installation complete!
echo.
echo Flint Launcher will start automatically on system startup.
echo You can find it in the system tray (notification area).
echo.
echo Config location: %APPDATA%\Flint
echo.
pause