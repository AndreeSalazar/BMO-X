
use crate::abi::wddm_structs::*;

pub unsafe fn configure_mmu_caps(mmcaps: *mut DXGK_GPUMMUCAPS) {
    // Ampere GA106 specific values
    (*mmcaps).virtual_address_bit_count = 49;
    (*mmcaps).physical_address_bit_count = 40;
    (*mmcaps).page_table_level_count = 5;
    (*mmcaps).page_table_page_size = 4096;
}
