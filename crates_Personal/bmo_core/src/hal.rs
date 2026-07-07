pub struct HalServices {
    // dev::console
    pub serial_write: fn(&str),
    pub serial_write_u64: fn(u64, usize),
    // dev::watchdog
    pub pet_fch_watchdog: fn(),
    pub watchdog_disarm: fn(),
    // dev::framebuffer
    pub backbuffer_ptr: fn() -> *mut u32,
    pub backbuffer_stride: fn() -> usize,
    pub framebuffer_present: fn(),
    pub framebuffer_put_pixel: fn(u32, u32, u32),
    // dev::devour
    pub devour_init: fn(),
    // mm::phys
    pub phys_to_pt: unsafe fn(u64) -> *mut u64,
    pub alloc_pages_contiguous: fn(usize) -> u64,
    pub free_pages: unsafe fn(u64, usize),
    pub page_size: fn() -> usize,
    pub alloc_gbil_page: fn() -> u64,
    pub free_gbil_page: fn(u64),
    pub total_ram: fn() -> u64,
    // mm::virt
    pub map_user_range: fn(u64, u64, u64, usize, u64) -> i32,
    pub mark_current_identity_user_range: fn(u64, usize) -> i32,
    pub read_cr3: fn() -> u64,
    pub write_cr3: unsafe fn(u64),
    pub free_user_page_tables: unsafe fn(u64),
    pub create_user_page_table: unsafe fn(u64) -> u64,
    pub HIGH_MEM_BASE: u64,
    // mm::heap
    pub heap_alloc: fn(usize, usize) -> *mut u8,
    pub heap_free: fn(*mut u8, usize, usize),
    // cpu
    pub rdtsc: fn() -> u64,
    pub tsc_per_sec: fn() -> u64,
    pub tsc_calibrate: fn() -> u64,
    pub tsc_busy_wait_ms: fn(u64, u64),
    pub busy_wait_ms: fn(u64),
    pub halt: fn(),
    // info — writes bmo_core::info statics at init
    pub fb_addr: u64,
    pub fb_width: u32,
    pub fb_height: u32,
    pub fb_stride: u32,
    pub fb_pixel_format: u32,
    pub boot_info: *mut bmo_boot_protocol::BootInfo,
    // uefi_rt
    pub write_boot_stage: fn(&str),
    // visual
    pub clear: fn(),
    pub print_at: fn(u64, u64, u32, &str),
    pub print_at_u64: fn(u64, u64, u32, u64),
    pub fill_rect: fn(u64, u64, u64, u64, u32),
    pub draw_image: fn(*const u8, u64, u64, u64, u64),
    pub draw_image_clip: fn(*const u8, u64, u64, u64, u64, u64, u64, u64, u64),
    // font
    pub FONT8x16: *const [u8; 4096],
    // log
    pub log_write: fn(u8, &str),
    // cabina
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
    // proc::task
    pub task_current_index: fn() -> usize,
    pub task_set_current: fn(usize),
    pub task_get: fn(usize) -> *mut u8,
    pub task_alloc: fn(u32, u32) -> *mut u8,
    pub task_free: fn(*mut u8),
    pub task_current: fn() -> *mut u8,
    pub task_pick_next: fn() -> usize,
    pub task_block_on: fn(u64),
    pub task_wake_on: fn(u64, usize) -> usize,
    // proc::schedule
    pub schedule: fn(),
    pub yield_now: fn(),
    // phase_1_RING_0
    pub write_crash_marker: fn(u32),
    pub clear_crash_marker: fn(),
    // defense
    pub defense_init: fn(),
    // timeback
    pub timeback_init: fn(),
    // userland
    pub userland_init: fn(),
    // context
    pub context: *mut u8,
    // serial
    pub register_cabina_sink: fn(),
    // profile
    pub profile_main: fn() -> !,
    // omni::hud
    pub hud_tick: fn(),
    // arch::gdt
    pub KERNEL_CS: u64,
    pub KERNEL_DS: u64,
    pub USER_CS: u64,
    pub USER_DS: u64,
    pub set_kernel_stack: fn(u64),
    // arch::syscall
    pub set_syscall_kernel_stack: fn(u64),
    // ring3::transition
    pub ring3_transition: unsafe fn(u64, u64) -> !,
    // vendor
    pub issue_ibpb: fn(),
    pub amd_cpu_name: fn() -> &'static str,
    // bmo_audio
    pub audio_init: fn(u64),
    pub audio_play: fn(u32),
    pub audio_play_logon_chime: fn(),
    pub audio_beep: fn(u32, u32),
    pub audio_set_volume: fn(u32),
    // dev::storage
    pub storage_test: fn() -> bool,
}

pub static mut HAL: Option<HalServices> = None;

pub fn init(h: HalServices) {
    unsafe {
        crate::info::FB_ADDR = h.fb_addr;
        crate::info::FB_WIDTH = h.fb_width;
        crate::info::FB_HEIGHT = h.fb_height;
        crate::info::FB_STRIDE = h.fb_stride;
        crate::info::FB_PIXEL_FORMAT = h.fb_pixel_format;
        crate::info::BOOT_INFO = h.boot_info as *const bmo_boot_protocol::BootInfo;
        HAL = Some(h);
    }
}
