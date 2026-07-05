use crate::bmo_core::bef::parsers::Image;

/// Spawn a process from a parsed Image (ELF/BEF).
///
/// Maps each section into user space at its virtual address, sets up a
/// user stack, creates a Task, and marks the process for Linux emulation
/// if the image was devoured from an ELF binary.
pub fn spawn_from_image(img: &Image) {
    crate::dev::console::serial_write("[bmo_core::proc] spawn_from_image: starting\n");

    let proc = match crate::proc::process::alloc_process() {
        Some(p) => p,
        None => {
            crate::dev::console::serial_write("[bmo_core::proc] spawn_from_image: no process slots\n");
            return;
        }
    };

    match img.format {
        crate::bmo_core::bef::parsers::BinaryFormat::ElfDevoured => {
            proc.set_name("elf.bef");
            proc.linux_emulation = true;
        }
        crate::bmo_core::bef::parsers::BinaryFormat::BefNative => {
            proc.set_name("bef.bef");
        }
    }

    let pid = proc.pid;

    // Create user address space
    let kernel_cr3 = crate::mm::virt::read_cr3();
    let pml4_paddr = match unsafe { crate::mm::virt::create_user_page_table(kernel_cr3) } {
        Some(p) => p,
        None => {
            crate::dev::console::serial_write("[bmo_core::proc] spawn_from_image: can't create user page table\n");
            crate::proc::process::free_process(proc);
            return;
        }
    };
    proc.page_table_root = pml4_paddr;

    // Map each section into user space
    use crate::mm::virt::flags;
    for section in &img.sections {
        if section.size == 0 {
            continue;
        }
        let pages = ((section.size + 4095) / 4096) as usize;
        if pages == 0 {
            continue;
        }

        let section_paddr = match unsafe { crate::mm::phys::alloc_pages_contiguous(pages) } {
            Some(p) => p,
            None => {
                crate::dev::console::serial_write("[bmo_core::proc] spawn_from_image: no phys for section\n");
                crate::proc::process::free_process(proc);
                return;
            }
        };

        // Copy section data from kernel heap to physical pages
        if section.data_ptr != 0 {
            let src = unsafe {
                core::slice::from_raw_parts(
                    section.data_ptr as *const u8,
                    core::cmp::min(section.size as usize, pages * 4096),
                )
            };
            for page_i in 0..pages {
                let page_vaddr = crate::mm::virt::phys_to_virt(section_paddr + (page_i as u64) * 4096);
                let copy_start = page_i * 4096;
                let copy_end = core::cmp::min(copy_start + 4096, section.size as usize);
                let count = copy_end - copy_start;
                if count > 0 {
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            src.as_ptr().add(copy_start),
                            page_vaddr as *mut u8,
                            count,
                        );
                    }
                }
            }
        }

        // Determine page table flags
        let page_flags = flags::PRESENT | flags::USER
            | if (section.flags & 0x2) != 0 { flags::WRITABLE } else { 0 }
            | if (section.flags & 0x4) == 0 { flags::NO_EXECUTE } else { 0 };

        if let Err(e) = unsafe {
            crate::mm::virt::map_user_range(
                pml4_paddr,
                section.virt_addr,
                section_paddr,
                pages,
                page_flags,
            )
        } {
            crate::dev::console::serial_write("[bmo_core::proc] spawn_from_image: map section failed: ");
            crate::dev::console::serial_write(e);
            crate::dev::console::serial_write("\n");
            crate::proc::process::free_process(proc);
            return;
        }
    }

    // Map a user stack (64 KB at typical ELF stack address)
    let stack_base: u64 = 0x7FFF_0000;
    let stack_size: u64 = 65536;
    let stack_pages = (stack_size as usize + 4095) / 4096;
    let stack_paddr = match unsafe { crate::mm::phys::alloc_pages_contiguous(stack_pages) } {
        Some(p) => p,
        None => {
            crate::dev::console::serial_write("[bmo_core::proc] spawn_from_image: no phys for stack\n");
            crate::proc::process::free_process(proc);
            return;
        }
    };

    let stack_page_flags = flags::PRESENT | flags::WRITABLE | flags::USER | flags::NO_EXECUTE;
    if let Err(e) = unsafe {
        crate::mm::virt::map_user_range(
            pml4_paddr,
            stack_base,
            stack_paddr,
            stack_pages,
            stack_page_flags,
        )
    } {
        crate::dev::console::serial_write("[bmo_core::proc] spawn_from_image: map stack failed: ");
        crate::dev::console::serial_write(e);
        crate::dev::console::serial_write("\n");
        crate::proc::process::free_process(proc);
        return;
    }

    proc.user_code_base = img.entry_point;
    proc.user_code_size = img.sections.iter().map(|s| s.size as usize).sum();
    proc.user_stack_base = stack_base;
    proc.user_stack_size = stack_size as usize;
    proc.entry_point = img.entry_point;

    // Allocate kernel stack + create Task
    let kernel_stack = unsafe {
        let layout = core::alloc::Layout::from_size_align(8192, 16).unwrap();
        let ptr = alloc::alloc::alloc(layout);
        if ptr.is_null() {
            crate::dev::console::serial_write("[bmo_core::proc] spawn_from_image: no heap for kernel stack\n");
            crate::proc::process::free_process(proc);
            return;
        }
        ptr as u64 + 8192
    };

    let task = match crate::proc::task::alloc(pid, crate::proc::Priority::Interactive) {
        Some(t) => t,
        None => {
            crate::dev::console::serial_write("[bmo_core::proc] spawn_from_image: no task slots\n");
            crate::proc::process::free_process(proc);
            return;
        }
    };
    task.kernel_stack_top = kernel_stack;
    task.regs = crate::proc::task::SavedRegs::new_user(img.entry_point, stack_base + stack_size);

    crate::dev::console::serial_write("[bmo_core::proc] spawn_from_image: DONE\n");
    crate::cabina::info_u64("bmo_core.proc", "spawned pid from image", pid.0 as u64);
}

