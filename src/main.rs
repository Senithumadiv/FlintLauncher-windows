use eframe::egui;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use rayon::prelude::*;
use reqwest;
use serde::Deserialize;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use dirs;
use std::sync::mpsc;
use std::thread;

#[derive(Clone, Debug)]
struct HotkeyConfig {
    launcher_key: String,
    settings_key: String,
    enabled: bool,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            launcher_key: "Alt+Space".to_string(),
            settings_key: "Alt+Shift+S".to_string(),
            enabled: true,
        }
    }
}

impl HotkeyConfig {
    fn load() -> Self {
        let config_path = get_config_dir().join("hotkeys.conf");
        
        if let Ok(content) = fs::read_to_string(&config_path) {
            let mut config = Self::default();
            for line in content.lines() {
                if line.starts_with('#') || line.is_empty() {
                    continue;
                }
                if line.starts_with("launcher_key=") {
                    config.launcher_key = line.replace("launcher_key=", "").trim().to_string();
                } else if line.starts_with("settings_key=") {
                    config.settings_key = line.replace("settings_key=", "").trim().to_string();
                } else if line.starts_with("enabled=") {
                    config.enabled = line.replace("enabled=", "").trim() == "true";
                }
            }
            config
        } else {
            Self::default()
        }
    }
    
    fn save(&self) {
        let config_dir = get_config_dir();
        let _ = fs::create_dir_all(&config_dir);
        
        let content = format!(
            "# Flint Launcher Hotkey Configuration\n\
             # Format: Key+Modifier (e.g., Space+Alt, C+Ctrl+Shift)\n\
             # Supported modifiers: Ctrl, Alt, Shift, Super/Win, Cmd\n\n\
             launcher_key={}\n\
             settings_key={}\n\
             enabled={}\n",
            self.launcher_key,
            self.settings_key,
            self.enabled
        );
        
        let _ = fs::write(config_dir.join("hotkeys.conf"), content);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum AppMode {
    Launcher,
    Settings,
    Hidden,
}

struct Theme {
    background: String,
    text_color: String,
    selection_bg: String,
    selection_text: String,
    border_color: String,
    font_size: f32,
    border_radius: f32,
    font_family: String,
    highlight_color: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: "#2d2d30".to_string(),
            text_color: "#ffffff".to_string(),
            selection_bg: "#0078d4".to_string(),
            selection_text: "#ffffff".to_string(),
            border_color: "#3e3e42".to_string(),
            font_size: 16.0,
            border_radius: 2.0,
            font_family: "Segoe UI".to_string(),
            highlight_color: "#0078d4".to_string(),
        }
    }
}

impl Theme {
    fn load_from_config() -> Self {
        let config_dir = get_config_dir();
        let theme_path = config_dir.join("theme.conf");
        
        if !theme_path.exists() {
            create_default_theme(&theme_path);
            return Self::default();
        }
        
        let mut theme = Self::default();
        
        if let Ok(content) = fs::read_to_string(&theme_path) {
            for line in content.lines() {
                let parts: Vec<&str> = line.splitn(2, '=').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim();
                    let value = parts[1].trim();
                    
                    match key {
                        "background" => theme.background = value.to_string(),
                        "text_color" => theme.text_color = value.to_string(),
                        "selection_bg" => theme.selection_bg = value.to_string(),
                        "selection_text" => theme.selection_text = value.to_string(),
                        "border_color" => theme.border_color = value.to_string(),
                        "highlight_color" => theme.highlight_color = value.to_string(),
                        "font_size" => {
                            if let Ok(size) = value.parse() {
                                theme.font_size = size;
                            }
                        }
                        "border_radius" => {
                            if let Ok(radius) = value.parse() {
                                theme.border_radius = radius;
                            }
                        }
                        "font_family" => theme.font_family = value.to_string(),
                        _ => {}
                    }
                }
            }
        }
        
        theme
    }
    
    fn hex_to_rgb(&self, hex: &str) -> [f32; 3] {
        let hex = hex.trim_start_matches('#');
        if hex.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                return [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0];
            }
        }
        [0.0, 0.0, 0.0]
    }
}

#[derive(Clone)]
enum ResultType {
    App(AppEntry),
    Calculator(String),
    Command(String),
    WebSearch(String),
    Url(String),
    File(PathBuf),
    Emoji(String, String),
    Currency(String, String, f64),
}

#[derive(Clone)]
struct AppEntry {
    name: String,
    desktop_id: String,
    exec_command: String,
    match_indices: Vec<usize>,
}

struct AnimationState {
    progress: f32,
    start_time: Instant,
    duration: Duration,
    animation_type: AnimationType,
}

impl AnimationState {
    fn new(duration: Duration, animation_type: AnimationType) -> Self {
        Self {
            progress: 0.0,
            start_time: Instant::now(),
            duration,
            animation_type,
        }
    }
    
    fn update(&mut self) -> bool {
        let elapsed = self.start_time.elapsed();
        self.progress = (elapsed.as_millis() as f32 / self.duration.as_millis() as f32).min(1.0);
        self.progress < 1.0
    }
    
    fn ease_out(&self) -> f32 {
        1.0 - (1.0 - self.progress).powf(2.0)
    }
}

#[derive(Clone, Copy)]
enum AnimationType {
    FadeIn,
    SlideDown,
}

