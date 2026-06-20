//! NT Syscall Table — maps Nt* functions to BMO syscalls.
//!
//! This is the core of the ntdll gateway. Each NT syscall is translated
//!
//! v1.6.16: allow(unreachable_code) — `kill_current_process` returns
//! `-> !` (never), so the `NtStatus::Success` lines after it are
//! unreachable. The compiler still wants the explicit return for the
//! type checker; we keep it as documentation of the syscall contract.

#![allow(unreachable_code)]
//! to the corresponding BMO syscall, similar to how Wine's syscall
//! dispatcher works.
//!
//! Wine's approach:
//!   NtCreateFile → __wine_syscall_dispatcher → Unix implementation
//!
//! Our approach:
//!   NtCreateFile → BMO syscall 0x20 (FileOpen) → ramdisk::open

#![allow(dead_code)]

use crate::bmo_abi::interop::win32::ntdll::NtStatus;
use crate::bmo_abi::primitives::bx_u64;

/// NT syscall number → BMO syscall number mapping.
///
/// These are the standard Windows NT syscall numbers (from ReactOS/Wine).
/// We map them to BMO syscall numbers.
pub const NT_SYSCALL_MAP: &[(u32, &str, u16)] = &[
    // Memory management
    (0x0014, "NtAllocateVirtualMemory", 0x10),   // → BMO Mmap
    (0x001E, "NtFreeVirtualMemory", 0x11),       // → BMO Munmap
    (0x0050, "NtProtectVirtualMemory", 0x12),    // → BMO Mprotect

    // File I/O
    (0x0055, "NtCreateFile", 0x20),              // → BMO FileOpen
    (0x0003, "NtReadFile", 0x21),                // → BMO FileRead
    (0x0008, "NtWriteFile", 0x22),               // → BMO FileWrite
    (0x000E, "NtClose", 0x23),                   // → BMO FileClose
    (0x0033, "NtSetInformationFile", 0x24),      // → BMO FileSeek (simplified)
    (0x0010, "NtQueryInformationFile", 0x25),    // → BMO FileSize (simplified)

    // Process management
    (0x002C, "NtTerminateProcess", 0x00),        // → BMO ProcessExit
    (0x004C, "NtCreateProcess", 0x01),           // → BMO ProcessCreate (stub)
    (0x004D, "NtCreateProcessEx", 0x01),         // → BMO ProcessCreate (stub)

    // Thread management
    (0x004B, "NtCreateThread", 0x04),            // → BMO ThreadCreate
    (0x004E, "NtCreateThreadEx", 0x04),          // → BMO ThreadCreate
    (0x0030, "NtTerminateThread", 0x05),         // → BMO ThreadExit
    (0x0000, "NtWaitForSingleObject", 0x03),     // → BMO Yield (simplified)
    (0x0001, "NtWaitForMultipleObjects", 0x03),  // → BMO Yield (simplified)

    // Time
    (0x005A, "NtQuerySystemTime", 0x50),         // → BMO ClockGetTime
    (0x005B, "NtSetSystemTime", 0x50),           // → BMO ClockGetTime (read-only)
    (0x005C, "NtQueryPerformanceCounter", 0x50), // → BMO ClockGetTime

    // Debug
    (0x005E, "NtQuerySystemInformation", 0xF0),  // → BMO DebugPrint (stub)
];

/// Resolve an NT syscall number to a BMO syscall number.
pub fn nt_to_bmo_syscall(nt_nr: u32) -> Option<u16> {
    for &(nr, _, bmo_nr) in NT_SYSCALL_MAP {
        if nr == nt_nr {
            return Some(bmo_nr);
        }
    }
    None
}

/// Resolve an NT syscall name to a BMO syscall number.
pub fn nt_name_to_bmo_syscall(name: &str) -> Option<u16> {
    for &(_, n, bmo_nr) in NT_SYSCALL_MAP {
        if n == name {
            return Some(bmo_nr);
        }
    }
    None
}

