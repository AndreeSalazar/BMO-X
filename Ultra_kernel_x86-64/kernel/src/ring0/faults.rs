//! On-screen CPU fault reporter.
//!
//! The faggin `s1_cpu` stage installs exception handlers that print to COM1
//! serial and halt. On a headless machine (no serial cable) a Ring 3 fault
//! therefore freezes the display with no clue why. This module patches the
//! live IDT (`ctx.idt_ptr`, same table `timer::init` patches for vector 48)
//! so the most common faults paint their vector / error code / faulting RIP
//! / CR2 into the dashboard log before halting — making a CPL3 crash visible.
//!
//! The handlers are terminal: they gather state, draw, and `hlt` forever. No
//! register save/restore is needed because control never returns.

use boot_context::BootContext;
use core::arch::naked_asm;

use crate::ring0::dev::console::serial_write;

const KERNEL_CS: u16 = 0x08;

#[repr(C, packed)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    fn trap_gate(handler: u64, ist: u8) -> Self {
        Self {
            offset_low: handler as u16,
            selector: KERNEL_CS,
            ist,
            attributes: 0x8E, // present, DPL0, interrupt gate
            offset_mid: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }
}

// One naked stub per vector. Error-code vectors read the code the CPU pushed;
// no-error vectors report 0. All read CR2 (only meaningful for #PF but
// harmless otherwise). Then tail into the Rust reporter and halt.
macro_rules! err_stub {
    ($name:ident, $vec:expr) => {
        #[unsafe(naked)]
        unsafe extern "C" fn $name() -> ! {
            naked_asm!(
                "mov rdi, {v}",        // vector
                "mov rsi, [rsp]",      // error code (CPU-pushed)
                "mov rdx, [rsp + 8]",  // faulting RIP
                "mov rcx, cr2",        // fault address
                "mov r8, [rsp + 32]",  // faulting RSP (err: err,rip,cs,rfl,RSP,ss)
                "call {h}",
                "cli",
                "2: hlt",
                "jmp 2b",
                v = const $vec,
                h = sym fault_report,
            );
        }
    };
}

macro_rules! noerr_stub {
    ($name:ident, $vec:expr) => {
        #[unsafe(naked)]
        unsafe extern "C" fn $name() -> ! {
            naked_asm!(
                "mov rdi, {v}",        // vector
                "xor esi, esi",        // no error code
                "mov rdx, [rsp]",      // faulting RIP
                "mov rcx, cr2",        // fault address
                "mov r8, [rsp + 24]",  // faulting RSP (no-err: rip,cs,rfl,RSP,ss)
                "call {h}",
                "cli",
                "2: hlt",
                "jmp 2b",
                v = const $vec,
                h = sym fault_report,
            );
        }
    };
}

noerr_stub!(stub_ud, 6); // #UD invalid opcode
err_stub!(stub_df, 8); //  #DF double fault
err_stub!(stub_gp, 13); // #GP general protection
err_stub!(stub_pf, 14); // #PF page fault

/// Small fixed-capacity line builder (no alloc, exception-context safe).
struct Line {
    b: [u8; 80],
    n: usize,
}

impl Line {
    fn new() -> Self {
        Self { b: [0; 80], n: 0 }
    }
    fn s(&mut self, s: &str) {
        for &c in s.as_bytes() {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
    }
    fn hex(&mut self, mut v: u64, digits: usize) {
        const H: &[u8; 16] = b"0123456789ABCDEF";
        let mut tmp = [0u8; 16];
        for i in 0..digits {
            tmp[digits - 1 - i] = H[(v & 0xF) as usize];
            v >>= 4;
        }
        for i in 0..digits {
            if self.n < self.b.len() {
                self.b[self.n] = tmp[i];
                self.n += 1;
            }
        }
    }
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.b[..self.n]).unwrap_or("")
    }
}

fn paint(row: usize, msg: &str) {
    serial_write("[fault] ");
    serial_write(msg);
    serial_write("\n");
    if crate::info::has_fb() {
        crate::ring0::core::splash::splash_dashboard_log(row, msg);
    }
}

