use std::time::{Duration, Instant};
use ratatui::layout::Rect;
use ratatui::widgets::Row;
use tachyonfx::EffectManager;
use crate::theme::{Theme, ThemeManager};
use crate::types::SortCategory;
use crate::constants::SWEEP_DURATION_MS;
use crate::system_info::ProcessCache;

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

pub struct AppState {
    pub effects: EffectManager<()>,
    pub refresh_interval: Duration,
    pub selected_process: usize,
    pub sort_category: SortCategory,
    pub current_interface: &'static str,
    pub show_info: bool,
    pub current_disk_index: usize,
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

    // Theme management
    pub theme_manager: ThemeManager,
}

impl AppState {
    pub fn new() -> Self {
        let mut effects: EffectManager<()> = EffectManager::default();
        effects.add_effect(tachyonfx::fx::coalesce((400, tachyonfx::Interpolation::QuintInOut)));
        
        Self {
            effects,
            refresh_interval: Duration::from_millis(2000),
            selected_process: 0,
            sort_category: SortCategory::CpuPerCore,
            current_interface: "eth0",
            show_info: false,
            current_disk_index: 0,
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

            // Themes
            theme_manager: ThemeManager::new(),
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
}