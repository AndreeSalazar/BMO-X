//! Ring 3 Desktop — animated entry sequence.
//!
//! Visual sequence:
//!   1. Dark gradient background (instant)
//!   2. Card appears centered
//!   3. Accent bar pulses (3x glow)
//!   4. Heartbeat idle loop
//!
//! Runs in CPL=3 with identity-mapped framebuffer (USER page-table flag).

use core::arch::global_asm;

global_asm!(
    ".section .ring3_code, \"ax\"",
    ".globl ring3_desktop_entry",
"ring3_desktop_entry:",
    "jmp desktop_start",

    // fb_info (offset 2): kernel injects real values before jump
"fb_info_desk:",
    ".quad 0",    // +0: fb_addr
    ".int 0",     // +8: fb_width
    ".int 0",     // +12: fb_height
    ".int 0",     // +16: fb_stride

"desktop_start:",
    // Load framebuffer info
    "lea r10, [rip + fb_info_desk]",
    "mov r8,  [r10]",       // fb_addr
    "mov r9d, [r10 + 8]",   // fb_width
    "mov esi, [r10 + 12]",  // fb_height
    "mov edi, [r10 + 16]",  // fb_stride

    // ═══ 1. Gradient background ═══════════════════════════
    "xor ecx, ecx",         // y = 0
"bg_y:",
    "cmp ecx, esi",
    "jae bg_done",
    "mov eax, ecx",
    "shl eax, 8",
    "xor edx, edx",
    "div esi",              // t = y*256/height
    "mov ebx, eax",
    // r = 5 + 9*t/256
    "mov eax, 9",
    "mul ebx",
    "shr eax, 8",
    "add eax, 5",
    "shl eax, 16",
    "mov r12d, eax",
    // g = 11 + 16*t/256
    "mov eax, 16",
    "mul ebx",
    "shr eax, 8",
    "add eax, 11",
    "shl eax, 8",
    "or r12d, eax",
    // b = 18 + 28*t/256
    "mov eax, 28",
    "mul ebx",
    "shr eax, 8",
    "add eax, 18",
    "or r12d, eax",
    "or r12d, 0xFF000000",
    // fill row
    "xor ebx, ebx",
"bg_x:",
    "cmp ebx, r9d",
    "jae bg_next_y",
    "mov eax, ecx",
    "mul edi",
    "add eax, ebx",
    "shl eax, 2",
    "mov [r8 + rax], r12d",
    "inc ebx",
    "jmp bg_x",
"bg_next_y:",
    "inc ecx",
    "jmp bg_y",
"bg_done:",

    // ═══ 2. Card (centered, 900×380) ═════════════════════
    "mov r10d, 900",
    "mov eax, r9d",
    "sub eax, 60",
    "cmp r10d, eax",
    "cmova r10d, eax",     // cw = min(900, w-60)
    "mov r11d, 380",
    "mov eax, esi",
    "sub eax, 60",
    "cmp r11d, eax",
    "cmova r11d, eax",     // ch = min(380, h-60)
    "mov eax, r9d",
    "sub eax, r10d",
    "shr eax, 1",
    "mov r12d, eax",        // cx
    "mov eax, esi",
    "sub eax, r11d",
    "shr eax, 1",
    "mov r13d, eax",        // cy

    // ── Shadow ─────────────────────────────────────
    "push r13","add dword ptr [rsp], 10","push r12","add dword ptr [rsp], 8",
    "push r10","push r11",
    "mov edx, 0xFF020610",
    "call fr",
    "add rsp, 32",

    // ── Card body ──────────────────────────────────
    "push r13","push r12","push r10","push r11",
    "mov edx, 0xFF0F1827",
    "call fr",
    "add rsp, 32",

    // ── Top border ─────────────────────────────────
    "push r13","push r12","push r10","push 2",
    "mov edx, 0xFF1F4D5C",
    "call fr",
    "add rsp, 32",

    // ── Bottom border ──────────────────────────────
    "push r13","add dword ptr [rsp], r11d","sub dword ptr [rsp], 2",
    "push r12","push r10","push 2",
    "mov edx, 0xFF1F4D5C",
    "call fr",
    "add rsp, 32",

    // ── Accent bar (top) ───────────────────────────
    "push r13","add dword ptr [rsp], 30",
    "push r12","add dword ptr [rsp], 24",
    "mov eax, r10d","sub eax, 48","push rax","push 3",
    "mov edx, 0xFF4ECCA3",
    "call fr",
    "add rsp, 32",

    // ── Divider ────────────────────────────────────
    "push r13","add dword ptr [rsp], 150",
    "push r12","add dword ptr [rsp], 60",
    "mov eax, r10d","sub eax, 120","push rax","push 1",
    "mov edx, 0xFF1F4D5C",
    "call fr",
    "add rsp, 32",

    // ═══ 3. Neon pulse (3 glow cycles) ════════════════
    "mov r14d, 3",         // 3 pulses
"pulse_outer:",
    // Bright glow
    "push r13","add dword ptr [rsp], 30",
    "push r12","add dword ptr [rsp], 24",
    "mov eax, r10d","sub eax, 48","push rax","push 3",
    "mov edx, 0xFF4ECCA3",
    "call fr",
    "add rsp, 32",
    // Delay
    "mov ecx, 0x3000000",
"wait1:","dec ecx","jnz wait1",
    // Dim
    "push r13","add dword ptr [rsp], 30",
    "push r12","add dword ptr [rsp], 24",
    "mov eax, r10d","sub eax, 48","push rax","push 3",
    "mov edx, 0xFF147A4D",
    "call fr",
    "add rsp, 32",
    "mov ecx, 0x3000000",
"wait2:","dec ecx","jnz wait2",
    "dec r14d",
    "jnz pulse_outer",
    // Final accent
    "push r13","add dword ptr [rsp], 30",
    "push r12","add dword ptr [rsp], 24",
    "mov eax, r10d","sub eax, 48","push rax","push 3",
    "mov edx, 0xFF4ECCA3",
    "call fr",
    "add rsp, 32",

    // ═══ 4. Heartbeat idle ═════════════════════════════
    "xor ebx, ebx",        // phase counter
"hb_loop:",
    "inc ebx",
    // Track
    "mov eax, r13d","add eax, r11d","sub eax, 48",
    "push rax","mov eax, r12d","add eax, 60",
    "push rax","mov eax, r10d","sub eax, 120","push rax","push 8",
    "mov edx, 0xFF0A1018",
    "call fr",
    "add rsp, 32",
    // Moving dot
    "mov ecx, ebx","shl ecx, 1",
    "mov eax, r13d","add eax, r11d","sub eax, 48",
    "push rax","mov eax, r12d","add eax, 62","add eax, ecx",
    "push rax","push 14","push 8",
    "mov edx, 0xFF39FF14",
    "call fr",
    "add rsp, 32",
    // Delay
    "mov ecx, 0x800000",
"hb_wait:","dec ecx","jnz hb_wait",
    "jmp hb_loop",

    // ══════════ fill_rect helper ════════════════════
    // Args (pushed before call, bottom-to-top): y, x, w, h
    // Color in edx. Preserves r10-r15. Uses r8=fb, r9d=fb_w, esi=fb_h, edi=stride.
    // Stack layout: [rbp+16]=h, [rbp+24]=w, [rbp+32]=x, [rbp+40]=y
"fr:",
    "push rbp",
    "mov rbp, rsp",
    "sub rsp, 16",                 // locals: [rbp-8]=max_x, [rbp-16]=max_y
    // max_y = min(y+h, fb_h)
    "mov eax, [rbp + 40]",         // y
    "mov ecx, [rbp + 16]",         // h
    "add eax, ecx",
    "cmp eax, esi",
    "cmova eax, esi",
    "mov [rbp - 16], eax",         // local max_y
    // max_x = min(x+w, fb_w)
    "mov eax, [rbp + 32]",         // x
    "mov ecx, [rbp + 24]",         // w
    "add eax, ecx",
    "cmp eax, r9d",
    "cmova eax, r9d",
    "mov [rbp - 8], eax",          // local max_x
    // row loop
    "mov ecx, [rbp + 40]",         // row = y
"fr_row:",
    "cmp ecx, [rbp - 16]",
    "jae fr_done",
    "mov ebx, [rbp + 32]",         // col = x
"fr_col:",
    "cmp ebx, [rbp - 8]",
    "jae fr_next",
    "mov eax, ecx",
    "imul eax, edi",               // y * stride (imul preserves edx!)
    "add eax, ebx",               // + x
    "shl eax, 2",                 // * 4
    "mov [r8 + rax], edx",        // *fb = color (edx still valid!)
    "inc ebx",
    "jmp fr_col",
"fr_next:",
    "inc ecx",
    "jmp fr_row",
"fr_done:",
    "add rsp, 16",                 // free locals
    "pop rbp",
    "ret",

    ".globl ring3_desktop_end",
"ring3_desktop_end:",
);

