//! HAL initialization — populates bmo_core's HalServices with ring0 function pointers.
//!
//! Called from `boot_phase::main()` after all subsystems are initialized.

use bmo_core::hal::HalServices;
use bmo_core::proc::process::Pid;

/// Build the HalServices table, wiring ring0 implementations into bmo_core.
///
/// Functions marked `/* stub */` correspond to HAL entries that exist for
/// completeness but are never called by the current bmo_core code path.
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
        phys_to_pt:               |paddr| addr_to_pt(paddr),
        alloc_pages_contiguous:   |n| unsafe { crate::mm::frame_alloc::alloc_pages_contiguous(n).unwrap_or(0) },
        free_pages:               |addr, count| unsafe { crate::mm::frame_alloc::free_pages(addr, count); },
        page_size:                || crate::mm::PAGE_SIZE as usize,
        alloc_gbil_page:          || 0,       /* stub */
        free_gbil_page:           |_| {},     /* stub */
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
        tsc_busy_wait_ms:         |_, _| {}, /* stub */
        busy_wait_ms:             crate::cpu::busy_wait_ms,
        halt:                     || loop { unsafe { core::arch::asm!("hlt"); } },

        // ── info (from ring0/info.rs statics) ───────────────────────
        fb_addr:                  unsafe { crate::info::FB_ADDR },
        fb_width:                 unsafe { crate::info::FB_WIDTH },
        fb_height:                unsafe { crate::info::FB_HEIGHT },
        fb_stride:                unsafe { crate::info::FB_STRIDE },
        fb_pixel_format:          unsafe { crate::info::FB_PIXEL_FORMAT as u32 },
        boot_info:                unsafe { crate::info::BOOT_INFO as *mut bmo_boot_protocol::BootInfo },

        // ── uefi_rt ─────────────────────────────────────────────────
        write_boot_stage:         |s| { let _ = crate::uefi_rt::write_boot_stage(s); },

        // ── visual (stubs — bmo_core uses Framebuffer methods directly) ─
        clear:                    || { crate::visual::clear(); },
        print_at:                 |_,_,_,_| {}, /* stub */
        print_at_u64:             |_,_,_,_| {}, /* stub */
        fill_rect:                |_,_,_,_,_| {}, /* stub */
        draw_image:               |_,_,_,_,_| {}, /* stub */
        draw_image_clip:          |_,_,_,_,_,_,_,_,_| {}, /* stub */

        // ── font ────────────────────────────────────────────────────
        FONT8x16:                 core::ptr::null(), /* stub */

        // ── log ─────────────────────────────────────────────────────
        log_write:                |_, _| {}, /* stub */

        // ── cabina ──────────────────────────────────────────────────
        cabina_init:              cabina_daemon::init,
        cabina_boot_ready:        || {
            cabina_daemon::info("bmo-kernel", "boot_ready — entering Ring 0 supervisor");
        },
        cabina_info:              |module, msg| {
            let s: &'static str = unsafe { core::mem::transmute(module) };
            let m: &'static str = unsafe { core::mem::transmute(msg) };
            cabina_daemon::info(s, m);
        },
        cabina_fault:             |module, msg| {
            let s: &'static str = unsafe { core::mem::transmute(module) };
            let m: &'static str = unsafe { core::mem::transmute(msg) };
            cabina_daemon::fault(s, m);
        },
        cabina_warn:              |module, msg| {
            let s: &'static str = unsafe { core::mem::transmute(module) };
            let m: &'static str = unsafe { core::mem::transmute(msg) };
            cabina_daemon::warn(s, m);
        },
        cabina_trace:             |module, msg| {
            let s: &'static str = unsafe { core::mem::transmute(module) };
            let m: &'static str = unsafe { core::mem::transmute(msg) };
            cabina_daemon::trace(s, m);
        },
        cabina_panic_msg:         |module, msg| {
            let s: &'static str = unsafe { core::mem::transmute(module) };
            let m: &'static str = unsafe { core::mem::transmute(msg) };
            cabina_daemon::fault(s, m);
        },
        cabina_info_u64:          |module, msg, v| {
            let s: &'static str = unsafe { core::mem::transmute(module) };
            let m: &'static str = unsafe { core::mem::transmute(msg) };
            cabina_daemon::emit_full(cabina_core::Severity::Info, cabina_core::Layer::Ring0,
                cabina_core::Entity::Module, s, v as u32, m, v);
        },
        cabina_warn_u64:          |module, msg, v| {
            let s: &'static str = unsafe { core::mem::transmute(module) };
            let m: &'static str = unsafe { core::mem::transmute(msg) };
            cabina_daemon::emit_full(cabina_core::Severity::Warning, cabina_core::Layer::Ring0,
                cabina_core::Entity::Module, s, v as u32, m, v);
        },
        cabina_fault_u64:         |module, msg, v| {
            let s: &'static str = unsafe { core::mem::transmute(module) };
            let m: &'static str = unsafe { core::mem::transmute(msg) };
            cabina_daemon::emit_full(cabina_core::Severity::Fault, cabina_core::Layer::Ring0,
                cabina_core::Entity::Module, s, v as u32, m, v);
        },
        cabina_trace_u64:         |module, msg, v| {
            let s: &'static str = unsafe { core::mem::transmute(module) };
            let m: &'static str = unsafe { core::mem::transmute(msg) };
            cabina_daemon::emit_full(cabina_core::Severity::Trace, cabina_core::Layer::Ring0,
                cabina_core::Entity::Module, s, v as u32, m, v);
        },
        cabina_is_overlay_enabled:|| crate::omni::hud::is_active(),
        cabina_set_overlay_enabled:|on| crate::omni::hud::set_enabled(on),
        cabina_cycle_tab:         || crate::omni::hud::cycle_tab(),
        cabina_cycle_query:       || crate::omni::hud::cycle_query(),
        cabina_paint_overlay:     || crate::omni::hud::paint(),

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
        timeback_init:            || {
            // Wire the NVRAM sink so checkpoints persist to UEFI variables.
            timeback::storage::register_nvram_sink(|name, data| {
                let _ = nvram_log::set_variable(name, data);
            });
            // Wire the SSD backend so the repo can write to T:/TIMEBACK.
            timeback::storage::register_ssd_backend(ssd_backend_wrapper);
            // Use the kernel's TSC as the monotonic tick source.
            timeback::set_tick_source(crate::cpu::rdtsc);
            timeback::init();
            // Auto-init the repo at T:/TIMEBACK if the SSD is available.
            if bmo_ahci::controller().is_some() {
                timeback::repo::init("T:/TIMEBACK");
                cabina_daemon::info("timeback", "Repo initialized at T:/TIMEBACK");
            } else {
                cabina_daemon::warn("timeback", "No AHCI controller — repo will use NVRAM only");
            }
            cabina_daemon::info("timeback", "TimeBack initialized with SSD + NVRAM persistence");
        },
        userland_init:            || {},

        // ── context ─────────────────────────────────────────────────
        context:                  ctx as *const _ as *mut u8,

        // ── serial ──────────────────────────────────────────────────
        register_cabina_sink:     crate::serial::register_cabina_sink,

        // ── profile ─────────────────────────────────────────────────
        profile_main:             || loop { unsafe { core::arch::asm!("hlt"); } },

        // ── omni::hud ───────────────────────────────────────────────
        hud_tick:                 crate::omni::hud::tick,

        // ── arch::gdt ───────────────────────────────────────────────
        KERNEL_CS:                crate::arch::gdt::KERNEL_CS as u64,
        KERNEL_DS:                crate::arch::gdt::KERNEL_DS as u64,
        USER_CS:                  crate::arch::gdt::USER_CS as u64,
        USER_DS:                  crate::arch::gdt::USER_DS as u64,
        set_kernel_stack:         crate::arch::gdt::set_kernel_stack,

        // ── arch::syscall ───────────────────────────────────────────
        set_syscall_kernel_stack: crate::arch::syscall::set_syscall_kernel_stack,

        // ── ring3::transition ───────────────────────────────────────
        ring3_transition:         crate::ring3::transition::ring3_transition,

        // ── vendor ──────────────────────────────────────────────────
        issue_ibpb:               crate::vendor::amd::cpu::zen3::errata_workarounds::issue_ibpb,
        amd_cpu_name:             || "AMD Ryzen",

        // ── bmo_audio (HD Audio Realtek ALC via HDA driver) ──────────
        audio_init:               |_freq| {
            // HD Audio doesn't need TSC frequency — volume is register-based
        },
        audio_play:               |_tone| {},
        audio_play_logon_chime:   || {
            // HD Audio doesn't have PIT beeps; logon sound is silent on HDA
            // Future: load a WAV file and play through HDA stream
        },
        audio_beep:               |_hz, _ms| {},
        audio_set_volume:         |v| crate::dev::hda::set_volume(v as u8),

        // ── dev::storage ────────────────────────────────────────────
        storage_test:             || false,
    }
}

