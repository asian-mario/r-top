use std::{collections::VecDeque, time::Duration};
use ratatui::{prelude::*, symbols::bar::Set, widgets::*, style::*};
use sysinfo::{System, Networks, Disks, Process};
use crate::constants::*;
use crate::utils::format_bytes;
use crate::app_state::AppState;
use crate::system_info::{calculate_avg_cpu_history, get_busiest_core_info};

pub fn render_ui(
    frame: &mut ratatui::Frame,
    system: &System,
    networks: &Networks,
    disks: &Disks,
    processes: &Vec<&Process>,
    cpu_history: &Vec<VecDeque<f32>>,
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
    render_memory(frame, system, layout[2]);
    render_network(frame, networks, app_state, layout[3]);
    render_processes(frame, system, processes, app_state, layout[4]);

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
    cpu_history: &Vec<VecDeque<f32>>,
    app_state: &AppState,
    area: Rect,
) {
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
        .title_style(Style::default().fg(Color::White))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CUSTOM_PURPLE));

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

            let color = if usage < 50.0 {
                Color::Rgb((126.0 * (usage / 50.0)) as u8, 207, 126)
            } else {
                Color::Rgb(255, (255.0 * ((100.0 - usage) / 50.0)) as u8, 0)
            };

            let split = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(10), Constraint::Min(10)])
                .split(area);

            let label = Paragraph::new(format!("Core {:>2}", i))
                .style(Style::default().fg(Color::White));
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
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(cpu_chunks[1]);

    let avg_history = calculate_avg_cpu_history(cpu_history);
    let avg_cpu: f32 = system.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / system.cpus().len() as f32;

    let graph_color = if avg_cpu > 80.0 {
        Color::Red
    } else if avg_cpu > 50.0 {
        Color::Yellow
    } else if avg_cpu > 20.0 {
        Color::LightBlue
    } else {
        Color::White
    };

    let graph = Sparkline::default()
        .block(
            Block::default()
                .title(format!(
                    " CPU Avg Usage (0–100%) - {}ms | Set Refresh: +/- ",
                    app_state.refresh_interval.as_millis()
                ))
                .title_style(Style::default().fg(Color::White))
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
        .style(Style::default().fg(Color::Gray))
        .block(
            Block::default()
                .title(" CPU Info ")
                .title_style(Style::default().fg(Color::White))
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
    let avg_cpu: f32 = system.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / system.cpus().len() as f32;
    let (busiest_core_idx, busiest_core_usage, top_process_name, top_process_pid) = get_busiest_core_info(system);

    let avg_color = if avg_cpu > 80.0 {
        Color::Red
    } else if avg_cpu > 50.0 {
        Color::LightYellow
    } else {
        CUSTOM_PURPLE
    };

    let left = format!("Average CPU Usage: {:.2}% | ", avg_cpu);
    let right = format!(
        "Busiest Core : {} | {:.2}% - PID {} ({})",
        busiest_core_idx, busiest_core_usage, top_process_pid, top_process_name
    );

    let avg_text = Paragraph::new(format!("{:<10}{}", left, right))
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title(" CPU Average ")
                .title_style(Style::default().fg(Color::White))
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
        .title_style(Style::default().fg(Color::White))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CUSTOM_LIGHT_PURPLE));

    let usage = current_disk.total_space().saturating_sub(current_disk.available_space()) as f64
        / current_disk.total_space().max(1) as f64;

    let disk_gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(CUSTOM_G_PURPLE).bg(CUSTOM_BG_PURPLE))
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
    area: Rect,
) {
    // Memory gauge taking up 100% of the area (no spacing)
    let used = system.used_memory() as f64 / 1024.0 / 1024.0;
    let total = system.total_memory() as f64 / 1024.0 / 1024.0;
    let ratio = used / total;
    let mem_color = if ratio > 0.9 {
        Color::Red
    } else if ratio > 0.7 {
        Color::Yellow
    } else {
        CUSTOM_G_PURPLE
    };

    let memory_gauge = Gauge::default()
        .block(
            Block::default()
                .title(" Memory Usage ")
                .title_style(Style::default().fg(Color::White))
                .borders(Borders::ALL),
        )
        .set_style(Style::default().fg(CUSTOM_LIGHT_PURPLE))
        .gauge_style(Style::default().fg(mem_color).bg(CUSTOM_BG_PURPLE))
        .ratio(ratio)
        .label(format!("{:.2} / {:.2} GB", used / 1024.0, total / 1024.0));

    frame.render_widget(memory_gauge, area);
}

fn render_network(
    frame: &mut ratatui::Frame,
    networks: &Networks,
    app_state: &mut AppState,
    area: Rect,
) {
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
    .style(Style::default().fg(Color::White))
    .block(
        Block::default()
            .title(" Network Usage | Switch: b/n ")
            .title_style(Style::default().fg(Color::White))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    app_state.net_area = area;
    frame.render_widget(net_text, area);
}

fn render_processes(
    frame: &mut ratatui::Frame,
    system: &System,
    processes: &Vec<&Process>,
    app_state: &mut AppState,
    area: Rect,
) {
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
                Color::Red
            } else if usage > 20.0 {
                Color::Yellow
            } else {
                Color::White
            };

            Row::new(vec![
                proc.pid().to_string(),
                name_str,
                format!("{:.2}%", usage),
                format!("{:.2} MB", proc.memory() as f64 / 1024.0 / 1024.0),
            ])
            .style(Style::default().fg(if actual_index == app_state.selected_process {
                Color::LightCyan
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
                    .style(Style::default().fg(Color::Cyan)),
                );
            }
        }
    }

    let header = Row::new(vec!["PID", "Name", "CPU", "Memory"])
        .style(Style::default().fg(Color::Gray));

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
    .style(Style::default().fg(Color::LightCyan))
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
            .title_style(Style::default().fg(Color::White))
            .borders(Borders::ALL),
    );

    app_state.info_area = area;
    frame.render_widget(table, area);
}