pub fn spawn_hello() {
    crate::dev::console::serial_write("[bmo_core::proc] spawn_hello: starting\n");

    // 1. Allocate a Process slot (Ring 0 primitive)
    let proc = match crate::proc::process::alloc_process() {
        Some(p) => p,
        None => {
            crate::dev::console::serial_write("[bmo_core::proc] spawn_hello: no process slots\n");
            return;
        }
    };
    proc.set_name("hello.bef");
    let pid = proc.pid;

    // 2. Create user address space (clones kernel upper half)
    let kernel_cr3 = crate::mm::virt::read_cr3();
    let pml4_paddr = match unsafe { crate::mm::virt::create_user_page_table(kernel_cr3) } {
        Some(p) => p,
        None => {
            crate::dev::console::serial_write("[bmo_core::proc] spawn_hello: can't create user page table\n");
            crate::proc::process::free_process(proc);
            return;
        }
    };
    proc.page_table_root = pml4_paddr;

    // 3. Allocate + map user code page (4KB)
    let code_paddr = match unsafe { crate::mm::phys::alloc_pages_contiguous(1) } {
        Some(p) => p,
        None => {
            crate::dev::console::serial_write("[bmo_core::proc] spawn_hello: no phys for code\n");
            crate::proc::process::free_process(proc);
            return;
        }
    };
    unsafe {
        let code_vaddr = crate::mm::virt::phys_to_virt(code_paddr);
        core::ptr::write_bytes(code_vaddr as *mut u8, 0xF4, 4096); // HLT sled
    }

    let user_code_base: u64 = 0x400_000;
    proc.user_code_base = user_code_base;
    proc.user_code_size = 4096;

    use crate::mm::virt::flags;
    let page_flags = flags::PRESENT | flags::WRITABLE | flags::USER | flags::NO_EXECUTE;
    if let Err(e) = unsafe {
        crate::mm::virt::map_user_range(pml4_paddr, user_code_base, code_paddr, 1, page_flags)
    } {
        crate::dev::console::serial_write("[bmo_core::proc] spawn_hello: map code failed: ");
        crate::dev::console::serial_write(e);
        crate::dev::console::serial_write("\n");
        crate::proc::process::free_process(proc);
        return;
    }

    // 4. Allocate + map user stack (4KB)
    let stack_paddr = match unsafe { crate::mm::phys::alloc_pages_contiguous(1) } {
        Some(p) => p,
        None => {
            crate::dev::console::serial_write("[bmo_core::proc] spawn_hello: no phys for stack\n");
            crate::proc::process::free_process(proc);
            return;
        }
    };
    let user_stack_base: u64 = 0x7FE0_0000;
    proc.user_stack_base = user_stack_base;
    proc.user_stack_size = 4096;

    if let Err(e) = unsafe {
        crate::mm::virt::map_user_range(pml4_paddr, user_stack_base, stack_paddr, 1, page_flags)
    } {
        crate::dev::console::serial_write("[bmo_core::proc] spawn_hello: map stack failed: ");
        crate::dev::console::serial_write(e);
        crate::dev::console::serial_write("\n");
        crate::proc::process::free_process(proc);
        return;
    }

    // 5. Allocate kernel stack + create Task
    let kernel_stack = unsafe {
        let layout = core::alloc::Layout::from_size_align(8192, 16).unwrap();
        let ptr = alloc::alloc::alloc(layout);
        if ptr.is_null() {
            crate::dev::console::serial_write("[bmo_core::proc] spawn_hello: no heap for kernel stack\n");
            crate::proc::process::free_process(proc);
            return;
        }
        ptr as u64 + 8192
    };

    let task = match crate::proc::task::alloc(pid, crate::proc::Priority::Interactive) {
        Some(t) => t,
        None => {
            crate::dev::console::serial_write("[bmo_core::proc] spawn_hello: no task slots\n");
            crate::proc::process::free_process(proc);
            return;
        }
    };
    task.kernel_stack_top = kernel_stack;
    task.regs = crate::proc::task::SavedRegs::new_user(user_code_base, user_stack_base + 4096);

    crate::dev::console::serial_write("[bmo_core::proc] spawn_hello: DONE\n");
    crate::cabina::info_u64("bmo_core.proc", "spawned pid", pid.0 as u64);
}

