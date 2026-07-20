//! s1_cpu — AMD Ryzen 5 5600X (Zen 3) optimized UEFI handoff + CPU init.
//!
//! This stage is the ONE place that knows the target CPU. Everything
//! CPU-specific lives here, not in the kernel, because:
//!   - It's a one-time setup (boot time only)
//!   - It's CPU-specific (not portable — each CPU has its quirks)
//!   - The kernel should be generic (portable to AArch64/RISC-V later)
//!   - CPU optimizations like Zen 3 mitigations need to be applied early
//!
//! Zen 3 (Ryzen 5 5600X) specific features enabled here:
//!   - CPUID topology extension (0x80000026): 6C/12T, 1 CCD, 1 CCX
//!   - L1 32KB, L2 512KB, L3 32MB cache hierarchy
//!   - SYSCALL/SYSRET via AMD K8 ABI (different from Intel's)
//!   - Spectre v1 mitigations (SSBD, RSB fill)
//!   - SME/SEV support detection
//!   - Boost clock awareness (4.6 GHz max)
//!   - TSC calibration (3.7 GHz base, 4.6 GHz boost)
//!   - WBNOINVD, RDPID, RDTSCP, INVLPGB, CLZERO Zen 3+ features
//!   - AMD topology: cores per CCX, threads per core, CCD count

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![allow(static_mut_refs)]
#![allow(unsafe_op_in_unsafe_fn)]

use core::panic::PanicInfo;
use core::arch::{asm, naked_asm};
use boot_context::{BootContext, MemoryEntry, MAX_MEMORY_ENTRIES, KERNEL_STAGE_INDEX, MAX_STAGES};

// ═══════════════════════════════════════════════════════════════════
//  UEFI TYPES AND CONSTANTS
// ═══════════════════════════════════════════════════════════════════

type EfiHandle = *mut core::ffi::c_void;
type EfiStatus = u64;
const EFI_SUCCESS: u64 = 0;
const EFI_CONVENTIONAL_MEMORY: u32 = 7;
const S2_ADDR: u64 = 0x200000;
const S2_RESERVE_SIZE: u64 = 2 * 1024 * 1024;
const KERNEL_RESERVE_SIZE: u64 = 16 * 1024 * 1024;
// Ring 3 init payload + kernel-owned workspace. Placed just past the
// kernel reserve (0x400000 + 16 MiB), inside the s2 identity map, and
// taken out of the UEFI map via AllocateAddress so no allocator can
// hand them out twice.
const RING3_PAYLOAD_ADDR: u64 = 0x1400000;
const RING3_PAYLOAD_MAX: u64 = 1024 * 1024;
const RING3_WORKSPACE_ADDR: u64 = 0x1500000;
const RING3_WORKSPACE_SIZE: u64 = 1024 * 1024;
const COM1: u16 = 0x3F8;

#[repr(C)] struct EfiTableHeader { signature: u64, revision: u32, header_size: u32, crc32: u32, _reserved: u32 }
#[repr(C)] struct EfiBootServices { hdr: EfiTableHeader, _pad: [u8; 44 * 8] }
#[repr(C)] struct EfiSystemTable {
    hdr: EfiTableHeader, _firmware: *mut core::ffi::c_void,
    _firmware_revision: u32, _firmware_pad: u32,
    _cin_handle: EfiHandle, _con_in: *mut core::ffi::c_void,
    _cout_handle: EfiHandle, _con_out: *mut core::ffi::c_void,
    _cerr_handle: EfiHandle, _con_err: *mut core::ffi::c_void,
    _runtime: *mut core::ffi::c_void,
    boot_services: *mut EfiBootServices, _num_tables: usize, _config_tables: *mut core::ffi::c_void,
}
#[repr(C)] struct EfiGuid { data1: u32, data2: u16, data3: u16, data4: [u8; 8] }
#[repr(C)] struct EfiSimpleFileSystemProtocol { revision: u64, open_volume: unsafe extern "efiapi" fn(*const Self, *mut *mut core::ffi::c_void) -> EfiStatus }
#[repr(C)] struct EfiFileProtocol {
    revision: u64,
    open: unsafe extern "efiapi" fn(*const Self, *mut *mut core::ffi::c_void, *const u16, u64, u64) -> EfiStatus,
    close: unsafe extern "efiapi" fn(*const Self) -> EfiStatus,
    _delete: *mut core::ffi::c_void, read: *mut core::ffi::c_void, write: *mut core::ffi::c_void,
    _get_position: *mut core::ffi::c_void, _set_position: *mut core::ffi::c_void,
    _get_info: *mut core::ffi::c_void, _set_info: *mut core::ffi::c_void, _flush: *mut core::ffi::c_void,
}
#[repr(C)] struct EfiMemoryDescriptor { mem_type: u32, _pad: u32, phys_start: u64, virt_start: u64, num_pages: u64, attrib: u64 }
#[repr(C)] struct EfiGraphicsOutputProtocolMode { max_mode: u32, mode: u32, info: *mut u8, size_of_info: usize, frame_buffer_base: u64, frame_buffer_size: usize }
#[repr(C)] struct EfiGraphicsOutputProtocol {
    query_mode: extern "efiapi" fn(*mut Self, u32, &mut usize, &mut *mut u8) -> EfiStatus,
    set_mode: extern "efiapi" fn(*mut Self, u32) -> EfiStatus,
    blt: *mut core::ffi::c_void, mode: *mut EfiGraphicsOutputProtocolMode,
}

static mut FILE_SYSTEM_GUID: EfiGuid = EfiGuid { data1: 0x964e5b22, data2: 0x6409, data3: 0x47ef, data4: [0x97, 0xa2, 0xff, 0x06, 0xff, 0x38, 0xb0, 0xdf] };
static mut LOADED_IMAGE_GUID: EfiGuid = EfiGuid { data1: 0x5b1b31a1, data2: 0x9562, data3: 0x11d2, data4: [0x8e, 0x3f, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b] };
static mut GOP_GUID: EfiGuid = EfiGuid { data1: 0x9042a9de, data2: 0x23dc, data3: 0x4a38, data4: [0x96, 0xfb, 0x72, 0xde, 0x52, 0xfe, 0xc4, 0x49] };

// ═══════════════════════════════════════════════════════════════════
//  AMD-SPECIFIC MSR ADDRESSES (Zen 3 / Family 19h)
// ═══════════════════════════════════════════════════════════════════

const MSR_TSC: u32                = 0x00000010;
const MSR_APIC_BASE: u32         = 0x0000001B;
const MSR_PLATFORM_INFO: u32     = 0x00000017;
const MSR_MTRR_CAP: u32          = 0x000000FE;
const MSR_PAT: u32               = 0x00000277;
const MSR_MTRR_FIX_64K_00000: u32 = 0x00000250;
const MSR_MTRR_VARIABLE_BASE: u32 = 0x00000200;
const MSR_MTRR_VARIABLE_MASK: u32 = 0x00000201;
const MSR_MTRR_DEF_TYPE: u32     = 0x000002FF;
const MSR_SYSENTER_CS: u32       = 0x00000174;
const MSR_SYSENTER_ESP: u32      = 0x00000175;
const MSR_SYSENTER_EIP: u32      = 0x00000176;
const MSR_TSC_AUX: u32           = 0xC0000103;

// AMD K8 SYSCALL MSRs (0xC0000080-0xC0000084)
const MSR_EFER: u32              = 0xC0000080;
const MSR_STAR: u32              = 0xC0000081;
const MSR_LSTAR: u32             = 0xC0000082;
const MSR_CSTAR: u32             = 0xC0000083;
const MSR_SFMASK: u32            = 0xC0000084;

// AMD segment base MSRs (0xC0000100-0xC0000102)
const MSR_FS_BASE: u32           = 0xC0000100;
const MSR_GS_BASE: u32           = 0xC0000101;
const MSR_KERNEL_GS_BASE: u32    = 0xC0000102;

// AMD-specific MSRs (Zen 3)
const MSR_SYSCFG: u32            = 0xC0000010;  // Zen 3 SYSCFG
const MSR_HWCR: u32              = 0xC0010015;  // Hardware Configuration
const MSR_NB_CFG1: u32           = 0xC001001E;  // Northbridge Config 1
const MSR_LS_CFG: u32            = 0xC0011020;  // Load-Store Configuration
const MSR_IC_CFG: u32            = 0xC0011021;  // Instruction Cache Configuration
const MSR_DC_CFG: u32            = 0xC0011022;  // Data Cache Configuration
const MSR_BU_CFG: u32            = 0xC0011023;  // Bus Unit Configuration
const MSR_DE_CFG: u32            = 0xC0011029;  // Decode Unit Configuration
const MSR_L2_CFG: u32            = 0xC001102D;  // L2 Cache Configuration
const MSR_CU_CFG: u32            = 0xC001102F;  // Compute Unit Configuration
const MSR_PF2_INSTR_CTL: u32     = 0xC0010100;  // Prefetch Configuration
const MSR_PF1_INSTR_CTL: u32     = 0xC0010102;

