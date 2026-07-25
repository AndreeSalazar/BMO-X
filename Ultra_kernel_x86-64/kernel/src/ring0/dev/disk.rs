//! Disco: el puente entre Ring 0 y el driver AHCI/SATA.
//!
//! El kernel no sabe de puertos SATA ni de FIS: eso vive en `bmo-ahci`. Aquí
//! solo se le prestan al driver los tres servicios que no puede tener por su
//! cuenta (memoria DMA contigua, traducción física→virtual y una salida de
//! log) y se le dice A QUÉ CONTROLADOR hablar.
//!
//! ## Por qué AHCI y no NVMe
//!
//! Esta máquina tiene los dos. El primer controlador del barrido PCI es el
//! NVMe, y en el NVMe vive el Windows del dueño; el disco de BMO — el que
//! lleva la partición de arranque y BMO-DATA — cuelga de SATA. Pedir "el
//! primer disco" y escribir habría sido escribir en el sistema ajeno. Por eso
//! se pide el controlador POR TIPO, nunca por orden de aparición.
//!
//! ## Solo lectura, a propósito
//!
//! `bmo-ahci` sabe escribir sectores. Este puente NO expone esa función. Un
//! disco se identifica antes de tocarlo: mientras BMO no pueda demostrar que
//! el disco que tiene delante es el suyo (leyendo su tabla de particiones y
//! reconociendo su propio arranque), escribir es apostar. La escritura llega
//! cuando la identificación esté cerrada, no antes.

use crate::ring0::mm::{self, phys};
use crate::ring0::dev::pci::{self, StorageKind};
use bmo_ahci::{storage_hal, StorageHal};

/// Tamaño de sector de un disco SATA moderno visto por LBA de 512 B.
pub const SECTOR: usize = 512;

// ── Log del driver, línea a línea ───────────────────────────────────────────
// El driver escribe en fragmentos; se acumulan hasta el '\n' y se vuelca la
// línea entera al panel. Mismo patrón que el puente USB: sin esto, en una
// placa sin cable serie el diagnóstico del driver es invisible.

const DLOG_MAX: usize = 96;
static mut DLOG: [u8; DLOG_MAX] = [0u8; DLOG_MAX];
static mut DLOG_N: usize = 0;

fn dlog(s: &str) {
    crate::ring0::dev::console::serial_write(s);
    if !crate::info::has_fb() { return; }
    unsafe {
        let buf = &mut *core::ptr::addr_of_mut!(DLOG);
        for &b in s.as_bytes() {
            if b == b'\n' {
                let n = DLOG_N;
                if n > 0 {
                    if let Ok(line) = core::str::from_utf8(&buf[..n]) {
                        crate::ring0::core::phase::dashboard_log(line);
                    }
                }
                DLOG_N = 0;
            } else if b >= 0x20 && b < 0x7F && DLOG_N < DLOG_MAX {
                buf[DLOG_N] = b;
                DLOG_N += 1;
            }
        }
    }
}

/// Hex compacto al log del driver: los registros se leen en hexadecimal.
fn dlog_u64(val: u64) {
    const H: &[u8; 16] = b"0123456789ABCDEF";
    let mut tmp = [0u8; 18];
    let mut o = 0;
    tmp[o] = b'0'; o += 1;
    tmp[o] = b'x'; o += 1;
    let mut started = false;
    for i in (0..16).rev() {
        let nib = ((val >> (i * 4)) & 0xF) as usize;
        if nib != 0 || started || i == 0 {
            tmp[o] = H[nib];
            o += 1;
            started = true;
        }
    }
    if let Ok(s) = core::str::from_utf8(&tmp[..o]) { dlog(s); }
}

/// Lo que `bmo-ahci` necesita del kernel. Nada más que esto.
struct KernelStorageHal;

impl StorageHal for KernelStorageHal {
    fn alloc_dma_pages(&self, count: usize) -> Option<u64> {
        // CONTIGUOS: la lista de comandos y las tablas de descriptores las
        // recorre el HBA por dirección física, linealmente. Dos frames que no
        // se tocan serían dos estructuras rotas.
        phys::alloc_frames_contig(count as u64)
    }
    fn free_dma_pages(&self, _addr: u64, _count: usize) {
        // El disco se abre una vez y vive lo que vive el kernel: no hay ciclo
        // de vida que liberar. Cuando lo haya, aquí va phys::free_frames.
    }
    fn phys_to_virt(&self, phys: u64) -> *mut u8 {
        mm::phys_to_virt(phys) as *mut u8
    }
    fn log(&self, msg: &str) {
        dlog(msg);
    }
    fn log_hex(&self, msg: &str, value: u64) {
        dlog(msg);
        dlog_u64(value);
    }
}

static HAL: KernelStorageHal = KernelStorageHal;

