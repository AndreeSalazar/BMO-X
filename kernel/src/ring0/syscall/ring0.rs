//! `syscall/ring0.rs` — Ring 0 syscall table (0x00..=0x5F, 0xE0..=0xFF).
//!
//! v1.8.8: skeleton. Will host the dispatch table for the Ring 0
//! syscalls (process management, VFS, framebuffer, input, time,
//! system info, power, debug). The actual dispatch is still in
//! `arch::system_call_dispatcher` for backwards compatibility.

#![allow(dead_code)]

use super::numbers::*;

/// Returns true if the syscall number is handled by Ring 0.
pub const fn is_ring0_syscall(nr: u32) -> bool {
    // 0x00..=0x0F: process / thread
    nr <= 0x0F ||
    // 0x10..=0x1F: VFS / ramdisk
    (nr >= 0x10 && nr <= 0x1F) ||
    // 0x20..=0x2F: framebuffer
    (nr >= 0x20 && nr <= 0x2F) ||
    // 0x30..=0x3F: input
    (nr >= 0x30 && nr <= 0x3F) ||
    // 0x40..=0x4F: time
    (nr >= 0x40 && nr <= 0x4F) ||
    // 0x50..=0x5F: system info
    (nr >= 0x50 && nr <= 0x5F) ||
    // 0xE0..=0xEF: power
    (nr >= 0xE0 && nr <= 0xEF) ||
    // 0xF0..=0xFF: debug
    (nr >= 0xF0 && nr <= 0xFF)
}

/// Returns the name of a Ring 0 syscall (for diagnostics).
pub fn name(nr: u32) -> &'static str {
    match nr {
        NR_PROCESS_EXIT => "process_exit",
        NR_PROCESS_CREATE => "process_create",
        NR_PROCESS_WAIT => "process_wait",
        NR_THREAD_YIELD => "thread_yield",
        NR_THREAD_CREATE => "thread_create",
        NR_THREAD_EXIT => "thread_exit",
        NR_THREAD_JOIN => "thread_join",
        NR_THREAD_SET_AFFINITY => "thread_set_affinity",
        NR_THREAD_GET_ID => "thread_get_id",
        NR_PROCESS_GET_ID => "process_get_id",
        NR_FS_OPEN => "fs_open",
        NR_FS_CLOSE => "fs_close",
        NR_FS_READ => "fs_read",
        NR_FS_WRITE => "fs_write",
        NR_FS_SEEK => "fs_seek",
        NR_FS_STAT => "fs_stat",
        NR_FS_READDIR => "fs_readdir",
        NR_FS_MKDIR => "fs_mkdir",
        NR_FB_INFO => "fb_info",
        NR_FB_FILL => "fb_fill",
        NR_FB_BLIT => "fb_blit",
        NR_FB_PRESENT => "fb_present",
        NR_INPUT_POLL_KEY => "input_poll_key",
        NR_INPUT_POLL_MOUSE => "input_poll_mouse",
        NR_INPUT_READ_KEY => "input_read_key",
        NR_INPUT_READ_MOUSE => "input_read_mouse",
        NR_CLOCK_GET_TIME => "clock_get_time",
        NR_CLOCK_NANO_SLEEP => "clock_nano_sleep",
        NR_CLOCK_GET_TICKS => "clock_get_ticks",
        NR_SYS_INFO => "sys_info",
        NR_SYS_UPTIME => "sys_uptime",
        NR_SYS_CPU_INFO => "sys_cpu_info",
        NR_SYS_MEM_INFO => "sys_mem_info",
        NR_SYS_BEEP => "sys_beep",
        NR_POWEROFF => "poweroff",
        NR_REBOOT => "reboot",
        NR_SUSPEND => "suspend",
        NR_HIBERNATE => "hibernate",
        NR_DEBUG_PRINT => "debug_print",
        NR_PANIC => "panic",
        _ => "<unknown ring0 syscall>",
    }
}
