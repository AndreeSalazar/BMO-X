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
"9:  hlt",
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
const USER_CODE_BASE: u64 = 0x100_0000;
const USER_STACK_BASE: u64 = 0x7FFF_FF00_0000;

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

    use crate::ring0::mm::vmm as virt;
    use crate::ring0::mm::phys;

    let kernel_cr3 = virt::read_cr3();
    let pml4 = match unsafe { virt::create_user_page_table(kernel_cr3) } {
        Some(p) => p,
        None => loop { unsafe { core::arch::asm!("hlt"); } },
    };

    let code_phys = match unsafe { phys::alloc_pages_contiguous(1) } {
        Some(p) => p,
        None => loop { unsafe { core::arch::asm!("hlt"); } },
    };

    let stack_phys = match unsafe { phys::alloc_pages_contiguous(16) } {
        Some(p) => p,
        None => loop { unsafe { core::arch::asm!("hlt"); } },
    };

    if code_size > PAGE_SIZE
        || code_phys.saturating_add(PAGE_SIZE as u64) > BOOTSTRAP_IDENTITY_LIMIT
        || stack_phys.saturating_add((16 * PAGE_SIZE) as u64) > BOOTSTRAP_IDENTITY_LIMIT
    {
        loop { unsafe { core::arch::asm!("hlt"); } }
    }

    unsafe {
        // The high-half direct map is intentionally deferred in the stable
        // LLFree bootstrap. The frame allocator returns low identity-mapped
        // pages here, so populate the temporary Ring3 code page through its
        // identity address instead of HIGH_MEM_BASE + phys.
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

    use virt::flags;
    unsafe {
        let _ = virt::map_user_range(
            pml4, USER_CODE_BASE, code_phys, 1,
            flags::PRESENT | flags::WRITABLE | flags::USER,
        );
        let _ = virt::map_user_range(
            pml4, USER_STACK_BASE, stack_phys, 16,
            flags::PRESENT | flags::WRITABLE | flags::USER,
        );

        let fb_size_bytes = (fb_stride as u64) * (fb_height as u64) * 4;
        let fb_pages = ((fb_size_bytes + PAGE_SIZE as u64 - 1) / PAGE_SIZE as u64) as usize;
        let _ = virt::map_user_range(
            pml4, fb_addr, fb_addr, fb_pages,
            flags::PRESENT | flags::WRITABLE | flags::USER,
        );
    }

    let stack_top = USER_STACK_BASE + 65536;
    unsafe { virt::write_cr3(pml4); }

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
            entry    = in(reg) USER_CODE_BASE,
            options(noreturn),
        );
    }
}
