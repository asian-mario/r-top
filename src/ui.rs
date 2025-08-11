use std::os::unix::process;
use std::thread::current;
use std::{time::Duration};
use ratatui::{prelude::*, symbols::bar::Set, widgets::*, style::*};
use sysinfo::{System, Networks, Disks, Process};
use crate::constants::*;
use crate::utils::{format_bytes, CircularBuffer};
use crate::app_state::{AppState, SearchType};
use crate::system_info::{sort_and_filter_processes_cached, get_actual_process_index, get_filtered_process_count,calculate_avg_cpu_history, get_busiest_core_info, build_process_tree, get_tree_stats, memory_used_gib};

pub fn render_ui(
    frame: &mut ratatui::Frame,
    system: &System,
    networks: &Networks,
    disks: &Disks,
    processes: &Vec<&Process>,
    cpu_history: &Vec<CircularBuffer<f32>>,
    app_state: &mut AppState,
) {
    let area = frame.area();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(system.cpus().len() as u16 + 4),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(5),
        ])
        .split(area);
    /*
        lalala this is stupid!
        i *really* need to make this more dynamic
     */

    render_cpu_section(frame, system, cpu_history, app_state, layout[0]);
    render_cpu_average(frame, system, disks, app_state, layout[1]);
    render_memory(frame, system, app_state, layout[2]);
    render_network(frame, networks, app_state, layout[3]);
    render_processes_optimized(frame, system, processes, app_state, layout[4]);

    // Process effects
    app_state.effects.process_effects(
        Duration::from_millis(16).into(),
        frame.buffer_mut(),
        area,
    );
}

fn render_cpu_section(
    frame: &mut ratatui::Frame,
    system: &System,
    cpu_history: &Vec<CircularBuffer<f32>>,
    app_state: &AppState,
    area: Rect,
) {
    let theme = app_state.theme_manager.current_theme();
    /*
    FOR SOME REASON! before the refactor 70/30 was FINE! it displayed all the things in CPU info but now its not?? I don't want to tweak this b.s again because
    i'm essentially choosing do I want a graph that actually has some meaning to it or cpu info
    -> also the monitor im working with is pretty buns so it'll look better on other monitors
     */
    let cpu_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    // Draw CPU cores
    let bordered_block = Block::default()
        .title(" CPU Usage ")
        .title_style(Style::default().fg(theme.primary_text))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.primary_border));

    frame.render_widget(&bordered_block, cpu_chunks[0]);
    let inner_area = bordered_block.inner(cpu_chunks[0]);

    let core_count = system.cpus().len();
    let max_rows = 8;
    let columns = (core_count + max_rows - 1) / max_rows;

    let core_columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Percentage(100 / columns as u16); columns])
        .split(inner_area);

    for (col, chunk) in core_columns.iter().enumerate() {
        let start = col * max_rows;
        let end = ((col + 1) * max_rows).min(core_count);

        let rows: Vec<Rect> = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(1); end - start])
            .split(*chunk)
            .to_vec();

        for (i, area) in (start..end).zip(rows.into_iter()) {
            let cpu = &system.cpus()[i];
            let usage = cpu.cpu_usage();
            let ratio = (usage / 100.0).max(0.01);

            let color = if usage > 80.0 {
                theme.cpu_critical
            } else if usage > 50.0 {
                theme.cpu_high
            } else if usage > 20.0 {
                theme.cpu_medium
            } else {
                theme.cpu_low
            };

            let split = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(10), Constraint::Min(10)])
                .split(area);

            let label = Paragraph::new(format!("Core {:>2}", i))
                .style(Style::default().fg(theme.primary_text));
            frame.render_widget(label, split[0]);

            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(color))
                .ratio(ratio as f64)
                .label(format!("{:>5.1}%", usage));
            frame.render_widget(gauge, split[1]);
        }
    }

    // Draw CPU graph and info
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(cpu_chunks[1]);

    let avg_history = calculate_avg_cpu_history(cpu_history);
    let avg_cpu: f32 = system.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / system.cpus().len() as f32;

    let graph_color = if avg_cpu > 80.0 {
        theme.cpu_critical
    } else if avg_cpu > 50.0 {
        theme.cpu_high
    } else if avg_cpu > 20.0 {
        theme.cpu_medium
    } else {
        theme.cpu_low
    };

    let graph = Sparkline::default()
        .block(
            Block::default()
                .title(format!(
                    " CPU Avg Usage (0–100%) - {}ms | Set Refresh: +/- ",
                    app_state.refresh_interval.as_millis()
                ))
                .title_style(Style::default().fg(theme.primary_text))
                .borders(Borders::ALL),
        )
        .style(Style::default().fg(graph_color))
        .data(&avg_history)
        .max(100)
        .bar_set(Set::default());

    frame.render_widget(graph, right_chunks[0]);

    // CPU Info
    let cpu_info = &system.cpus()[0];
    let logical_threads = system.cpus().len();
    let physical_cores = System::physical_core_count().unwrap_or(logical_threads);
    let current_speed = cpu_info.frequency();

    let cpu_info_text = format!(
        "Model: {}\n\
        Physical Cores:  {}\n\
        Logical Threads: {}\n\
        Base Clock Speed: {} MHz",
        cpu_info.brand(),
        physical_cores,
        logical_threads,
        current_speed
    );

    let cpu_info_paragraph = Paragraph::new(cpu_info_text)
        .style(Style::default().fg(theme.secondary_text))
        .block(
            Block::default()
                .title(" CPU Info ")
                .title_style(Style::default().fg(theme.primary_text))
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(cpu_info_paragraph, right_chunks[1]);
}

