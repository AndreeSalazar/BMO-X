//! PCI config-space: acceso directo por puertos 0xCF8/0xCFC + scan completo.
//!
//! Motivo: el `pci_devices` del BootContext (scan de s2) no ve los
//! controladores que cuelgan detras de bridges -- y en los Ryzen los xHC USB
//! viven exactamente ahi (buses > 0). Este modulo escanea TODO el espacio
//! bus/dev/funcion por fuerza bruta (barato: unos miles de lecturas de
//! config una sola vez) y encuentra el dispositivo por su clase real,
//! incluyendo el prog-if que el BootContext no captura.
//!
//! Tambien habilita Memory Space + Bus Master en el command register del
//! dispositivo -- sin BME el xHC no puede hacer DMA y el driver ve silencio.

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

// -- ** MSI: que el aparato llame al CPU sin cablear nada ---------------------
//
// === Por que MSI y no la linea de interrupcion de siempre ===
//
// La forma clasica (INTx) es un cable: el aparato lo baja, un IOAPIC lo traduce
// a un vector, y el kernel tiene que saber **por que patilla entra cada
// dispositivo** -- routing de la placa, tablas del firmware, `_PRT` del ACPI. Es
// burocracia de verdad: un intermediario al que hay que preguntarle permiso para
// que dos partes que ya se conocen se hablen.
//
// MSI le da la vuelta: **la interrupcion es una ESCRITURA en memoria**. Al
// aparato se le dice "cuando termines, escribe este numero en esta direccion", y
// esa direccion es el LAPIC del CPU. No hay IOAPIC, no hay tabla de routing, no
// hay que preguntarle a nadie. El aparato habla con el CPU **directamente**.
//
// Para este kernel eso ademas quita un subsistema entero: aqui no hay codigo de
// IOAPIC y con esto no hace falta.
//
// === Lo que hay que escribirle, y ya esta ===
//
// | campo | que lleva |
// |---|---|
// | Message Address | `0xFEE0_0000 | (apic_id << 12)` -- la ventana del LAPIC |
// | Message Data | el numero de vector |
// | Message Control, bit 0 | ENABLE |
//
// El bit 7 del Control dice si el aparato es de 64 bits: si lo es, el dato vive
// cuatro bytes mas alla porque la direccion ocupa dos palabras. Equivocarse en
// eso escribe el vector encima de la mitad alta de la direccion, y entonces la
// interrupcion se manda **a una direccion inventada**.

/// La ventana de mensajes del LAPIC. Escribir aqui ES interrumpir.
const MSI_LAPIC_BASE: u32 = 0xFEE0_0000;
/// Id de la capability MSI en la lista encadenada de PCI.
const CAP_ID_MSI: u8 = 0x05;
/// Bit 10 del registro de comando: **deshabilitar INTx**. Con MSI activo, la
/// linea de siempre tiene que callarse, o el aparato podria avisar por las dos.
const CMD_INTX_DISABLE: u32 = 1 << 10;
/// Bit 2: maestro de bus. Sin el no hay DMA -- y sin DMA no hay nada que
/// interrumpir. Se comprueba en vez de suponerlo.
const CMD_BUS_MASTER: u32 = 1 << 2;

/// **Programa MSI en un dispositivo**: que sus interrupciones lleguen como
/// `vector` al CPU `apic_id`. `true` si quedo armado.
///
/// Devuelve `false` --sin tocar nada-- si el dispositivo no anuncia MSI. Quien
/// llame tiene que quedarse entonces como estaba: encender las interrupciones de
/// un aparato cuya senal no va a llegar a ninguna parte es peor que no
/// encenderlas, porque el aparato se queda esperando a que alguien le conteste.
pub fn msi_activar(bus: u8, dev: u8, func: u8, vector: u8, apic_id: u8) -> bool {
    // Hay lista de capabilities? Bit 4 del registro de estado (offset 0x06).
    let status = cfg_read32(bus, dev, func, 0x04) >> 16;
    if status & (1 << 4) == 0 {
        return false;
    }
    let mut off = (cfg_read32(bus, dev, func, 0x34) & 0xFC) as u8;
    // Tope de vueltas: una lista encadenada corrupta no puede colgar el arranque.
    let mut saltos = 0;
    while off >= 0x40 && saltos < 48 {
        saltos += 1;
        let cab = cfg_read32(bus, dev, func, off);
        let id = (cab & 0xFF) as u8;
        if id == CAP_ID_MSI {
            let control = (cab >> 16) as u16;
            let de_64 = control & (1 << 7) != 0;
            // La direccion primero, el dato despues, y el ENABLE al final: un
            // aparato al que se le enciende el permiso antes de decirle a donde
            // escribir puede mandar un mensaje a la direccion que hubiera.
            cfg_write32(bus, dev, func, off + 4, MSI_LAPIC_BASE | ((apic_id as u32) << 12));
            if de_64 {
                cfg_write32(bus, dev, func, off + 8, 0);
                cfg_write32(bus, dev, func, off + 12, vector as u32);
            } else {
                cfg_write32(bus, dev, func, off + 8, vector as u32);
            }
            // Multiple Message Enable a 0 (un solo mensaje) y ENABLE a 1.
            let nuevo = (control & !(0x7 << 4)) | 1;
            cfg_write32(bus, dev, func, off, (cab & 0xFFFF) | ((nuevo as u32) << 16));
            // Y ahora que MSI habla, que INTx se calle.
            //
            // [!] La mitad ALTA se escribe a cero. Ahi vive el registro de
            // ESTADO, y sus bits son "escribe 1 para borrar": devolver lo que se
            // acaba de leer borraria en silencio los avisos de error que el
            // aparato tuviera puestos. Cero no borra nada.
            let cmd = cfg_read32(bus, dev, func, 0x04);
            cfg_write32(bus, dev, func, 0x04, (cmd & 0xFFFF) | CMD_INTX_DISABLE);
            if cmd & CMD_BUS_MASTER == 0 {
                // No se rechaza --MSI ya esta armado y es correcto-- pero se
                // dice: un aparato sin maestro de bus no hace DMA, asi que no
                // va a tener nada que anunciar.
                crate::ring0::cabina::warn("pci", "MSI armado en un aparato SIN maestro de bus", off as u64);
            }
            return true;
        }
        off = ((cab >> 8) & 0xFC) as u8;
    }
    false
}

