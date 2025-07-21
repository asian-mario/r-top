use ratatui::prelude::*;

pub const HISTORY_LEN: usize = 64;

// Global static variables for session tracking
pub static mut SESSION_TOTAL_BYTES: u64 = 0;
pub static mut PREV_RX: u64 = 0;
pub static mut PREV_TX: u64 = 0;

// Custom colors
pub const CUSTOM_PURPLE: Color = Color::Rgb(126, 48, 219);
pub const CUSTOM_LIGHT_PURPLE: Color = Color::Rgb(137, 125, 219);
pub const CUSTOM_G_PURPLE: Color = Color::Rgb(126, 48, 219);
pub const CUSTOM_BG_PURPLE: Color = Color::Rgb(31, 3, 64);
/*
    PLAN!
    Theme support, somehow. Get rid of all this hardcoded nonsense
*/

// Animation constants
pub const SWEEP_DURATION_MS: u64 = 300;
pub const ANIMATION_COLOR: u32 = 0x1E1E1E;
pub const ANIMATION_TIMER_MS: u32 = 200;