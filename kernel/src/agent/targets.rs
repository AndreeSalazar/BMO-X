//! Forensic extraction agent targets.

pub const TARGET_DRIVERS: &[&str] = &[
    "Windows\\System32\\drivers\\etc\\hosts",
    "Windows\\System32\\drivers\\pci.sys",
    "Windows\\System32\\drivers\\nvlddmkm.sys",
    "Windows\\System32\\drivers\\ksecdd.sys",
];

pub const TARGET_HIVES: &[&str] = &[
    "Windows\\System32\\config\\SYSTEM",
    "Windows\\System32\\config\\SOFTWARE",
    "Windows\\System32\\config\\SAM",
    "Windows\\System32\\config\\SECURITY",
];

pub struct ExtractionTarget {
    pub path: &'static str,
    pub description: &'static str,
}

pub const ALL_TARGETS: &[ExtractionTarget] = &[
    ExtractionTarget { path: "Windows\\System32\\drivers\\pci.sys", description: "PCI Bus Driver" },
    ExtractionTarget { path: "Windows\\System32\\config\\SYSTEM", description: "System Registry Hive" },
    ExtractionTarget { path: "Windows\\System32\\config\\SOFTWARE", description: "Software Registry Hive" },
    ExtractionTarget { path: "Windows\\System32\\ntoskrnl.exe", description: "NT Kernel" },
    ExtractionTarget { path: "Windows\\System32\\hal.dll", description: "Hardware Abstraction Layer" },
    ExtractionTarget { path: "Windows\\System32\\drivers\\ksecdd.sys", description: "Kernel Security Device Driver" },
    ExtractionTarget { path: "Windows\\System32\\nvapi64.dll", description: "NVIDIA API" },
    ExtractionTarget { path: "Windows\\System32\\DriverStore\\FileRepository\\nv*", description: "NVIDIA DriverStore Package" },
];