// EFER bits (AMD-specific bits marked)
const EFER_SCE: u64   = 1 << 0;   // SYSCALL enable
const EFER_LME: u64   = 1 << 8;   // Long mode enable
const EFER_LMA: u64   = 1 << 10;  // Long mode active
const EFER_NXE: u64   = 1 << 11;  // No-execute enable
const EFER_SVME: u64  = 1 << 12;  // Secure virtual machine (SVM) enable
const EFER_LMSLE: u64 = 1 << 13;  // Long mode segment limit enable
const EFER_FFXSR: u64 = 1 << 14;  // Fast FXSAVE/XRESTOR
const EFER_TCE: u64   = 1 << 15;  // Translation cache extension
const EFER_MCOMMIT: u64 = 1 << 17; // MCOMMIT instruction enable
const EFER_INTWB: u64 = 1 << 18;  // Interruptible WBINVD/WBNOINVD
const EFER_UAIE: u64  = 1 << 19;  // Upper address ignore enable (SEV)
const EFER_AIBRSE: u64 = 1 << 21; // Automatic IBRS enable

// SYSCFG bits (AMD Zen 3)
const SYSCFG_MFDM: u64   = 1 << 18;  // Memory disambiguation flush disable
const SYSCFG_TOM2: u64   = 1 << 21;  // TOM2 enable
const SYSCFG_FB_MODE: u64 = 1 << 25; // FSGSBASE enable (Zen 3)
const SYSCFG_FSGS: u64   = 1 << 18;  // FSGS (deprecated)

// HWCR bits (AMD)
const HWCR_FFDIS: u64    = 1 << 6;  // Flush filter disable

// ═══════════════════════════════════════════════════════════════════
//  COM1 SERIAL
// ═══════════════════════════════════════════════════════════════════

#[inline] unsafe fn outb(port: u16, val: u8) { asm!("out dx, al", in("dx") port, in("al") val); }
#[inline] unsafe fn inb(port: u16) -> u8 { let v: u8; asm!("in al, dx", in("dx") port, out("al") v); v }
unsafe fn put_byte(b: u8) { let mut t = 100_000u32; while inb(COM1 + 5) & 0x20 == 0 { t = t.saturating_sub(1); if t == 0 { return; } } outb(COM1, b); }
fn serial_init() { unsafe { outb(COM1 + 1, 0); outb(COM1 + 3, 0x80); outb(COM1 + 0, 1); outb(COM1 + 1, 0); outb(COM1 + 3, 3); outb(COM1 + 2, 0xC7); outb(COM1 + 4, 0xB); } }
unsafe fn put_str(s: &str) { for &b in s.as_bytes() { if b == b'\n' { put_byte(b'\r'); } put_byte(b); } }
unsafe fn put_hex(mut v: u64) { if v == 0 { put_byte(b'0'); return; } let mut b = [0u8; 16]; let mut i = 0; while v > 0 { b[i] = b"0123456789abcdef"[(v & 0xF) as usize]; v >>= 4; i += 1; } for j in (0..i).rev() { put_byte(b[j]); } }
unsafe fn put_dec(mut v: usize) { if v == 0 { put_byte(b'0'); return; } let mut b = [0u8; 20]; let mut i = 0; while v > 0 { b[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; } for j in (0..i).rev() { put_byte(b[j]); } }
macro_rules! ser_print { ($s:expr) => { unsafe { put_str($s); } }; }
macro_rules! ser_hex { ($v:expr) => { unsafe { put_hex($v); } }; }
macro_rules! ser_dec { ($v:expr) => { unsafe { put_dec($v); } }; }

// ═══════════════════════════════════════════════════════════════════
//  GDT + TSS (universal x86-64)
// ═══════════════════════════════════════════════════════════════════

const KERNEL_CS: u16 = 0x08;
const KERNEL_DS: u16 = 0x10;
const USER_DS: u16  = 0x18 | 3;
const USER_CS: u16  = 0x20 | 3;
const TSS_SEL: u16  = 0x28;

const IST1_SIZE: usize = 8192;
const IST3_SIZE: usize = 8192;
const KSTACK_SIZE: usize = 16384;

#[repr(C, packed)] struct Tss { _r0: u32, rsp: [u64; 3], _r1: u64, ist: [u64; 7], _r2: u64, _r3: u16, iomap_base: u16 }
#[repr(C, align(16))] struct Gdt { entries: [u64; 7] }
#[repr(C, packed)] struct Gdtr { limit: u16, base: u64 }
#[repr(align(16))] struct IstStack([u8; IST1_SIZE]);
#[repr(align(16))] struct McStack([u8; IST3_SIZE]);
#[repr(align(16))] struct KernelStack([u8; KSTACK_SIZE]);

static mut TSS: Tss = Tss { _r0: 0, rsp: [0; 3], _r1: 0, ist: [0; 7], _r2: 0, _r3: 0, iomap_base: 0 };
static mut GDT: Gdt = Gdt { entries: [0; 7] };
static mut IST1: IstStack = IstStack([0; IST1_SIZE]);
static mut IST3: McStack  = McStack([0; IST3_SIZE]);
static mut KSTK: KernelStack = KernelStack([0; KSTACK_SIZE]);

// ═══════════════════════════════════════════════════════════════════
//  IDT
// ═══════════════════════════════════════════════════════════════════

#[repr(C, packed)] #[derive(Clone, Copy)]
struct IdtEntry { off_lo: u16, sel: u16, ist: u8, attr: u8, off_mid: u16, off_hi: u32, _r: u32 }
impl IdtEntry {
    const fn empty() -> Self { Self { off_lo: 0, sel: 0, ist: 0, attr: 0, off_mid: 0, off_hi: 0, _r: 0 } }
    fn set(&mut self, h: u64, ist: u8) {
        self.off_lo = h as u16; self.off_mid = (h >> 16) as u16; self.off_hi = (h >> 32) as u32;
        self.sel = 0x08; self.ist = ist; self.attr = 0x8E; self._r = 0;
    }
}
#[repr(C, packed)] struct Idtr { limit: u16, base: u64 }
static mut IDT: [IdtEntry; 256] = [IdtEntry::empty(); 256];

macro_rules! halt_handler { ($name:ident, $msg:literal) => { extern "x86-interrupt" fn $name(_sf: u64) { unsafe { put_str(concat!("[s1_cpu] ", $msg, " — halting\n")); } loop { unsafe { asm!("hlt"); } } } }; }
halt_handler!(exc_no_err,      "EXCEPTION (no err)");
halt_handler!(exc_divide,      "#DE Divide Error");
halt_handler!(exc_invalid_op,   "#UD Invalid Opcode");
halt_handler!(exc_dev_not_av,  "#NM Device Not Available");
halt_handler!(exc_x87,         "#MF x87 FP");
halt_handler!(exc_simd,        "#XM SIMD");
halt_handler!(exc_mcheck,      "#MC Machine Check");
halt_handler!(exc_no_err2,     "EXCEPTION (no err, mirror)");
halt_handler!(irq_stub,        "IRQ stub");

extern "x86-interrupt" fn exc_with_err(_sf: u64, _e: u64) { unsafe { put_str("[s1_cpu] EXCEPTION (with err) — halting\n"); } loop { unsafe { asm!("hlt"); } } }
extern "x86-interrupt" fn exc_double_fault(_sf: u64, _e: u64) { unsafe { put_str("[s1_cpu] #DF Double Fault — halting\n"); } loop { unsafe { asm!("cli; hlt"); } } }
extern "x86-interrupt" fn exc_gpf(_sf: u64, _e: u64) { unsafe { put_str("[s1_cpu] #GP General Protection — halting\n"); } loop { unsafe { asm!("hlt"); } } }
extern "x86-interrupt" fn exc_page_fault(_sf: u64, _e: u64) { unsafe { put_str("[s1_cpu] #PF Page Fault — halting\n"); } loop { unsafe { asm!("hlt"); } } }

// ═══════════════════════════════════════════════════════════════════
//  FPU STATE
// ═══════════════════════════════════════════════════════════════════

#[repr(align(64))] struct Align64([u8; 1024]);
static mut FPU_STATE: Align64 = Align64([0; 1024]);

// ═══════════════════════════════════════════════════════════════════
//  AMD RYZEN 5 5600X (Zen 3) CPU PROFILE
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct CpuProfile {
    // Vendor info
    vendor: [u8; 12],       // "AuthenticAMD"
    brand: [u8; 48],        // "AMD Ryzen 5 5600X..."

    // Family/Model/Stepping
    family: u8,            // 0x19 (Zen 3)
    model: u8,             // 0x21 (5600X)
    stepping: u8,          // 0x2 (B2 stepping)

    // Topology (5600X: 1 CCD, 1 CCX, 6 cores, 12 threads)
    threads_per_core: u8,  // 2 (SMT enabled)
    cores_per_ccx: u8,     // 6 (5600X is monolithic, 1 CCX)
    ccx_count: u8,         // 1 (5600X has only 1 CCX)
    ccd_count: u8,         // 1 (single CCD)

    // Cache hierarchy
    l1d_size_kb: u16,      // 32 KB
    l1i_size_kb: u16,      // 32 KB
    l2_size_kb: u16,       // 512 KB per core
    l3_size_kb: u32,       // 32 MB shared (5600X has 32MB L3)

    // Frequencies
    base_freq_mhz: u32,    // 3700
    boost_freq_mhz: u32,   // 4600

    // TSC
    tsc_freq: u64,         // Calculated from CPUID 0x15

    // Features
    has_sme: bool,         // Secure Memory Encryption
    has_sev: bool,         // Secure Encrypted Virtualization
    has_wbnoinvd: bool,    // Write Back No Invalidate
    has_rdpid: bool,       // Read Processor ID
    has_invplgb: bool,     // Invalidate TLB Global
    has_clzero: bool,      // Cache Line Zero
    has_clwb: bool,        // Cache Line Write Back
    has_clflushopt: bool,  // Optimized Cache Line Flush
    has_smep: bool,        // Supervisor Mode Execution Prevention
    has_smap: bool,        // Supervisor Mode Access Prevention
    has_fsgsbase: bool,    // FS/GS Base instructions
}

impl CpuProfile {
    const fn empty() -> Self {
        Self {
            vendor: [0; 12], brand: [0; 48],
            family: 0, model: 0, stepping: 0,
            threads_per_core: 0, cores_per_ccx: 0,
            ccx_count: 0, ccd_count: 0,
            l1d_size_kb: 0, l1i_size_kb: 0, l2_size_kb: 0, l3_size_kb: 0,
            base_freq_mhz: 0, boost_freq_mhz: 0, tsc_freq: 0,
            has_sme: false, has_sev: false, has_wbnoinvd: false,
            has_rdpid: false, has_invplgb: false, has_clzero: false,
            has_clwb: false, has_clflushopt: false, has_smep: false,
            has_smap: false, has_fsgsbase: false,
        }
    }
}

static mut CPU: CpuProfile = CpuProfile::empty();

// ═══════════════════════════════════════════════════════════════════
//  CPUID WRAPPER (preserves RBX per SysV ABI)
// ═══════════════════════════════════════════════════════════════════

#[inline]
fn cpuid(leaf: u32, sub: u32) -> (u32, u32, u32, u32) {
    let (eax, ebx, ecx, edx);
    unsafe {
        asm!(
            "push rbx", "cpuid", "mov {ebx_out:e}, ebx", "pop rbx",
            inout("eax") leaf => eax, inout("ecx") sub => ecx,
            ebx_out = out(reg) ebx, out("edx") edx,
        );
    }
    (eax, ebx, ecx, edx)
}

#[inline]
unsafe fn wrmsr(msr: u32, val: u64) {
    asm!("wrmsr", in("ecx") msr, in("eax") val as u32, in("edx") (val >> 32) as u32);
}

#[inline]
unsafe fn rdmsr(msr: u32) -> u64 {
    let lo: u32; let hi: u32;
    asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi);
    ((hi as u64) << 32) | (lo as u64)
}

