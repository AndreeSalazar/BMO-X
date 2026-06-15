//! Auto-generated Intelligence Graph
use super::core_types::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleCategory {
    Driver,
    Firmware,
    HardwareEngine,
    KernelDriver,
    NtKernel,
    UserModeDll,
    SystemProcess,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub struct ModuleMetadata {
    pub name: &'static str,
    pub category: ModuleCategory,
    pub description: &'static str,
}

pub const MODULES: &[ModuleMetadata] = &[
    ModuleMetadata {
        name: "nvlddmkm.sys",
        category: ModuleCategory::Driver,
        description: "NVIDIA Windows Kernel Mode Driver (WDDM miniport)",
    },
    ModuleMetadata {
        name: "dxgkrnl.sys",
        category: ModuleCategory::KernelDriver,
        description: "DirectX Graphics Kernel (WDDM framework)",
    },
    ModuleMetadata {
        name: "dxgmms2.sys",
        category: ModuleCategory::KernelDriver,
        description: "DirectX Graphics Memory Management Subsystem v2",
    },
    ModuleMetadata {
        name: "dxgmms1.sys",
        category: ModuleCategory::KernelDriver,
        description: "DirectX Graphics Memory Management Subsystem v1 (legacy)",
    },
    ModuleMetadata {
        name: "ntoskrnl.exe",
        category: ModuleCategory::NtKernel,
        description: "Windows NT Kernel Executive",
    },
    ModuleMetadata {
        name: "hal.dll",
        category: ModuleCategory::NtKernel,
        description: "Hardware Abstraction Layer",
    },
    ModuleMetadata {
        name: "gsp_ga10x.bin",
        category: ModuleCategory::Firmware,
        description: "GPU System Processor firmware for Ampere GA10x",
    },
    ModuleMetadata {
        name: "gsp_tu10x.bin",
        category: ModuleCategory::Firmware,
        description: "GPU System Processor firmware for Turing TU10x",
    },
    ModuleMetadata {
        name: "sec2_falcon",
        category: ModuleCategory::HardwareEngine,
        description: "Secure Engine 2 - Falcon-based authenticated boot processor",
    },
    ModuleMetadata {
        name: "pci.sys",
        category: ModuleCategory::KernelDriver,
        description: "PCI/PCIe Bus Driver",
    },
    ModuleMetadata {
        name: "d3d11.dll",
        category: ModuleCategory::UserModeDll,
        description: "Direct3D 11 Runtime",
    },
    ModuleMetadata {
        name: "D3D12.dll / D3D12Core.dll",
        category: ModuleCategory::UserModeDll,
        description: "Direct3D 12 Runtime",
    },
    ModuleMetadata {
        name: "dwm.exe / dwmcore.dll",
        category: ModuleCategory::SystemProcess,
        description: "Desktop Window Manager - WDDM composition engine",
    },
];

// Engine Descriptors
pub const ENGINE_SEC2: EngineDescriptor = EngineDescriptor {
    name: "SEC2",
    mmio_base: 0x840000,
    requires_falcon: true,
    requires_authenticated_boot: true,
    confidence: ConfidenceLevel::Confirmed,
};
pub const ENGINE_GSP: EngineDescriptor = EngineDescriptor {
    name: "GSP",
    mmio_base: 0x110000,
    requires_falcon: true,
    requires_authenticated_boot: false,
    confidence: ConfidenceLevel::Confirmed,
};