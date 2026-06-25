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

const KERNEL_STACK_SIZE: usize = 4 * 1024 * 1024; // 4 MiB (v1.8.17: was 256 KiB)
const TARGET_FB_WIDTH: usize = 1920;
const TARGET_FB_HEIGHT: usize = 1080;
const TARGET_REFRESH_HZ: u32 = 74;

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

// ── NVRAM Variable Access ──────────────────────────────────────────────────
//
// Before exit_boot_services, we use UEFI Runtime Services to access NVRAM.
// The vendor GUID matches the one the kernel uses for Runtime Services.

/// FastOS vendor GUID for NVRAM variables.
fn fastos_vendor_guid() -> uefi::runtime::VariableVendor {
    uefi::runtime::VariableVendor(uefi::Guid::from_bytes([
        0x01, 0x00, 0xA5, 0xF1,
        0x02, 0x00,
        0x03, 0x00,
        0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
    ]))
}

/// Read a UEFI NVRAM variable using runtime services.
fn read_nvram_variable(_device_handle: uefi::Handle, name: &str) -> Option<alloc::string::String> {
    let name_cstr = uefi::CString16::try_from(name).ok()?;
    let guid = fastos_vendor_guid();
    let mut buf = [0u8; 256];

    match uefi::runtime::get_variable(&name_cstr, &guid, &mut buf) {
        Ok((data, _attrs)) => {
            let mut result = alloc::string::String::new();
            for &b in data.iter() {
                if b == 0 { break; }
                result.push(b as char);
            }
            if result.is_empty() { None } else { Some(result) }
        }
        Err(_) => None,
    }
}

/// Write a UEFI NVRAM variable using runtime services.
fn write_nvram_variable(_device_handle: uefi::Handle, name: &str, value: &str) {
    let name_cstr = match uefi::CString16::try_from(name) {
        Ok(c) => c,
        Err(_) => return,
    };
    let guid = fastos_vendor_guid();
    let data = value.as_bytes();
    let attrs = uefi::runtime::VariableAttributes::BOOTSERVICE_ACCESS
        | uefi::runtime::VariableAttributes::RUNTIME_ACCESS;

    match uefi::runtime::set_variable(&name_cstr, &guid, attrs, data) {
        Ok(()) => info!("NVRAM: set {}={}", name, value),
        Err(e) => info!("NVRAM: set {} failed: {:?}", name, e.status()),
    }
}

// ── Crash Log ──────────────────────────────────────────────────────────────
//
// Strategy:
//   1. Kernel writes a crash marker to physical address 0x90000
//   2. On next boot, bootloader reads 0x90000 (may survive warm reset)
//   3. Bootloader ALWAYS writes crash.log to USB with boot info
//   4. User reads crash.log from Windows to see what happened
//
// Physical memory at 0x90000 may or may not survive an AMD watchdog reset.
// If it does, we get the exact crash stage. If not, we still get a log file.

const CRASH_MARKER_ADDR: u64 = 0x9_0000;
const CRASH_MAGIC: u32 = 0x464F_5343; // "FOSC"

/// Read the crash marker from physical address 0x90000.
fn read_crash_marker() -> Option<u32> {
    let magic = unsafe { core::ptr::read_volatile(CRASH_MARKER_ADDR as *const u32) };
    if magic == CRASH_MAGIC {
        let stage = unsafe { core::ptr::read_volatile((CRASH_MARKER_ADDR + 4) as *const u32) };
        Some(stage)
    } else {
        None
    }
}

/// Clear the crash marker at physical address 0x90000.
fn clear_crash_marker() {
    unsafe {
        core::ptr::write_volatile(CRASH_MARKER_ADDR as *mut u32, 0);
        core::ptr::write_volatile((CRASH_MARKER_ADDR + 4) as *mut u32, 0);
    }
}

/// Increment boot counter stored at physical address 0x90008.
/// If memory survives the reset, this counter persists.
fn read_and_increment_boot_counter() -> u32 {
    let counter_addr = CRASH_MARKER_ADDR + 8;
    let val = unsafe { core::ptr::read_volatile(counter_addr as *const u32) };
    let next = val.wrapping_add(1);
    unsafe { core::ptr::write_volatile(counter_addr as *mut u32, next) };
    next
}

