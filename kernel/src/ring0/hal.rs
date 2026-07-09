//! HAL initialization — builds HalServices function pointer table for modules.
//!
//! Essential Ring 0 services only. Ring 3 services (cabina, storage, input,
//! audio, visual) are stubbed — modules provide or request their own drivers.

use bmo_hal_defs::HalServices;
use crate::proc::process::Pid;

pub static mut HAL_SERVICES: *const HalServices = core::ptr::null();

pub fn build(ctx: &crate::context::BootContext) -> HalServices {
    HalServices {
        // ── dev::console ────────────────────────────────────────────
        serial_write:             crate::dev::console::serial_write,
        serial_write_u64:         crate::dev::console::serial_write_u64,

        // ── dev::watchdog ───────────────────────────────────────────
        pet_fch_watchdog:         crate::dev::watchdog::pet_fch_watchdog,
        watchdog_disarm:          crate::dev::watchdog::disarm,

        // ── dev::framebuffer ────────────────────────────────────────
        backbuffer_ptr:           crate::dev::framebuffer::backbuffer_ptr,
        backbuffer_stride:        || unsafe { crate::info::FB_STRIDE as usize * 4 },
        framebuffer_present:      crate::dev::framebuffer::present,
        framebuffer_put_pixel:    |x, y, c| crate::dev::framebuffer::put_pixel(x, y, crate::dev::framebuffer::Color(c)),

        // ── dev::devour ─────────────────────────────────────────────
        devour_init:              || {},

        // ── mm::phys ────────────────────────────────────────────────
        phys_to_pt:               |paddr| crate::mm::vmm::phys_to_virt(paddr) as *mut u64,
        alloc_pages_contiguous:   |n| unsafe { crate::mm::frame_alloc::alloc_pages_contiguous(n).unwrap_or(0) },
        free_pages:               |addr, count| unsafe { crate::mm::frame_alloc::free_pages(addr, count); },
        page_size:                || crate::mm::PAGE_SIZE as usize,
        alloc_gbil_page:          || 0,
        free_gbil_page:           |_| {},
        total_ram:                crate::mm::frame_alloc::total_ram,

        // ── mm::virt ────────────────────────────────────────────────
        map_user_range:           |a,b,c,d,e| adapt_map_user_range(a,b,c,d,e),
        mark_current_identity_user_range: |a,b| adapt_mark_identity(a,b),
        read_cr3:                 crate::mm::vmm::read_cr3,
        write_cr3:                |pml4| unsafe { crate::mm::vmm::write_cr3(pml4); },
        free_user_page_tables:    |pml4| unsafe { crate::mm::vmm::free_user_page_tables(pml4); },
        create_user_page_table:   |cr3| unsafe { crate::mm::vmm::create_user_page_table(cr3).unwrap_or(0) },
        HIGH_MEM_BASE:            0xFFFF_8000_0000_0000,

        // ── mm::heap ────────────────────────────────────────────────
        heap_alloc:               |size, align| unsafe { crate::mm::slab::heap_alloc(size, align) },
        heap_free:                |ptr, size, align| unsafe { crate::mm::slab::heap_free(ptr, size, align); },

        // ── cpu ─────────────────────────────────────────────────────
        rdtsc:                    crate::cpu::rdtsc,
        tsc_per_sec:              crate::cpu::tsc_per_sec,
        tsc_calibrate:            crate::cpu::tsc_per_sec,
        tsc_busy_wait_ms:         |_, _| {},
        busy_wait_ms:             crate::cpu::busy_wait_ms,
        halt:                     || loop { unsafe { core::arch::asm!("hlt"); } },

        // ── info ────────────────────────────────────────────────────
        fb_addr:                  unsafe { crate::info::FB_ADDR },
        fb_width:                 unsafe { crate::info::FB_WIDTH },
        fb_height:                unsafe { crate::info::FB_HEIGHT },
        fb_stride:                unsafe { crate::info::FB_STRIDE },
        fb_pixel_format:          unsafe { crate::info::FB_PIXEL_FORMAT as u32 },
        boot_info:                unsafe { crate::info::BOOT_INFO as *mut bmo_boot_protocol::BootInfo },

        // ── uefi_rt ─────────────────────────────────────────────────
        write_boot_stage:         |s| { let _ = crate::uefi_rt::write_boot_stage(s); },

        // ── visual (stubbed — module handles rendering) ─────────────
        clear:                    || {},
        print_at:                 |_,_,_,_| {},
        print_at_u64:             |_,_,_,_| {},
        fill_rect:                |_,_,_,_,_| {},
        draw_image:               |_,_,_,_,_| {},
        draw_image_clip:          |_,_,_,_,_,_,_,_,_| {},

        // ── font ────────────────────────────────────────────────────
        FONT8x16:                 core::ptr::null(),

        // ── log ─────────────────────────────────────────────────────
        log_write:                |_, _| {},

        // ── cabina (stubbed — runs as separate Ring 3 module) ────
        cabina_init:              || {},
        cabina_boot_ready:        || {},
        cabina_info:              |_, _| {},
        cabina_fault:             |_, _| {},
        cabina_warn:              |_, _| {},
        cabina_trace:             |_, _| {},
        cabina_panic_msg:         |_, _| {},
        cabina_info_u64:          |_, _, _| {},
        cabina_warn_u64:          |_, _, _| {},
        cabina_fault_u64:         |_, _, _| {},
        cabina_trace_u64:         |_, _, _| {},
        cabina_is_overlay_enabled:|| false,
        cabina_set_overlay_enabled:|_| {},
        cabina_cycle_tab:         || {},
        cabina_cycle_query:       || false,
        cabina_paint_overlay:     || {},

        // ── proc::task ──────────────────────────────────────────────
        task_current_index:       crate::proc::task::current_index,
        task_set_current:         crate::proc::task::set_current,
        task_get:                 |idx| {
            crate::proc::task::get(idx)
                .map(|t| t as *mut _ as *mut u8)
                .unwrap_or(core::ptr::null_mut())
        },
        task_alloc:               |pid, _prio| {
            match crate::proc::task::alloc(Pid(pid), crate::proc::Priority::Interactive) {
                Some(t) => t as *mut _ as *mut u8,
                None => core::ptr::null_mut(),
            }
        },
        task_free:                |ptr| {
            if !ptr.is_null() {
                let t = unsafe { &mut *(ptr as *mut crate::proc::task::Task) };
                crate::proc::task::free_task(t);
            }
        },
        task_current:             || {
            crate::proc::task::current()
                .map(|t| t as *mut _ as *mut u8)
                .unwrap_or(core::ptr::null_mut())
        },
        task_pick_next:           || crate::proc::task::pick_next().unwrap_or(usize::MAX),
        task_block_on:            crate::proc::task::block_on,
        task_wake_on:             crate::proc::task::wake_on,

        // ── proc::schedule ──────────────────────────────────────────
        schedule:                 || {},
        yield_now:                || {},

        // ── boot_phase ─────────────────────────────────────────────
        write_crash_marker:       crate::boot_phase::write_crash_marker,
        clear_crash_marker:       crate::boot_phase::clear_crash_marker,

        // ── defense / timeback / userland ───────────────────────────
        defense_init:             || {},
        timeback_init:            || {},
        userland_init:            || {},

        // ── context ─────────────────────────────────────────────────
        context:                  ctx as *const _ as *mut u8,

        // ── serial ──────────────────────────────────────────────────
        register_cabina_sink:     || {},

        // ── profile ─────────────────────────────────────────────────
        profile_main:             || loop { unsafe { core::arch::asm!("hlt"); } },

        // ── omni::hud ───────────────────────────────────────────────
        hud_tick:                 || {},

        // ── arch::gdt ───────────────────────────────────────────────
        KERNEL_CS:                crate::arch::gdt::KERNEL_CS as u64,
        KERNEL_DS:                crate::arch::gdt::KERNEL_DS as u64,
        USER_CS:                  crate::arch::gdt::USER_CS as u64,
        USER_DS:                  crate::arch::gdt::USER_DS as u64,
        set_kernel_stack:         crate::arch::gdt::set_kernel_stack,

        // ── arch::syscall ───────────────────────────────────────────
        set_syscall_kernel_stack: crate::arch::syscall::set_syscall_kernel_stack,

        // ── ring3::transition ───────────────────────────────────────
        ring3_transition:         |_, _| loop { core::hint::spin_loop() },

        // ── vendor ──────────────────────────────────────────────────
        issue_ibpb:               crate::vendor::amd::cpu::zen3::errata_workarounds::issue_ibpb,
        amd_cpu_name:             || "AMD Ryzen",

        // ── audio (stubbed — module provides HDA driver) ───────────
        audio_init:               |_| {},
        audio_play:               |_| {},
        audio_play_logon_chime:   || {},
        audio_beep:               |_, _| {},
        audio_set_volume:         |_| {},

        // ── dev::storage (stubbed — module provides AHCI driver) ───
        storage_test:             || false,

        // ── input (stubbed — module provides input drivers) ─────────
        input_init:               || false,
        input_poll:               |_| 0,

        // ── storage HAL (stubbed — module provides AHCI) ────────────
        storage_read_sectors:     |_, _, _, _| false,
        storage_write_sectors:    |_, _, _, _| false,
        storage_port_count:       || 0,
        storage_port_active:      |_| false,

        // ── filesystem HAL (stubbed — module provides FAT32) ────────
        fs_mount:                 |_| false,
        fs_read_file:             |_, _, _| None,
        fs_write_file:            |_, _, _| false,
        fs_find_subdir:           |_, _| None,
    }
}

fn adapt_map_user_range(pml4: u64, virt: u64, phys: u64, pages: usize, flags: u64) -> i32 {
    match unsafe { crate::mm::vmm::map_user_range(pml4, virt, phys, pages, flags) } {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

fn adapt_mark_identity(start: u64, len: usize) -> i32 {
    match unsafe { crate::mm::vmm::mark_current_identity_user_range(start, len) } {
        Ok(()) => 0,
        Err(_) => -1,
    }
}