fn render_cpu_average(
    frame: &mut ratatui::Frame, 
    system: &System, 
    disks: &Disks,
    app_state: &mut AppState,
    area: Rect
) {
    let theme = app_state.theme_manager.current_theme();
    let avg_cpu: f32 = system.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / system.cpus().len() as f32;
    let (busiest_core_idx, busiest_core_usage, top_process_name, top_process_pid) = get_busiest_core_info(system);

    let avg_color = if avg_cpu > 80.0 {
        theme.cpu_critical
    } else if avg_cpu > 50.0 {
        theme.cpu_high
    } else {
        theme.primary_border
    };

    let left = format!("Average CPU Usage: {:.2}% | ", avg_cpu);
    let right = format!(
        "Busiest Core : {} | {:.2}% - PID {} ({})",
        busiest_core_idx, busiest_core_usage, top_process_pid, top_process_name
    );

    let avg_text = Paragraph::new(format!("{:<10}{}", left, right))
        .style(Style::default().fg(theme.primary_text))
        .block(
            Block::default()
                .title(" CPU Average ")
                .title_style(Style::default().fg(theme.primary_text))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(avg_color)),
        );

    // Split the CPU average section: 70% for CPU info, 30% for disk
    let avg_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    frame.render_widget(avg_text, avg_chunks[0]);

    // Disk gauge in the CPU section (30% space)
    let disks_list = disks.list();
    let current_disk = disks_list.get(app_state.current_disk_index).unwrap_or(&disks_list[0]);
    let name = current_disk.name().to_string_lossy();
    let title = format!("Disk: {} | Switch Disk: u/i ", name);

    let disk_block = Block::default()
        .title(title)
        .title_style(Style::default().fg(theme.primary_text))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.secondary_border));

    let usage = current_disk.total_space().saturating_sub(current_disk.available_space()) as f64
        / current_disk.total_space().max(1) as f64;

    let disk_gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(theme.gauge_primary).bg(theme.gauge_background))
        .ratio(usage)
        .label(format!("{:.1}%", usage * 100.0));

    let disk_inner = disk_block.inner(avg_chunks[1]);
    app_state.disk_area = avg_chunks[1];
    frame.render_widget(disk_block, avg_chunks[1]);
    frame.render_widget(disk_gauge, disk_inner);
}

