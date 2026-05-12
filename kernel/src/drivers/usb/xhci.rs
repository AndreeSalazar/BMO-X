//! xHCI 1.2 — eXtensible Host Controller Interface.
//!
//! El controller xHCI vive en el chipset 500-series del Ryzen 5600X
//! (B550/X570). PCI class 0x0C 0x03 0x30 (Serial Bus / USB / xHCI).
//! Soporta de USB 1.1 hasta USB 3.x sin necesidad de EHCI/UHCI/OHCI.

#![allow(dead_code)]

/// Direcciones MMIO base del xHCI — descubierta vía PCI BAR0.
#[derive(Debug, Clone, Copy)]
pub struct XhciMmio {
    pub base: u64,
    pub size: u32,
}

/// Capability Registers (offset 0x00, read-only).
#[repr(C)]
pub struct CapabilityRegs {
    pub cap_length: u8,         // 0x00
    pub _rsvd: u8,
    pub hci_version: u16,       // 0x02 — esperar ≥ 0x0100
    pub hcs_params1: u32,       // 0x04
    pub hcs_params2: u32,       // 0x08
    pub hcs_params3: u32,       // 0x0C
    pub hcc_params1: u32,       // 0x10
    pub db_off: u32,            // 0x14 — Doorbell Array offset
    pub rts_off: u32,           // 0x18 — Runtime Registers offset
    pub hcc_params2: u32,       // 0x1C
}

/// Operational Registers (offset = cap_length).
#[repr(C)]
pub struct OperationalRegs {
    pub usbcmd: u32,            // 0x00
    pub usbsts: u32,            // 0x04
    pub pagesize: u32,          // 0x08
    pub _rsvd0: [u32; 2],
    pub dnctrl: u32,            // 0x14
    pub crcr_lo: u32,           // 0x18 — Command Ring Control
    pub crcr_hi: u32,           // 0x1C
    pub _rsvd1: [u32; 4],
    pub dcbaap_lo: u32,         // 0x30 — Device Context Base Address Array Ptr
    pub dcbaap_hi: u32,         // 0x34
    pub config: u32,            // 0x38
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct UsbCmd: u32 {
        const RUN_STOP            = 1 << 0;
        const HOST_CONTROLLER_RESET = 1 << 1;
        const INTE                = 1 << 2;
        const HSEE                = 1 << 3;
    }

    #[derive(Debug, Clone, Copy)]
    pub struct UsbSts: u32 {
        const HCH                 = 1 << 0;  // HCHalted
        const HSE                 = 1 << 2;  // Host System Error
        const EINT                = 1 << 3;  // Event Interrupt
        const PCD                 = 1 << 4;  // Port Change Detect
        const SSS                 = 1 << 8;  // Save State Status
        const RSS                 = 1 << 9;  // Restore State Status
        const SRE                 = 1 << 10; // Save/Restore Error
        const CNR                 = 1 << 11; // Controller Not Ready
    }
}

/// Tipos de TRB (Transfer Request Block) — el lenguaje del xHCI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TrbType {
    Reserved        = 0,
    Normal          = 1,
    SetupStage      = 2,
    DataStage       = 3,
    StatusStage     = 4,
    Isoch           = 5,
    Link            = 6,
    EventData       = 7,
    NoOp            = 8,
    EnableSlotCmd   = 9,
    DisableSlotCmd  = 10,
    AddressDeviceCmd = 11,
    ConfigureEpCmd  = 12,
    EvaluateCtxCmd  = 13,
    ResetEpCmd      = 14,
    StopEndpointCmd = 15,
    NoOpCmd         = 23,
    TransferEvent   = 32,
    CommandCompletion = 33,
    PortStatusChange  = 34,
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct Trb {
    pub parameter: u64,
    pub status: u32,
    pub control: u32,
}

pub struct XhciController {
    pub mmio: XhciMmio,
    pub max_slots: u8,
    pub max_ports: u8,
    pub initialized: bool,
}

impl XhciController {
    pub fn probe(_pci_bdf: u32) -> Result<Self, &'static str> {
        // TODO: leer BAR0, mapear MMIO, detectar capability/operational/runtime,
        //       hacer reset (USBCMD.HCRST), inicializar device context array,
        //       command ring, event ring, habilitar IE.
        Err("xhci::probe no implementado todavía")
    }

    pub fn enumerate_ports(&mut self) -> Result<u8, &'static str> {
        // TODO: por cada puerto con CCS=1, asignar slot, address device, leer descriptors,
        //       notificar a hid::attach o audio_class::attach según class.
        Err("xhci::enumerate_ports no implementado todavía")
    }
}