struct FlintApp {
    query: String,
    results: Vec<ResultType>,
    items: Vec<AppEntry>,
    selected: usize,
    should_close: bool,
    has_focused: bool,
    theme: Theme,
    _lock_file: File,
    window_animation: AnimationState,
    result_animations: Vec<AnimationState>,
    runtime: tokio::runtime::Runtime,
    app_mode: AppMode,
    hotkey_config: Arc<Mutex<HotkeyConfig>>,
    temp_launcher_key: String,
    temp_settings_key: String,
    temp_enabled: bool,
    status_message: String,
    status_color: egui::Color32,
    message_time: Instant,
    tray_sender: mpsc::Sender<TrayMessage>,
}

#[derive(Debug)]
enum TrayMessage {
    ShowLauncher,
    ShowSettings,
    Exit,
}

impl FlintApp {
    fn new() -> Result<Self, String> {
        let lock_file = acquire_lock()?;
        let items = scan_apps();
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create async runtime: {}", e))?;
        
        let (tray_sender, tray_receiver) = mpsc::channel();
        
        start_tray_thread(tray_receiver);
        
        Ok(Self {
            query: String::new(),
            results: Vec::new(),
            items,
            selected: 0,
            should_close: false,
            has_focused: false,
            theme: Theme::load_from_config(),
            _lock_file: lock_file,
            window_animation: AnimationState::new(Duration::from_millis(300), AnimationType::FadeIn),
            result_animations: Vec::new(),
            runtime,
            app_mode: AppMode::Launcher,
            hotkey_config: Arc::new(Mutex::new(HotkeyConfig::load())),
            temp_launcher_key: String::new(),
            temp_settings_key: String::new(),
            temp_enabled: false,
            status_message: String::new(),
            status_color: egui::Color32::GREEN,
            message_time: Instant::now(),
            tray_sender,
        })
    }
    
    fn update_result_animations(&mut self) {
        if self.result_animations.len() != self.results.len() {
            self.result_animations = self.results.iter()
                .enumerate()
                .map(|(i, _)| {
                    let delay = Duration::from_millis((i * 40) as u64).min(Duration::from_millis(200));
                    let mut anim = AnimationState::new(Duration::from_millis(250), AnimationType::SlideDown);
                    anim.start_time += delay;
                    anim
                })
                .collect();
        }
        
        for anim in &mut self.result_animations {
            anim.update();
        }
    }
    
    fn get_result_offset(&self, index: usize) -> f32 {
        self.result_animations.get(index)
            .map(|anim| {
                match anim.animation_type {
                    AnimationType::SlideDown => (1.0 - anim.ease_out()) * -30.0,
                    _ => 0.0,
                }
            })
            .unwrap_or(0.0)
    }
    
    fn get_result_alpha(&self, index: usize) -> f32 {
        self.result_animations.get(index)
            .map(|anim| anim.ease_out())
            .unwrap_or(1.0)
    }
    
    fn handle_tray_messages(&mut self) {
        if let Ok(message) = self.tray_sender.try_recv() {
            match message {
                TrayMessage::ShowLauncher => {
                    self.app_mode = AppMode::Launcher;
                    self.should_close = false;
                }
                TrayMessage::ShowSettings => {
                    self.app_mode = AppMode::Settings;
                    self.should_close = false;
                }
                TrayMessage::Exit => {
                    self.should_close = true;
                }
            }
        }
    }
}

impl eframe::App for FlintApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_tray_messages();
        
        if self.should_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        
        match self.app_mode {
            AppMode::Settings => self.render_settings(ctx),
            AppMode::Launcher => self.render_launcher(ctx),
            AppMode::Hidden => {
                ctx.request_repaint_after(Duration::from_secs(1));
            }
        }
    }
}

fn start_tray_thread(receiver: mpsc::Receiver<TrayMessage>) {
    thread::spawn(move || {
        #[cfg(target_os = "windows")]
        {
            use tray_item::TrayItem;
            
            let mut tray = TrayItem::new("Flint Launcher", "").unwrap();
            
            tray.add_label("Flint Launcher").unwrap();
            tray.inner_mut().add_separator().unwrap();
            
            tray.add_menu_item("Show Launcher", || {
                if let Ok(sender) = receiver.try_recv() {
                    let _ = sender.send(TrayMessage::ShowLauncher);
                }
            }).unwrap();
            
            tray.add_menu_item("Settings", || {
                if let Ok(sender) = receiver.try_recv() {
                    let _ = sender.send(TrayMessage::ShowSettings);
                }
            }).unwrap();
            
            tray.inner_mut().add_separator().unwrap();
            
            tray.add_menu_item("Open Config Folder", || {
                let config_dir = get_config_dir();
                let _ = open_file(&config_dir);
            }).unwrap();
            
            tray.inner_mut().add_separator().unwrap();
            
            tray.add_menu_item("Exit", || {
                if let Ok(sender) = receiver.try_recv() {
                    let _ = sender.send(TrayMessage::Exit);
                }
            }).unwrap();
        }
        
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    });
}

impl FlintApp {
    fn render_launcher(&mut self, ctx: &egui::Context) {
        let _window_animating = self.window_animation.update();
        self.update_result_animations();
        
        let still_animating = self.window_animation.progress < 1.0 || self.result_animations.iter().any(|a| a.progress < 1.0);
        
        if still_animating {
            ctx.request_repaint();
        }
        
        if ctx.input(|i| i.pointer.any_click()) {
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                let rect = ctx.screen_rect();
                if !rect.contains(pos) {
                    self.should_close = true;
                }
            }
        }

        if self.has_focused {
            if let Some(focused) = ctx.input(|i| i.viewport().focused) {
                if !focused {
                    self.should_close = true;
                }
            }
        }
        
        if self.should_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        let window_alpha = self.window_animation.ease_out();
        
