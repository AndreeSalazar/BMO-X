
use crate::abi::wddm_structs::*;
use core::ptr::null_mut;

pub const GA106_VRAM_SIZE: u64 = 12 * 1024 * 1024 * 1024; // 12GB
pub const GA106_WPR2_SIZE: u64 = 0x4000000; // 64MB

pub unsafe fn configure_segments(seg_out: *mut DXGK_QUERYSEGMENTOUT) {
    // (*seg_out).nb_segments = 2;
    // let desc = (*seg_out).segment_descriptor;
    if !desc.is_null() {
        // VRAM
        (*desc.offset(0)).base_address = 0;
        (*desc.offset(0)).cpu_visible_address = 0x100000000;
        (*desc.offset(0)).size = GA106_VRAM_SIZE;
        (*desc.offset(0)).flags = 0x1;
        
        // WPR2 (SEC2)
        (*desc.offset(1)).base_address = GA106_VRAM_SIZE;
        (*desc.offset(1)).cpu_visible_address = 0;
        (*desc.offset(1)).size = GA106_WPR2_SIZE;
        (*desc.offset(1)).flags = 0x8; // WPR Enabled
    }
}
