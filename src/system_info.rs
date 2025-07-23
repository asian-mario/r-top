use sysinfo::{System, Process, Pid};
use crate::types::SortCategory;
use crate::constants::HISTORY_LEN;
use crate::utils::CircularBuffer;
use std::collections::HashMap;
use std::collections::HashSet;
use crate::app_state::*;

// Add this struct to cache sorted processes
pub struct ProcessCache {
    cached_processes: Vec<Pid>,
    last_sort_category: SortCategory,
    cache_valid: bool,
    process_count: usize,
}

impl ProcessCache {
    pub fn new() -> Self {
        Self {
            cached_processes: Vec::new(),
            last_sort_category: SortCategory::CpuPerCore,
            cache_valid: false,
            process_count: 0,
        }
    }

    pub fn invalidate(&mut self) {
        self.cache_valid = false;
    }
}

pub fn update_cpu_history(cpu_history: &mut Vec<CircularBuffer<f32>>, system: &System) {
    for (i, cpu) in system.cpus().iter().enumerate() {
        if let Some(buffer) = cpu_history.get_mut(i) {
            buffer.push(cpu.cpu_usage());
        }
    }
}

/*
    BOOM! cpu cache now resolves the need to do a vec refresh per frame
    -> keeping the old one incase i mess this up
*/
pub fn sort_processes_cached<'a>(
    system: &'a System, 
    sort_category: &SortCategory,
    cache: &mut ProcessCache
) -> Vec<&'a Process> {
    let current_process_count = system.processes().len();
    
    // Check if we need to invalidate cache
    let needs_resort = !cache.cache_valid 
        || !matches!((cache.last_sort_category, sort_category), 
            (SortCategory::CpuPerCore, SortCategory::CpuPerCore) |
            (SortCategory::CpuAverage, SortCategory::CpuAverage) |
            (SortCategory::Memory, SortCategory::Memory) |
            (SortCategory::Network, SortCategory::Network))
        || cache.process_count != current_process_count;

    if needs_resort {
        // Full resort needed
        let num_cores = system.cpus().len() as f32;
        let mut process_pairs: Vec<(Pid, f64)> = system.processes()
            .iter()
            .map(|(pid, process)| {
                let sort_value = match sort_category {
                    SortCategory::CpuPerCore | SortCategory::CpuAverage => {
                        (process.cpu_usage() / num_cores) as f64
                    }
                    SortCategory::Memory => process.memory() as f64,
                    SortCategory::Network => 0.0, // No specific sorting for network
                };
                (*pid, sort_value)
            })
            .collect();

        // Sort by the calculated values (descending order)
        if !matches!(sort_category, SortCategory::Network) {
            process_pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        }

        // Update cache
        cache.cached_processes = process_pairs.into_iter().map(|(pid, _)| pid).collect();
        cache.last_sort_category = *sort_category;
        cache.cache_valid = true;
        cache.process_count = current_process_count;
    } else {
        // Cache is valid, but we might need to update sort values for dynamic sorting
        // For CPU and Memory, values change frequently, so we do a lightweight update
        if matches!(sort_category, SortCategory::CpuPerCore | SortCategory::CpuAverage | SortCategory::Memory) {
            let num_cores = system.cpus().len() as f32;
            
            // Create a map for quick lookup of current values
            let current_values: HashMap<Pid, f64> = system.processes()
                .iter()
                .map(|(pid, process)| {
                    let sort_value = match sort_category {
                        SortCategory::CpuPerCore | SortCategory::CpuAverage => {
                            (process.cpu_usage() / num_cores) as f64
                        }
                        SortCategory::Memory => process.memory() as f64,
                        SortCategory::Network => 0.0,
                    };
                    (*pid, sort_value)
                })
                .collect();

            // Re-sort only if there are significant changes
            let mut needs_full_resort = false;
            
            // Check if top 10 processes have changed significantly
            for (i, &pid) in cache.cached_processes.iter().take(10).enumerate() {
                if let Some(&current_value) = current_values.get(&pid) {
                    // If a top process has dropped significantly, resort
                    if i < 5 && current_value < 1.0 {
                        needs_full_resort = true;
                        break;
                    }
                } else {
                    // Process no longer exists
                    needs_full_resort = true;
                    break;
                }
            }

            if needs_full_resort {
                cache.invalidate();
                return sort_processes_cached(system, sort_category, cache);
            }
        }
    }

    // Return processes in cached order, filtering out non-existent processes
    cache.cached_processes
        .iter()
        .filter_map(|pid| system.process(*pid))
        .collect()
}

// keep the original function for backward compatibility, but mark it as deprecated
#[deprecated(note = "deprecated! use cached version")]
pub fn sort_processes<'a>(system: &'a System, sort_category: &SortCategory) -> Vec<&'a Process> {
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
        SortCategory::Network => {} // No specific sorting for network
    }

    processes
}