        let window_width = 600.0;
        let search_box_height = 50.0;
        let result_item_height = 44.0;
        let max_visible_results = 8;
        let visible_results = self.results.len().min(max_visible_results);
        let results_height = if visible_results > 0 {
            (visible_results as f32 * result_item_height) + 10.0
        } else {
            0.0
        };
        let total_height = search_box_height + results_height;
        
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
            window_width,
            total_height
        )));

        let bg_rgb = self.theme.hex_to_rgb(&self.theme.background);
        let border_rgb = self.theme.hex_to_rgb(&self.theme.border_color);
        
        let bg_color = egui::Color32::from_rgba_premultiplied(
            (bg_rgb[0] * 255.0 * window_alpha) as u8,
            (bg_rgb[1] * 255.0 * window_alpha) as u8,
            (bg_rgb[2] * 255.0 * window_alpha) as u8,
            (window_alpha * 255.0) as u8,
        );
        
        let border_color = egui::Color32::from_rgba_premultiplied(
            (border_rgb[0] * 255.0 * window_alpha) as u8,
            (border_rgb[1] * 255.0 * window_alpha) as u8,
            (border_rgb[2] * 255.0 * window_alpha) as u8,
            (window_alpha * 255.0) as u8,
        );
        
        egui::CentralPanel::default()
            .frame(egui::Frame::none()
                .fill(bg_color)
                .stroke(egui::Stroke::new(1.0, border_color))
                .rounding(self.theme.border_radius)
                .shadow(egui::epaint::Shadow {
                    offset: egui::vec2(0.0, 2.0),
                    blur: 8.0,
                    spread: 0.0,
                    color: egui::Color32::from_rgba_premultiplied(0, 0, 0, (50.0 * window_alpha) as u8),
                }))
            .show(ctx, |ui| {
                ui.set_min_width(window_width);
                ui.set_max_width(window_width);
                
                ui.vertical(|ui| {
                    let text_rgb = self.theme.hex_to_rgb(&self.theme.text_color);
                    
                    ui.add_space(5.0);
                    ui.add_space(5.0);
                    
                    let search_text_color = egui::Color32::from_rgba_premultiplied(
                        (text_rgb[0] * 255.0 * window_alpha) as u8,
                        (text_rgb[1] * 255.0 * window_alpha) as u8,
                        (text_rgb[2] * 255.0 * window_alpha) as u8,
                        (window_alpha * 255.0) as u8,
                    );
                    
                    ui.horizontal(|ui| {
                        ui.add_space(15.0);
                        
                        let response = ui.add_sized(
                            [window_width - 30.0, 30.0],
                            egui::TextEdit::singleline(&mut self.query)
                                .hint_text("Search...")
                                .frame(false)
                                .text_color(search_text_color)
                                .font(egui::FontId::proportional(20.0))
                                .id(egui::Id::new("search_field"))
                        );

                        if !self.has_focused {
                            ui.ctx().memory_mut(|mem| mem.request_focus(response.id));
                            self.has_focused = true;
                        }
                        
                        ui.add_space(15.0);
                    });
                    
                    if !self.results.is_empty() {
                        ui.add_space(5.0);
                        let separator_alpha = (window_alpha * 255.0) as u8;
                        let border_rgb = self.theme.hex_to_rgb(&self.theme.border_color);
                        let separator_color = egui::Color32::from_rgba_premultiplied(
                            (border_rgb[0] * 255.0) as u8,
                            (border_rgb[1] * 255.0) as u8,
                            (border_rgb[2] * 255.0) as u8,
                            separator_alpha
                        );
                        
                        let separator_height = 1.0;
                        let available_width = ui.available_width();
                        let separator_rect = egui::Rect::from_min_size(
                            ui.cursor().min,
                            egui::vec2(available_width, separator_height)
                        );
                        ui.painter().rect_filled(separator_rect, 0.0, separator_color);
                        
                        ui.add_space(separator_height + 5.0);
                    }

                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.should_close = true;
                    }

                    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) && !self.results.is_empty() {
                        self.selected = (self.selected + 1) % self.results.len();
                    }

                    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && !self.results.is_empty() {
                        if self.selected == 0 {
                            self.selected = self.results.len() - 1;
                        } else {
                            self.selected -= 1;
                        }
                    }

                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.results.is_empty() {
                        if let Some(result) = self.results.get(self.selected) {
                            execute_result(result);
                            self.should_close = true;
                        }
                    }

                    self.results.clear();

                    if !self.query.is_empty() {
                        if self.query.starts_with("file:") {
                            let file_query = &self.query[5..].trim();
                            if !file_query.is_empty() {
                                let file_results = search_files(file_query);
                                for path in file_results {
                                    self.results.push(ResultType::File(path));
                                }
                            } else {
                                self.results.push(ResultType::Command("Search files...".to_string()));
                            }
                        }
                        else if self.query.starts_with("e:") {
                            let emoji_query = &self.query[2..].trim();
                            if !emoji_query.is_empty() {
                                let emoji_results = search_emojis(emoji_query);
                                for (name, emoji) in emoji_results {
                                    self.results.push(ResultType::Emoji(name, emoji));
                                }
                            } else {
                                self.results.push(ResultType::Command("Search emojis...".to_string()));
                            }
                        }
                        else if let Some((from, to, result)) = self.runtime.block_on(convert_currency_online(&self.query)) {
                            self.results.push(ResultType::Currency(from, to, result));
                        }
                        else if looks_like_url(&self.query) {
                            let url = if self.query.contains("://") {
                                self.query.clone()
                            } else {
                                format!("https://{}", self.query)
                            };
                            self.results.push(ResultType::Url(url));
                        }
                        else if is_calculation(&self.query) {
                            let expr = self.query.trim();
                            if !expr.is_empty() {
                                match meval::eval_str(expr) {
                                    Ok(result) => {
                                        self.results.push(ResultType::Calculator(result.to_string()));
                                    }
                                    Err(_) => {}
                                }
                            }
                        }
                        else if self.query.starts_with('$') {
                            let cmd = &self.query[1..].trim();
                            if !cmd.is_empty() {
                                self.results.push(ResultType::Command(cmd.to_string()));
                            } else {
                                self.results.push(ResultType::Command("Enter command...".to_string()));
                            }
                        }
                        else if self.query.starts_with('@') {
                            let search = &self.query[1..].trim();
                            if !search.is_empty() {
                                self.results.push(ResultType::WebSearch(search.to_string()));
                            } else {
                                self.results.push(ResultType::Command("Search the web...".to_string()));
                            }
                        }
                        
                        if self.results.is_empty() {
                            let matcher = SkimMatcherV2::default();
                            let query = self.query.clone();
                            
                            let mut scored_results: Vec<(i64, AppEntry)> = self
                                .items
                                .par_iter()
                                .filter_map(|app| {
                                    if let Some((score, indices)) = matcher.fuzzy_indices(&app.name, &query) {
                                        let mut app_with_match = app.clone();
                                        app_with_match.match_indices = indices;
                                        return Some((score + 100, app_with_match));
                                    }
                                    
                                    if let Some((score, _)) = matcher.fuzzy_indices(&app.exec_command, &query) {
                                        let mut app_with_match = app.clone();
                                        app_with_match.match_indices = Vec::new();
                                        return Some((score, app_with_match));
                                    }
                                    
                                    None
                                })
                                .collect();
                            
                            scored_results.sort_by(|a, b| b.0.cmp(&a.0));
                            
                            for (_, app) in scored_results.into_iter().take(max_visible_results) {
                                self.results.push(ResultType::App(app));
                            }
                            
                            if self.results.is_empty() {
                                self.results.push(ResultType::WebSearch(query));
                            }
                        }
                        
                        if self.selected >= self.results.len() && !self.results.is_empty() {
                            self.selected = 0;
                        }
                    }

                    if !self.results.is_empty() {
                        egui::ScrollArea::vertical()
                            .max_height(result_item_height * max_visible_results as f32)
                            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                            .show(ui, |ui| {
                                for (i, result) in self.results.iter().enumerate() {
                                    let is_selected = i == self.selected;
                                    let item_alpha = self.get_result_alpha(i);
                                    let item_offset = self.get_result_offset(i);
                                    
                                    let sel_bg_rgb = self.theme.hex_to_rgb(&self.theme.selection_bg);
                                    
                                    let item_bg = if is_selected {
                                        egui::Color32::from_rgba_premultiplied(
                                            (sel_bg_rgb[0] * 255.0 * item_alpha) as u8,
                                            (sel_bg_rgb[1] * 255.0 * item_alpha) as u8,
                                            (sel_bg_rgb[2] * 255.0 * item_alpha) as u8,
                                            (item_alpha * 255.0) as u8,
                                        )
                                    } else {
                                        egui::Color32::TRANSPARENT
                                    };
                                    
                                    ui.add_space(item_offset);
                                    
                                    let item_frame = egui::Frame::none()
                                        .fill(item_bg)
                                        .inner_margin(egui::Margin::symmetric(15.0, 8.0));
                                    
                                    let response = item_frame.show(ui, |ui| {
                                        ui.set_min_height(result_item_height - 16.0);
                                        ui.set_width(window_width);
                                        
                                        ui.horizontal(|ui| {
                                            render_result_item(ui, result, is_selected, &self.theme, item_alpha, &self.query);
                                        });
                                    }).response;
                                    
                                    if is_selected {
                                        response.scroll_to_me(Some(egui::Align::Center));
                                    }
                                    
                                    if response.clicked() {
                                        execute_result(result);
                                    }
                                    
                                    ui.add_space(-item_offset);
                                }
                            });
                    }
                });
        });

        ctx.request_repaint();
    }
    
    fn render_settings(&mut self, ctx: &egui::Context) {
        let config = self.hotkey_config.lock().ok();
        if let Some(cfg) = config {
            self.temp_launcher_key = cfg.launcher_key.clone();
            self.temp_settings_key = cfg.settings_key.clone();
            self.temp_enabled = cfg.enabled;
        }
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("⚙️ Flint Launcher Settings");
            ui.separator();
            
            ui.heading("⌨️ Hotkey Configuration");
            
            ui.label("Launcher Hotkey:");
            ui.text_edit_singleline(&mut self.temp_launcher_key);
            ui.label("Example: Alt+Space, Ctrl+`, Super+Shift+D");
            
            ui.separator();
            
            ui.label("Settings Hotkey:");
            ui.text_edit_singleline(&mut self.temp_settings_key);
            ui.label("Example: Alt+Shift+S");
            
            ui.separator();
            
            ui.checkbox(&mut self.temp_enabled, "Enable Hotkeys");
            
            ui.separator();
            
            if self.message_time.elapsed().as_secs() < 4 {
                ui.colored_label(self.status_color, &self.status_message);
            }
            
            ui.separator();
            
            ui.horizontal(|ui| {
                if ui.button("💾 Save").clicked() {
                    if let Ok(mut config) = self.hotkey_config.lock() {
                        config.launcher_key = self.temp_launcher_key.clone();
                        config.settings_key = self.temp_settings_key.clone();
                        config.enabled = self.temp_enabled;
                        config.save();
                        
                        self.status_message = "✓ Saved! Restart to apply.".to_string();
                        self.status_color = egui::Color32::GREEN;
                        self.message_time = Instant::now();
                    }
                }
                
                if ui.button("🔄 Reset").clicked() {
                    let defaults = HotkeyConfig::default();
                    self.temp_launcher_key = defaults.launcher_key.clone();
                    self.temp_settings_key = defaults.settings_key.clone();
                    self.temp_enabled = defaults.enabled;
                    
                    self.status_message = "Reset to defaults".to_string();
                    self.status_color = egui::Color32::YELLOW;
                    self.message_time = Instant::now();
                }
            });
            
            ui.separator();
            if ui.button("Back to Launcher").clicked() {
                self.app_mode = AppMode::Launcher;
            }
        });
    }
}

