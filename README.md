Flint Launcher - Quick Start Guide
==================================

🎯 Features:
- Lightning fast app launcher
- System tray integration  
- Customizable hotkeys
- Theme support
- File search, emoji search, calculator, and more!

🚀 How to Use:

1. INSTALLATION:
   - Run "install.bat" as Administrator for full installation
   - Or run "start.bat" for portable use

2. SYSTEM TRAY:
   - After installation, Flint runs in system tray
   - Right-click the tray icon for options:
     * "Show Launcher" - Open the main launcher (Alt+Space)
     * "Settings" - Configure hotkeys and options
     * "Open Config Folder" - Edit theme.conf manually
     * "Exit" - Close Flint completely

3. CONFIGURATION:
   Config files are located at: %APPDATA%\Flint\
   
   - theme.conf - Customize appearance
   - hotkeys.conf - Configure keyboard shortcuts

4. DEFAULT HOTKEYS:
   - Launcher: Alt+Space
   - Settings: Alt+Shift+S

5. SEARCH FEATURES:
   - Apps: Just type the app name
   - Files: file:filename
   - Emojis: e:smile
   - Web: @search term
   - Commands: $command
   - Calculator: 2+2 (no prefix needed)
   - Currency: 100 USD to EUR

🛠️ Troubleshooting:
- If Flint doesn't start, check if another instance is running
- Run "StartFlint.bat" to start manually
- Delete %TEMP%\flint.lock if you get "already running" error
- Run "uninstall.bat" to remove completely

🎨 Theme Customization:
Edit %APPDATA%\Flint\theme.conf to change colors and fonts:
```conf
background=#2d2d30
text_color=#ffffff
selection_bg=#0078d4
selection_text=#ffffff
border_color=#3e3e42
highlight_color=#0078d4
font_size=16
font_family=Segoe UI
border_radius=2
```
Enjoy! 🚀