pub fn calculate_avg_cpu_history(cpu_history: &Vec<CircularBuffer<f32>>) -> Vec<u64> {
    if cpu_history.is_empty() {
        return vec![0; HISTORY_LEN];
    }

    let max_len = cpu_history.iter().map(|buf| buf.len()).max().unwrap_or(0);
    
    (0..max_len)
        .map(|i| {
            let sum: f32 = cpu_history
                .iter()
                .filter_map(|buffer| buffer.get(i))
                .sum();
            let count = cpu_history
                .iter()
                .filter(|buffer| buffer.get(i).is_some())
                .count();
            
            if count > 0 {
                (sum / count as f32) as u64
            } else {
                0
            }
        })
        .collect()
}

pub fn get_busiest_core_info(system: &System) -> (usize, f32, String, u32) {
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

    (busiest_core_idx, busiest_core_usage.cpu_usage(), top_process_name, top_process_pid)
}

// more tree stuff below
pub fn build_process_tree(
    system: &System,
    app_state: &mut AppState,
) -> Vec<TreeItem> {
    if app_state.tree_cache_valid {
        return app_state.tree_items.clone();
    }

    let mut tree_items = Vec::new();
    let mut parent_child_map: HashMap<Pid, Vec<Pid>> = HashMap::new();
    let mut all_processes: HashMap<Pid, &Process> = HashMap::new();

    // find parent-child relationships
    for (pid, process) in system.processes() {
        all_processes.insert(*pid, process);
        
        if let Some(parent_pid) = process.parent() {
            parent_child_map
                .entry(parent_pid)
                .or_insert_with(Vec::new)
                .push(*pid);
        }
    }

    // root process finding
    let mut root_processes: Vec<Pid> = Vec::new();
    for (pid, process) in system.processes() {
        if let Some(parent_pid) = process.parent() {
            if !all_processes.contains_key(&parent_pid) {
                root_processes.push(*pid);
            }
        } else {
            root_processes.push(*pid);
        }
    }

    root_processes.sort_by(|a, b| {
        let a_usage = all_processes.get(a).map(|p| p.cpu_usage()).unwrap_or(0.0);
        let b_usage = all_processes.get(b).map(|p| p.cpu_usage()).unwrap_or(0.0);
        b_usage.partial_cmp(&a_usage).unwrap_or(std::cmp::Ordering::Equal)
    });

    for root_pid in root_processes {
        if let Some(process) = all_processes.get(&root_pid) {
            build_tree_recursive(
                root_pid,
                None,
                0,
                &all_processes,
                &parent_child_map,
                &app_state.tree_expanded_nodes,
                &mut tree_items,
            );
        }
    }

    app_state.tree_items = tree_items.clone();
    app_state.tree_cache_valid = true;
    
    tree_items
}

fn build_tree_recursive(
    pid: Pid,
    parent_pid: Option<Pid>,
    level: usize,
    all_processes: &HashMap<Pid, &Process>,
    parent_child_map: &HashMap<Pid, Vec<Pid>>,
    expanded_nodes: &HashSet<Pid>,
    tree_items: &mut Vec<TreeItem>,
) {
    if let Some(process) = all_processes.get(&pid) {
        let children = parent_child_map.get(&pid).cloned().unwrap_or_default();
        let has_children = !children.is_empty();
        let is_expanded = expanded_nodes.contains(&pid);

        let tree_item = TreeItem {
            pid,
            name: process.name().to_string_lossy().into_owned(),
            cpu_usage: process.cpu_usage(),
            memory: process.memory(),
            level,
            is_expanded,
            has_children,
            parent_pid,
        };

        tree_items.push(tree_item);

        if is_expanded && has_children {
            let mut sorted_children = children;
            sorted_children.sort_by(|a, b| {
                let a_usage = all_processes.get(a).map(|p| p.cpu_usage()).unwrap_or(0.0);
                let b_usage = all_processes.get(b).map(|p| p.cpu_usage()).unwrap_or(0.0);
                b_usage.partial_cmp(&a_usage).unwrap_or(std::cmp::Ordering::Equal)
            });

            for child_pid in sorted_children {
                build_tree_recursive(
                    child_pid,
                    Some(pid),
                    level + 1,
                    all_processes,
                    parent_child_map,
                    expanded_nodes,
                    tree_items,
                );
            }
        }
    }
}

// Helper function to get tree statistics
pub fn get_tree_stats(tree_items: &[TreeItem]) -> (usize, usize, usize) {
    let total_processes = tree_items.len();
    let expanded_nodes = tree_items.iter().filter(|item| item.is_expanded).count();
    let max_depth = tree_items.iter().map(|item| item.level).max().unwrap_or(0);
    
    (total_processes, expanded_nodes, max_depth)
}