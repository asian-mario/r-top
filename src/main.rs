use std::{collections::VecDeque, io, time::{Duration, Instant}};
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

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app_state = AppState::new();
    
    let refresh = RefreshKind::everything();
    let mut system = System::new_with_specifics(refresh);
    let mut networks = Networks::new_with_refreshed_list();
    let mut disks = Disks::new_with_refreshed_list();

    let mut cpu_history: Vec<VecDeque<f32>> = vec![];
    /*
        terrible for memory, TODO: will need to make this more memory efficient ->
        either limit history per core OR circular buffer
     */
    let mut last_refresh = Instant::now();

    loop {
        let now = Instant::now();
        if now.duration_since(last_refresh) >= app_state.refresh_interval {
            /*
                a bit shit to refresh_all

                nevermind:
                forgot it only does cpu, proc, mem -> completely o.k
             */
            system.refresh_all();
            networks.refresh(false);
            disks.refresh(false);
            last_refresh = now;
        }

        if cpu_history.is_empty() {
            cpu_history = vec![VecDeque::from(vec![0.0; HISTORY_LEN]); system.cpus().len()];
        }

        update_cpu_history(&mut cpu_history, &system);
        let processes = sort_processes(&system, &app_state.sort_category);

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