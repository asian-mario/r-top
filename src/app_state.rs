use std::time::{Duration, Instant};
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use ratatui::layout::Rect;
use ratatui::widgets::Row;
use tachyonfx::EffectManager;

use crate::theme::{Theme, ThemeManager};
use crate::types::SortCategory;
use crate::constants::SWEEP_DURATION_MS;
use crate::system_info::ProcessCache;
use crate::event::{KeyEvent, KeyCode};

use std::collections::{HashMap, HashSet};
use sysinfo::Pid; 

//again, should this be in its own file? maybe
#[derive(Clone, Debug)]
pub struct TreeItem {
    pub pid: Pid,
    pub name: String,
    pub cpu_usage: f32,
    pub memory: u64,
    pub level: usize, //child proc. etc.
    pub is_expanded: bool,   
    pub has_children: bool, 
    pub parent_pid: Option<Pid>, 
    /*
        i think theres an issue with usage tracking here, parent_pid is clearly used when the tree is being built so?
     */
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchType {
    Name,
    Pid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserSettings {
    pub refresh_interval_ms: u64,
    pub default_interface: String,
    pub default_usage_view: String, // "cpu" or "gpu"
    pub default_theme: String,
    pub show_info_on_start: bool,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            refresh_interval_ms: 2000,
            default_interface: "eth0".to_string(),
            default_usage_view: "cpu".to_string(),
            default_theme: "DarkPurple".to_string(),
            show_info_on_start: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DaemonSettings {
    pub enabled_service: String, // Name of service to auto-run via daemon, empty string means none
}

impl Default for DaemonSettings {
    fn default() -> Self {
        Self {
            enabled_service: String::new(),
        }
    }
}

pub struct AppState {
    pub effects: EffectManager<()>,
    pub pause_overlay: bool,
    pub refresh_interval: Duration,
    pub selected_process: usize,
    pub sort_category: SortCategory,
    pub current_interface: &'static str,
    pub show_info: bool,
    pub current_disk_index: usize,
    pub current_gpu_index: usize,
    pub scroll_offset: usize,
    pub visible_rows: usize,
    pub switch_interface_at: Instant,

    // Cache
    pub process_cache: ProcessCache,

    /*
        ONLY used if the optimized version of the function is used
     */
    //Process table rendering cache
    pub cached_rows: Vec<Row<'static>>,
    pub last_process_count: usize,
    pub last_scroll_offset: usize,
    pub last_selected_process: usize,
    pub rows_cache_valid: bool,

    //Process tree (only when show_info is true)
    pub tree_expanded_nodes: HashSet<Pid>,
    pub tree_selected_index: usize,
    pub tree_items: Vec<TreeItem>,
    pub tree_cache_valid: bool,
    pub show_tree_view: bool,
    
    // Animated areas
    pub info_area: Rect,
    pub net_area: Rect,
    pub disk_area: Rect,
    pub terminal_area: Rect,
    pub cpu_usage_area: Rect,

    // Theme management
    pub theme_manager: ThemeManager,

    // Search
    pub search_active: bool,
    pub search_query: String,
    pub filtered_processes: Vec<usize>,
    pub search_cache_valid: bool,

    // Popup for errors/warnings
    pub popup_visible: bool,
    pub popup_message: String,

    // Pause menu
    pub pause_menu_selected: usize,

    // Theme panel
    pub theme_panel_visible: bool,
    pub theme_selected_index: usize,

    // Settings panel
    pub settings_panel_visible: bool,
    pub settings_selected_index: usize,
    pub user_settings: UserSettings,

    // Daemon settings panel
    pub daemon_panel_visible: bool,
    pub daemon_selected_index: usize,
    pub daemon_settings: DaemonSettings,
    pub available_services: Vec<String>,

    // GPU cache
    pub gpu_info_cache: Vec<crate::system_info::GpuInfo>,
    pub gpu_cache_last_update: Instant,

    // GPU view toggle
    pub gpu_usage_view: bool,
}


impl AppState {
    pub fn new() -> Self {
        let mut effects: EffectManager<()> = EffectManager::default();
        effects.add_effect(tachyonfx::fx::coalesce((400, tachyonfx::Interpolation::QuintInOut)));
        
        Self {
            effects,
            pause_overlay: false,
            refresh_interval: Duration::from_millis(2000),
            selected_process: 0,
            sort_category: SortCategory::CpuPerCore,
            current_interface: "eth0",
            show_info: false,
            current_disk_index: 0,
            current_gpu_index: 0,
            scroll_offset: 0,
            visible_rows: 0,
            switch_interface_at: Instant::now() + Duration::from_millis(SWEEP_DURATION_MS),
            process_cache: ProcessCache::new(),
            
            // Row Cache
            cached_rows: Vec::with_capacity(50), // Pre-allocate for typical screen size -> May not be the best method
            last_process_count: 0,
            last_scroll_offset: usize::MAX, // Force initial cache miss
            last_selected_process: usize::MAX,
            rows_cache_valid: false,

            // Tree & Tree Cache
            tree_expanded_nodes: HashSet::new(),
            tree_selected_index: 0,
            tree_items: Vec::new(),
            tree_cache_valid: false,
            show_tree_view: false, // Base flat list

            // Areas for Rendering
            info_area: Rect::default(),
            net_area: Rect::default(),
            disk_area: Rect::default(),
            terminal_area: Rect::default(),
            cpu_usage_area: Rect::default(),

            // Themes
            theme_manager: ThemeManager::new(),

            // Search
            search_active: false,
            search_query: String::new(),
            filtered_processes: Vec::new(),
            search_cache_valid: false,

            // Popup
            popup_visible: false,
            popup_message: String::new(),

            // Pause menu
            pause_menu_selected: 0,

            // Theme panel
            theme_panel_visible: false,
            theme_selected_index: 0,

            // Settings panel
            settings_panel_visible: false,
            settings_selected_index: 0,
            user_settings: UserSettings::default(),

            // Daemon settings panel
            daemon_panel_visible: false,
            daemon_selected_index: 0,
            daemon_settings: DaemonSettings::default(),
            available_services: Vec::new(),

            // GPU cache (will be populated on first render)
            gpu_info_cache: Vec::new(),
            gpu_cache_last_update: Instant::now(),

            // GPU view toggle
            gpu_usage_view: false,
        }
    }
    // methods to for tree shit
    pub fn invalidate_tree_cache(&mut self) {
        self.tree_cache_valid = false;
    }

    pub fn toggle_tree_view(&mut self) {
        if self.show_info {
            self.show_tree_view = !self.show_tree_view;
            self.invalidate_tree_cache();
        }
    }

    pub fn toggle_tree_node(&mut self, pid: Pid) {
        if self.tree_expanded_nodes.contains(&pid) {
            self.tree_expanded_nodes.remove(&pid);
        } else {
            self.tree_expanded_nodes.insert(pid);
        }
        self.invalidate_tree_cache();
    }

    pub fn tree_navigate_up(&mut self) {
        if self.tree_selected_index > 0 {
            self.tree_selected_index -= 1;
        }
    }

    pub fn tree_navigate_down(&mut self) {
        if self.tree_selected_index + 1 < self.tree_items.len() {
            self.tree_selected_index += 1;
        }
    }

    pub fn get_selected_tree_item(&self) -> Option<&TreeItem> {
        self.tree_items.get(self.tree_selected_index)
    }

    pub fn expand_current_node(&mut self) {
        if let Some(item) = self.tree_items.get(self.tree_selected_index) {
            if item.has_children {
                self.tree_expanded_nodes.insert(item.pid);
                self.invalidate_tree_cache();
            }
        }
    }

    pub fn collapse_current_node(&mut self) {
        if let Some(item) = self.tree_items.get(self.tree_selected_index) {
            self.tree_expanded_nodes.remove(&item.pid);
            self.invalidate_tree_cache();
        }
    }

    pub fn update_terminal_area(&mut self, area: Rect) {
        self.terminal_area = area;
    }

    //invalidate row cache when needed -> PLEASE call this anytime you do something with processes, or else it has to wait for an action to invaludate rows cache and makes it jank
    pub fn invalidate_rows_cache(&mut self) {
        self.rows_cache_valid = false;
    }
    pub fn cycle_sort_left(&mut self) {
        self.sort_category = self.sort_category.previous();
        self.invalidate_rows_cache();
    }

    pub fn cycle_sort_right(&mut self) {
        self.sort_category = self.sort_category.next();
        self.invalidate_rows_cache()
    }

    pub fn switch_to_loopback(&mut self) {
        if Instant::now() >= self.switch_interface_at && self.current_interface != "lo" {
            self.current_interface = "lo";
        }
    }

    pub fn switch_to_ethernet(&mut self) {
        if Instant::now() >= self.switch_interface_at && self.current_interface != "eth0" {
            self.current_interface = "eth0";
        }
    }

    //change this up to work with the new tree diagram (please work)
    pub fn toggle_info(&mut self) {
        self.show_info = !self.show_info;
        if !self.show_info {
            // reset tree when show_info is collapsed
            self.show_tree_view = false;
            self.tree_selected_index = 0;
            self.invalidate_tree_cache();
        }
        self.invalidate_rows_cache();
    }

    pub fn previous_disk(&mut self) {
        if self.current_disk_index > 0 {
            self.current_disk_index -= 1;
        }
    }
    pub fn next_disk(&mut self, max_disks: usize) {
        if self.current_disk_index + 1 < max_disks {
            self.current_disk_index += 1;
        }
    }

    pub fn previous_gpu(&mut self) {
        if self.current_gpu_index > 0 {
            self.current_gpu_index -= 1;
        }
    }

    pub fn next_gpu(&mut self, max_gpus: usize) {
        if self.current_gpu_index + 1 < max_gpus {
            self.current_gpu_index += 1;
        }
    }

    pub fn toggle_gpu_usage_view(&mut self) {
        self.gpu_usage_view = !self.gpu_usage_view;
    }

    pub fn update_gpu_cache_if_needed(&mut self) {
        // Only update GPU info every 5 seconds to avoid performance hit
        if self.gpu_info_cache.is_empty() || self.gpu_cache_last_update.elapsed() > Duration::from_secs(5) {
            self.gpu_info_cache = crate::system_info::get_gpu_info();
            self.gpu_cache_last_update = Instant::now();
        }
    }

    // Settings panel control
    pub fn open_settings_panel(&mut self) {
        self.settings_panel_visible = true;
        self.settings_selected_index = 0;
    }

    pub fn close_settings_panel(&mut self) {
        self.settings_panel_visible = false;
    }

    pub fn settings_up(&mut self) {
        if self.settings_selected_index > 0 {
            self.settings_selected_index -= 1;
        }
    }

    pub fn settings_down(&mut self, max: usize) {
        if self.settings_selected_index + 1 < max {
            self.settings_selected_index += 1;
        }
    }

    pub fn apply_user_settings(&mut self) {
        // Refresh interval
        let ms = self.user_settings.refresh_interval_ms.clamp(100, 10_000);
        self.refresh_interval = Duration::from_millis(ms);

        // Interface (only eth0/lo recognized for now)
        self.current_interface = if self.user_settings.default_interface.to_lowercase() == "lo" {
            "lo"
        } else {
            "eth0"
        };

        // Usage view
        self.gpu_usage_view = self.user_settings.default_usage_view.to_lowercase() == "gpu";

        // Theme
        if let Some(t) = self.theme_manager.list_theme_types().iter().find(|t| t.as_str() == self.user_settings.default_theme) {
            self.theme_manager.set_theme(*t);
        }

        // Show info toggle
        self.show_info = self.user_settings.show_info_on_start;
    }

    pub fn settings_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("r-top").join("config.toml"))
    }

    pub fn save_user_settings(&self) -> Result<(), String> {
        let path = Self::settings_path().ok_or("Unable to resolve config directory")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {}", e))?;
        }
        let toml = toml::to_string_pretty(&self.user_settings).map_err(|e| format!("Serialize settings failed: {}", e))?;
        std::fs::write(&path, toml).map_err(|e| format!("Write settings failed: {}", e))?;
        Ok(())
    }