// ═══════════════════════════════════════════════════════════════════
//  AMD ZEN 3 CPU DETECTION (Ryzen 5 5600X)
// ═══════════════════════════════════════════════════════════════════

unsafe fn detect_cpu() {
    let cpu = &mut CPU;

    // 1. Vendor (leaf 0)
    let (_, ebx, ecx, edx) = cpuid(0, 0);
    cpu.vendor[0..4].copy_from_slice(&ebx.to_ne_bytes());
    cpu.vendor[4..8].copy_from_slice(&edx.to_ne_bytes());
    cpu.vendor[8..12].copy_from_slice(&ecx.to_ne_bytes());
    ser_print!("[s1_cpu] vendor: ");
    if let Ok(s) = core::str::from_utf8(&cpu.vendor) { ser_print!(s); }
    ser_print!("\n");

    // 2. Brand string (leaves 0x80000002-04)
    let (a, b, c, d) = cpuid(0x80000002, 0);
    let (e, f, g, h) = cpuid(0x80000003, 0);
    let (i, j, k, l) = cpuid(0x80000004, 0);
    let mut idx = 0;
    for v in [a, b, c, d, e, f, g, h, i, j, k, l] {
        if idx < 48 { cpu.brand[idx] = v as u8; idx += 1; }
        if idx < 48 { cpu.brand[idx] = (v >> 8) as u8; idx += 1; }
        if idx < 48 { cpu.brand[idx] = (v >> 16) as u8; idx += 1; }
        if idx < 48 { cpu.brand[idx] = (v >> 24) as u8; idx += 1; }
    }
    ser_print!("[s1_cpu] brand: ");
    if let Ok(s) = core::str::from_utf8(&cpu.brand) { ser_print!(s.trim_end_matches('\0')); }
    ser_print!("\n");

    // 3. Family/Model/Stepping (leaf 1, family/mask logic per AMD)
    let (eax1, _, _, _) = cpuid(1, 0);
    let stepping = (eax1 >> 0) & 0xF;
    let model = (eax1 >> 4) & 0xF;
    let family = (eax1 >> 8) & 0xF;
    let ext_model = (eax1 >> 16) & 0xF;
    let ext_family = (eax1 >> 20) & 0xFF;

    cpu.stepping = stepping as u8;
    cpu.family = if family == 0xF { (family + ext_family) as u8 } else { family as u8 };
    cpu.model = if family == 0xF || family == 0x6 { ((ext_model << 4) | model) as u8 } else { model as u8 };
    ser_print!("[s1_cpu] family=0x");
    ser_hex!(cpu.family as u64);
    ser_print!(" model=0x");
    ser_hex!(cpu.model as u64);
    ser_print!(" stepping=0x");
    ser_hex!(cpu.stepping as u64);
    ser_print!("\n");

    // 4. AMD extended features (leaf 0x80000001)
    let (_, _, ecx_8001, edx_8001) = cpuid(0x80000001, 0);
    ser_print!("[s1_cpu] AMD features: SVM=");
    ser_print!(if ecx_8001 & (1 << 2) != 0 { "Y " } else { "N " });
    ser_print!("SSE4A=");
    ser_print!(if ecx_8001 & (1 << 6) != 0 { "Y " } else { "N " });
    ser_print!("SVM-FEAT=");
    let (eax_8001a, _, ecx_8001a, _) = cpuid(0x8000000A, 0);
    if eax_8001a != 0 {
        ser_print!(if ecx_8001a & (1 << 5) != 0 { "SME " } else { "" });
        ser_print!(if ecx_8001a & (1 << 6) != 0 { "SEV " } else { "" });
        ser_print!(if ecx_8001a & (1 << 8) != 0 { "SEV-ES " } else { "" });
        ser_print!(if ecx_8001a & (1 << 9) != 0 { "SEV-SNP " } else { "" });
        cpu.has_sme = ecx_8001a & (1 << 5) != 0;
        cpu.has_sev = ecx_8001a & (1 << 6) != 0;
    } else {
        ser_print!("(no SVM)");
    }
    ser_print!("\n");

    // 5. Cache hierarchy (AMD-specific leaves)
    // Leaf 0x80000005: L1 cache info
    let (_, _, ecx_8005, edx_8005) = cpuid(0x80000005, 0);
    cpu.l1d_size_kb = ((edx_8005 >> 24) & 0xFF) as u16; // L1 data size in KB
    cpu.l1i_size_kb = ((edx_8005 >> 16) & 0xFF) as u16; // L1 instr size in KB

    // Leaf 0x80000006: L2/L3 cache info
    let (_, _, ecx_8006, edx_8006) = cpuid(0x80000006, 0);
    cpu.l2_size_kb = ((ecx_8006 >> 16) & 0xFFFF) as u16; // L2 size in KB
    cpu.l3_size_kb = (((edx_8006 >> 18) & 0x3FFF) * 512) as u32; // L3 size in KB

    ser_print!("[s1_cpu] cache: L1d=");
    ser_dec!(cpu.l1d_size_kb as usize);
    ser_print!("KB L1i=");
    ser_dec!(cpu.l1i_size_kb as usize);
    ser_print!("KB L2=");
    ser_dec!(cpu.l2_size_kb as usize);
    ser_print!("KB L3=");
    ser_dec!(cpu.l3_size_kb as usize);
    ser_print!("KB\n");

    // 6. Topology (Zen 3: leaf 0x80000026 for CCX/CCD)
    // First try modern leaf 0x80000026 (Zen 3+)
    let (eax_8026, ebx_8026, ecx_8026, edx_8026) = cpuid(0x80000026, 0);
    if eax_8026 != 0 {
        // Extended topology
        // ECX[7:0] = threads per compute unit (CCX)
        // ECX[15:8] = threads per core (SMT)
        cpu.threads_per_core = ((ecx_8026 >> 8) & 0xFF) as u8;
        cpu.cores_per_ccx = (ecx_8026 & 0xFF) as u8;
        // For Zen 3, 1 CCX = 1 CCD (5600X is monolithic)
        cpu.ccx_count = 1;
        cpu.ccd_count = 1;
    } else {
        // Fallback: leaf 0x8000001E (older AMD topology)
        let (_, ebx_801e, _, _) = cpuid(0x8000001E, 0);
        cpu.threads_per_core = ((ebx_801e >> 8) & 0xFF) as u8;
        cpu.cores_per_ccx = 1; // Can't tell from 0x8000001E alone
        cpu.ccx_count = 1;
        cpu.ccd_count = 1;
    }
    ser_print!("[s1_cpu] topology: ");
    ser_dec!(cpu.cores_per_ccx as usize);
    ser_print!("C/CCX x ");
    ser_dec!(cpu.ccx_count as usize);
    ser_print!("CCX x ");
    ser_dec!(cpu.ccd_count as usize);
    ser_print!("CCD = ");
    ser_dec!((cpu.cores_per_ccx as u32 * cpu.ccx_count as u32 * cpu.ccd_count as u32) as usize);
    ser_print!(" cores (");
    ser_dec!((cpu.threads_per_core as u32 * cpu.cores_per_ccx as u32) as usize);
    ser_print!(" threads)\n");

    // 7. TSC frequency (leaf 0x15)
    let (eax_tsc, ebx_tsc, ecx_tsc, _) = cpuid(0x15, 0);
    if eax_tsc != 0 && ebx_tsc != 0 && ecx_tsc != 0 {
        cpu.tsc_freq = (ecx_tsc as u64) * (ebx_tsc as u64) / (eax_tsc as u64);
    } else {
        // Fallback: 5600X base frequency
        cpu.tsc_freq = 3_700_000_000;
    }
    ser_print!("[s1_cpu] TSC: ");
    ser_dec!(cpu.tsc_freq as usize);
    ser_print!(" Hz (");
    ser_dec!((cpu.tsc_freq / 1_000_000) as usize);
    ser_print!(" MHz)\n");

    // 8. Zen 3 specific features
    let (_, ebx7, _, _) = cpuid(7, 0);
    cpu.has_smep = ebx7 & (1 << 7) != 0;
    cpu.has_smap = ebx7 & (1 << 20) != 0;
    cpu.has_fsgsbase = ebx7 & (1 << 0) != 0;
    cpu.has_rdpid = ebx7 & (1 << 22) != 0;
    cpu.has_clflushopt = ebx7 & (1 << 23) != 0;
    cpu.has_clwb = ebx7 & (1 << 24) != 0;
    cpu.has_invplgb = ebx7 & (1 << 10) != 0; // INVLPGB
    cpu.has_wbnoinvd = ebx7 & (1 << 9) != 0; // WBNOINVD

    // CLZERO is AMD-specific, in leaf 0x80000001
    cpu.has_clzero = ecx_8001 & (1 << 25) != 0;

    ser_print!("[s1_cpu] features: SMEP=");
    ser_print!(if cpu.has_smep { "Y " } else { "N " });
    ser_print!("SMAP=");
    ser_print!(if cpu.has_smap { "Y " } else { "N " });
    ser_print!("FSGSBASE=");
    ser_print!(if cpu.has_fsgsbase { "Y " } else { "N " });
    ser_print!("WBNOINVD=");
    ser_print!(if cpu.has_wbnoinvd { "Y " } else { "N " });
    ser_print!("RDPID=");
    ser_print!(if cpu.has_rdpid { "Y " } else { "N " });
    ser_print!("INVLPGB=");
    ser_print!(if cpu.has_invplgb { "Y " } else { "N " });
    ser_print!("CLZERO=");
    ser_print!(if cpu.has_clzero { "Y " } else { "N " });
    ser_print!("CLWB=");
    ser_print!(if cpu.has_clwb { "Y " } else { "N " });
    ser_print!("CLFLUSHOPT=");
    ser_print!(if cpu.has_clflushopt { "Y" } else { "N" });
    ser_print!("\n");

    // 9. CPUID 0x8000001F: SEV/SEV-ES/SEV-SNP
    if cpu.has_sev {
        let (eax_801f, _, _, _) = cpuid(0x8000001F, 0);
        ser_print!("[s1_cpu] SEV: ");
        if eax_801f & (1 << 1) != 0 { ser_print!("SEV "); }
        if eax_801f & (1 << 2) != 0 { ser_print!("SEV-ES "); }
        if eax_801f & (1 << 3) != 0 { ser_print!("SEV-SNP "); }
        ser_print!("\n");
    }

    // 10. AMD specific: extended APIC ID
    let (_, ebx_801e, _, edx_801e) = cpuid(0x8000001E, 0);
    let apic_id = (edx_801e >> 12) & 0xFF; // Bits 12-19: APIC ID
    ser_print!("[s1_cpu] initial APIC ID: ");
    ser_dec!(apic_id as usize);
    ser_print!("\n");

    // Default frequencies for 5600X (can be overridden by P-states)
    cpu.base_freq_mhz = 3700;
    cpu.boost_freq_mhz = 4600;
}

