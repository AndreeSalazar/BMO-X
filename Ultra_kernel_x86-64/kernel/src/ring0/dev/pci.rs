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
// ===================================================================
//  ECAM -- la configuracion de PCIe en memoria, y los 4096 bytes
// ===================================================================
//
//  ## Por que hay DOS caminos y no se sustituye el viejo
//
//  ```text
//     puertos 0xCF8/0xCFC   256 bytes por funcion.  Funciona, arranca la maquina
//     ECAM (memoria)       4096 bytes por funcion.  Lo unico que alcanza las
//                                                   capabilities extendidas
//  ```
//
//  ** El camino de puertos NO se toca. Es el que enumera el disco, la NIC y el
//  xHCI en cada arranque desde hace meses, y sustituirlo por uno nuevo el mismo
//  dia que el nuevo se escribe es cambiar lo que funciona por lo que todavia no
//  se ha visto funcionar. El nuevo se anade AL LADO y se gana el sitio.
//
//  ## *** Y COMO SE GANA EL SITIO: DOS TESTIGOS
//
//  La direccion base de ECAM sale del MCFG, o sea del firmware. Si esa base
//  fuera la equivocada, leer por ella daria numeros -- **numeros plausibles**,
//  porque cualquier memoria leida da un `u32`. Y entonces el fallo no seria una
//  excepcion: seria un vendor id inventado, tres arranques mas tarde.
//
//  Asi que antes de creerse ECAM se lee **el mismo registro por los dos
//  caminos** y se comparan. Es el metodo con el que se midio la puerta --dos
//  instrumentos que coinciden validan el instrumento-- y aqui contesta la unica
//  pregunta que importa: *la base que dio el firmware, lleva a donde dice?*

/// La base de ECAM del segmento 0, y el rango de buses que cubre.
/// `(base, bus_desde, bus_hasta)`. Base cero = no hay MCFG o no se creyo.
static mut ECAM: (u64, u8, u8) = (0, 0, 0);

/// **Se pudo creer el ECAM?** Falso hasta que los dos testigos coincidan.
static mut ECAM_CREIBLE: bool = false;

/// Donde cae el registro `off` de una funcion, en memoria fisica.
///
/// [!] `bus - bus_desde`: la ventana empieza en `bus_desde`, no en el bus 0. En
/// una placa donde vale 0 --que son casi todas-- restar o no restar da lo mismo,
/// y por eso el fallo no aparece hasta la placa que no lo es.
unsafe fn ecam_addr(bus: u8, dev: u8, func: u8, off: u16) -> Option<u64> {
    unsafe {
        let (base, desde, hasta) = ECAM;
        if base == 0 || bus < desde || bus > hasta {
            return None;
        }
        if dev > 31 || func > 7 || off >= 4096 {
            return None;
        }
        Some(
            base + (((bus - desde) as u64) << 20)
                + ((dev as u64) << 15)
                + ((func as u64) << 12)
                + off as u64,
        )
    }
}

/// Lee por ECAM **sin comprobar si se creyo**. Para el propio careo.
unsafe fn ecam_read32_crudo(bus: u8, dev: u8, func: u8, off: u16) -> Option<u32> {
    unsafe {
        let a = ecam_addr(bus, dev, func, off)?;
        let v = crate::ring0::mm::phys_to_virt(a) as *const u32;
        Some(core::ptr::read_volatile(v))
    }
}

