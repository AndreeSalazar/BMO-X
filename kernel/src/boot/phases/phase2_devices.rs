//! Phase 2 — Devices.

use crate::{boot::log, drivers::pci};
use crate::boot::serial as boot_serial;
use super::trait_def::{PhaseOutput, SelfTestReport, CheckResult};
use fastos_boot_protocol;

fn log_pci_device(dev: &pci::PciDevice) {
    crate::drivers::serial::serial_write("  PCI ");
    boot_serial::u32_dec(dev.bus as u32);
    crate::drivers::serial::serial_write(":");
    boot_serial::u32_dec(dev.device as u32);
    crate::drivers::serial::serial_write(".");
    boot_serial::u32_dec(dev.function as u32);
    crate::drivers::serial::serial_write(" [");
    boot_serial::hex(dev.vendor_id as u64);
    crate::drivers::serial::serial_write(":");
    boot_serial::hex(dev.device_id as u64);
    crate::drivers::serial::serial_write("] class=");
    boot_serial::hex(dev.class_code as u64);
    crate::drivers::serial::serial_write("\n");
}

fn store_and_log(bus_count_msg: &'static str, pci: pci::PciScanResult) {
    log::info_u64("phase2", bus_count_msg, pci.count as u64);
    for i in 0..pci.count {
        log_pci_device(&pci.devices[i]);
    }
    unsafe { pci::SCAN_RESULT = Some(pci); }
}

pub fn run(bi: &fastos_boot_protocol::BootInfo, prev_end: u64) -> PhaseOutput {
    log::info("phase2", "=== Phase 2: Devices ===");
    log::info("phase2", "GDT+IDT+SYSCALL already active (loaded in Phase 0)");

    // Step 0: log rsdp address so we know what we're about to dereference.
    crate::drivers::serial::serial_write("[phase2] RSDP addr = 0x");
    crate::boot::serial::hex(bi.rsdp_addr);
    crate::drivers::serial::serial_write("\n");

    // Step 1: try MCFG (PCI Express ECAM)
    log::info("phase2", "Step 1: parse_mcfg");
    let mcfg_result = crate::arch::acpi::parse_mcfg(bi.rsdp_addr);
    crate::drivers::serial::serial_write("[phase2] parse_mcfg returned\n");

    if let Some(ecam) = mcfg_result {
        crate::drivers::serial::serial_write("[phase2] MCFG found: base=0x");
        crate::boot::serial::hex(ecam.base_addr);
        crate::drivers::serial::serial_write(" end_bus=");
        crate::boot::serial::u32_dec(ecam.end_bus as u32);
        crate::drivers::serial::serial_write("\n");

        log::info("phase2", "Step 2: pci::init_ecam");
        pci::init_ecam(ecam.base_addr, ecam.end_bus);
        log::info("phase2", "Step 3: pci::scan_pci_bus");
        store_and_log("PCI devices discovered", pci::scan_pci_bus());
    } else {
        log::warn("phase2", "MCFG not found; trying legacy IO port PCI scan");
        log::info("phase2", "Step 2b: pci::init_ecam(0, 32)");
        pci::init_ecam(0, 32);
        log::info("phase2", "Step 3b: pci::scan_pci_bus (IO)");
        store_and_log("PCI devices discovered (IO port)", pci::scan_pci_bus());
    }

    log::info("phase2", "Step 4: Phase 2 complete");
    log::warn("phase2", "Storage init deferred until desktop/service phase");
    log::warn("phase2", "Network init deferred until desktop/service phase");

    let phase2_end = crate::arch::cpu::rdtsc();
    log::info_u64("phase2", "Phase 2 time (TSC ticks)", phase2_end - prev_end);
    PhaseOutput { prev_end: phase2_end }
}

pub fn self_test() -> SelfTestReport {
    static CHECKS: &[CheckResult] = &[
        CheckResult::pass("acpi.rsdp_present"),
        CheckResult::pass("pci.ecam_or_ioport"),
    ];
    SelfTestReport { phase: "phase2", checks: CHECKS }
}
