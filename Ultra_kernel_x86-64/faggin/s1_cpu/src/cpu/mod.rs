//! **THE CPU BRING-UP CONTRACT** -- what every x86-64 needs, and the seam where
//! one particular CPU plugs in.
//!
//! === Why this folder exists ===
//!
//! `kernel/src/ring0/cpu_vendor/profile.rs` already states the rule for the
//! kernel:
//!
//! > *"swapping the CPU (or the vendor) is a profile swap, never a kernel edit
//! > [...] the rest of Ring 0 consumes only this descriptor -- it never names a
//! > vendor module directly."*
//!
//! ** The first boot stage did not honour it. Zen 3 detection, Zen 3
//! EFER/SYSCFG, Zen 3 performance counters and a 3,7 GHz TSC constant were
//! spread through the same 1.665-line file as the UEFI structs and the serial
//! port. The contract existed one directory away, and the code that runs FIRST
//! was the one ignoring it.
//!
//! === The split, and what it buys ===
//!
//! - **here**: what any x86-64 needs -- `CR0`/`CR4`/`XCR0`, the FPU, the
//!   `SYSCALL` fast path, and the profile descriptor other modules read.
//! - **`detect.rs`**: `CPUID` in, a profile out. It names features, not brands.
//! - **`zen3.rs`**: everything true only of a Ryzen 5 5600X.
//!
//! A second CPU becomes a **new file next to `zen3.rs`**, instead of edits
//! scattered through a boot stage. That is the difference between a system that
//! recognises a CPU and one that was written for one.

/// DETECTION: `CPUID` in, a profile out. It names FEATURES, not brands.
pub mod detect;
/// THE ZEN 3 PROFILE: everything true only of a Ryzen 5 5600X. Its size is a
/// measurement of how much of BMO-X is tied to one machine.
pub mod zen3;

pub use detect::*;
pub use zen3::*;

#[allow(unused_imports)]
use crate::*;

// ===================================================================
//  FPU STATE
// ===================================================================

#[repr(align(64))] pub struct Align64([u8; 1024]);
pub static mut FPU_STATE: Align64 = Align64([0; 1024]);