/// NtAllocateVirtualMemory — allocate virtual memory.
///
/// Windows signature:
///   NTSTATUS NtAllocateVirtualMemory(
///     HANDLE ProcessHandle,
///     PVOID *BaseAddress,
///     ULONG_PTR ZeroBits,
///     PSIZE_T RegionSize,
///     ULONG AllocationType,
///     ULONG Protect
///   );
///
/// BMO mapping: syscall 0x10 (Mmap)
#[no_mangle]
pub extern "C" fn NtAllocateVirtualMemory(
    process_handle: bx_u64,
    base_address: *mut bx_u64,
    zero_bits: bx_u64,
    region_size: *mut bx_u64,
    allocation_type: u32,
    protect: u32,
) -> i32 {
    let _ = (process_handle, zero_bits, allocation_type, protect);

    if base_address.is_null() || region_size.is_null() {
        return NtStatus::InvalidParameter as i32;
    }

    let size = unsafe { *region_size };
    let requested_addr = unsafe { *base_address };

    // Map to BMO Mmap syscall (0x10)
    // For now, use the kernel heap allocator
    let layout = core::alloc::Layout::from_size_align(size as usize, 4096);
    let Ok(layout) = layout else {
        return NtStatus::NoMemory as i32;
    };

    let ptr = if requested_addr != 0 {
        // TODO: respect requested address (requires page allocator)
        unsafe { alloc::alloc::alloc_zeroed(layout) }
    } else {
        unsafe { alloc::alloc::alloc_zeroed(layout) }
    };

    if ptr.is_null() {
        return NtStatus::NoMemory as i32;
    }

    unsafe {
        *base_address = ptr as bx_u64;
        *region_size = size;
    }

    NtStatus::Success as i32
}

/// NtFreeVirtualMemory — free virtual memory.
///
/// BMO mapping: syscall 0x11 (Munmap)
#[no_mangle]
pub extern "C" fn NtFreeVirtualMemory(
    process_handle: bx_u64,
    base_address: *mut bx_u64,
    region_size: *mut bx_u64,
    free_type: u32,
) -> i32 {
    let _ = (process_handle, free_type);

    if base_address.is_null() || region_size.is_null() {
        return NtStatus::InvalidParameter as i32;
    }

    let addr = unsafe { *base_address };
    let size = unsafe { *region_size };

    if addr == 0 {
        return NtStatus::InvalidHandle as i32;
    }

    // Map to BMO Munmap syscall (0x11)
    // For now, use the kernel heap deallocator
    let layout = core::alloc::Layout::from_size_align(size as usize, 4096);
    if let Ok(layout) = layout {
        unsafe {
            alloc::alloc::dealloc(addr as *mut u8, layout);
        }
    }

    unsafe {
        *base_address = 0;
        *region_size = 0;
    }

    NtStatus::Success as i32
}

/// NtProtectVirtualMemory — change memory protection.
///
/// BMO mapping: syscall 0x12 (Mprotect)
#[no_mangle]
pub extern "C" fn NtProtectVirtualMemory(
    process_handle: bx_u64,
    base_address: *mut bx_u64,
    region_size: *mut bx_u64,
    new_protect: u32,
    old_protect: *mut u32,
) -> i32 {
    let _ = (process_handle, base_address, region_size, new_protect);

    // BMO doesn't have fine-grained memory protection yet
    // Just return success and pretend it worked
    if !old_protect.is_null() {
        unsafe { *old_protect = 0x04; } // PAGE_READWRITE
    }

    NtStatus::Success as i32
}

