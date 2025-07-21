use std::time::{Duration, Instant};
use ratatui::layout::Rect;
use ratatui::widgets::Row;
use tachyonfx::EffectManager;
use crate::types::SortCategory;
use crate::constants::SWEEP_DURATION_MS;
use crate::system_info::ProcessCache;

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
    
    // Animated areas
    pub info_area: Rect,
    pub net_area: Rect,
    pub disk_area: Rect,
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

            // Areas for Rendering
            info_area: Rect::default(),
            net_area: Rect::default(),
            disk_area: Rect::default(),
        }
    }

    //invalidate row cache when needed
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

    pub fn toggle_info(&mut self) {
        self.show_info = !self.show_info;
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
    /*
        all fine?
     */
}