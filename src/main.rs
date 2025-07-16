use std::{collections::VecDeque, io, time::{Duration, Instant}};
use ratatui::{prelude::*, style::Styled, symbols::bar::Set, widgets::*};
use crossterm::event::{self, Event, KeyCode};
use tachyonfx::{fx, EffectManager, Motion, Interpolation};
use sysinfo::{System, RefreshKind, Networks, Disks};
use crate::block::Title;
use libc::{kill, SIGKILL};


const HISTORY_LEN: usize = 50;
static mut SESSION_TOTAL_BYTES: u64 = 0;
static mut PREV_RX: u64 = 0;
static mut PREV_TX: u64 = 0;


enum SortCategory {
    CpuPerCore,
    CpuAverage,
    Memory,
    Network,
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.2} GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    } else if bytes >= 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / 1024.0 / 1024.0)
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}


fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut effects: EffectManager<()> = EffectManager::default();
    effects.add_effect(fx::coalesce((400, tachyonfx::Interpolation::QuintInOut)));

    let refresh = RefreshKind::everything();
    let mut system = System::new_with_specifics(refresh);
    let mut networks = Networks::new_with_refreshed_list();
    let disks = Disks::new_with_refreshed_list();

    let mut cpu_history: Vec<VecDeque<f32>> = vec![];
    let mut last_refresh = Instant::now();
    let mut refresh_interval = Duration::from_millis(2000);
    let mut selected_process = 0;
    let mut sort_category = SortCategory::CpuPerCore;
    let mut current_interface = "eth0";
    let mut show_info = false;
    let mut current_disk_index: usize = 0;

    //animated areas
    let mut info_area = ratatui::layout::Rect::default();
    let mut net_area = ratatui::layout::Rect::default();
    let mut disk_area = ratatui::layout::Rect::default();

    //custom colors
    let custom_green = Color::Rgb(100, 149, 107);
    let custom_yellow = Color::Rgb(138, 136, 46);

    let sweep_duration_ms = 300; 
    let switch_interface_at = Instant::now() + Duration::from_millis(sweep_duration_ms);
    let mut scroll_offset: usize = 0;
    let mut visible_rows: usize = 0;


    Duration::from_millis(sweep_duration_ms);

    loop {
        let now = Instant::now();
        if now.duration_since(last_refresh) >= refresh_interval {
            system.refresh_all();
            networks.refresh(false);
            /*rant:
                I understand that sysinfo is still under development but WHY does refreshing all the entire system to update proc data NOT also refresh the netwroks
                it took me ages because I have the combined IQ of a lukewarm mayonnaise jar to figure it out.
             */
            last_refresh = now;
        }

        if cpu_history.is_empty() {
            cpu_history = vec![VecDeque::from(vec![0.0; HISTORY_LEN]); system.cpus().len()];
        }

        for (i, cpu) in system.cpus().iter().enumerate() {
            if let Some(buf) = cpu_history.get_mut(i) {
                if buf.len() >= HISTORY_LEN {
                    buf.pop_front();
                }
                buf.push_back(cpu.cpu_usage());
            }
        }

        let num_cores = system.cpus().len() as f32;
        let mut processes: Vec<_> = system.processes().values().collect();

        match sort_category {
            SortCategory::CpuPerCore | SortCategory::CpuAverage => {
                processes.sort_by(|a, b| {
                    let a_usage = a.cpu_usage() / num_cores;
                    let b_usage = b.cpu_usage() / num_cores;
                    b_usage.partial_cmp(&a_usage).unwrap()
                });
            }
            SortCategory::Memory => {
                processes.sort_by(|a, b| b.memory().cmp(&a.memory()));
            }
            SortCategory::Network => {}
        }

        let avg_cpu: f32 = system.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / system.cpus().len() as f32;
        let eth0 = networks.get("eth0");
        let lo = networks.get("lo");

        terminal.draw(|frame| {
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

            let cpu_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(70),
                    Constraint::Percentage(30),
                ])
                .split(layout[0]);

            // Draw the bordered block first
            let bordered_block = Block::default()
                .title(" CPU Usage ")
                .title_style(Style::default().fg(Color::White)) 
                .borders(Borders::ALL)
                .border_style(Style::default().fg(custom_green));

            frame.render_widget(&bordered_block, cpu_chunks[0]);

            // Get the inner area to render content inside the border
            let inner_area = bordered_block.inner(cpu_chunks[0]);

            let core_count = system.cpus().len();
            let max_rows = 8; // Adjust based on terminal height
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
                    let ratio = (usage / 100.0).max(0.01); // Ensure at least a tiny bar

                    let color = if usage < 50.0 {
                        Color::Rgb((255.0 * (usage / 50.0)) as u8, 255, 0)
                    } else {
                        Color::Rgb(255, (255.0 * ((100.0 - usage) / 50.0)) as u8, 0)
                    };

                    // Split each row into label and bar
                    let split = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Length(10), Constraint::Min(10)])
                        .split(area);

                    let label_area = split[0];
                    let bar_area = split[1];

                    let label = Paragraph::new(format!("Core {:>2}", i))
                        .style(Style::default().fg(Color::White));
                    frame.render_widget(label, label_area);

                    let gauge = Gauge::default()
                        .gauge_style(Style::default().fg(color).bg(Color::Black))
                        .ratio(ratio as f64)
                        .label(format!("{:>5.1}%", usage));
                    frame.render_widget(gauge, bar_area);
                }
            }

            let avg_history: Vec<u64> = (0..HISTORY_LEN)
                .map(|i| {
                    let sum: f32 = cpu_history
                        .iter()
                        .filter_map(|core| core.get(i))
                        .sum();
                    let count = cpu_history.iter().filter(|core| core.get(i).is_some()).count();
                    if count > 0 {
                        (sum / count as f32) as u64
                    } else {
                        0
                    }
                })
                .collect();

            let graph_color = if avg_cpu > 80.0 {
                Color::Red
            } else if avg_cpu > 50.0 {
                Color::Yellow
            } else if avg_cpu > 20.0 {
                Color::LightBlue
            } 
            else {
                Color::White
            };

            let graph = Sparkline::default()
                .block(Block::default()
                    .title(format!(" CPU Avg Usage (0–100%) - {}ms | Set Refresh: +/- ", refresh_interval.as_millis()))
                    .title_style(Style::default().fg(Color::White)) 
                    .borders(Borders::ALL))
                .style(Style::default().fg(graph_color))
                .data(&avg_history)
                .max(100)
                .bar_set(Set::default());

            let right_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ])
                .split(cpu_chunks[1]);


            frame.render_widget(graph, right_chunks[0]);

            
            let cpu_info = &system.cpus()[0];
            let logical_threads = system.cpus().len();
            let physical_cores = System::physical_core_count()
                .unwrap_or(logical_threads);
            let current_speed = cpu_info.frequency();

            let cpu_info_text = format!(
                "Model:           {}\n\
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
                .block(Block::default().title(" CPU Info ").title_style(Style::default().fg(Color::White)).borders(Borders::ALL)).wrap(Wrap { trim: false });

            frame.render_widget(cpu_info_paragraph, right_chunks[1]);


            let avg_color = if avg_cpu > 80.0 {
                Color::Red
            } else if avg_cpu > 50.0 {
                Color::LightYellow
            } else {
                custom_green
            };

            // Find the hardest working core and its top process
            let (busiest_core_idx, busiest_core_usage) = system
                .cpus()
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.cpu_usage().partial_cmp(&b.cpu_usage()).unwrap())
                .unwrap_or((0, &system.cpus()[0]));

            let mut top_process_name = "N/A".to_string();
            let mut top_process_pid = 0;

            for process in system.processes().values() {
                if process.cpu_usage() > 0.0 && process.cpu_usage() as usize % system.cpus().len() == busiest_core_idx {
                    top_process_name = process.name().to_string_lossy().to_string();
                    top_process_pid = process.pid().as_u32();
                    break;
                }
            }

            let left = format!("Average CPU Usage: {:.2}% | ", avg_cpu);
            let right = format!(
                "Busiest Core : {} | {:.2}% - PID {} ({})",
                busiest_core_idx,
                busiest_core_usage.cpu_usage(),
                top_process_pid,
                top_process_name
            );


            let avg_text = Paragraph::new(format!("{:<10}{}", left, right))
                .style(Style::default().fg(Color::White)) 
                .block(
                    Block::default()
                        .title(" CPU Average ")
                        .title_style(Style::default().fg(Color::White)) 
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(avg_color)) 
                );


            let avg_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(70),
                    Constraint::Percentage(30),
                ])
                .split(layout[1]);

            frame.render_widget(avg_text, avg_chunks[0]);

            let disks_list = disks.list();

            let current_disk = disks_list.get(current_disk_index).unwrap_or(&disks_list[0]);
            let name = current_disk.name().to_string_lossy();
            
            let title = format!("Disk: {} | Switch Disk: u/i ", name);

            let disk_block = Block::default()
                .title(title)
                .title_style(Style::default().fg(Color::White)) 
                .borders(Borders::ALL)
                .border_style(Style::default().fg(custom_yellow));

            let usage = current_disk.total_space().saturating_sub(current_disk.available_space()) as f64
                / current_disk.total_space().max(1) as f64;

            let disk_gauge = Gauge::default()
                .block(Block::default().borders(Borders::NONE))
                .gauge_style(Style::default().fg(Color::Green).bg(Color::Black))
                .ratio(usage)
                .label(format!("{:.1}%", usage * 100.0));
            
            let disk_inner = disk_block.inner(avg_chunks[1]);
            
            disk_area = avg_chunks[1];
            frame.render_widget(disk_block, avg_chunks[1]);
            frame.render_widget(disk_gauge, disk_inner);



            let used = system.used_memory() as f64 / 1024.0 / 1024.0;
            let total = system.total_memory() as f64 / 1024.0 / 1024.0;
            let ratio = used / total;
            let mem_color = if ratio > 0.9 {
                Color::Red
            } else if ratio > 0.7 {
                Color::Yellow
            } else {
                Color::Green
            };
            let gauge = Gauge::default()
                .block(Block::default().title(" Memory Usage ").title_style(Style::default().fg(Color::White)).borders(Borders::ALL)).set_style(Style::default().fg(custom_yellow))
                .gauge_style(Style::default().fg(mem_color).bg(Color::Black))
                .ratio(ratio)
                .label(format!("{:.2} / {:.2} GB", used / 1024.0, total / 1024.0));
            frame.render_widget(gauge, layout[2]);

            let net = if current_interface == "eth0" { eth0 } else { lo };

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
                current_interface,
                format_bytes(rx),
                format_bytes(tx),
                total_mib,
                session_mib
            ))
            .style(Style::default().fg(Color::White))
            .block(Block::default().title(" Network Usage | Switch: b/n ").title_style(Style::default().fg(Color::White)).borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)) );
            net_area = layout[3];
            frame.render_widget(net_text, layout[3]);

            visible_rows = layout[4].height.saturating_sub(3) as usize;

            let mut rows: Vec<Row> = processes
                .iter()
                .skip(scroll_offset)
                .take(visible_rows)
                .enumerate()
                .map(|(i, proc)| {
                    let actual_index = scroll_offset + i;
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
                    .style(Style::default().fg(if actual_index == selected_process {
                        Color::LightCyan
                    } else {
                        color
                    }))
                })
                .collect();

            if show_info {
                if let Some(proc) = processes.get(selected_process) {
                    let insert_index = selected_process.saturating_sub(scroll_offset);
                    if insert_index < rows.len() {                  
                        let args = proc.cmd()
                            .iter()
                            .map(|s| s.to_string_lossy())
                            .collect::<Vec<_>>()
                            .join(" ");

                        let thread_count = proc.tasks().map_or(0, |tasks| tasks.len());
                        rows.insert(insert_index + 1, Row::new(vec![
                            format!("Args: {:?}", args),
                            format!("Threads: {}", thread_count),
                            format!("Core: {}", proc.cpu_usage() as usize % system.cpus().len()),
                            format!("Status: {:?}", proc.status()),
                        ]).style(Style::default().fg(Color::Cyan)));
                    }
                }
            }


            let header = Row::new(vec!["PID", "Name", "CPU", "Memory"])
                .style(Style::default().fg(Color::Gray));

            let table = Table::new(rows, &[
                Constraint::Percentage(50),
                Constraint::Percentage(30),
                Constraint::Length(10),
                Constraint::Length(20),
            ])
            .header(header)
            .style(Style::default().fg(Color::LightCyan))
            .block(Block::default().title(Title::from(format!(
                " Top Processes - Enter: Info | o/p: Nice | k: kill | {}",
                match sort_category {
                    SortCategory::CpuPerCore => "CPU (per Core %)",
                    SortCategory::CpuAverage => "CPU (average %)",
                    SortCategory::Memory => "Memory Usage",
                    SortCategory::Network => "Network Usage",
                }
            )))
            .title_style(Style::default().fg(Color::White))
            .borders(Borders::ALL));

            info_area = layout[4];
            frame.render_widget(table, layout[4]);

            /*let legend = Paragraph::new(
                "↑/↓: Scroll  |  PgUp/PgDn: Jump  |  Home: Top  |  ←/→: Sort  |  b/n: Net IF  |  +/-: Change Refresh | q: Quit"
            )
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().title(" Controls ").borders(Borders::ALL));
            frame.render_widget(legend, layout[5]);*/

            effects.process_effects(Duration::from_millis(16).into(), frame.buffer_mut(), area);
        })?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,

                    KeyCode::Down => {
                        if selected_process + 1 < processes.len() {
                            selected_process += 1;
                            if selected_process >= scroll_offset + visible_rows {
                                scroll_offset += 1;
                            }
                        }
                    }
                    KeyCode::Up => {
                        if selected_process > 0 {
                            selected_process -= 1;
                            if selected_process < scroll_offset {
                                scroll_offset = scroll_offset.saturating_sub(1);
                            }
                        }
                    }
                    KeyCode::PageDown => {
                        selected_process = (selected_process + visible_rows).min(processes.len().saturating_sub(1));
                        scroll_offset = (scroll_offset + visible_rows).min(processes.len().saturating_sub(visible_rows));
                    }
                    KeyCode::PageUp => {
                        selected_process = selected_process.saturating_sub(visible_rows);
                        scroll_offset = scroll_offset.saturating_sub(visible_rows);
                    }
                    KeyCode::Home => {
                        selected_process = 0;
                        scroll_offset = 0;
                    }
                    KeyCode::Left => {
                        sort_category = match sort_category {
                            SortCategory::CpuPerCore => SortCategory::Network,
                            SortCategory::CpuAverage => SortCategory::CpuPerCore,
                            SortCategory::Memory => SortCategory::CpuAverage,
                            SortCategory::Network => SortCategory::Memory,
                        };
                    }
                    KeyCode::Right => {
                        sort_category = match sort_category {
                            SortCategory::CpuPerCore => SortCategory::CpuAverage,
                            SortCategory::CpuAverage => SortCategory::Memory,
                            SortCategory::Memory => SortCategory::Network,
                            SortCategory::Network => SortCategory::CpuPerCore,
                        };
                    }

                    KeyCode::Char('b') => {
                        let color = Color::from_u32(0x1E1E1E);
                        let timer = (200, Interpolation::QuintInOut);
                         effects.add_effect(
                            fx::sweep_in(
                                Motion::LeftToRight,
                                20,
                                10,
                                color,
                                timer,
                            ).with_area(net_area)
                        );
                        if Instant::now() >= switch_interface_at && current_interface != "lo" {
                            current_interface = "lo";
                        }
                    }
                    KeyCode::Char('n') => {
                        let color = Color::from_u32(0x1E1E1E);
                        let timer = (200, Interpolation::QuintInOut);
                         effects.add_effect(
                            fx::sweep_in(
                                Motion::LeftToRight,
                                20,
                                10,
                                color,
                                timer,
                            ).with_area(net_area)                    
                        );
                        
                        if Instant::now() >= switch_interface_at && current_interface != "eth0" {
                            current_interface = "eth0";
                        }
                    }
                    
                    KeyCode::Enter => {
                        let color = Color::from_u32(0x1E1E1E);
                        let timer = (200, Interpolation::QuintInOut);

                        if show_info {
                            effects.add_effect(
                                fx::sweep_in(
                                    Motion::LeftToRight,
                                    20,
                                    10,
                                    color,
                                    timer,
                                ).with_area(info_area)
                            );
                        } else {
                            effects.add_effect(
                                fx::sweep_in(
                                    Motion::RightToLeft,
                                    20,
                                    10,
                                    color,
                                    timer,
                                ).with_area(info_area)
                            );
                        }

                        show_info = !show_info;
                    }

                    // why would this not be a mutable? won't system processes change over time?
                    KeyCode::Char('o') => {
                        let mut processes: Vec<_> = system.processes().values().collect();
                        if let Some(proc) = processes.get(selected_process) {
                            let _ = std::process::Command::new("renice")
                                .arg("-n")
                                .arg("5")
                                .arg("-p")
                                .arg(proc.pid().to_string())
                                .status();
                        }
                    }
                    KeyCode::Char('p') => {
                        let mut processes: Vec<_> = system.processes().values().collect();
                        if let Some(proc) = processes.get(selected_process) {
                            let _ = std::process::Command::new("renice")
                                .arg("-n")
                                .arg("-5")
                                .arg("-p")
                                .arg(proc.pid().to_string())
                                .status();
                        }
                    }
                     // effects.add_effect(fx::dissolve((100, tachyonfx::Interpolation::Linear)));
                    KeyCode::Char('u') => {
                        let color = Color::from_u32(0x1E1E1E);
                        let timer = (200, Interpolation::QuintInOut);
                         effects.add_effect(
                            fx::sweep_in(
                                Motion::LeftToRight,
                                20,
                                10,
                                color,
                                timer,
                            ).with_area(disk_area)
                        );
                        if current_disk_index > 0 {
                            current_disk_index -= 1;
                        }
                    }
                    KeyCode::Char('i') => {
                        let color = Color::from_u32(0x1E1E1E);
                        let timer = (200, Interpolation::QuintInOut);
                         effects.add_effect(
                            fx::sweep_in(
                                Motion::LeftToRight,
                                20,
                                10,
                                color,
                                timer,
                            ).with_area(disk_area)
                        );
                        if current_disk_index + 1 < disks.list().len() {
                            current_disk_index += 1;
                        }
                    }
                    KeyCode::Char('k') => {
                        let mut processes: Vec<_> = system.processes().values().collect();
                        if let Some(proc) = processes.get(selected_process) {
                            let pid = proc.pid().as_u32() as i32;

                            effects.add_effect(fx::dissolve((100, tachyonfx::Interpolation::QuintInOut)).with_area(info_area));

                            // Kill the process using libc
                            unsafe {
                                kill(pid, SIGKILL);
                            }
                            effects.add_effect(fx::coalesce((100, tachyonfx::Interpolation::QuintInOut)).with_area(info_area));

                        }
                    }
                    KeyCode::Char('+') => {
                        let new_ms = (refresh_interval.as_millis() + 100).min(10000);
                        refresh_interval = Duration::from_millis(new_ms as u64);
                    }
                    KeyCode::Char('-') => {
                        let new_ms = refresh_interval.as_millis().saturating_sub(100).max(100);
                        refresh_interval = Duration::from_millis(new_ms as u64);
                    }

                _ => {}
                }
            }
        }

        std::thread::sleep(Duration::from_millis(16));
    }

    ratatui::restore();
    Ok(())
}