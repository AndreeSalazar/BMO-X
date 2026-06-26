//! FastOS UEFI Bootloader v0.4.0
//!
//! Loads kernel.elf from the EFI System Partition, parses ELF64,
//! queries GOP for framebuffer, finds RSDP, builds BootInfo,
//! exits boot services, and jumps to the kernel.
//!
//! v0.4.0: Stack-safe BootInfo allocation, ELF bounds checking,
//!         GOP struct, cleaner crash diagnostics.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use fastos_boot_protocol::{BootInfo, MemoryEntry, MemoryType as BootMemType, PixelFormat, MAX_MEMORY_ENTRIES};
use log::info;
use uefi::boot;
use uefi::mem::memory_map::{MemoryMap, MemoryType};
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::media::fs::SimpleFileSystem;

// ── Constants ────────────────────────────────────────────────────────────────

const KERNEL_STACK_SIZE: usize = 4 * 1024 * 1024;
const TARGET_FB_WIDTH: usize = 1920;
const TARGET_FB_HEIGHT: usize = 1080;

const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];
const PT_LOAD: u32 = 1;

const CRASH_MARKER_ADDR: u64 = 0x9_0000;
const CRASH_MAGIC: u32 = 0x464F_5343; // "FOSC"

const CRASH_LOG_MAX: usize = 16384;
const CRASH_LOG_PATH: &str = "\\EFI\\BOOT\\crash.log";

// ── GOP Info ─────────────────────────────────────────────────────────────────

struct GopInfo {
    fb_addr: u64,
    fb_size: u64,
    width: u32,
    height: u32,
    stride: u32,
    pixel_format: PixelFormat,
}

// ── ELF64 ────────────────────────────────────────────────────────────────────

fn read_u16(data: &[u8], off: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(off..off + 2)?.try_into().ok()?))
}

fn read_u32(data: &[u8], off: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(off..off + 4)?.try_into().ok()?))
}

fn read_u64(data: &[u8], off: usize) -> Option<u64> {
    Some(u64::from_le_bytes(data.get(off..off + 8)?.try_into().ok()?))
}

struct Elf64Phdr {
    p_type: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_filesz: u64,
    p_memsz: u64,
}

fn parse_elf64(data: &[u8]) -> Option<(u64, Vec<Elf64Phdr>)> {
    if data.len() < 64 || data[0..4] != ELF_MAGIC {
        info!("ELF: bad magic");
        return None;
    }
    if data[4] != 2 {
        info!("ELF: not 64-bit");
        return None;
    }

    let entry = read_u64(data, 24)?;
    let e_phoff = read_u64(data, 32)? as usize;
    let e_phentsize = read_u16(data, 54)? as usize;
    let e_phnum = read_u16(data, 56)? as usize;

    if e_phentsize == 0 || e_phnum == 0 {
        info!("ELF: no program headers");
        return None;
    }

    info!("ELF: entry=0x{:x} phnum={}", entry, e_phnum);

    let mut phdrs = Vec::with_capacity(e_phnum);
    for i in 0..e_phnum {
        let off = e_phoff.checked_add(i.checked_mul(e_phentsize)?)?;
        // Bounds check: header must fit within data
        if off + e_phentsize > data.len() {
            info!("ELF: phdr {} out of bounds (off={} size={} data={})", i, off, e_phentsize, data.len());
            return None;
        }
        phdrs.push(Elf64Phdr {
            p_type: read_u32(data, off)?,
            p_offset: read_u64(data, off + 8)?,
            p_vaddr: read_u64(data, off + 16)?,
            p_filesz: read_u64(data, off + 32)?,
            p_memsz: read_u64(data, off + 40)?,
        });
    }

    Some((entry, phdrs))
}

// ── File I/O ─────────────────────────────────────────────────────────────────