fn render_memory(
    frame: &mut ratatui::Frame,
    system: &System,
    app_state: &AppState,
    area: Rect,
) {
    let theme = app_state.theme_manager.current_theme();
    // Memory gauge taking up 100% of the area (no spacing)
    let used_gib = memory_used_gib(system);       
    let total_gib = (system.total_memory() as f64) / 1024.0 / 1024.0; 
    let ratio = (used_gib / total_gib).clamp(0.0, 1.0);
    let mem_color = if ratio > 0.9 {
        theme.memory_critical
    } else if ratio > 0.7 {
        theme.memory_warning
    } else {
        theme.memory_normal
    };

    let memory_gauge = Gauge::default()
        .block(
            Block::default()
                .title(" Memory Usage ")
                .title_style(Style::default().fg(theme.primary_text))
                .borders(Borders::ALL),
        )
        .set_style(Style::default().fg(theme.secondary_border))
        .gauge_style(Style::default().fg(mem_color).bg(theme.gauge_background))
        .ratio(ratio)
        .label(format!("{:.2} / {:.2} GiB", used_gib / 1024.0, total_gib / 1024.0));

    frame.render_widget(memory_gauge, area);
}

fn render_network(
    frame: &mut ratatui::Frame,
    networks: &Networks,
    app_state: &mut AppState,
    area: Rect,
) {
    let theme = app_state.theme_manager.current_theme();
    let eth0 = networks.get("eth0");
    let lo = networks.get("lo");
    let net = if app_state.current_interface == "eth0" { eth0 } else { lo };

    let (rx, tx) = match net {
        Some(n) => (n.received(), n.transmitted()),
        None => (0, 0),
    };

    let (_delta_rx, _delta_tx, total_delta) = unsafe {
        let delta_rx = rx.saturating_sub(PREV_RX);
        let delta_tx = tx.saturating_sub(PREV_TX);
        PREV_RX = rx;
        PREV_TX = tx;
        let total_delta = delta_rx + delta_tx;
        (delta_rx, delta_tx, total_delta)
    };

    unsafe {
        SESSION_TOTAL_BYTES += total_delta;
    }

    let total_mib = (rx + tx) as f64 / 1024.0 / 1024.0;
    let session_mib = unsafe { SESSION_TOTAL_BYTES as f64 / 1024.0 / 1024.0 };

    let net_text = Paragraph::new(format!(
        "{} | ▽ RX: {} | △ TX: {} | ▷ RX+TX: {:.2} MiB | Session: {:.2} MiB",
        app_state.current_interface,
        format_bytes(rx),
        format_bytes(tx),
        total_mib,
        session_mib
    ))
    .style(Style::default().fg(theme.primary_text))
    .block(
        Block::default()
            .title(" Network Usage | Switch: b/n ")
            .title_style(Style::default().fg(theme.primary_text))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.network_border)),
    );

    app_state.net_area = area;
    frame.render_widget(net_text, area);
}

fn render_processes_optimized(
    frame: &mut ratatui::Frame,
    system: &System,
    processes: &Vec<&Process>,
    app_state: &mut AppState,
    area: Rect,
) {
    let theme = app_state.theme_manager.current_theme();
    let (search_area, process_area) = if app_state.search_active {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(5)])
            .split(area);
        (Some(chunks[0]), chunks[1])
    } else {
        (None, area)
    };

    if let Some(search_area) = search_area {
        render_search_bar(frame, app_state, search_area);
    }

    
    let filtered_processes = sort_and_filter_processes_cached(system, app_state);

    app_state.visible_rows = area.height.saturating_sub(3) as usize;

    let max_processes = filtered_processes.len();
    if app_state.selected_process >= max_processes && max_processes > 0 {
        app_state.selected_process = max_processes - 1;
    }
    
    let show_tree = app_state.show_info && app_state.show_tree_view;

    if show_tree{
        render_tree_view(frame, system, app_state, process_area);
    } else {
        render_flat_view(frame, system, &filtered_processes, app_state, process_area);
    }
}

