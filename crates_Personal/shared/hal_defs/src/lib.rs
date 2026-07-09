//! BMO HAL Definitions — shared types between Ring 0 kernel and Ring 3 modules.
//!
//! This crate contains NO implementations — only type definitions and
//! function pointer signatures. Ring 0 populates the table, Ring 3 consumes it.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;

// ═══════════════════════════════════════════════════════════════════
//  Input Event Types (replaces direct bmo_input dependency)
// ═══════════════════════════════════════════════════════════════════

/// Device-agnostic input event.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct InputEvent {
    pub timestamp: u64,
    pub device_id: u16,
    pub kind: InputEventKind,
    pub _pad: u8,
    pub code: u8,
    pub value: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InputEventKind {
    KeyDown     = 0x01,
    KeyUp       = 0x02,
    MouseMove   = 0x03,
    MouseButton = 0x04,
    MouseWheel  = 0x05,
}

impl InputEvent {
    pub const fn empty() -> Self {
        Self { timestamp: 0, device_id: 0, kind: InputEventKind::KeyDown, _pad: 0, code: 0, value: 0 }
    }

    pub fn mouse_dx(&self) -> i16 {
        (self.value & 0xFFFF) as i16
    }

    pub fn mouse_dy(&self) -> i16 {
        ((self.value >> 16) & 0xFFFF) as i16
    }

    pub fn mouse_buttons(&self) -> u8 {
        (self.value & 0xFF) as u8
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Module Loading Types
// ═══════════════════════════════════════════════════════════════════

/// Ring level for a loaded module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ModuleRing {
    Ring0 = 0,
    Ring3 = 3,
}

/// Header embedded in each module ELF at a known section.
#[repr(C)]
pub struct ModuleHeader {
    pub magic: u32,          // 0x464D4F44 = "FMOD"
    pub version: u32,
    pub name: [u8; 64],
    pub ring_level: u8,
    pub deps_count: u8,
    pub _pad: [u8; 2],
    pub entry_fn: [u8; 64],  // symbol name of entry function
}

pub const FMOD_MAGIC: u32 = 0x464D_4F44;

/// Registers passed to a module's entry function.
#[repr(C)]
pub struct ModuleInitRegs {
    pub hal_services: *const HalServices,
    pub module_base: u64,
    pub module_size: u64,
}

// ═══════════════════════════════════════════════════════════════════
//  HalServices — the Ring 0 → Ring 3 bridge
// ═══════════════════════════════════════════════════════════════════

/// Function pointer table populated by Ring 0, consumed by Ring 3.
///
/// Every hardware/kernel service is accessed through this table.
/// Ring 3 code NEVER imports Ring 0 modules directly.
#[derive(Copy, Clone)]
pub struct HalServices {
    // ── dev::console ────────────────────────────────────────────
    pub serial_write: fn(&str),
    pub serial_write_u64: fn(u64, usize),

    // ── dev::watchdog ───────────────────────────────────────────
    pub pet_fch_watchdog: fn(),
    pub watchdog_disarm: fn(),

    // ── dev::framebuffer ────────────────────────────────────────
    pub backbuffer_ptr: fn() -> *mut u32,
    pub backbuffer_stride: fn() -> usize,
    pub framebuffer_present: fn(),
    pub framebuffer_put_pixel: fn(u32, u32, u32),

    // ── dev::devour ─────────────────────────────────────────────
    pub devour_init: fn(),

    // ── mm::phys ────────────────────────────────────────────────
    pub phys_to_pt: unsafe fn(u64) -> *mut u64,
    pub alloc_pages_contiguous: fn(usize) -> u64,
    pub free_pages: unsafe fn(u64, usize),
    pub page_size: fn() -> usize,
    pub alloc_gbil_page: fn() -> u64,
    pub free_gbil_page: fn(u64),
    pub total_ram: fn() -> u64,

    // ── mm::virt ────────────────────────────────────────────────
    pub map_user_range: fn(u64, u64, u64, usize, u64) -> i32,
    pub mark_current_identity_user_range: fn(u64, usize) -> i32,
    pub read_cr3: fn() -> u64,
    pub write_cr3: unsafe fn(u64),
    pub free_user_page_tables: unsafe fn(u64),
    pub create_user_page_table: unsafe fn(u64) -> u64,
    pub HIGH_MEM_BASE: u64,

    // ── mm::heap ────────────────────────────────────────────────
    pub heap_alloc: fn(usize, usize) -> *mut u8,
    pub heap_free: fn(*mut u8, usize, usize),

    // ── cpu ─────────────────────────────────────────────────────
    pub rdtsc: fn() -> u64,
    pub tsc_per_sec: fn() -> u64,
    pub tsc_calibrate: fn() -> u64,
    pub tsc_busy_wait_ms: fn(u64, u64),
    pub busy_wait_ms: fn(u64),
    pub halt: fn(),

    // ── info ────────────────────────────────────────────────────
    pub fb_addr: u64,
    pub fb_width: u32,
    pub fb_height: u32,
    pub fb_stride: u32,
    pub fb_pixel_format: u32,
    pub boot_info: *mut bmo_boot_protocol::BootInfo,

