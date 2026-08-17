//! Disco: el puente entre Ring 0 y el driver AHCI/SATA.
//!
//! El kernel no sabe de puertos SATA ni de FIS: eso vive en `bmo-ahci`. Aqui
//! solo se le prestan al driver los tres servicios que no puede tener por su
//! cuenta (memoria DMA contigua, traduccion fisica->virtual y una salida de
//! log) y se le dice A QUE CONTROLADOR hablar.
//!
//! ## Por que AHCI y no NVMe
//!
//! Esta maquina tiene los dos. El primer controlador del barrido PCI es el
//! NVMe, y en el NVMe vive el Windows del dueno; el disco de BMO -- el que
//! lleva la particion de arranque y BMO-DATA -- cuelga de SATA. Pedir "el
//! primer disco" y escribir habria sido escribir en el sistema ajeno. Por eso
//! se pide el controlador POR TIPO, nunca por orden de aparicion.
//!
//! ## La escritura pasa por un gate, y el gate es codigo
//!
//! `bmo-ahci` sabe escribir sectores. Este puente solo abre esa puerta cuando
//! `verify_identity()` ha demostrado tres cosas: que el disco dijo QUIEN ES
//! (modelo y serie por IDENTIFY), que su tabla de particiones es coherente con
//! los sectores que el mismo declara, y que es un disco del que se arranca por
//! EFI. Hasta entonces `write()` devuelve 0 y dice por que.
//!
//! **Lo que el gate demuestra y lo que no.** Demuestra que este disco es un
//! disco de arranque EFI coherente consigo mismo, y que no se esta escribiendo
//! a ciegas en "el primero que aparecio" -- que era el peligro real, porque el
//! primero que aparece en esta maquina es el NVMe donde vive el Windows del
//! dueno. NO demuestra todavia que sea *este* disco y no otro igual: para eso
//! hace falta grabar la identidad DENTRO del volumen (el `disco_id` de
//! ESTRATOS) y compararla al montar. Mientras tanto la segunda linea de
//! defensa es la WINDOW: ningun sector fuera de una particion de datos
//! reconocida es escribible, y la particion de arranque EFI no lo es nunca.

use crate::ring0::mm::{self, phys};
use crate::ring0::dev::pci::{self, StorageKind};
use bmo_ahci::{storage_hal, StorageHal};

/// Tamano de sector de un disco SATA moderno visto por LBA de 512 B.

/// **THE IDENTITY GATE**: the eighty lines that decide whether this machine may
/// write to this disk at all. On the owner's machine the other drive holds
/// Windows, so this is the highest-consequence code in the driver.
mod irq;
mod ventana;
mod gate;
// ** Lo que el disco CONTESTA, empaquetado para salir por `OP_INFO`. El reparto
// de verdad vive FUERA del kernel, en cuatro generaciones (L7): `bmo-identify`
// para los hechos y `bmo-disco-juicio` para el veredicto. Aqui solo se pega.
mod perfil;
pub use perfil::{enlace, geometria, juicio, medio, trim_bloques_max};
/// ** DEVOLVER SECTORES AL DISCO. Destructivo, asi que pasa por los MISMOS
/// guardianes que escribir y por uno propio: lo que el aparato declaro.
mod trim;
pub use trim::{cuentas_trim, recortar, Recorte};
pub use gate::verify_identity;
/// WHO HOLDS THE DISK: one owner at a time, with a count of waits and thefts.
mod owner;
pub use owner::{cuentas_dueno, Testigo};
use owner::tomar_disco;
/// MOVING THE BYTES: read, DMA, and the bounce buffer -- both paths counted.
mod transfer;
pub use transfer::{cuentas_dma, read};

pub const SECTOR: usize = 512;

// -- Log del driver, linea a linea -------------------------------------------
// El driver escribe en fragmentos; se acumulan hasta el '\n' y se vuelca la
// linea entera al panel. Mismo patron que el puente USB: sin esto, en una
// placa sin cable serie el diagnostico del driver es invisible.

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
                        crate::ring0::core::dashboard::dashboard_log(line);
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

/// Lo que `bmo-ahci` necesita del kernel. Nada mas que esto.
struct KernelStorageHal;

