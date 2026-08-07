//! CPU topology for the Ryzen 5 5600X (1 CCD, 1 CCX, 6C/12T).
//!
//! Recovers the legacy `topology.rs` from the deleted
//! `crates_Personal/ring0/cpu_vendor_profile/.../topology.rs`,
//! adapted for in-kernel use.
//!
//! Uses CPUID leaves 0x0B (extended topology) and 0x8000001E
//! (extended APIC ID) to determine SMT, cores, CCX, CCD, APIC IDs.

use super::cpuid::cpuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuId {
    pub apic_id: u8,
    pub thread: u8,    // 0 or 1 on Zen 3 (SMT)
    pub core: u8,      // 0..=5 on the 5600X
    pub ccd: u8,       // 0 on the 5600X (single CCD)
    pub ccx: u8,       // 0 on the 5600X (single CCX)
}

impl CpuId {
    /// Linear index 0..=11 (BSP=0). Useful for per-CPU tables.
    pub fn linear(&self) -> u8 { self.ccd * 12 + self.core * 2 + self.thread }
}

#[derive(Debug, Clone, Copy)]
pub struct Topology {
    pub bsp: CpuId,
    pub cpus: [CpuId; 64],
    pub cpu_count: u32,
    pub total_threads: u32,
    pub total_cores: u32,
    pub total_ccxs: u32,
    pub total_ccds: u32,
}

impl Topology {
    /// ⚠️ **ESTE ARRAY NO ESTÁ ENUMERADO. Son `cpu_count` copias del BSP.**
    ///
    /// Se rellena con `[bsp; 64]` y nunca se toca después, así que `cpus()`
    /// devuelve doce veces el mismo núcleo con el mismo `apic_id`. Tiene forma
    /// de censo de CPUs y no lo es — la clase de campo que engaña a quien lo
    /// lee, porque *parece* un dato y es un relleno.
    ///
    /// **Dónde está el censo de verdad**: en la tabla **MADT** de ACPI, en sus
    /// entradas de tipo 0 (*Processor Local APIC*), que traen el APIC ID de cada
    /// hilo. `s2_mem` ya localiza la MADT —`find_table(xsdt, b"APIC")`— pero
    /// **sólo le lee el campo de la dirección base del LAPIC** (offset 36) y no
    /// recorre sus entradas. Enumerarlas es el trabajo pendiente.
    ///
    /// Mientras tanto, `plat::smp` despierta suponiendo APIC IDs `0..hilos-1`,
    /// que es lo correcto en un Zen 3 de un solo CCD y **una suposición** en
    /// cualquier otra cosa. Dicho en `smp/mod.rs`.
    #[deprecated(note = "no enumerado: son copias del BSP. El censo real esta en la MADT")]
    pub fn cpus(&self) -> &[CpuId] { &self.cpus[..self.cpu_count as usize] }

    /// Buscar aquí sólo puede encontrar al BSP. Ver [`Topology::cpus`].
    #[deprecated(note = "el array no esta enumerado; esto solo puede encontrar al BSP")]
    pub fn find_by_apic(&self, apic: u32) -> Option<&CpuId> {
        #[allow(deprecated)]
        self.cpus().iter().find(|c| c.apic_id as u32 == apic)
    }
}

/// Detect the full topology.
///
/// This implementation populates the BSP only (the CPU we're
/// currently running on). Detecting all APs requires SMP bring-up
/// (INIT-SIPI-SIPI), which is out of scope for the minimal Ring 0
/// base. The full topology will be filled in by the SMP bring-up
/// code (future work) via `Topology::add_cpu()`.
pub fn detect_bsp() -> Topology {
    // SMT level (sub-leaf 0): ECX[15:8] = threads at this level
    // (2 on Zen 3). EAX = extended APIC ID for this level.
    let (smt_eax, _, smt_ecx, _) = cpuid(0x0B, 0);
    let _smt_count = ((smt_ecx >> 8) & 0xFF) as u8;
    let apic_id = smt_eax as u8;

    // Core level (sub-leaf 1): ECX[15:8] = threads at this level
    // (cores * SMT = 12 on the 5600X). EAX = APIC ID for this level.
    let (core_eax, _, core_ecx, _) = cpuid(0x0B, 1);
    let _core_count = ((core_ecx >> 8) & 0xFF) as u8;

    // 5600X = 1 CCD, 1 CCX, 6 cores, 2 threads/core = 12 logical
    let thread = apic_id & 1;
    let core = (apic_id >> 1) & 0x07;

    let bsp = CpuId { apic_id, thread, core, ccd: 0, ccx: 0 };
    let mut cpus = [bsp; 64];
    cpus[0] = bsp;
    let total_threads = core_count_u32() as u32;
    let total_cores = total_threads / 2;

    Topology {
        bsp,
        cpus,
        cpu_count: total_threads.min(64),
        total_threads,
        total_cores,
        total_ccxs: 1,
        total_ccds: 1,
    }
}

/// Helper: read CPUID.1:EBX[23:16] for the logical core count
/// (per the legacy `cpuid_detection.rs`).
fn core_count_u32() -> usize {
    let (_, ebx, _, _) = cpuid(1, 0);
    ((ebx >> 16) & 0xFF) as usize
}