/// NtCreateFile — open or create a file.
///
/// BMO mapping: syscall 0x20 (FileOpen)
#[no_mangle]
pub extern "C" fn NtCreateFile(
    file_handle: *mut bx_u64,
    desired_access: u32,
    object_attributes: *const crate::bmo_abi::interop::win32::ntdll::ObjectAttributes,
    io_status_block: *mut crate::bmo_abi::interop::win32::ntdll::IoStatusBlock,
    allocation_size: *const crate::bmo_abi::interop::win32::ntdll::LargeInteger,
    file_attributes: u32,
    share_access: u32,
    create_disposition: u32,
    create_options: u32,
    ea_buffer: bx_u64,
    ea_length: u32,
) -> i32 {
    let _ = (desired_access, allocation_size, file_attributes, share_access,
             create_disposition, create_options, ea_buffer, ea_length);

    if file_handle.is_null() || object_attributes.is_null() {
        return NtStatus::InvalidParameter as i32;
    }

    // Extract file name from object_attributes
    // In real Windows, object_attributes->object_name is a UNICODE_STRING
    // For now, we'll use a simplified approach

    // Map to BMO FileOpen syscall (0x20)
    // This is a stub — real implementation would parse UNICODE_STRING
    let fd = crate::fs::ramdisk::open(0, 0); // TODO: pass real filename

    if fd == u64::MAX {
        return NtStatus::NoSuchFile as i32;
    }

    unsafe {
        *file_handle = fd;
    }

    if !io_status_block.is_null() {
        unsafe {
            (*io_status_block).status = NtStatus::Success as i32;
            (*io_status_block).information = 1; // FILE_OPENED
        }
    }

    NtStatus::Success as i32
}

/// NtReadFile — read from a file.
///
/// BMO mapping: syscall 0x21 (FileRead)
#[no_mangle]
pub extern "C" fn NtReadFile(
    file_handle: bx_u64,
    event: bx_u64,
    apc_routine: bx_u64,
    apc_context: bx_u64,
    io_status_block: *mut crate::bmo_abi::interop::win32::ntdll::IoStatusBlock,
    buffer: bx_u64,
    length: u32,
    byte_offset: *const crate::bmo_abi::interop::win32::ntdll::LargeInteger,
    key: *mut u32,
) -> i32 {
    let _ = (event, apc_routine, apc_context, byte_offset, key);

    if buffer == 0 || length == 0 {
        return NtStatus::InvalidParameter as i32;
    }

    // Map to BMO FileRead syscall (0x21)
    let bytes_read = crate::fs::ramdisk::read(file_handle, buffer, length as u64);

    if bytes_read == u64::MAX {
        return NtStatus::Unsuccessful as i32;
    }

    if !io_status_block.is_null() {
        unsafe {
            (*io_status_block).status = NtStatus::Success as i32;
            (*io_status_block).information = bytes_read;
        }
    }

    NtStatus::Success as i32
}

/// NtWriteFile — write to a file.
///
/// BMO mapping: syscall 0x22 (FileWrite)
#[no_mangle]
pub extern "C" fn NtWriteFile(
    file_handle: bx_u64,
    event: bx_u64,
    apc_routine: bx_u64,
    apc_context: bx_u64,
    io_status_block: *mut crate::bmo_abi::interop::win32::ntdll::IoStatusBlock,
    buffer: bx_u64,
    length: u32,
    byte_offset: *const crate::bmo_abi::interop::win32::ntdll::LargeInteger,
    key: *mut u32,
) -> i32 {
    let _ = (event, apc_routine, apc_context, byte_offset, key);

    if buffer == 0 || length == 0 {
        return NtStatus::InvalidParameter as i32;
    }

    // Map to BMO FileWrite syscall (0x22)
    let bytes_written = crate::fs::ramdisk::write(file_handle, buffer, length as u64);

    if !io_status_block.is_null() {
        unsafe {
            (*io_status_block).status = NtStatus::Success as i32;
            (*io_status_block).information = bytes_written;
        }
    }

    NtStatus::Success as i32
}