/// Write crash log to USB filesystem. Called on every boot.
fn write_crash_log(
    device_handle: uefi::Handle,
    nvram_stage: Option<&str>,
    ram_marker: Option<u32>,
    boot_num: u32,
) {
    use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode};

    let Ok(mut fs) = boot::open_protocol_exclusive::<SimpleFileSystem>(device_handle) else {
        info!("crash.log: cannot open filesystem");
        return;
    };
    let Ok(mut root) = fs.open_volume() else {
        info!("crash.log: cannot open volume");
        return;
    };

    // Read existing log (append mode)
    let mut existing = Vec::new();
    let mut ucs2_buf = [0u16; 256];
    if let Ok(ucs2_name) = uefi::CStr16::from_str_with_buf(
        "\\EFI\\BOOT\\crash.log", &mut ucs2_buf,
    ) {
        if let Ok(mut handle) = root.open(ucs2_name, FileMode::Read, FileAttribute::empty()) {
            if let Some(mut file) = handle.into_regular_file() {
                let mut info_buf = [0u8; 256];
                if let Ok(finfo) = file.get_info::<FileInfo>(&mut info_buf) {
                    let size = finfo.file_size() as usize;
                    if size > 0 && size < 16384 {
                        let mut buf = vec![0u8; size];
                        let _ = file.read(&mut buf);
                        existing.extend_from_slice(&buf);
                    }
                }
            }
        }
    }

    // Build new entry
    let mut entry = Vec::new();
    entry.extend_from_slice(b"[Boot #");

    // Integer to string
    let mut tmp = [0u8; 12];
    let mut pos = 0;
    let mut n = boot_num;
    if n == 0 { tmp[0] = b'0'; pos = 1; }
    else {
        while n > 0 { tmp[pos] = b'0' + (n % 10) as u8; n /= 10; pos += 1; }
        tmp[..pos].reverse();
    }
    entry.extend_from_slice(&tmp[..pos]);
    entry.extend_from_slice(b"] ");

    // NVRAM stage (survives resets — most reliable)
    match nvram_stage {
        Some("ok") => {
            entry.extend_from_slice(b"OK (welcome reached)");
        }
        Some(stage) => {
            entry.extend_from_slice(b"CRASH at: ");
            entry.extend_from_slice(stage.as_bytes());
        }
        None => {
            entry.extend_from_slice(b"no NVRAM data");
        }
    }

    // Physical RAM marker (may or may not survive)
    if let Some(stage) = ram_marker {
        entry.extend_from_slice(b" | RAM=");
        let mut sbuf = [0u8; 12];
        let mut spos = 0;
        let mut sn = stage;
        if sn == 0 { sbuf[0] = b'0'; spos = 1; }
        else {
            while sn > 0 { sbuf[spos] = b'0' + (sn % 10) as u8; sn /= 10; spos += 1; }
            sbuf[..spos].reverse();
        }
        entry.extend_from_slice(&sbuf[..spos]);
    }

    entry.extend_from_slice(b"\r\n");

    // Append to existing
    let mut content = existing;
    content.extend_from_slice(&entry);

    // Write file
    let mut ucs2_buf2 = [0u16; 256];
    if let Ok(ucs2_name) = uefi::CStr16::from_str_with_buf(
        "\\EFI\\BOOT\\crash.log", &mut ucs2_buf2,
    ) {
        match root.open(ucs2_name, FileMode::CreateReadWrite, FileAttribute::empty()) {
            Ok(mut handle) => {
                if let Some(mut file) = handle.into_regular_file() {
                    let _ = file.set_position(0);
                    let _ = file.write(&content);
                    info!("crash.log: written {} bytes (boot #{})", content.len(), boot_num);
                }
            }
            Err(e) => {
                info!("crash.log: create failed: {:?}", e.status());
            }
        }
    }
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

