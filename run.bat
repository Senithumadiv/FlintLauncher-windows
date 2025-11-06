@echo off
echo Building Flint Launcher for Windows...
cargo build --release
if %errorlevel% equ 0 (
    echo Build successful!
    echo Running Flint Launcher...
    target\release\flint.exe
) else (
    echo Build failed!
    pause
)