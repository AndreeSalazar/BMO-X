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