fn prefer_full_hd_gop_mode(gop: &mut GraphicsOutput) {
    use uefi::proto::console::gop::PixelFormat as GopPixelFormat;

    info!(
        "Selecting GOP mode: prefer {}x{}; target refresh {} Hz is handled by firmware/monitor",
        TARGET_FB_WIDTH, TARGET_FB_HEIGHT, TARGET_REFRESH_HZ
    );

    let current = gop.current_mode_info();
    let (cur_w, cur_h) = current.resolution();
    if cur_w == TARGET_FB_WIDTH && cur_h == TARGET_FB_HEIGHT {
        info!("GOP already at preferred mode: {}x{}", cur_w, cur_h);
        return;
    }

    let mut best_exact = None;
    let mut best_any = None;

    for mode in gop.modes() {
        let info = mode.info();
        let (w, h) = info.resolution();
        if w != TARGET_FB_WIDTH || h != TARGET_FB_HEIGHT {
            continue;
        }

        match info.pixel_format() {
            GopPixelFormat::Bgr | GopPixelFormat::Rgb => {
                best_exact = Some(mode);
                break;
            }
            _ => {
                if best_any.is_none() {
                    best_any = Some(mode);
                }
            }
        }
    }

    let Some(mode) = best_exact.or(best_any) else {
        info!(
            "WARNING: GOP mode {}x{} not exposed; keeping firmware mode {}x{}",
            TARGET_FB_WIDTH, TARGET_FB_HEIGHT, cur_w, cur_h
        );
        return;
    };

    let info = *mode.info();
    let (w, h) = info.resolution();
    let stride = info.stride();
    let fmt = info.pixel_format();

    match gop.set_mode(&mode) {
        Ok(()) => info!("GOP mode set: {}x{} stride={} fmt={:?}", w, h, stride, fmt),
        Err(e) => info!("WARNING: failed to set GOP {}x{}: {:?}", w, h, e.status()),
    }
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

    prefer_full_hd_gop_mode(&mut gop);

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

fn paint_bootloader_marker(gop: &GopInfo, stage: u32) {
    if gop.fb_addr == 0 || gop.width == 0 || gop.height == 0 || gop.stride == 0 {
        return;
    }

    let fb = gop.fb_addr as *mut u32;
    let width = gop.width as usize;
    let stride = gop.stride as usize;
    let y0 = 44usize + (stage as usize * 10);

    for y in y0..(y0 + 8).min(gop.height as usize) {
        for x in 0..width.min(640) {
            unsafe { fb.add(y * stride + x).write_volatile(0xFF00FF00); }
        }
    }
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

    // UEFI arms a firmware watchdog when it launches an EFI image. If it
    // remains active after our handoff, the firmware can reset the PC without
    // any Ring 0 watchdog message. Disable it as soon as boot services exist.
    // Try multiple times — some AMD firmware ignores the first call.
    for attempt in 0..5 {
        match boot::set_watchdog_timer(0, 0x1_0000, None) {
            Ok(()) => info!("UEFI watchdog disabled (attempt {})", attempt + 1),
            Err(e) => info!("WARNING: failed to disable UEFI watchdog (attempt {}): {:?}", attempt + 1, e.status()),
        }
    }

    // ── 1. Get boot device handle via LoadedImage ───────────────────────────
    info!("Getting boot device handle...");
    let loaded_image =
        boot::open_protocol_exclusive::<LoadedImage>(boot::image_handle()).unwrap();
    let device_handle = loaded_image
        .device()
        .expect("LoadedImage has no device handle");
    drop(loaded_image); // release protocol before opening FS on same device

    // ── Crash log: read NVRAM variable from previous boot ──────────────────
    // UEFI NVRAM variables survive warm resets (unlike physical RAM).
    // The kernel writes "FastOSBootStage" at each phase. If the system
    // reboots, the variable persists and we can see where it died.
    let prev_stage = read_nvram_variable(device_handle, "FastOSBootStage");

    // Also try physical memory (backup, may or may not survive)
    let crash_marker = read_crash_marker();

    let boot_num = read_and_increment_boot_counter();

    info!("Boot #{} — NVRAM stage: {:?} — RAM marker: {:?}",
        boot_num, prev_stage.as_deref(), crash_marker);

    // Write crash.log to USB
    write_crash_log(device_handle, prev_stage.as_deref(), crash_marker, boot_num);

    // Set current stage
    write_nvram_variable(device_handle, "FastOSBootStage", "bootloader");

    // Clear physical memory marker
    clear_crash_marker();

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

    // ── 4. Load PT_LOAD segments into memory ────────────────────────────────
    let kernel_buffer_ptr: *mut u8;
    let mut needs_relocation = false;

    // Try allocating at the exact fixed address first. If the firmware has this
    // region reserved (firmware conflict), allocate temporary pages and copy after exit_boot_services.
    match boot::allocate_pages(
        boot::AllocateType::Address(kernel_page_base),
        MemoryType::LOADER_CODE,
        total_pages,
    ) {
        Ok(addr) => {
            info!("Allocated kernel pages at fixed address 0x{:x}", kernel_page_base);
            kernel_buffer_ptr = addr.as_ptr() as *mut u8;
        }
        Err(_) => {
            info!("WARN: Fixed address 0x{:x} occupied. Allocating fallback temporary pages...", kernel_page_base);
            let temp_addr = boot::allocate_pages(
                boot::AllocateType::AnyPages,
                MemoryType::LOADER_DATA,
                total_pages,
            )
            .expect("Failed to allocate temporary pages for kernel fallback");
            kernel_buffer_ptr = temp_addr.as_ptr() as *mut u8;
            needs_relocation = true;
            info!("Temporary buffer allocated at 0x{:x}", kernel_buffer_ptr as u64);
        }
    }

    // Second pass — copy each segment into the selected buffer
    for phdr in &elf.phdrs {
        if phdr.p_type != PT_LOAD || phdr.p_memsz == 0 {
            continue;
        }

        let seg_start = phdr.p_vaddr;
        let seg_end = seg_start + phdr.p_memsz;

        // Calculate destination relative to the allocated buffer
        let dst_offset = seg_start - kernel_page_base;
        let dst = unsafe { kernel_buffer_ptr.add(dst_offset as usize) };

        let page_base = seg_start & !0xFFF;
        let pages = ((seg_end - page_base + 0xFFF) / 0x1000) as usize;

        info!(
            "PT_LOAD: vaddr=0x{:x} filesz=0x{:x} memsz=0x{:x} pages={} dst=0x{:x}",
            seg_start, phdr.p_filesz, phdr.p_memsz, pages, dst as u64
        );

        unsafe {
            // Copy file data
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

    // ── 4b. GPU firmware path disabled ─────────────────────────────────────
    // FastOS debe arrancar con UEFI GOP/framebuffer sin blobs privados ni
    // archivos de una GPU concreta. Los campos legacy del BootInfo se dejan en
    // cero por compatibilidad con el layout del protocolo.
    info!("GPU firmware loading disabled; using UEFI GOP framebuffer only");


    // ── 5. Query GOP ────────────────────────────────────────────────────────
    let gop = query_gop().expect("Failed to query GOP");
    paint_bootloader_marker(&gop, 0);

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

        // Reserved payload: intentionally zero en el GOP path.
        bi.reserved_addr = 0;
        bi.reserved_size = 0;

        // UEFI System Table pointer — kernel uses Runtime Services for NVRAM
        bi.uefi_system_table = uefi::table::system_table_raw()
            .map(|p| p.as_ptr() as u64)
            .unwrap_or(0);
    }

    info!("BootInfo at 0x{:x}", boot_info_ptr as u64);
    paint_bootloader_marker(&gop, 1);

    // ── 9. Exit boot services (with retry) ──────────────────────────────────
    info!("Exiting boot services...");

    // We can no longer log after this point.
    let memory_map = unsafe { boot::exit_boot_services(Some(MemoryType::LOADER_DATA)) };

    // Relocate kernel to its fixed link address if we used a fallback buffer
    if needs_relocation {
        unsafe {
            core::ptr::copy_nonoverlapping(
                kernel_buffer_ptr,
                kernel_page_base as *mut u8,
                total_pages * 4096,
            );
        }
    }

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
    paint_bootloader_marker(&gop, 2);

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