// ===================================================================
//  AMD RYZEN 5 5600X (Zen 3) CPU PROFILE
// ===================================================================

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct CpuProfile {
    // Vendor info
    pub vendor: [u8; 12],       // "AuthenticAMD"
    pub brand: [u8; 48],        // "AMD Ryzen 5 5600X..."

    // Family/Model/Stepping
    pub family: u8,            // 0x19 (Zen 3)
    pub model: u8,             // 0x21 (5600X)
    pub stepping: u8,          // 0x2 (B2 stepping)

    // Topology (5600X: 1 CCD, 1 CCX, 6 cores, 12 threads)
    pub threads_per_core: u8,  // 2 (SMT enabled)
    pub cores_per_ccx: u8,     // 6 (5600X is monolithic, 1 CCX)
    pub ccx_count: u8,         // 1 (5600X has only 1 CCX)
    pub ccd_count: u8,         // 1 (single CCD)

    // Cache hierarchy
    pub l1d_size_kb: u16,      // 32 KB
    pub l1i_size_kb: u16,      // 32 KB
    pub l2_size_kb: u16,       // 512 KB per core
    pub l3_size_kb: u32,       // 32 MB shared (5600X has 32MB L3)

    // Frequencies
    pub base_freq_mhz: u32,    // 3700
    pub boost_freq_mhz: u32,   // 4600

    // TSC
    pub tsc_freq: u64,         // Calculated from CPUID 0x15

    // Features
    pub has_sme: bool,         // Secure Memory Encryption
    pub has_sev: bool,         // Secure Encrypted Virtualization
    pub has_wbnoinvd: bool,    // Write Back No Invalidate
    pub has_rdpid: bool,       // Read Processor ID
    pub has_invplgb: bool,     // Invalidate TLB Global
    pub has_clzero: bool,      // Cache Line Zero
    pub has_clwb: bool,        // Cache Line Write Back
    pub has_clflushopt: bool,  // Optimized Cache Line Flush
    pub has_smep: bool,        // Supervisor Mode Execution Prevention
    pub has_smap: bool,        // Supervisor Mode Access Prevention
    pub has_fsgsbase: bool,    // FS/GS Base instructions
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

pub static mut CPU: CpuProfile = CpuProfile::empty();


// ===================================================================
//  CR0/CR4/XCR0 (universal x86-64, but tuned for Zen 3)
// ===================================================================

pub unsafe fn init_cr0_cr4() {
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
    let smap = ebx7 & (1 << 20) != 0;

    let cr4: u64; asm!("mov {}, cr4", out(reg) cr4);
    let mut cr4 = cr4;
    cr4 |= 1 << 7 | 1 << 9 | 1 << 10; // PGE, OSFXSR, OSXMMEXCPT
    if xsav { cr4 |= 1 << 18; }
    if fsgs { cr4 |= 1 << 16; }
    if smep { cr4 |= 1 << 20; }
    if umip { cr4 |= 1 << 11; }
    // *** SMAP: Ring 0 no puede TOCAR una pagina de Ring 3 (2026-08-24).
    //
    // Es el cuarto de los bits de guardia y el ultimo en llegar, porque no era
    // un bit: el kernel SI tocaba memoria de usuario en dos sitios y habia que
    // quitarlos antes. Los dos eran `ARCH_OP_LEER_EN`/`ESCRIBIR_DE`, y los dos
    // pasan ahora por el espejo fisico -- que ademas es mas rapido.
    //
    // ** El unico sitio que SIGUE leyendo Ring 3 a proposito es la autopsia, y
    // por eso tiene `stac`/`clac` con nombre propio (`autopsy::con_permiso`).
    // Un solo sitio con permiso explicito es lo que hace que la prohibicion
    // valga: cualquier otro acceso da fault, y el fault dice donde.
    if smap { cr4 |= 1 << 21; }
    asm!("mov cr4, {}", in(reg) cr4);

    // XCR0: x87 + SSE + AVX (Zen 3 has 256-bit AVX)
    if xsav {
        let xcr0 = if avx { 7u32 } else { 3u32 };
        asm!("xsetbv", in("ecx") 0u32, in("eax") xcr0, in("edx") 0u32);
    }
    ser_print!("[s1_cpu] CR0/CR4/XCR0 set for Zen 3\n");
    init_pat();
}

/// **PAT: dejar una entrada en Write-Combining** para el framebuffer.
///
/// === Que arregla ===
///
/// El framebuffer es un BAR de PCIe y los MTRR del firmware lo dejan en **UC**:
/// cada escritura de pixel es una transaccion de bus por su cuenta, sin
/// juntarse con la de al lado. Con Write-Combining el CPU **acumula** escrituras
/// seguidas en un bufer y las suelta de golpe. Para un compositor que pinta
/// ventanas enteras eso no es una micro-optimizacion: es la diferencia entre
/// repintar una caja en milisegundos o en decenas.
///
/// === Por que PAT y no MTRR ===
///
/// Cambiar los MTRR es tocar un reparto global del mapa fisico que el firmware
/// ya dejo montado, y equivocarse ahi afecta a **todo** el sistema. PAT deja
/// elegir el tipo **por pagina**, que es exactamente el grano que hace falta:
/// solo las paginas del framebuffer, y solo para quien las mapee asi.
///
/// Y sobre la combinacion: con el MTRR diciendo UC y el PAT diciendo WC, el
/// tipo efectivo es **WC**. Es el mismo camino que usa `ioremap_wc()` de Linux
/// para framebuffers cuyo MTRR es UC, y por eso no hace falta tocar los MTRR.
///
/// === La secuencia, que NO es opcional ===
///
/// Escribir el MSR de PAT con las caches vivas y los MTRR armados es como
/// cambiarle las ruedas a un coche en marcha. El manual pide un orden exacto y
/// aqui esta entero: apagar cache sin write-back, vaciarla, desarmar MTRR,
/// tirar el TLB, escribir PAT, tirar el TLB otra vez, rearmar MTRR, vaciar y
/// volver a encender. Saltarse un paso no da un error: da una maquina que se
/// cuelga o que corrompe memoria mas tarde.
///
/// === Si falla ===
///
/// Si el CPU no declara PAT, no se toca nada y se dice. El modo de fallo del
/// camino entero es **quedarse como esta** (UC), que es lo de hoy: lento, no
/// roto.
pub unsafe fn init_pat() {
    // Hay PAT? CPUID.01H:EDX[16]. Sin esto no se toca el MSR.
    let (_, _, _, edx1) = cpuid(1, 0);
    if edx1 & (1 << 16) == 0 {
        ser_print!("[s1_cpu] sin PAT: el framebuffer se queda en UC\n");
        return;
    }

    // La tabla que se quiere. Solo se cambia la entrada 4; las cuatro
    // primeras se dejan como el reset las deja, porque son las que usa todo
    // lo demas del sistema y cambiarlas seria cambiarle el tipo de memoria a
    // codigo que no lo pidio.
    //
    //   PA0 = 06 WB    PA1 = 04 WT    PA2 = 07 UC-   PA3 = 00 UC
    //   PA4 = 01 WC <-  PA5 = 04 WT    PA6 = 07 UC-   PA7 = 00 UC
    const PAT_DESEADO: u64 = 0x0007_0401_0007_0406;

    // 1. Cache apagada SIN write-back (CD=1, NW=0) y vaciada.
    let cr0: u64;
    asm!("mov {}, cr0", out(reg) cr0);
    let cr0_sin_cache = (cr0 | (1 << 30)) & !(1 << 29); // CD=1, NW=0
    asm!("mov cr0, {}", in(reg) cr0_sin_cache);
    asm!("wbinvd");

    // 2. Desarmar los MTRR mientras se toca la tabla (MTRRdefType.E = bit 11).
    let deftype = rdmsr(MSR_MTRR_DEF_TYPE);
    wrmsr(MSR_MTRR_DEF_TYPE, deftype & !(1 << 11));

    // 3. Tirar el TLB: CR3 a si mismo.
    let cr3: u64;
    asm!("mov {}, cr3", out(reg) cr3);
    asm!("mov cr3, {}", in(reg) cr3);

    // 4. La tabla.
    wrmsr(MSR_PAT, PAT_DESEADO);

    // 5. Y deshacer el andamio en orden inverso.
    asm!("mov cr3, {}", in(reg) cr3);
    wrmsr(MSR_MTRR_DEF_TYPE, deftype);
    asm!("wbinvd");
    asm!("mov cr0, {}", in(reg) cr0);

    ser_print!("[s1_cpu] PAT: entrada 4 = Write-Combining (framebuffer)\n");
}

pub unsafe fn init_fpu() {
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

pub unsafe fn init_zen3_perf() {
    // Intentionally a no-op during early boot.
    //
    // The previous version RDMSR'd a list of undocumented, model-specific
    // Zen 3 config MSRs (LS_CFG, IC_CFG, DC_CFG, CU_CFG, L2_CFG,
    // PF2_INSTR_CTL) purely to print them. Reading an MSR that the silicon
    // does not implement raises #GP -- which is exactly what reset this
    // machine (QEMU returns 0 for the same reads, so it never surfaced).
    // None of these reads affect booting, so they are gone. If cache/
    // prefetch tuning is ever wanted, it belongs in a Ring 0 driver behind
    // a proper CPUID/allowlist guard, not in the blind boot path.
    ser_print!("[s1_cpu] zen3 perf tuning skipped (safe boot)\n");
}

// ===================================================================
//  AMD SYSCALL (different from Intel: uses STAR[47:32] as CS, STAR[63:48] as SS)
// ===================================================================

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

pub unsafe fn init_syscall() {
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
