//! **DETECTION** -- `CPUID` in, a profile out.
//!
//! === The rule this file follows ===
//!
//! It asks about FEATURES, not about brands. What matters at boot is whether
//! this CPU has `XSAVE`, how many leaves it answers, what its cache geometry
//! is -- not whose logo is on it. A check written as *"is it a 5600X"* is a
//! check that fails on the next machine for no good reason.
//!
//! The brand string is read and reported because a human wants to see it, and
//! **that is the only thing it is used for**: nothing branches on it.
//!
//! [!] Which is why the vendor-specific work lives in `zen3.rs` and is selected
//! from what was detected here, rather than the other way round.

#[allow(unused_imports)]
use crate::*;

// ===================================================================
//  AMD ZEN 3 CPU DETECTION (Ryzen 5 5600X)
// ===================================================================

pub unsafe fn detect_cpu() {
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
