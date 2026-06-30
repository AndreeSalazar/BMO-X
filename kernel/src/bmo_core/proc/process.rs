pub type Capabilities = u32;
const CAP_NONE: Capabilities = 0;

pub const MAX_PROCESSES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pid(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Free,
    Active,
    Zombie,
}

#[derive(Debug)]
pub struct Process {
    pub pid: Pid,
    pub state: ProcessState,
    pub page_table_root: u64,
    pub caps: Capabilities,
    pub name: [u8; 32],
    pub name_len: usize,
    pub entry_point: u64,
    pub exit_code: i32,
    pub user_code_base: u64,
    pub user_code_size: usize,
    pub user_stack_base: u64,
    pub user_stack_size: usize,
    pub addr_space: crate::mm::virt::AddressSpace,
    /// If true, ring3 `syscall` instructions route to Linux syscall emulation
    /// instead of BMO native syscalls.
    pub linux_emulation: bool,
}

impl Process {
    pub const fn empty() -> Self {
        Self {
            pid: Pid(0),
            state: ProcessState::Free,
            page_table_root: 0,
            caps: CAP_NONE,
            name: [0u8; 32],
            name_len: 0,
            entry_point: 0,
            exit_code: 0,
            user_code_base: 0,
            user_code_size: 0,
            user_stack_base: 0,
            user_stack_size: 0,
            addr_space: crate::mm::virt::AddressSpace::empty(),
            linux_emulation: false,
        }
    }

    pub fn set_name(&mut self, n: &str) {
        let bytes = n.as_bytes();
        let len = bytes.len().min(31);
        self.name[..len].copy_from_slice(&bytes[..len]);
        self.name_len = len;
    }

    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("???")
    }
}

static mut PROCESS_TABLE: [Process; MAX_PROCESSES] = {
    const EMPTY: Process = Process::empty();
    [EMPTY; MAX_PROCESSES]
};

static mut NEXT_PID: u32 = 1;

pub fn alloc_process() -> Option<&'static mut Process> {
    unsafe {
        for i in 0..MAX_PROCESSES {
            if PROCESS_TABLE[i].state == ProcessState::Free {
                PROCESS_TABLE[i].pid = Pid(NEXT_PID);
                PROCESS_TABLE[i].state = ProcessState::Active;
                NEXT_PID += 1;
                return Some(&mut PROCESS_TABLE[i]);
            }
        }
        None
    }
}

#[allow(static_mut_refs)]
pub fn get_process(pid: Pid) -> Option<&'static mut Process> {
    unsafe {
        PROCESS_TABLE.iter_mut().find(|p| p.pid == pid && p.state != ProcessState::Free)
    }
}

#[allow(static_mut_refs)]
pub fn process_count() -> usize {
    unsafe {
        PROCESS_TABLE.iter().filter(|p| p.state != ProcessState::Free).count()
    }
}

pub fn free_process(proc: &mut Process) {
    if proc.state == ProcessState::Free {
        return;
    }

    if proc.user_code_size > 0 {
        let code_pages = (proc.user_code_size + crate::mm::phys::page_size() - 1) / crate::mm::phys::page_size();
        unsafe {
            crate::mm::phys::free_pages(proc.user_code_base, code_pages);
        }
    }

    if proc.user_stack_size > 0 {
        let stack_pages = (proc.user_stack_size + crate::mm::phys::page_size() - 1) / crate::mm::phys::page_size();
        unsafe {
            crate::mm::phys::free_pages(proc.user_stack_base, stack_pages);
        }
    }

    if proc.page_table_root != 0 {
        unsafe {
            crate::mm::virt::free_user_page_tables(proc.page_table_root);
            crate::mm::phys::free_pages(proc.page_table_root, 1);
        }
        proc.page_table_root = 0;
    }

    proc.state = ProcessState::Free;
    proc.pid = Pid(0);
    proc.caps = CAP_NONE;
    proc.name = [0u8; 32];
    proc.name_len = 0;
    proc.entry_point = 0;
    proc.exit_code = 0;
    proc.user_code_base = 0;
    proc.user_code_size = 0;
    proc.user_stack_base = 0;
    proc.user_stack_size = 0;
    proc.linux_emulation = false;
}

pub fn kill_current_process(vector: u64, _error_code: u64, _cr2: u64) -> ! {
    let current_idx = crate::proc::task::current_index();
    if let Some(thread) = crate::proc::task::get(current_idx) {
        let pid = thread.pid;

        thread.state = crate::proc::task::State::Dead;
        thread.tid = crate::proc::task::Tid(0);
        thread.pid = Pid(0);
        thread.time_slice = 0;

        if let Some(proc) = get_process(pid) {
            proc.exit_code = -1;
            proc.state = ProcessState::Zombie;

            let kernel_cr3 = crate::mm::virt::read_cr3();
            if proc.page_table_root != 0 && proc.page_table_root != kernel_cr3 {
                unsafe { crate::mm::virt::write_cr3(kernel_cr3); }
            }

            free_process(proc);
        }

        crate::proc::task::set_current(usize::MAX);
    }

    crate::proc::schedule();

    loop {
        unsafe { core::arch::asm!("sti; hlt"); }
    }
}
