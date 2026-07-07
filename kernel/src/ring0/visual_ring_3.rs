use core::arch::global_asm;

global_asm!(
    ".section .ring3_code, \"ax\"",
    ".globl ring3_entry",
"ring3_entry:",
    "jmp code_start",
"fb_info:",
    ".quad 0",
    ".int 0",
    ".int 0",
    ".int 0",
"code_start:",
    "lea r10, [rip + fb_info]",
    "mov r8,  [r10]",
    "mov r9d, [r10 + 8]",
    "mov r14d,[r10 + 12]",
    "mov r15d,[r10 + 16]",
    "xor r12d, r12d",
"1:  xor r13d, r13d",
"2:  mov eax, r12d",
    "mul r15d",
    "add eax, r13d",
    "shl eax, 2",
    "mov dword ptr [r8 + rax], 0x800080",
    "inc r13d",
    "cmp r13d, r9d",
    "jb 2b",
    "inc r12d",
    "cmp r12d, 10",
    "jb 1b",
    "mov r12d, r14d",
    "sub r12d, 10",
"3:  xor r13d, r13d",
"4:  mov eax, r12d",
    "mul r15d",
    "add eax, r13d",
    "shl eax, 2",
    "mov dword ptr [r8 + rax], 0x800080",
    "inc r13d",
    "cmp r13d, r9d",
    "jb 4b",
    "inc r12d",
    "cmp r12d, r14d",
    "jb 3b",
    "xor r12d, r12d",
"5:  xor r13d, r13d",
"6:  mov eax, r12d",
    "mul r15d",
    "add eax, r13d",
    "shl eax, 2",
    "mov dword ptr [r8 + rax], 0x800080",
    "inc r13d",
    "cmp r13d, 10",
    "jb 6b",
    "inc r12d",
    "cmp r12d, r14d",
    "jb 5b",
    "xor r12d, r12d",
"7:  mov r13d, r9d",
    "sub r13d, 10",
"8:  mov eax, r12d",
    "mul r15d",
    "add eax, r13d",
    "shl eax, 2",
    "mov dword ptr [r8 + rax], 0x800080",
    "inc r13d",
    "cmp r13d, r9d",
    "jb 8b",
    "inc r12d",
    "cmp r12d, r14d",
    "jb 7b",
"9:  pause",
    "jmp 9b",
    ".globl ring3_entry_end",
"ring3_entry_end:",
);

extern "C" {
    static ring3_entry: u8;
    static ring3_entry_end: u8;
}

const PAGE_SIZE: usize = 4096;
const BOOTSTRAP_IDENTITY_LIMIT: u64 = 0x8000_0000;
const STACK_PAGES: usize = 16;

pub fn jump_to_ring3() -> ! {
    let fb_addr   = unsafe { crate::info::FB_ADDR };
    let fb_width  = unsafe { crate::info::FB_WIDTH };
    let fb_height = unsafe { crate::info::FB_HEIGHT };
    let fb_stride = unsafe { crate::info::FB_STRIDE };

    if fb_addr == 0 || fb_width == 0 || fb_height == 0 || fb_stride == 0 {
        loop { unsafe { core::arch::asm!("hlt"); } }
    }

    let code_size = unsafe {
        (&ring3_entry_end as *const u8 as usize)
            - (&ring3_entry as *const u8 as usize)
    };

    use crate::ring0::mm::virt;
    use crate::ring0::mm::phys;

    let code_phys = match unsafe { phys::alloc_pages_contiguous(1) } {
        Some(p) => p,
        None => loop { unsafe { core::arch::asm!("hlt"); } },
    };

    let stack_phys = match unsafe { phys::alloc_pages_contiguous(STACK_PAGES) } {
        Some(p) => p,
        None => loop { unsafe { core::arch::asm!("hlt"); } },
    };

    if code_size > PAGE_SIZE
        || code_phys.saturating_add(PAGE_SIZE as u64) > BOOTSTRAP_IDENTITY_LIMIT
        || stack_phys.saturating_add((STACK_PAGES * PAGE_SIZE) as u64) > BOOTSTRAP_IDENTITY_LIMIT
    {
        loop { unsafe { core::arch::asm!("hlt"); } }
    }

    unsafe {
        // code_phys and stack_phys are identity-mapped by UEFI (low memory,
        // < 2 GB).  Write the ring3 code directly through its identity
        // address — no need for a separate virtual mapping.
        let code_kvirt = code_phys as *mut u8;
        core::ptr::write_bytes(code_kvirt, 0, PAGE_SIZE);
        core::ptr::copy_nonoverlapping(
            &ring3_entry as *const u8,
            code_kvirt,
            code_size,
        );
        code_kvirt.add(2).cast::<u64>().write(fb_addr);
        code_kvirt.add(10).cast::<u32>().write(fb_width);
        code_kvirt.add(14).cast::<u32>().write(fb_height);
        code_kvirt.add(18).cast::<u32>().write(fb_stride);
    }

    unsafe {
        // Mark code, stack and framebuffer as user-accessible in the
        // current (kernel) page table.  All three are identity-mapped,
        // so we use mark_current_identity_user_range which correctly
        // handles huge pages (both 1 GiB PDPT and 2 MiB PD).
        // map_user_range is NOT used — it would fail on 1 GiB huge pages
        // that the UEFI identity mapping may use.
        let _ = virt::mark_current_identity_user_range(code_phys, PAGE_SIZE);
        let _ = virt::mark_current_identity_user_range(stack_phys, STACK_PAGES * PAGE_SIZE);
        let fb_size_bytes = (fb_stride as u64) * (fb_height as u64) * 4;
        let _ = virt::mark_current_identity_user_range(fb_addr, fb_size_bytes as usize);
    }

    // Entry is the identity-mapped code physical address.
    // Stack top is also identity-mapped: stack_phys + stack_size.
    let entry = code_phys;
    let stack_top = stack_phys + (STACK_PAGES * PAGE_SIZE) as u64;

    unsafe {
        core::arch::asm!(
            "push qword ptr {user_ss}",
            "push {stack_top}",
            "push qword ptr 0x202",
            "push qword ptr {user_cs}",
            "push {entry}",
            "iretq",
            user_ss  = const 0x1B_u64,
            user_cs  = const 0x23_u64,
            stack_top = in(reg) stack_top,
            entry    = in(reg) entry,
            options(noreturn),
        );
    }
}
