use super::errno;
use core::sync::atomic::{AtomicU64, Ordering};

const MMAP_BASE: u64 = 0x1_0000_0000;
const MMAP_END: u64 = 0x7_0000_0000;

static NEXT_MMAP_ADDR: AtomicU64 = AtomicU64::new(MMAP_BASE);

fn alloc_vaddr(size: usize) -> Option<u64> {
    let align = 4096u64;
    let addr = NEXT_MMAP_ADDR.fetch_add((size as u64 + align - 1) & !(align - 1), Ordering::Relaxed);
    let end = addr.checked_add(size as u64)?;
    if end > MMAP_END { return None; }
    Some(addr)
}

pub fn sys_mmap(a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> i64 {
    let addr = a0 as *mut u8;
    let len = a1 as usize;
    let prot = a2 as i32;
    let flags = a3 as i32;
    let _fd = a4 as i32;
    let _off = a5 as isize;

    let align = 4096usize;
    let size = (len + align - 1) & !(align - 1);
    if size == 0 { return -errno::EINVAL; }

    let hint = addr as u64;
    let fixed = (flags & 0x10) != 0;
    let anonymous = (flags & 0x20) != 0;

    let vaddr = if fixed {
        if hint == 0 || hint % 4096 != 0 { return -errno::EINVAL; }
        // Reject kernel-space addresses.
        if hint >= 0x0000_8000_0000_0000 { return -errno::EINVAL; }
        hint
    } else if anonymous {
        alloc_vaddr(size).unwrap_or(0)
    } else {
        return -errno::ENODEV;
    };

    if vaddr == 0 { return -errno::ENOMEM; }

    let pages = size / 4096;
    let paddr = match unsafe { crate::mm::phys::alloc_pages_contiguous(pages) } {
        Some(p) => p,
        None => return -errno::ENOMEM,
    };

    for i in 0..pages {
        let pv = crate::mm::virt::phys_to_virt(paddr + (i as u64) * 4096);
        unsafe { core::ptr::write_bytes(pv as *mut u8, 0, 4096); }
    }

    let mut pt_flags = crate::mm::virt::flags::PRESENT | crate::mm::virt::flags::USER;
    if (prot & 0x02) != 0 { pt_flags |= crate::mm::virt::flags::WRITABLE; }
    if (prot & 0x04) == 0 { pt_flags |= crate::mm::virt::flags::NO_EXECUTE; }

    if let Some(task) = crate::proc::task::current() {
        if let Some(proc) = crate::proc::process::get_process(task.pid) {
            let result = unsafe {
                crate::mm::virt::map_user_range(proc.page_table_root, vaddr, paddr, pages, pt_flags)
            };
            if let Err(e) = result {
                crate::cabina::info("linux.mmap", e);
                unsafe { crate::mm::phys::free_pages(paddr, pages); }
                return -errno::ENOMEM;
            }
        } else {
            unsafe { crate::mm::phys::free_pages(paddr, pages); }
            return -errno::ENOMEM;
        }
    } else {
        unsafe { crate::mm::phys::free_pages(paddr, pages); }
        return -errno::ENOMEM;
    }

    vaddr as i64
}

pub fn sys_munmap(_a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 { 0 }

pub fn sys_mprotect(_a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 { 0 }

static mut PROGRAM_BREAK: u64 = 0x7F00_0000_0000;

pub fn sys_brk(a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    let new_brk = a0;
    unsafe {
        if new_brk == 0 { return PROGRAM_BREAK as i64; }
        if new_brk > PROGRAM_BREAK {
            let grow = (new_brk - PROGRAM_BREAK + 4095) & !4095;
            if grow > 0 {
                let pages = (grow / 4096) as usize;
                let paddr = match crate::mm::phys::alloc_pages_contiguous(pages) {
                    Some(p) => p,
                    None => return PROGRAM_BREAK as i64,
                };
                for i in 0..pages {
                    let pv = crate::mm::virt::phys_to_virt(paddr + (i as u64) * 4096);
                    core::ptr::write_bytes(pv as *mut u8, 0, 4096);
                }
                let pf = crate::mm::virt::flags::PRESENT
                    | crate::mm::virt::flags::USER
                    | crate::mm::virt::flags::WRITABLE
                    | crate::mm::virt::flags::NO_EXECUTE;
                if let Some(task) = crate::proc::task::current() {
                    if let Some(proc) = crate::proc::process::get_process(task.pid) {
                        let result = unsafe {
                            crate::mm::virt::map_user_range(proc.page_table_root, PROGRAM_BREAK, paddr, pages, pf)
                        };
                        if result.is_err() {
                            unsafe { crate::mm::phys::free_pages(paddr, pages); }
                            return PROGRAM_BREAK as i64;
                        }
                    } else {
                        unsafe { crate::mm::phys::free_pages(paddr, pages); }
                        return PROGRAM_BREAK as i64;
                    }
                } else {
                    unsafe { crate::mm::phys::free_pages(paddr, pages); }
                    return PROGRAM_BREAK as i64;
                }
                PROGRAM_BREAK += grow;
            }
        }
        PROGRAM_BREAK as i64
    }
}
