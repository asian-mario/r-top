use std::io;
use crossterm::event::{KeyEvent, KeyCode};
use sysinfo::{System, Process};
use tachyonfx::{fx, Motion, Interpolation};
use ratatui::prelude::Color;
use libc::{kill, SIGKILL};
use crate::app_state::AppState;
use crate::constants::{ANIMATION_COLOR, ANIMATION_TIMER_MS};

pub fn handle_key_event(
    key: KeyEvent,
    app_state: &mut AppState,
    system: &System, //-> yes i know its not used since we already have appstate, but i'm using this as a fallback incase I need to make some quick tests
    processes: &Vec<&Process>
) -> io::Result<bool> {
    match key.code {
        KeyCode::Char('q') => return Ok(true),

        KeyCode::Down => {
            if app_state.selected_process + 1 < processes.len() {
                app_state.selected_process += 1;
                if app_state.selected_process >= app_state.scroll_offset + app_state.visible_rows {
                    app_state.scroll_offset += 1;
                }
            }
        }
        KeyCode::Up => {
            if app_state.selected_process > 0 {
                app_state.selected_process -= 1;
                if app_state.selected_process < app_state.scroll_offset {
                    app_state.scroll_offset = app_state.scroll_offset.saturating_sub(1);
                }
            }
        }
        KeyCode::PageDown => {
            app_state.selected_process = (app_state.selected_process + app_state.visible_rows)
                .min(processes.len().saturating_sub(1));
            app_state.scroll_offset = (app_state.scroll_offset + app_state.visible_rows)
                .min(processes.len().saturating_sub(app_state.visible_rows));
        }
        KeyCode::PageUp => {
            app_state.selected_process = app_state.selected_process.saturating_sub(app_state.visible_rows);
            app_state.scroll_offset = app_state.scroll_offset.saturating_sub(app_state.visible_rows);
        }
        KeyCode::Home => {
            app_state.selected_process = 0;
            app_state.scroll_offset = 0;
        }
        KeyCode::Left => {
            app_state.cycle_sort_left();
            app_state.process_cache.invalidate();
        }
        KeyCode::Right => {
            app_state.cycle_sort_right();
            app_state.process_cache.invalidate();
        }

        KeyCode::Char('b') => {
            add_sweep_effect(&mut app_state.effects, app_state.net_area);
            app_state.switch_to_loopback();
        }
        KeyCode::Char('n') => {
            add_sweep_effect(&mut app_state.effects, app_state.net_area);
            app_state.switch_to_ethernet();
        }
        
        KeyCode::Enter => {
            let motion = if app_state.show_info {
                Motion::LeftToRight
            } else {
                Motion::RightToLeft
            };
            
            let color = Color::from_u32(ANIMATION_COLOR);
            let timer = (ANIMATION_TIMER_MS, Interpolation::QuintInOut);
            app_state.effects.add_effect(
                fx::sweep_in(motion, 20, 10, color, timer).with_area(app_state.info_area)
            );
            app_state.toggle_info();
            app_state.invalidate_rows_cache();
        }

        KeyCode::Char('o') => {
            change_process_priority(processes, app_state.selected_process, "5");
        }
        KeyCode::Char('p') => {
            change_process_priority(processes, app_state.selected_process, "-5");
        }

        KeyCode::Char('u') => {
            add_sweep_effect(&mut app_state.effects, app_state.disk_area);
            app_state.previous_disk();
        }
        KeyCode::Char('i') => {
            add_sweep_effect(&mut app_state.effects, app_state.disk_area);
            // We'll need to pass the disk count from main
            app_state.next_disk(usize::MAX); // Temporary - should be actual disk count
        }
        
        KeyCode::Char('k') => {
            if let Some(proc) = processes.get(app_state.selected_process) {
                let pid = proc.pid().as_u32() as i32;
                
                app_state.effects.add_effect(
                    fx::dissolve((100, Interpolation::QuintInOut)).with_area(app_state.info_area)
                );
                
                unsafe {
                    kill(pid, SIGKILL);
                }
                
                app_state.effects.add_effect(
                    fx::coalesce((100, Interpolation::QuintInOut)).with_area(app_state.info_area)
                );
            }
        }
        
        KeyCode::Char('+') => {
            app_state.increase_refresh_interval();
        }
        KeyCode::Char('-') => {
            app_state.decrease_refresh_interval();
        }

        _ => {}
    }
    
    Ok(false)
}

fn add_sweep_effect(effects: &mut tachyonfx::EffectManager<()>, area: ratatui::layout::Rect) {
    let color = Color::from_u32(ANIMATION_COLOR);
    let timer = (ANIMATION_TIMER_MS, Interpolation::QuintInOut);
    effects.add_effect(
        fx::sweep_in(Motion::LeftToRight, 20, 10, color, timer).with_area(area)
    );
}

fn change_process_priority(processes: &Vec<&Process>, selected_process: usize, nice_value: &str) {
    if let Some(proc) = processes.get(selected_process) {
        let _ = std::process::Command::new("renice")
            .arg("-n")
            .arg(nice_value)
            .arg("-p")
            .arg(proc.pid().to_string())
            .status();
    }
}