impl StorageHal for KernelStorageHal {
    fn alloc_dma_pages(&self, count: usize) -> Option<u64> {
        // CONTIGUOS: la lista de comandos y las tablas de descriptores las
        // recorre el HBA por direccion fisica, linealmente. Dos frames que no
        // se tocan serian dos estructuras rotas.
        phys::alloc_frames_contig(count as u64)
    }
    fn free_dma_pages(&self, _addr: u64, _count: usize) {
        // El disco se abre una vez y vive lo que vive el kernel: no hay ciclo
        // de vida que liberar. Cuando lo haya, aqui va phys::free_frames.
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
    fn delay_ms(&self, ms: u64) {
        // Tiempo REAL por TSC. Los milisegundos del SATA son fisicos: contar
        // vueltas de bucle mide la velocidad del CPU, no el tiempo.
        let f = crate::ring0::task::scheduler::tsc_freq();
        if f == 0 {
            for _ in 0..ms * 2_000_000 { core::hint::spin_loop(); }
            return;
        }
        let end = crate::ring0::task::scheduler::rdtsc() + ms * (f / 1000);
        while crate::ring0::task::scheduler::rdtsc() < end { core::hint::spin_loop(); }
    }
}

static HAL: KernelStorageHal = KernelStorageHal;

// -- Estado ------------------------------------------------------------------

static mut READY: bool = false;
static mut PORT: u8 = 0xFF;
static mut MMIO: u64 = 0;
/// Sectores del disco, si la tabla de particiones lo declara.
static mut LAST_LBA: u64 = 0;

/// Pagina de rebote para el DMA. El HBA escribe SIEMPRE aqui, en una
/// direccion fisica conocida y contigua, y el kernel copia de aqui al buffer
/// del llamante. Asi ninguna capa de arriba necesita saber de direcciones
/// fisicas ni tener memoria apta para DMA.
static mut DMA_PHYS: u64 = 0;

/// Modelo y serie que el propio disco declara (IDENTIFY DEVICE).
static mut MODEL: [u8; 40] = [0; 40];
static mut MODEL_LEN: usize = 0;
/// Numero de serie (palabras 10..19 del IDENTIFY). El modelo dice que disco
/// ES; la serie dice CUAL de ellos. Dos Kingston del mismo modelo comparten
/// todo menos esto.
static mut SERIAL: [u8; 20] = [0; 20];
static mut SERIAL_LEN: usize = 0;
static mut TOTAL_SECTORS: u64 = 0;

// ** EL AVISO DEL DISCO VIVE EN `irq.rs` (paso 3 del plan).
//
// No salio por tamano: salio porque es **lo unico de este fichero que corre en
// contexto de interrupcion**, y mezclar eso con codigo que puede tomar candados
// es como se cuelga una maquina sin dejar rastro.
//
// Se re-exporta con los nombres de antes: el reparto no toca a los llamantes.
pub use irq::CLAVE_ESPERA;

/// Avisa el disco por su cuenta, y cuantas veces lo ha hecho.
pub fn irq_estado() -> (bool, u64) { irq::estado() }

/// **Lo llama el manejador del vector del disco.** Ver `plat/irq.rs`.
pub fn atender_irq() { irq::atender(unsafe { PORT }) }


/// Ha pasado el disco el gate de identidad? Mientras sea `false`, `write()`
/// no mueve un solo sector.
static mut WRITE_ARMED: bool = false;
/// Por que el gate dijo que si o que no. Un booleano no se puede fotografiar.
static mut GATE_REASON: &str = "sin comprobar";

/// Hay un disco listo para leer?
pub fn is_ready() -> bool { unsafe { READY } }
/// Puerto AHCI en uso (0xFF = ninguno).
pub fn port() -> u8 { unsafe { PORT } }
/// MMIO del HBA.
pub fn mmio() -> u64 { unsafe { MMIO } }
/// Modelo declarado por el disco. Vacio si aun no se le pregunto.
pub fn model() -> &'static str {
    unsafe {
        let p = core::ptr::addr_of!(MODEL) as *const u8;
        core::str::from_utf8(core::slice::from_raw_parts(p, MODEL_LEN)).unwrap_or("")
    }
}
/// Serie declarada por el disco. Vacia si aun no se le pregunto.
pub fn serial() -> &'static str {
    unsafe {
        let p = core::ptr::addr_of!(SERIAL) as *const u8;
        core::str::from_utf8(core::slice::from_raw_parts(p, SERIAL_LEN)).unwrap_or("")
    }
}
/// Sectores totales que declara el disco (IDENTIFY, LBA48).
pub fn total_sectors() -> u64 { unsafe { TOTAL_SECTORS } }
/// Esta abierta la puerta de escritura?
pub fn write_armed() -> bool { unsafe { WRITE_ARMED } }

// ** LAS VENTANAS DE ESCRITURA VIVEN EN `ventana.rs` (paso 2 del plan).
//
// Eran politica pura --una decision sobre rangos de LBA-- mezclada con los
// registros del HBA. Se re-exportan para no tocar a ningun llamante.
pub use ventana::{armar_ventana_estratos, desarmar_ventana_estratos, ventana_estratos};
/// Que dictamino el gate de identidad, en palabras.
pub fn gate_reason() -> &'static str { unsafe { GATE_REASON } }