// ── Estado ──────────────────────────────────────────────────────────────────

static mut READY: bool = false;
static mut PORT: u8 = 0xFF;
static mut MMIO: u64 = 0;
/// Sectores del disco, si la tabla de particiones lo declara.
static mut LAST_LBA: u64 = 0;

/// Página de rebote para el DMA. El HBA escribe SIEMPRE aquí, en una
/// dirección física conocida y contigua, y el kernel copia de aquí al buffer
/// del llamante. Así ninguna capa de arriba necesita saber de direcciones
/// físicas ni tener memoria apta para DMA.
static mut DMA_PHYS: u64 = 0;

/// Modelo y serie que el propio disco declara (IDENTIFY DEVICE).
static mut MODEL: [u8; 40] = [0; 40];
static mut MODEL_LEN: usize = 0;
static mut TOTAL_SECTORS: u64 = 0;

/// ¿Hay un disco listo para leer?
pub fn is_ready() -> bool { unsafe { READY } }
/// Puerto AHCI en uso (0xFF = ninguno).
pub fn port() -> u8 { unsafe { PORT } }
/// MMIO del HBA.
pub fn mmio() -> u64 { unsafe { MMIO } }
/// Modelo declarado por el disco. Vacío si aún no se le preguntó.
pub fn model() -> &'static str {
    unsafe {
        let p = core::ptr::addr_of!(MODEL) as *const u8;
        core::str::from_utf8(core::slice::from_raw_parts(p, MODEL_LEN)).unwrap_or("")
    }
}
/// Sectores totales que declara el disco (IDENTIFY, LBA48).
pub fn total_sectors() -> u64 { unsafe { TOTAL_SECTORS } }

/// Despierta el disco de BMO: busca el HBA **SATA** por PCI, lo prepara y deja
/// listo el primer puerto con un disco de verdad conectado.
pub fn init() {
    storage_hal::init_hal(&HAL);

    // Una placa puede traer más de un HBA SATA. Se prueban en orden hasta dar
    // con uno que tenga un disco enlazado — el mismo patrón que el USB, que ya
    // nos enseñó que el teclado estaba en el segundo controlador.
    let mut chosen = 0xFFu8;
    let mut loc_ok = None;
    for skip in 0..4usize {
        let loc = match pci::find_storage_of(StorageKind::Ahci, skip) {
            Some(l) => l,
            None => break,
        };
        let mmio_va = if loc.mmio < 0x1_0000_0000 { loc.mmio } else { mm::phys_to_virt(loc.mmio) };
        crate::ring0::cabina::info("disk", "HBA SATA/AHCI hallado en PCI", loc.mmio);

        bmo_ahci::reset_ctrl();
        if !unsafe { bmo_ahci::probe(mmio_va) } {
            crate::ring0::cabina::warn("disk", "el HBA no inicializo, probando el siguiente", loc.mmio);
            continue;
        }

        let ctrl = match bmo_ahci::controller() { Some(c) => c, None => continue };
        // El estado crudo de cada puerto lo pinta el driver (`probe`). Aquí
        // solo se cuenta y se elige.
        //
        // ★ Se itera por ÍNDICE, no por `p.port_number`. Las entradas vacías
        // del array llevan port_number = 0, así que filtrar por su campo hacía
        // que CADA hueco se colara haciéndose pasar por el puerto 0: catorce
        // líneas idénticas del mismo puerto inexistente, y una espera de
        // enlace completa concedida a cada fantasma (los 3-4 segundos de
        // arranque). El índice del array ES el número de puerto; el campo solo
        // significa algo en las entradas que `probe` llenó.
        let mut active = 0u64;
        for i in 0..32usize {
            if ctrl.ports_implemented & (1 << i) == 0 { continue; }
            let p = &ctrl.ports[i];
            if p.state == bmo_ahci::PortState::Active {
                active += 1;
                // Firma 0x00000101 = disco duro SATA. Un ATAPI (0xEB140101) es
                // una unidad óptica: no es donde vive BMO.
                if chosen == 0xFF && p.signature == bmo_ahci::SIG_SATA_DISK {
                    chosen = i as u8;
                }
            }
        }
        crate::ring0::cabina::info("disk", "puertos SATA con disco enlazado", active);
        if chosen != 0xFF {
            unsafe { MMIO = loc.mmio; }
            loc_ok = Some(loc);
            break;
        }
    }
    if chosen == 0xFF || loc_ok.is_none() {
        crate::ring0::cabina::fault("disk", "ningun puerto SATA con disco (mira ssts)", 0);
        return;
    }

    if !unsafe { bmo_ahci::init_port_dma(chosen) } {
        crate::ring0::cabina::fault("disk", "no se pudo preparar el DMA del puerto", chosen as u64);
        return;
    }
    // Página de rebote para el DMA, contigua y de dirección física conocida.
    let dma = match phys::alloc_frames_contig(1) {
        Some(p) => p,
        None => {
            crate::ring0::cabina::fault("disk", "sin memoria para el buffer DMA", 0);
            return;
        }
    };
    unsafe { DMA_PHYS = dma; PORT = chosen; READY = true; }
    crate::ring0::cabina::info("disk", "puerto SATA listo para leer", chosen as u64);

    identify();
}

