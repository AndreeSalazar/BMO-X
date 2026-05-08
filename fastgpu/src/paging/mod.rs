

use crate::handles::HandleTable;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ResidencyState {
    Evicted = 1,
    Resident = 4,
    Active = 5,
}

/// Represents a Paging Buffer / Memory Range (EventID 301, 302, 303)
/// Tracks the residency of memory blocks on the GPU.
#[derive(Copy, Clone)]
pub struct ResidencyRange {
    pub h_range: u32,
    pub start_address: u64,
    pub end_address: u64,
    pub state: ResidencyState,
    pub eprocess_ptr: u64, // DxgProcess ownership
}

pub struct PagingState {
    pub ranges: HandleTable<ResidencyRange>,
}

impl PagingState {
    pub const fn new() -> Self {
        Self {
            ranges: HandleTable::new(),
        }
    }
}

pub static mut GPU_PAGING: PagingState = PagingState::new();

/// Tracks a new residency range (EventID 301)
pub unsafe fn track_residency_range(start: u64, end: u64, process: u64) -> Option<u32> {
    let range = ResidencyRange {
        h_range: 0,
        start_address: start,
        end_address: end,
        state: ResidencyState::Resident, // Usually starts as resident
        eprocess_ptr: process,
    };
    
    GPU_PAGING.ranges.allocate(range)
}

/// Modifies the state of a residency block (EventID 302)
/// e.g. moving from Resident (4) to Active (5) when commands are executing.
pub unsafe fn update_residency_state(h_range: u32, new_state: u32) -> bool {
    if let Some(mut range) = GPU_PAGING.ranges.get(h_range) {
        let state = match new_state {
            1 => ResidencyState::Evicted,
            4 => ResidencyState::Resident,
            5 => ResidencyState::Active,
            _ => return false,
        };
        range.state = state;
        return true;
    }
    false
}