/// **Monta ECAM y lo somete al careo.** Se llama una vez, al arrancar.
///
/// *** El careo es la parte que no se puede saltar. Lee el vendor/device de
/// cada funcion que el camino de puertos ya encontro, y exige que ECAM diga lo
/// mismo. Si UNA sola discrepa, ECAM se queda apagado entero: una ventana que
/// acierta a veces es peor que ninguna, porque el dia que falla lo hace con un
/// numero con pinta de buen dato.
pub fn ecam_montar(rsdp: u64) {
    use crate::ring0::plat::placa;

    let mut r = [placa::RangoEcam { base: 0, segmento: 0, bus_desde: 0, bus_hasta: 0 };
        placa::MAX_ECAM];
    let n = placa::ecam(rsdp, &mut r);
    // Se toma el segmento 0: es el unico que hay en una maquina de escritorio, y
    // elegir "el primero" sin mirar el segmento seria acertar por costumbre.
    let Some(cero) = r[..n].iter().find(|x| x.segmento == 0) else {
        crate::ring0::cabina::count("pci", "sin MCFG: la config se queda en 256 B", 0);
        return;
    };
    unsafe {
        ECAM = (cero.base, cero.bus_desde, cero.bus_hasta);
        ECAM_CREIBLE = false;
    }

    // === EL CAREO ===================================================
    //
    // Se recorre el bus 0 entero, que es donde vive el chipset y donde el
    // camino de puertos ya sabe leer. Cada funcion presente tiene que dar el
    // MISMO vendor/device por los dos caminos.
    let mut vistas = 0u32;
    let mut discrepan = 0u32;
    for dev in 0u8..32 {
        for func in 0u8..8 {
            let por_puertos = cfg_read32(0, dev, func, 0x00);
            // 0xFFFFFFFF es "aqui no hay nadie", y no se carea con nada.
            if por_puertos == 0xFFFF_FFFF {
                continue;
            }
            vistas += 1;
            let por_memoria = unsafe { ecam_read32_crudo(0, dev, func, 0x00) };
            match por_memoria {
                Some(v) if v == por_puertos => {}
                _ => discrepan += 1,
            }
            // Una funcion 0 que no es multifuncion no tiene 1..7.
            if func == 0 {
                let hdr = cfg_read32(0, dev, 0, 0x0C);
                if (hdr >> 16) & 0x80 == 0 {
                    break;
                }
            }
        }
    }

    if vistas > 0 && discrepan == 0 {
        unsafe { ECAM_CREIBLE = true };
        crate::ring0::cabina::addr("pci", "ECAM montado y careado, base", cero.base);
        crate::ring0::cabina::count("pci", "  ...funciones que coinciden por los 2 caminos", vistas as u64);
    } else {
        unsafe {
            ECAM = (0, 0, 0);
            ECAM_CREIBLE = false;
        }
        // *** Apagado ENTERO, no "con cuidado". Ver la cabecera de esta funcion.
        crate::ring0::cabina::warn(
            "pci",
            "[!] ECAM NO coincide con los puertos: apagado. Funciones que discrepan",
            discrepan as u64,
        );
    }
}

/// Se puede leer configuracion extendida?
pub fn hay_ecam() -> bool {
    unsafe { ECAM_CREIBLE }
}

/// **Lee configuracion PCIe, incluidos los 3.840 bytes que los puertos no
/// alcanzan.**
///
/// ** El `off` es un `u16` y no un `u8`, y ahi esta la diferencia entera con
/// [`cfg_read32`]: el techo de 256 de aquella funcion **esta en su tipo**, no en
/// una comprobacion que se pueda olvidar.
///
/// `None` si ECAM no se pudo creer, o si el offset se sale. Y `None` es la
/// respuesta correcta -- devolver `0xFFFFFFFF` seria indistinguible de una
/// funcion que no existe, y devolver `0` seria un dato inventado.
pub fn cfg_read32_ext(bus: u8, dev: u8, func: u8, off: u16) -> Option<u32> {
    if !hay_ecam() {
        return None;
    }
    unsafe { ecam_read32_crudo(bus, dev, func, off) }
}

