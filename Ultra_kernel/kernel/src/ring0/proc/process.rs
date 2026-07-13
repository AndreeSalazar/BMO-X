//! Process — minimal stub for the Ring 0 base.

#[derive(Debug, Clone, Copy)]
pub struct Process {
    pub pid: u32,
    pub cr3: u64,
}

pub fn get_process(_pid: u32) -> Option<&'static Process> { None }
