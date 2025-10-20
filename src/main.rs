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
mod theme;
mod daemon;

use clap::{Arg, Command as ClapCommand, ArgAction};
use std::path::PathBuf;

use constants::*;
use ui::*;
use event_handler::*;
use system_info::*;
use app_state::*;
use utils::CircularBuffer;
use daemon::{run_daemon_mode, DaemonSupervisor};
use ctrlc::*;

use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::thread::{self, JoinHandle};

/*
    please refrain from taking any comments that dont have proper punctuation as serious
    i shitpost a lot because its lonely
*/

/*
    MEMORY OPT. LIST:
    - CPU PROC CACHE
    - CPU HISTORY CIRC BUFFER
*/

fn setup_signal_handler() -> Arc<AtomicBool> {
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown_signal.clone();

    ctrlc::set_handler(move || {
        shutdown_clone.store(true, Ordering::Relaxed);
    }).expect("Error setting signal handler");

    shutdown_signal
}
fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    eprintln!("Raw args: {:?}", args);

    let matches = ClapCommand::new("r-top")
        .version("0.2.5")
        .about("r-top is a tui system monitor written in Rust with an extended daemon supervisor.")
        .arg(
            Arg::new("daemon")
                .short('d')
                .long("daemon")
                .help("Run r-top in daemon mode, which will run the daemon supervisor in the background.")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("integrate")
                .short('i')
                .long("integrate")
                .help("Run daemon in the background while showing the process monitor (requires -d). THIS FEATURE IS EXPERIMENTAL. IT MAY NOT SHUTDOWN WITH THE PROGRAM.")
                .action(ArgAction::SetTrue)
                .required(false)
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Path to the configuration file.")
                .action(clap::ArgAction::Set)
                .value_parser(clap::value_parser!(PathBuf))
        )
        .get_matches();

    let daemon_mode = matches.get_flag("daemon");
    let integrate_mode = matches.get_flag("integrate");
    let config_path = matches.get_one::<String>("config").map(PathBuf::from);

    if let Some(ref path) = config_path {
        println!("Config Path: {:?}", path);
    }

    if integrate_mode && !daemon_mode {
        println!("Error: --integrate (-i) requires --daemon (-d)");
        println!("Usage: r-top -d -i");
        std::process::exit(1);
    }

    if daemon_mode && integrate_mode {
        println!("=== STARTING INTEGRATED MODE ===");
        return run_integrated_mode(config_path);
    } else if daemon_mode {
        println!("=== STARTING DAEMON MODE ===");
        return run_daemon_mode_wrapper(config_path);
    } else {
        println!("=== STARTING PROCMON MODE ===");
        return run_process_monitor();
    }
}

fn run_daemon_mode_wrapper(config_path: Option<PathBuf>) -> io::Result<()> {
    match run_daemon_mode(config_path) {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Error running daemon mode: {}", e);
            Err(io::Error::new(io::ErrorKind::Other, e.to_string()))
        }
    }
}

fn run_integrated_mode(config_path: Option<PathBuf>) -> io::Result<()> {
    println!("Starting r-top and b-daemon in integration mode.");

    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let shutdown_signal_clone = shutdown_signal.clone();

    let daemon_config = config_path.clone();
    let daemon_handle: JoinHandle<Result<(), Box<dyn std::error::Error + Send>>> = thread::spawn(move || {
        run_daemon_mode_silent(daemon_config, shutdown_signal_clone)
    });

    thread::sleep(Duration::from_millis(500));

    let result = run_process_monitor();

    shutdown_signal.store(true, Ordering::Relaxed);

    match daemon_handle.join() {
        Ok(daemon_result) => {
            if let Err(e) = daemon_result {
                eprintln!("Daemon thread error: {}", e);
            }
        }
        Err(_) => {
            eprintln!("Failed to join daemon thread.")
        }
    }

    result
}

fn run_daemon_mode_silent(config_path: Option<PathBuf>, shutdown_signal: Arc<AtomicBool>) -> Result<(), Box<dyn std::error::Error + Send>> {
    use daemon::DaemonSupervisor;

    let mut supervisor = DaemonSupervisor::new(config_path);
    supervisor.load_config().map_err(|e| -> Box<dyn std::error::Error + Send> {
        Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    })?;

    let service_names: Vec<String> = supervisor.services.keys().cloned().collect();
    for name in &service_names {
        let _ = supervisor.start_service(&name);
    }

    while !shutdown_signal.load(Ordering::Relaxed) {
        supervisor.check_services_silent();

        for _ in 0..50 {
            if shutdown_signal.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    for name in &service_names{
        if let Err(e) = supervisor.stop_service_silent(name) {
            eprintln!("Error stopping service '{}': {}", name, e)
        }
    }

    Ok(())
}

fn run_process_monitor() -> io::Result<()> {
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
        let processes = sort_processes_cached(&system, &app_state.sort_category, &mut app_state.process_cache, &app_state.search_active);
        
        terminal.draw(|frame| {
            app_state.update_terminal_area(frame.size());  //-> should i seperate this from render_ui?
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