extern "C" {
    static ring3_desktop_entry: u8;
    static ring3_desktop_end: u8;
}

const PAGE_SIZE: usize = 4096;
const BOOTSTRAP_IDENTITY_LIMIT: u64 = 0x8000_0000;
const STACK_PAGES: usize = 16;

/// Jump to Ring 3 desktop: gradient + card + neon pulse + heartbeat.
///
/// Allocates code + stack from identity-mapped physical pages, marks them
/// USER-accessible, and calls ring3_transition() — no CR3 switch.
pub fn enter() -> ! {
    let fb_addr   = unsafe { crate::info::FB_ADDR };
    let fb_width  = unsafe { crate::info::FB_WIDTH };
    let fb_height = unsafe { crate::info::FB_HEIGHT };
    let fb_stride = unsafe { crate::info::FB_STRIDE };

    if fb_addr == 0 || fb_width == 0 || fb_height == 0 || fb_stride == 0 {
        loop { unsafe { core::arch::asm!("pause"); } }
    }

    let code_size = unsafe {
        (&ring3_desktop_end as *const u8 as usize)
            - (&ring3_desktop_entry as *const u8 as usize)
    };

    use crate::ring0::mm::virt;
    use crate::ring0::mm::phys;

    let code_phys = match unsafe { phys::alloc_pages_contiguous(1) } {
        Some(p) => p,
        None => loop { unsafe { core::arch::asm!("pause"); } },
    };

    let stack_phys = match unsafe { phys::alloc_pages_contiguous(STACK_PAGES) } {
        Some(p) => p,
        None => loop { unsafe { core::arch::asm!("pause"); } },
    };

    if code_size > PAGE_SIZE
        || code_phys.saturating_add(PAGE_SIZE as u64) > BOOTSTRAP_IDENTITY_LIMIT
        || stack_phys.saturating_add((STACK_PAGES * PAGE_SIZE) as u64) > BOOTSTRAP_IDENTITY_LIMIT
    {
        loop { unsafe { core::arch::asm!("pause"); } }
    }

    unsafe {
        let code_kvirt = code_phys as *mut u8;
        core::ptr::write_bytes(code_kvirt, 0, PAGE_SIZE);
        core::ptr::copy_nonoverlapping(
            &ring3_desktop_entry as *const u8,
            code_kvirt,
            code_size,
        );
        code_kvirt.add(2).cast::<u64>().write(fb_addr);
        code_kvirt.add(10).cast::<u32>().write(fb_width);
        code_kvirt.add(14).cast::<u32>().write(fb_height);
        code_kvirt.add(18).cast::<u32>().write(fb_stride);
    }

    unsafe {
        let _ = virt::mark_current_identity_user_range(code_phys, PAGE_SIZE);
        let _ = virt::mark_current_identity_user_range(stack_phys, STACK_PAGES * PAGE_SIZE);
        let fb_size_bytes = (fb_stride as u64) * (fb_height as u64) * 4;
        let _ = virt::mark_current_identity_user_range(fb_addr, fb_size_bytes as usize);
    }

    let entry = code_phys;
    let stack_top = stack_phys + (STACK_PAGES * PAGE_SIZE) as u64;

    unsafe { crate::ring3::transition::ring3_transition(entry, stack_top); }
}
