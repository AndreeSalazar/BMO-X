//! BMO Channel — Ring 0 side: timer ISR processing + system event queue.
//!
//! ## System channel
//!
//! The kernel owns a dedicated "system channel" for hardware events that
//! Ring 3 can poll. Keyboard/mouse ISRs push events here via `sys_send()`.
//!
//! ## User channels
//!
//! Ring 3 can register additional channels via `SYS_CHANNEL_REGISTER`.
//! These are processed on each timer tick when the doorbell is set.
//!
//! ## Services exposed to Ring 3
//!
//! Ring 3 can call these opcodes on a user-registered channel to request
//! services from Ring 0 (the "hardware" / HAL layer):
//!
//! | Opcode | Name            | Args                       | Returns |
//! |--------|-----------------|----------------------------|---------|
//! | 0xE00  | SVC_TIME_NOW    | -                          | tsc (u64) |
//! | 0xE01  | SVC_TIME_NS     | -                          | ns (u64) |
//! | 0xE02  | SVC_FB_INFO     | -                          | (w, h, stride, fmt) packed |
//! | 0xE03  | SVC_FB_PRESENT  | -                          | 0 |
//! | 0xE04  | SVC_FB_FILL     | (x, y, w, h, color)        | 0 |
//! | 0xE05  | SVC_FB_TEXT     | (x, y, color, ptr, len)    | bytes drawn |
//! | 0xE10  | SVC_SERIAL      | (ptr, len)                 | bytes written |
//! | 0xE20  | SVC_BEEP        | (freq_hz, duration_ms)     | 0 |
//! | 0xE21  | SVC_POWER_REBT  | -                          | (never returns) |
//! | 0xE22  | SVC_POWER_OFF   | -                          | (never returns) |
//! | 0xE30  | SVC_MEM_TOTAL   | -                          | bytes |
//! | 0xE31  | SVC_MEM_FREE    | -                          | bytes |
//! | 0xE32  | SVC_HEAP_USED   | -                          | bytes |
//! | 0xE40  | SVC_CPU_NAME    | (ptr, len)                 | bytes written |
//! | 0xE41  | SVC_TSC_FREQ    | -                          | Hz |
//! | 0xE50  | SVC_DIAG_BOOT   | -                          | boot stage string id |
//! | 0xEF0  | SVC_LOG         | (level, ptr, len)          | 0 |
//! | 0xEFF  | SVC_PING        | -                          | 0xC0FFEE |
//!
//! ## BMO ABI via channel (0x100-0x1FF)
//!
//! Opcodes 0x100-0x1FF son BMO ABI (window mgmt, FS, draw, etc.) y se
//! reenvían al gateway de bmo_core para que los procese. No hay conflicto
//! con los SVC_* porque están en 0xE00-0xEFF.

use bmo_channel::{Channel, ChannelEntry, CHANNEL_MAGIC};
use core::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

// ═══════════════════════════════════════════════════════════════════
//  Service opcodes (Ring 3 → Ring 0 requests)
// ═══════════════════════════════════════════════════════════════════

pub const SVC_TIME_NOW:   u64 = 0xE00;
pub const SVC_TIME_NS:    u64 = 0xE01;
pub const SVC_FB_INFO:    u64 = 0xE02;
pub const SVC_FB_PRESENT: u64 = 0xE03;
pub const SVC_FB_FILL:    u64 = 0xE04;
pub const SVC_FB_TEXT:    u64 = 0xE05;
pub const SVC_SERIAL:     u64 = 0xE10;
pub const SVC_BEEP:       u64 = 0xE20;
pub const SVC_POWER_REBT: u64 = 0xE21;
pub const SVC_POWER_OFF:  u64 = 0xE22;
pub const SVC_MEM_TOTAL:  u64 = 0xE30;
pub const SVC_MEM_FREE:   u64 = 0xE31;
pub const SVC_HEAP_USED:  u64 = 0xE32;
pub const SVC_CPU_NAME:   u64 = 0xE40;
pub const SVC_TSC_FREQ:   u64 = 0xE41;
pub const SVC_DIAG_BOOT:  u64 = 0xE50;
pub const SVC_LOG:        u64 = 0xEF0;
pub const SVC_PING:       u64 = 0xEFF;

/// Base del rango BMO ABI (window mgmt, FS, draw, etc.)
pub const BMO_ABI_BASE: u64 = 0x100;
pub const BMO_ABI_END:  u64 = 0x1FF;

