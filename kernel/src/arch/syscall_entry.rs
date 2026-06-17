//! Syscall Entry Point — IA32_LSTAR / STAR / FMASK MSR setup.
//!
//! x86-64 `syscall` instruction:
//!   - Saves RIP in RCX, RFLAGS in R11
//!   - Loads CS from STAR[47:32], SS from STAR[47:32]+8
//!   - Loads RIP from IA32_LSTAR
//!   - Masks RFLAGS with IA32_FMASK
//!
//! Return to Ring 3 uses `iretq` (not `sysretq`) for safety:
//!   - Pops RIP, CS, RFLAGS, RSP, SS from kernel stack
//!   - Handles exceptions properly (sysretq cannot return to faulting state)
//!
//! BMO ABI syscall convention:
//!   RAX = syscall number
//!   RDI, RSI, RDX, R10, R8, R9 = arguments
//!   Return: RAX = result (negative = error)

use core::arch::{asm, naked_asm};

// MSR addresses
const IA32_STAR: u32  = 0xC000_0081;
const IA32_LSTAR: u32 = 0xC000_0082;
const IA32_FMASK: u32 = 0xC000_0084;
const IA32_EFER: u32  = 0xC000_0080;

/// Write a 64-bit value to an MSR.
#[inline]
unsafe fn wrmsr(msr: u32, val: u64) {
    let lo = val as u32;
    let hi = (val >> 32) as u32;
    asm!("wrmsr", in("ecx") msr, in("eax") lo, in("edx") hi, options(nostack));
}

/// Read a 64-bit value from an MSR.
#[inline]
unsafe fn rdmsr(msr: u32) -> u64 {
    let (lo, hi): (u32, u32);
    asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi, options(nostack));
    ((hi as u64) << 32) | (lo as u64)
}

/// Initialize syscall/sysret MSRs.
///
/// Must be called AFTER `init_gdt()` since it references segment selectors.
pub fn init_syscall() {
    unsafe {
        // Enable SCE (System Call Extension) in IA32_EFER
        let efer = rdmsr(IA32_EFER);
        wrmsr(IA32_EFER, efer | 1); // bit 0 = SCE

        // IA32_STAR:
        //   bits 47:32 = kernel CS (for syscall) — CS = STAR[47:32], SS = STAR[47:32]+8
        //   bits 63:48 = user CS base (for sysret) — CS = STAR[63:48]+16, SS = STAR[63:48]+8
        //
        // With our GDT layout:
        //   Kernel CS = 0x08, Kernel SS = 0x10
        //   User CS   = 0x23 (RPL=3), User SS = 0x1B (RPL=3)
        //
        // For syscall: STAR[47:32] = 0x08:
        //   CS = 0x08 (Kernel Code) ✓
        //   SS = 0x08 + 8 = 0x10 (Kernel Data) ✓
        let kernel_base: u64 = 0x08; // STAR[47:32]
        let user_base: u64   = 0x10; // STAR[63:48]
        let star = (user_base << 48) | (kernel_base << 32);
        wrmsr(IA32_STAR, star);

        // IA32_LSTAR = address of syscall entry point
        wrmsr(IA32_LSTAR, syscall_entry_naked as *const () as u64);

        // IA32_FMASK = RFLAGS bits to clear on syscall
        // Clear IF (bit 9) to disable interrupts during syscall entry,
        // and DF (bit 10) for consistent string ops direction.
        wrmsr(IA32_FMASK, (1 << 9) | (1 << 10));
    }
}

/// Saved user context for iretq return.
/// Layout must match the REVERSED push order in syscall_entry_naked.
///
/// Push order (last pushed = lowest address):
///   r15, r14, r13, r12, rbp, rbx, r9, r8, r10, rdx, rsi, rdi, rax,
///   saved_RSP, CS, RIP, RFLAGS, SS
///
/// Stack layout (low to high address after all pushes):
///   [rsp+0]   = r15
///   [rsp+8]   = r14
///   [rsp+16]  = r13
///   [rsp+24]  = r12
///   [rsp+32]  = rbp
///   [rsp+40]  = rbx
///   [rsp+48]  = r9
///   [rsp+56]  = r8
///   [rsp+64]  = r10
///   [rsp+72]  = rdx
///   [rsp+80]  = rsi
///   [rsp+88]  = rdi
///   [rsp+96]  = rax (syscall number)
///   [rsp+104] = saved user RSP
///   [rsp+112] = CS (0x23)
///   [rsp+120] = RIP (user return address)
///   [rsp+128] = RFLAGS
///   [rsp+136] = SS (0x1B)
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct InterruptFrame {
    // General-purpose registers (pushed in reverse order)
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub r9: u64,
    pub r8: u64,
    pub r10: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rax: u64,    // syscall number
    // CPU-saved context for iretq
    pub rsp: u64,    // saved user RSP
    pub cs: u64,     // user code segment (0x23)
    pub rip: u64,    // user return address
    pub rflags: u64, // user flags
    pub ss: u64,     // user data segment (0x1B)
}

