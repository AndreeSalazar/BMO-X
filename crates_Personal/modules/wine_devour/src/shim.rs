//! Shim module — Windows NT → BMO syscall translator.
//!
//! Fase 1: 6 NT syscalls for Hello World (.exe)
//! Fase 2: +3 graphics syscalls (DirectX window + GPU)
//! Fase 3: +4 Steam/network syscalls

/// NT syscall number → BMO syscall number.
pub fn translate_nt_to_bmo(nt_nr: u64) -> u64 {
    match nt_nr {
        // Fase 1: ntdll.dll básico
        8   => 0xF0, // NtWriteFile → debug_print
        44  => 0x00, // NtTerminateProcess → exit
        24  => 0x10, // NtAllocateVirtualMemory → MMAP
        30  => 0x11, // NtFreeVirtualMemory → unmap
        85  => 0x20, // NtCreateFile → open
        21  => 0x24, // NtClose → close

        // Fase 2: Graphics
        4096 => 0x60, // NtUserCreateWindowEx → fb_info
        2048 => 0xA0, // NtGdiDdDDICreateDevice → GPU submit
        2049 => 0xA0, // NtGdiDdDDISubmitCommand → GPU submit

        // Fase 3: Steam
        7   => 0x90, // NtDeviceIoControlFile → net
        193 => 0x04, // NtCreateThreadEx → task_alloc
        4   => 0x03, // NtWaitForSingleObject → yield
        34  => 0x50, // NtQueryInformationProcess → clock

        _ => u64::MAX,
    }
}

/// Human-readable NT syscall name.
pub fn nt_syscall_name(nr: u64) -> &'static str {
    match nr {
        4 => "NtWaitForSingleObject", 7 => "NtDeviceIoControlFile",
        8 => "NtWriteFile", 21 => "NtClose", 24 => "NtAllocateVirtualMemory",
        30 => "NtFreeVirtualMemory", 34 => "NtQueryInformationProcess",
        44 => "NtTerminateProcess", 85 => "NtCreateFile",
        193 => "NtCreateThreadEx",
        2048 => "NtGdiDdDDICreateDevice", 2049 => "NtGdiDdDDISubmitCommand",
        4096 => "NtUserCreateWindowEx",
        _ => "unknown",
    }
}