fn read_file(handle: uefi::Handle, path: &str) -> Option<Vec<u8>> {
    use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode};

    let mut fs = boot::open_protocol_exclusive::<SimpleFileSystem>(handle).ok()?;
    let mut root = fs.open_volume().ok()?;

    let mut ucs2 = [0u16; 256];
    let name = uefi::CStr16::from_str_with_buf(path, &mut ucs2).ok()?;
    let mut file = root.open(name, FileMode::Read, FileAttribute::empty())
        .ok()?.into_regular_file()?;

    let mut info_buf = [0u8; 256];
    let size = file.get_info::<FileInfo>(&mut info_buf).ok()?.file_size() as usize;
    if size > 16 * 1024 * 1024 {
        info!("file too large: {} bytes", size);
        return None;
    }

    let mut buf = vec![0u8; size];
    file.read(&mut buf).ok()?;
    Some(buf)
}

// ── NVRAM ────────────────────────────────────────────────────────────────────

fn nvram_get(name: &str) -> Option<alloc::string::String> {
    let cstr = uefi::CString16::try_from(name).ok()?;
    let mut buf = [0u8; 256];
    let (data, _) = uefi::runtime::get_variable(
        &cstr, &uefi::runtime::VariableVendor::GLOBAL_VARIABLE, &mut buf,
    ).ok()?;
    let len = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    if len == 0 { return None; }
    Some(alloc::string::String::from_utf8_lossy(&data[..len]).into_owned())
}

fn nvram_set(name: &str, value: &str) -> Result<(), alloc::string::String> {
    let cstr = uefi::CString16::try_from(name)
        .map_err(|e| alloc::format!("CString16: {:?}", e))?;
    let attrs = uefi::runtime::VariableAttributes::NON_VOLATILE
        | uefi::runtime::VariableAttributes::BOOTSERVICE_ACCESS
        | uefi::runtime::VariableAttributes::RUNTIME_ACCESS;
    uefi::runtime::set_variable(
        &cstr, &uefi::runtime::VariableVendor::GLOBAL_VARIABLE, attrs, value.as_bytes(),
    ).map_err(|e| alloc::format!("SetVariable: {:?}", e.status()))
}

// ── Crash Log ────────────────────────────────────────────────────────────────

fn read_crash_marker() -> Option<u32> {
    let magic = unsafe { core::ptr::read_volatile(CRASH_MARKER_ADDR as *const u32) };
    if magic == CRASH_MAGIC {
        Some(unsafe { core::ptr::read_volatile((CRASH_MARKER_ADDR + 4) as *const u32) })
    } else {
        None
    }
}

fn clear_crash_marker() {
    unsafe {
        core::ptr::write_volatile(CRASH_MARKER_ADDR as *mut u32, 0);
        core::ptr::write_volatile((CRASH_MARKER_ADDR + 4) as *mut u32, 0);
    }
}

fn read_and_increment_boot_counter() -> u32 {
    let addr = CRASH_MARKER_ADDR + 8;
    let val = unsafe { core::ptr::read_volatile(addr as *const u32) };
    let next = val.wrapping_add(1);
    unsafe { core::ptr::write_volatile(addr as *mut u32, next) };
    next
}

fn write_int(buf: &mut Vec<u8>, mut n: u32) {
    if n == 0 { buf.push(b'0'); return; }
    let mut tmp = [0u8; 12];
    let mut len = 0;
    while n > 0 { tmp[len] = b'0' + (n % 10) as u8; n /= 10; len += 1; }
    for i in (0..len).rev() { buf.push(tmp[i]); }
}

fn nvram_status_ok(result: &Result<(), alloc::string::String>) -> &'static [u8] {
    match result { Ok(()) => b"OK", Err(_) => b"FAIL" }
}

