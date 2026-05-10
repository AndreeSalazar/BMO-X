//! Auto-generated MMIO Register Map
use super::core_types::*;

// Registers for SEC2_FALCON
pub const SEC2_FALCON_CPUCTL: RegisterDescriptor = RegisterDescriptor {
    offset: 0x840100,
    name: "SEC2_FALCON_CPUCTL",
    source: "Extracted from Windows 11 DriverStore / nvlddmkm.sys",
    confidence: ConfidenceLevel::Confirmed,
};
pub const SEC2_FALCON_BOOTVEC: RegisterDescriptor = RegisterDescriptor {
    offset: 0x840104,
    name: "SEC2_FALCON_BOOTVEC",
    source: "Extracted from Windows 11 DriverStore / nvlddmkm.sys",
    confidence: ConfidenceLevel::Confirmed,
};
pub const SEC2_FALCON_IRQSTAT: RegisterDescriptor = RegisterDescriptor {
    offset: 0x840008,
    name: "SEC2_FALCON_IRQSTAT",
    source: "Extracted from Windows 11 DriverStore / nvlddmkm.sys",
    confidence: ConfidenceLevel::Confirmed,
};
pub const SEC2_FALCON_ENGINE: RegisterDescriptor = RegisterDescriptor {
    offset: 0x8403C0,
    name: "SEC2_FALCON_ENGINE",
    source: "Extracted from Windows 11 DriverStore / nvlddmkm.sys",
    confidence: ConfidenceLevel::Confirmed,
};
pub const SEC2_FALCON_IMEMC: RegisterDescriptor = RegisterDescriptor {
    offset: 0x840180,
    name: "SEC2_FALCON_IMEMC",
    source: "Extracted from Windows 11 DriverStore / nvlddmkm.sys",
    confidence: ConfidenceLevel::Confirmed,
};
pub const SEC2_FALCON_DMEMC: RegisterDescriptor = RegisterDescriptor {
    offset: 0x8401C0,
    name: "SEC2_FALCON_DMEMC",
    source: "Extracted from Windows 11 DriverStore / nvlddmkm.sys",
    confidence: ConfidenceLevel::Confirmed,
};
pub const SEC2_FALCON_IMEMD: RegisterDescriptor = RegisterDescriptor {
    offset: 0x840184,
    name: "SEC2_FALCON_IMEMD",
    source: "Inferred from IMEMC + 0x4 Falcon pattern",
    confidence: ConfidenceLevel::Experimental,
};
pub const SEC2_FALCON_DMEMD: RegisterDescriptor = RegisterDescriptor {
    offset: 0x8401C4,
    name: "SEC2_FALCON_DMEMD",
    source: "Inferred from DMEMC + 0x4 Falcon pattern",
    confidence: ConfidenceLevel::Experimental,
};

// Registers for PMC
pub const PMC_ENABLE: RegisterDescriptor = RegisterDescriptor {
    offset: 0x000200,
    name: "PMC_ENABLE",
    source: "Inferred PMC engine enable register",
    confidence: ConfidenceLevel::Experimental,
};
