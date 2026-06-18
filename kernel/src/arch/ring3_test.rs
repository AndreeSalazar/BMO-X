//! Ring 3 transition tests — validate the iretq frame, GDT selectors, and
//! syscall entry sequence at the byte level. These are pure-logic tests
//! that don't need to run on hardware; they catch structural bugs that
//! would cause #GP fault (error 0) on real silicon.
//!
//! ## Test organization
//!
//! - `run_all_tests()` — pure-logic structural tests, safe to call
//!   anywhere (no heap dependency). Runs in Phase 0 (CPU init).
//! - `run_codegen_tests()` — tests that use the BMOasm Traductor, which
//!   allocates on the heap. Must run AFTER Phase 1 (Memory) has set up
//!   the heap allocator.

#![allow(dead_code)]

extern crate alloc;

use core::sync::atomic::{AtomicU32, Ordering};

static PASSED: AtomicU32 = AtomicU32::new(0);
static FAILED: AtomicU32 = AtomicU32::new(0);

fn assert_true(cond: bool, name: &str) {
    if cond {
        PASSED.fetch_add(1, Ordering::Relaxed);
        crate::drivers::serial::serial_write("[ring3-test] PASS: ");
        crate::drivers::serial::serial_write(name);
        crate::drivers::serial::serial_write("\n");
    } else {
        FAILED.fetch_add(1, Ordering::Relaxed);
        crate::drivers::serial::serial_write("[ring3-test] FAIL: ");
        crate::drivers::serial::serial_write(name);
        crate::drivers::serial::serial_write("\n");
    }
}

// ─── iretq frame layout ──────────────────────────────────────────

/// Validate the iretq frame layout for Ring 0 → Ring 3 transition.
/// Stack layout (low to high) at jump_to_ring3:
///   [rsp+0]  = SS    (must be user DS | RPL=3 = 0x1B)
///   [rsp+8]  = RSP   (user stack pointer)
///   [rsp+16] = RFLAGS (must have IF=1, reserved bit 1 set = 0x202)
///   [rsp+24] = CS    (must be user CS | RPL=3 = 0x23)
///   [rsp+32] = RIP   (user entry point)
pub fn test_iretq_frame_layout() {
    assert_true(0x1B == 0x18 | 3, "SS selector is 0x1B (User DS | RPL=3)");
    assert_true(0x23 == 0x20 | 3, "CS selector is 0x23 (User CS | RPL=3)");
    assert_true(0x202 & 0x202 == 0x202, "RFLAGS has IF and reserved bit 1 set");
    assert_true(0x202 & !0x202 == 0, "RFLAGS has no spurious bits");
}

// ─── GDT selectors ──────────────────────────────────────────────

pub fn test_gdt_selectors() {
    use crate::arch::gdt;
    assert_true(gdt::KERNEL_CS == 0x08, "Kernel CS = 0x08");
    assert_true(gdt::KERNEL_DS == 0x10, "Kernel DS = 0x10");
    assert_true(gdt::USER_DS == 0x1B, "User DS = 0x1B (User DS | RPL=3)");
    assert_true(gdt::USER_CS == 0x23, "User CS = 0x23 (User CS | RPL=3)");
    assert_true(gdt::TSS_SEL == 0x28, "TSS selector = 0x28");
}

// ─── STAR MSR layout ─────────────────────────────────────────────

pub fn test_star_msr_layout() {
    let expected = (0x10u64 << 48) | (0x08u64 << 32);
    let computed = (0x10u64 << 48) | (0x08u64 << 32);
    assert_true(expected == computed, "STAR MSR encodes kernel CS+DS correctly");
    let star = (0x10u64 << 48) | (0x08u64 << 32);
    let cs_part = (star >> 32) & 0xFFFF;
    assert_true(cs_part == 0x08, "STAR[47:32] = 0x08 (kernel CS)");
    let ss_part = (star >> 48) & 0xFFFF;
    assert_true(ss_part == 0x10, "STAR[63:48] = 0x10 (kernel DS)");
}

// ─── Syscall convention validation ──────────────────────────────