fn render_result_item(
    ui: &mut egui::Ui,
    result: &ResultType,
    is_selected: bool,
    theme: &Theme,
    item_alpha: f32,
    query: &str,
) {
    let text_rgb = theme.hex_to_rgb(&theme.text_color);
    let sel_text_rgb = theme.hex_to_rgb(&theme.selection_text);
    
    let color = if is_selected { sel_text_rgb } else { text_rgb };
    let color_val = egui::Color32::from_rgba_premultiplied(
        (color[0] * 255.0 * item_alpha) as u8,
        (color[1] * 255.0 * item_alpha) as u8,
        (color[2] * 255.0 * item_alpha) as u8,
        (item_alpha * 255.0) as u8,
    );
    
    match result {
        ResultType::App(app) => {
            render_highlighted_text(ui, &app.name, &app.match_indices, is_selected, theme, item_alpha);
        }
        ResultType::Calculator(res) => {
            ui.label(
                egui::RichText::new(format!("🧮 {} = {}", query, res))
                    .color(color_val)
                    .size(theme.font_size)
            );
        }
        ResultType::Command(cmd) => {
            ui.label(
                egui::RichText::new(format!("💻 {}", cmd))
                    .color(color_val)
                    .size(theme.font_size)
            );
        }
        ResultType::WebSearch(search_query) => {
            ui.label(
                egui::RichText::new(format!("🔍 Search DuckDuckGo: {}", search_query))
                    .color(color_val)
                    .size(theme.font_size)
            );
        }
        ResultType::Url(url) => {
            ui.label(
                egui::RichText::new(format!("🌐 Open: {}", url))
                    .color(color_val)
                    .size(theme.font_size)
            );
        }
        ResultType::File(path) => {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("Unknown");
            let parent_dir = path.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("");
            ui.label(
                egui::RichText::new(format!("📄 {} ({})", file_name, parent_dir))
                    .color(color_val)
                    .size(theme.font_size)
            );
        }
        ResultType::Emoji(name, emoji) => {
            ui.label(
                egui::RichText::new(format!("{} :{}", emoji, name))
                    .color(color_val)
                    .size(theme.font_size)
            );
        }
        ResultType::Currency(from, to, result) => {
            ui.label(
                egui::RichText::new(format!("💱 {} {} = {:.2} {} (Live)", query, from, result, to))
                    .color(color_val)
                    .size(theme.font_size)
            );
        }
    }
}