/// Despierta el disco de BMO: busca el HBA **SATA** por PCI, lo prepara y deja
/// listo el primer puerto con un disco de verdad conectado.
pub fn init() {
    storage_hal::init_hal(&HAL);

    // Una placa puede traer mas de un HBA SATA. Se prueban en orden hasta dar
    // con uno que tenga un disco enlazado -- el mismo patron que el USB, que ya
    // nos enseno que el teclado estaba en el segundo controlador.
    let mut chosen = 0xFFu8;
    let mut loc_ok = None;
    // AHCI primero; si ninguno tiene disco, se prueba el que la BIOS declare
    // en modo RAID. Muchos controladores AMD en "modo RAID" siguen hablando
    // AHCI por registros -- y si no lo hacen, el censo lo dira con sus propios
    // numeros en vez de dejarnos suponiendo que el disco no existe.
    'busqueda: for kind in [StorageKind::Ahci, StorageKind::Raid] {
    for skip in 0..4usize {
        let loc = match pci::find_storage_of(kind, skip) {
            Some(l) => l,
            None => break,
        };
        if kind == StorageKind::Raid {
            crate::ring0::cabina::warn("disk", "probando un controlador en modo RAID como AHCI", loc.mmio);
        }
        // * SIEMPRE por el physmap, nunca por la identidad.
        //
        // La identidad 0..4 GiB que monta s2_mem vive en PML4[0], y un espacio
        // de Ring 3 solo hereda de ahi la PRIMERA entrada del PDPT -- el primer
        // GiB (ver `vmm::new_address_space`, y no puede heredar mas: la imagen
        // del proceso vive justo en PDPT[1], `USER_IMAGE_BASE = 0x4000_0000`).
        //
        // El ABAR del AHCI cae en `0xFC68_0000`, o sea PDPT[3]. Bajo el CR3 de
        // un proceso esa entrada NO EXISTE, asi que cualquier syscall de Ring 3
        // que llegara al disco hacia **#PF en Ring 0** dentro de
        // `bmo_ahci::controller::run_command`. Desde el shell de Ring 0 no
        // pasaba --ahi manda el PML4 del kernel-- y por eso parecia funcionar.
        //
        // El physmap (0..16 GiB en `HIGH_MEM_BASE`) esta en el MEDIO-KERNEL,
        // que todo espacio comparte por construccion. Misma memoria, mismo
        // cache, y alcanzable bajo cualquier CR3.
        let mmio_va = mm::phys_to_virt(loc.mmio);
        crate::ring0::cabina::info("disk", "HBA SATA/AHCI hallado en PCI", loc.mmio);

        bmo_ahci::reset_ctrl();
        if !unsafe { bmo_ahci::probe(mmio_va) } {
            crate::ring0::cabina::warn("disk", "el HBA no inicializo, probando el siguiente", loc.mmio);
            continue;
        }

        let ctrl = match bmo_ahci::controller() { Some(c) => c, None => continue };
        // El estado crudo de cada puerto lo pinta el driver (`probe`). Aqui
        // solo se cuenta y se elige.
        //
        // * Se itera por INDICE, no por `p.port_number`. Las entries vacias
        // del array llevan port_number = 0, asi que filtrar por su campo hacia
        // que CADA hueco se colara haciendose pasar por el puerto 0: catorce
        // lineas identicas del mismo puerto inexistente, y una espera de
        // enlace completa concedida a cada fantasma (los 3-4 segundos de
        // arranque). El indice del array ES el numero de puerto; el campo solo
        // significa algo en las entries que `probe` lleno.
        crate::ring0::cabina::info("ahci", "puertos implementados (PI) segun el firmware", ctrl.ports_implemented as u64);
        let mut active = 0u64;
        // TODOS los puertos que CAP declara, no solo los que PI reconoce: el
        // firmware puede estar ocultando justo el que lleva el disco.
        for i in 0..(ctrl.port_count as usize).min(32) {
            let p = &ctrl.ports[i];
            // Un puerto que ni siquiera acepta escrituras (cmd sigue en 0 tras
            // pedirle spin-up) no existe fisicamente: no merece una linea.
            let declared = ctrl.ports_implemented & (1 << i) != 0;
            if !declared && p.cmd == 0 && p.ssts == 0 { continue; }
            // El estado de cada puerto va a la BITACORA, no solo al log
            // rodante: un numero que se lleva el desplazamiento antes de que
            // lo fotografies es un numero que no existe. El valor es el PxSSTS
            // crudo -- su digito bajo (DET) es 3 si el enlace esta vivo.
            let msg = match p.ssts & 0xF {
                0x3 => "puerto con enlace vivo (DET=3)",
                0x1 => "puerto con dispositivo pero SIN enlace (DET=1)",
                _ => "puerto vacio (DET=0)",
            };
            if p.ssts & 0xF == 0x3 {
                crate::ring0::cabina::info("ahci", msg, p.ssts as u64);
            } else {
                crate::ring0::cabina::warn("ahci", msg, p.ssts as u64);
            }
            if p.state == bmo_ahci::PortState::Active {
                active += 1;
                // Firma 0x00000101 = disco duro SATA. Un ATAPI (0xEB140101) es
                // una unidad optica: no es donde vive BMO.
                if chosen == 0xFF && p.signature == bmo_ahci::SIG_SATA_DISK {
                    chosen = i as u8;
                }
            }
        }
        crate::ring0::cabina::info("disk", "puertos SATA con disco enlazado", active);
        if chosen != 0xFF {
            unsafe { MMIO = loc.mmio; }
            loc_ok = Some(loc);
            break 'busqueda;
        }
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
    // Pagina de rebote para el DMA, contigua y de direccion fisica conocida.
    let dma = match phys::alloc_frames_contig(1) {
        Some(p) => p,
        None => {
            crate::ring0::cabina::fault("disk", "sin memoria para el buffer DMA", 0);
            return;
        }
    };
    unsafe { DMA_PHYS = dma; PORT = chosen; READY = true; }
    crate::ring0::cabina::info("disk", "puerto SATA listo para leer", chosen as u64);

    // -- ** QUE EL DISCO AVISE, si la placa deja --
    //
    // El orden es el unico posible y por eso va comentado: primero se le dice al
    // aparato **a donde** mandar el aviso (MSI), y solo si eso quedo armado se
    // le dice **que avise** (`GHC.IE`). Al reves, el disco anunciaria a una
    // direccion que no escucha nadie y se quedaria esperando respuesta.
    //
    // Y si no hay MSI no pasa nada: el driver sigue preguntando por MMIO como
    // toda la vida. Ver la red de seguridad de `run_command`.
    if let Some(loc) = loc_ok.as_ref() {
        let idt = crate::info::idt_ptr();
        let vector = crate::ring0::plat::irq::VECTOR_DISCO as u8;
        if idt != 0 && crate::ring0::plat::irq::instalar(idt) {
            if crate::ring0::dev::pci::msi_activar(loc.bus, loc.dev, loc.func, vector, 0) {
                if unsafe { bmo_ahci::habilitar_irq(chosen) } {
                    irq::marcar_armada();
                    crate::ring0::cabina::info("disk", "el disco avisa por MSI, vector", vector as u64);
                }
            } else {
                crate::ring0::cabina::warn("disk", "el HBA no anuncia MSI: se sigue preguntando", 0);
            }
        }
    }

    identify();

    // A partir de aqui este disco ES el dispositivo de bloques de BMO. Se
    // registra UNO, y el que elige cual es el kernel mirando el tipo de
    // controlador -- nunca un bucle sobre una lista, que es como se acaba
    // escribiendo en el disco del vecino.
    bmo_block::register(&AHCI_DISK);
    crate::ring0::cabina::info("disk", "contrato de bloques registrado", unsafe { TOTAL_SECTORS });
}

/// Extrae una cadena del buffer de IDENTIFY, de la palabra `first` a `last`.
///
/// Las cadenas de IDENTIFY vienen en palabras de 16 bits con los dos bytes AL
/// REVES (convencion ATA de toda la vida): el caracter que va PRIMERO esta en
/// el byte ALTO de la palabra. Leerlas en orden de memoria da "IKGNTSMOS"
/// donde pone "KINGSTON" -- cada par cambiado. Se emite primero el byte alto
/// (offset impar en little-endian) y despues el bajo.
///
/// Devuelve la longitud util, ya sin el relleno de espacios del final.
unsafe fn ata_string(src: *const u8, first: usize, last: usize, dst: &mut [u8]) -> usize {
    let mut n = 0usize;
    for w in first..last {
        let hi = src.add(w * 2 + 1).read_volatile();
        let lo = src.add(w * 2).read_volatile();
        for c in [hi, lo] {
            if n < dst.len() && c >= 0x20 && c < 0x7F { dst[n] = c; n += 1; }
        }
    }
    while n > 0 && dst[n - 1] == b' ' { n -= 1; }
    n
}

/// Le pregunta al disco QUIEN ES.
///
/// Esta maquina tiene tres discos y en uno vive el sistema del dueno. Un
/// kernel que va a escribir algun dia tiene que poder decir "estoy hablando
/// con el Kingston de 480 GB", no "estoy hablando con el primero que salio".
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
    unsafe {
        // Palabras 27..46: modelo. Palabras 10..19: numero de serie.
        MODEL_LEN = ata_string(src, 27, 47, &mut *core::ptr::addr_of_mut!(MODEL));
        SERIAL_LEN = ata_string(src, 10, 20, &mut *core::ptr::addr_of_mut!(SERIAL));
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

    // ** Y AQUI SE LE PREGUNTA AL DISCO LO QUE NUNCA SE LE HABIA PREGUNTADO.
    //
    // El sector ya esta leido: las palabras 217 (gira o no), 169 (TRIM), 106 y
    // 209 (geometria) y 75/76/77 (cola y cable) estaban **en este mismo buffer**
    // desde el primer dia, y nadie las miraba. Ver R-DISCO6 y el capitulo
    // `docs/componente/EL_DISCO_EXIGE.md`.
    //
    // Se le pasa el sector entero y no palabras sueltas a proposito: decidir
    // que palabras importan es del padre, y este fichero no puede empezar a
    // interpretar sin romper el reparto.
    let sector = unsafe { core::slice::from_raw_parts(src, 512) };
    perfil::tomar_foto(sector);
    // Y se dice en el arranque, con su cifra: el medio es lo primero que decide
    // como se le escribe a este aparato.
    let (que_es, cifra) = perfil::medio_en_palabras();
    crate::ring0::cabina::info("disk", que_es, cifra);
}

// -- ** EL DISCO TIENE UN DUENO CADA VEZ ------------------------------------
//
// === El fallo que esto tapa, y no era teorico ===
//
// El HBA tiene 32 ranuras de comando y este driver usa **la 0**, siempre. Un
// comando "en vuelo" es un estado global del puerto: la tabla de comando, el
// PRDT y el `PRDBC` de la cabecera son UNOS.
//
// Y el temporizador **expropia**. O sea que la secuencia armar -> campana ->
// esperar -> leer `PRDBC` se puede partir por la mitad en cualquier punto, y
// otra tarea puede entrar por otro camino: hoy llegan aqui **FAT32** (los
// `.bex`, la GPT) y **ESTRATOS** (por `bmo_block`), y encima desde Ring 3
// cualquiera que abra un archivo. Dos lecturas solapadas escriben la misma
// ranura, y la primera acaba leyendo el `PRDBC` de la segunda -- **sectores del
// sitio equivocado, sin que nada falle**.
//
// === Por que un DUENO y no un cerrojo ===
//
// Un `SpinLock` giraria con el planificador expropiando por debajo, y quien lo
// tomara y muriera lo dejaria tomado para siempre. Aqui se apunta **quien** lo
// tiene: si el que espera ve que el dueno ya no existe, lo toma y **lo dice**.
// Un candado que se puede quedar cerrado sin que nadie sepa por que es peor que
// la corrupcion que evita.
//
// Es la misma idea que la pantalla: exclusiva, con dueno, y recuperable cuando
// el dueno se muere.

use core::sync::atomic::{AtomicU32, Ordering};

/// La particion de datos ELEGIDA, una vez que alguien la ha identificado.
///
/// `None` hasta que `fsys::fs::mount_data` consigue montar una y la fija aqui.
static mut PART_DATOS: Option<Partition> = None;

/// **Fija cual es la particion de datos.** La llama quien lo ha DEMOSTRADO.
///
/// El unico que puede demostrarlo es el sistema de ficheros: montarla es la
/// prueba. Este modulo sabe de sectores y de GUIDs, no de FAT32.
///
/// [!] Y es lo que hace que la ventana de escritura y el volumen montado hablen
/// de la MISMA particion. Mientras cada uno la elegia por su cuenta con la misma
/// heuristica, coincidian por casualidad; si un dia dejaran de coincidir, el
/// sistema estaria leyendo de una y protegiendo la otra.
pub fn fijar_particion_datos(p: Partition) {
    unsafe { PART_DATOS = Some(p) };
}

/// La particion donde BMO puede escribir.
///
/// == ** POR LO QUE ES, NUNCA POR DONDE ESTA (2026-08-11) ==
///
/// Esto era `partitions().iter().find(|p| !p.is_esp())` -- **la primera que no
/// es la de arranque**. Y eso rompe la regla que este mismo fichero declara en su
/// cabecera, cuatro parrafos mas arriba:
///
/// > *"Pedir 'el primer disco' y escribir habria sido escribir en el sistema
/// > ajeno. Por eso se pide el controlador POR TIPO, nunca por orden de
/// > aparicion."*
///
/// La leccion se aprendio para el CONTROLADOR y se siguio incumpliendo una capa
/// mas abajo, con la PARTICION. Y el disco de esta maquina tiene tres que no son
/// la de arranque: la FAT32 de BMO y la de ESTRATOS. "La primera" era una
/// moneda al aire que dependia del orden de la GPT.
///
/// Ahora la elige quien puede demostrarlo --el que consigue montarla-- y aqui
/// solo se recuerda. Mientras nadie lo haya demostrado, esto contesta `None`:
/// **no saber cual es se dice, no se adivina.**
pub fn data_partition() -> Option<Partition> {
    unsafe { PART_DATOS }
}

/// Puede escribirse el rango `[lba, lba+count)`? Devuelve el motivo si no.
///
/// Es la segunda linea de defensa, independiente del gate: aunque la identidad
/// este armada, un sector fuera de la particion de datos sigue siendo
/// intocable. Los dos fallos que esto ataja son el desbordamiento aritmetico
/// de un calculo de LBA y un sistema de ficheros que se cree en otro sitio.
/// Recoge el estado y deja que `ventana::decidir` juzgue.
///
/// La division no es cosmetica: aqui se leen cuatro globales --y por eso esta
/// funcion no se puede probar-- y alli se decide con lo que llegue por
/// parametro, que es lo que hace que sus siete casillas existan.
fn write_window(lba: u64, count: u16) -> Result<(), &'static str> {
    // La particion se pasa como RANGO y no como `Partition`: el contrato no
    // tiene por que saber que existe una tabla de particiones, y asi
    // `bmo-block` no depende de `bmo-particiones`.
    bmo_block::ventana::decidir(
        is_ready(),
        write_armed(),
        data_partition().map(|w| (w.first_lba, w.last_lba)),
        ventana_estratos(),
        lba,
        count,
    )
}

