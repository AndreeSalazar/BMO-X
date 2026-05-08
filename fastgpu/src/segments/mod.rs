
use crate::abi::wddm_structs::*;
use core::ptr::null_mut;

pub const GA106_VRAM_SIZE: u64 = 12 * 1024 * 1024 * 1024; // 12GB
pub const GA106_WPR2_SIZE: u64 = 0x4000000; // 64MB

pub unsafe fn configure_segments(seg_out: *mut DXGK_QUERYSEGMENTOUT) {
    (*seg_out).NbSegment = 2;
    let desc = (*seg_out).pSegmentDescriptor as *mut DXGK_SEGMENTDESCRIPTOR;
    if !desc.is_null() {
        // VRAM
        (*desc.offset(0)).BaseAddress = 0;
        (*desc.offset(0)).CpuTranslatedAddress = 0x100000000;
        (*desc.offset(0)).Size = GA106_VRAM_SIZE as usize;
        (*desc.offset(0)).Flags = 0x1;
        
        // WPR2 (SEC2)
        (*desc.offset(1)).BaseAddress = GA106_VRAM_SIZE;
        (*desc.offset(1)).CpuTranslatedAddress = 0;
        (*desc.offset(1)).Size = GA106_WPR2_SIZE as usize;
        (*desc.offset(1)).Flags = 0x8; // WPR Enabled
    }
}