fn execute_result(result: &ResultType) {
    match result {
        ResultType::App(app) => launch_app(&app.exec_command),
        ResultType::Calculator(res) => copy_to_clipboard(res),
        ResultType::Command(cmd) => execute_command(cmd),
        ResultType::WebSearch(query) => open_web_search(query),
        ResultType::Url(url) => open_url(url),
        ResultType::File(path) => open_file(path),
        ResultType::Emoji(_, emoji) => copy_to_clipboard(emoji),
        ResultType::Currency(_, _, result) => copy_to_clipboard(&result.to_string()),
    }
}

fn render_highlighted_text(
    ui: &mut egui::Ui,
    text: &str,
    match_indices: &[usize],
    is_selected: bool,
    theme: &Theme,
    alpha: f32,
) {
    let normal_color = if is_selected {
        theme.hex_to_rgb(&theme.selection_text)
    } else {
        theme.hex_to_rgb(&theme.text_color)
    };
    
    let highlight_color = theme.hex_to_rgb(&theme.highlight_color);
    
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        
        for (i, ch) in text.chars().enumerate() {
            let base_color = if match_indices.contains(&i) {
                highlight_color
            } else {
                normal_color
            };
            
            let color = egui::Color32::from_rgba_premultiplied(
                (base_color[0] * 255.0 * alpha) as u8,
                (base_color[1] * 255.0 * alpha) as u8,
                (base_color[2] * 255.0 * alpha) as u8,
                (alpha * 255.0) as u8,
            );
            
            ui.label(
                egui::RichText::new(ch.to_string())
                    .color(color)
                    .font(egui::FontId::proportional(theme.font_size))
            );
        }
    });
}

#[derive(Debug, Deserialize)]
struct ExchangeRatesResponse {
    rates: std::collections::HashMap<String, f64>,
}