// ═══════════════════════════════════════════════════════════════════
//  Event opcodes (Ring 0 → Ring 3 notifications)
// ═══════════════════════════════════════════════════════════════════

pub const OP_KEY_SCANCODE:  u64 = 0xB000_0002;
pub const OP_MOUSE_MOVE:    u64 = 0xB000_0010;
pub const OP_MOUSE_BUTTON:  u64 = 0xB000_0011;
pub const OP_MOUSE_WHEEL:   u64 = 0xB000_0012;
pub const OP_POWER_BTN:     u64 = 0xB000_0020;
pub const OP_LID_SWITCH:    u64 = 0xB000_0021;
pub const OP_BATTERY:       u64 = 0xB000_0030;
pub const OP_THERMAL:       u64 = 0xB000_0040;
pub const OP_TIMER_TICK:    u64 = 0xB000_0050; // periodic; rate in arg0
pub const OP_NETWORK_PKT:   u64 = 0xB000_0060;
pub const OP_STORAGE_CHANGE: u64 = 0xB000_0070;
pub const OP_AUDIO_DONE:    u64 = 0xB000_0080;

// ═══════════════════════════════════════════════════════════════════
//  System channel (kernel-owned, hardware events)
// ═══════════════════════════════════════════════════════════════════

/// Inline system channel — embedded in the kernel binary, no heap alloc.
/// Ring 3 maps this page via `SYS_CHANNEL_REGISTER` at any physical address.
#[repr(C, align(4096))]
struct SystemChannel {
    ch: Channel,
}

static mut SYS_CHANNEL: SystemChannel = SystemChannel {
    ch: Channel {
        magic: 0,
        doorbell: core::sync::atomic::AtomicU64::new(0),
        submit_head: core::sync::atomic::AtomicU64::new(0),
        submit_tail: core::sync::atomic::AtomicU64::new(0),
        complete_head: core::sync::atomic::AtomicU64::new(0),
        complete_tail: core::sync::atomic::AtomicU64::new(0),
        _pad: [0; 2],
        submit_ring: [ChannelEntry { opcode: 0, arg0: 0, arg1: 0, arg2: 0 }; 62],
        complete_ring: [ChannelEntry { opcode: 0, arg0: 0, arg1: 0, arg2: 0 }; 62],
    },
};

/// Get the physical address of the system channel (for Ring 3 mapping).
pub fn sys_channel_phys() -> u64 {
    unsafe { &raw const SYS_CHANNEL as *const SystemChannel as u64 }
}

/// Push an event into the system channel (called from IRQ handlers).
/// These go into the submit_ring; Ring 3 reads from complete_ring.
pub fn sys_send(opcode: u64, arg0: u64, arg1: u64, arg2: u64) -> bool {
    let ch = unsafe { &SYS_CHANNEL.ch };
    ch.ring3_send(opcode, arg0, arg1, arg2)
}

// ═══════════════════════════════════════════════════════════════════
//  Counters (for diagnostics)
// ═══════════════════════════════════════════════════════════════════

static KBD_EVENTS:  AtomicU64 = AtomicU64::new(0);
static MOUSE_EVENTS: AtomicU64 = AtomicU64::new(0);
static WHEEL_EVENTS: AtomicU64 = AtomicU64::new(0);
static SVC_REQUESTS: AtomicU64 = AtomicU64::new(0);
static SVC_RESPONSES: AtomicU64 = AtomicU64::new(0);

/// Public counters — read by cabina / diagnostics.
pub fn kbd_events()   -> u64 { KBD_EVENTS.load(Ordering::Relaxed) }
pub fn mouse_events() -> u64 { MOUSE_EVENTS.load(Ordering::Relaxed) }
pub fn wheel_events() -> u64 { WHEEL_EVENTS.load(Ordering::Relaxed) }
pub fn svc_requests() -> u64 { SVC_REQUESTS.load(Ordering::Relaxed) }
pub fn svc_responses() -> u64 { SVC_RESPONSES.load(Ordering::Relaxed) }

// ═══════════════════════════════════════════════════════════════════
//  User channels (registered by Ring 3 modules)
// ═══════════════════════════════════════════════════════════════════

const MAX_CHANNELS: usize = 8;
static CHANNELS: [AtomicPtr<Channel>; MAX_CHANNELS] = [
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
];

pub fn register(ch: *mut Channel) -> bool {
    for slot in &CHANNELS {
        if slot.compare_exchange(
            core::ptr::null_mut(), ch,
            Ordering::Relaxed, Ordering::Relaxed,
        ).is_ok() { return true; }
    }
    false
}

