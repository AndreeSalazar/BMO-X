//! PCI config-space: acceso directo por puertos 0xCF8/0xCFC + scan completo.
//!
//! Motivo: el `pci_devices` del BootContext (scan de s2) no ve los
//! controladores que cuelgan detrás de bridges — y en los Ryzen los xHC USB
//! viven exactamente ahí (buses > 0). Este módulo escanea TODO el espacio
//! bus/dev/función por fuerza bruta (barato: unos miles de lecturas de
//! config una sola vez) y encuentra el dispositivo por su clase real,
//! incluyendo el prog-if que el BootContext no captura.
//!
//! También habilita Memory Space + Bus Master en el command register del
//! dispositivo — sin BME el xHC no puede hacer DMA y el driver ve silencio.

const CONFIG_ADDR: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

#[inline]
fn outl(port: u16, val: u32) {
    unsafe { core::arch::asm!("out dx, eax", in("dx") port, in("eax") val, options(nostack)) };
}

#[inline]
fn inl(port: u16) -> u32 {
    let v: u32;
    unsafe { core::arch::asm!("in eax, dx", in("dx") port, out("eax") v, options(nostack)) };
    v
}

fn cfg_addr(bus: u8, dev: u8, func: u8, off: u8) -> u32 {
    0x8000_0000
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((off as u32) & 0xFC)
}

pub fn cfg_read32(bus: u8, dev: u8, func: u8, off: u8) -> u32 {
    outl(CONFIG_ADDR, cfg_addr(bus, dev, func, off));
    inl(CONFIG_DATA)
}

pub fn cfg_write32(bus: u8, dev: u8, func: u8, off: u8, val: u32) {
    outl(CONFIG_ADDR, cfg_addr(bus, dev, func, off));
    outl(CONFIG_DATA, val);
}

/// Un controlador xHCI localizado.
#[derive(Clone, Copy)]
pub struct XhciLoc {
    pub bus: u8,
    pub dev: u8,
    pub func: u8,
    /// Dirección FÍSICA del MMIO (BAR0, con BAR1 como mitad alta si es
    /// 64-bit). El caller decide el mapeo virtual.
    pub mmio: u64,
}

/// Tipo de controlador de almacenamiento (clase PCI 0x01).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StorageKind { Ahci, Nvme, Ide, Raid, Other }

impl StorageKind {
    pub fn name(self) -> &'static str {
        match self {
            StorageKind::Ahci => "SATA/AHCI",
            StorageKind::Nvme => "NVMe",
            StorageKind::Ide  => "IDE",
            StorageKind::Raid => "RAID",
            StorageKind::Other => "storage",
        }
    }
}

pub struct StorageLoc {
    pub bus: u8, pub dev: u8, pub func: u8,
    /// MMIO base: ABAR (BAR5) para AHCI, BAR0 para NVMe.
    pub mmio: u64,
    pub kind: StorageKind,
}

/// Escanea el PCI buscando un controlador de ALMACENAMIENTO (clase 0x01):
/// subclase 0x06=SATA(AHCI), 0x08=NVMe, 0x01=IDE, 0x04=RAID. Habilita MEM+BME
/// y devuelve el primero. Primer paso para que el kernel aprenda a leer/escribir
/// disco (la caja negra de CABINA).
pub fn find_storage() -> Option<StorageLoc> {
    for bus in 0u16..=255 {
        let bus = bus as u8;
        for dev in 0u8..32 {
            let vd0 = cfg_read32(bus, dev, 0, 0x00);
            if vd0 == 0xFFFF_FFFF { continue; }
            let header0 = (cfg_read32(bus, dev, 0, 0x0C) >> 16) & 0xFF;
            let max_func = if header0 & 0x80 != 0 { 8 } else { 1 };
            for func in 0u8..max_func {
                let vd = cfg_read32(bus, dev, func, 0x00);
                if vd == 0xFFFF_FFFF { continue; }
                let class = cfg_read32(bus, dev, func, 0x08);
                let base = (class >> 24) as u8;
                let sub = (class >> 16) as u8;
                if base != 0x01 { continue; } // no es almacenamiento
                let kind = match sub {
                    0x06 => StorageKind::Ahci,
                    0x08 => StorageKind::Nvme,
                    0x01 => StorageKind::Ide,
                    0x04 => StorageKind::Raid,
                    _ => StorageKind::Other,
                };
                // Habilitar Memory Space + Bus Master (DMA).
                let cmd = cfg_read32(bus, dev, func, 0x04);
                cfg_write32(bus, dev, func, 0x04, cmd | 0x0006);
                // ABAR: AHCI usa BAR5 (0x24); NVMe usa BAR0 (0x10).
                let bar_off: u8 = if kind == StorageKind::Ahci { 0x24 } else { 0x10 };
                let bar = cfg_read32(bus, dev, func, bar_off);
                let mut mmio = (bar & 0xFFFF_FFF0) as u64;
                if (bar >> 1) & 0x3 == 0x2 {
                    let barhi = cfg_read32(bus, dev, func, bar_off + 4);
                    mmio |= (barhi as u64) << 32;
                }
                return Some(StorageLoc { bus, dev, func, mmio, kind });
            }
        }
    }
    None
}

/// Escanea todos los buses buscando xHCI (clase 0x0C, subclase 0x03,
/// prog-if 0x30). Al encontrarlo habilita MEM+BME y devuelve su ubicación.
/// `skip` permite saltar los primeros N hallazgos (para probar el segundo
/// controlador si el primero no tiene el teclado).
pub fn find_xhci(skip: usize) -> Option<XhciLoc> {
    let mut seen = 0usize;
    for bus in 0u16..=255 {
        let bus = bus as u8;
        for dev in 0u8..32 {
            // ¿Existe la función 0? Si no, el device entero está vacío.
            let vd0 = cfg_read32(bus, dev, 0, 0x00);
            if vd0 == 0xFFFF_FFFF {
                continue;
            }
            let header0 = (cfg_read32(bus, dev, 0, 0x0C) >> 16) & 0xFF;
            let max_func = if header0 & 0x80 != 0 { 8 } else { 1 };
            for func in 0u8..max_func {
                let vd = cfg_read32(bus, dev, func, 0x00);
                if vd == 0xFFFF_FFFF {
                    continue;
                }
                let class = cfg_read32(bus, dev, func, 0x08);
                let base = (class >> 24) as u8;
                let sub = (class >> 16) as u8;
                let prog = (class >> 8) as u8;
                if base == 0x0C && sub == 0x03 && prog == 0x30 {
                    if seen < skip {
                        seen += 1;
                        continue;
                    }
                    // Habilitar Memory Space (bit 1) + Bus Master (bit 2):
                    // sin BME el xHC no puede leer sus anillos por DMA.
                    let cmd = cfg_read32(bus, dev, func, 0x04);
                    cfg_write32(bus, dev, func, 0x04, cmd | 0x0006);
                    // BAR0 (+ BAR1 si el tipo es 64-bit: bits 2:1 == 10b).
                    let bar0 = cfg_read32(bus, dev, func, 0x10);
                    let mut mmio = (bar0 & 0xFFFF_FFF0) as u64;
                    if (bar0 >> 1) & 0x3 == 0x2 {
                        let bar1 = cfg_read32(bus, dev, func, 0x14);
                        mmio |= (bar1 as u64) << 32;
                    }
                    if mmio == 0 {
                        continue;
                    }
                    return Some(XhciLoc { bus, dev, func, mmio });
                }
            }
        }
    }
    None
}
