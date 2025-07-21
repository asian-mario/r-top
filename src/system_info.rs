use std::collections::VecDeque;
use sysinfo::{System, Process};
use crate::types::SortCategory;
use crate::constants::HISTORY_LEN;

pub fn update_cpu_history(cpu_history: &mut Vec<VecDeque<f32>>, system: &System) {
    for (i, cpu) in system.cpus().iter().enumerate() {
        if let Some(buf) = cpu_history.get_mut(i) {
            if buf.len() >= HISTORY_LEN {
                buf.pop_front();
            }
            buf.push_back(cpu.cpu_usage());
        }
    }
}

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

pub fn calculate_avg_cpu_history(cpu_history: &Vec<VecDeque<f32>>) -> Vec<u64> {
    (0..HISTORY_LEN)
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