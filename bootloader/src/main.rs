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

#[global_allocator]
static ALLOC: uefi::allocator::Allocator = uefi::allocator::Allocator;

// Kernel load address
const KERNEL_LOAD_ADDR: u64 = 0x100000; // 1MB

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("hlt") };
    }
}

/// Read a file from the ESP into a buffer
fn read_file(boot_services: &BootServices, filename: &str) -> Option<Vec<u8>> {
    use uefi::proto::media::file::{File, FileMode, FileAttribute, FileInfo};
    use uefi::table::boot::SearchType;
    
    // Get SimpleFileSystem protocol
    let handles = match boot_services.locate_handle_buffer(
        SearchType::ByProtocol(&SimpleFileSystem::GUID)
    ) {
        Ok(h) => h,
        Err(_) => return None,
    };
    
    let fs_handle = match handles.get(0) {
        Some(h) => h,
        None => return None,
    };
    
    // Open the protocol using the correct uefi-rs 0.26 API
    let mut fs = match unsafe {
        boot_services.open_protocol::<SimpleFileSystem>(
            uefi::table::boot::OpenProtocolParams {
                handle: *fs_handle,
                agent: boot_services.image_handle(),
                controller: Some(*fs_handle),
            },
            uefi::table::boot::OpenProtocolAttributes::Exclusive
        )
    } {
        Ok(f) => f,
        Err(_) => return None,
    };
    
    let mut root = match fs.open_volume() {
        Ok(r) => r,
        Err(_) => return None,
    };
    
    // Convert filename to UCS-2
    let mut ucs2_buf = [0u16; 256];
    let ucs2_filename = match uefi::CStr16::from_str_with_buf(filename, &mut ucs2_buf) {
        Ok(s) => s,
        Err(_) => return None,
    };
    
    let mut file = match root.open(ucs2_filename, FileMode::Read, FileAttribute::empty()) {
        Ok(f) => f.into_regular_file().expect("Not a regular file"),
        Err(_) => return None,
    };
    
    // Get file size
    let mut info_buf = [0u8; 128];
    let info = match file.get_info::<FileInfo>(&mut info_buf) {
        Ok(i) => i,
        Err(_) => return None,
    };
    let file_size = info.file_size() as usize;
    
    // Read file into buffer
    let mut buffer = vec![0u8; file_size];
    match file.read(&mut buffer) {
        Ok(_) => {},
        Err(_) => return None,
    }
    
    Some(buffer)
}

/// Jump to kernel entry point
unsafe fn jump_to_kernel(kernel_addr: u64, gsp_addr: u64, gsp_size: u64, mem_map: u64) -> ! {
    type KernelMain = extern "C" fn(u64, u64, u64) -> !;
    let kernel_main: KernelMain = core::mem::transmute(kernel_addr);
    kernel_main(gsp_addr, gsp_size, mem_map);
}

#[entry]
fn main(image: Handle, st: SystemTable<Boot>) -> Status {
    let boot_services = st.boot_services();
    
    // Read kernel.bin from ESP
    let kernel_data = match read_file(boot_services, r"kernel.bin") {
        Some(data) => data,
        None => {
            return Status::DEVICE_ERROR;
        }
    };
    
    // Allocate memory for kernel at 1MB
    let kernel_pages = (kernel_data.len() + 4095) / 4096;
    let kernel_mem = boot_services.allocate_pages(
        uefi::table::boot::AllocateType::Address(KERNEL_LOAD_ADDR),
        uefi::table::boot::MemoryType::LOADER_DATA,
        kernel_pages
    );
    
    if kernel_mem.is_err() {
        return Status::DEVICE_ERROR;
    }
    
    // Copy kernel to allocated memory
    unsafe {
        let kernel_ptr = KERNEL_LOAD_ADDR as *mut u8;
        core::ptr::copy_nonoverlapping(kernel_data.as_ptr(), kernel_ptr, kernel_data.len());
    }
    
    // Exit boot services
    let _memory_map = st.exit_boot_services(uefi::table::boot::MemoryType::LOADER_DATA);
    
    // Jump to kernel (gsp_addr=0, gsp_size=0, mem_map=0 for now)
    unsafe {
        jump_to_kernel(KERNEL_LOAD_ADDR, 0, 0, 0);
    }
}
