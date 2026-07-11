//! BMO UEFI Bootloader v0.5.0
//!
//! Loads kernel.elf + Ring 3 modules from the EFI System Partition,
//! parses ELF64, queries GOP, finds RSDP, builds BootInfo,
//! exits boot services, and jumps to the kernel.
//!
//! v0.5.0: Module pre-loading — bootloader loads mod_bmo_core.elf,
//!         mod_timeback.elf, mod_cabina.elf from \EFI\BOOT\modules\
//!         before ExitBootServices.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use bmo_boot_protocol::{BootInfo, MemoryEntry, MemoryType as BootMemType, ModuleEntry, PixelFormat, MAX_MEMORY_ENTRIES, MAX_MODULES};
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

const MODULE_PATHS: &[&str] = &[
    "\\EFI\\BOOT\\modules\\mod_bmo_core.elf",
    "\\EFI\\BOOT\\modules\\mod_timeback.elf",
    "\\EFI\\BOOT\\modules\\mod_cabina.elf",
    "\\EFI\\BOOT\\modules\\mod_linux_devour.elf",
    "\\EFI\\BOOT\\modules\\mod_wine_devour.elf",
];

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



/// Load a single module ELF: read, parse, allocate pages, copy segments,
/// zero BSS. Returns ModuleEntry or None on failure.
fn load_module(device: uefi::Handle, path: &str) -> Option<ModuleEntry> {
    let data = read_file(device, path)?;
    let (entry_point, phdrs) = parse_elf64(&data)?;

    let mut base: u64 = u64::MAX;
    let mut end: u64 = 0;
    for ph in &phdrs {
        if ph.p_type != PT_LOAD || ph.p_memsz == 0 { continue; }
        base = base.min(ph.p_vaddr);
        end = end.max(ph.p_vaddr + ph.p_memsz);
    }
    if end == 0 { return None; }

    let page_base = base & !0xFFF;
    let total_pages = ((end - page_base + 0xFFF) / 0x1000) as usize;

    info!("module {}: vaddr=0x{:x}-0x{:x} ({} pages)", path, base, end, total_pages);

    let ptr = match boot::allocate_pages(
        boot::AllocateType::Address(page_base), MemoryType::LOADER_CODE, total_pages,
    ) {
        Ok(addr) => addr.as_ptr() as *mut u8,
        Err(e) => {
            info!("module {}: alloc at 0x{:x} failed: {:?}", path, page_base, e);
            return None;
        }
    };

    for ph in &phdrs {
        if ph.p_type != PT_LOAD || ph.p_memsz == 0 { continue; }
        let dst = unsafe { ptr.add((ph.p_vaddr - page_base) as usize) };
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr().add(ph.p_offset as usize), dst, ph.p_filesz as usize);
            core::ptr::write_bytes(dst.add(ph.p_filesz as usize), 0, (ph.p_memsz - ph.p_filesz) as usize);
        }
    }

    Some(ModuleEntry { base: page_base, size: end - page_base, entry_point })
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
    let mut read_total = 0usize;
    while read_total < size {
        let n = file.read(&mut buf[read_total..]).ok()?;
        if n == 0 {
            break;
        }
        read_total += n;
    }
    if read_total != size {
        info!("short read: expected {} bytes, got {}", size, read_total);
        return None;
    }
    Some(buf)
}

// ── NVRAM ────────────────────────────────────────────────────────────────────

/// BMO vendor GUID (uuid v5 from "bmo-nvram").
/// Same GUID is used by the nvram-log crate in the kernel.
#[repr(C)]
#[derive(Clone, Copy)]
struct EfiGuid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

static BMO_NVRAM_GUID: EfiGuid = EfiGuid {
    data1: 0xc22a_0b40,
    data2: 0x52b8,
    data3: 0x5f95,
    data4: [0xa6, 0x81, 0x4b, 0x5c, 0x42, 0xeb, 0x02, 0x9a],
};

