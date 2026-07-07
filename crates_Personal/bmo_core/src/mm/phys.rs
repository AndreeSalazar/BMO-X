use crate::hal;

pub unsafe fn phys_to_pt(paddr: u64) -> *mut u64 {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.phys_to_pt)(paddr) } else { core::ptr::null_mut() }
}

pub fn alloc_pages_contiguous(count: usize) -> Option<u64> {
    if let Some(h) = unsafe { hal::HAL.as_ref() } {
        let p = (h.alloc_pages_contiguous)(count);
        if p == 0 { None } else { Some(p) }
    } else { None }
}

pub unsafe fn free_pages(addr: u64, count: usize) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.free_pages)(addr, count); }
}

pub fn page_size() -> usize {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.page_size)() } else { 4096 }
}

pub fn alloc_gbil_page() -> u64 {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.alloc_gbil_page)() } else { 0 }
}

pub fn free_gbil_page(addr: u64) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.free_gbil_page)(addr); }
}

pub fn total_ram() -> u64 {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.total_ram)() } else { 0 }
}