fn write_crash_log(
    handle: uefi::Handle,
    nvram_stage: Option<&str>,
    ram_marker: Option<u32>,
    boot_num: u32,
    nvram_result: &Result<(), alloc::string::String>,
) {
    use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode};

    let Ok(mut fs) = boot::open_protocol_exclusive::<SimpleFileSystem>(handle) else { return };
    let Ok(mut root) = fs.open_volume() else { return };

    // Read existing log
    let mut existing = Vec::new();
    let mut ucs2 = [0u16; 256];
    if let Ok(name) = uefi::CStr16::from_str_with_buf(CRASH_LOG_PATH, &mut ucs2) {
        if let Ok(mut h) = root.open(name, FileMode::Read, FileAttribute::empty()) {
            if let Some(mut f) = h.into_regular_file() {
                let mut info_buf = [0u8; 256];
                if let Ok(fi) = f.get_info::<FileInfo>(&mut info_buf) {
                    let size = (fi.file_size() as usize).min(CRASH_LOG_MAX);
                    if size > 0 {
                        let mut buf = vec![0u8; size];
                        let _ = f.read(&mut buf);
                        existing.extend_from_slice(&buf);
                    }
                }
            }
        }
    }

    // Build entry: [Boot #N] STAGE | RAM=X | NVRAM: OK/FAIL
    let mut entry = Vec::with_capacity(128);
    entry.extend_from_slice(b"[Boot #");
    write_int(&mut entry, boot_num);
    entry.extend_from_slice(b"] ");

    match nvram_stage {
        Some("ok") => entry.extend_from_slice(b"OK (welcome reached)"),
        Some(stage) => {
            entry.extend_from_slice(b"CRASH at: ");
            entry.extend_from_slice(stage.as_bytes());
        }
        None => entry.extend_from_slice(b"no NVRAM data"),
    }

    if let Some(m) = ram_marker {
        entry.extend_from_slice(b" | RAM=");
        write_int(&mut entry, m);
    }

    entry.extend_from_slice(b" | NVRAM: ");
    entry.extend_from_slice(nvram_status_ok(nvram_result));
    if let Err(msg) = nvram_result {
        entry.extend_from_slice(b" (");
        entry.extend_from_slice(msg.as_bytes());
        entry.extend_from_slice(b")");
    }
    entry.extend_from_slice(b"\r\n");

    // Append + truncate if over limit
    let mut content = existing;
    content.extend_from_slice(&entry);
    if content.len() > CRASH_LOG_MAX {
        let trim = content.len() - CRASH_LOG_MAX;
        content.drain(..trim);
        content.extend_from_slice(b"[...truncated...]\r\n");
    }

    let mut ucs2b = [0u16; 256];
    if let Ok(name) = uefi::CStr16::from_str_with_buf(CRASH_LOG_PATH, &mut ucs2b) {
        if let Ok(mut h) = root.open(name, FileMode::CreateReadWrite, FileAttribute::empty()) {
            if let Some(mut f) = h.into_regular_file() {
                let _ = f.set_position(0);
                let _ = f.write(&content);
                info!("crash.log: {} bytes (boot #{})", content.len(), boot_num);
            }
        }
    }
}

// ── GOP ──────────────────────────────────────────────────────────────────────

fn query_gop() -> Option<GopInfo> {
    use uefi::proto::console::gop::PixelFormat as GopFmt;

    let gop = boot::open_protocol_exclusive::<GraphicsOutput>(boot::image_handle());
    let mut gop = match gop {
        Ok(g) => g,
        Err(_) => {
            let handles = boot::locate_handle_buffer(boot::SearchType::ByProtocol(
                &<GraphicsOutput as uefi::Identify>::GUID,
            )).ok()?;
            let h = *handles.first()?;
            boot::open_protocol_exclusive::<GraphicsOutput>(h).ok()?
        }
    };

    // Prefer 1920x1080
    let (cur_w, cur_h) = gop.current_mode_info().resolution();
    if cur_w != TARGET_FB_WIDTH || cur_h != TARGET_FB_HEIGHT {
        for mode in gop.modes() {
            let (w, h) = mode.info().resolution();
            if w == TARGET_FB_WIDTH && h == TARGET_FB_HEIGHT {
                let _ = gop.set_mode(&mode);
                break;
            }
        }
    }

    let mode_info = gop.current_mode_info();
    let (w, h) = mode_info.resolution();
    let stride = mode_info.stride() as u32;
    let fmt = match mode_info.pixel_format() {
        GopFmt::Bgr => PixelFormat::Bgr,
        GopFmt::Rgb => PixelFormat::Rgb,
        _ => PixelFormat::Unknown,
    };

    let mut fb = gop.frame_buffer();
    let addr = fb.as_mut_ptr() as u64;
    let size = fb.size() as u64;

    info!("GOP: {}x{} stride={} fb=0x{:x}", w, h, stride, addr);
    Some(GopInfo { fb_addr: addr, fb_size: size, width: w as u32, height: h as u32, stride, pixel_format: fmt })
}