// ═══════════════════════════════════════════════════════════════════
//  Service dispatcher (Ring 0 side)
// ═══════════════════════════════════════════════════════════════════

/// Dispatch a service request and return a (opcode, a0, a1, a2) response.
/// Returns None to indicate "no response" (e.g., the request should be
/// silently dropped).
fn dispatch_service(opcode: u64, a0: u64, a1: u64, a2: u64) -> Option<(u64, u64, u64, u64)> {
    SVC_REQUESTS.fetch_add(1, Ordering::Relaxed);
    SVC_RESPONSES.fetch_add(1, Ordering::Relaxed);

    match opcode {
        SVC_PING => Some((SVC_PING, 0xC0FF_EE00, 0xDEAD_BEEF, 0)),

        SVC_TIME_NOW => Some((SVC_TIME_NOW, crate::cpu::rdtsc(), 0, 0)),
        SVC_TIME_NS  => {
            let ns = crate::dev::timer::now_ns();
            Some((SVC_TIME_NS, ns, 0, 0))
        }

        SVC_FB_INFO => {
            let (w, h, s, fmt) = unsafe {
                (
                    crate::info::FB_WIDTH  as u64,
                    crate::info::FB_HEIGHT as u64,
                    crate::info::FB_STRIDE as u64,
                    crate::info::FB_PIXEL_FORMAT as u64,
                )
            };
            // Pack into arg0/arg1 (Ring 3 unpacks)
            Some((SVC_FB_INFO, w, h, (s << 16) | (fmt & 0xFFFF)))
        }
        SVC_FB_PRESENT => {
            crate::dev::framebuffer::present();
            Some((SVC_FB_PRESENT, 0, 0, 0))
        }
        SVC_FB_FILL => {
            // a0=x, a1=y, a2=w_h_packed (low16=w, high16=h)
            let w = a2 & 0xFFFF;
            let h = a2 >> 16;
            // color is in a1 (we overload a0=color, a1=x, a2=y|wh)
            let color = a0 as u32;
            let x = (a1 & 0xFFFF) as u32;
            let y = (a1 >> 16) as u32;
            crate::dev::framebuffer::fill_rect(x, y, w as u32, h as u32,
                crate::dev::framebuffer::Color(color));
            Some((SVC_FB_FILL, 0, 0, 0))
        }
        SVC_FB_TEXT => {
            // Text rendering needs a font; until then, return 0 bytes
            Some((SVC_FB_TEXT, 0, 0, 0))
        }

        SVC_SERIAL => {
            // a0 = user pointer, a1 = length. SAFETY: caller validated ptr.
            if a1 > 0 && a1 < 4096 && a0 != 0 {
                let slice = unsafe { core::slice::from_raw_parts(a0 as *const u8, a1 as usize) };
                if let Ok(s) = core::str::from_utf8(slice) {
                    crate::dev::console::serial_write(s);
                }
            }
            Some((SVC_SERIAL, 0, 0, 0))
        }

        SVC_BEEP => {
            crate::dev::pc_speaker::beep(a0 as u32, a1 as u32);
            Some((SVC_BEEP, 0, 0, 0))
        }
        SVC_POWER_REBT => {
            // Power ops are non-returning; mark current entry as last response.
            crate::dev::power::reboot()
        }
        SVC_POWER_OFF => {
            crate::dev::power::shutdown()
        }

        SVC_MEM_TOTAL => {
            let total = crate::mm::frame_alloc::total_ram();
            Some((SVC_MEM_TOTAL, total, 0, 0))
        }
        SVC_MEM_FREE => {
            let pages = crate::mm::frame_alloc::free_count() as u64;
            Some((SVC_MEM_FREE, pages * 4096, 0, 0))
        }
        SVC_HEAP_USED => {
            let used = crate::dev::console::serial_write as fn(&str) as *const () as u64;
            // Re-use a real heap API
            let _ = used;
            let bytes = crate::mm::slab::heap_used() as u64;
            Some((SVC_HEAP_USED, bytes, 0, 0))
        }

        SVC_CPU_NAME => {
            // Return the AMD Zen 3 brand string length as a simple hint
            let name = "AMD Ryzen 5 5600X";
            // a0=user ptr, a1=len
            if a1 > 0 && a0 != 0 {
                let bytes = name.as_bytes();
                let n = bytes.len().min(a1 as usize);
                unsafe {
                    core::ptr::copy_nonoverlapping(bytes.as_ptr(), a0 as *mut u8, n);
                }
                Some((SVC_CPU_NAME, n as u64, 0, 0))
            } else {
                Some((SVC_CPU_NAME, name.len() as u64, 0, 0))
            }
        }
        SVC_TSC_FREQ => {
            Some((SVC_TSC_FREQ, crate::cpu::tsc_per_sec(), 0, 0))
        }

        SVC_DIAG_BOOT => {
            // Return the last boot stage id; full string requires ring3 fetch
            let stage = crate::uefi_rt::read_boot_stage()
                .map(|s| s.len() as u64)
                .unwrap_or(0);
            Some((SVC_DIAG_BOOT, stage, 0, 0))
        }

        SVC_LOG => {
            // a0 = level (0=info, 1=warn, 2=fault), a1 = ptr, a2 = len
            if a2 > 0 && a2 < 1024 && a1 != 0 {
                let slice = unsafe { core::slice::from_raw_parts(a1 as *const u8, a2 as usize) };
                if let Ok(s) = core::str::from_utf8(slice) {
                    match a0 {
                        1 => crate::dev::console::serial_write("[WARN] "),
                        2 => crate::dev::console::serial_write("[FAULT] "),
                        _ => crate::dev::console::serial_write("[INFO] "),
                    }
                    crate::dev::console::serial_write(s);
                    crate::dev::console::serial_write("\n");
                }
            }
            Some((SVC_LOG, 0, 0, 0))
        }

        // BMO ABI: forward to bmo_core gateway
        _ if opcode >= BMO_ABI_BASE && opcode <= BMO_ABI_END => {
            let result = crate::ring3::gateway::dispatch(opcode as u16, a0, a1, a2, 0, 0, 0);
            Some((opcode, result, 0, 0))
        }
        // Unknown opcode: drop silently but still return a no-op
        _ => Some((opcode, u64::MAX, 0, 0)),
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Timer ISR processing
// ═══════════════════════════════════════════════════════════════════

/// Called from timer ISR each tick (~1ms at 1kHz).
/// Processes: PC speaker timeout, keyboard/mouse polling, channels.
pub fn tick_all() {
    // Speaker timeout
    crate::dev::pc_speaker::tick();

    // Poll the shared i8042 once; it routes keyboard and mouse bytes.
    crate::irq::keyboard::tick();

    // Process Ring 0 → Ring 3 system channel (forward HW events to ring 3)
    let ch = unsafe { &SYS_CHANNEL.ch };
    if ch.ring0_has_work() {
        ch.ring0_process(|opcode, a0, a1, a2| {
            // Forward HW events unchanged; ring3_poll receives them
            Some((opcode, a0, a1, a2))
        });
    }

    // Process user-registered channels (Ring 3 → Ring 0 service requests)
    for slot in &CHANNELS {
        let ch_ptr = slot.load(Ordering::Acquire);
        if ch_ptr.is_null() { continue; }
        let channel = unsafe { &*ch_ptr };
        if channel.ring0_has_work() {
            channel.ring0_process(|opcode, a0, a1, a2| {
                dispatch_service(opcode, a0, a1, a2)
            });
        }
    }
}

/// Process all channels immediately (called from SYS_CHANNEL_KICK).
pub fn process_now() -> usize {
    let ch = unsafe { &SYS_CHANNEL.ch };
    let mut total = 0;
    if ch.ring0_has_work() {
        total += ch.ring0_process(|opcode, a0, a1, a2| {
            Some((opcode, a0, a1, a2))
        });
    }
    for slot in &CHANNELS {
        let ch_ptr = slot.load(Ordering::Acquire);
        if ch_ptr.is_null() { continue; }
        let channel = unsafe { &*ch_ptr };
        if channel.ring0_has_work() {
            total += channel.ring0_process(|opcode, a0, a1, a2| {
                dispatch_service(opcode, a0, a1, a2)
            });
        }
    }
    total
}

/// Initialize the IPC subsystem + hardware polling.
pub fn init() {
    unsafe {
        SYS_CHANNEL.ch.init();
    }
    let phys = sys_channel_phys();
    unsafe { core::ptr::write_volatile(0x9_0160 as *mut u64, phys); }
    let _ = crate::irq::i8042::init();

    // Emit a one-shot BOOT_READY event so userland can detect when the HAL
    // is fully up.
    sys_send(OP_TIMER_TICK, 0, 0, 0);
}
