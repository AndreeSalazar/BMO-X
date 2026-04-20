//! FastOS UEFI Bootloader
//!
//! Loads kernel.bin from EFI System Partition and jumps to it.
//! Uses uefi-rs 0.26 for UEFI services.

#![no_std]
#![no_main]

extern crate alloc;

use uefi::prelude::*;
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::Identify;
use alloc::vec::Vec;
use alloc::vec;
use log::info;

// Kernel load address
const KERNEL_LOAD_ADDR: u64 = 0x100000; // 1MB

/// Read a file from the ESP into a buffer
fn read_file(filename: &str) -> Option<Vec<u8>> {
    use uefi::proto::media::file::{File, FileMode, FileAttribute, FileInfo};
    use uefi::boot::SearchType;
    
    info!("Locating SimpleFileSystem protocol...");
    // Get SimpleFileSystem protocol
    let handles = match uefi::boot::locate_handle_buffer(
        SearchType::ByProtocol(&SimpleFileSystem::GUID)
    ) {
        Ok(h) => h,
        Err(_) => {
            info!("ERROR: Failed to locate SimpleFileSystem");
            return None;
        }
    };
    
    let fs_handle = match handles.get(0) {
        Some(h) => h,
        None => {
            info!("ERROR: No filesystem handle found");
            return None;
        }
    };
    
    info!("Opening SimpleFileSystem protocol...");
    // Open the protocol using the correct uefi-rs 0.37 API
    let mut fs = match unsafe {
        uefi::boot::open_protocol::<SimpleFileSystem>(
            uefi::boot::OpenProtocolParams {
                handle: *fs_handle,
                agent: uefi::boot::image_handle(),
                controller: Some(*fs_handle),
            },
            uefi::boot::OpenProtocolAttributes::Exclusive
        )
    } {
        Ok(f) => f,
        Err(_) => {
            info!("ERROR: Failed to open SimpleFileSystem protocol");
            return None;
        }
    };
    
    info!("Opening volume...");
    let mut root = match fs.open_volume() {
        Ok(r) => r,
        Err(_) => {
            info!("ERROR: Failed to open volume");
            return None;
        }
    };
    
    // Convert filename to UCS-2
    info!("Converting filename to UCS-2...");
    let mut ucs2_buf = [0u16; 256];
    let ucs2_filename = match uefi::CStr16::from_str_with_buf(filename, &mut ucs2_buf) {
        Ok(s) => s,
        Err(_) => {
            info!("ERROR: Failed to convert filename to UCS-2");
            return None;
        }
    };
    
    info!("Opening file: {}", filename);
    let mut file = match root.open(ucs2_filename, FileMode::Read, FileAttribute::empty()) {
        Ok(f) => f.into_regular_file().expect("Not a regular file"),
        Err(_) => {
            info!("ERROR: Failed to open file");
            return None;
        }
    };
    
    // Get file size
    info!("Getting file info...");
    let mut info_buf = [0u8; 128];
    let info = match file.get_info::<FileInfo>(&mut info_buf) {
        Ok(i) => i,
        Err(_) => {
            info!("ERROR: Failed to get file info");
            return None;
        }
    };
    let file_size = info.file_size() as usize;
    info!("File size: {} bytes", file_size);
    
    // Read file into buffer
    info!("Reading file into buffer...");
    let mut buffer = vec![0u8; file_size];
    match file.read(&mut buffer) {
        Ok(_) => {},
        Err(_) => {
            info!("ERROR: Failed to read file");
            return None;
        }
    }
    
    info!("File read successfully");
    Some(buffer)
}

/// Jump to kernel entry point
unsafe fn jump_to_kernel(kernel_addr: u64, _gsp_addr: u64, _gsp_size: u64, _mem_map: u64) -> ! {
    // Direct jump to kernel address (not a function call)
    // This works for raw binary kernels
    let kernel_ptr = kernel_addr as *const ();
    core::arch::asm!(
        "jmp {0}",
        in(reg) kernel_ptr,
        options(noreturn)
    );
}

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    info!("FastOS UEFI Bootloader v0.1.0");
    
    // Read kernel.bin from ESP
    info!("Reading kernel.bin from ESP...");
    let kernel_data = match read_file(r"kernel.bin") {
        Some(data) => data,
        None => {
            info!("ERROR: Failed to read kernel.bin");
            return Status::DEVICE_ERROR;
        }
    };
    info!("Kernel loaded: {} bytes", kernel_data.len());
    
    // Allocate memory for kernel at 1MB
    let kernel_pages = (kernel_data.len() + 4095) / 4096;
    info!("Allocating {} pages at 0x{:x}", kernel_pages, KERNEL_LOAD_ADDR);
    let kernel_mem = uefi::boot::allocate_pages(
        uefi::boot::AllocateType::Address(KERNEL_LOAD_ADDR),
        uefi::boot::MemoryType::LOADER_DATA,
        kernel_pages
    );
    
    if kernel_mem.is_err() {
        info!("ERROR: Failed to allocate memory for kernel");
        return Status::DEVICE_ERROR;
    }
    
    // Copy kernel to allocated memory
    info!("STEP 1: About to copy kernel to 0x{:x}", KERNEL_LOAD_ADDR);
    unsafe {
        let kernel_ptr = KERNEL_LOAD_ADDR as *mut u8;
        core::ptr::copy_nonoverlapping(kernel_data.as_ptr(), kernel_ptr, kernel_data.len());
    }
    info!("STEP 2: Kernel copied successfully");
    
    // Print final message before exiting boot services (logger will be invalidated)
    info!("STEP 3: About to exit boot services and jump to kernel at 0x{:x}", KERNEL_LOAD_ADDR);
    
    // Exit boot services - this invalidates all boot services including stdout/stderr
    info!("STEP 4: Calling exit_boot_services...");
    let _memory_map = unsafe { uefi::boot::exit_boot_services(None) };
    // Logger is now invalid - no more info!() calls
    
    // Jump to kernel (gsp_addr=0, gsp_size=0, mem_map=0 for now)
    unsafe {
        jump_to_kernel(KERNEL_LOAD_ADDR, 0, 0, 0);
    }
}