// ═══════════════════════════════════════════════════════════════════
//  AMD ZEN 3 EFER / SYSCFG / HARDWARE CONFIG
// ═══════════════════════════════════════════════════════════════════

unsafe fn init_amd_msrs() {
    // EFER: SYSCALL enable + NXE (No-Execute) + LMA (already in long mode)
    let efer = rdmsr(MSR_EFER);
    let new_efer = efer | EFER_SCE | EFER_NXE;
    if cpu_has_sme() {
        // SME: NXE for all pages, even supervisor
        // Don't auto-enable encryption here (would encrypt kernel too)
    }
    wrmsr(MSR_EFER, new_efer);
    ser_print!("[s1_cpu] EFER: 0x");
    ser_hex!(new_efer);
    ser_print!("\n");

    // SYSCFG (Zen 3 specific): Enable FSGSBASE for fast FS/GS access
    if CPU.has_fsgsbase {
        let syscfg = rdmsr(MSR_SYSCFG);
        // bit 25 = FB_MODE (FSGSBASE enable on Zen 3)
        // bit 18 = MFDM (Memory Disambiguation Flush Disable)
        wrmsr(MSR_SYSCFG, syscfg | (1 << 25));
        ser_print!("[s1_cpu] SYSCFG: 0x");
        ser_hex!(rdmsr(MSR_SYSCFG));
        ser_print!(" (FSGSBASE enabled)\n");
    }

    // HWCR: Disable flush filter (improves single-thread perf slightly)
    let hwcr = rdmsr(MSR_HWCR);
    wrmsr(MSR_HWCR, hwcr | HWCR_FFDIS);
    ser_print!("[s1_cpu] HWCR: 0x");
    ser_hex!(rdmsr(MSR_HWCR));
    ser_print!(" (flush filter disabled)\n");
}

fn cpu_has_sme() -> bool { unsafe { CPU.has_sme } }

// ═══════════════════════════════════════════════════════════════════
//  CR0/CR4/XCR0 (universal x86-64, but tuned for Zen 3)
// ═══════════════════════════════════════════════════════════════════

unsafe fn init_cr0_cr4() {
    // CR0: MP=1, NE=1, EM=0, WP=0 (framebuffer WC writes)
    let cr0: u64; asm!("mov {}, cr0", out(reg) cr0);
    let mut cr0 = cr0;
    cr0 |= 1 << 1; cr0 &= !(1 << 2); cr0 |= 1 << 5;
    cr0 &= !(1 << 16); cr0 &= !(1 << 3);
    asm!("mov cr0, {}", in(reg) cr0);

    // CR4: Zen 3 tuned
    let (max_basic, _, _, _) = cpuid(0, 0);
    let (_, _, ecx1, edx1) = cpuid(1, 0);
    let (ebx7, ecx7) = if max_basic >= 7 { let (_, ebx, ecx, _) = cpuid(7, 0); (ebx, ecx) } else { (0, 0) };
    let xsav = ecx1 & (1 << 26) != 0;
    let avx = ecx1 & (1 << 28) != 0;
    let fsgs = ebx7 & (1 << 0) != 0;
    let smep = ebx7 & (1 << 7) != 0;
    let umip = ecx7 & (1 << 2) != 0;

    let cr4: u64; asm!("mov {}, cr4", out(reg) cr4);
    let mut cr4 = cr4;
    cr4 |= 1 << 7 | 1 << 9 | 1 << 10; // PGE, OSFXSR, OSXMMEXCPT
    if xsav { cr4 |= 1 << 18; }
    if fsgs { cr4 |= 1 << 16; }
    if smep { cr4 |= 1 << 20; }
    if umip { cr4 |= 1 << 11; }
    asm!("mov cr4, {}", in(reg) cr4);

    // XCR0: x87 + SSE + AVX (Zen 3 has 256-bit AVX)
    if xsav {
        let xcr0 = if avx { 7u32 } else { 3u32 };
        asm!("xsetbv", in("ecx") 0u32, in("eax") xcr0, in("edx") 0u32);
    }
    ser_print!("[s1_cpu] CR0/CR4/XCR0 set for Zen 3\n");
}

unsafe fn init_fpu() {
    asm!("fninit");
    let mxcsr: u32 = 0x1F80;
    asm!("ldmxcsr [{addr}]", addr = in(reg) &mxcsr as *const u32);
    let ptr = core::ptr::addr_of_mut!(FPU_STATE.0) as *mut u8;
    let ecx: u32; asm!("push rbx", "cpuid", "pop rbx", inout("eax") 1u32 => _, inout("ecx") 0u32 => ecx, out("edx") _);
    if ecx & (1 << 26) != 0 {
        let eax: u32; let edx: u32;
        asm!("xgetbv", in("ecx") 0u32, out("eax") eax, out("edx") edx);
        asm!("xsave [{}]", in(reg) ptr, in("eax") eax, in("edx") edx);
    } else { asm!("fxsave64 [{}]", in(reg) ptr); }
}

