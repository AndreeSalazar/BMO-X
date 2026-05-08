

use crate::handles::HandleTable;

/// Represents a Monitored Fence (EventID 295, 550, 551)
/// WDDM 3.x relies heavily on Monitored Fences for CPU/GPU synchronization.
#[derive(Copy, Clone)]
pub struct MonitoredFence {
    pub h_sync_object: u32,
    pub current_value: u64,
    pub signaled: bool,
    // Emula VidSchSyncObjectArray / DxgSyncObjectArray
    pub dxg_sync_object_ptr: u64, 
}

/// Represents a Hardware Queue (EventID 450, 305)
/// nvlddmkm submits DMA buffers directly to this queue.
#[derive(Copy, Clone)]
pub struct HwQueue {
    pub h_hw_queue: u32,
    pub h_context: u32,
    pub progress_fence_value: u64,
    pub pending_dma_buffer: u64, // pDmaBuffer address
}

#[derive(Copy, Clone)]
pub struct GpuContext {
    pub h_context: u32,
    pub node_ordinal: u32,
    pub engine_affinity: u32,
}

pub struct SchedulerState {
    pub fences: HandleTable<MonitoredFence>,
    pub hw_queues: HandleTable<HwQueue>,
    pub contexts: HandleTable<GpuContext>,
    pub global_submit_sequence: u64,
}

impl SchedulerState {
    pub const fn new() -> Self {
        Self {
            fences: HandleTable::new(),
            hw_queues: HandleTable::new(),
            contexts: HandleTable::new(),
            global_submit_sequence: 0,
        }
    }
}

pub static mut GPU_SCHEDULER: SchedulerState = SchedulerState::new();

/// Called when Dxgkrnl requests to create a sync object array (EventID 550)
pub unsafe fn create_monitored_fence(dxg_sync_ptr: u64) -> Option<u32> {
    let fence = MonitoredFence {
        h_sync_object: 0,
        current_value: 0,
        signaled: false,
        dxg_sync_object_ptr: dxg_sync_ptr,
    };
    GPU_SCHEDULER.fences.allocate(fence)
}

pub unsafe fn create_context(node: u32, affinity: u32, _is_paging: bool) -> Option<u32> {
    let ctx = GpuContext {
        h_context: 0,
        node_ordinal: node,
        engine_affinity: affinity,
    };
    GPU_SCHEDULER.contexts.allocate(ctx)
}

pub unsafe fn destroy_context(h_context: u32) {
    GPU_SCHEDULER.contexts.free(h_context);
}

/// Called when Dxgkrnl submits a DMA buffer to the hardware queue (EventID 450)
pub unsafe fn submit_to_hw_queue(h_queue: u32, dma_buffer: u64, fence_val: u64) -> bool {
    if let Some(mut q) = GPU_SCHEDULER.hw_queues.get(h_queue) {
        q.pending_dma_buffer = dma_buffer;
        q.progress_fence_value = fence_val;
        GPU_SCHEDULER.global_submit_sequence += 1;
        
        // Aquí FastOS le pasaría el DMA Buffer al hardware real (o al simulador GSP)
        // Por ahora lo aceptamos pasivamente para satisfacer la ABI.
        return true;
    }
    false
}