/// Le pregunta al disco QUIÉN ES.
///
/// Esta máquina tiene tres discos y en uno vive el sistema del dueño. Un
/// kernel que va a escribir algún día tiene que poder decir "estoy hablando
/// con el Kingston de 480 GB", no "estoy hablando con el primero que salió".
fn identify() {
    let dma = unsafe { DMA_PHYS };
    if dma == 0 { return; }
    match unsafe { bmo_ahci::identify_phys(unsafe { PORT }, dma) } {
        Ok(_) => {}
        Err(e) => {
            crate::ring0::cabina::warn("disk", e.name(), 0);
            return;
        }
    }
    let src = mm::phys_to_virt(dma) as *const u8;
    // Las cadenas de IDENTIFY vienen en palabras de 16 bits con los dos bytes
    // AL REVÉS (convención ATA de toda la vida). Modelo: palabras 27..46.
    let mut n = 0usize;
    unsafe {
        for w in 27..47usize {
            let hi = src.add(w * 2).read_volatile();
            let lo = src.add(w * 2 + 1).read_volatile();
            for c in [hi, lo] {
                if n < MODEL.len() && c >= 0x20 && c < 0x7F { MODEL[n] = c; n += 1; }
            }
        }
        // Quitar el relleno de espacios del final.
        while n > 0 && MODEL[n - 1] == b' ' { n -= 1; }
        MODEL_LEN = n;
        // Palabras 100..103: sectores direccionables (LBA48).
        let mut total = 0u64;
        for i in (0..4usize).rev() {
            let w = 100 + i;
            let lo = src.add(w * 2).read_volatile() as u64;
            let hi = src.add(w * 2 + 1).read_volatile() as u64;
            total = (total << 16) | (hi << 8) | lo;
        }
        TOTAL_SECTORS = total;
    }
    crate::ring0::cabina::info("disk", model(), unsafe { TOTAL_SECTORS });
}

/// Lee `count` sectores desde `lba` en `buf`. Devuelve los sectores leídos.
///
/// Es la ÚNICA puerta al disco que este kernel abre hoy. La escritura existe
/// en el driver y no se expone: ver la nota de cabecera.
///
/// Va por la página de rebote y copia: el llamante puede tener su buffer donde
/// quiera —la pila, un estático— sin que nada de eso tenga que ser memoria
/// apta para DMA ni de dirección física contigua.
pub fn read(lba: u64, count: u16, buf: &mut [u8]) -> u16 {
    if !is_ready() || count == 0 { return 0; }
    let want = count as usize * SECTOR;
    if buf.len() < want { return 0; }
    let dma = unsafe { DMA_PHYS };
    if dma == 0 { return 0; }

    const PER_BATCH: u16 = (4096 / SECTOR) as u16; // 8 sectores por página
    let mut done = 0u16;
    while done < count {
        let batch = (count - done).min(PER_BATCH);
        let got = match unsafe { bmo_ahci::read_sectors_phys(unsafe { PORT }, lba + done as u64, batch, dma) } {
            Ok(n) => n,
            Err(e) => {
                crate::ring0::cabina::fault("disk", e.name(), lba + done as u64);
                return done;
            }
        };
        if got == 0 { return done; }
        let src = mm::phys_to_virt(dma) as *const u8;
        let dst_off = done as usize * SECTOR;
        let n = got as usize * SECTOR;
        unsafe { core::ptr::copy_nonoverlapping(src, buf.as_mut_ptr().add(dst_off), n); }
        done += got;
        if got < batch { break; } // lectura corta: el disco dijo basta
    }
    done
}

// ── GPT: la tabla de particiones ────────────────────────────────────────────
//
// Leerla es cómo BMO reconoce el disco que tiene delante. No hace falta
// confiar en el orden del PCI ni en que el firmware enumere igual dos veces:
// el disco propio es el que lleva estas particiones y no otras.

/// Una partición encontrada en la GPT.
#[derive(Clone, Copy)]
pub struct Partition {
    pub index: u32,
    pub first_lba: u64,
    pub last_lba: u64,
    /// Primeros 4 bytes del GUID de tipo — basta para distinguir las que nos
    /// importan sin arrastrar 16 bytes por todos lados.
    pub type_lo: u32,
    /// Nombre de la partición (UTF-16 en disco, aquí solo su parte ASCII).
    pub name: [u8; 36],
    pub name_len: usize,
}