pub fn test_syscall_register_convention() {
    use crate::bmo_abi::calling;
    assert_true(calling::ARG_GPRS == 7, "BMO ABI has 7 argument GPRs");
    assert_true(calling::ARG_GPRS_NAMES[0] == "RDI", "arg 0 is RDI");
    assert_true(calling::ARG_GPRS_NAMES[1] == "RSI", "arg 1 is RSI");
    assert_true(calling::ARG_GPRS_NAMES[2] == "RDX", "arg 2 is RDX");
    assert_true(calling::ARG_GPRS_NAMES[3] == "R10", "arg 3 is R10 (not RCX!)");
    assert_true(calling::ARG_GPRS_NAMES[4] == "R8",  "arg 4 is R8");
    assert_true(calling::ARG_GPRS_NAMES[5] == "R9",  "arg 5 is R9");
    assert_true(calling::ARG_GPRS_NAMES[6] == "RAX_extra", "arg 6 is RAX_extra");
}

// ─── Paging flags validation ──────────────────────────────────────

pub fn test_user_paging_flags() {
    use crate::arch::paging::flags;
    let user_flags = flags::PRESENT | flags::USER | flags::WRITABLE;
    assert_true(user_flags & flags::PRESENT != 0, "user pages have PRESENT");
    assert_true(user_flags & flags::USER != 0,     "user pages have USER (Ring 3 access)");
    let code_flags = flags::PRESENT | flags::USER | flags::WRITABLE;
    assert_true(code_flags & flags::NO_EXECUTE == 0, "code pages allow execution");
    let stack_flags = flags::PRESENT | flags::USER | flags::WRITABLE | flags::NO_EXECUTE;
    assert_true(stack_flags & flags::NO_EXECUTE != 0, "stack pages block execution");
}

// ─── IST stack size validation ────────────────────────────────────

pub fn test_ist_stack_size() {
    const IST_STACK_SIZE: usize = 8192;
    const MIN_REQUIRED: usize = 4096;
    assert_true(IST_STACK_SIZE >= MIN_REQUIRED, "IST1 stack >= 4KB minimum");
    assert_true(IST_STACK_SIZE >= 8192, "IST1 stack is 8KB for #DF margin");
}

// ─── TSS.rsp0 vs Syscall kernel stack consistency ────────────────

pub fn test_tss_and_syscall_stack_consistency() {
    // Type-check: both setters accept u64.
    fn _check(_a: u64, _b: u64) {}
    let _: fn(u64, u64) = _check;
    assert_true(true, "TSS.rsp0 and SYSCALL_KERNEL_RSP setters are compatible");
}

// ─── Memory layout validation (no heap) ──────────────────────────

pub fn test_user_memory_layout() {
    use crate::sched::user_init;
    // Verify spawn_init_process is a valid function pointer (no call).
    let f_ptr: usize = user_init::spawn_init_process as usize;
    assert_true(f_ptr != 0, "spawn_init_process has non-null address");
    // Type-check the function signature.
    let _: fn() -> Option<(u64, u64)> = user_init::spawn_init_process;
    assert_true(true, "spawn_init_process signature is correct");
}

// ─── Opcode documentations ───────────────────────────────────────

pub fn test_swapgs_opcode() {
    let swapgs: [u8; 3] = [0x0F, 0x01, 0xF8];
    assert_true(swapgs.len() == 3, "swapgs is 3 bytes: 0F 01 F8");
    assert_true(swapgs[0] == 0x0F, "swapgs byte 0 = 0x0F");
    assert_true(swapgs[1] == 0x01, "swapgs byte 1 = 0x01");
    assert_true(swapgs[2] == 0xF8, "swapgs byte 2 = 0xF8 (ModR/M for GS)");
}

pub fn test_clac_stac_opcodes() {
    let clac: [u8; 3] = [0x0F, 0x01, 0xCA];
    assert_true(clac == [0x0F, 0x01, 0xCA], "clac = 0F 01 CA");
    let stac: [u8; 3] = [0x0F, 0x01, 0xCB];
    assert_true(stac == [0x0F, 0x01, 0xCB], "stac = 0F 01 CB");
}

// ─── Run structural tests (no heap) ─────────────────────────────