fn render_tree_view(
    frame: &mut ratatui::Frame,
    system: &System,
    app_state: &mut AppState,
    area: Rect,
) {
    let tree_items = build_process_tree(system, app_state);

    let theme = app_state.theme_manager.current_theme();
    
    // mutable borrows kind of confusing
    let visible_rows = app_state.visible_rows;
    let tree_selected_index = app_state.tree_selected_index;
    
    // tree rows for visible items
    let mut rows: Vec<Row> = Vec::new();
    let visible_start = tree_selected_index.saturating_sub(visible_rows / 2);
    let visible_end = (visible_start + visible_rows).min(tree_items.len());
    
    for (i, item) in tree_items
        .iter()
        .skip(visible_start)
        .take(visible_end - visible_start)
        .enumerate()
    {
        let actual_index = visible_start + i;
        let is_selected = actual_index == tree_selected_index;
        
        // Create indentation based on tree level
        let indent = "  ".repeat(item.level);
        let expansion_indicator = if item.has_children {
            if item.is_expanded { "▼ " } else { "▶ " }
        } else {
            "  "
        };
        
        let name_with_tree = format!("{}{}{}", indent, expansion_indicator, item.name);
        
      
        let color = if is_selected {
            theme.process_selected
        } else if item.cpu_usage > 50.0 {
            theme.process_high_cpu
        } else if item.level > 0 {
            theme.secondary_text // child process dimming
        } else {
            theme.process_normal
        };

        let row = Row::new(vec![
            item.pid.to_string(),
            name_with_tree,
            format!("{:.2}%", item.cpu_usage),
            format!("{:.2} MB", item.memory as f64 / 1024.0 / 1024.0),
        ])
        .style(Style::default().fg(color));

        rows.push(row);
    }

    let (total_processes, expanded_nodes, max_depth) = get_tree_stats(&tree_items);
    
    let header = Row::new(vec!["PID", "Process Tree", "CPU", "Memory"])
        .style(Style::default().fg(theme.secondary_text));

    let table = Table::new(
        rows,
        &[
            Constraint::Length(8),
            Constraint::Percentage(60),
            Constraint::Length(10),
            Constraint::Length(15),
        ],
    )
    .header(header)
    .style(Style::default().fg(theme.highlight_text))
    .block(
        Block::default()
            .title(format!(
                " Process Tree ({}/{} processes, {} expanded, depth {}) - Tab: Switch View | ←→: Expand/Collapse | Enter: Toggle | k: Kill ",
                tree_items.len(),
                total_processes,
                expanded_nodes,
                max_depth + 1
            ))
            .title_style(Style::default().fg(theme.primary_text))
            .borders(Borders::ALL),
    );

    app_state.info_area = area;
    frame.render_widget(table, area);
}

fn render_search_bar(
    frame: &mut ratatui::Frame,
    app_state: &AppState,
    area: Rect,
) {
    let theme = app_state.theme_manager.current_theme();

    let (search_text, title) = if app_state.search_query.is_empty() {
        ("Type to search processes... (use 'pid:1234' for PID search)".to_string(),
        " Search Processes (ESC to exit, / to toggle) ")
    } else {
        let search_type = app_state.get_search_type();
        let search_value = app_state.get_search_value();

        match search_type {
            SearchType::Pid => {
                (format!("pid:{}", search_value), " PID Search (ESC to exit) ")
            }
            SearchType::Name => {
                (app_state.search_query.clone(), " Name Search (ESC to exit) ")
            }
        }
        
    };

    let search_style = if app_state.search_query.is_empty() {
        Style::default().fg(theme.secondary_text)
    } else {
        Style::default().fg(theme.primary_text)
    };

    let search_widget = Paragraph::new(search_text)
        .style(search_style) 
        .block(
            Block::default()
                .title(title)
                .title_style(Style::default().fg(theme.highlight_text))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.active_border))
        );

    frame.render_widget(search_widget, area);
}