impl Partition {
    pub fn sectors(&self) -> u64 { self.last_lba.saturating_sub(self.first_lba) + 1 }
    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }
    /// ¿Es la partición de sistema EFI? (GUID C12A7328-...) Ahí vive el
    /// arranque de BMO.
    pub fn is_esp(&self) -> bool { self.type_lo == 0xC12A_7328 }
    /// ¿Datos básicos de Microsoft? (GUID EBD0A0A2-...) BMO-DATA es de este
    /// tipo mientras siga en NTFS.
    pub fn is_basic_data(&self) -> bool { self.type_lo == 0xEBD0_A0A2 }
}

const MAX_PARTS: usize = 8;
static mut PARTS: [Partition; MAX_PARTS] = [Partition {
    index: 0, first_lba: 0, last_lba: 0, type_lo: 0, name: [0; 36], name_len: 0,
}; MAX_PARTS];
static mut PART_COUNT: usize = 0;

/// Particiones leídas de la GPT.
pub fn partitions() -> &'static [Partition] {
    unsafe {
        let p = core::ptr::addr_of!(PARTS) as *const Partition;
        core::slice::from_raw_parts(p, PART_COUNT)
    }
}

/// Último LBA utilizable que declara la cabecera GPT (0 = sin leer).
pub fn last_lba() -> u64 { unsafe { LAST_LBA } }

fn le32(b: &[u8], o: usize) -> u32 {
    (b[o] as u32) | ((b[o+1] as u32) << 8) | ((b[o+2] as u32) << 16) | ((b[o+3] as u32) << 24)
}
fn le64(b: &[u8], o: usize) -> u64 {
    let mut v = 0u64;
    for i in (0..8).rev() { v = (v << 8) | b[o + i] as u64; }
    v
}

/// Lee la GPT del disco y guarda sus particiones. `true` si la cabecera es
/// válida (firma "EFI PART" en el LBA 1).
pub fn scan_partitions() -> bool {
    if !is_ready() { return false; }
    let mut sec = [0u8; SECTOR];

    // LBA 1: cabecera GPT.
    if read(1, 1, &mut sec) == 0 {
        crate::ring0::cabina::fault("disk", "no se pudo leer el LBA 1 (cabecera GPT)", 0);
        return false;
    }
    if &sec[0..8] != b"EFI PART" {
        crate::ring0::cabina::warn("disk", "el disco no tiene tabla GPT", 0);
        return false;
    }
    unsafe { LAST_LBA = le64(&sec, 48); }
    let entries_lba = le64(&sec, 72);
    let entry_count = le32(&sec, 80);
    let entry_size = le32(&sec, 84) as usize;
    if entry_size < 128 || entry_size > SECTOR {
        crate::ring0::cabina::warn("disk", "tamano de entrada GPT inesperado", entry_size as u64);
        return false;
    }

    let per_sector = SECTOR / entry_size;
    let mut found = 0usize;
    let mut i = 0u32;
    while i < entry_count && found < MAX_PARTS {
        let sector_index = (i as usize) / per_sector;
        if read(entries_lba + sector_index as u64, 1, &mut sec) == 0 { break; }
        let mut slot = (i as usize) % per_sector;
        while slot < per_sector && i < entry_count && found < MAX_PARTS {
            let o = slot * entry_size;
            let type_lo = le32(&sec, o);
            // Una entrada con GUID de tipo todo ceros es un hueco.
            let empty = type_lo == 0 && le32(&sec, o + 4) == 0
                && le32(&sec, o + 8) == 0 && le32(&sec, o + 12) == 0;
            if !empty {
                let mut p = Partition {
                    index: i + 1,
                    first_lba: le64(&sec, o + 32),
                    last_lba: le64(&sec, o + 40),
                    type_lo,
                    name: [0; 36],
                    name_len: 0,
                };
                // El nombre son 36 unidades UTF-16LE. Aquí solo se conserva
                // lo representable en ASCII: el font es de un byte y el
                // objetivo es reconocer "BMO", no renderizar cualquier idioma.
                let mut n = 0usize;
                for k in 0..36 {
                    let lo = sec[o + 56 + k * 2];
                    let hi = sec[o + 56 + k * 2 + 1];
                    if lo == 0 && hi == 0 { break; }
                    if hi == 0 && lo >= 0x20 && lo < 0x7F { p.name[n] = lo; n += 1; }
                }
                p.name_len = n;
                unsafe {
                    let arr = core::ptr::addr_of_mut!(PARTS) as *mut Partition;
                    core::ptr::write(arr.add(found), p);
                }
                found += 1;
            }
            slot += 1;
            i += 1;
        }
    }
    unsafe { PART_COUNT = found; }
    crate::ring0::cabina::info("disk", "particiones GPT leidas", found as u64);
    found > 0
}