    pub fn load_user_settings(&mut self) -> Result<(), String> {
        let path = Self::settings_path().ok_or("Unable to resolve config directory")?;
        let settings = if path.exists() {
            let content = std::fs::read_to_string(&path).map_err(|e| format!("Read settings failed: {}", e))?;
            toml::from_str::<UserSettings>(&content).unwrap_or_default()
        } else {
            let defaults = UserSettings::default();
            let toml = toml::to_string_pretty(&defaults).map_err(|e| format!("Serialize defaults failed: {}", e))?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {}", e))?;
            }
            std::fs::write(&path, toml).map_err(|e| format!("Write default settings failed: {}", e))?;
            defaults
        };
        self.user_settings = settings;
        self.apply_user_settings();
        Ok(())
    }

    pub fn increase_refresh_interval(&mut self) {
        let new_ms = (self.refresh_interval.as_millis() + 100).min(10000);
        self.refresh_interval = Duration::from_millis(new_ms as u64);
    }

    pub fn decrease_refresh_interval(&mut self) {
        let new_ms = self.refresh_interval.as_millis().saturating_sub(100).max(100);
        self.refresh_interval = Duration::from_millis(new_ms as u64);
    }
    
    pub fn switch_theme(&mut self) {
        self.theme_manager.switch_theme();
        self.invalidate_rows_cache();
    }

    pub fn toggle_search(&mut self){
        self.search_active = !self.search_active;
        if !self.search_active {
            //reset all vars
            self.search_query.clear();
            self.filtered_processes.clear();
            self.search_cache_valid = false;
            self.selected_process = 0;
            self.scroll_offset = 0;
        }

        self.invalidate_rows_cache();
    }

    pub fn toggle_pause_overlay(&mut self) {
        self.pause_overlay = !self.pause_overlay;
    }

    pub fn add_search_char(&mut self, c: char){
        if self.search_active {
            self.search_query.push(c);
            self.search_cache_valid = false;
            self.selected_process = 0;
            self.scroll_offset = 0;
            self.invalidate_rows_cache();
        }
    }

    pub fn remove_search_char(&mut self) {
        if self.search_active && !self.search_query.is_empty() {
            self.search_query.pop();
            self.search_cache_valid = false;
            self.selected_process = 0;
            self.scroll_offset = 0;
            self.invalidate_rows_cache();
        }
    }

    pub fn invalidate_search_cache(&mut self) {
        self.search_cache_valid = false;
    }

    pub fn is_search_empty(&self) -> bool {
        self.search_query.trim().is_empty()
    }

    pub fn handle_search_input(&mut self, key: KeyEvent) -> bool {
        if !self.search_active{
            return false;
        }

        match key.code {
            KeyCode::Char(c) => {
                if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ' | ':') {
                    self.add_search_char(c);
                    return true;
                } else{
                    return false;
                }
            }
            KeyCode::Backspace => {
                if !self.search_query.is_empty() {
                    self.remove_search_char();
                    return true;
                } else {
                    return false;
                }
            }
            KeyCode::Delete => {
                if !self.search_query.is_empty(){
                    self.search_query.clear();
                    self.search_cache_valid = false;
                    self.selected_process = 0;
                    self.scroll_offset = 0;
                    self.invalidate_rows_cache();
                } else {
                    return false;
                }
            }
            _ => {}
        }

        false

    }

    pub fn get_search_type(&self) -> SearchType {
        if self.search_query.starts_with("pid:") {
            SearchType::Pid
        } else {
            SearchType::Name
        }
    }

    pub fn get_search_value(&self) -> &str {
        if self.search_query.starts_with("pid:") {
            &self.search_query[4..]
        } else {
            &self.search_query
        }
    }

    pub fn show_popup(&mut self, message: String) {
        self.popup_visible = true;
        self.popup_message = message;
    }

    pub fn dismiss_popup(&mut self) {
        self.popup_visible = false;
        self.popup_message.clear();
    }

    // Theme panel controls
    pub fn open_theme_panel(&mut self) {
        self.theme_panel_visible = true;
        self.theme_selected_index = 0;
    }

    pub fn close_theme_panel(&mut self) {
        self.theme_panel_visible = false;
    }

    pub fn theme_panel_up(&mut self) {
        if self.theme_selected_index > 0 {
            self.theme_selected_index -= 1;
        }
    }

    pub fn theme_panel_down(&mut self) {
        if self.theme_selected_index < 2 {
            self.theme_selected_index += 1;
        }
    }

    pub fn pause_menu_up(&mut self) {
        if self.pause_menu_selected > 0 {
            self.pause_menu_selected -= 1;
        }
    }

    pub fn pause_menu_down(&mut self) {
        if self.pause_menu_selected < 3 {
            self.pause_menu_selected += 1;
        }
    }

    pub fn reset_pause_menu(&mut self) {
        self.pause_menu_selected = 0;
    }

    // Daemon settings panel controls
    pub fn open_daemon_panel(&mut self) {
        self.daemon_panel_visible = true;
        self.daemon_selected_index = 0;
        // Load available services from config
        let _ = self.load_available_services();
    }

    pub fn close_daemon_panel(&mut self) {
        self.daemon_panel_visible = false;
    }

    pub fn daemon_panel_up(&mut self) {
        if self.daemon_selected_index > 0 {
            self.daemon_selected_index -= 1;
        }
    }

    pub fn daemon_panel_down(&mut self, max: usize) {
        if self.daemon_selected_index + 1 < max {
            self.daemon_selected_index += 1;
        }
    }

    pub fn load_available_services(&mut self) -> Result<(), String> {
        // Load services from daemon config file
        let daemon_config_path = dirs::config_dir()
            .ok_or("Unable to resolve config directory")?
            .join("r-top")
            .join("services.toml");

        self.available_services.clear();

        if daemon_config_path.exists() {
            let content = std::fs::read_to_string(&daemon_config_path)
                .map_err(|e| format!("Read daemon config failed: {}", e))?;
            
            // Parse TOML and extract service names
            if let Ok(toml_val) = content.parse::<toml::Value>() {
                if let Some(services) = toml_val.get("services").and_then(|v| v.as_array()) {
                    for service in services {
                        if let Some(name) = service.get("name").and_then(|n| n.as_str()) {
                            self.available_services.push(name.to_string());
                        }
                    }
                }
            }
        }

        // Add "None" as first option
        let mut services = vec!["None".to_string()];
        services.extend(self.available_services.clone());
        self.available_services = services;

        Ok(())
    }

    pub fn daemon_settings_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("r-top").join("daemon_settings.toml"))
    }

    pub fn save_daemon_settings(&self) -> Result<(), String> {
        let path = Self::daemon_settings_path().ok_or("Unable to resolve config directory")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {}", e))?;
        }
        let toml = toml::to_string_pretty(&self.daemon_settings).map_err(|e| format!("Serialize daemon settings failed: {}", e))?;
        std::fs::write(&path, toml).map_err(|e| format!("Write daemon settings failed: {}", e))?;
        Ok(())
    }

    pub fn load_daemon_settings(&mut self) -> Result<(), String> {
        let path = Self::daemon_settings_path().ok_or("Unable to resolve config directory")?;
        let settings = if path.exists() {
            let content = std::fs::read_to_string(&path).map_err(|e| format!("Read daemon settings failed: {}", e))?;
            toml::from_str::<DaemonSettings>(&content).unwrap_or_default()
        } else {
            let defaults = DaemonSettings::default();
            let toml = toml::to_string_pretty(&defaults).map_err(|e| format!("Serialize defaults failed: {}", e))?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {}", e))?;
            }
            std::fs::write(&path, toml).map_err(|e| format!("Write default settings failed: {}", e))?;
            defaults
        };
        self.daemon_settings = settings;
        Ok(())
    }

    pub fn reset_daemon_settings(&mut self) {
        self.daemon_settings = DaemonSettings::default();
    }
}