    // ── uefi_rt ─────────────────────────────────────────────────
    pub write_boot_stage: fn(&str),

    // ── visual ──────────────────────────────────────────────────
    pub clear: fn(),
    pub print_at: fn(u64, u64, u32, &str),
    pub print_at_u64: fn(u64, u64, u32, u64),
    pub fill_rect: fn(u64, u64, u64, u64, u32),
    pub draw_image: fn(*const u8, u64, u64, u64, u64),
    pub draw_image_clip: fn(*const u8, u64, u64, u64, u64, u64, u64, u64, u64),

    // ── font ────────────────────────────────────────────────────
    pub FONT8x16: *const [u8; 4096],

    // ── log ─────────────────────────────────────────────────────
    pub log_write: fn(u8, &str),

    // ── cabina ──────────────────────────────────────────────────
    pub cabina_init: fn(),
    pub cabina_boot_ready: fn(),
    pub cabina_info: fn(&str, &str),
    pub cabina_fault: fn(&str, &str),
    pub cabina_warn: fn(&str, &str),
    pub cabina_trace: fn(&str, &str),
    pub cabina_panic_msg: fn(&str, &str),
    pub cabina_info_u64: fn(&str, &str, u64),
    pub cabina_warn_u64: fn(&str, &str, u64),
    pub cabina_fault_u64: fn(&str, &str, u64),
    pub cabina_trace_u64: fn(&str, &str, u64),
    pub cabina_is_overlay_enabled: fn() -> bool,
    pub cabina_set_overlay_enabled: fn(bool),
    pub cabina_cycle_tab: fn(),
    pub cabina_cycle_query: fn() -> bool,
    pub cabina_paint_overlay: fn(),

    // ── proc::task ──────────────────────────────────────────────
    pub task_current_index: fn() -> usize,
    pub task_set_current: fn(usize),
    pub task_get: fn(usize) -> *mut u8,
    pub task_alloc: fn(u32, u32) -> *mut u8,
    pub task_free: fn(*mut u8),
    pub task_current: fn() -> *mut u8,
    pub task_pick_next: fn() -> usize,
    pub task_block_on: fn(u64),
    pub task_wake_on: fn(u64, usize) -> usize,

    // ── proc::schedule ──────────────────────────────────────────
    pub schedule: fn(),
    pub yield_now: fn(),

    // ── boot_phase ─────────────────────────────────────────────
    pub write_crash_marker: fn(u32),
    pub clear_crash_marker: fn(),

    // ── defense / timeback / userland ───────────────────────────
    pub defense_init: fn(),
    pub timeback_init: fn(),
    pub userland_init: fn(),

    // ── context ─────────────────────────────────────────────────
    pub context: *mut u8,

    // ── serial ──────────────────────────────────────────────────
    pub register_cabina_sink: fn(),

    // ── profile ─────────────────────────────────────────────────
    pub profile_main: fn() -> !,

    // ── omni::hud ───────────────────────────────────────────────
    pub hud_tick: fn(),

    // ── arch::gdt ───────────────────────────────────────────────
    pub KERNEL_CS: u64,
    pub KERNEL_DS: u64,
    pub USER_CS: u64,
    pub USER_DS: u64,
    pub set_kernel_stack: fn(u64),

    // ── arch::syscall ───────────────────────────────────────────
    pub set_syscall_kernel_stack: fn(u64),

    // ── ring3::transition ───────────────────────────────────────
    pub ring3_transition: unsafe fn(u64, u64) -> !,

    // ── vendor ──────────────────────────────────────────────────
    pub issue_ibpb: fn(),
    pub amd_cpu_name: fn() -> &'static str,

    // ── bmo_audio ───────────────────────────────────────────────
    pub audio_init: fn(u64),
    pub audio_play: fn(u32),
    pub audio_play_logon_chime: fn(),
    pub audio_beep: fn(u32, u32),
    pub audio_set_volume: fn(u32),

    // ── dev::storage ────────────────────────────────────────────
    pub storage_test: fn() -> bool,

    // ═══════════════════════════════════════════════════════════
    //  NEW: Input HAL (replaces direct bmo_input/bmo_uhid imports)
    // ═══════════════════════════════════════════════════════════
    pub input_init: fn() -> bool,
    pub input_poll: fn(&mut [InputEvent]) -> usize,

    // ═══════════════════════════════════════════════════════════
    //  NEW: Storage HAL (replaces direct bmo_ahci imports)
    // ═══════════════════════════════════════════════════════════
    pub storage_read_sectors: fn(u8, u64, u16, *mut u8) -> bool,
    pub storage_write_sectors: fn(u8, u64, u16, *const u8) -> bool,
    pub storage_port_count: fn() -> u8,
    pub storage_port_active: fn(u8) -> bool,

    // ═══════════════════════════════════════════════════════════
    //  NEW: Filesystem HAL (replaces direct bmo_fat32 imports)
    // ═══════════════════════════════════════════════════════════
    pub fs_mount: fn(u8) -> bool,
    pub fs_read_file: fn(u8, &str, &mut [u8]) -> Option<usize>,
    pub fs_write_file: fn(u8, &str, &[u8]) -> bool,
    pub fs_find_subdir: fn(u8, &str) -> Option<u64>,
}