// ── Memory type conversion ───────────────────────────────────────────────────

fn convert_memory_type(ty: MemoryType) -> BootMemType {
    match ty {
        MemoryType::CONVENTIONAL => BootMemType::Usable,
        MemoryType::LOADER_CODE | MemoryType::LOADER_DATA => BootMemType::Bootloader,
        MemoryType::BOOT_SERVICES_CODE | MemoryType::BOOT_SERVICES_DATA => BootMemType::Usable,
        MemoryType::ACPI_RECLAIM => BootMemType::AcpiReclaimable,
        MemoryType::ACPI_NON_VOLATILE => BootMemType::AcpiNvs,
        _ => BootMemType::Reserved,
    }
}

// ── RSDP ─────────────────────────────────────────────────────────────────────

fn find_rsdp() -> u64 {
    use uefi::table::cfg::ConfigTableEntry;
    uefi::system::with_config_table(|config| {
        for entry in config {
            if entry.guid == ConfigTableEntry::ACPI2_GUID || entry.guid == ConfigTableEntry::ACPI_GUID {
                return entry.address as u64;
            }
        }
        0
    })
}

// ── Entry ────────────────────────────────────────────────────────────────────

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    info!("FastOS Bootloader v0.4.0");

    // Disable UEFI firmware watchdog
    for _ in 0..5 {
        let _ = boot::set_watchdog_timer(0, 0x1_0000, None);
    }

    // Get boot device
    let loaded_image = boot::open_protocol_exclusive::<LoadedImage>(boot::image_handle()).unwrap();
    let device = loaded_image.device().expect("no device handle");
    drop(loaded_image);

    // Crash diagnostics
    let prev_stage = nvram_get("FastOSBootStage");
    let ram_marker = read_crash_marker();
    let boot_num = read_and_increment_boot_counter();
    let nvram_result = nvram_set("FastOSBootStage", "bootloader");

    info!("Boot #{} — NVRAM: {:?} — RAM: {:?}", boot_num, prev_stage.as_deref(), ram_marker);
    write_crash_log(device, prev_stage.as_deref(), ram_marker, boot_num, &nvram_result);
    clear_crash_marker();

    // Load kernel
    let elf_data = read_file(device, "\\EFI\\BOOT\\kernel.elf").expect("failed to read kernel.elf");
    let (entry_point, phdrs) = parse_elf64(&elf_data).expect("failed to parse ELF64");

    // Compute kernel span
    let mut base: u64 = u64::MAX;
    let mut end: u64 = 0;
    for ph in &phdrs {
        if ph.p_type != PT_LOAD || ph.p_memsz == 0 { continue; }
        base = base.min(ph.p_vaddr);
        end = end.max(ph.p_vaddr + ph.p_memsz);
    }
    let page_base = base & !0xFFF;
    let total_pages = ((end - page_base + 0xFFF) / 0x1000) as usize;

    // Allocate and load
    let (kernel_ptr, needs_reloc) = match boot::allocate_pages(
        boot::AllocateType::Address(page_base), MemoryType::LOADER_CODE, total_pages,
    ) {
        Ok(addr) => (addr.as_ptr() as *mut u8, false),
        Err(_) => {
            info!("WARN: fixed address 0x{:x} occupied, using fallback", page_base);
            let tmp = boot::allocate_pages(
                boot::AllocateType::AnyPages, MemoryType::LOADER_DATA, total_pages,
            ).expect("alloc failed");
            (tmp.as_ptr() as *mut u8, true)
        }
    };

    for ph in &phdrs {
        if ph.p_type != PT_LOAD || ph.p_memsz == 0 { continue; }
        let dst = unsafe { kernel_ptr.add((ph.p_vaddr - page_base) as usize) };
        unsafe {
            core::ptr::copy_nonoverlapping(elf_data.as_ptr().add(ph.p_offset as usize), dst, ph.p_filesz as usize);
            core::ptr::write_bytes(dst.add(ph.p_filesz as usize), 0, (ph.p_memsz - ph.p_filesz) as usize);
        }
    }

    // GOP
    let gop = query_gop().expect("GOP failed");

    // Stack
    let stack_pages = KERNEL_STACK_SIZE / 4096;
    let stack_base = boot::allocate_pages(
        boot::AllocateType::AnyPages, MemoryType::LOADER_DATA, stack_pages,
    ).expect("stack alloc failed").as_ptr() as u64;
    let stack_top = stack_base + KERNEL_STACK_SIZE as u64;

    // Build BootInfo on heap (it's ~8KB, too large for UEFI loader stack)
    let boot_info_ptr = boot::allocate_pages(
        boot::AllocateType::AnyPages, MemoryType::LOADER_DATA, 2, // 8 KiB = 2 pages
    ).expect("BootInfo alloc failed").as_ptr() as *mut BootInfo;

    unsafe {
        core::ptr::write_bytes(boot_info_ptr, 0, 1);
        let bi = &mut *boot_info_ptr;
        bi.magic = fastos_boot_protocol::BOOT_MAGIC;
        bi.version = fastos_boot_protocol::PROTOCOL_VERSION;
        bi.fb_addr = gop.fb_addr;
        bi.fb_size = gop.fb_size;
        bi.fb_width = gop.width;
        bi.fb_height = gop.height;
        bi.fb_stride = gop.stride;
        bi.fb_pixel_format = gop.pixel_format;
        bi.rsdp_addr = find_rsdp();
        bi.kernel_base = page_base;
        bi.kernel_size = end - page_base;
        bi.stack_top = stack_top;
        bi.stack_size = KERNEL_STACK_SIZE as u64;
        bi.uefi_system_table = uefi::table::system_table_raw()
            .map(|p| p.as_ptr() as u64).unwrap_or(0);
    }

    // Exit boot services
    let memory_map = unsafe { boot::exit_boot_services(Some(MemoryType::LOADER_DATA)) };

    if needs_reloc {
        unsafe {
            core::ptr::copy_nonoverlapping(kernel_ptr, page_base as *mut u8, total_pages * 4096);
        }
    }

    // Fill memory map into BootInfo (after EBS, memory addresses are final)
    let bi = unsafe { &mut *boot_info_ptr };
    let mut count: u32 = 0;
    for desc in memory_map.entries() {
        if count >= MAX_MEMORY_ENTRIES as u32 { break; }
        bi.memory_map[count as usize] = MemoryEntry {
            base: desc.phys_start,
            size: desc.page_count * 4096,
            mem_type: convert_memory_type(desc.ty),
            _pad: 0,
        };
        count += 1;
    }
    bi.memory_map_count = count;

    info!("BootInfo at 0x{:x} entries={}", boot_info_ptr as u64, count);

    // Jump to kernel
    unsafe {
        core::arch::asm!(
            "mov rsp, {stack}",
            "mov rdi, {info}",
            "jmp {entry}",
            stack = in(reg) stack_top,
            info = in(reg) boot_info_ptr as u64,
            entry = in(reg) entry_point,
            options(noreturn)
        );
    }
}
