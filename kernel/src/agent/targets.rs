//! Forensic extraction agent targets — Spy Edition.
//! Defines all files to scan for digital signatures, certificates, and crypto keys.

/// Category of extraction target.
#[derive(Clone, Copy, PartialEq)]
pub enum TargetCategory {
    /// Extract Authenticode signature from PE binary
    PeSignature,
    /// Extract certificates from registry hive
    CertStore,
    /// Extract .cat catalog files
    DriverCatalog,
    /// Extract crypto keys from registry
    RegistryCrypto,
    /// Full registry hive dump
    RegistryHive,
    /// NVIDIA DriverStore firmware files
    NvidiaFirmware,
}

pub struct ExtractionTarget {
    pub path: &'static str,
    pub description: &'static str,
    pub category: TargetCategory,
}

pub const ALL_TARGETS: &[ExtractionTarget] = &[
    // ── PE Binaries with Authenticode Signatures ──
    ExtractionTarget {
        path: "Windows\\System32\\ntoskrnl.exe",
        description: "NT Kernel",
        category: TargetCategory::PeSignature,
    },
    ExtractionTarget {
        path: "Windows\\System32\\hal.dll",
        description: "Hardware Abstraction Layer",
        category: TargetCategory::PeSignature,
    },
    ExtractionTarget {
        path: "Windows\\System32\\ci.dll",
        description: "Code Integrity Module",
        category: TargetCategory::PeSignature,
    },
    ExtractionTarget {
        path: "Windows\\System32\\drivers\\ksecdd.sys",
        description: "Kernel Security Device Driver",
        category: TargetCategory::PeSignature,
    },
    ExtractionTarget {
        path: "Windows\\System32\\drivers\\nvlddmkm.sys",
        description: "NVIDIA Display Driver",
        category: TargetCategory::PeSignature,
    },
    ExtractionTarget {
        path: "Windows\\System32\\nvapi64.dll",
        description: "NVIDIA API Library",
        category: TargetCategory::PeSignature,
    },
    ExtractionTarget {
        path: "Windows\\System32\\bcryptprimitives.dll",
        description: "BCrypt Primitives",
        category: TargetCategory::PeSignature,
    },
    ExtractionTarget {
        path: "Windows\\System32\\drivers\\pci.sys",
        description: "PCI Bus Driver",
        category: TargetCategory::PeSignature,
    },

    // ── Registry Hives ──
    ExtractionTarget {
        path: "Windows\\System32\\config\\SYSTEM",
        description: "System Registry Hive",
        category: TargetCategory::RegistryHive,
    },
    ExtractionTarget {
        path: "Windows\\System32\\config\\SOFTWARE",
        description: "Software Registry Hive",
        category: TargetCategory::RegistryHive,
    },
    ExtractionTarget {
        path: "Windows\\System32\\config\\SAM",
        description: "Security Accounts Manager",
        category: TargetCategory::RegistryHive,
    },
    ExtractionTarget {
        path: "Windows\\System32\\config\\SECURITY",
        description: "Security Registry Hive",
        category: TargetCategory::RegistryHive,
    },

    // ── Driver Catalogs (wildcard — matched via starts_with) ──
    ExtractionTarget {
        path: "Windows\\System32\\CatRoot\\{F750E6C3-38EE-11D1-85E5-00C04FC295EE}\\*",
        description: "Third-Party Driver Catalogs",
        category: TargetCategory::DriverCatalog,
    },
    
    // ── NVIDIA Firmware & DriverStore (wildcard) ──
    ExtractionTarget {
        path: "Windows\\System32\\DriverStore\\FileRepository\\nv*",
        description: "NVIDIA Driver Repository",
        category: TargetCategory::NvidiaFirmware,
    },
    ExtractionTarget {
        path: "Windows\\System32\\CatRoot\\*",
        description: "Driver Catalog Store L1",
        category: TargetCategory::DriverCatalog,
    },
    ExtractionTarget {
        path: "Windows\\System32\\CatRoot2\\*",
        description: "Driver Catalog Store L2",
        category: TargetCategory::DriverCatalog,
    },

    // ── DriverStore NVIDIA packages ──
    ExtractionTarget {
        path: "Windows\\System32\\DriverStore\\FileRepository\\nv*",
        description: "NVIDIA DriverStore Package",
        category: TargetCategory::PeSignature,
    },
];
