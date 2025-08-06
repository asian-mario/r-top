use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use sysinfo::{System, Process, Pid};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: Option<String>, 
    pub env_vars: HashMap<String, String>,
    pub restart_policy: String, 
    pub max_restarts: u32,
    pub restart_delay_secs: u64, 
    pub health_check: Option<HealthCheckConfig>, 
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub command: String,
    pub interval_secs: u64, 
    pub timeout_secs: u64, 
    pub retries: u32,
}

#[derive(Debug, Clone)]
pub enum RestartPolicy {
    Always,
    OnFailure,
    Never,  
}

impl RestartPolicy {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "always" => RestartPolicy::Always,
            "onfailure" | "on_failure" => RestartPolicy::OnFailure,
            "never" => RestartPolicy::Never,
            _ => RestartPolicy::OnFailure, // default
        }
    }
}

#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub command: String,
    pub interval: Duration,
    pub timeout: Duration,
    pub retries: u32,
}

#[derive(Debug, Clone)]
pub struct ServiceState {
    pub config: ServiceConfig,
    pub pid: Option<u32>,
    pub status: ServiceStatus,
    pub restart_count: u32,
    pub last_restart: Option<Instant>,
    pub last_health_check: Option<Instant>,
    pub health_failures: u32,
    pub restart_policy: RestartPolicy,
    pub health_check: Option<HealthCheck>,
}

#[derive(Serialize, Deserialize)]
struct ConfigWrapper {
    services: Vec<ServiceConfig>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServiceStatus {
    Running,
    Stopped,
    Failed,
    Starting,
    Stopping,
    Unhealthy,
}

pub struct DaemonSupervisor {
    services: HashMap<String, ServiceState>,
    config_path: PathBuf,
    system: System,
}

impl DaemonSupervisor {
    pub fn new(config_path: Option<PathBuf>) -> Self {
        let config_path = config_path.unwrap_or_else(|| {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("/etc"))
                .join("b-top")
                .join("services.toml")
        });

        Self {
            services: HashMap::new(),
            config_path,
            system: System::new_all(),
        }
    }

    pub fn load_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.config_path.exists() {
            if let Some(parent) = self.config_path.parent() {
                fs::create_dir_all(parent)?;
            }
            self.create_default_config()?;
            return Ok(());
        }

        let config_content = fs::read_to_string(&self.config_path)?;
        
        // parse as a wrapper struct to handle the array properly     
        let wrapper: ConfigWrapper = toml::from_str(&config_content)?;
        let configs = wrapper.services;

        for config in configs {
            // convert from TOML-friendly format to internal format
            let restart_policy = RestartPolicy::from_str(&config.restart_policy);
            let health_check = config.health_check.as_ref().map(|hc| HealthCheck {
                command: hc.command.clone(),
                interval: Duration::from_secs(hc.interval_secs),
                timeout: Duration::from_secs(hc.timeout_secs),
                retries: hc.retries,
            });

            let state = ServiceState {
                config: config.clone(),
                pid: None,
                status: ServiceStatus::Stopped,
                restart_count: 0,
                last_restart: None,
                last_health_check: None,
                health_failures: 0,
                restart_policy,
                health_check,
            };
            self.services.insert(config.name.clone(), state);
        }