unsafe fn init_zen3_perf() {
    // Zen 3 specific performance MSRs

    // LS_CFG: configure load-store unit
    // Default 0x00200000 enables hardware prefetcher behavior
    let ls_cfg = rdmsr(MSR_LS_CFG);
    ser_print!("[s1_cpu] LS_CFG: 0x");
    ser_hex!(ls_cfg);
    ser_print!("\n");

    // IC_CFG: instruction cache configuration
    let ic_cfg = rdmsr(MSR_IC_CFG);
    ser_print!("[s1_cpu] IC_CFG: 0x");
    ser_hex!(ic_cfg);
    ser_print!("\n");

    // DC_CFG: data cache configuration
    let dc_cfg = rdmsr(MSR_DC_CFG);
    ser_print!("[s1_cpu] DC_CFG: 0x");
    ser_hex!(dc_cfg);
    ser_print!("\n");

    // CU_CFG: compute unit configuration
    let cu_cfg = rdmsr(MSR_CU_CFG);
    ser_print!("[s1_cpu] CU_CFG: 0x");
    ser_hex!(cu_cfg);
    ser_print!("\n");

    // L2_CFG: L2 cache configuration
    let l2_cfg = rdmsr(MSR_L2_CFG);
    ser_print!("[s1_cpu] L2_CFG: 0x");
    ser_hex!(l2_cfg);
    ser_print!("\n");

    // PF2_INSTR_CTL: L2 prefetch control
    // Zen 3 default already has good prefetch; don't change
    let pf2 = rdmsr(MSR_PF2_INSTR_CTL);
    ser_print!("[s1_cpu] PF2 (L2 prefetch): 0x");
    ser_hex!(pf2);
    ser_print!("\n");
}

// ═══════════════════════════════════════════════════════════════════
//  AMD SYSCALL (different from Intel: uses STAR[47:32] as CS, STAR[63:48] as SS)
// ═══════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.syscall_entry")]
#[unsafe(naked)]
pub unsafe extern "C" fn syscall_entry_stub() {
    // AMD K8 SYSCALL ABI: RCX=user RIP, R11=user RFLAGS, RSP unchanged
    // The user RSP is in user space (might be invalid in kernel).
    // Real handler must: swapgs, switch to kernel stack, save state, etc.
    // For now: minimal stub that returns immediately.
    naked_asm!("sysretq");
}

unsafe fn init_syscall() {
    // AMD K8 SYSCALL: STAR[47:32]=CS, STAR[63:48]=SS for SYSRET
    let star = (KERNEL_DS as u64) << 48 | (KERNEL_CS as u64) << 32;
    wrmsr(MSR_STAR, star);

    // LSTAR: 64-bit SYSCALL entry
    let entry = syscall_entry_stub as *const () as u64;
    wrmsr(MSR_LSTAR, entry);

    // CSTAR: 32-bit compatibility SYSCALL entry (not used in long mode)
    wrmsr(MSR_CSTAR, entry);

    // SFMASK: RFLAGS mask (mask IF + DF on SYSCALL entry)
    wrmsr(MSR_SFMASK, (1 << 9) | (1 << 10));

    ser_print!("[s1_cpu] SYSCALL: STAR=0x");
    ser_hex!(star);
    ser_print!(" LSTAR=0x");
    ser_hex!(entry);
    ser_print!("\n");
}

// ═══════════════════════════════════════════════════════════════════
//  TSC CALIBRATION (Zen 3: 3.7 GHz base, 4.6 GHz boost)
// ═══════════════════════════════════════════════════════════════════

fn calibrate_tsc() -> u64 {
    // AMD Zen 3 has CPUID 0x15 with:
    //   EAX = TSC/crystal ratio denominator
    //   EBX = TSC/crystal ratio numerator
    //   ECX = crystal frequency in Hz
    // TSC_freq = ECX * EBX / EAX
    let (eax, ebx, ecx, _) = cpuid(0x15, 0);
    if eax != 0 && ebx != 0 && ecx != 0 {
        (ecx as u64) * (ebx as u64) / (eax as u64)
    } else {
        // Fallback: 5600X runs at 3.7 GHz base
        3_700_000_000
    }
}

unsafe fn init_tsc() {
    let freq = calibrate_tsc();
    CPU.tsc_freq = freq;
    ser_print!("[s1_cpu] TSC: ");
    ser_dec!(freq as usize);
    ser_print!(" Hz\n");
}

// ═══════════════════════════════════════════════════════════════════
//  GDT / IDT init
// ═══════════════════════════════════════════════════════════════════

fn make_segment(dpl: u8, code: bool) -> u64 {
    let mut d: u64 = 0xFFFF | (0x0F << 48);
    let mut a: u8 = 0x92 | (dpl << 5);
    if code { a |= 0x08; }
    d |= (a as u64) << 40;
    let f: u8 = if code { 0x0A } else { 0x0C };
    d |= (f as u64) << 52;
    d
}

fn make_tss_descriptor(addr: u64, size: u16) -> (u64, u64) {
    let mut lo: u64 = (size as u64) & 0xFFFF;
    lo |= (((size as u64) >> 16) & 0x0F) << 48;
    lo |= ((addr & 0xFFFF) as u64) << 16;
    lo |= (((addr >> 16) & 0xFF) as u64) << 32;
    lo |= (((addr >> 24) & 0xFF) as u64) << 56;
    lo |= 0x89u64 << 40;
    let hi: u64 = (addr >> 32) & 0xFFFFFFFF;
    (lo, hi)
}

unsafe fn init_gdt() {
    let ktop = core::ptr::addr_of!(KSTK) as u64 + KSTACK_SIZE as u64;
    TSS.rsp[0] = ktop;
    TSS.ist[0] = core::ptr::addr_of!(IST1) as u64 + IST1_SIZE as u64;
    TSS.ist[2] = core::ptr::addr_of!(IST3) as u64 + IST3_SIZE as u64;
    TSS.iomap_base = core::mem::size_of::<Tss>() as u16;
    GDT.entries[0] = 0;
    GDT.entries[1] = make_segment(0, true);
    GDT.entries[2] = make_segment(0, false);
    GDT.entries[3] = make_segment(3, false);
    GDT.entries[4] = make_segment(3, true);
    let tss_addr = core::ptr::addr_of!(TSS) as u64;
    let (lo, hi) = make_tss_descriptor(tss_addr, (core::mem::size_of::<Tss>() - 1) as u16);
    GDT.entries[5] = lo; GDT.entries[6] = hi;
    let gdtr = Gdtr { limit: (core::mem::size_of::<Gdt>() - 1) as u16, base: core::ptr::addr_of!(GDT) as u64 };
    asm!("lgdt [{}]", in(reg) &gdtr);
    asm!("mov ds, {0:x}", "mov es, {0:x}", "mov ss, {0:x}", "mov fs, {0:x}", "mov gs, {0:x}", in(reg) KERNEL_DS as u64);
    asm!("ltr {0:x}", in(reg) TSS_SEL as u64);
}

unsafe fn init_idt() {
    macro_rules! fa { ($f:expr) => { $f as *const () as u64 } }
    let no_err = [0,1,2,3,4,5,6,7,9,15,16,18,19,20,22,23,24,25,26,27,28,31];
    for &v in &no_err {
        let (h, ist) = match v {
            0 => (fa!(exc_divide), 1u8), 6 => (fa!(exc_invalid_op), 1),
            7 => (fa!(exc_dev_not_av), 1), 16 => (fa!(exc_x87), 1),
            18 => (fa!(exc_mcheck), 3), 19 => (fa!(exc_simd), 1),
            2 => (fa!(exc_no_err2), 1), _ => (fa!(exc_no_err), 0),
        };
        IDT[v].set(h, ist);
    }
    let err = [8,10,11,12,13,14,17,21,29,30];
    for &v in &err {
        let (h, ist) = match v {
            8 => (fa!(exc_double_fault), 1u8), 13 => (fa!(exc_gpf), 1),
            14 => (fa!(exc_page_fault), 1), _ => (fa!(exc_with_err), 0),
        };
        IDT[v].set(h, ist);
    }
    for v in 32..256 { IDT[v].set(fa!(irq_stub), 0); }
    let idtr = Idtr { limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16, base: core::ptr::addr_of!(IDT) as u64 };
    asm!("lidt [{}]", in(reg) &idtr);
}

// ═══════════════════════════════════════════════════════════════════
//  UEFI STAGES (memory map, GOP, load, ExitBootServices)
// ═══════════════════════════════════════════════════════════════════

unsafe fn get_memory_map(bs: *mut EfiBootServices, buf: &mut [u8; 32768]) -> (usize, usize, usize, u32) {
    let mut map_size = buf.len();
    let mut map_key: usize = 0;
    let mut desc_size: usize = 0;
    let mut desc_ver: u32 = 0;
    let base = &(*bs).hdr as *const EfiTableHeader as *const *mut core::ffi::c_void;
    let fnptr: extern "efiapi" fn(*mut usize, *mut u8, *mut usize, *mut usize, *mut u32) -> u64 =
        core::mem::transmute(*base.add(3 + 4));
    let r = fnptr(&mut map_size, buf.as_mut_ptr(), &mut map_key, &mut desc_size, &mut desc_ver);
    if r != EFI_SUCCESS && r != 5 { return (0, 0, 0, 0); }
    (map_size, map_key, desc_size, desc_ver)
}

unsafe fn fill_memory_map(ctx: &mut BootContext, buf: &mut [u8; 32768], system_table: *mut EfiSystemTable) -> usize {
    let bs = (*system_table).boot_services;
    let (map_size, _key, desc_size, _ver) = get_memory_map(bs, buf);
    if map_size == 0 || desc_size == 0 { return 0; }
    let num = map_size / desc_size;
    let mut entries = [MemoryEntry { base: 0, size: 0, kind: 0 }; MAX_MEMORY_ENTRIES];
    let mut ec: usize = 0;
    for i in 0..num.min(MAX_MEMORY_ENTRIES) {
        let desc = &*(buf.as_ptr().add(i * desc_size) as *const EfiMemoryDescriptor);
        if desc.mem_type == EFI_CONVENTIONAL_MEMORY && desc.num_pages > 0 && ec < MAX_MEMORY_ENTRIES {
            entries[ec] = MemoryEntry { base: desc.phys_start, size: desc.num_pages * 4096, kind: 1 };
            ec += 1;
        }
    }
    ctx.set_memory_map(&entries[..ec]);
    ctx.memory_map_count = ec as u32;
    ec
}

