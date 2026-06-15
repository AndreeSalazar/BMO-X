//! xHCI 1.2 — eXtensible Host Controller Interface.
//!
//! El controller xHCI vive en el chipset 500-series del Ryzen 5600X
//! (B550/X570). PCI class 0x0C 0x03 0x30 (Serial Bus / USB / xHCI).
//! Soporta de USB 1.1 hasta USB 3.x sin necesidad de EHCI/UHCI/OHCI.

#![allow(dead_code)]

use crate::drivers::pci::{self, PciDevice};
use crate::drivers::serial;
use core::ptr::{read_volatile, write_volatile};
use bmo_usb::{CapRegisters, OpRegisters};

const USB_CLASS: u8 = 0x0C;
const USB_SUBCLASS: u8 = 0x03;
const USB_INTERFACE_XHCI: u8 = 0x30;

/// Direcciones MMIO base del xHCI — descubierta vía PCI BAR0.
#[derive(Debug, Clone, Copy)]
pub struct XhciMmio {
    pub base: u64,
    pub size: u32,
}

pub struct XhciController {
    pub pci_device: PciDevice,
    pub base_addr: u64,
    pub cap_regs: *mut CapRegisters,
    pub op_regs: *mut OpRegisters,
    pub max_slots: u8,
    pub max_ports: u8,
    pub initialized: bool,
}

impl XhciController {
    pub fn detect() -> Option<Self> {
        serial::serial_write("[xHCI] Escaneando bus PCI para controladores USB...\n");
        let pci_devs = pci::scan_pci_bus();
        for i in 0..pci_devs.count {
            let dev = pci_devs.devices[i];
            
            // Leer class, subclass e interface
            let class_rev = pci::pci_read32(dev.bus, dev.device, dev.function, 0x08);
            let class = (class_rev >> 24) as u8;
            let subclass = (class_rev >> 16) as u8;
            let interface = ((class_rev >> 8) & 0xFF) as u8;

            if class == USB_CLASS && subclass == USB_SUBCLASS && interface == USB_INTERFACE_XHCI {
                serial::serial_write("[xHCI] ¡Controlador xHCI detectado en PCI!\n");
                match Self::probe(dev) {
                    Ok(controller) => return Some(controller),
                    Err(e) => {
                        serial::serial_write("[xHCI] ERROR en probe: ");
                        serial::serial_write(e);
                        serial::serial_write("\n");
                    }
                }
            }
        }
        None
    }

    pub fn probe(dev: PciDevice) -> Result<Self, &'static str> {
        // Habilitar Bus Mastering y Memory Space en PCI command register
        let cmd_reg = pci::pci_read32(dev.bus, dev.device, dev.function, 0x04);
        pci::pci_write32(
            dev.bus, dev.device, dev.function, 0x04,
            (cmd_reg & 0xFFFF) | 0x06 // Bus Master (0x4) + Memory Space (0x2)
        );

        // Obtener la dirección base desde BAR0/BAR1 (soporta BARs de 64 bits)
        let bar0 = dev.bar0;
        let is_64bit = (bar0 & 0x6) == 0x4;
        let mut base_addr = (bar0 & !0xF) as u64;
        if is_64bit {
            let bar1 = dev.bar1;
            base_addr |= (bar1 as u64) << 32;
        }

        if base_addr == 0 {
            return Err("BAR0 de xHCI es cero");
        }

        serial::serial_write("[xHCI] MMIO BAR base: ");
        crate::serial_hex(base_addr);
        serial::serial_write("\n");

        let cap_regs = base_addr as *mut CapRegisters;
        
        // El offset de los Operational Registers está dado por cap_length
        let cap_len = unsafe { read_volatile(&(*cap_regs).cap_length) };
        if cap_len == 0 || cap_len > 0x80 {
            return Err("Cap Length de xHCI no válida");
        }
        
        let op_regs = (base_addr + cap_len as u64) as *mut OpRegisters;

        // Leer versión de xHCI
        let hci_version = unsafe { read_volatile(&(*cap_regs).hci_version) };
        serial::serial_write("[xHCI] HCI Versión: ");
        crate::serial_hex(hci_version as u64);
        serial::serial_write("\n");

        // Leer parámetros estructurales
        let hcs1 = unsafe { read_volatile(&(*cap_regs).hcs_params1) };
        let max_slots = (hcs1 & 0xFF) as u8;
        let max_ports = ((hcs1 >> 24) & 0xFF) as u8;

        serial::serial_write("[xHCI] Max Slots: ");
        crate::serial_hex(max_slots as u64);
        serial::serial_write(" | Max Ports: ");
        crate::serial_hex(max_ports as u64);
        serial::serial_write("\n");

        // 1. Resetear el controlador
        serial::serial_write("[xHCI] Reseteando host controller...\n");
        unsafe {
            // Asegurarse de que esté detenido antes de resetear
            let mut usb_cmd = read_volatile(&(*op_regs).usb_cmd);
            usb_cmd &= !1; // RUN/STOP = 0 (Stop)
            write_volatile(&mut (*op_regs).usb_cmd, usb_cmd);

            // Esperar a que el controlador se detenga (USBSTS.HCH = 1)
            let mut timeout = 100_000;
            while (read_volatile(&(*op_regs).usb_sts) & 1) == 0 {
                timeout -= 1;
                if timeout == 0 {
                    return Err("El controlador no se detuvo");
                }
                core::hint::spin_loop();
            }

            // Iniciar reset de hardware
            usb_cmd = read_volatile(&(*op_regs).usb_cmd);
            usb_cmd |= 2; // HCRST = 1
            write_volatile(&mut (*op_regs).usb_cmd, usb_cmd);

            // Esperar a que HCRST se limpie a 0 y CNR (Controller Not Ready) sea 0
            timeout = 500_000;
            loop {
                let usb_cmd_val = read_volatile(&(*op_regs).usb_cmd);
                let usb_sts_val = read_volatile(&(*op_regs).usb_sts);
                let hcrst = (usb_cmd_val & 2) != 0;
                let cnr = (usb_sts_val & (1 << 11)) != 0;
                if !hcrst && !cnr {
                    break;
                }
                timeout -= 1;
                if timeout == 0 {
                    return Err("Timeout esperando reset del xHCI");
                }
                core::hint::spin_loop();
            }
        }

        serial::serial_write("[xHCI] Reset exitoso.\n");

        Ok(Self {
            pci_device: dev,
            base_addr,
            cap_regs,
            op_regs,
            max_slots,
            max_ports,
            initialized: true,
        })
    }

    pub fn enumerate_ports(&mut self) -> Result<u8, &'static str> {
        let mut connected_count = 0;
        for port in 1..=self.max_ports {
            let port_reg_addr = (self.base_addr + 0x400 + ((port as u64 - 1) * 0x10)) as *mut u32;
            let portsc = unsafe { read_volatile(port_reg_addr) };
            let ccs = (portsc & 1) != 0; // Current Connect Status
            let ped = (portsc & 2) != 0; // Port Enabled/Disabled

            if ccs {
                connected_count += 1;
                serial::serial_write("[xHCI] Puerto ");
                crate::serial_hex(port as u64);
                serial::serial_write(" conectado: PED=");
                serial::serial_write(if ped { "1\n" } else { "0\n" });
            }
        }
        Ok(connected_count)
    }
}