        Ok(())
    }

    fn create_default_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let default_services = vec![
            ServiceConfig {
                name: "log-monitor".to_string(),
                command: "/usr/bin/tail".to_string(), 
                args: vec!["-f".to_string(), "/var/log/syslog".to_string()],
                working_dir: Some("/tmp".to_string()),
                env_vars: HashMap::new(),
                restart_policy: "always".to_string(),
                max_restarts: 3,
                restart_delay_secs: 5,
                health_check: Some(HealthCheckConfig {
                    command: "pgrep -f 'tail.*syslog' > /dev/null".to_string(), 
                    interval_secs: 60,
                    timeout_secs: 5,
                    retries: 1,
                }),
            },
            ServiceConfig {
                name: "stats-collector".to_string(),
                command: "/bin/bash".to_string(),
                args: vec!["-c".to_string(), "while true; do echo \"$(date): $(uptime)\" >> /tmp/system-stats.log; sleep 30; done".to_string()],
                working_dir: Some("/tmp".to_string()),
                env_vars: HashMap::new(),
                restart_policy: "always".to_string(),
                max_restarts: 10,
                restart_delay_secs: 2,
                health_check: Some(HealthCheckConfig {
                    command: "test -f /tmp/system-stats.log && find /tmp/system-stats.log -mmin -2 | grep -q system-stats.log".to_string(),
                    interval_secs: 45,
                    timeout_secs: 5,
                    retries: 2,
                }),
            },
            ServiceConfig {
                name: "network-monitor".to_string(),
                command: "/bin/bash".to_string(),
                args: vec!["-c".to_string(), "while true; do ping -c 1 8.8.8.8 >/dev/null && echo \"$(date): Network OK\" >> /tmp/network.log || echo \"$(date): Network FAIL\" >> /tmp/network.log; sleep 10; done".to_string()],
                working_dir: Some("/tmp".to_string()),
                env_vars: HashMap::new(),
                restart_policy: "on_failure".to_string(),
                max_restarts: 5,
                restart_delay_secs: 10,
                health_check: Some(HealthCheckConfig {
                    command: "test -f /tmp/network.log && find /tmp/network.log -mmin -1 | grep -q network.log".to_string(), 
                    interval_secs: 30,
                    timeout_secs: 5,
                    retries: 1,
                }),
            },
        ];


        let wrapper = ConfigWrapper {
            services: default_services,
        };

        let toml_content = toml::to_string_pretty(&wrapper)?;
        fs::write(&self.config_path, toml_content)?;
        
        println!("Created default service configuration at {:?}", self.config_path);
        println!("Default services include: log-monitor, stats-collector, network-monitor");
        println!("Which monitors the following and stores results in: /var/log/syslog, /tmp/system-stats.log, and /tmp/network.log.");
        println!("");
        println!("This example configuration is designed to work out of the box for Ubuntu systems.");
        println!("Please edit the config file to add your own services or modify existing ones.");

        Ok(())
    }

    pub fn start_service(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let service = self.services.get_mut(name)
            .ok_or(format!("Service '{}' not found", name))?;

        if service.status == ServiceStatus::Running {
            return Ok(());
        } else {
            service.status = ServiceStatus::Starting;

            let mut cmd = Command::new(&service.config.command);
            cmd.args(&service.config.args);

            if let Some(working_dir) = &service.config.working_dir {
                cmd.current_dir(working_dir);
            }

            for (key, value) in &service.config.env_vars {
                cmd.env(key, value);
            }

            let child = cmd.spawn()?;
            service.pid = Some(child.id());
            service.status = ServiceStatus::Running;

            println!("Started service '{}', PID: {}", name, service.pid.unwrap());
            Ok(())
        }
    }

    pub fn stop_service(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let service = self.services.get_mut(name)
            .ok_or(format!("Service '{}' not found", name))?;

        if let Some(pid) = service.pid {
            service.status = ServiceStatus::Stopping;

            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }

            std::thread::sleep(Duration::from_secs(5)); // wait for graceful shutdown

            self.system.refresh_all();
            if self.system.process(sysinfo::Pid::from(pid as usize)).is_some() {
                unsafe {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
            }

            service.pid = None;
            service.status = ServiceStatus::Stopped;
            println!("Stopped service '{}'", name);

        }

        Ok(())
    }

    pub fn check_services(&mut self) {
        self.system.refresh_all();
        let now = Instant::now();
        let mut to_restart: Vec<String> = Vec::new();
        let mut health_checks_to_run: Vec<(String, HealthCheck)> = Vec::new();

        let mut restart_candidates: Vec<String> = Vec::new();

        for (name, service) in self.services.iter_mut() {
            if let Some(pid) = service.pid {
                if self.system.process(sysinfo::Pid::from(pid as usize)).is_none() {
                    println!("Service '{}' (PID: {}) is not running", name, pid);
                    service.pid = None;
                    service.status = ServiceStatus::Failed;

                    restart_candidates.push(name.clone());
                }
            }

            if let Some(health_check) = &service.health_check {
                if service.status == ServiceStatus::Running {
                    let should_check = service.last_health_check
                        .map_or(true, |last| now.duration_since(last) >= health_check.interval);

                    if should_check {
                        health_checks_to_run.push((name.clone(), health_check.clone()));
                        service.last_health_check = Some(now);
                    }
                }
            }
        }

        // check restart policy outside the mutable borrow
        for name in restart_candidates {
            if let Some(service) = self.services.get(&name) {
                if self.should_restart(service) {
                    println!("Restarting service '{}'", name);
                    to_restart.push(name.clone());
                }
            }
        }

        for (name, health_check) in health_checks_to_run {
            self.perform_health_check(&name, &health_check);
        }

        for name in to_restart {
            if let Err(e) = self.restart_service(&name) {
                eprintln!("Failed to restart service '{}': {}", name, e);
            }
        }
    }

    fn should_restart(&self, service: &ServiceState) -> bool {
        match service.restart_policy {
            RestartPolicy::Always => true,
            RestartPolicy::OnFailure => service.status == ServiceStatus::Failed && service.restart_count < service.config.max_restarts,
            RestartPolicy::Never => false,
        }
    }

    fn restart_service(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let service = self.services.get_mut(name).unwrap();

        if let Some(last_restart) = service.last_restart {
            let restart_delay = Duration::from_secs(service.config.restart_delay_secs);
            let elapsed = Instant::now().duration_since(last_restart);
            if elapsed < restart_delay {
                std::thread::sleep(restart_delay - elapsed);
            }
        }

        service.restart_count += 1;
        service.last_restart = Some(Instant::now());

        self.start_service(name)
    }

    fn perform_health_check(&mut self, name: &str, health_check: &HealthCheck) {
        let output = Command::new("sh")
            .arg("-c")
            .arg(&health_check.command)
            .output();

        let service = self.services.get_mut(name).unwrap();

        match output {
            Ok(output) if output.status.success() => {
                service.health_failures = 0;
                if service.status == ServiceStatus::Unhealthy {
                    service.status = ServiceStatus::Running;
                    println!("Service '{}' is now healthy", name);
                }
            }
            _ => {
                service.health_failures += 1;
                if service.health_failures >= health_check.retries {
                    service.status = ServiceStatus::Unhealthy;
                    println!("Service '{}' is unhealthy, failed health check.", name);
                }
            }
        }
    }

    pub fn list_services(&self) -> Vec<(&String, &ServiceState)> {
        self.services.iter().collect()
    }

    pub fn get_service_status(&self, name: &str) -> Option<&ServiceStatus> {
        self.services.get(name).map(|s| &s.status)
    }
}

