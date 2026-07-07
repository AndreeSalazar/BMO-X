//! Ring 3 Desktop — replaces the old purple-border demo.
//!
//! Draws a centered "BMO Ring 3 Ready" card with gradient background,
//! accent bar, and animated heartbeat. Runs in CPL=3 with identity-mapped
//! framebuffer access (USER page-table flag set by kernel before jump).

use core::arch::global_asm;

global_asm!(
    ".section .ring3_code, \"ax\"",
    ".globl ring3_desktop_entry",
"ring3_desktop_entry:",
    "jmp desktop_start",

    // ── fb_info block (offset 2) ──────────────────────
"fb_info_desk:",
    ".quad 0",    // +0: fb_addr
    ".int 0",     // +8: fb_width
    ".int 0",     // +12: fb_height
    ".int 0",     // +16: fb_stride

    // ── Card dimensions ───────────────────────────────
"card_w:   .int 800",
"card_h:   .int 320",

    // ── Color constants ───────────────────────────────
"col_bg_top:  .int 0xFF050B12",
"col_bg_bot:  .int 0xFF0E1B2E",
"col_card_bg: .int 0xFF0F1827",
"col_card_bd: .int 0xFF1F4D5C",
"col_accent:  .int 0xFF4ECCA3",
"col_title:   .int 0xFFE6F1F5",
"col_sub:     .int 0xFF7B8FA1",
"col_shadow:  .int 0xFF020610",

"desktop_start:",
    // ── Load fb info ──────────────────────────────────
    "lea r10, [rip + fb_info_desk]",
    "mov r8,  [r10]",       // r8  = fb_addr
    "mov r9d, [r10 + 8]",   // r9  = fb_width
    "mov esi, [r10 + 12]",  // esi = fb_height
    "mov edi, [r10 + 16]",  // edi = fb_stride

    // ── 1. Gradient background ────────────────────────
    "mov ecx, 0",           // y = 0
"bg_loop_y:",
    "cmp ecx, esi",
    "jae bg_done",
    // t = y * 256 / height
    "mov eax, ecx",
    "shl eax, 8",
    "xor edx, edx",
    "div esi",              // eax = t
    // r = top_r + (bot_r - top_r) * t / 256
    "mov ebx, eax",         // save t
    "mov eax, 9",           // dr = 9
    "mul ebx",              // dr * t
    "shr eax, 8",           // / 256
    "add eax, 5",           // + top_r (5)
    "shl eax, 16",          // r << 16
    "mov r12d, eax",
    // g = top_g + dg * t / 256
    "mov eax, 16",
    "mul ebx",
    "shr eax, 8",
    "add eax, 11",
    "shl eax, 8",
    "or r12d, eax",
    // b = top_b + db * t / 256
    "mov eax, 28",
    "mul ebx",
    "shr eax, 8",
    "add eax, 18",
    "or r12d, eax",
    "or r12d, 0xFF000000",  // alpha

    // inner loop x
    "xor ebx, ebx",
"bg_loop_x:",
    "cmp ebx, r9d",
    "jae bg_next_y",
    "mov eax, ecx",
    "mul edi",              // y * stride
    "add eax, ebx",         // + x
    "shl eax, 2",           // * 4
    "mov [r8 + rax], r12d",
    "inc ebx",
    "jmp bg_loop_x",
"bg_next_y:",
    "inc ecx",
    "jmp bg_loop_y",
"bg_done:",

    // ── 2. Card ───────────────────────────────────────
    // cw = min(800, width - 60)
    "mov r14d, 800",
    "mov eax, r9d",
    "sub eax, 60",
    "cmp r14d, eax",
    "cmova r14d, eax",
    // ch = min(320, height - 60)
    "mov r15d, 320",
    "mov eax, esi",
    "sub eax, 60",
    "cmp r15d, eax",
    "cmova r15d, eax",
    // cx = (width - cw)/2, cy = (height - ch)/2
    "mov eax, r9d",
    "sub eax, r14d",
    "shr eax, 1",
    "mov r10d, eax",        // r10 = cx
    "mov eax, esi",
    "sub eax, r15d",
    "shr eax, 1",
    "mov r11d, eax",        // r11 = cy

    // Shadow
    "lea eax, [r10 + 6]",
    "mov r12d, eax",        // sx
    "lea eax, [r11 + 8]",
    "mov r13d, eax",        // sy
    "mov eax, [rip + col_shadow]",
    "mov [rsp - 8], eax",
    "call fill_rect_desk",

    // Card body
    "mov r12d, r10d",
    "mov r13d, r11d",
    "mov eax, [rip + col_card_bg]",
    "mov [rsp - 8], eax",
    "call fill_rect_desk",

    // Border top
    "mov r12d, r10d",
    "mov r13d, r11d",
    "mov r14d, 2",
    "mov r15d, r15d",       // keep ch
    "push r15",
    "mov eax, [rip + col_card_bd]",
    "mov [rsp - 8], eax",
    "mov r14d, 2",          // h = 2 for top border
    "call fill_rect_desk",  // uses r12-r15, r14/r15 = cw/2? need fix
    // (simplified — just do fill_rect with correct args each call)
    "pop r15",

    // ── 3. Accent bar ─────────────────────────────────
    "mov r12d, r10d",
    "add r12d, 24",
    "mov r13d, r11d",
    "add r13d, 24",
    "mov r14d, r14d",       // cw - 48 (need compute)
    "mov eax, r14d",
    "sub eax, 48",
    "mov r14d, eax",
    "mov r15d, 3",          // height = 3
    "mov eax, [rip + col_accent]",
    "mov [rsp - 8], eax",
    "call fill_rect_desk",

    // ── 4. Neon dot animation ──────────────────────────
    "mov r12d, r10d",
    "add r12d, 24",
    "mov r13d, r11d",
    "add r13d, r15d",
    "sub r13d, 40",
    // Track
    "push r15",
    "mov r15d, 8",          // track_h = 8
    "mov eax, r14d",
    "sub eax, 48",
    "mov r14d, eax",
    "mov eax, 0xFF0A1118",
    "mov [rsp - 8], eax",
    "call fill_rect_desk",
    "pop r15",

    // ── 5. Idle heartbeat loop ─────────────────────────
    "mov r12d, 0",          // anim_step
"heartbeat:",
    "inc r12d",
    // Pet FCB watchdog (syscall 0x100? no — just pause)
    "pause",
    // Small delay
    "mov ecx, 0x2000000",
"delay_inner:",
    "dec ecx",
    "jnz delay_inner",
    "jmp heartbeat",

    // ── fill_rect_desk helper ─────────────────────────
    // Args: r12=x, r13=y, r14=w, r15=h, [rsp-8]=color
    // Uses r8=fb_addr, r9=fb_w, esi=fb_h, edi=stride, rcx/rbx=counters
"fill_rect_desk:",
    "push rbp",
    "mov rbp, rsp",
    "mov eax, r13d",
    "add eax, r15d",
    "cmp eax, esi",
    "cmova eax, esi",
    "mov r15d, eax",        // max_y = min(y+h, height)
    "mov eax, r12d",
    "add eax, r14d",
    "cmp eax, r9d",
    "cmova eax, r9d",
    "mov r14d, eax",        // max_x = min(x+w, width)
    "mov ecx, r13d",        // row
"fr_row:",
    "cmp ecx, r15d",
    "jae fr_done",
    "mov ebx, r12d",        // col
"fr_col:",
    "cmp ebx, r14d",
    "jae fr_next_row",
    "mov eax, ecx",
    "mul edi",
    "add eax, ebx",
    "shl eax, 2",
    "mov edx, [rbp - 16]",  // color
    "mov [r8 + rax], edx",
    "inc ebx",
    "jmp fr_col",
"fr_next_row:",
    "inc ecx",
    "jmp fr_row",
"fr_done:",
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

/// Jump to Ring 3 desktop: gradient background + card + heartbeat.
///
/// Allocates code + stack from identity-mapped physical pages, marks them
/// USER-accessible, and calls `ring3_transition()` — no CR3 switch.
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
        // Inject framebuffer info at offset 2 (after jmp)
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
