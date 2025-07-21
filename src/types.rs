#[derive(Debug, Clone, Copy)]
pub enum SortCategory {
    CpuPerCore,
    CpuAverage,
    Memory,
    Network,
}
/*
FURTHER:
    add disk i/o usage
    pot. add GPU usage as well but that also means I have to call GPU refreshes and add it in. lazy, but i'll see.
*/
impl SortCategory {
    pub fn previous(&self) -> Self {
        match self {
            SortCategory::CpuPerCore => SortCategory::Network,
            SortCategory::CpuAverage => SortCategory::CpuPerCore,
            SortCategory::Memory => SortCategory::CpuAverage,
            SortCategory::Network => SortCategory::Memory,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            SortCategory::CpuPerCore => SortCategory::CpuAverage,
            SortCategory::CpuAverage => SortCategory::Memory,
            SortCategory::Memory => SortCategory::Network,
            SortCategory::Network => SortCategory::CpuPerCore,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SortCategory::CpuPerCore => "CPU (per Core %)",
            SortCategory::CpuAverage => "CPU (average %)",
            SortCategory::Memory => "Memory Usage",
            SortCategory::Network => "Network Usage",
        }
    }
}