type RawGetVariableFn = unsafe extern "efiapi" fn(
    name: *const u16,
    guid: *const EfiGuid,
    attributes: *mut u32,
    data_size: *mut usize,
    data: *mut u8,
) -> u64;

type RawSetVariableFn = unsafe extern "efiapi" fn(
    name: *const u16,
    guid: *const EfiGuid,
    attributes: u32,
    data_size: usize,
    data: *const u8,
) -> u64;

/// Cached GetVariable pointer resolved BEFORE ExitBootServices.
static mut CACHED_GET_VAR: Option<RawGetVariableFn> = None;

/// Cached SetVariable pointer resolved BEFORE ExitBootServices.
static mut CACHED_SET_VAR: Option<RawSetVariableFn> = None;

/// NVRAM attributes: NON_VOLATILE | BOOTSERVICE_ACCESS | RUNTIME_ACCESS.
/// Must match kernel's nvram-log NVRAM_ATTRS exactly.
const NVRAM_ATTRS: u32 = 0x01 | 0x02 | 0x04;

/// Must be called BEFORE ExitBootServices to cache RuntimeServices pointers.
/// After EBS, the uefi crate may invalidate its internal state.
fn nvram_init() {
    let st = match uefi::table::system_table_raw() {
        Some(s) => s,
        None => return,
    };
    let rt_ptr = unsafe { core::ptr::read_volatile((st.as_ptr() as u64 + 0x58) as *const u64) };
    if rt_ptr == 0 { return; }

    // GetVariable at RuntimeServices + 0x48
    let get_fp = unsafe { core::ptr::read_volatile((rt_ptr + 0x48) as *const u64) };
    if get_fp != 0 {
        unsafe { CACHED_GET_VAR = Some(core::mem::transmute(get_fp)); }
    }

    // SetVariable at RuntimeServices + 0x58
    let set_fp = unsafe { core::ptr::read_volatile((rt_ptr + 0x58) as *const u64) };
    if set_fp != 0 {
        unsafe { CACHED_SET_VAR = Some(core::mem::transmute(set_fp)); }
    }
}

fn str_to_ucs2(name: &str) -> [u16; 64] {
    let mut ucs2 = [0u16; 64];
    let mut i = 0;
    for ch in name.bytes() {
        if i >= 63 { break; }
        ucs2[i] = ch as u16;
        i += 1;
    }
    ucs2[i] = 0;
    ucs2
}


/// Read NVRAM variable. Safe to call before or after EBS.
fn nvram_get(name: &str) -> Option<alloc::string::String> {
    let get_var = unsafe { CACHED_GET_VAR? };
    let ucs2_name = str_to_ucs2(name);
    let mut attrs: u32 = 0;
    let mut data_size: usize = 256;
    let mut buf = [0u8; 256];
    let status = unsafe {
        get_var(ucs2_name.as_ptr(), &BMO_NVRAM_GUID, &mut attrs, &mut data_size, buf.as_mut_ptr())
    };
    if status != 0 { return None; }
    let len = buf.iter().position(|&b| b == 0).unwrap_or(256);
    if len == 0 { return None; }
    Some(alloc::string::String::from_utf8_lossy(&buf[..len]).into_owned())
}