/// Terminal fault reporter. Draws to the top of the dashboard log (rows that
/// stay visible) so a Ring 3 crash is unmistakable instead of a silent hang.
extern "C" fn fault_report(vector: u64, error: u64, rip: u64, cr2: u64, fault_rsp: u64) {
    // Terminal reporter: force the KERNEL CR3 before painting. A fault taken
    // under a user CR3 (which does not map the framebuffer identity range)
    // would otherwise #PF on the first pixel and recurse through this very
    // handler forever — a frozen screen instead of a report. We never
    // return, so no need to restore. Guard: before `vmm::init` captures the
    // boot CR3, kernel_pml4() is 0 — loading that would triple-fault.
    let kpml4 = crate::ring0::mm::vmm::kernel_pml4();
    if kpml4 != 0 {
        crate::ring0::mm::vmm::switch_to(kpml4);
    }
    let name = match vector {
        6 => "#UD invalid-op",
        8 => "#DF double-fault",
        13 => "#GP protection",
        14 => "#PF page-fault",
        _ => "#?? exception",
    };
    let mut l0 = Line::new();
    l0.s("*** CPU FAULT ");
    l0.s(name);
    paint(0, l0.as_str());

    let mut l1 = Line::new();
    l1.s("vec=0x");
    l1.hex(vector, 2);
    l1.s(" err=0x");
    l1.hex(error, 8);
    paint(1, l1.as_str());

    let mut l2 = Line::new();
    l2.s("rip=0x");
    l2.hex(rip, 16);
    paint(2, l2.as_str());

    let mut l3 = Line::new();
    l3.s("cr2=0x");
    l3.hex(cr2, 16);
    paint(3, l3.as_str());

    // ALWAYS show the faulting RSP: it is the stack the faulting instruction
    // ran on. For the timer iretq entering CPL3 this should be the user task's
    // high-mem kernel stack; if it is something else, our model is wrong and
    // this number says exactly where the CPU actually was.
    let mut l4 = Line::new();
    l4.s("flt_rsp=0x");
    l4.hex(fault_rsp, 16);
    paint(10, l4.as_str());

    // Last user-task switch (see scheduler::SWITCH_SNAP): the context the
    // scheduler handed over, its back-pointer AT the switch instant, and the
    // same slot re-read NOW under the fault CR3. b valid + n zero ⇒ content
    // clobbered between switch and epilogue; b zero ⇒ handed over already
    // dead; b==n==valid ⇒ the epilogue never used this context at all.
    let snap = crate::ring0::scheduler::switch_snap();
    let live = if snap[0] != 0 {
        unsafe { ((snap[0] + 512) as *const u64).read_volatile() }
    } else {
        0
    };
    let mut l7 = Line::new();
    l7.s("sw");
    l7.hex(snap[3], 2);
    l7.s(" c=");
    l7.hex(snap[0], 12);
    l7.s(" b=");
    l7.hex(snap[1], 12);
    l7.s(" n=");
    l7.hex(live, 12);
    paint(13, l7.as_str());

    // GS split-brain check: the two GS MSRs vs. the PerCpu static address
    // they are supposed to hold, and the tick count at death. If `b` (live
    // GS_BASE) differs from `s` (&PER_CPUS[0]), some path moved GS after
    // init_bsp — the trap stubs then publish the context somewhere the
    // dispatcher never reads, which restores address 0 (rsp 0x78, cs=0).
    let (gsb, kgs, pcaddr) = crate::ring0::percpu::gs_diag();
    let mut l8 = Line::new();
    l8.s("gs b=");
    l8.hex(gsb, 12);
    l8.s(" k=");
    l8.hex(kgs, 12);
    paint(11, l8.as_str());
    let mut l9 = Line::new();
    l9.s("pc s=");
    l9.hex(pcaddr, 12);
    l9.s(" tk=");
    l9.hex(crate::ring0::timer::ticks(), 4);
    paint(12, l9.as_str());

    // If that RSP is in a plausibly-mapped range, read the 5 iretq operands
    // (rip,cs,rflags,rsp,ss) sitting there — the LIVE values the CPU tried to
    // load. Compare against the fabricated frame (rows 5-6): match ⇒ the
    // target/CR3 is at fault; garbage ⇒ the scheduler handed a bad context.
    let mapped = fault_rsp >= 0xFFFF_8000_0000_0000
        || (fault_rsp >= 0x1000 && fault_rsp < 0x1_0000_0000);
    if mapped {
        let p = fault_rsp as *const u64;
        let (irip, ics, irfl, irsp, iss) = unsafe {
            (
                p.read_volatile(),
                p.add(1).read_volatile(),
                p.add(2).read_volatile(),
                p.add(3).read_volatile(),
                p.add(4).read_volatile(),
            )
        };
        let mut l5 = Line::new();
        l5.s("iq rip=");
        l5.hex(irip, 12);
        l5.s(" cs=");
        l5.hex(ics, 4);
        l5.s(" ss=");
        l5.hex(iss, 4);
        paint(11, l5.as_str());
        let mut l6 = Line::new();
        l6.s("iq rsp=");
        l6.hex(irsp, 12);
        l6.s(" rfl=");
        l6.hex(irfl, 6);
        paint(12, l6.as_str());
    }
}

/// Patch the live IDT so #UD/#DF/#GP/#PF report on screen. Uses IST1 (set up
/// by the faggin TSS) so a fault on a bad stack still lands somewhere sane.
pub fn init(ctx: &BootContext) {
    if ctx.idt_ptr == 0 {
        return;
    }
    let idt = ctx.idt_ptr as *mut IdtEntry;
    unsafe {
        idt.add(6)
            .write_volatile(IdtEntry::trap_gate(stub_ud as *const () as u64, 1));
        idt.add(8)
            .write_volatile(IdtEntry::trap_gate(stub_df as *const () as u64, 1));
        idt.add(13)
            .write_volatile(IdtEntry::trap_gate(stub_gp as *const () as u64, 1));
        idt.add(14)
            .write_volatile(IdtEntry::trap_gate(stub_pf as *const () as u64, 1));
    }
    serial_write("[fault] on-screen exception reporter armed (#UD/#DF/#GP/#PF)\n");
}