fn is_calculation(query: &str) -> bool {
    let trimmed = query.trim();
    
    let has_operator = trimmed.contains('+') || 
                      trimmed.contains('-') || 
                      trimmed.contains('*') || 
                      trimmed.contains('/') ||
                      trimmed.contains('%') ||
                      trimmed.contains('^');
    
    let has_numbers = trimmed.chars().any(|c| c.is_ascii_digit());
    
    let has_letters = trimmed.chars().any(|c| c.is_ascii_alphabetic() && c != 'e' && c != 'E' && c != 'p' && c != 'P' && c != 'i' && c != 'I');
    
    let reasonable_length = trimmed.len() >= 2 && trimmed.len() <= 50;
    
    has_operator && has_numbers && !has_letters && reasonable_length
}

fn normalize_currency_code(code: &str) -> Option<String> {
    let code_lower = code.to_lowercase();
    let result = match code_lower.as_str() {
        "usd" | "dollar" | "dollars" => "USD",
        "eur" | "euro" | "euros" => "EUR", 
        "gbp" | "pound" | "pounds" | "sterling" => "GBP",
        "jpy" | "yen" => "JPY",
        "cad" | "canadian dollar" => "CAD",
        "aud" | "australian dollar" => "AUD",
        "chf" | "swiss franc" => "CHF",
        "cny" | "yuan" | "renminbi" => "CNY",
        "inr" | "rupee" | "rupees" => "INR",
        _ if code.len() == 3 => {
            return Some(code.to_uppercase());
        }
        _ => return None,
    };
    Some(result.to_string())
}

async fn convert_currency_online(query: &str) -> Option<(String, String, f64)> {
    let parts: Vec<&str> = query.split_whitespace().collect();
    
    if parts.len() >= 3 {
        let mut amount_str = parts[0];
        let mut from_currency_str = parts[1];
        let mut to_currency_str = parts.get(2).copied().unwrap_or("");
        
        if parts[0].to_lowercase() == "convert" && parts.len() >= 4 {
            amount_str = parts[1];
            from_currency_str = parts[2];
            to_currency_str = parts.get(3).copied().unwrap_or("");
        }
        
        if parts.len() >= 4 && parts[2].to_lowercase() == "to" {
            to_currency_str = parts[3];
        } else if parts.len() >= 4 && parts[0].to_lowercase() == "convert" && parts[3].to_lowercase() == "to" {
            to_currency_str = parts.get(4).copied().unwrap_or("");
        }
        
        if to_currency_str.is_empty() {
            return None;
        }
        
        if let (Ok(amount), Some(from_currency), Some(to_currency)) = (
            amount_str.parse::<f64>(),
            normalize_currency_code(from_currency_str),
            normalize_currency_code(to_currency_str),
        ) {
            if from_currency == to_currency {
                return Some((from_currency.to_string(), to_currency.to_string(), amount));
            }
            
            let client = reqwest::Client::new();
            let url = format!("https://api.exchangerate-api.com/v4/latest/{}", from_currency);
            
            match client.get(&url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        if let Ok(exchange_data) = response.json::<ExchangeRatesResponse>().await {
                            if let Some(rate) = exchange_data.rates.get(&to_currency) {
                                let converted = amount * rate;
                                return Some((from_currency.to_string(), to_currency.to_string(), converted));
                            }
                        }
                    }
                }
                Err(_) => {
                    let fallback_url = format!("https://api.frankfurter.app/latest?from={}", from_currency);
                    if let Ok(fallback_response) = client.get(&fallback_url).send().await {
                        if fallback_response.status().is_success() {
                            if let Ok(exchange_data) = fallback_response.json::<ExchangeRatesResponse>().await {
                                if let Some(rate) = exchange_data.rates.get(&to_currency) {
                                    let converted = amount * rate;
                                    return Some((from_currency.to_string(), to_currency.to_string(), converted));
                                }
                            }
                        }
                    }
                    return None;
                }
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn copy_to_clipboard(text: &str) {
    let _ = Command::new("cmd")
        .args(&["/C", &format!("echo {} | clip", text)])
        .spawn();
}

#[cfg(not(target_os = "windows"))]
fn copy_to_clipboard(text: &str) {
    let _ = Command::new("xclip")
        .arg("-selection")
        .arg("clipboard")
        .arg("-i")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.as_mut().map(|stdin| {
                let _ = stdin.write_all(text.as_bytes());
            });
            Ok(child)
        });
}

#[cfg(target_os = "windows")]
fn execute_command(cmd: &str) {
    let _ = Command::new("cmd")
        .args(&["/C", "start", "cmd", "/C", cmd])
        .spawn();
}

#[cfg(not(target_os = "windows"))]
fn execute_command(cmd: &str) {
    let _ = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .spawn();
}

#[cfg(target_os = "windows")]
fn open_web_search(query: &str) {
    let url = format!("https://duckduckgo.com/?q={}", urlencoding::encode(query));
    open_url(&url);
}

#[cfg(not(target_os = "windows"))]
fn open_web_search(query: &str) {
    let url = format!("https://duckduckgo.com/?q={}", urlencoding::encode(query));
    let _ = Command::new("xdg-open")
        .arg(&url)
        .spawn();
}

#[cfg(target_os = "windows")]
fn open_url(url: &str) {
    let _ = Command::new("cmd")
        .args(&["/C", "start", "", url])
        .spawn();
}

#[cfg(not(target_os = "windows"))]
fn open_url(url: &str) {
    let _ = Command::new("xdg-open")
        .arg(url)
        .spawn();
}

#[cfg(target_os = "windows")]
fn open_file(path: &PathBuf) {
    let _ = Command::new("cmd")
        .args(&["/C", "start", "", &path.to_string_lossy()])
        .spawn();
}

#[cfg(not(target_os = "windows"))]
fn open_file(path: &PathBuf) {
    let _ = Command::new("xdg-open")
        .arg(path)
        .spawn();
}