fn render_flat_view(
    frame: &mut ratatui::Frame,
    system: &System,
    processes: &Vec<&Process>,
    app_state: &mut AppState,
    area: Rect,
) {
    let theme = app_state.theme_manager.current_theme();
    let num_cores = system.cpus().len() as f32;
    let current_process_count = processes.len();
    
    // Check if we need to rebuild the row cache (same logic as before)
    let needs_rebuild = !app_state.rows_cache_valid
        || app_state.last_process_count != current_process_count
        || app_state.last_scroll_offset != app_state.scroll_offset
        || app_state.last_selected_process != app_state.selected_process
        || !app_state.search_cache_valid
        || app_state.search_active;

    if needs_rebuild {
        // Clear and rebuild cache (same as existing logic)
        app_state.cached_rows.clear();
        
        if app_state.cached_rows.capacity() < app_state.visible_rows {
            app_state.cached_rows.reserve(app_state.visible_rows);
        }
        
        for (i, proc) in processes
            .iter()
            .skip(app_state.scroll_offset)
            .take(app_state.visible_rows)
            .enumerate()
        {
            let actual_index = app_state.scroll_offset + i;
            let name_str = proc.name().to_string_lossy().into_owned();
            let usage = proc.cpu_usage() / num_cores;
            
            let mut color = if usage > 50.0 {
                theme.process_high_cpu
            } else if actual_index == app_state.selected_process {
                theme.process_selected
            } else {
                theme.process_normal
            };

            if app_state.search_active && !app_state.is_search_empty() {
                if name_str.to_lowercase().contains(&app_state.search_query.to_lowercase()) {
                    color = if actual_index == app_state.selected_process {
                        theme.secondary_text
                    } else {
                        theme.highlight_text
                    };
                }
            }

            let row = Row::new(vec![
                proc.pid().to_string(),
                name_str,
                format!("{:.2}%", usage),
                format!("{:.2} MB", proc.memory() as f64 / 1024.0 / 1024.0),
            ])
            .style(Style::default().fg(color));

            app_state.cached_rows.push(row);
        }
        
        // Handle info insertion if needed
        if app_state.show_info {
            if let Some(proc) = processes.get(app_state.selected_process) {
                let insert_index = app_state.selected_process.saturating_sub(app_state.scroll_offset);
                if insert_index < app_state.cached_rows.len() {
                    let args = proc
                        .cmd()
                        .iter()
                        .map(|s| s.to_string_lossy())
                        .collect::<Vec<_>>()
                        .join(" ");

                    let thread_count = proc.tasks().map_or(0, |tasks| tasks.len());
                    
                    let info_row = Row::new(vec![
                        format!("Args: {:?}", args),
                        format!("Threads: {}", thread_count),
                        format!("Core: {}", proc.cpu_usage() as usize % system.cpus().len()),
                        format!("Status: {:?}", proc.status()),
                    ])
                    .style(Style::default().fg(theme.process_info));
                    
                    app_state.cached_rows.insert(insert_index + 1, info_row);
                }
            }
        }

        // Update cache validity markers
        app_state.last_process_count = current_process_count;
        app_state.last_scroll_offset = app_state.scroll_offset;
        app_state.last_selected_process = app_state.selected_process;
        app_state.rows_cache_valid = true;
    }

    let header = Row::new(vec!["PID", "Name", "CPU", "Memory"])
        .style(Style::default().fg(theme.secondary_text));

    let title_extra = if app_state.show_info { " | Tab: Tree View" } else { "" };

    let search_info = if app_state.search_active {
        let search_type = app_state.get_search_type();
        let results_count = current_process_count;
        
        match search_type {
            SearchType::Pid => format!(" | PID Search: {} result(s)", results_count),
            SearchType::Name => format!(" | Name Search: {}/{}", results_count, app_state.last_process_count),
        }
    } else {
        String::new()
    };

    let table = Table::new(
        app_state.cached_rows.clone(),
        &[
            Constraint::Percentage(50),
            Constraint::Percentage(30),
            Constraint::Length(10),
            Constraint::Length(20),
        ],
    )
    .header(header)
    .style(Style::default().fg(theme.highlight_text))
    .block(
        Block::default()
            .title(format!(
                " Top Processes - /: Search{}{} | Enter: Info{} | o/p: Nice | k: kill | {}",
                search_info,
                if app_state.search_active{" | ESC: Exit Search"} else {""},
                title_extra,
                app_state.sort_category.as_str()
            ))
            .title_style(Style::default().fg(theme.primary_text))
            .borders(Borders::ALL),
    );

    app_state.info_area = area;
    frame.render_widget(table, area);
}