pub fn run_daemon_mode(config_path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    print!(r"
    
    $$\                     $$\                                                       
    $$ |                    $$ |                                                      
    $$$$$$$\           $$$$$$$ | $$$$$$\   $$$$$$\  $$$$$$\$$$$\   $$$$$$\  $$$$$$$\  
    $$  __$$\ $$$$$$\ $$  __$$ | \____$$\ $$  __$$\ $$  _$$  _$$\ $$  __$$\ $$  __$$\ 
    $$ |  $$ |\______|$$ /  $$ | $$$$$$$ |$$$$$$$$ |$$ / $$ / $$ |$$ /  $$ |$$ |  $$ |
    $$ |  $$ |        $$ |  $$ |$$  __$$ |$$   ____|$$ | $$ | $$ |$$ |  $$ |$$ |  $$ |
    $$$$$$$  |        \$$$$$$$ |\$$$$$$$ |\$$$$$$$\ $$ | $$ | $$ |\$$$$$$  |$$ |  $$ |
    \_______/          \_______| \_______| \_______|\__| \__| \__| \______/ \__|  \__|
                                                                                    
                                                                                    
                                                                                    
    ");

    println!("Starting daemon mode...");
    let mut supervisor = DaemonSupervisor::new(config_path);
    supervisor.load_config()?;

    let service_names: Vec<String> = supervisor.services.keys().cloned().collect();
    for name in service_names {
        if let Err(e) = supervisor.start_service(&name) {
            eprintln!("Failed to start service '{}': {}", name, e);
        }
    }

    println!("Daemon supervisor started. Monitoring {} services.", supervisor.services.len());
        println!("\n=== Service Log Locations ===");
    for (name, service) in supervisor.services.iter() {
        match name.as_str() {
            // for default services only
            "log-monitor" => {
                println!("  {} - Monitors: /var/log/syslog (output to stdout)", name);
            },
            "stats-collector" => {
                println!("  {} - Logs to: /tmp/system-stats.log", name);
            },
            "network-monitor" => {
                println!("  {} - Logs to: /tmp/network.log", name);
            },
            _ => {
                // for custom services, try to extract log paths from their commands
                let command_str = format!("{} {}", service.config.command, service.config.args.join(" "));
                if command_str.contains(">>") {
                    if let Some(log_path) = command_str.split(">>").nth(1) {
                        let log_path = log_path.trim().split_whitespace().next().unwrap_or("unknown");
                        println!("  {} - Logs to: {}", name, log_path);
                    } else {
                        println!("  {} - Custom service (check config for details)", name);
                    }
                } else {
                    println!("  {} - Custom service (check config for details)", name);
                }
            }
        }
    }
    println!("Press Ctrl+C to stop the daemon.");

    loop {
        supervisor.check_services();
        std::thread::sleep(Duration::from_secs(5));
    }
}