// ===================================================================
//  Las capabilities EXTENDIDAS -- el primer usuario de ECAM
// ===================================================================
//
//  ** Existen para que ECAM no sea otra pieza escrita que no llama nadie. La
//  lista empieza en el offset `0x100` --justo donde se acaba lo que alcanzan los
//  puertos-- y esa direccion es literalmente la frontera entre los dos caminos.
//
//  ## *** Y dos de ellas deciden si la IOMMU sirve para algo
//
//  ```text
//     ATS   el aparato puede usar direcciones VIRTUALES y cachear sus
//           traducciones. Sin esto la IOMMU funciona, pero cada acceso del
//           aparato pasa por una traduccion
//
//     ACS   *** el aparato NO deja que dos funciones detras del mismo puente
//           se hablen ENTRE ELLAS saltandose la IOMMU
//  ```
//
//  ** La segunda es la que importa y casi nunca se cuenta: **sin ACS, dos
//  aparatos en el mismo puente pueden hacer DMA el uno contra el otro sin que la
//  IOMMU se entere.** Encender la IOMMU sin mirar ACS es poner una puerta en una
//  habitacion que tiene otra puerta.

/// Donde empieza la lista de capabilities extendidas. Es exactamente donde se
/// acaban los 256 bytes que alcanzan los puertos.
pub const CAPS_EXT_INICIO: u16 = 0x100;

/// Cuantas se recorren como mucho.
///
/// ** Un tope, porque la lista es una cadena de punteros que el APARATO
/// escribe: una cadena que apunte a si misma dejaria el bucle girando en el
/// arranque. Es la misma regla que el recorrido del IVRS -- **un bucle sobre
/// datos que da el hardware tiene que terminar aunque el hardware mienta.**
const MAX_CAPS: usize = 32;

/// Que es una capability extendida, por su id.
pub fn nombre_cap_ext(id: u16) -> &'static str {
    match id {
        0x0001 => "AER (errores del enlace, con detalle)",
        0x0002 => "canales virtuales",
        0x0003 => "numero de serie del aparato",
        0x0004 => "presupuesto de energia",
        0x000B => "del fabricante",
        0x000D => "ACS -- impide que dos funciones se salten la IOMMU",
        0x000E => "ARI (mas de 8 funciones)",
        0x000F => "ATS -- el aparato traduce direcciones",
        0x0010 => "SR-IOV (funciones virtuales)",
        0x0018 => "LTR (latencia tolerable)",
        0x0019 => "PCIe secundario",
        0x001E => "subestados L1 de energia",
        0x0025 => "DPC (contener un fallo del enlace)",
        _ => "",
    }
}

/// Una capability extendida encontrada.
#[derive(Clone, Copy)]
pub struct CapExt {
    pub id: u16,
    pub version: u8,
    pub offset: u16,
}

/// **Recorre las capabilities extendidas de una funcion.** Devuelve cuantas.
///
/// `None` implicito: si ECAM no se pudo creer, devuelve 0 -- y eso es correcto,
/// porque sin ECAM esas capabilities **no son ilegibles, son inalcanzables**.
pub fn caps_extendidas(bus: u8, dev: u8, func: u8, salida: &mut [CapExt]) -> usize {
    if !hay_ecam() || salida.is_empty() {
        return 0;
    }
    let mut off = CAPS_EXT_INICIO;
    let mut n = 0usize;
    let mut vueltas = 0usize;
    while off >= CAPS_EXT_INICIO && off < 4096 && n < salida.len() && vueltas < MAX_CAPS {
        vueltas += 1;
        let Some(cab) = cfg_read32_ext(bus, dev, func, off) else {
            break;
        };
        // ** Cero o todo unos: no hay lista. Las dos significan lo mismo aqui y
        // se miran las dos, porque un aparato sin capabilities extendidas puede
        // contestar cualquiera de ellas.
        if cab == 0 || cab == 0xFFFF_FFFF {
            break;
        }
        let id = (cab & 0xFFFF) as u16;
        salida[n] = CapExt {
            id,
            version: ((cab >> 16) & 0xF) as u8,
            offset: off,
        };
        n += 1;
        let siguiente = ((cab >> 20) & 0xFFF) as u16;
        // ** Una siguiente que no avanza es una cadena que se muerde la cola.
        // Cortarla aqui y no confiar solo en `MAX_CAPS` deja el motivo escrito
        // en el sitio donde pasa.
        if siguiente <= off {
            break;
        }
        off = siguiente;
    }
    n
}

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