/// Escribe `count` sectores en `lba`. Devuelve los sectores escritos.
///
/// Espejo de `read`, con dos guardias delante: el gate de identidad y la
/// ventana. Un rechazo NUNCA es mudo -- dice cual de las dos lo paro y donde.
pub fn write(lba: u64, count: u16, data: &[u8]) -> u16 {
    if let Err(why) = write_window(lba, count) {
        crate::ring0::cabina::fault("disk", why, lba);
        return 0;
    }
    if data.len() < count as usize * SECTOR {
        crate::ring0::cabina::fault("disk", "menos datos que sectores pedidos", count as u64);
        return 0;
    }
    let dma = unsafe { DMA_PHYS };
    if dma == 0 { return 0; }
    // Igual que en `read`, y aqui todavia mas: dos escrituras que se pisan la
    // ranura no dan un dato malo, dan un SECTOR malo en el disco.
    let _testigo = tomar_disco();

    const PER_BATCH: u16 = (4096 / SECTOR) as u16; // 8 sectores por pagina
    let mut done = 0u16;
    while done < count {
        let batch = (count - done).min(PER_BATCH);
        // A la pagina de rebote primero: el HBA solo lee de memoria fisica
        // contigua y conocida, no del buffer que traiga el llamante.
        let dst = mm::phys_to_virt(dma) as *mut u8;
        let src_off = done as usize * SECTOR;
        let n = batch as usize * SECTOR;
        unsafe { core::ptr::copy_nonoverlapping(data.as_ptr().add(src_off), dst, n); }
        let put = match unsafe { bmo_ahci::write_sectors_phys(unsafe { PORT }, lba + done as u64, batch, dma) } {
            Ok(n) => n,
            Err(e) => {
                crate::ring0::cabina::fault("disk", e.name(), lba + done as u64);
                return done;
            }
        };
        if put == 0 { return done; }
        done += put;
        if put < batch { break; } // escritura corta: el disco dijo basta
    }
    done
}

