use std::{collections::VecDeque, io, time::{Duration, Instant}};
use ratatui::{prelude::*, widgets::*};
use crossterm::event::{self, Event, KeyCode};
use tachyonfx::{fx, EffectManager};
use sysinfo::{System, RefreshKind, Networks};

const HISTORY_LEN: usize = 50;
const REFRESH_INTERVAL: Duration = Duration::from_millis(2000);

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut effects: EffectManager<()> = EffectManager::default();

    // Shader effects
    effects.add_effect(fx::coalesce((500, tachyonfx::Interpolation::SineInOut)));

    

    let refresh = RefreshKind::everything();
    let mut system = System::new_with_specifics(refresh);

    let mut cpu_history: Vec<VecDeque<f32>> = vec![];
    let mut last_refresh = Instant::now();
    let mut selected_process = 0;

    loop {
        let now = Instant::now();

        // Refresh system info every 2000ms
        if now.duration_since(last_refresh) >= REFRESH_INTERVAL {
            system.refresh_all();
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

        let avg_cpu: f32 = system.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / system.cpus().len() as f32;
        let networks = Networks::new_with_refreshed_list();
        let eth0 = networks.get("eth0");
        let lo = networks.get("lo");

        terminal.draw(|frame| {
            let area = frame.area();
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(system.cpus().len() as u16 + 2),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(3),
                ])
                .split(area);

            // CPU section
            let core_lines: Vec<ListItem> = system
                .cpus()
                .iter()
                .enumerate()
                .map(|(i, cpu)| {
                    let usage = cpu.cpu_usage();
                    let color = if usage > 80.0 {
                        Color::Red
                    } else if usage > 50.0 {
                        Color::Yellow
                    } else {
                        Color::Green
                    };
                    ListItem::new(format!("Core {:>2}: {:>5.2}%", i, usage))
                        .style(Style::default().fg(color))
                })
                .collect();

            let cpu_list = List::new(core_lines)
                .block(Block::default().title(" CPU Usage ").borders(Borders::ALL));
            frame.render_widget(cpu_list, layout[0]);
            let avg_color = if avg_cpu > 80.0 {
                Color::Red
            } else if avg_cpu > 50.0 {
                Color::Yellow
            } else {
                Color::Green
            };
            let avg_text = Paragraph::new(format!("Average CPU Usage: {:.2}%", avg_cpu))
                .style(Style::default().fg(avg_color))
                .block(Block::default().title(" CPU Average ").borders(Borders::ALL));
            frame.render_widget(avg_text, layout[1]);

            // CPU average
            

            // Memory bar
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
                .block(Block::default().title(" Memory Usage ").borders(Borders::ALL))
                .gauge_style(Style::default().fg(mem_color).bg(Color::Black))
                .ratio(ratio)
                .label(format!("{:.2} / {:.2} GB", used / 1024.0, total / 1024.0));
            frame.render_widget(gauge, layout[2]);

            // Network section
            let net_text = Paragraph::new(format!(
                "eth0: RX {:>10} B | TX {:>10} B\nlo:   RX {:>10} B | TX {:>10} B",
                eth0.map(|n| n.received()).unwrap_or(0),
                eth0.map(|n| n.transmitted()).unwrap_or(0),
                lo.map(|n| n.received()).unwrap_or(0),
                lo.map(|n| n.transmitted()).unwrap_or(0),
            ))
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().title(" Network Usage ").borders(Borders::ALL));
            frame.render_widget(net_text, layout[3]);

            // Process list
            let num_cores = system.cpus().len() as f32;
            let mut processes: Vec<_> = system.processes().values().collect();
            processes.sort_by(|a, b| {
                let a_usage = a.cpu_usage() / num_cores;
                let b_usage = b.cpu_usage() / num_cores;
                b_usage.partial_cmp(&a_usage).unwrap()
            });

            let rows: Vec<Row> = processes
                .iter()
                .skip(selected_process)
                .take(20)
                .enumerate()
                .map(|(i, proc)| {
                    let name = proc.name();
                    let name_str = name.to_string_lossy().to_string();
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
                    .style(Style::default().fg(if i == 0 { Color::Yellow } else { color }))
                })
                .collect();


            let header = Row::new(vec!["PID", "Name", "CPU", "Memory"])
                .style(Style::default().fg(Color::LightBlue));

            let table = Table::new(rows, &[
                Constraint::Length(8),
                Constraint::Percentage(50),
                Constraint::Length(10),
                Constraint::Length(12),
            ])
            .header(header)
            .block(Block::default().title(" Top Processes ").borders(Borders::ALL));
            frame.render_widget(table, layout[4]);

            // Legend
            let legend = Paragraph::new(
                "↑/↓: Scroll  |  PgUp/PgDn: Jump  |  Home: Top  |  q: Quit"
            )
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().title(" Controls ").borders(Borders::ALL));
            frame.render_widget(legend, layout[5]);

            // Animate effects every frame
            effects.process_effects(Duration::from_millis(16).into(), frame.buffer_mut(), area);
        })?;

        // Keyboard controls
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
        KeyCode::Left => {
            // Navigate to previous category
        },
        KeyCode::Right => {
            // Navigate to next category
        },
        KeyCode::Char('b') => {
            // Switch to 'lo' interface
        },
        KeyCode::Char('n') => {
            // Switch to 'eth0' interface
        },
    
                    KeyCode::Down => selected_process += 1,
                    KeyCode::Up => selected_process = selected_process.saturating_sub(1),
                    KeyCode::PageDown => selected_process += 5,
                    KeyCode::PageUp => selected_process = selected_process.saturating_sub(5),
                    KeyCode::Home => selected_process = 0,
                    _ => {}
                }
            }
        }

        std::thread::sleep(Duration::from_millis(16));
    }

    ratatui::restore();
    Ok(())
}
