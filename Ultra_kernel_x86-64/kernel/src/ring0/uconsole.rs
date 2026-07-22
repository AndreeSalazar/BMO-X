//! Ring 3 bootstrap console.
//!
//! The frozen syscall surface exposes no "print" call — text output is a
//! capability operation. Until a real display-server estuary exists, the
//! first Ring 3 program needs *some* auditable door to prove the CPL3→CPL0
//! path visually. `INVOKE(CURRENT_TASK, CONSOLE_WRITE, packed)` is that door
//! (see `syscall::invoke_current_task`): the kernel receives up to 8 bytes
//! packed little-endian per call and renders them here.
//!
//! Semantics: bytes accumulate into a line buffer; `\n` (or a full buffer)
//! flushes the line to serial and the on-screen kernel-log panel, tagged so
//! it is unmistakably Ring 3 output. There is no way for the caller to reach
//! any memory but this kernel-owned surface — it hands over bytes by value,
//! never a pointer.
//!
//! Single-core, and syscall dispatch runs with interrupts masked, so the
//! line buffer needs no lock: a CONSOLE_WRITE cannot be preempted mid-flush.

use crate::ring0::dev::console::serial_write_byte;

const LINE_MAX: usize = 96;

static mut LINE: [u8; LINE_MAX] = [0u8; LINE_MAX];
static mut LEN: usize = 0;

// Telemetry for the live dashboard heartbeat: how many CONSOLE_WRITE words
// arrived from CPL3 and how many lines were flushed to the log. Nonzero rx
// = the syscall path from Ring 3 has fired at least once.
static mut RX_WORDS: u64 = 0;
static mut FLUSHED: u64 = 0;

/// `(words_received, lines_flushed)` since boot.
pub fn stats() -> (u64, u64) {
    unsafe { (RX_WORDS, FLUSHED) }
}

/// Emit up to 8 bytes packed little-endian in `packed`. A zero byte ends the
/// word early (lets a short final chunk be zero-padded by the producer).
pub fn write_packed(packed: u64) {
    unsafe { RX_WORDS += 1 };
    let mut i = 0;
    while i < 8 {
        let b = ((packed >> (i * 8)) & 0xFF) as u8;
        if b == 0 {
            break;
        }
        push(b);
        i += 1;
    }
}

fn push(b: u8) {
    // Everything the caller emits also goes to serial verbatim, so a headless
    // boot still shows the Ring 3 output.
    serial_write_byte(b);
    if b == b'\n' {
        flush();
        return;
    }
    // Non-printable bytes are dropped from the framebuffer line (the FONT16
    // grid is ASCII-only) but were already echoed to serial above.
    if b >= 0x20 && b < 0x7f {
        unsafe {
            if LEN < LINE_MAX {
                LINE[LEN] = b;
                LEN += 1;
            } else {
                // Overflow: flush what we have and keep going on a fresh line.
                flush();
                LINE[0] = b;
                LEN = 1;
            }
        }
    }
}

fn flush() {
    unsafe { FLUSHED += 1 };
    let len = unsafe { LEN };
    // Tag the line so it reads as Ring 3 output in the shared kernel log.
    let mut tagged = [0u8; 8 + LINE_MAX];
    let tag = b"ring3> ";
    tagged[..tag.len()].copy_from_slice(tag);
    let body = unsafe { &LINE[..len] };
    tagged[tag.len()..tag.len() + len].copy_from_slice(body);
    if let Ok(s) = core::str::from_utf8(&tagged[..tag.len() + len]) {
        // Paint under the KERNEL CR3. This flush runs inside the syscall
        // dispatch of a Ring 3 caller, i.e. under the USER CR3 — whose
        // address space shares kernel identity only for 0..1 GiB (its PDPT
        // slots 1..3 hold the user image/stack/channels). The GOP
        // framebuffer sits at ~3.5 GiB identity, unmapped there: the first
        // pixel would #PF, and the fault reporter paints too → recursive
        // #PF on IST1 → the silent total freeze. Kernel code and stacks are
        // shared in every address space, so the switch is safe.
        let cur = crate::ring0::mm::vmm::read_cr3();
        let kpml4 = crate::ring0::mm::vmm::kernel_pml4();
        if cur != kpml4 {
            crate::ring0::mm::vmm::switch_to(kpml4);
        }
        crate::ring0::core::phase::dashboard_log(s);
        if cur != kpml4 {
            crate::ring0::mm::vmm::switch_to(cur);
        }
    }
    unsafe { LEN = 0 };
}