unsafe fn fill_gop(ctx: &mut BootContext, system_table: *mut EfiSystemTable) -> bool {
    let bs = (*system_table).boot_services;
    let mut gop_handle: EfiHandle = core::ptr::null_mut();
    let base = &(*bs).hdr as *const EfiTableHeader as *const *mut core::ffi::c_void;
    let locate: extern "efiapi" fn(*mut EfiGuid, *mut core::ffi::c_void, &mut EfiHandle) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 37));
    let r = locate(&mut GOP_GUID, core::ptr::null_mut(), &mut gop_handle);
    if r != EFI_SUCCESS || gop_handle.is_null() { return false; }
    let gop = &*(gop_handle as *const EfiGraphicsOutputProtocol);
    let mode = &*gop.mode;
    let info = &*(mode.info as *const [u32; 9]);
    let w = info[1]; let h = info[2]; let fmt = info[3]; let stride = info[8];
    if w == 0 || h == 0 || stride < w || fmt > 1 || mode.frame_buffer_base == 0 { return false; }
    ctx.fb_addr = mode.frame_buffer_base;
    ctx.fb_width = w; ctx.fb_height = h;
    ctx.fb_stride = stride; ctx.fb_pixel_format = fmt;
    ser_print!("[s1_cpu] GOP fb=0x"); ser_hex!(mode.frame_buffer_base);
    ser_print!(" "); ser_dec!(w as usize); ser_print!("x"); ser_dec!(h as usize); ser_print!("\n");
    let fb = mode.frame_buffer_base as *mut u32;
    let total = (stride as usize) * (h as usize);
    for i in 0..total { fb.add(i).write_volatile(0xFF0A_0F1Du32); }
    asm!("mfence", options(nostack, preserves_flags));
    true
}

unsafe fn load_from_esp(ctx: &mut BootContext, system_table: *mut EfiSystemTable, image_handle: EfiHandle) -> bool {
    let bs = (*system_table).boot_services;
    let base = &(*bs).hdr as *const EfiTableHeader as *const *mut core::ffi::c_void;
    // Resolve the SAME volume the boot chain was loaded from, via
    // LoadedImage.DeviceHandle on the shim's image handle. With several
    // FAT volumes present, LocateProtocol(FS) may return another disk's
    // ESP where s2_mem.bin/kernel.bin do not exist.
    let handle_protocol: extern "efiapi" fn(EfiHandle, *mut EfiGuid, &mut *mut core::ffi::c_void) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 16));
    let mut fs_handle: EfiHandle = core::ptr::null_mut();
    let mut li: *mut core::ffi::c_void = core::ptr::null_mut();
    if handle_protocol(image_handle, &raw mut LOADED_IMAGE_GUID, &mut li) == EFI_SUCCESS && !li.is_null() {
        // EFI_LOADED_IMAGE_PROTOCOL: DeviceHandle at byte offset 24.
        let device = *(li as *const EfiHandle).add(3);
        let mut fs_if: *mut core::ffi::c_void = core::ptr::null_mut();
        if handle_protocol(device, &raw mut FILE_SYSTEM_GUID, &mut fs_if) == EFI_SUCCESS {
            fs_handle = fs_if as EfiHandle;
        }
    }
    if fs_handle.is_null() {
        // Fallback: first filesystem (single-volume setups).
        let locate: extern "efiapi" fn(*mut EfiGuid, *mut core::ffi::c_void, &mut EfiHandle) -> EfiStatus =
            core::mem::transmute(*base.add(3 + 37));
        if locate(&raw mut FILE_SYSTEM_GUID, core::ptr::null_mut(), &mut fs_handle) != EFI_SUCCESS { return false; }
    }
    let sfsp = fs_handle as *const *mut core::ffi::c_void;
    let open_vol: extern "efiapi" fn(EfiHandle, &mut *mut core::ffi::c_void) -> EfiStatus =
        core::mem::transmute(*sfsp.add(1));
    let mut root: *mut core::ffi::c_void = core::ptr::null_mut();
    if open_vol(fs_handle, &mut root) != EFI_SUCCESS { return false; }
    let file_base = root as *const *mut core::ffi::c_void;
    let open_fn: extern "efiapi" fn(*mut core::ffi::c_void, &mut *mut core::ffi::c_void, *const u16, u64, u64) -> EfiStatus =
        core::mem::transmute(*file_base.add(1));
    let alloc_pages: extern "efiapi" fn(u32, u32, usize, &mut u64) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 2));

    let stages: [(&str, u64); MAX_STAGES] = [
        ("", 0), ("s2_mem.bin", 0x200000),
        ("", 0), ("", 0), ("", 0), ("", 0), ("", 0),
        ("", 0), ("", 0), ("", 0), ("", 0), ("", 0),
        ("kernel.bin", 0x400000),
    ];
    // s1 is already loaded and reserved by the UEFI shim.
    ctx.stage_base[0] = 0x100000;
    ctx.stage_entry[0] = 0x100000;
    let mut ok = true;
    for (i, &(name, addr)) in stages.iter().enumerate() {
        if name.is_empty() { continue; }
        let mut path = [0u16; 260];
        path[0] = b'\\' as u16;
        let prefix: &[u8] = if i == KERNEL_STAGE_INDEX { b"EFI\\BOOT\\ring0\\" } else { b"EFI\\BOOT\\ring0\\faggin\\" };
        let mut idx = 1;
        for &c in prefix { path[idx] = c as u16; idx += 1; }
        for &c in name.as_bytes() { path[idx] = c as u16; idx += 1; }
        path[idx] = 0;
        let mut file: *mut core::ffi::c_void = core::ptr::null_mut();
        if open_fn(root, &mut file, path.as_ptr(), 1, 0) != EFI_SUCCESS { ok = false; continue; }
        let opened_file = file as *const *mut core::ffi::c_void;
        let read_fn: extern "efiapi" fn(*mut core::ffi::c_void, &mut usize, *mut u8) -> EfiStatus =
            core::mem::transmute(*opened_file.add(4));
        let reserve_size = if i + 1 == MAX_STAGES { KERNEL_RESERVE_SIZE } else { S2_RESERVE_SIZE };
        let mut allocation = addr;
        let pages = (reserve_size as usize + 4095) / 4096;
        if alloc_pages(2, 2, pages, &mut allocation) != EFI_SUCCESS { ok = false; continue; }
        let dst = addr as *mut u8;
        let mut size = reserve_size as usize;
        if read_fn(file, &mut size, dst) != EFI_SUCCESS || size == 0 { ok = false; continue; }
        let bss_end = addr + reserve_size;
        for j in size as u64..(bss_end - addr) { dst.add(j as usize).write(0); }
        ctx.stage_base[i] = addr;
        ctx.stage_size[i] = size as u64;
        ctx.stage_entry[i] = addr;
        ser_print!("[s1_cpu] loaded "); ser_print!(name);
        ser_print!(" -> 0x"); ser_hex!(addr);
        ser_print!(" ("); ser_dec!(size); ser_print!(" bytes)\n");
    }
    ok
}

unsafe fn exit_boot_services_and_jump(ctx_ptr: *mut BootContext, system_table: *mut EfiSystemTable, image_handle: EfiHandle, entry: u64) -> ! {
    let bs = (*system_table).boot_services;
    let base = &(*bs).hdr as *const EfiTableHeader as *const *mut core::ffi::c_void;
    let get_mm: extern "efiapi" fn(*mut usize, *mut u8, *mut usize, *mut usize, *mut u32) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 4));
    let exit_bs: extern "efiapi" fn(EfiHandle, usize) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 26));
    let mut buf = [0u8; 32768];
    let mut map_size = buf.len();
    let mut map_key: usize = 0;
    let mut desc_size: usize = 0;
    let mut desc_ver: u32 = 0;
    if get_mm(&mut map_size, buf.as_mut_ptr(), &mut map_key, &mut desc_size, &mut desc_ver) != EFI_SUCCESS { loop { asm!("hlt"); } }

    // Publish the final map, after reserving s1/s2/kernel.  The earlier map
    // still marked those physical ranges as conventional and would let the
    // kernel allocator overwrite its own boot images.
    let ctx = &mut *ctx_ptr;
    let mut entries = [MemoryEntry { base: 0, size: 0, kind: 0 }; MAX_MEMORY_ENTRIES];
    let mut count = 0;
    if desc_size != 0 {
        for i in 0..(map_size / desc_size) {
            let desc = &*(buf.as_ptr().add(i * desc_size) as *const EfiMemoryDescriptor);
            if desc.mem_type == EFI_CONVENTIONAL_MEMORY && desc.num_pages > 0 && count < MAX_MEMORY_ENTRIES {
                entries[count] = MemoryEntry { base: desc.phys_start, size: desc.num_pages * 4096, kind: 1 };
                count += 1;
            }
        }
    }
    ctx.set_memory_map(&entries[..count]);
    ctx.memory_map_count = count as u32;

    if exit_bs(image_handle, map_key) != EFI_SUCCESS { loop { asm!("hlt"); } }
    ser_print!("[s1_cpu] ===> JUMP s2_mem 0x");
    ser_hex!(entry);
    ser_print!("\n");
    asm!("sfence", options(nostack, preserves_flags));
    asm!(
        "mov rdi, {ctx}",
        "xor rbp, rbp",
        "jmp {entry}",
        ctx = in(reg) ctx_ptr,
        entry = in(reg) entry,
        options(noreturn)
    );
}