#[deprecated(note = "deprecated! use render_processes_optimized() instead!")]
fn render_processes(
    frame: &mut ratatui::Frame,
    system: &System,
    processes: &Vec<&Process>,
    app_state: &mut AppState,
    area: Rect,
) {
    let theme = app_state.theme_manager.current_theme();
    let num_cores = system.cpus().len() as f32;
    app_state.visible_rows = area.height.saturating_sub(3) as usize;
    /*
        i should really keep a cache of the processes list instead of recreated the vec everyframe. very bad
        TODO:
        cache proc list
     */
    let mut rows: Vec<Row> = processes
        .iter()
        .skip(app_state.scroll_offset)
        .take(app_state.visible_rows)
        .enumerate()
        .map(|(i, proc)| {
            let actual_index = app_state.scroll_offset + i;
            let name_str = proc.name().to_string_lossy().to_string();
            let usage = proc.cpu_usage() / num_cores;
            let color = if usage > 50.0 {
                theme.process_high_cpu
            } else if usage > 20.0 {
                theme.cpu_medium
            } else {
                theme.process_normal
            };

            Row::new(vec![
                proc.pid().to_string(),
                name_str,
                format!("{:.2}%", usage),
                format!("{:.2} MB", proc.memory() as f64 / 1024.0 / 1024.0),
            ])
            .style(Style::default().fg(if actual_index == app_state.selected_process {
                theme.process_selected
            } else {
                color
            }))
        })
        .collect();

    if app_state.show_info {
        if let Some(proc) = processes.get(app_state.selected_process) {
            let insert_index = app_state.selected_process.saturating_sub(app_state.scroll_offset);
            if insert_index < rows.len() {
                let args = proc
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ");

                let thread_count = proc.tasks().map_or(0, |tasks| tasks.len());
                rows.insert(
                    insert_index + 1,
                    Row::new(vec![
                        format!("Args: {:?}", args),
                        format!("Threads: {}", thread_count),
                        format!("Core: {}", proc.cpu_usage() as usize % system.cpus().len()),
                        format!("Status: {:?}", proc.status()),
                    ])
                    .style(Style::default().fg(theme.process_info)),
                );
            }
        }
    }

    let header = Row::new(vec!["PID", "Name", "CPU", "Memory"])
        .style(Style::default().fg(theme.secondary_text));

    let table = Table::new(
        rows,
        &[
            Constraint::Percentage(50),
            Constraint::Percentage(30),
            Constraint::Length(10),
            Constraint::Length(20),
        ],
    )
    .header(header)
    .style(Style::default().fg(theme.highlight_text))
    .block(
        Block::default()
        /*
            i have to check if Nice even works, its been a while.
            -> WORKS!
         */
            .title(format!(
                " Top Processes - Enter: Info | o/p: Nice | k: kill | {}",
                app_state.sort_category.as_str()
            ))
            .title_style(Style::default().fg(theme.primary_text))
            .borders(Borders::ALL),
    );

    app_state.info_area = area;
    frame.render_widget(table, area);
}