/// Obliga al disco a bajar a la superficie lo que acepto.
///
/// Una escritura que devuelve OK solo promete que el disco se quedo con los
/// bytes. La caja negra existe para sobrevivir al corte que se investiga: sin
/// esto, el registro del vuelo se pierde justo en el accidente.
pub fn flush() -> bool {
    if !is_ready() || !write_armed() { return false; }
    let _testigo = tomar_disco();
    match unsafe { bmo_ahci::flush_cache(unsafe { PORT }) } {
        Ok(()) => true,
        Err(e) => {
            crate::ring0::cabina::fault("disk", e.name(), 0);
            false
        }
    }
}

// -- GPT: la tabla de particiones --------------------------------------------
//
// Leerla es como BMO reconoce el disco que tiene delante. No hace falta
// confiar en el orden del PCI ni en que el firmware enumere igual dos veces:
// el disco propio es el que lleva estas particiones y no otras.

/// **La particion vive en `bmo-particiones`**, no aqui.
///
/// Se fue en el paso 1 de `docs/plan/PLAN_ALMACENAMIENTO.md`. El criterio: de las
/// siete preguntas que respondia este fichero, dos **no tocan hardware**, y
/// "donde estan las cosas" es una de ellas -- leer una GPT es leer un formato
/// ajeno, igual que una cabecera BEF.
///
/// Lo que se gano al sacarlo: **un censo de siete casillas que corre en 0
/// segundos y sin disco** (tablas escritas a mano, incluida una con el
/// `entry_size` corrupto que haria leer las entradas en diagonal). Con el
/// bucle de lectura dentro, ninguna de esas casillas se podia escribir sin
/// arrancar la maquina.
///
/// Se re-exporta para que quien ya decia `disk::Partition` siga diciendolo:
/// el reparto no es excusa para tocar a los llamantes.
pub use bmo_particiones::Partition;

