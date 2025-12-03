use std::io;
use crossterm::event::{KeyEvent, KeyCode};
use sysinfo::{System, Process};
use tachyonfx::{fx, Motion, Interpolation};
use ratatui::prelude::Color;
use libc::{kill, SIGKILL};
use crate::app_state::AppState;
use crate::constants::{ANIMATION_COLOR, ANIMATION_TIMER_MS};
use crate::system_info::{filter_processes_cached, sort_processes_cached};

pub fn handle_key_event(
    key: KeyEvent,
    app_state: &mut AppState,
    system: &System, //-> yes i know its not used since we already have appstate, but i'm using this as a fallback incase I need to make some quick tests
    processes: &Vec<&Process>
) -> io::Result<bool> {
    // Handle popup dismissal first
    if app_state.popup_visible {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Enter | KeyCode::Esc => {
                app_state.dismiss_popup();
                return Ok(false);
            }
            _ => return Ok(false), // Ignore other keys when popup is visible
        }
    }

    if let KeyCode::Char('z') = key.code {
        app_state.effects.add_effect(
            fx::fade_from(Color::Black, Color::White, 100).with_area(app_state.terminal_area)
        );
        app_state.toggle_pause_overlay();
        return Ok(false);
    }

    if app_state.handle_search_input(key) {
        return Ok(false);
    }
    match key.code {
        KeyCode::Char('q') => return Ok(true),

        KeyCode::Char('/') => {
            app_state.toggle_search();
        }

        KeyCode::Esc => {
            if app_state.search_active {
                app_state.toggle_search();
            }
        }

        KeyCode::Tab => {
            if app_state.show_info {
                app_state.effects.add_effect(
                    fx::fade_from(Color::Black, Color::White, 100).with_area(app_state.info_area)
                );
                app_state.toggle_tree_view();
            }
        }
        KeyCode::Down => {
            if app_state.show_info && app_state.show_tree_view {
                app_state.tree_navigate_down();
            } else {
                if app_state.selected_process + 1 < processes.len() {
                    app_state.selected_process += 1;
                    if app_state.selected_process >= app_state.scroll_offset + app_state.visible_rows {
                        app_state.scroll_offset += 1;
                    }
                }
            }
        }
        KeyCode::Up => {
            if app_state.show_info && app_state.show_tree_view {
                app_state.tree_navigate_up();
            } else {
                if app_state.selected_process > 0 {
                    app_state.selected_process -= 1;
                    if app_state.selected_process < app_state.scroll_offset {
                        app_state.scroll_offset = app_state.scroll_offset.saturating_sub(1);
                    }
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
        /*
            will have to adjust controls to check whens the tree is visible
        */
        KeyCode::Left => {
            if app_state.show_info && app_state.show_tree_view {
                app_state.collapse_current_node();
            } else {
                app_state.cycle_sort_left();
                app_state.process_cache.invalidate();
            }
        }

        KeyCode::Right => {
            if app_state.show_info && app_state.show_tree_view {
                app_state.expand_current_node();
            } else {
                app_state.cycle_sort_right();
                app_state.process_cache.invalidate();
            }
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
            if app_state.show_info && app_state.show_tree_view {
                if let Some(item) = app_state.get_selected_tree_item() {
                    if item.has_children {
                        app_state.toggle_tree_node(item.pid);
                    }
                }
            } else {
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
        }

        KeyCode::Char('o') => {
            if let Err(err) = change_process_priority(processes, app_state.selected_process, "5") {
                app_state.show_popup(err);
            }
        }
        KeyCode::Char('p') => {
            if let Err(err) = change_process_priority(processes, app_state.selected_process, "-5") {
                app_state.show_popup(err);
            }
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
            let target_pid = if app_state.show_info && app_state.show_tree_view {
                app_state.get_selected_tree_item().map(|item| item.pid.as_u32() as i32)
            } else {
                let actual_process = if app_state.search_active && !app_state.is_search_empty() {
                    let sorted_processes = sort_processes_cached(system, &app_state.sort_category, &mut app_state.process_cache, &app_state.search_active);
                    filter_processes_cached(system, &sorted_processes, app_state)
                } else {
                    processes.clone()
                };

                actual_process.get(app_state.selected_process).map(|proc| proc.pid().as_u32() as i32)
            };

            if let Some(pid) = target_pid {
                app_state.effects.add_effect(
                    fx::dissolve((100, Interpolation::QuintInOut)).with_area(app_state.info_area)
                );
                
                unsafe {
                    kill(pid, SIGKILL);
                }
                
                app_state.effects.add_effect(
                    fx::coalesce((100, Interpolation::QuintInOut)).with_area(app_state.info_area)
                );
                
                app_state.invalidate_tree_cache();
                app_state.invalidate_search_cache();
                app_state.process_cache.invalidate();
            }
        }
        
        KeyCode::Char('+') => {
            app_state.increase_refresh_interval();
        }
        KeyCode::Char('-') => {
            app_state.decrease_refresh_interval();
        }

        KeyCode::Char('t') => {
            app_state.effects.add_effect(
                fx::sweep_in(Motion::RightToLeft, 20, 10, Color::from_u32(ANIMATION_COLOR), (ANIMATION_TIMER_MS, Interpolation::QuintInOut)).with_area(app_state.terminal_area)
            );

            app_state.switch_theme();
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

fn change_process_priority(processes: &Vec<&Process>, selected_process: usize, nice_value: &str) -> Result<(), String> {
    if let Some(proc) = processes.get(selected_process) {
        let output = std::process::Command::new("renice")
            .arg("-n")
            .arg(nice_value)
            .arg("-p")
            .arg(proc.pid().to_string())
            .output()
            .map_err(|e| format!("Failed to execute renice: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Permission denied: Failed to change process priority.\n\n\
                Error: {}\n\n\
                Try running r-top with sudo:\n  sudo r-top",
                stderr.trim()
            ));
        }
        Ok(())
    } else {
        Err("Process not found".to_string())
    }
}