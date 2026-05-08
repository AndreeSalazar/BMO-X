

use crate::handles::HandleTable;

/// Represents a GPU Allocation (EventID 466, 461)
/// This maps a system memory range or a VRAM segment to a GPU Virtual Address (49-bit).
#[derive(Copy, Clone)]
pub struct DxgAllocation {
    pub h_allocation: u32,
    pub gpu_virtual_address: u64,
    pub size: u64,
    pub is_dma_buffer: bool,
    pub dxg_process_ptr: u64,
}

pub struct AllocatorState {
    pub allocations: HandleTable<DxgAllocation>,
    // GPU Virtual Address Manager (49-bit space tracker)
    pub next_gpu_va: u64,
}

impl AllocatorState {
    pub const fn new() -> Self {
        Self {
            allocations: HandleTable::new(),
            // Ampere supports 49-bit VA. We start mapping at an arbitrary safe offset.
            next_gpu_va: 0x100000000, 
        }
    }
}

pub static mut GPU_ALLOCATOR: AllocatorState = AllocatorState::new();

/// Called when Dxgkrnl queries to create an allocation (EventID 461 for DMA buffers)
pub unsafe fn create_allocation(size: u64, is_dma: bool, process_ptr: u64) -> Option<u32> {
    let gpu_va = GPU_ALLOCATOR.next_gpu_va;
    
    // Simple bump allocator for VA space
    GPU_ALLOCATOR.next_gpu_va += (size + 0xFFF) & !0xFFF; // Align to 4K pages

    let alloc = DxgAllocation {
        h_allocation: 0,
        gpu_virtual_address: gpu_va,
        size,
        is_dma_buffer: is_dma,
        dxg_process_ptr: process_ptr,
    };

    GPU_ALLOCATOR.allocations.allocate(alloc)
}

/// Simulated write operation to a GPU Virtual Address (EventID 466)
pub unsafe fn map_allocation_to_va(h_alloc: u32, driver_handle: u64) -> u64 {
    if let Some(alloc) = GPU_ALLOCATOR.allocations.get(h_alloc) {
        // En la vida real, actualizaríamos las Page Tables (PTEs) de Ampere aquí.
        // Para satisfacer la ABI, solo retornamos el VA.
        return alloc.gpu_virtual_address;
    }
    0
}
