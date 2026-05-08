
use crate::abi::wddm_structs::*;

pub unsafe fn configure_mmu_caps(mmcaps: *mut DXGK_GPUMMUCAPS) {
    // Ampere GA106 specific values
    (*mmcaps).VirtualAddressBitCount = 49;
    (*mmcaps).PageTableLevelCount = 5;
    (*mmcaps).LeafPageTableSizeFor64KPagesInBytes = 4096;
}
