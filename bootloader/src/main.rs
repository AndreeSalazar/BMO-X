//! FastOS UEFI Bootloader
//!
//! Loads kernel.elf from the EFI System Partition, parses ELF64 headers,
//! queries GOP for framebuffer info, finds RSDP, builds BootInfo,
//! exits boot services, and jumps to the kernel entry point.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use fastos_boot_protocol::{
    BootInfo, MemoryEntry, MemoryType as BootMemType, PixelFormat, BOOT_MAGIC, MAX_MEMORY_ENTRIES,
};
use log::info;
use uefi::boot;
use uefi::mem::memory_map::{MemoryMap, MemoryType};
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::media::fs::SimpleFileSystem;
// ── Constants ────────────────────────────────────────────────────────────────

const KERNEL_STACK_SIZE: usize = 256 * 1024; // 256 KiB

// ELF64 constants
const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];
const PT_LOAD: u32 = 1;

// ── ELF64 header helpers (manual parsing, no external crate) ────────────────

/// Read a little-endian u16 from a byte slice at `off`.
fn read_u16(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

/// Read a little-endian u32 from a byte slice at `off`.
fn read_u32(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

/// Read a little-endian u64 from a byte slice at `off`.
fn read_u64(data: &[u8], off: usize) -> u64 {
    u64::from_le_bytes([
        data[off],
        data[off + 1],
        data[off + 2],
        data[off + 3],
        data[off + 4],
        data[off + 5],
        data[off + 6],
        data[off + 7],
    ])
}

/// Parsed ELF64 program header (only the fields we need).
struct Elf64Phdr {
    p_type: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_filesz: u64,
    p_memsz: u64,
}

/// Parsed ELF64 top-level info.
struct Elf64Info {
    entry: u64,
    phdrs: Vec<Elf64Phdr>,
}

/// Parse an ELF64 binary from raw bytes. Returns entry point + program headers.
fn parse_elf64(data: &[u8]) -> Option<Elf64Info> {
    // Validate magic
    if data.len() < 64 || data[0..4] != ELF_MAGIC {
        info!("ELF: bad magic");
        return None;
    }
    // Must be 64-bit (class 2)
    if data[4] != 2 {
        info!("ELF: not 64-bit");
        return None;
    }

    let e_entry = read_u64(data, 24);
    let e_phoff = read_u64(data, 32) as usize;
    let e_phentsize = read_u16(data, 54) as usize;
    let e_phnum = read_u16(data, 56) as usize;

    info!(
        "ELF: entry=0x{:x}  phoff=0x{:x}  phentsize={}  phnum={}",
        e_entry, e_phoff, e_phentsize, e_phnum
    );

    let mut phdrs = Vec::with_capacity(e_phnum);
    for i in 0..e_phnum {
        let off = e_phoff + i * e_phentsize;
        phdrs.push(Elf64Phdr {
            p_type: read_u32(data, off),
            p_offset: read_u64(data, off + 8),
            p_vaddr: read_u64(data, off + 16),
            p_filesz: read_u64(data, off + 32),
            p_memsz: read_u64(data, off + 40),
        });
    }

    Some(Elf64Info {
        entry: e_entry,
        phdrs,
    })
}

// ── File I/O ────────────────────────────────────────────────────────────────

/// Read a file from the filesystem on `device_handle`.
fn read_file_from_device(device_handle: uefi::Handle, filename: &str) -> Option<Vec<u8>> {
    use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode};

    info!("Opening SimpleFileSystem on boot device...");
    let mut fs = boot::open_protocol_exclusive::<SimpleFileSystem>(device_handle).ok()?;

    info!("Opening volume root...");
    let mut root = fs.open_volume().ok()?;

    // Convert filename to UCS-2
    let mut ucs2_buf = [0u16; 256];
    let ucs2_name = uefi::CStr16::from_str_with_buf(filename, &mut ucs2_buf).ok()?;

    info!("Opening file: {}", filename);
    let mut file = root
        .open(ucs2_name, FileMode::Read, FileAttribute::empty())
        .ok()?
        .into_regular_file()
        .expect("Not a regular file");

    // Query file size
    let mut info_buf = [0u8; 256];
    let finfo = file.get_info::<FileInfo>(&mut info_buf).ok()?;
    let size = finfo.file_size() as usize;
    info!("File size: {} bytes", size);

    // Read entire file
    let mut buf = vec![0u8; size];
    file.read(&mut buf).ok()?;

    info!("File read OK");
    Some(buf)
}

// ── GOP query ───────────────────────────────────────────────────────────────

struct GopInfo {
    fb_addr: u64,
    fb_size: u64,
    width: u32,
    height: u32,
    stride: u32,
    pixel_format: PixelFormat,
}

fn query_gop() -> Option<GopInfo> {
    use uefi::proto::console::gop::PixelFormat as GopPixelFormat;

    info!("Querying GOP...");
    let gop = boot::open_protocol_exclusive::<GraphicsOutput>(boot::image_handle());

    // GOP may not live on the image handle; try locating it instead.
    let mut gop = match gop {
        Ok(g) => g,
        Err(_) => {
            info!("GOP not on image handle, locating via handle buffer...");
            let handles = boot::locate_handle_buffer(boot::SearchType::ByProtocol(
                &<GraphicsOutput as uefi::Identify>::GUID,
            ))
            .ok()?;
            let h = *handles.first()?;
            boot::open_protocol_exclusive::<GraphicsOutput>(h).ok()?
        }
    };

    let mode_info = gop.current_mode_info();
    let (width, height) = mode_info.resolution();
    let stride = mode_info.stride() as u32;
    let pixel_format = match mode_info.pixel_format() {
        GopPixelFormat::Bgr => PixelFormat::Bgr,
        GopPixelFormat::Rgb => PixelFormat::Rgb,
        _ => PixelFormat::Unknown,
    };

    let mut fb = gop.frame_buffer();
    let fb_addr = fb.as_mut_ptr() as u64;
    let fb_size = fb.size() as u64;

    info!(
        "GOP: {}x{} stride={} fmt={:?} fb=0x{:x} size=0x{:x}",
        width, height, stride, pixel_format, fb_addr, fb_size
    );

    Some(GopInfo {
        fb_addr,
        fb_size,
        width: width as u32,
        height: height as u32,
        stride,
        pixel_format,
    })
}

// ── RSDP lookup ─────────────────────────────────────────────────────────────

fn find_rsdp() -> u64 {
    use uefi::table::cfg::ConfigTableEntry;

    info!("Searching for RSDP in UEFI config tables...");

    uefi::system::with_config_table(|config| {
        // Prefer ACPI 2.0
        for entry in config {
            if entry.guid == ConfigTableEntry::ACPI2_GUID {
                let addr = entry.address as u64;
                info!("Found ACPI 2.0 RSDP at 0x{:x}", addr);
                return addr;
            }
        }
        // Fallback to ACPI 1.0
        for entry in config {
            if entry.guid == ConfigTableEntry::ACPI_GUID {
                let addr = entry.address as u64;
                info!("Found ACPI 1.0 RSDP at 0x{:x}", addr);
                return addr;
            }
        }
        info!("WARNING: RSDP not found");
        0
    })
}

// ── UEFI memory type → BootInfo memory type ─────────────────────────────────

fn convert_memory_type(uefi_type: MemoryType) -> BootMemType {
    match uefi_type {
        MemoryType::CONVENTIONAL => BootMemType::Usable,
        MemoryType::LOADER_CODE | MemoryType::LOADER_DATA => BootMemType::Bootloader,
        MemoryType::BOOT_SERVICES_CODE | MemoryType::BOOT_SERVICES_DATA => BootMemType::Usable,
        MemoryType::ACPI_RECLAIM => BootMemType::AcpiReclaimable,
        MemoryType::ACPI_NON_VOLATILE => BootMemType::AcpiNvs,
        MemoryType::RUNTIME_SERVICES_CODE | MemoryType::RUNTIME_SERVICES_DATA => {
            BootMemType::Reserved
        }
        _ => BootMemType::Reserved,
    }
}

// ── Entry point ─────────────────────────────────────────────────────────────

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    info!("FastOS UEFI Bootloader v0.2.0");

    // ── 1. Get boot device handle via LoadedImage ───────────────────────────
    info!("Getting boot device handle...");
    let loaded_image =
        boot::open_protocol_exclusive::<LoadedImage>(boot::image_handle()).unwrap();
    let device_handle = loaded_image
        .device()
        .expect("LoadedImage has no device handle");
    drop(loaded_image); // release protocol before opening FS on same device

    // -- 2. Read kernel.elf from ESP -----------------------------------------
    info!("Loading kernel.elf...");
    let elf_data = read_file_from_device(device_handle, "\\EFI\\BOOT\\kernel.elf")
        .expect("Failed to read kernel.elf");
    info!("kernel.elf loaded: {} bytes", elf_data.len());

    // ── 3. Parse ELF64 ─────────────────────────────────────────────────────
    let elf = parse_elf64(&elf_data).expect("Failed to parse ELF64");
    let entry_point = elf.entry;
    info!("Kernel entry point: 0x{:x}", entry_point);

    // ── 4. Load PT_LOAD segments into memory ────────────────────────────────
    //
    // Strategy: first compute total kernel span, allocate ALL pages in one
    // call covering [kernel_base_page .. kernel_end_page), then copy each
    // segment.  A single large allocation is far more likely to succeed than
    // multiple small ones that may conflict with firmware reservations.
    //
    // We try AllocateType::Address first (exact placement).  If the firmware
    // has the region reserved we fall back to AnyPages and relocate after
    // exit_boot_services.

    let mut kernel_base: u64 = u64::MAX;
    let mut kernel_end: u64 = 0;

    // First pass — compute the full virtual address span
    for phdr in &elf.phdrs {
        if phdr.p_type != PT_LOAD || phdr.p_memsz == 0 {
            continue;
        }
        let seg_start = phdr.p_vaddr;
        let seg_end = seg_start + phdr.p_memsz;
        if seg_start < kernel_base {
            kernel_base = seg_start;
        }
        if seg_end > kernel_end {
            kernel_end = seg_end;
        }
    }

    let kernel_page_base = kernel_base & !0xFFF;
    let total_pages = ((kernel_end - kernel_page_base + 0xFFF) / 0x1000) as usize;

    info!(
        "Kernel span: base=0x{:x} end=0x{:x} total_pages={}",
        kernel_page_base, kernel_end, total_pages
    );

    // Allocate at the exact address the linker script specifies.
    // Kernel is linked at 0x200000 which is above the UEFI-reserved 1 MB region.
    // Allocate as LOADER_CODE so pages are executable on NX-enabled firmware.
    boot::allocate_pages(
        boot::AllocateType::Address(kernel_page_base),
        MemoryType::LOADER_CODE,
        total_pages,
    )
    .expect("Failed to allocate kernel pages at fixed address");
    info!("Allocated kernel pages at 0x{:x}", kernel_page_base);

    // Second pass — copy each segment into the allocated pages
    for phdr in &elf.phdrs {
        if phdr.p_type != PT_LOAD || phdr.p_memsz == 0 {
            continue;
        }

        let seg_start = phdr.p_vaddr;
        let seg_end = seg_start + phdr.p_memsz;

        let dst_base = seg_start;

        let page_base = seg_start & !0xFFF;
        let pages = ((seg_end - page_base + 0xFFF) / 0x1000) as usize;

        info!(
            "PT_LOAD: vaddr=0x{:x} filesz=0x{:x} memsz=0x{:x} pages={} dst=0x{:x}",
            seg_start, phdr.p_filesz, phdr.p_memsz, pages, dst_base
        );

        unsafe {
            // Copy file data
            let dst = dst_base as *mut u8;
            let src = elf_data.as_ptr().add(phdr.p_offset as usize);
            core::ptr::copy_nonoverlapping(src, dst, phdr.p_filesz as usize);

            // Zero BSS (memsz - filesz)
            let bss_start = dst.add(phdr.p_filesz as usize);
            let bss_len = (phdr.p_memsz - phdr.p_filesz) as usize;
            core::ptr::write_bytes(bss_start, 0, bss_len);
        }
    }

    let kernel_size = kernel_end - kernel_base;
    info!(
        "Kernel loaded: base=0x{:x} size=0x{:x}",
        kernel_base, kernel_size
    );

    // ── 4b. Load GSP firmware blobs (optional) ─────────────────────────────
    let mut gsp_addr: u64 = 0;
    let mut gsp_size: u64 = 0;
    let mut gsp_bootloader_addr: u64 = 0;
    let mut gsp_bootloader_size: u64 = 0;
    let mut gsp_booter_load_addr: u64 = 0;
    let mut gsp_booter_load_size: u64 = 0;
    let mut vbios_addr: u64 = 0;
    let mut vbios_size: u64 = 0;

    info!("Loading GSP firmware blobs...");

    // Helper: load a firmware file into page-aligned memory
    // Try each path in order, return (addr, size) or (0, 0)
    let mut load_fw = |paths: &[&str]| -> (u64, u64) {
        for path in paths {
            if let Some(data) = read_file_from_device(device_handle, path) {
                let size = data.len();
                let pages = (size + 0xFFF) / 0x1000;
                let ptr = boot::allocate_pages(
                    boot::AllocateType::AnyPages,
                    MemoryType::LOADER_DATA,
                    pages,
                )
                .expect("Failed to allocate pages for firmware blob")
                .as_ptr() as *mut u8;

                unsafe {
                    core::ptr::write_bytes(ptr, 0, pages * 0x1000);
                    core::ptr::copy_nonoverlapping(data.as_ptr(), ptr, size);
                }

                info!("  Loaded {} -> 0x{:x} ({} bytes)", path, ptr as u64, size);
                return (ptr as u64, size as u64);
            }
        }
        (0, 0)
    };

    // 1. GSP-RM payload (gsp_ga10x.bin) - 69MB
    let (a, s) = load_fw(&["\\gsp_ga10x.bin", "\\EFI\\BOOT\\gsp_ga10x.bin"]);
    gsp_addr = a;
    gsp_size = s;

    // 2. RISC-V bootloader (bootloader-535.113.01.bin) - ~20KB
    let (a, s) = load_fw(&[
        "\\firmware\\bootloader-535.113.01.bin",
        "\\bootloader-535.113.01.bin",
    ]);
    gsp_bootloader_addr = a;
    gsp_bootloader_size = s;

    // 3. Falcon HS booter (booter_load-535.113.01.bin) - ~60KB
    let (a, s) = load_fw(&[
        "\\firmware\\booter_load-535.113.01.bin",
        "\\booter_load-535.113.01.bin",
    ]);
    gsp_booter_load_addr = a;
    gsp_booter_load_size = s;

    // 4. VBIOS ROM (vbios_rtx3060.rom) - needed for FWSEC-FRTS
    let (a, s) = load_fw(&[
        "\\firmware\\vbios_rtx3060.rom",
        "\\vbios_rtx3060.rom",
    ]);
    vbios_addr = a;
    vbios_size = s;

    if gsp_addr == 0 {
        info!("WARNING: gsp_ga10x.bin not found — GSP will not be available");
    }
    if gsp_bootloader_addr == 0 {
        info!("WARNING: bootloader-535.113.01.bin not found");
    }
    if gsp_booter_load_addr == 0 {
        info!("WARNING: booter_load-535.113.01.bin not found");
    }
    if vbios_addr == 0 {
        info!("WARNING: vbios_rtx3060.rom not found — FWSEC-FRTS will not run");
    }


    // ── 5. Query GOP ────────────────────────────────────────────────────────
    let gop = query_gop().expect("Failed to query GOP");

    // ── 6. Find RSDP ────────────────────────────────────────────────────────
    let rsdp_addr = find_rsdp();

    // ── 7. Allocate kernel stack (256 KiB) ──────────────────────────────────
    let stack_pages = KERNEL_STACK_SIZE / 4096;
    let stack_base = boot::allocate_pages(
        boot::AllocateType::AnyPages,
        MemoryType::LOADER_DATA,
        stack_pages,
    )
    .expect("Failed to allocate kernel stack")
    .as_ptr() as u64;
    // Stack grows downward; top must be 16-byte aligned (guaranteed by page alignment)
    let stack_top = stack_base + KERNEL_STACK_SIZE as u64;
    info!("Stack: base=0x{:x} top=0x{:x}", stack_base, stack_top);

    // ── 8. Allocate and populate BootInfo ────────────────────────────────────
    let boot_info_pages = (core::mem::size_of::<BootInfo>() + 0xFFF) / 0x1000;
    let boot_info_ptr = boot::allocate_pages(
        boot::AllocateType::AnyPages,
        MemoryType::LOADER_DATA,
        boot_info_pages,
    )
    .expect("Failed to allocate BootInfo")
    .as_ptr() as *mut BootInfo;

    // Zero-init, then fill known fields
    unsafe {
        core::ptr::write_bytes(boot_info_ptr, 0, 1);
        let bi = &mut *boot_info_ptr;
        bi.magic = BOOT_MAGIC;

        // Framebuffer
        bi.fb_addr = gop.fb_addr;
        bi.fb_size = gop.fb_size;
        bi.fb_width = gop.width;
        bi.fb_height = gop.height;
        bi.fb_stride = gop.stride;
        bi.fb_pixel_format = gop.pixel_format;

        // ACPI
        bi.rsdp_addr = rsdp_addr;

        // Kernel
        bi.kernel_base = kernel_base;
        bi.kernel_size = kernel_size;

        // Stack
        bi.stack_top = stack_top;
        bi.stack_size = KERNEL_STACK_SIZE as u64;

        // GSP firmware (loaded in step 4b, or 0 if not found)
        bi.gsp_addr = gsp_addr;
        bi.gsp_size = gsp_size;
        bi.gsp_bootloader_addr = gsp_bootloader_addr;
        bi.gsp_bootloader_size = gsp_bootloader_size;
        bi.gsp_booter_load_addr = gsp_booter_load_addr;
        bi.gsp_booter_load_size = gsp_booter_load_size;
        bi.vbios_addr = vbios_addr;
        bi.vbios_size = vbios_size;
    }

    info!("BootInfo at 0x{:x}", boot_info_ptr as u64);

    // ── 9. Exit boot services (with retry) ──────────────────────────────────
    info!("Exiting boot services...");

    // We can no longer log after this point.
    let memory_map = unsafe { boot::exit_boot_services(Some(MemoryType::LOADER_DATA)) };

    // ── 10. Build memory map from UEFI map ──────────────────────────────────
    let bi = unsafe { &mut *boot_info_ptr };
    let mut count: usize = 0;
    for desc in memory_map.entries() {
        if count >= MAX_MEMORY_ENTRIES {
            break;
        }
        bi.memory_map[count] = MemoryEntry {
            base: desc.phys_start,
            size: desc.page_count * 4096,
            mem_type: convert_memory_type(desc.ty),
            _pad: 0,
        };
        count += 1;
    }
    bi.memory_map_count = count as u64;

    // ── 11. Jump to kernel ──────────────────────────────────────────────────
    unsafe {
        core::arch::asm!(
            "mov rsp, {stack}",
            "mov rdi, {boot_info}",
            "jmp {entry}",
            stack = in(reg) stack_top,
            boot_info = in(reg) boot_info_ptr as u64,
            entry = in(reg) entry_point,
            options(noreturn)
        );
    }
}