// ── Adapters for mismatched signatures ──────────────────────────────────

fn addr_to_pt(paddr: u64) -> *mut u64 {
    crate::mm::vmm::phys_to_virt(paddr) as *mut u64
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

// ── TimeBack SSD backend thunk ──────────────────────────────────
//
// This thunk is registered as the SSD backend for TimeBack. It writes
// to a static RAM buffer that simulates the T: partition. When the
// real AHCI+FAT32 stack is fully working, this thunk will be replaced
// with a proper FAT32 file writer.
//
// Operations (op is a timeback::storage::SsdOp, but we re-declare it
// as u8 to avoid a dependency cycle):
//   0 = Mkdir   (data ignored, creates the directory in our RAM FS)
//   1 = Write   (data is the file content)
//   2 = Read    (writes the file content into data)
//   3 = ListDir (writes a null-separated list of entry names into data)
//
// The path "T:/TIMEBACK/..." is mapped to a flat namespace in RAM.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

const RAM_FS_MAX: usize = 64;
const RAM_FS_FILE_MAX: usize = 4096;

struct RamFile {
    path: String,
    data: Vec<u8>,
}

use spin::Mutex;
static RAM_FS: Mutex<Vec<RamFile>> = Mutex::new(Vec::new());

unsafe fn ssd_backend_thunk(op: u8, path: &str, data: &mut [u8]) -> bool {
    let mut fs = RAM_FS.lock();
    match op {
        0 => {
            // Mkdir: ensure a "directory marker" file exists.
            let dir_marker = alloc::format!("{}/.dir", path.trim_end_matches('/'));
            for f in fs.iter() {
                if f.path == dir_marker { return true; }
            }
            if fs.len() < RAM_FS_MAX {
                fs.push(RamFile { path: dir_marker, data: Vec::new() });
                return true;
            }
            false
        }
        1 => {
            // Write: store or replace the file.
            for f in fs.iter_mut() {
                if f.path == path {
                    f.data = data.to_vec();
                    return true;
                }
            }
            if fs.len() < RAM_FS_MAX {
                fs.push(RamFile { path: String::from(path), data: data.to_vec() });
                return true;
            }
            false
        }
        2 => {
            // Read: copy file data into `data`.
            for f in fs.iter() {
                if f.path == path {
                    let n = f.data.len().min(data.len());
                    data[..n].copy_from_slice(&f.data[..n]);
                    return true;
                }
            }
            false
        }
        3 => {
            // ListDir: list entries in `path/`.
            let prefix_trimmed = path.trim_end_matches('/');
            let prefix = alloc::format!("{}/", prefix_trimmed);
            let mut off = 0;
            let mut seen: Vec<String> = Vec::new();
            for f in fs.iter() {
                if f.path.starts_with(&prefix) && f.path != alloc::format!("{}/.dir", prefix_trimmed) {
                    let rest = &f.path[prefix.len()..];
                    if let Some(slash) = rest.find('/') {
                        let dir = &rest[..slash];
                        if !seen.iter().any(|s| s == dir) {
                            seen.push(String::from(dir));
                            if off + dir.len() + 1 <= data.len() {
                                data[off..off + dir.len()].copy_from_slice(dir.as_bytes());
                                off += dir.len();
                                data[off] = 0;
                                off += 1;
                            }
                        }
                    } else if !rest.is_empty() && rest != ".dir" {
                        if !seen.iter().any(|s| s == rest) {
                            seen.push(String::from(rest));
                            if off + rest.len() + 1 <= data.len() {
                                data[off..off + rest.len()].copy_from_slice(rest.as_bytes());
                                off += rest.len();
                                data[off] = 0;
                                off += 1;
                            }
                        }
                    }
                }
            }
            true
        }
        _ => false,
    }
}

// Adapter to match timeback's SsdBackend signature (uses enum, not u8).
unsafe fn ssd_backend_wrapper(
    op: timeback::storage::SsdOp,
    path: &str,
    data: &mut [u8],
) -> bool {
    ssd_backend_thunk(op as u8, path, data)
}
