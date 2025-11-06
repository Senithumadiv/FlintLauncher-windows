@echo off
chcp 65001 >nul
title Building Flint Launcher

echo 🔨 Building Flint Launcher Release...
echo.

echo [1/3] Building executable...
cargo build --release

if %errorlevel% neq 0 (
    echo [!] Build failed!
    pause
    exit /b 1
)

echo [2/3] Copying files...
if not exist "release" mkdir "release"
copy "target\release\flintlauncher-windows.exe" "release\flint.exe" >nul
copy "install.bat" "release\" >nul
copy "StartFlint.bat" "release\" >nul
copy "uninstall.bat" "release\" >nul
copy "README.txt" "release\" >nul

echo [3/3] Creating release package...
powershell -Command "Compress-Archive -Path 'release\*' -DestinationPath 'FlintLauncher-Release.zip' -Force"

echo.
echo ✅ Build complete!
echo.
echo Files are in the 'release' folder.
echo Distribution package: FlintLauncher-Release.zip
echo.
dir release
echo.
pause