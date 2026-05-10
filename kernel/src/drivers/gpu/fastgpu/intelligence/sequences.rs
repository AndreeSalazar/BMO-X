//! Auto-generated Boot Sequences
use super::core_types::*;

pub const GPU_BOOT_SEQUENCE_STEPS: &[SequenceStep] = &[
    SequenceStep {
        step_num: 1,
        actor: "pci.sys",
        action: "Enumerate PCIe device VEN_10DE&DEV_2560",
    },
    SequenceStep {
        step_num: 2,
        actor: "dxgkrnl.sys",
        action: "DxgkDdiAddDevice callback",
    },
    SequenceStep {
        step_num: 3,
        actor: "dxgkrnl.sys",
        action: "DxgkDdiStartDevice callback",
    },
    SequenceStep {
        step_num: 4,
        actor: "nvlddmkm.sys",
        action: "Map BAR0 MMIO registers",
    },
    SequenceStep {
        step_num: 5,
        actor: "nvlddmkm.sys",
        action: "Initialize PMU engine",
    },
    SequenceStep {
        step_num: 6,
        actor: "nvlddmkm.sys",
        action: "Load SEC2 microcode (IMEM/DMEM)",
    },
    SequenceStep {
        step_num: 7,
        actor: "sec2_falcon",
        action: "Enter HS (High Secure) mode",
    },
    SequenceStep {
        step_num: 8,
        actor: "nvlddmkm.sys",
        action: "DMA transfer GSP firmware to VRAM",
    },
    SequenceStep {
        step_num: 9,
        actor: "sec2_falcon",
        action: "Authenticate GSP firmware (PKC/RSA-2048)",
    },
    SequenceStep {
        step_num: 10,
        actor: "gsp_ga10x.bin",
        action: "Start execution in WPR2 region",
    },
    SequenceStep {
        step_num: 11,
        actor: "gsp_ga10x.bin",
        action: "GSP_INIT_DONE RPC (0x1001) handshake",
    },
    SequenceStep {
        step_num: 12,
        actor: "nvlddmkm.sys",
        action: "Complete DxgkDdiStartDevice",
    },
    SequenceStep {
        step_num: 13,
        actor: "dxgkrnl.sys",
        action: "DxgkDdiQueryAdapterInfo",
    },
    SequenceStep {
        step_num: 14,
        actor: "dxgmms2.sys",
        action: "Initialize memory segments",
    },
    SequenceStep {
        step_num: 15,
        actor: "dxgkrnl.sys",
        action: "Activate GPU scheduler",
    },
];

pub const SEC2_BRINGUP_STEPS: &[SequenceStep] = &[
    SequenceStep {
        step_num: 1,
        actor: "nvlddmkm.sys",
        action: "Clock ungating (PMC engine enable SEC2)",
    },
    SequenceStep {
        step_num: 2,
        actor: "nvlddmkm.sys",
        action: "SEC2 reset release (NV_PSEC2_FALCON_ENGINE)",
    },
    SequenceStep {
        step_num: 3,
        actor: "nvlddmkm.sys",
        action: "IMEM load - authenticated boot microcode",
    },
    SequenceStep {
        step_num: 4,
        actor: "nvlddmkm.sys",
        action: "DMEM load - configuration data + PKC sig",
    },
    SequenceStep {
        step_num: 5,
        actor: "nvlddmkm.sys",
        action: "Set BOOTVEC = 0x0",
    },
    SequenceStep {
        step_num: 6,
        actor: "nvlddmkm.sys",
        action: "CPUCTL start CPU bit",
    },
    SequenceStep {
        step_num: 7,
        actor: "sec2_falcon",
        action: "HS mode entry (CPUCTL HALTED=0, check HS bit)",
    },
    SequenceStep {
        step_num: 8,
        actor: "sec2_falcon",
        action: "FRTS command (WPR2 region setup)",
    },
    SequenceStep {
        step_num: 9,
        actor: "sec2_falcon",
        action: "GSP DMA transfer command",
    },
    SequenceStep {
        step_num: 10,
        actor: "sec2_falcon",
        action: "PKC signature verification (RSA-2048)",
    },
    SequenceStep {
        step_num: 11,
        actor: "sec2_falcon",
        action: "GSP boot trigger",
    },
];

pub const WDDM_INIT_SEQUENCE_STEPS: &[SequenceStep] = &[
    SequenceStep {
        step_num: 1,
        actor: "nvlddmkm.sys",
        action: "DriverEntry -> populate DRIVER_INITIALIZATION_DATA",
    },
    SequenceStep {
        step_num: 2,
        actor: "dxgkrnl.sys",
        action: "DxgkDdiAddDevice (adapter context creation)",
    },
    SequenceStep {
        step_num: 3,
        actor: "dxgkrnl.sys",
        action: "DxgkDdiStartDevice (HW resource assignment)",
    },
    SequenceStep {
        step_num: 4,
        actor: "dxgkrnl.sys",
        action: "DxgkDdiQueryAdapterInfo (capability report)",
    },
    SequenceStep {
        step_num: 5,
        actor: "dxgkrnl.sys",
        action: "DxgkDdiQueryChildRelations (display outputs)",
    },
    SequenceStep {
        step_num: 6,
        actor: "dxgmms2.sys",
        action: "Memory segment initialization (VRAM/sysmem)",
    },
    SequenceStep {
        step_num: 7,
        actor: "dxgkrnl.sys",
        action: "GPU scheduler activation",
    },
    SequenceStep {
        step_num: 8,
        actor: "dxgkrnl.sys",
        action: "Display output configuration & mode set",
    },
];

pub const ALL_SEQUENCES: &[BootSequence] = &[
    BootSequence {
        name: "GPU Full Boot Sequence",
        target: "GA106 (RTX 3060)",
        steps: GPU_BOOT_SEQUENCE_STEPS,
    },
    BootSequence {
        name: "SEC2 Falcon Bring-Up Sequence",
        target: "GA106 SEC2 Engine",
        steps: SEC2_BRINGUP_STEPS,
    },
    BootSequence {
        name: "WDDM Driver Initialization Sequence",
        target: "Windows Display Driver Model",
        steps: WDDM_INIT_SEQUENCE_STEPS,
    },
];