const MAX_PARTS: usize = 8;
static mut PARTS: [Partition; MAX_PARTS] = [Partition::VACIA; MAX_PARTS];
static mut PART_COUNT: usize = 0;

/// Particiones leidas de la GPT.
pub fn partitions() -> &'static [Partition] {
    unsafe {
        let p = core::ptr::addr_of!(PARTS) as *const Partition;
        core::slice::from_raw_parts(p, PART_COUNT)
    }
}

/// Ultimo LBA utilizable que declara la cabecera GPT (0 = sin leer).
pub fn last_lba() -> u64 { unsafe { LAST_LBA } }

// -- El contrato de bloques, implementado ------------------------------------
//
// `bmo-block` declara la forma (leer / escribir / capacidad / identidad); aqui
// se dice que el AHCI de esta maquina la cumple. Es el paso 3 del orden de
// construccion de ESTRATOS: a partir de ahora, lo que hay por encima habla con
// un dispositivo de bloques y no con SATA.

/// El disco SATA de BMO visto como dispositivo de bloques.
///
/// Sin campos: el estado vive en los estaticos del modulo porque hay UN disco
/// y lo hay durante toda la vida del kernel. Un struct vacio es lo que permite
/// tener un `&'static dyn BlockDevice` sin reservar memoria, que en Ring 0 no
/// se puede.
pub struct AhciDisk;
static AHCI_DISK: AhciDisk = AhciDisk;