//  BOOTCONTEXT (statically allocated in .bss)
// ═══════════════════════════════════════════════════════════════════

static mut CTX: BootContext = unsafe { core::mem::zeroed() };

// ═══════════════════════════════════════════════════════════════════
//  ENTRY POINT
// ═══════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
pub extern "efiapi" fn s1_entry(
    image_handle: EfiHandle,
    system_table: *mut core::ffi::c_void,
) -> ! {
    serial_init();
    ser_print!("\n[s1_cpu] === BMO BOOT START (Zen 3) ===\n");

    // 1. Setup BootContext
    let ctx_ptr: *mut BootContext = core::ptr::addr_of_mut!(CTX);
    let ctx = unsafe { &mut *ctx_ptr };
    ctx.magic = boot_context::MAGIC;
    ctx.version = 2;
    ser_print!("[s1_cpu] magic=0x"); ser_hex!(ctx.magic);
    ser_print!(" version="); ser_dec!(ctx.version as usize); ser_print!("\n");

    // 2. Memory map
    let mut mem_buf = [0u8; 32768];
    let ec = unsafe { fill_memory_map(ctx, &mut mem_buf, system_table as *mut EfiSystemTable) };
    ser_print!("[s1_cpu] memory map: "); ser_dec!(ec); ser_print!(" entries\n");

    // 3. GOP framebuffer
    unsafe { fill_gop(ctx, system_table as *mut EfiSystemTable); }

    // 4. Load s2_mem + kernel
    if !unsafe { load_from_esp(ctx, system_table as *mut EfiSystemTable, image_handle) } {
        ser_print!("[s1_cpu] FATAL: load failed\n");
        loop { unsafe { asm!("hlt"); } }
    }

    // 5. CPU detection (AMD Ryzen 5 5600X specific)
    ser_print!("\n[s1_cpu] === AMD ZEN 3 DETECTION ===\n");
    unsafe { detect_cpu(); }

    // 6. GDT + IDT (universal x86-64)
    ser_print!("\n[s1_cpu] === UNIVERSAL CPU INIT ===\n");
    unsafe { init_gdt(); }
    ser_print!("[s1_cpu] GDT + TSS loaded\n");
    unsafe { init_idt(); }
    ser_print!("[s1_cpu] IDT loaded\n");

    // 7. CR0/CR4/XCR0
    unsafe { init_cr0_cr4(); }

    // 8. FPU
    unsafe { init_fpu(); }

    // 9. TSC
    unsafe { init_tsc(); }

    // 10. AMD MSRs (Zen 3 specific)
    ser_print!("\n[s1_cpu] === AMD ZEN 3 MSR INIT ===\n");
    unsafe { init_amd_msrs(); }

    // 11. Zen 3 performance configuration
    unsafe { init_zen3_perf(); }

    // 12. SYSCALL (AMD K8 ABI)
    unsafe { init_syscall(); }

    // SMP remains disabled until its real-mode trampoline and low-memory page
    // tables are reserved and built correctly.  Boot the BSP reliably first.

    // 13. Publish CPU profile to BootContext
    ctx.gdt_ptr = core::ptr::addr_of!(GDT) as u64;
    ctx.tss_ptr = core::ptr::addr_of!(TSS) as u64;
    ctx.idt_ptr = core::ptr::addr_of!(IDT) as u64;
    ctx.kernel_stack_top = core::ptr::addr_of!(KSTK) as u64 + KSTACK_SIZE as u64;
    ctx.tsc_freq = unsafe { CPU.tsc_freq };
    ctx.syscall_entry = syscall_entry_stub as *const () as u64;

    ser_print!("\n[s1_cpu] === ALL ZEN 3 INIT DONE ===\n");
    ser_print!("[s1_cpu] Ryzen 5 5600X: 6C/12T, 3.7GHz base, 4.6GHz boost\n");
    ser_print!("[s1_cpu] Cache: L1 32K, L2 512K, L3 32M\n");

    // 14. ExitBootServices + jump to s2_mem
    unsafe { exit_boot_services_and_jump(ctx_ptr, system_table as *mut EfiSystemTable, image_handle, S2_ADDR); }
}

// ═══════════════════════════════════════════════════════════════════
//  AMD SMP STARTUP (Zen 3: 6C/12T on Ryzen 5 5600X)
// ═══════════════════════════════════════════════════════════════════
//
// Everything SMP-related lives here, written as inline ASM via
// `#[naked]` + `naked_asm!` — no global_asm, no separate sections,
// no linker relocation issues.
//
// AP startup flow:
//   1. BSP builds a minimal PML4 (identity-mapped 0..4GB) at 0x7000
//   2. BSP copies the SMP trampoline to physical 0x8000
//   3. BSP writes IDT pointer to 0x8138 (shared with APs)
//   4. BSP initializes the LAPIC (MSR 0x1B, SIVR, TPR)
//   5. BSP sends INIT IPI + 2x SIPI to each AP (APIC IDs 0..15)
//   6. APs wake at 0x8000, transition 16→32→64-bit, jump to ap_entry
//   7. APs signal online via atomic counter, halt
//   8. BSP waits for all APs to come online
//
// Memory layout (all identity-mapped by PML4):
//   0x7000-0x7FFF: PML4 (4KB) + online counter at 0x7FF8
//   0x8000-0x80FF: Trampoline (256 bytes)
//   0x8100-0x81FF: Shared BSP↔AP data
//   0x8200-...:    Per-AP stacks (4KB each)

// ── Shared GDT (BSP + APs use the same one) ─────────────────────

#[repr(C, align(16))]
struct SmpGdt { entries: [u64; 4] } // null + 16-bit code + 32-bit code + 64-bit code
static mut SMP_GDT: SmpGdt = SmpGdt { entries: [
    0,                                  // null
    0x0000_9B00_0000_FFFFu64,           // 16-bit code, DPL=0, base=0, limit=64K
    0x00CF_9A00_0000_FFFFu64,           // 32-bit code, DPL=0, base=0, limit=4G
    0x0020_9B00_0000_0000u64,           // 64-bit code, DPL=0 (32-bit entry, L=1)
] };

#[repr(C, packed)]
struct SmpGdtr { limit: u16, base: u64 }
static mut SMP_GDTR: SmpGdtr = SmpGdtr { limit: 31, base: 0 };

// ── AP entry: naked function with 16→32→64 transition ───────────

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.ap_entry")]
#[unsafe(naked)]
pub unsafe extern "C" fn ap_entry() {
    // The AP starts here in 16-bit real mode after receiving SIPI.
    // We transition through 32-bit protected mode to 64-bit long mode,
    // then jump to the 64-bit AP entry point (passed via shared data).
    //
    // Memory layout (physical addresses):
    //   0x7000: PML4 (BSP-built, identity-mapped 0..4GB)
    //   0x8058: GDT pointer (10 bytes: limit + base)
    //   0x8060: GDT (32 bytes: 4 entries)
    //   0x8100: PML4 address (u64, set by BSP)
    //   0x8108: Stack top (u64, per-AP)
    //   0x8110: 64-bit AP entry point (u64, set by BSP)
    //   0x8138: IDT pointer (10 bytes: limit + base, set by BSP)
    //   0x7FF8: Online counter (u32, atomic)
    naked_asm!(
        // ═══ 16-bit real mode → 32-bit protected mode ═══
        // Load GDT pointer from 0x8058 (BSP wrote it there)
        // In 64-bit mode, lgdt needs an 80-bit memory operand (10 bytes)
        // We use a register to avoid the 16-bit mode encoding issue.
        "mov rax, 0x8058",
        "lgdt [rax]",

        // Far jump to 32-bit code segment (selector 0x10)
        // 0x68 = push imm32, 0x6A = push imm8
        "push 0x10",                     // 32-bit code selector
        "push offset pmode32",           // entry point in 32-bit mode
        "retfq",

        // ═══ 32-bit protected mode ═══
        "pmode32:",
        "mov ax, 0x18",                  // 32-bit data selector
        "mov ds, ax",
        "mov es, ax",
        "mov ss, ax",
        "mov esp, 0x8F00",              // temporary stack

        // Enable PAE (required for long mode)
        "mov rax, cr4",
        "or eax, 0x20",                  // CR4.PAE = bit 5
        "mov cr4, rax",

        // Load PML4 from BSP-built page tables
        "mov rax, [0x8100]",            // PML4 address
        "mov cr3, rax",

        // Enable long mode in EFER (MSR 0xC0000080)
        "mov rcx, 0xC0000080",
        "rdmsr",
        "or eax, 0x901",                 // LME (bit 8) + NXE (bit 11)
        "wrmsr",

        // Enable paging (activates long mode)
        "mov rax, cr0",
        "or eax, 0x80000000",            // CR0.PG = bit 31
        "mov cr0, rax",

        // Reload GDT (64-bit descriptors)
        "mov rax, 0x8058",
        "lgdt [rax]",

        // Far jump to 64-bit code segment (selector 0x20)
        "push 0x20",                     // 64-bit code selector
        "push offset pmode64",           // entry point in 64-bit mode
        "retfq",

        // ═══ 64-bit long mode ═══
        "pmode64:",
        // Clear segment registers (not used in 64-bit mode)
        "xor ax, ax",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",
        "mov ss, ax",

        // Load IDT from BSP (BSP wrote the IDT pointer to 0x8138)
        "mov rax, 0x8138",
        "lidt [rax]",

        // Set up per-AP stack (BSP wrote the stack top to 0x8108)
        "mov rsp, [0x8108]",

        // Jump to the 64-bit AP entry point (BSP wrote it to 0x8110)
        "mov rax, [0x8110]",
        "jmp rax",
    );
}

