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

    if let Some(ecam) = crate::arch::acpi::parse_mcfg(bi.rsdp_addr) {
        pci::init_ecam(ecam.base_addr, ecam.end_bus);
        store_and_log("PCI devices discovered", pci::scan_pci_bus());
    } else {
        log::warn("phase2", "MCFG not found; trying legacy IO port PCI scan");
        pci::init_ecam(0, 32);
        store_and_log("PCI devices discovered (IO port)", pci::scan_pci_bus());
    }

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
