# r-top

**r-top** is a terminal-based, Rust-powered system resource viewer for UNIX-like systems. Inspired by tools like `bashtop` and `htop`, `b-top` provides a visually impressive process monitoring and management tool with shader-like implementations by using TachyonFX.

---

## Features

- **Per-Core CPU Usage**: Real-time usage display for each CPU core with color-coded indicators.
- **CPU Usage Graph**: Sparkline graph showing historical average CPU usage.
- **Memory Monitoring**: Visual gauge showing used vs. total memory in GB.
- **Network Stats**: Displays RX/TX bytes for interfaces like `eth0` and `lo`.
- **Process Table**: Sortable list of top processes by CPU, memory, or network usage (WIP).
- **Smooth Animations**: Powered by `tachyonfx` for subtle UI transitions.
- **Keyboard Navigation**: Scroll, jump, sort, and switch interfaces with intuitive keybindings.
- **Disk Usage**: Track disk usage with a visual gauge and switch between disks for active monitoring.
- **Tree View**: See parent and child processes for each running/sleeping process.
- **Search Filter**: Filter and find specific processes in the process table
- **Daemon Supervisor**: Create your own service profiles to run a `b-daemon` in either integrated or active modes


---

## Built With

- Rust
- ratatui – Terminal UI rendering
- sysinfo – System information
- crossterm – Terminal input handling
- tachyonfx – Animation effects

---

## Controls

| Key             | Action                          |
|----------------|----------------------------------|
| `↑ / ↓`        | Scroll through process list      |
| `PgUp / PgDn`  | Jump up/down in process list     |
| `Home`         | Jump to top of process list      |
| `← / →`        | Change sorting category          |
| `b / n`        | Switch between `lo` and `eth0`   |
| `q`            | Quit the application             |
| `ENTER`        | View more process info           |
| `k`            | Kill selected process            |
| `u / i`        | Switch between disks             |
| `tab`          | Open tree view during `show_info`|
| `/`            | Use the search filter            |
---

## Installation

### Prerequisites

- Rust toolchain (`cargo`, `rustc`)
- A UNIX-like OS (Linux, macOS)

### Build and Run

```bash
git clone https://github.com/asian-mario/r-top.git
cd r-top
cargo run --release
```

---
### Downloading r-top
```bash
wget https://github.com/asian-mario/r-top/releases/download/[VERSION]/r-top-linux-x86_64.tar.gz
tar -xzf r-top-linux-x86_64.tar.gz
sudo mv r-top /usr/local/bin/

```
---
### Service Configuaration
```
$$\                     $$\                                                       
$$ |                    $$ |                                                      
$$$$$$$\           $$$$$$$ | $$$$$$\   $$$$$$\  $$$$$$\$$$$\   $$$$$$\  $$$$$$$\  
$$  __$$\ $$$$$$\ $$  __$$ | \____$$\ $$  __$$\ $$  _$$  _$$\ $$  __$$\ $$  __$$\ 
$$ |  $$ |\______|$$ /  $$ | $$$$$$$ |$$$$$$$$ |$$ / $$ / $$ |$$ /  $$ |$$ |  $$ |
$$ |  $$ |        $$ |  $$ |$$  __$$ |$$   ____|$$ | $$ | $$ |$$ |  $$ |$$ |  $$ |
$$$$$$$  |        \$$$$$$$ |\$$$$$$$ |\$$$$$$$\ $$ | $$ | $$ |\$$$$$$  |$$ |  $$ |
\_______/          \_______| \_______| \_______|\__| \__| \__| \______/ \__|  \__|
                                                                                  
```                                                                                  
                                                                                  

The r-top daemon (or `b-daemon`) active and integrated modes use a TOML configuration file to define the services you want to monitor and manage, the configuration file is automatically created at `~/.config/r-top/services.toml` when you *first* run `r-top -d`. The *integrated* mode is still highly experimental, the daemon may run even when r-top is shut down. Please use it at your own risk.

#### Configuration File Location
- Linux/macOS: `~/.config/r-top/services.toml`
- Custom location: Use `r-top -d -c /path/to/your/config.toml`

#### Basic Service Structure
Each service is defined in the `[[services]]` array with the following fields
```
[[services]]
name = "my-service"                    # Unique service name
command = "/usr/bin/python3"           # Executable path
args = ["app.py", "--port", "8080"]    # Command arguments
working_dir = "/home/user/myapp"       # Optional working directory
restart_policy = "always"             # Restart behavior: always, on_failure, never
max_restarts = 5                       # Maximum restart attempts
restart_delay_secs = 10                # Seconds to wait before restart
```
#### Restart Policies
b-daemon has multiple restart policies depending on how you want to treat the service you are monitoring
- `always`: Restart the service whenever it stops
- `on_failure`: Only restart if the service exits with a non-zero status code
- `never`: Never automatically restart the service

#### Configuration Examples
If you are struggling on how to set up your services.toml (or other custom configuration files) look at the examples within `./example-services/`

#### Common Health Checks
```
# HTTP service health check
curl -f http://localhost:8080/health

# Process existence check
pgrep -f "my-service"

# File freshness check (modified within last 5 minutes)
find /path/to/output -mmin -5 | grep -q output

# Database connection check
pg_isready -h localhost -p 5432

# Docker container health
docker exec container-name health-command

# Log file activity (new entries in last 2 minutes)
find /var/log/myapp.log -mmin -2 | grep -q myapp.log
```

---

## License

MIT License

---

## Acknowledgements

Thanks to `ratatui` and `TachyonFX` repositories for maintaining their projects for for the UI animations.

---