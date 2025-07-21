use std::{io, time::{Duration, Instant}};
use crossterm::event::{self, Event};
use sysinfo::{System, RefreshKind, Networks, Disks};

mod constants;
mod types;
mod utils;
mod ui;
mod event_handler;
mod system_info;
mod app_state;

use constants::*;
use ui::*;
use event_handler::*;
use system_info::*;
use app_state::*;
use utils::CircularBuffer;

/*
    please refrain from taking any comments that dont have proper punctuation as serious
    i shitpost a lot because its lonely
*/

/*
    MEMORY OPT. LIST:
    - CPU PROC CACHE
    - CPU HISTORY CIRC BUFFER
*/
fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app_state = AppState::new();
    
    let refresh = RefreshKind::everything();
    let mut system = System::new_with_specifics(refresh);
    let mut networks = Networks::new_with_refreshed_list();
    let mut disks = Disks::new_with_refreshed_list();

    // Replace VecDeque with CircularBuffer - much more memory efficient!
    let mut cpu_history: Vec<CircularBuffer<f32>> = vec![];
    let mut last_refresh = Instant::now();

    loop {
        let now = Instant::now();
        if now.duration_since(last_refresh) >= app_state.refresh_interval {
            /*
            UPDATE: still wondering if I should fix the refresh_all() method
             */
            system.refresh_all();
            networks.refresh(false);
            disks.refresh(false);
            app_state.invalidate_rows_cache();
            last_refresh = now;
        }

        // Initialize circular buffers once we know the CPU count
        if cpu_history.is_empty() {
            cpu_history = (0..system.cpus().len())
                .map(|_| CircularBuffer::new(HISTORY_LEN))
                .collect();
        }

        update_cpu_history(&mut cpu_history, &system);
        let processes = sort_processes_cached(&system, &app_state.sort_category, &mut app_state.process_cache);

        terminal.draw(|frame| {
            render_ui(frame, &system, &networks, &disks, &processes, &cpu_history, &mut app_state);
        })?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if handle_key_event(key, &mut app_state, &system, &processes)? {
                    break;
                }
            }
        }

        std::thread::sleep(Duration::from_millis(16));
    }

    ratatui::restore();
    Ok(())
}