/// Un controlador xHCI localizado.
#[derive(Clone, Copy)]
pub struct XhciLoc {
    pub bus: u8,
    pub dev: u8,
    pub func: u8,
    /// Direccion FISICA del MMIO (BAR0, con BAR1 como mitad alta si es
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

/// Escanea el PCI buscando un controlador de ALMACENAMIENTO (clase 0x01) de un
/// TIPO CONCRETO: subclase 0x06=SATA(AHCI), 0x08=NVMe, 0x01=IDE, 0x04=RAID.
///
/// * POR QUE POR TIPO Y NO "EL PRIMERO": en esta maquina el primer controlador
/// del barrido es el NVMe, y en el NVMe vive WINDOWS. El disco de BMO (A: con
/// el arranque, y BMO-DATA) cuelga de SATA. Pedir "el primer disco que
/// encuentres" y escribir en el habria sido escribir en el sistema del dueno.
/// Un driver de almacenamiento no adivina a quien le habla: se le dice.
///
/// `skip` salta los primeros N hallazgos de ese tipo (placas con dos HBA).
pub fn find_storage_of(kind: StorageKind, skip: usize) -> Option<StorageLoc> {
    let mut seen = 0usize;
    let mut index = 0usize;
    while let Some(loc) = storage_at(index) {
        index += 1;
        if loc.kind != kind { continue; }
        if seen < skip { seen += 1; continue; }
        enable_mem_bus_master(loc.bus, loc.dev, loc.func);
        return Some(loc);
    }
    None
}

/// Habilita Memory Space + Bus Master (DMA) en el dispositivo. Sin BME el
/// controlador no puede leer sus estructuras en RAM y el driver ve silencio.
fn enable_mem_bus_master(bus: u8, dev: u8, func: u8) {
    let cmd = cfg_read32(bus, dev, func, 0x04);
    cfg_write32(bus, dev, func, 0x04, cmd | 0x0006);
}

/// El controlador de almacenamiento numero `index` del barrido, SIN tocar su
/// configuracion. Para hacer censo (mirar) sin habilitar nada (actuar).
pub fn storage_at(index: usize) -> Option<StorageLoc> {
    let mut seen = 0usize;
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
                if (class >> 24) as u8 != 0x01 { continue; } // no es almacenamiento
                let kind = match (class >> 16) as u8 {
                    0x06 => StorageKind::Ahci,
                    0x08 => StorageKind::Nvme,
                    0x01 => StorageKind::Ide,
                    0x04 => StorageKind::Raid,
                    _ => StorageKind::Other,
                };
                if seen != index { seen += 1; continue; }
                // ABAR: AHCI usa BAR5 (0x24); el resto BAR0 (0x10).
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
/// prog-if 0x30). Al encontrarlo habilita MEM+BME y devuelve su ubicacion.
/// `skip` permite saltar los primeros N hallazgos (para probar el segundo
/// controlador si el primero no tiene el teclado).
pub fn find_xhci(skip: usize) -> Option<XhciLoc> {
    let mut seen = 0usize;
    for bus in 0u16..=255 {
        let bus = bus as u8;
        for dev in 0u8..32 {
            // Existe la funcion 0? Si no, el device entero esta vacio.
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