impl bmo_block::BlockDevice for AhciDisk {
    fn identity(&self) -> bmo_block::DeviceId {
        let mut id = bmo_block::DeviceId::EMPTY;
        unsafe {
            // Slices pedidos explicitamente: tomar `&(*addr_of!(ARR))[..n]`
            // crea una referencia a la desreferencia de un puntero crudo, que
            // es justo lo que el compilador rechaza sobre un `static mut`.
            id.model_len = MODEL_LEN.min(id.model.len());
            let m = core::slice::from_raw_parts(core::ptr::addr_of!(MODEL) as *const u8, id.model_len);
            id.model[..id.model_len].copy_from_slice(m);
            id.serial_len = SERIAL_LEN.min(id.serial.len());
            let s = core::slice::from_raw_parts(core::ptr::addr_of!(SERIAL) as *const u8, id.serial_len);
            id.serial[..id.serial_len].copy_from_slice(s);
            id.blocks = TOTAL_SECTORS;
        }
        id.block_size = SECTOR as u32;
        id
    }

    fn read(&self, lba: u64, count: u16, buf: &mut [u8]) -> Result<u16, bmo_block::BlockError> {
        use bmo_block::BlockError as E;
        if !is_ready() { return Err(E::NotReady); }
        if count == 0 { return Ok(0); }
        if buf.len() < count as usize * SECTOR { return Err(E::ShortBuffer); }
        let end = lba.checked_add(count as u64).ok_or(E::OutOfRange)?;
        let total = unsafe { TOTAL_SECTORS };
        if total != 0 && end > total { return Err(E::OutOfRange); }
        // `read` sin `self.` es la funcion libre del modulo, no este metodo.
        match read(lba, count, buf) {
            0 => Err(E::Device),
            n => Ok(n),
        }
    }