/// Kernel stack for syscall entry. Updated by set_syscall_kernel_stack().
#[unsafe(no_mangle)]
static mut SYSCALL_KERNEL_RSP: u64 = 0;

/// Set the kernel stack pointer used by the syscall entry trampoline.
pub fn set_syscall_kernel_stack(rsp: u64) {
    unsafe { SYSCALL_KERNEL_RSP = rsp; }
}

/// Check if any Ring 3 syscall has been seen.
pub fn ring3_alive() -> bool {
    unsafe { RING3_SYSCALL_SEEN }
}

/// Naked syscall entry point — called by hardware via IA32_LSTAR.
///
/// On entry from `syscall`:
///   RCX = saved RIP (user return address)
///   R11 = saved RFLAGS
///   RAX = syscall number
///   RDI, RSI, RDX, R10, R8, R9 = arguments
///   RSP = still user RSP! We must switch to kernel stack.
#[unsafe(naked)]
unsafe extern "C" fn syscall_entry_naked() {
    naked_asm!(
        // ── Save user context for iretq return ──
        // CPU automatically saves: RCX (RIP), R11 (RFLAGS)
        // We need to save: SS, RSP, CS (for iretq frame)

        // Build iretq frame first (pushed first = highest address)
        "push qword ptr [{ss_val}]",   // [rsp+0]  = SS (0x1B)
        "push rsp",                     // [rsp+8]  = placeholder for user RSP (will fix below)
        "add qword ptr [rsp], 8",      //           adjust to point past this push
        "push qword ptr [rsp]",        //           duplicate RSP value
        "push qword ptr [{cs_val}]",   // [rsp+16] = CS (0x23)
        "push rcx",                     // [rsp+24] = RIP (saved by syscall)
        "push r11",                     // [rsp+32] = RFLAGS (saved by syscall)

        // Now save all general-purpose registers (pushed in reverse order)
        "push rax",                     // syscall number
        "push rdi",                     // arg 0
        "push rsi",                     // arg 1
        "push rdx",                     // arg 2
        "push r10",                     // arg 3
        "push r8",                      // arg 4
        "push r9",                      // arg 5
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // Load kernel stack
        "mov r15, rsp",                 // save pointer to saved context
        "mov rsp, [rip + {kstack}]",    // load kernel stack

        // Call Rust handler with pointer to saved context
        "mov rdi, r15",                 // arg 0: pointer to InterruptFrame
        "call {handler}",

        // Restore kernel stack pointer
        "mov rsp, r15",

        // Restore all general-purpose registers
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbp",
        "pop rbx",
        "pop r9",
        "pop r8",
        "pop r10",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop rax",

        // Restore iretq frame (RFLAGS, RIP, CS, RSP, SS)
        "pop r11",                      // RFLAGS
        "pop rcx",                      // RIP
        "add rsp, 8",                   // skip CS (loaded by iretq from stack)
        "pop r11",                      // user RSP (will be loaded into RSP by iretq)
        "pop r10",                      // SS (loaded by iretq from stack)

        // Build proper iretq frame: SS, RSP, RFLAGS, CS, RIP (low to high)
        "push r10",                     // SS
        "push r11",                     // user RSP
        "push r11",                     // RFLAGS (use saved value)
        "push qword ptr [{cs_val}]",   // CS
        "push rcx",                     // RIP

        // Return to Ring 3 via iretq
        "iretq",

        kstack = sym SYSCALL_KERNEL_RSP,
        handler = sym syscall_handler_rust,
        ss_val = const 0x1B_u64,        // USER_DS | RPL=3
        cs_val = const 0x23_u64,        // USER_CS | RPL=3
    );
}