// -- ** LA TARJETA DE RED. Solo encontrarla. ---------------------------------

/// Una NIC localizada. **Nada de esto la maneja**: la reconoce.
#[derive(Clone, Copy)]
pub struct NetLoc {
    pub bus: u8,
    pub dev: u8,
    pub func: u8,
    /// Quien la fabrica y que modelo. `0x10EC` = Realtek.
    pub vendor: u16,
    pub device: u16,
    /// El primer BAR de MEMORIA, ya compuesto si era de 64 bits. `0` = la
    /// tarjeta solo declara puertos de E/S y aqui no hay nada que mapear.
    pub mmio: u64,
    /// **Cual de los seis era.** No es curiosidad: ver `find_net`.
    pub bar_index: u8,
    /// Los seis BAR crudos, sin interpretar. Para poder DECIRLOS.
    pub bars: [u32; 6],
}

/// Escanea el PCI buscando una **NIC Ethernet** (clase 0x02, subclase 0x00).
/// Habilita MEM+BME y devuelve donde esta. `skip` salta los primeros N.
///
/// == Por que el BAR se BUSCA y no se sabe ==
///
/// Cada familia pone su MMIO donde quiere: AHCI en el BAR5, xHCI en el BAR0, y
/// las Realtek **han cambiado de sitio segun el modelo** -- las 8169 usan la
/// region 1 y las 8168 la 2. Escribir `0x18` aqui seria acertar en esta placa y
/// fallar en la siguiente, que es justo el fallo que ya se pago dos veces en
/// este proyecto:
///
/// > **Por lo que ES, nunca por donde esta.**
///
/// Asi que se recorren los seis y se coge **el primero que sea de memoria**, que
/// es lo mismo que hace el driver de Linux desde que dejo de llevar una tabla
/// por modelo. La regla no necesita mantenimiento: una NIC que mueva su MMIO de
/// BAR sigue funcionando sin tocar una linea.
///
/// [!] Y se guardan **los seis crudos**. Si la MAC sale mal, la primera pregunta
/// va a ser "de que BAR la leiste", y esa foto tiene que existir ya.
pub fn find_net(skip: usize) -> Option<NetLoc> {
    let mut seen = 0usize;
    for bus in 0u16..=255 {
        let bus = bus as u8;
        for dev in 0u8..32 {
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
                // 0x02 = red; subclase 0x00 = Ethernet. Un Wi-Fi es 0x80 y no
                // entra: se parece en la clase y no en nada mas.
                if (class >> 24) as u8 != 0x02 || (class >> 16) as u8 != 0x00 {
                    continue;
                }
                if seen < skip {
                    seen += 1;
                    continue;
                }
                enable_mem_bus_master(bus, dev, func);

                let mut bars = [0u32; 6];
                for k in 0..6 {
                    bars[k] = cfg_read32(bus, dev, func, 0x10 + (k as u8) * 4);
                }
                // El primero de MEMORIA. Un BAR de 64 bits se come el siguiente
                // --su mitad alta-- y por eso el paso no es fijo: leer esa mitad
                // como si fuera un BAR suelto daria una direccion inventada.
                let mut mmio = 0u64;
                let mut bar_index = 0xFFu8;
                let mut i = 0usize;
                while i < 6 {
                    let b = bars[i];
                    let es_io = b & 1 != 0;
                    let de_64 = !es_io && (b >> 1) & 0x3 == 0x2;
                    if !es_io && mmio == 0 && (b & 0xFFFF_FFF0) != 0 {
                        mmio = (b & 0xFFFF_FFF0) as u64;
                        if de_64 && i + 1 < 6 {
                            mmio |= (bars[i + 1] as u64) << 32;
                        }
                        bar_index = i as u8;
                    }
                    i += if de_64 { 2 } else { 1 };
                }
                return Some(NetLoc {
                    bus,
                    dev,
                    func,
                    vendor: (vd & 0xFFFF) as u16,
                    device: (vd >> 16) as u16,
                    mmio,
                    bar_index,
                    bars,
                });
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