    fn write(&self, lba: u64, count: u16, data: &[u8]) -> Result<u16, bmo_block::BlockError> {
        use bmo_block::BlockError as E;
        if !is_ready() { return Err(E::NotReady); }
        // El gate primero y con su propio codigo: "no me han autorizado a
        // escribir" no es lo mismo que "el disco fallo", y quien llama va a
        // hacer cosas distintas con cada respuesta.
        if !write_armed() { return Err(E::ReadOnly); }
        if count == 0 { return Ok(0); }
        if data.len() < count as usize * SECTOR { return Err(E::ShortBuffer); }
        let end = lba.checked_add(count as u64).ok_or(E::OutOfRange)?;
        let total = unsafe { TOTAL_SECTORS };
        if total != 0 && end > total { return Err(E::OutOfRange); }
        // La WINDOW sigue mandando por debajo: el contrato de bloques no la
        // sustituye. Un rango fuera de la particion de datos se rechaza aqui
        // como ReadOnly, que es exactamente lo que es para ese sector.
        if write_window(lba, count).is_err() { return Err(E::ReadOnly); }
        match write(lba, count, data) {
            0 => Err(E::Device),
            n => Ok(n),
        }
    }

    fn flush(&self) -> Result<(), bmo_block::BlockError> {
        if !is_ready() { return Err(bmo_block::BlockError::NotReady); }
        if flush() { Ok(()) } else { Err(bmo_block::BlockError::Device) }
    }

    fn writable(&self) -> bool { write_armed() }
}

// -- Las funciones sueltas que consume bmo-fat32 -----------------------------
//
// `bmo-fat32` recibe punteros a funcion, no un objeto de trait, y se queda
// asi: cambiarlo seria tocar un sistema de ficheros que YA funciona en
// hardware para no ganar nada hoy. Cuando ESTRATOS llegue, hablara con
// `bmo_block::device()` directamente.

/// Lector de bloques del disco montado. LBA ABSOLUTO del dispositivo.
pub fn block_read(lba: u64, count: u16, buf: &mut [u8]) -> bool {
    if count == 0 || buf.len() < count as usize * SECTOR { return false; }
    read(lba, count, buf) == count
}

/// Escritor de bloques. LBA ABSOLUTO del dispositivo.
///
/// La otra mitad del contrato. Se le entrega a `bmo-fat32` **solo** para el
/// volumen de datos: al de arranque se le sigue montando con `None`, y asi su
/// inmutabilidad no depende de que nadie se equivoque, sino de que no exista
/// la funcion.
pub fn block_write(lba: u64, count: u16, data: &[u8]) -> bool {
    if count == 0 || data.len() < count as usize * SECTOR { return false; }
    write(lba, count, data) == count
}


/// Lee la GPT del disco y guarda sus particiones. `true` si la cabecera es
/// valida (firma "EFI PART" en el LBA 1).
pub fn scan_partitions() -> bool {
    if !is_ready() { return false; }
    let mut sec = [0u8; SECTOR];

    // ** EL BUCLE SE QUEDA AQUI Y EL PARSEO SE VA, y esa es la frontera.
    //
    // Lo que solo puede hacerse aqui es pedir sectores (hay un dispositivo) y
    // escribir en CABINA (hay un kernel). Interpretar los bytes no necesita ni
    // una cosa ni la otra, asi que vive en `bmo-particiones` con su censo.
    if read(1, 1, &mut sec) == 0 {
        crate::ring0::cabina::fault("disk", "no se pudo leer el LBA 1 (cabecera GPT)", 0);
        return false;
    }
    let gpt = match bmo_particiones::cabecera(&sec) {
        Ok(g) => g,
        Err(e) => {
            // El motivo con nombre: "no hay GPT" es normal en un disco ajeno,
            // y "el tamano de entrada es absurdo" es un disco roto. Antes los
            // dos salian como el mismo `false`.
            crate::ring0::cabina::warn("disk", e.name(), 0);
            return false;
        }
    };
    unsafe { LAST_LBA = gpt.last_lba; }

    let per_sector = gpt.por_sector();
    let mut found = 0usize;
    let mut i = 0u32;
    while i < gpt.entry_count && found < MAX_PARTS {
        let sector_index = (i as usize) / per_sector;
        if read(gpt.entries_lba + sector_index as u64, 1, &mut sec) == 0 { break; }
        let mut slot = (i as usize) % per_sector;
        while slot < per_sector && i < gpt.entry_count && found < MAX_PARTS {
            if let Some(part) = bmo_particiones::entrada(&sec, slot * gpt.entry_size as usize, i + 1) {
                unsafe {
                    let arr = core::ptr::addr_of_mut!(PARTS) as *mut Partition;
                    core::ptr::write(arr.add(found), part);
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