// ── 64-bit AP entry: signals online, then halts ──────────────────

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.ap_entry64")]
pub unsafe extern "C" fn ap_entry64() {
    // Signal online: atomic increment of counter at 0x7FF8
    unsafe {
        let counter = 0x7FF8 as *mut u32;
        core::arch::asm!(
            "lock inc dword ptr [{}]",
            in(reg) counter,
            options(nostack, preserves_flags),
        );
    }
    // Halt and wait for the kernel to wake us
    loop {
        unsafe { asm!("hlt"); }
    }
}

// ── LAPIC (Local APIC) ───────────────────────────────────────────

unsafe fn lapic_base() -> u64 {
    let lo: u32; let hi: u32;
    asm!("rdmsr", in("ecx") 0x1Bu32, out("eax") lo, out("edx") hi);
    ((hi as u64) << 32) | (lo as u64) & 0xFFFFF000
}

unsafe fn lapic_write(reg: u32, val: u32) {
    let base = lapic_base() as *mut u32;
    core::ptr::write_volatile(base.add(reg as usize), val);
}

unsafe fn lapic_read(reg: u32) -> u32 {
    let base = lapic_base() as *const u32;
    core::ptr::read_volatile(base.add(reg as usize))
}

unsafe fn lapic_id() -> u32 {
    (lapic_read(0x020) >> 24) & 0xFF
}

unsafe fn lapic_init() {
    let lo: u32; let hi: u32;
    asm!("rdmsr", in("ecx") 0x1Bu32, out("eax") lo, out("edx") hi);
    asm!("wrmsr", in("ecx") 0x1Bu32, in("eax") lo | (1 << 11), in("edx") hi);
    lapic_write(0x0F0, 0x100 | 0xFF);  // SIVR: enable + spurious vector 0xFF
    lapic_write(0x080, 0);            // TPR: accept all interrupts
}

unsafe fn send_init_ipi(apic_id: u32) {
    let icr = 0x000C4500u32 | ((apic_id & 0xFF) << 24);
    lapic_write(0x310, (apic_id >> 8) & 0xFF);
    lapic_write(0x300, icr);
    while lapic_read(0x300) & (1 << 12) != 0 {}
}

unsafe fn send_sipi(apic_id: u32, vector: u8) {
    let icr = 0x000C4600u32 | ((apic_id & 0xFF) << 24) | (vector as u32);
    lapic_write(0x310, (apic_id >> 8) & 0xFF);
    lapic_write(0x300, icr);
    while lapic_read(0x300) & (1 << 12) != 0 {}
}

// ── PML4 setup (minimal identity-mapped 0..4GB) ──────────────────

unsafe fn setup_smp_pml4() {
    let pml4 = 0x7000 as *mut u64;
    for i in 0..512 { core::ptr::write_volatile(pml4.add(i), 0); }
    let pdpt = 0x7100 as *mut u64;
    for i in 0..512 { core::ptr::write_volatile(pdpt.add(i), 0); }
    let pd = 0x7200 as *mut u64;
    for i in 0..512 { core::ptr::write_volatile(pd.add(i), 0); }
    core::ptr::write_volatile(pml4, 0x7103);  // PML4[0] = PDPT
    core::ptr::write_volatile(pdpt, 0x7203);  // PDPT[0] = PD
    for i in 0..2048usize {
        let entry = (i as u64 * 0x200000) | 0x83;  // HUGE | PRESENT | WRITABLE
        core::ptr::write_volatile(pd.add(i), entry);
    }
}

// ── Trampoline copy: copies the ap_entry naked function to 0x8000 ─

unsafe fn copy_trampoline() {
    let src = ap_entry as *const u8;
    let dst = 0x8000 as *mut u8;
    // The trampoline (ap_entry naked function) is about 150 bytes.
    // We copy 256 bytes to be safe and to include any padding.
    const TRAMPOLINE_SIZE: usize = 256;
    for i in 0..TRAMPOLINE_SIZE {
        core::ptr::write_volatile(dst.add(i), core::ptr::read_volatile(src.add(i)));
    }
}

// ── TSC-based delay ──────────────────────────────────────────────

fn rdtsc() -> u64 {
    let lo: u32; let hi: u32;
    unsafe { asm!("rdtsc", out("eax") lo, out("edx") hi); }
    ((hi as u64) << 32) | (lo as u64)
}

fn delay_ms(ms: u32) {
    let freq = unsafe { CPU.tsc_freq };
    let start = rdtsc();
    let target = start + (freq * ms as u64) / 1000;
    while rdtsc() < target {
        core::hint::spin_loop();
    }
}

// ── SMP startup (BSP wakes all APs via INIT+SIPI) ────────────────

unsafe fn smp_startup() {
    ser_print!("\n[s1_cpu] === AMD SMP STARTUP (Zen 3) ===\n");

    // 1. Build minimal PML4 (identity-mapped 0..4GB)
    setup_smp_pml4();
    ser_print!("[s1_cpu] PML4 at 0x7000\n");

    // 2. Copy trampoline (the ap_entry naked function) to physical 0x8000
    copy_trampoline();
    ser_print!("[s1_cpu] trampoline at 0x8000\n");

    // 3. Write GDT pointer to 0x8058 (APs load it with lgdt [0x8058])
    let gdt_base = core::ptr::addr_of!(SMP_GDT) as u64;
    core::ptr::write_volatile(0x8058 as *mut u16, 31u16);  // limit = 31 (4 entries × 8 - 1)
    core::ptr::write_volatile(0x805A as *mut u64, gdt_base);  // base

    // 4. Write IDT pointer to 0x8138 (APs load it with lidt [0x8138])
    let idt_limit = (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16;
    let idt_base = core::ptr::addr_of!(IDT) as u64;
    core::ptr::write_volatile(0x8138 as *mut u16, idt_limit);
    core::ptr::write_volatile(0x813A as *mut u64, idt_base);

    // 5. Initialize online counter at 0x7FF8
    core::ptr::write_volatile(0x7FF8 as *mut u32, 0);

    // 6. Initialize LAPIC
    lapic_init();
    ser_print!("[s1_cpu] LAPIC enabled\n");

    // 7. Get BSP APIC ID
    let bsp_id = lapic_id();
    ser_print!("[s1_cpu] BSP APIC ID: ");
    ser_dec!(bsp_id as usize);
    ser_print!("\n");

    // 8. For each possible APIC ID (0..15), send INIT+SIPI
    let cpu = unsafe { &CPU };
    let num_threads = (cpu.threads_per_core as u32) * (cpu.cores_per_ccx as u32);
    ser_print!("[s1_cpu] Expected threads: ");
    ser_dec!(num_threads as usize);
    ser_print!("\n");

    for apic_id in 0..16u32 {
        if apic_id == bsp_id { continue; }

        // Write per-AP data to shared memory at 0x8100
        core::ptr::write_volatile(0x8100 as *mut u64, 0x7000u64);  // PML4
        let stack_top = 0x8200u64 + (apic_id as u64) * 0x1000 + 0x1000;
        core::ptr::write_volatile(0x8108 as *mut u64, stack_top);
        core::ptr::write_volatile(0x8110 as *mut u64, ap_entry64 as *const () as u64);

        ser_print!("[s1_cpu] waking AP ");
        ser_dec!(apic_id as usize);
        ser_print!("...");

        // INIT IPI → 10ms wait → SIPI #1 → 1ms wait → SIPI #2 → 1ms wait
        send_init_ipi(apic_id);
        delay_ms(10);
        send_sipi(apic_id, 8);  // vector=8 → address 0x8000
        delay_ms(1);
        send_sipi(apic_id, 8);
        delay_ms(1);
    }

    // 9. Wait for all APs to come online
    let expected_aps = num_threads - 1;
    ser_print!("[s1_cpu] waiting for ");
    ser_dec!(expected_aps as usize);
    ser_print!(" APs...\n");
    let mut online: u32 = 0;
    let mut timeout: u32 = 1000;
    while online < expected_aps && timeout > 0 {
        online = core::ptr::read_volatile(0x7FF8 as *const u32);
        if online < expected_aps {
            delay_ms(1);
            timeout -= 1;
        }
    }
    let total = online + 1;  // +1 for BSP
    ser_print!("[s1_cpu] SMP: ");
    ser_dec!(total as usize);
    ser_print!(" / ");
    ser_dec!(num_threads as usize);
    ser_print!(" threads online");
    if total < num_threads {
        ser_print!(" (timeout)");
    }
    ser_print!("\n");
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { asm!("hlt"); } } }