/// Write NVRAM variable. Safe to call after EBS (uses cached pointer).
/// Returns true on success.
fn nvram_set(name: &str, data: &[u8]) -> bool {
    let set_var = match unsafe { CACHED_SET_VAR } {
        Some(f) => f,
        None => return false,
    };
    let ucs2_name = str_to_ucs2(name);
    let status = unsafe {
        set_var(ucs2_name.as_ptr(), &BMO_NVRAM_GUID, NVRAM_ATTRS, data.len(), data.as_ptr())
    };
    status == 0
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

fn read_ram_stage() -> Option<u32> {
    let val = unsafe { core::ptr::read_volatile(0x9_0010 as *const u32) };
    if val != 0 { Some(val) } else { None }
}

fn ram_stage_name(val: u32) -> &'static str {
    match val {
        0x4542_5300 => "post_EBS",          // "EBS\0"
        0x4E56_5200 => "post_NVRAM",         // "NVR\0"
        0x4A4D_5000 => "post_kernel_jump",   // "JMP\0"
        // Kernel mirrors its numeric crash marker here after handoff. If the
        // primary magic slot is lost on warm reset, still decode the phase.
        n if n < 10_000 => ram_marker_name(n),
        _ => "unknown_ram_stage",
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

fn ram_marker_name(stage: u32) -> &'static str {
    match stage {
        0 => "kernel_main_real",
        1 => "kernel_start/bootinfo",
        2 => "phase_0_to_4",
        20 => "p0_arch",
        21 => "p1_mem",
        22 => "p2_dev",
        23 => "p3_display",
        24 => "p4_bmo",
        // Phase 0 sub-markers (fine-grained)
        200 => "p0_gdt",
        201 => "p0_idt",
        202 => "p0_syscall",
        203 => "p0_cpu_init",
        204 => "p0_cpu_done",
        205 => "p0_abi_init",
        206 => "p0_clock_done",
        207 => "p0_timer",
        208 => "p0_timer_done",
        2100 => "p1_enter",
        2101 => "p1_bootinfo",
        2102 => "p1_phys_init",
        2103 => "p1_phys_done",
        2104 => "p1_highmem_deferred",
        2105 => "p1_heap_init",
        2106 => "p1_heap_smoke",
        2107 => "p1_done",
        2200 => "p2_enter",
        2201 => "p2_acpi_mcfg",
        2202 => "p2_pci_scan",
        2203 => "p2_ps2_input",
        2204 => "p2_mmio_deferred",
        2205 => "p2_power",
        2206 => "p2_done",
        // cpu::init() sub-markers
        2031 => "cpu_step1_features",
        2032 => "cpu_step2_regs",
        2033 => "cpu_step3_xcr0",
        2034 => "cpu_step4_fpu",
        2035 => "cpu_step5_cache",
        2036 => "cpu_step6_perf",
        2037 => "cpu_step7_lazyfpu",
        2038 => "cpu_step8_tsc",
        2039 => "cpu_step9_info",
        2040 => "cpu_init_done",
        3 => "init_bmo_cpu",
        4 => "init_acpi",
        45 => "smp_init",
        5 => "bmo_core_init",
        6 => "welcome_dispatch",
        7 => "bmo_enter",
        8 => "welcome_running",
        _ => "unknown_ram_stage",
    }
}

fn write_crash_log(
    handle: uefi::Handle,
    nvram_stage: Option<&str>,
    ram_marker: Option<u32>,
    ram_stage: Option<u32>,
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
        if let Ok(h) = root.open(name, FileMode::Read, FileAttribute::empty()) {
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

    if let Some(m) = ram_marker {
        entry.extend_from_slice(b"CRASH at: ");
        entry.extend_from_slice(ram_marker_name(m).as_bytes());
        entry.extend_from_slice(b" | RAM=");
        write_int(&mut entry, m);
        if let Some(stage) = nvram_stage {
            entry.extend_from_slice(b" | prev NVRAM=");
            entry.extend_from_slice(stage.as_bytes());
        }
    } else if let Some(rs) = ram_stage {
        entry.extend_from_slice(b"boot died at: ");
        entry.extend_from_slice(ram_stage_name(rs).as_bytes());
        if let Some(stage) = nvram_stage {
            entry.extend_from_slice(b" | prev NVRAM=");
            entry.extend_from_slice(stage.as_bytes());
        }
    } else {
        match nvram_stage {
            Some("ok") => entry.extend_from_slice(b"OK (welcome reached)"),
            Some("bootloader") => entry.extend_from_slice(b"previous boot reached kernel handoff"),
            Some("kernel_jump") => entry.extend_from_slice(b"bootloader jumped to kernel, kernel died before NVRAM write"),
            Some(stage) => {
                entry.extend_from_slice(b"CRASH at: ");
                entry.extend_from_slice(stage.as_bytes());
            }
            None => entry.extend_from_slice(b"no NVRAM data"),
        }
    }

    entry.extend_from_slice(b" | NVRAM: ");
    entry.extend_from_slice(nvram_status_ok(nvram_result));
    if let Err(msg) = nvram_result {
        entry.extend_from_slice(b" (");
        entry.extend_from_slice(msg.as_bytes());
        entry.extend_from_slice(b")");
    }

    // Read kernel diagnostic breadcrumbs
    let d1 = nvram_get("BMODiag1");
    let d2 = nvram_get("BMODiag2");
    let d3 = nvram_get("BMODiag3");
    let phase = nvram_get("BMOPhase");
    if d1.is_some() || d2.is_some() || d3.is_some() || phase.is_some() {
        entry.extend_from_slice(b" | diag:");
        if let Some(v) = &phase {
            entry.extend_from_slice(b" phase=");
            entry.extend_from_slice(v.as_bytes());
        }
        if let Some(v) = &d1 {
            entry.extend_from_slice(b" D1=");
            entry.extend_from_slice(v.as_bytes());
        }
        if let Some(v) = &d2 {
            entry.extend_from_slice(b" D2=");
            entry.extend_from_slice(v.as_bytes());
        }
        if let Some(v) = &d3 {
            entry.extend_from_slice(b" D3=");
            entry.extend_from_slice(v.as_bytes());
        }
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
        if let Ok(h) = root.open(name, FileMode::CreateReadWrite, FileAttribute::empty()) {
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
    info!("BMO Bootloader v0.4.0");

    // Disable UEFI firmware watchdog
    for _ in 0..5 {
        let _ = boot::set_watchdog_timer(0, 0x1_0000, None);
    }

    // Get boot device
    let loaded_image = boot::open_protocol_exclusive::<LoadedImage>(boot::image_handle()).unwrap();
    let device = loaded_image.device().expect("no device handle");
    drop(loaded_image);

    // Cache NVRAM function pointers BEFORE ExitBootServices
    nvram_init();

    // Crash diagnostics — nvram_get is allowed during Boot Services (UEFI spec)
    let prev_stage = nvram_get("BMOBootStage");
    let ram_marker = read_crash_marker();
    let ram_stage = read_ram_stage();
    let boot_num = read_and_increment_boot_counter();

    // NOTE: nvram_set CANNOT be called here — SetVariable is only valid after
    // ExitBootServices per UEFI spec Section 8. Calling it before EBS causes
    // INVALID_PARAMETER on AMD firmware. We write NVRAM after EBS below.
    info!("Boot #{} — RAM: {:?} stage: {:?}", boot_num, ram_marker, ram_stage);
    write_crash_log(device, prev_stage.as_deref(), ram_marker, ram_stage, boot_num, &Ok(()));

    clear_crash_marker();

    // Also clear the RAM boot-stage markers from previous boot
    unsafe {
        core::ptr::write_volatile(0x9_0010 as *mut u32, 0u32);
        core::ptr::write_volatile(0x9_0014 as *mut u32, 0u32);
        core::ptr::write_volatile(0x9_0018 as *mut u32, 0u32);
        core::ptr::write_volatile(0x9_0020 as *mut u32, 0u32);
        core::ptr::write_volatile(0x9_0024 as *mut u32, 0u32);
    }

    // ── Allocate kernel ───────────────────────────────────────────────
    // Load kernel binary (flat kernel.bin or ELF kernel.elf)
    let mut is_elf = true;
    let elf_data = match read_file(device, "\\EFI\\BOOT\\kernel.bin") {
        Some(data) => {
            is_elf = false;
            data
        }
        None => {
            read_file(device, "\\EFI\\BOOT\\kernel.elf").expect("failed to read kernel.elf/kernel.bin")
        }
    };

    let mut entry_point: u64 = 0;
    let mut page_base: u64 = 0;
    let mut end: u64 = 0;

    if is_elf {
        let (ep, phdrs) = parse_elf64(&elf_data).expect("failed to parse ELF64");
        entry_point = ep;

        // Compute kernel span
        let mut base: u64 = u64::MAX;
        for ph in &phdrs {
            if ph.p_type != PT_LOAD || ph.p_memsz == 0 { continue; }
            base = base.min(ph.p_vaddr);
            end = end.max(ph.p_vaddr + ph.p_memsz);
        }
        page_base = base & !0xFFF;
        let total_pages = ((end - page_base + 0xFFF) / 0x1000) as usize;

        let kernel_ptr = match boot::allocate_pages(
            boot::AllocateType::Address(page_base), MemoryType::LOADER_CODE, total_pages,
        ) {
            Ok(addr) => addr.as_ptr() as *mut u8,
            Err(_) => {
                info!("FATAL: fixed kernel address 0x{:x} occupied", page_base);
                return Status::OUT_OF_RESOURCES;
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
    } else {
        // Flat raw binary loaded at fixed 0x2000000
        page_base = 0x2000000;
        let file_size = elf_data.len() as u64;
        end = page_base + file_size;
        entry_point = page_base;
        let total_pages = ((file_size + 0xFFF) / 0x1000) as usize;

        let kernel_ptr = match boot::allocate_pages(
            boot::AllocateType::Address(page_base), MemoryType::LOADER_CODE, total_pages,
        ) {
            Ok(addr) => addr.as_ptr() as *mut u8,
            Err(_) => {
                info!("FATAL: fixed flat kernel address 0x{:x} occupied", page_base);
                return Status::OUT_OF_RESOURCES;
            }
        };

        unsafe {
            core::ptr::copy_nonoverlapping(elf_data.as_ptr(), kernel_ptr, file_size as usize);
        }
    }

    // Load kernel_services ELF
    let s_elf_data = read_file(device, "\\EFI\\BOOT\\kernel_services.elf").expect("failed to read kernel_services.elf");
    let (s_entry, s_phdrs) = parse_elf64(&s_elf_data).expect("failed to parse kernel_services ELF64");

    let mut s_base: u64 = u64::MAX;
    let mut s_end: u64 = 0;
    for ph in &s_phdrs {
        if ph.p_type != PT_LOAD || ph.p_memsz == 0 { continue; }
        s_base = s_base.min(ph.p_vaddr);
        s_end = s_end.max(ph.p_vaddr + ph.p_memsz);
    }
    let s_page_base = s_base & !0xFFF;
    let s_total_pages = ((s_end - s_page_base + 0xFFF) / 0x1000) as usize;

    let s_kernel_ptr = match boot::allocate_pages(
        boot::AllocateType::Address(s_page_base), MemoryType::LOADER_CODE, s_total_pages,
    ) {
        Ok(addr) => addr.as_ptr() as *mut u8,
        Err(_) => {
            info!("FATAL: fixed kernel services address 0x{:x} occupied", s_page_base);
            return Status::OUT_OF_RESOURCES;
        }
    };

    for ph in &s_phdrs {
        if ph.p_type != PT_LOAD || ph.p_memsz == 0 { continue; }
        let dst = unsafe { s_kernel_ptr.add((ph.p_vaddr - s_page_base) as usize) };
        unsafe {
            core::ptr::copy_nonoverlapping(s_elf_data.as_ptr().add(ph.p_offset as usize), dst, ph.p_filesz as usize);
            core::ptr::write_bytes(dst.add(ph.p_filesz as usize), 0, (ph.p_memsz - ph.p_filesz) as usize);
        }
    }

    // GOP

    // Pre-load Ring 3 modules (before ExitBootServices)
    let mut module_entries: [bmo_boot_protocol::ModuleEntry; MAX_MODULES] =
        [bmo_boot_protocol::ModuleEntry { base: 0, size: 0, entry_point: 0 }; MAX_MODULES];
    let mut module_count: u32 = 0;
    for mod_path in MODULE_PATHS {
        info!("loading module: {}", mod_path);
        if let Some(entry) = load_module(device, mod_path) {
            let idx = module_count as usize;
            if idx < MAX_MODULES {
                module_entries[idx] = entry;
                module_count += 1;
                info!("  => loaded at 0x{:x} entry=0x{:x}", entry.base, entry.entry_point);
            } else {
                info!("  => MAX_MODULES reached, skipping");
                break;
            }
        } else {
            info!("  => not found or failed, skipping");
        }
    }
    let gop = query_gop().expect("GOP failed");

    // ── Allocate stack + BootInfo below 4GB ──────────────────────────
    // Using MaxAddress ensures these are always identity-mapped after
    // ExitBootServices, even on firmware that only maps low memory.
    let stack_pages = KERNEL_STACK_SIZE / 4096;
    let stack_base = boot::allocate_pages(
        boot::AllocateType::MaxAddress(0x7FFF_F000), MemoryType::LOADER_DATA, stack_pages,
    ).expect("stack alloc failed").as_ptr() as u64;
    let stack_top = stack_base + KERNEL_STACK_SIZE as u64;

    // BootInfo (16 KiB = 4 pages)
    let boot_info_ptr = boot::allocate_pages(
        boot::AllocateType::MaxAddress(0x7FFE_F000), MemoryType::LOADER_DATA, 4,
    ).expect("BootInfo alloc failed").as_ptr() as *mut BootInfo;

    // Fill BootInfo
    unsafe {
        core::ptr::write_bytes(boot_info_ptr, 0, 1);
        let bi = &mut *boot_info_ptr;
        bi.magic = bmo_boot_protocol::BOOT_MAGIC;
        bi.version = bmo_boot_protocol::PROTOCOL_VERSION;
        bi.fb_addr = gop.fb_addr;
        bi.fb_size = gop.fb_size;
        bi.fb_width = gop.width;
        bi.fb_height = gop.height;
        bi.fb_stride = gop.stride;
        bi.fb_pixel_format = gop.pixel_format;
        bi.rsdp_addr = find_rsdp();
        bi.kernel_base = page_base;
        bi.kernel_size = end - page_base;
        bi.services_base = s_page_base;
        bi.services_size = s_end - s_page_base;
        bi.services_entry = s_entry;
        bi.stack_top = stack_top;
        bi.stack_size = KERNEL_STACK_SIZE as u64;
        bi.uefi_system_table = uefi::table::system_table_raw()
            .map(|p| p.as_ptr() as u64).unwrap_or(0);
        bi.module_count = module_count;
        for i in 0..module_count as usize {
            bi.modules[i] = module_entries[i];
        }

    }

    // ── Exit Boot Services ───────────────────────────────────────────
    // This is the POINT OF NO RETURN. After this call:
    //   - NO Boot Services (file I/O, console, allocation, protocols)
    //   - ONLY Runtime Services (NVRAM SetVariable/GetVariable)
    //   - NO hardware access except direct memory/IO
    // Per UEFI spec, the firmware disables its software watchdog on EBS.
    // The FCH hardware watchdog (if any) is the KERNEL's responsibility
    // after it sets up its own IDT.
    let memory_map = unsafe { boot::exit_boot_services(Some(MemoryType::LOADER_DATA)) };

    // ── Post-EBS: ONLY safe operations ───────────────────────────────
    // RAM markers survive warm reset (if RAM preserved). Checked on next boot.
    unsafe {
        core::ptr::write_volatile(0x9_0010 as *mut u32, 0x4542_5300u32); // "EBS\0"
    }

    // Write "bootloader" to NVRAM — confirms bootloader reached this point.
    // Uses cached SetVariable pointer; safe post-EBS as a UEFI Runtime Service.
    nvram_set("BMOBootStage", b"bootloader");

    // Fill memory map into BootInfo
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

    unsafe {
        core::ptr::write_volatile(0x9_0020 as *mut u32, 0x4A4D_5000u32); // "JMP\0"
        core::ptr::write_volatile(0x9_0024 as *mut u32, entry_point as u32);
    }

    // ── Jump to kernel ───────────────────────────────────────────────
    // RDI = boot_info_ptr (first argument per System V AMD64 ABI)
    // RSP = stack_top (16-byte aligned by UEFI page allocator)
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