static mut RING3_SYSCALL_SEEN: bool = false;

/// Rust syscall handler — dispatches by syscall number.
///
/// `frame` points to the saved user context on the kernel stack.
/// On return, the frame will be restored and iretq will return to Ring 3.
#[unsafe(no_mangle)]
extern "C" fn syscall_handler_rust(frame: *mut InterruptFrame) {
    unsafe {
        if !RING3_SYSCALL_SEEN {
            RING3_SYSCALL_SEEN = true;
            crate::diag::info("ring3", "first syscall received; Ring 3 is alive");
        }

        let f = &mut *frame;
        let nr = f.rax;
        let a0 = f.rdi;
        let a1 = f.rsi;
        let a2 = f.rdx;
        let a3 = f.r10;
        let a4 = f.r8;
        let _a5 = f.r9; // 7th arg (BMO ABI) — reserved for future use

        crate::diag::trace_u64("syscall", "dispatch nr", nr);

        let result = match nr {
            // ─── Procesos ─────────────────────────────────────────────
            // ProcessExit (0x00) — kill current process and return to scheduler
            0x00 => {
                crate::diag::trace("syscall", "ProcessExit");
                // Kill current process — never returns
                crate::sched::process::kill_current_process(0, a0, 0);
            }

            // ThreadCreate (0x01) — not implemented yet
            0x01 => u64::MAX,

            // ThreadExit (0x02) — not implemented yet
            0x02 => u64::MAX,

            // ThreadYield (0x03)
            0x03 => {
                crate::sched::yield_now();
                0
            }

            // FutexWait (0x04) — not implemented yet
            0x04 => u64::MAX,

            // FutexWake (0x05) — not implemented yet
            0x05 => u64::MAX,

            // ─── Memoria ──────────────────────────────────────────────
            // Mmap (0x10) — not implemented yet
            0x10 => u64::MAX,

            // Munmap (0x11) — not implemented yet
            0x11 => u64::MAX,

            // Mprotect (0x12) — not implemented yet
            0x12 => u64::MAX,

            // ─── VFS ──────────────────────────────────────────────────
            // FileOpen (0x20): a0=name_ptr, a1=name_len → fd or u64::MAX
            0x20 => crate::fs::ramdisk::open(a0, a1),

            // FileRead (0x21): a0=fd, a1=ptr, a2=len → bytes read
            0x21 => crate::fs::ramdisk::read(a0, a1, a2),

            // FileWrite (0x22) — not implemented yet
            0x22 => u64::MAX,

            // FileClose (0x23): a0=fd → 0 or u64::MAX
            0x23 => crate::fs::ramdisk::close(a0),

            // FileSeek (0x24) — not implemented yet
            0x24 => u64::MAX,

            // FileStat (0x25): a0=fd → bytes total
            0x25 => crate::fs::ramdisk::size(a0),

            // ─── IPC ──────────────────────────────────────────────────
            // PortCreate (0x30) — not implemented yet
            0x30 => u64::MAX,

            // PortSend (0x31) — not implemented yet
            0x31 => u64::MAX,

            // PortRecv (0x32) — not implemented yet
            0x32 => u64::MAX,

            // ─── BareX bridges ────────────────────────────────────────
            // BarexGfxSubmit (0x40) — not implemented yet
            0x40 => u64::MAX,

            // BarexAudioSubmit (0x41) — not implemented yet
            0x41 => u64::MAX,

            // BarexInputPoll (0x42) — not implemented yet
            0x42 => u64::MAX,

            // BarexNetSubmit (0x43) — not implemented yet
            0x43 => u64::MAX,

            // ─── Tiempo ───────────────────────────────────────────────
            // ClockGetTime (0x50): returns rdtsc value
            0x50 => crate::arch::cpu::rdtsc(),

            // NanoSleep (0x51): a0 = nanoseconds (busy-wait)
            0x51 => {
                let target_ns = a0;
                let target_cycles = (target_ns as u128 * 37) / 10; // ~3.7 GHz
                let start = crate::arch::cpu::rdtsc();
                while (crate::arch::cpu::rdtsc() - start) < target_cycles as u64 {
                    core::hint::spin_loop();
                }
                0
            }

            // ─── Framebuffer (compositor Ring 3) ──────────────────────
            // FbInfo (0x60): no args, returns packed:
            //   bits  0..31 = width  | bits 32..47 = height | bits 48..63 = stride/4
            0x60 => {
                let w = crate::boot_info::FB_WIDTH as u64;
                let h = crate::boot_info::FB_HEIGHT as u64;
                let s = (crate::boot_info::FB_STRIDE / 1) as u64;
                w | (h << 32) | ((s & 0xFFFF) << 48)
            }

            // FbFill (0x61): a0=x, a1=y, a2=w, a3=h, a4=color (0xAARRGGBB)
            0x61 => {
                crate::desktop::fb_fill(a0 as u32, a1 as u32, a2 as u32, a3 as u32, a4 as u32);
                0
            }

            // FbText (0x62): a0=x, a1=y, a2=ptr_utf8, a3=len, a4=color
            0x62 => {
                if a3 > 0 && a3 < 256 {
                    let slice = core::slice::from_raw_parts(a2 as *const u8, a3 as usize);
                    crate::desktop::fb_text(a0 as u32, a1 as u32, slice, a4 as u32);
                }
                0
            }

            // FbPresent (0x63): no-op (direct framebuffer writes)
            0x63 => 0,

            // FbBlit (0x64): a0=x, a1=y, a2=w, a3=h, a4=src_ptr (XRGB-8888 raster)
            0x64 => {
                crate::desktop::fb_blit(a0 as u32, a1 as u32, a2 as u32, a3 as u32, a4);
                0
            }

            // DesktopFrame (0x65): renderiza un frame completo del escritorio
            0x65 => {
                crate::desktop::render::render_frame();
                crate::diag::paint_overlay();
                crate::desktop::state::STATE.frame
            }

            // ─── Input ────────────────────────────────────────────────
            // KeyPoll (0x70): returns PS/2 scancode or 0 if no key
            0x70 => crate::desktop::poll_key() as u64,

            // MousePoll (0x71): returns x:i16 | y:i16<<16 | buttons<<32
            0x71 => crate::desktop::poll_mouse(),

            // ─── Sonido ───────────────────────────────────────────────
            // Beep (0x80): a0=freq_hz, a1=duration_ms
            0x80 => {
                crate::desktop::beep(a0 as u32, a1 as u32);
                0
            }

            // ─── Security ───────────────────────────────────────────────
            // BD_Scan (0xA0): a0=ptr, a1=len → threat level (0=clean, 1-4=threat)
            0xA0 => {
                if a1 > 0 && a1 < 1024 * 1024 {
                    let data = core::slice::from_raw_parts(a0 as *const u8, a1 as usize);
                    let result = crate::security::bytedefender::scanner::scan_memory(data);
                    result.level as u64
                } else {
                    0
                }
            }

            // BD_Status (0xA1): no args → files_scanned | threats_blocked << 32
            0xA1 => {
                let state = crate::security::bytedefender::state();
                state.files_scanned | (state.threats_blocked << 32)
            }

            // SnapshotCreate (0xA2): a0=label_ptr, a1=label_len → snapshot id
            0xA2 => {
                let label = if a1 > 0 && a1 < 64 {
                    core::slice::from_raw_parts(a0 as *const u8, a1 as usize)
                } else {
                    b"manual"
                };
                crate::security::restaurer::create_snapshot(label, b"User snapshot")
            }

            // SnapshotRollback (0xA3): a0=snapshot_id → 0=ok, 1=error
            0xA3 => {
                if crate::security::restaurer::rollback(a0) { 0 } else { 1 }
            }

            // SnapshotList (0xA4): no args → count of snapshots
            0xA4 => {
                crate::security::restaurer::state().snapshot_count
            }

            // ─── Debug ────────────────────────────────────────────────
            // DebugPrint (0xF0): a0=ptr_utf8, a1=length → serial out
            0xF0 => {
                if a1 > 0 && a1 < 4096 {
                    let slice = core::slice::from_raw_parts(a0 as *const u8, a1 as usize);
                    if let Ok(s) = core::str::from_utf8(slice) {
                        crate::drivers::serial::serial_write(s);
                    }
                }
                0
            }

            // Unknown syscall
            _ => {
                crate::diag::warn_u64("syscall", "unknown syscall", nr);
                u64::MAX
            }
        };

        // Store result in RAX for return to Ring 3
        f.rax = result;
    }
}