/// Generate a minimal static x86-64 ELF that calls write(1, msg, len) then exit(0).
/// Returns (bytes, entry_point).
fn make_elf_hello() -> alloc::vec::Vec<u8> {
    // Layout: [ELF header 64B] [Phdr 56B] [code + data @ 0x80]
    let entry: u64 = 0x400_000;
    let ph_offset: u64 = 64;
    let code_offset: u64 = 0x80;

    // Machine code: write(1, msg, 7); exit(0)
    // msg is appended right after the code
    let code: &[u8] = &[
        0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00, // mov rax, 1  (SYS_write)
        0x48, 0xc7, 0xc7, 0x01, 0x00, 0x00, 0x00, // mov rdi, 1  (stdout)
        0x48, 0x8d, 0x35, 0x15, 0x00, 0x00, 0x00, // lea rsi, [rip+0x15] -> msg
        0x48, 0xc7, 0xc2, 0x07, 0x00, 0x00, 0x00, // mov rdx, 7  (count)
        0x0f, 0x05,                               // syscall
        0x48, 0xc7, 0xc0, 0x3c, 0x00, 0x00, 0x00, // mov rax, 60 (SYS_exit)
        0x48, 0x31, 0xff,                         // xor rdi, rdi
        0x0f, 0x05,                               // syscall
        b'H', b'e', b'l', b'l', b'o', b'!', b'\n', // msg
    ];
    let code_len = code.len() as u64; // 49 bytes

    // e_ident
    let ident: &[u8] = &[
        0x7f, b'E', b'L', b'F', // magic
        0x02, // 64-bit
        0x01, // little-endian
        0x01, // ELFOSABI_NONE
        0x00, // padding
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    let mut elf = alloc::vec::Vec::new();
    elf.extend_from_slice(ident);
    elf.extend_from_slice(&(2u16).to_le_bytes());      // e_type = ET_EXEC
    elf.extend_from_slice(&(62u16).to_le_bytes());     // e_machine = EM_X86_64
    elf.extend_from_slice(&(1u32).to_le_bytes());       // e_version
    elf.extend_from_slice(&entry.to_le_bytes());         // e_entry
    elf.extend_from_slice(&ph_offset.to_le_bytes());     // e_phoff
    elf.extend_from_slice(&0u64.to_le_bytes());          // e_shoff
    elf.extend_from_slice(&0u32.to_le_bytes());          // e_flags
    elf.extend_from_slice(&64u16.to_le_bytes());         // e_ehsize
    elf.extend_from_slice(&56u16.to_le_bytes());         // e_phentsize
    elf.extend_from_slice(&1u16.to_le_bytes());          // e_phnum
    elf.extend_from_slice(&0u16.to_le_bytes());          // e_shentsize
    elf.extend_from_slice(&0u16.to_le_bytes());          // e_shnum
    elf.extend_from_slice(&0u16.to_le_bytes());          // e_shstrndx
    debug_assert_eq!(elf.len(), 64);

    // Program header: PT_LOAD, PF_R|PF_X
    elf.extend_from_slice(&1u32.to_le_bytes());          // p_type = PT_LOAD
    elf.extend_from_slice(&5u32.to_le_bytes());          // p_flags = PF_R | PF_X
    elf.extend_from_slice(&code_offset.to_le_bytes());   // p_offset
    elf.extend_from_slice(&entry.to_le_bytes());         // p_vaddr
    elf.extend_from_slice(&entry.to_le_bytes());         // p_paddr
    elf.extend_from_slice(&code_len.to_le_bytes());      // p_filesz
    elf.extend_from_slice(&code_len.to_le_bytes());      // p_memsz
    elf.extend_from_slice(&0x1000u64.to_le_bytes());     // p_align
    debug_assert_eq!(elf.len(), 64 + 56);

    // Code + data
    elf.extend_from_slice(code);

    elf
}

/// Build and spawn a minimal ELF hello_world binary.
pub fn spawn_elf_hello() {
    crate::dev::console::serial_write("[bmo_core::proc] spawn_elf_hello: building ELF\n");

    let bytes = make_elf_hello();

    let img = match crate::bmo_core::bef::parsers::load(&bytes) {
        Ok(img) => img,
        Err(e) => {
            crate::dev::console::serial_write("[bmo_core::proc] spawn_elf_hello: parse failed\n");
            return;
        }
    };

    crate::dev::console::serial_write("[bmo_core::proc] spawn_elf_hello: parse OK, spawning\n");
    spawn_from_image(&img);
}

