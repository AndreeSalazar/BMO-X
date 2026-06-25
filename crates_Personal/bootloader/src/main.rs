//! FastOS UEFI Bootloader v0.3.0
//!
//! Loads kernel.elf from the EFI System Partition, parses ELF64,
//! queries GOP for framebuffer, finds RSDP, builds BootInfo,
//! exits boot services, and jumps to the kernel.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use fastos_boot_protocol::{BootInfoBuilder, MemoryEntry, MemoryType as BootMemType, PixelFormat, MAX_MEMORY_ENTRIES};
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

// ELF64
const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];
const PT_LOAD: u32 = 1;

// Crash log
const CRASH_MARKER_ADDR: u64 = 0x9_0000;
const CRASH_MAGIC: u32 = 0x464F_5343; // "FOSC"

// ── ELF64 ────────────────────────────────────────────────────────────────────

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

    let entry = u64::from_le_bytes(data[24..32].try_into().ok()?);
    let e_phoff = u64::from_le_bytes(data[32..40].try_into().ok()?) as usize;
    let e_phentsize = u16::from_le_bytes(data[54..56].try_into().ok()?) as usize;
    let e_phnum = u16::from_le_bytes(data[56..58].try_into().ok()?) as usize;

    info!("ELF: entry=0x{:x} phnum={}", entry, e_phnum);

    let mut phdrs = Vec::with_capacity(e_phnum);
    for i in 0..e_phnum {
        let off = e_phoff + i * e_phentsize;
        phdrs.push(Elf64Phdr {
            p_type: u32::from_le_bytes(data[off..off + 4].try_into().ok()?),
            p_offset: u64::from_le_bytes(data[off + 8..off + 16].try_into().ok()?),
            p_vaddr: u64::from_le_bytes(data[off + 16..off + 24].try_into().ok()?),
            p_filesz: u64::from_le_bytes(data[off + 32..off + 40].try_into().ok()?),
            p_memsz: u64::from_le_bytes(data[off + 40..off + 48].try_into().ok()?),
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
    let attrs = uefi::runtime::VariableAttributes::BOOTSERVICE_ACCESS
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
    if let Ok(name) = uefi::CStr16::from_str_with_buf("\\EFI\\BOOT\\crash.log", &mut ucs2) {
        if let Ok(mut h) = root.open(name, FileMode::Read, FileAttribute::empty()) {
            if let Some(mut f) = h.into_regular_file() {
                let mut info_buf = [0u8; 256];
                if let Ok(fi) = f.get_info::<FileInfo>(&mut info_buf) {
                    let size = fi.file_size() as usize;
                    if size > 0 && size < 16384 {
                        let mut buf = vec![0u8; size];
                        let _ = f.read(&mut buf);
                        existing.extend_from_slice(&buf);
                    }
                }
            }
        }
    }

    // Build entry
    let mut entry = Vec::new();
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
        write_int(&mut entry, m as u32);
    }
    entry.extend_from_slice(b"\r\n");

    match nvram_result {
        Ok(()) => entry.extend_from_slice(b"NVRAM write: OK\r\n"),
        Err(msg) => {
            entry.extend_from_slice(b"NVRAM write: FAIL - ");
            entry.extend_from_slice(msg.as_bytes());
            entry.extend_from_slice(b"\r\n");
        }
    }

    let mut content = existing;
    content.extend_from_slice(&entry);

    let mut ucs2b = [0u16; 256];
    if let Ok(name) = uefi::CStr16::from_str_with_buf("\\EFI\\BOOT\\crash.log", &mut ucs2b) {
        if let Ok(mut h) = root.open(name, FileMode::CreateReadWrite, FileAttribute::empty()) {
            if let Some(mut f) = h.into_regular_file() {
                let _ = f.set_position(0);
                let _ = f.write(&content);
                info!("crash.log: {} bytes (boot #{})", content.len(), boot_num);
            }
        }
    }
}

fn write_int(buf: &mut Vec<u8>, mut n: u32) {
    if n == 0 { buf.push(b'0'); return; }
    let mut tmp = [0u8; 12];
    let mut len = 0;
    while n > 0 { tmp[len] = b'0' + (n % 10) as u8; n /= 10; len += 1; }
    for i in (0..len).rev() { buf.push(tmp[i]); }
}

// ── GOP ──────────────────────────────────────────────────────────────────────

fn query_gop() -> Option<(u64, u64, u32, u32, u32, PixelFormat)> {
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
    let current = gop.current_mode_info();
    let (cur_w, cur_h) = current.resolution();
    if cur_w != TARGET_FB_WIDTH || cur_h != TARGET_FB_HEIGHT {
        for mode in gop.modes() {
            let info = mode.info();
            let (w, h) = info.resolution();
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
    Some((addr, size, w as u32, h as u32, stride, fmt))
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
    info!("FastOS Bootloader v0.3.0");

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
            info!("WARN: fixed address occupied, using fallback");
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
    let (fb_addr, fb_size, fb_w, fb_h, fb_stride, fb_fmt) = query_gop().expect("GOP failed");

    // Stack
    let stack_pages = KERNEL_STACK_SIZE / 4096;
    let stack_base = boot::allocate_pages(
        boot::AllocateType::AnyPages, MemoryType::LOADER_DATA, stack_pages,
    ).expect("stack alloc failed").as_ptr() as u64;
    let stack_top = stack_base + KERNEL_STACK_SIZE as u64;

    // Build BootInfo using builder
    let mut builder = BootInfoBuilder::new()
        .framebuffer(fb_addr, fb_size, fb_w, fb_h, fb_stride, fb_fmt)
        .rsdp(find_rsdp())
        .kernel(page_base, end - page_base)
        .stack(stack_top, KERNEL_STACK_SIZE as u64)
        .uefi_system_table(
            uefi::table::system_table_raw().map(|p| p.as_ptr() as u64).unwrap_or(0)
        );

    // Exit boot services
    let memory_map = unsafe { boot::exit_boot_services(Some(MemoryType::LOADER_DATA)) };

    if needs_reloc {
        unsafe {
            core::ptr::copy_nonoverlapping(kernel_ptr, page_base as *mut u8, total_pages * 4096);
        }
    }

    // Build memory map
    let mut bi = builder.build();
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

    // Jump
    unsafe {
        core::arch::asm!(
            "mov rsp, {stack}",
            "mov rdi, {info}",
            "jmp {entry}",
            stack = in(reg) stack_top,
            info = in(reg) &bi as *const _ as u64,
            entry = in(reg) entry_point,
            options(noreturn)
        );
    }
}