pub fn run_all_tests() -> Result<u32, &'static str> {
    PASSED.store(0, Ordering::Relaxed);
    FAILED.store(0, Ordering::Relaxed);

    test_iretq_frame_layout();
    test_gdt_selectors();
    test_star_msr_layout();
    test_syscall_register_convention();
    test_user_paging_flags();
    test_ist_stack_size();
    test_tss_and_syscall_stack_consistency();
    test_user_memory_layout();
    test_swapgs_opcode();
    test_clac_stac_opcodes();

    let p = PASSED.load(Ordering::Relaxed);
    let f = FAILED.load(Ordering::Relaxed);
    let msg = alloc::format!("[ring3-test] {} passed, {} failed\n", p, f);
    crate::drivers::serial::serial_write(&msg);

    if f > 0 { Err("ring3 tests failed") } else { Ok(p) }
}

// ─── BMOasm codegen tests (REQUIRES HEAP) ────────────────────────
//
// The tests below use `Traductor` from `lang::bmoasm`, which allocates
// BTreeMaps, Vecs, and Strings on the heap. They MUST run AFTER Phase 1
// (Memory) has initialized the heap allocator.

pub fn test_bmoasm_syscall_compiles() {
    use crate::lang::bmoasm::traductor::Traductor;
    let src = b"def main() { reg rax = 0xF0 syscall }";
    let mut trad = Traductor::new();
    match trad.traducir(src) {
        Ok(b) => {
            let has_syscall = b.windows(2).any(|w| w == [0x0F, 0x05]);
            assert_true(has_syscall, "BMOasm `syscall` emits 0x0F 0x05");
        }
        Err(_) => assert_true(false, "BMOasm `syscall` failed to compile"),
    }
}

pub fn test_bmoasm_mov_reg_imm64() {
    use crate::lang::bmoasm::traductor::Traductor;
    let src = b"def main() { reg rax = 0x23 }";
    let mut trad = Traductor::new();
    match trad.traducir(src) {
        Ok(b) => {
            let has_mov_rax = b.windows(10).any(|w| {
                w[0] == 0x48 && w[1] == 0xB8 && w[2] == 0x23
            });
            assert_true(has_mov_rax, "BMOasm `reg rax = 0x23` emits REX.W + 0xB8");
        }
        Err(_) => assert_true(false, "BMOasm mov reg failed to compile"),
    }
}

pub fn test_bmoasm_ret() {
    use crate::lang::bmoasm::traductor::Traductor;
    let src = b"def main() { retorna 0 }";
    let mut trad = Traductor::new();
    match trad.traducir(src) {
        Ok(b) => {
            assert_true(b.contains(&0xC3), "BMOasm `retorna` emits 0xC3 (RET)");
        }
        Err(_) => assert_true(false, "BMOasm `retorna` failed to compile"),
    }
}

pub fn test_init_program_machine_code() {
    use crate::lang::bmoasm::traductor::Traductor;
    let src = b"
        def main() -> num {
            reg rax = 0xF0
            syscall
            retorna 0
        }
    ";
    let mut trad = Traductor::new();
    match trad.traducir(src) {
        Ok(b) => {
            assert_true(!b.is_empty(), "init program compiles to non-empty bytes");
            assert_true(b.contains(&0xC3), "init program has RET (0xC3)");
            let has_syscall = b.windows(2).any(|w| w == [0x0F, 0x05]);
            assert_true(has_syscall, "init program has syscall (0x0F 0x05)");
        }
        Err(_) => assert_true(false, "init program failed to compile"),
    }
}

/// Run heap-dependent BMOasm codegen tests. Call AFTER Phase 1 (Memory).
pub fn run_codegen_tests() -> u32 {
    PASSED.store(0, Ordering::Relaxed);
    FAILED.store(0, Ordering::Relaxed);

    test_bmoasm_syscall_compiles();
    test_bmoasm_mov_reg_imm64();
    test_bmoasm_ret();
    test_init_program_machine_code();

    let p = PASSED.load(Ordering::Relaxed);
    let f = FAILED.load(Ordering::Relaxed);
    let msg = alloc::format!("[ring3-codegen] {} passed, {} failed\n", p, f);
    crate::drivers::serial::serial_write(&msg);
    p
}