fn search_files(query: &str) -> Vec<PathBuf> {
    let mut results = Vec::new();
    let query_lower = query.to_lowercase();
    
    let search_dirs = [
        dirs::download_dir(),
        dirs::document_dir(),
        dirs::desktop_dir(),
        dirs::picture_dir(),
        dirs::audio_dir(),
        dirs::video_dir(),
    ];
    
    for dir_option in &search_dirs {
        if let Some(dir) = dir_option {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                        if file_name.to_lowercase().contains(&query_lower) {
                            results.push(path);
                            if results.len() >= 5 {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    
    results.sort_by(|a, b| {
        let a_name = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let b_name = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
        a_name.len().cmp(&b_name.len())
    });
    
    results.into_iter().take(8).collect()
}

fn search_emojis(query: &str) -> Vec<(String, String)> {
    let query_lower = query.to_lowercase();
    
    let common_aliases: Vec<(&str, &str)> = vec![
        ("smile", "😊"), ("happy", "😊"), ("laugh", "😂"), ("heart", "❤️"), ("love", "❤️"),
        ("kiss", "😘"), ("cool", "😎"), ("thinking", "🤔"), ("thumbsup", "👍"), ("like", "👍"),
        ("ok", "👌"), ("clap", "👏"), ("pray", "🙏"), ("wave", "👋"), ("muscle", "💪"),
        ("eyes", "👀"), ("cat", "🐱"), ("dog", "🐶"), ("car", "🚗"), ("plane", "✈️"),
        ("rocket", "🚀"), ("computer", "💻"), ("phone", "📱"), ("camera", "📷"), ("music", "🎵"),
        ("game", "🎮"), ("food", "🍕"), ("coffee", "☕"), ("beer", "🍺"), ("fire", "🔥"),
        ("star", "⭐"), ("money", "💰"), ("clock", "⏰"), ("email", "📧"), ("book", "📖"),
    ];
    
    let alias_results: Vec<(String, String)> = common_aliases
        .iter()
        .filter(|(alias, _)| alias.contains(&query_lower))
        .map(|(alias, emoji)| (alias.to_string(), emoji.to_string()))
        .take(3)
        .collect();
    
    let crate_results: Vec<(String, String)> = emojis::iter()
        .filter_map(|emoji| {
            if emoji.name().to_lowercase().contains(&query_lower) {
                Some((emoji.name().to_string(), emoji.as_str().to_string()))
            } else {
                None
            }
        })
        .take(2)
        .collect();
    
    let mut combined = alias_results;
    for result in crate_results {
        if !combined.iter().any(|(_, emoji)| emoji == &result.1) {
            combined.push(result);
        }
    }
    
    combined.truncate(5);
    combined
}

fn looks_like_url(text: &str) -> bool {
    let text = text.trim();
    
    if text.contains("://") {
        return text.starts_with("http://") || text.starts_with("https://");
    }
    
    if text.contains('.') && !text.contains(' ') {
        let domain_part = if text.contains('/') {
            text.split('/').next().unwrap_or("")
        } else {
            text
        };
        
        let parts: Vec<&str> = domain_part.split('.').collect();
        if parts.len() >= 2 {
            let last_part = parts.last().unwrap();
            
            let common_tlds = [
                "com", "org", "net", "io", "co", "me", "dev", "app", "tech", "xyz",
                "us", "uk", "ca", "au", "de", "fr", "jp", "in", "br", "ru",
            ];
            
            return common_tlds.iter().any(|&tld| *last_part == tld) || 
                   last_part.len() == 2;
        }
    }
    
    false
}

fn acquire_lock() -> Result<File, String> {
    let lock_path = get_lock_path();
    
    if lock_path.exists() {
        return Err("Flint is already running!".to_string());
    }
    
    let mut lock_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
        .map_err(|e| format!("Failed to create lock file: {}", e))?;
    
    let pid = std::process::id();
    lock_file.write_all(pid.to_string().as_bytes())
        .map_err(|e| format!("Failed to write PID: {}", e))?;
    
    Ok(lock_file)
}

fn get_lock_path() -> PathBuf {
    std::env::temp_dir().join("flint.lock")
}

#[cfg(target_os = "windows")]
fn launch_app(exec_command: &str) {
    let _ = Command::new("cmd")
        .args(&["/C", "start", "", exec_command])
        .spawn();
}

#[cfg(not(target_os = "windows"))]
fn launch_app(exec_command: &str) {
    let _ = Command::new("sh")
        .arg("-c")
        .arg(exec_command)
        .spawn();
}

#[cfg(target_os = "windows")]
fn scan_apps() -> Vec<AppEntry> {
    scan_windows_apps()
}

#[cfg(not(target_os = "windows"))]
fn scan_apps() -> Vec<AppEntry> {
    scan_linux_apps()
}

fn scan_windows_apps() -> Vec<AppEntry> {
    let mut apps = Vec::new();

    let common_apps = [
        ("Notepad", "notepad.exe"),
        ("Calculator", "calc.exe"), 
        ("Paint", "mspaint.exe"),
        ("Command Prompt", "cmd.exe"),
        ("PowerShell", "powershell.exe"),
        ("File Explorer", "explorer.exe"),
        ("Task Manager", "taskmgr.exe"),
        ("Control Panel", "control.exe"),
        ("System Configuration", "msconfig.exe"),
        ("Registry Editor", "regedit.exe"),
        ("Windows Media Player", "wmplayer.exe"),
        ("WordPad", "write.exe"),
        ("Snipping Tool", "snippingtool.exe"),
        ("Sticky Notes", "stikynot.exe"),
    ];

    for (name, exec) in common_apps {
        apps.push(AppEntry {
            name: name.to_string(),
            desktop_id: name.to_string(),
            exec_command: exec.to_string(),
            match_indices: Vec::new(),
        });
    }

    let program_dirs = [
        std::env::var("PROGRAMFILES").unwrap_or_else(|_| "C:\\Program Files".to_string()),
        std::env::var("PROGRAMFILES(X86)").unwrap_or_else(|_| "C:\\Program Files (x86)".to_string()),
        std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string()),
    ];

    for program_dir in &program_dirs {
        let program_path = PathBuf::from(program_dir);
        if program_path.exists() {
            if let Ok(entries) = fs::read_dir(&program_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Some(folder_name) = path.file_name().and_then(|s| s.to_str()) {
                            if let Ok(sub_entries) = fs::read_dir(&path) {
                                for sub_entry in sub_entries.flatten() {
                                    let sub_path = sub_entry.path();
                                    if sub_path.extension().and_then(|e| e.to_str()) == Some("exe") {
                                        if let Some(exe_name) = sub_path.file_stem().and_then(|s| s.to_str()) {
                                            apps.push(AppEntry {
                                                name: format!("{} - {}", folder_name, exe_name),
                                                desktop_id: folder_name.to_string(),
                                                exec_command: sub_path.to_string_lossy().to_string(),
                                                match_indices: Vec::new(),
                                            });
                                        }
                                    }
                                }
                            }
                            
                            apps.push(AppEntry {
                                name: folder_name.to_string(),
                                desktop_id: folder_name.to_string(),
                                exec_command: format!("explorer \"{}\"", path.display()),
                                match_indices: Vec::new(),
                            });
                        }
                    }
                }
            }
        }
    }

    apps.sort_by(|a, b| a.name.cmp(&b.name));
    apps.dedup_by(|a, b| a.name == b.name);
    apps
}

fn scan_linux_apps() -> Vec<AppEntry> {
    let mut apps = Vec::new();
    
    let common_apps = [
        ("Firefox", "firefox"),
        ("Terminal", "gnome-terminal"),
        ("Files", "nautilus"),
        ("Text Editor", "gedit"),
        ("Calculator", "gnome-calculator"),
        ("Settings", "gnome-control-center"),
    ];
    
    for (name, exec) in common_apps {
        apps.push(AppEntry {
            name: name.to_string(),
            desktop_id: name.to_string(),
            exec_command: exec.to_string(),
            match_indices: Vec::new(),
        });
    }
    
    let desktop_dirs = [
        dirs::data_dir().map(|p| p.join("applications")),
        Some(PathBuf::from("/usr/share/applications")),
        Some(PathBuf::from("/usr/local/share/applications")),
    ];
    
    for dir_option in &desktop_dirs {
        if let Some(dir) = dir_option {
            if dir.exists() {
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
                            if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                                apps.push(AppEntry {
                                    name: name.to_string(),
                                    desktop_id: name.to_string(),
                                    exec_command: path.to_string_lossy().to_string(),
                                    match_indices: Vec::new(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    
    apps.sort_by(|a, b| a.name.cmp(&b.name));
    apps.dedup_by(|a, b| a.name == b.name);
    apps
}

#[cfg(target_os = "windows")]
fn get_config_dir() -> PathBuf {
    dirs::config_dir()
        .map(|p| p.join("Flint"))
        .unwrap_or_else(|| PathBuf::from("C:\\ProgramData\\Flint"))
}

#[cfg(not(target_os = "windows"))]
fn get_config_dir() -> PathBuf {
    dirs::config_dir()
        .map(|p| p.join("flint"))
        .unwrap_or_else(|| PathBuf::from("~/.config/flint"))
}

fn create_default_theme(theme_path: &PathBuf) {
    let default_theme = r#"# Flint Theme Configuration
# Dark Theme

# Main window colors
background=#2d2d30
text_color=#ffffff
selection_bg=#0078d4
selection_text=#ffffff
border_color=#3e3e42
highlight_color=#0078d4

# Font settings
font_size=16
font_family=Segoe UI

# Border radius
border_radius=2
"#;
    
    if let Some(parent) = theme_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(theme_path, default_theme);
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let run_in_tray = args.len() > 1 && args[1] == "--tray";
    
    if run_in_tray {
        println!("Flint Launcher running in system tray...");
        println!("Config location: {}", get_config_dir().display());
        println!("Right-click the tray icon to access options.");
        
        loop {
            thread::sleep(Duration::from_secs(10));
        }
    }
    
    let mode = if args.len() > 1 && args[1] == "settings" {
        AppMode::Settings
    } else {
        AppMode::Launcher
    };

    let app = match FlintApp::new() {
        Ok(mut app) => {
            app.app_mode = mode;
            app
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };
    
    let (title, width, height) = if mode == AppMode::Settings {
        ("Flint Settings", 550.0, 600.0)
    } else {
        ("Flint", 600.0, 50.0)
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([width, height])
            .with_decorations(mode == AppMode::Settings)
            .with_always_on_top()
            .with_resizable(false)
            .with_window_level(egui::WindowLevel::AlwaysOnTop)
            .with_position(egui::pos2(
                (1920.0 - width) / 2.0,
                200.0,
            )),
        centered: false,
        ..Default::default()
    };

    eframe::run_native(
        title,
        options,
        Box::new(move |cc| {
            cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            Box::new(app)
        }),
    )
}