/// NtClose — close a handle.
///
/// BMO mapping: syscall 0x23 (FileClose)
#[no_mangle]
pub extern "C" fn NtClose(handle: bx_u64) -> i32 {
    // Map to BMO FileClose syscall (0x23)
    let result = crate::fs::ramdisk::close(handle);

    if result == u64::MAX {
        NtStatus::InvalidHandle as i32
    } else {
        NtStatus::Success as i32
    }
}

/// NtTerminateProcess — terminate a process.
///
/// BMO mapping: syscall 0x00 (ProcessExit)
#[no_mangle]
pub extern "C" fn NtTerminateProcess(process_handle: bx_u64, exit_status: i32) -> i32 {
    let _ = process_handle;
    crate::sched::process::kill_current_process(0, exit_status as u64, 0);
    NtStatus::Success as i32
}

/// NtCreateThreadEx — create a thread.
///
/// BMO mapping: syscall 0x04 (ThreadCreate)
#[no_mangle]
pub extern "C" fn NtCreateThreadEx(
    thread_handle: *mut bx_u64,
    desired_access: u32,
    object_attributes: bx_u64,
    process_handle: bx_u64,
    start_routine: bx_u64,
    parameter: bx_u64,
    create_flags: u32,
    zero_bits: bx_u64,
    stack_size: bx_u64,
    maximum_stack_size: bx_u64,
    attribute_list: bx_u64,
) -> i32 {
    let _ = (desired_access, object_attributes, process_handle, parameter,
             create_flags, zero_bits, stack_size, maximum_stack_size, attribute_list);

    if thread_handle.is_null() || start_routine == 0 {
        return NtStatus::InvalidParameter as i32;
    }

    // Map to BMO ThreadCreate syscall (0x04)
    match crate::sched::thread::alloc_thread(
        crate::sched::process::Pid(1),
        crate::sched::Priority::Interactive,
    ) {
        Some(thr) => {
            thr.regs = crate::sched::thread::SavedRegs::new_user(start_routine, 0);
            thr.state = crate::sched::thread::ThreadState::Ready;
            unsafe {
                *thread_handle = thr.tid.0 as bx_u64;
            }
            NtStatus::Success as i32
        }
        None => NtStatus::NoMemory as i32,
    }
}

/// NtTerminateThread — terminate a thread.
///
/// BMO mapping: syscall 0x05 (ThreadExit)
#[no_mangle]
pub extern "C" fn NtTerminateThread(thread_handle: bx_u64, exit_status: i32) -> i32 {
    let _ = thread_handle;
    crate::sched::process::kill_current_process(0, exit_status as u64, 0);
    NtStatus::Success as i32
}

/// NtQuerySystemTime — get current system time.
///
/// BMO mapping: syscall 0x50 (ClockGetTime)
#[no_mangle]
pub extern "C" fn NtQuerySystemTime(system_time: *mut i64) -> i32 {
    if system_time.is_null() {
        return NtStatus::InvalidParameter as i32;
    }

    // Map to BMO ClockGetTime syscall (0x50)
    let tsc = crate::arch::cpu::rdtsc();
    unsafe {
        *system_time = tsc as i64;
    }

    NtStatus::Success as i32
}

/// NtQueryPerformanceCounter — get performance counter.
///
/// BMO mapping: syscall 0x50 (ClockGetTime)
#[no_mangle]
pub extern "C" fn NtQueryPerformanceCounter(
    performance_counter: *mut i64,
    performance_frequency: *mut i64,
) -> i32 {
    if performance_counter.is_null() {
        return NtStatus::InvalidParameter as i32;
    }

    let tsc = crate::arch::cpu::rdtsc();
    unsafe {
        *performance_counter = tsc as i64;
    }

    if !performance_frequency.is_null() {
        // Assume 3.7 GHz for Ryzen 5 5600X
        unsafe {
            *performance_frequency = 3_700_000_000;
        }
    }

    NtStatus::Success as i32
}

