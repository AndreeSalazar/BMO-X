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
//! ## La escritura pasa por un gate, y el gate es código
//!
//! `bmo-ahci` sabe escribir sectores. Este puente solo abre esa puerta cuando
//! `verify_identity()` ha demostrado tres cosas: que el disco dijo QUIÉN ES
//! (modelo y serie por IDENTIFY), que su tabla de particiones es coherente con
//! los sectores que él mismo declara, y que es un disco del que se arranca por
//! EFI. Hasta entonces `write()` devuelve 0 y dice por qué.
//!
//! **Lo que el gate demuestra y lo que no.** Demuestra que este disco es un
//! disco de arranque EFI coherente consigo mismo, y que no se está escribiendo
//! a ciegas en "el primero que apareció" — que era el peligro real, porque el
//! primero que aparece en esta máquina es el NVMe donde vive el Windows del
//! dueño. NO demuestra todavía que sea *este* disco y no otro igual: para eso
//! hace falta grabar la identidad DENTRO del volumen (el `disco_id` de
//! ESTRATOS) y compararla al montar. Mientras tanto la segunda línea de
//! defensa es la VENTANA: ningún sector fuera de una partición de datos
//! reconocida es escribible, y la partición de arranque EFI no lo es nunca.

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
    fn delay_ms(&self, ms: u64) {
        // Tiempo REAL por TSC. Los milisegundos del SATA son físicos: contar
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
/// Número de serie (palabras 10..19 del IDENTIFY). El modelo dice qué disco
/// ES; la serie dice CUÁL de ellos. Dos Kingston del mismo modelo comparten
/// todo menos esto.
static mut SERIAL: [u8; 20] = [0; 20];
static mut SERIAL_LEN: usize = 0;
static mut TOTAL_SECTORS: u64 = 0;

/// ¿Ha pasado el disco el gate de identidad? Mientras sea `false`, `write()`
/// no mueve un solo sector.
static mut WRITE_ARMED: bool = false;
/// Por qué el gate dijo que sí o que no. Un booleano no se puede fotografiar.
static mut GATE_REASON: &str = "sin comprobar";

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
/// Serie declarada por el disco. Vacía si aún no se le preguntó.
pub fn serial() -> &'static str {
    unsafe {
        let p = core::ptr::addr_of!(SERIAL) as *const u8;
        core::str::from_utf8(core::slice::from_raw_parts(p, SERIAL_LEN)).unwrap_or("")
    }
}
/// Sectores totales que declara el disco (IDENTIFY, LBA48).
pub fn total_sectors() -> u64 { unsafe { TOTAL_SECTORS } }
/// ¿Está abierta la puerta de escritura?
pub fn write_armed() -> bool { unsafe { WRITE_ARMED } }

/// ★ **La segunda ventana de escritura: la de ESTRATOS.**
///
/// `write_window` sólo admitía la partición de datos (la FAT32 donde viven los
/// `.bex`), y eso era correcto mientras ESTRATOS no escribiera. Pero ESTRATOS
/// **vive en otra partición**, así que su primera escritura habría sido
/// rechazada con `fuera de la particion de datos` — un mensaje correcto y
/// desconcertante.
///
/// La tentación es ensanchar la ventana existente. **No.** Ensanchar un
/// guardián es quitarlo: la ventana de datos protege de que un bug del sistema
/// de ficheros se coma la ESP —donde vive el `BOOTX64.EFI` con el que arrancó
/// la máquina, y en una máquina con Windows también el cargador del dueño— y
/// eso tiene que seguir protegido aunque ESTRATOS escriba.
///
/// Así que hay DOS ventanas con nombre propio, y ésta se registra sólo cuando
/// se cumplen las dos condiciones: el volumen está montado **y el gate de
/// identidad del §5 dijo que nació en este disco**. Un volumen clonado no
/// registra ventana y por tanto **no puede escribir**, aunque el disco esté
/// armado.
static mut VENTANA_ES: Option<(u64, u64)> = None;

/// Registra la ventana de ESTRATOS. La llama `fsys::estratos::mount`.
///
/// # Safety
/// Se llama una vez al montar, con las interrupciones donde ya están.
pub fn armar_ventana_estratos(primer_lba: u64, ultimo_lba: u64) {
    unsafe { VENTANA_ES = Some((primer_lba, ultimo_lba)) };
    crate::ring0::cabina::info("estratos", "ventana de escritura armada en", primer_lba);
}

/// Quita la ventana. Se llama al empezar a montar: si el montaje falla o la
/// identidad no cuadra, **no queda ventana de la vez anterior**.
pub fn desarmar_ventana_estratos() {
    unsafe { VENTANA_ES = None };
}

pub fn ventana_estratos() -> Option<(u64, u64)> { unsafe { VENTANA_ES } }
/// Qué dictaminó el gate de identidad, en palabras.
pub fn gate_reason() -> &'static str { unsafe { GATE_REASON } }

/// Despierta el disco de BMO: busca el HBA **SATA** por PCI, lo prepara y deja
/// listo el primer puerto con un disco de verdad conectado.
pub fn init() {
    storage_hal::init_hal(&HAL);

    // Una placa puede traer más de un HBA SATA. Se prueban en orden hasta dar
    // con uno que tenga un disco enlazado — el mismo patrón que el USB, que ya
    // nos enseñó que el teclado estaba en el segundo controlador.
    let mut chosen = 0xFFu8;
    let mut loc_ok = None;
    // AHCI primero; si ninguno tiene disco, se prueba el que la BIOS declare
    // en modo RAID. Muchos controladores AMD en "modo RAID" siguen hablando
    // AHCI por registros — y si no lo hacen, el censo lo dirá con sus propios
    // números en vez de dejarnos suponiendo que el disco no existe.
    'busqueda: for kind in [StorageKind::Ahci, StorageKind::Raid] {
    for skip in 0..4usize {
        let loc = match pci::find_storage_of(kind, skip) {
            Some(l) => l,
            None => break,
        };
        if kind == StorageKind::Raid {
            crate::ring0::cabina::warn("disk", "probando un controlador en modo RAID como AHCI", loc.mmio);
        }
        // ★ SIEMPRE por el physmap, nunca por la identidad.
        //
        // La identidad 0..4 GiB que monta s2_mem vive en PML4[0], y un espacio
        // de Ring 3 sólo hereda de ahí la PRIMERA entrada del PDPT — el primer
        // GiB (ver `vmm::new_address_space`, y no puede heredar más: la imagen
        // del proceso vive justo en PDPT[1], `USER_IMAGE_BASE = 0x4000_0000`).
        //
        // El ABAR del AHCI cae en `0xFC68_0000`, o sea PDPT[3]. Bajo el CR3 de
        // un proceso esa entrada NO EXISTE, así que cualquier syscall de Ring 3
        // que llegara al disco hacía **#PF en Ring 0** dentro de
        // `bmo_ahci::controller::run_command`. Desde el shell de Ring 0 no
        // pasaba —ahí manda el PML4 del kernel— y por eso parecía funcionar.
        //
        // El physmap (0..16 GiB en `HIGH_MEM_BASE`) está en el MEDIO-KERNEL,
        // que todo espacio comparte por construcción. Misma memoria, mismo
        // caché, y alcanzable bajo cualquier CR3.
        let mmio_va = mm::phys_to_virt(loc.mmio);
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
        crate::ring0::cabina::info("ahci", "puertos implementados (PI) segun el firmware", ctrl.ports_implemented as u64);
        let mut active = 0u64;
        // TODOS los puertos que CAP declara, no solo los que PI reconoce: el
        // firmware puede estar ocultando justo el que lleva el disco.
        for i in 0..(ctrl.port_count as usize).min(32) {
            let p = &ctrl.ports[i];
            // Un puerto que ni siquiera acepta escrituras (cmd sigue en 0 tras
            // pedirle spin-up) no existe físicamente: no merece una línea.
            let declared = ctrl.ports_implemented & (1 << i) != 0;
            if !declared && p.cmd == 0 && p.ssts == 0 { continue; }
            // El estado de cada puerto va a la BITÁCORA, no solo al log
            // rodante: un número que se lleva el desplazamiento antes de que
            // lo fotografíes es un número que no existe. El valor es el PxSSTS
            // crudo — su dígito bajo (DET) es 3 si el enlace está vivo.
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

    // A partir de aquí este disco ES el dispositivo de bloques de BMO. Se
    // registra UNO, y el que elige cuál es el kernel mirando el tipo de
    // controlador — nunca un bucle sobre una lista, que es como se acaba
    // escribiendo en el disco del vecino.
    bmo_block::register(&AHCI_DISK);
    crate::ring0::cabina::info("disk", "contrato de bloques registrado", unsafe { TOTAL_SECTORS });
}

/// Extrae una cadena del buffer de IDENTIFY, de la palabra `first` a `last`.
///
/// Las cadenas de IDENTIFY vienen en palabras de 16 bits con los dos bytes AL
/// REVÉS (convención ATA de toda la vida): el carácter que va PRIMERO está en
/// el byte ALTO de la palabra. Leerlas en orden de memoria da "IKGNTSMOS"
/// donde pone "KINGSTON" — cada par cambiado. Se emite primero el byte alto
/// (offset impar en little-endian) y después el bajo.
///
/// Devuelve la longitud útil, ya sin el relleno de espacios del final.
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
    unsafe {
        // Palabras 27..46: modelo. Palabras 10..19: número de serie.
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
}

/// Lee `count` sectores desde `lba` en `buf`. Devuelve los sectores leídos.
///
/// La lectura no tiene ventana ni gate: mirar un sector no rompe nada, y es
/// justo mirando como BMO averigua de quién es el disco.
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

// ── El gate de identidad ────────────────────────────────────────────────────

/// Comprueba QUIÉN es el disco y, si convence, abre la puerta de escritura.
///
/// Se llama una vez, después de `scan_partitions()` y del montaje de arranque.
/// Devuelve `true` si la escritura queda armada. El motivo —diga que sí o que
/// no— queda en `gate_reason()` y en la bitácora de CABINA, porque un booleano
/// no se puede fotografiar.
pub fn verify_identity() -> bool {
    unsafe { WRITE_ARMED = false; }
    let deny = |reason: &'static str, val: u64| -> bool {
        unsafe { GATE_REASON = reason; }
        crate::ring0::cabina::warn("disk", reason, val);
        false
    };

    if !is_ready() {
        return deny("gate: no hay disco que identificar", 0);
    }
    // 1. El disco tiene que haber dicho quién es. Un IDENTIFY que no contestó
    //    deja las cadenas vacías, y sobre un desconocido no se escribe.
    if unsafe { MODEL_LEN } == 0 || unsafe { SERIAL_LEN } == 0 {
        return deny("gate: el disco no declaro modelo o serie", 0);
    }
    if unsafe { TOTAL_SECTORS } == 0 {
        return deny("gate: el disco no declaro su tamano", 0);
    }
    // 2. La tabla de particiones tiene que existir y CUADRAR con el tamaño que
    //    el propio disco declara. Si la GPT dice que el disco es más grande de
    //    lo que el disco dice ser, uno de los dos miente y no se sabe cuál:
    //    puede ser una imagen clonada a un disco menor, y escribir ahí es
    //    escribir sobre datos que la tabla cree libres.
    let parts = partitions();
    if parts.is_empty() || last_lba() == 0 {
        return deny("gate: sin tabla GPT legible", 0);
    }
    let (total, last) = (unsafe { TOTAL_SECTORS }, last_lba());
    if last >= total {
        return deny("gate: la GPT declara mas sectores que el disco", last);
    }
    // La cola reservada de una GPT sana son ~34 sectores (copia de la tabla al
    // final). Un hueco grande significa que la tabla se hizo para otro disco.
    if total - last > 128 {
        return deny("gate: la GPT no cuadra con el tamano del disco", total - last);
    }
    // 3. Tiene que ser un disco de arranque EFI. No prueba que sea EL nuestro
    //    —eso llega con el `disco_id` grabado dentro del volumen— pero descarta
    //    el caso que de verdad daba miedo: escribir en un disco de datos ajeno
    //    porque el barrido PCI lo puso primero.
    if !parts.iter().any(|p| p.is_esp()) {
        return deny("gate: el disco no tiene particion de arranque EFI", 0);
    }
    // 4. Y tiene que haber una partición de datos donde escribir que NO sea la
    //    de arranque.
    let win = match data_partition() {
        Some(p) => p,
        None => return deny("gate: no hay particion de datos fuera de la EFI", 0),
    };

    unsafe {
        WRITE_ARMED = true;
        GATE_REASON = "gate: disco identificado, escritura armada";
    }
    crate::ring0::cabina::info("disk", model(), win.first_lba);
    crate::ring0::cabina::info("disk", "escritura ARMADA en la particion de datos", win.sectors());
    true
}

/// La partición donde BMO puede escribir: la primera que NO es la de arranque.
///
/// La EFI queda fuera por diseño. Ahí vive el `BOOTX64.EFI` con el que arrancó
/// esta misma ejecución y, en una máquina con Windows, también el cargador del
/// dueño. Un bug de sistema de ficheros que se coma esa partición no da un
/// fault bonito: deja la máquina sin arrancar.
pub fn data_partition() -> Option<Partition> {
    partitions().iter().find(|p| !p.is_esp()).copied()
}

/// ¿Puede escribirse el rango `[lba, lba+count)`? Devuelve el motivo si no.
///
/// Es la segunda línea de defensa, independiente del gate: aunque la identidad
/// esté armada, un sector fuera de la partición de datos sigue siendo
/// intocable. Los dos fallos que esto ataja son el desbordamiento aritmético
/// de un cálculo de LBA y un sistema de ficheros que se cree en otro sitio.
fn write_window(lba: u64, count: u16) -> Result<(), &'static str> {
    if !is_ready() { return Err("sin disco"); }
    if !write_armed() { return Err("la escritura no esta armada (gate de identidad)"); }
    if count == 0 { return Err("cero sectores"); }
    // Sin `checked_add` un LBA cerca del máximo daría la vuelta y el rango
    // parecería diminuto y válido.
    let end = match lba.checked_add(count as u64) {
        Some(e) => e,
        None => return Err("el rango de LBA desborda"),
    };

    // ── Ventana 1: la partición de datos (FAT32, los `.bex`) ──
    if let Some(w) = data_partition() {
        if lba >= w.first_lba && end <= w.last_lba + 1 {
            return Ok(());
        }
    }
    // ── Ventana 2: el volumen ESTRATOS ──
    //
    // Sólo existe si el volumen está montado Y nació en este disco. Son dos
    // ventanas y no una ensanchada: la ESP —donde vive el `BOOTX64.EFI` con el
    // que arrancó la máquina— no está en ninguna de las dos, y ésa es la
    // propiedad que hay que conservar mientras el resto crece.
    if let Some((primero, ultimo)) = ventana_estratos() {
        if lba >= primero && end <= ultimo + 1 {
            return Ok(());
        }
    }
    Err("fuera de las ventanas de escritura (datos / ESTRATOS)")
}

/// Escribe `count` sectores en `lba`. Devuelve los sectores escritos.
///
/// Espejo de `read`, con dos guardias delante: el gate de identidad y la
/// ventana. Un rechazo NUNCA es mudo — dice cuál de las dos lo paró y dónde.
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

    const PER_BATCH: u16 = (4096 / SECTOR) as u16; // 8 sectores por página
    let mut done = 0u16;
    while done < count {
        let batch = (count - done).min(PER_BATCH);
        // A la página de rebote primero: el HBA solo lee de memoria física
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

/// Obliga al disco a bajar a la superficie lo que aceptó.
///
/// Una escritura que devuelve OK solo promete que el disco se quedó con los
/// bytes. La caja negra existe para sobrevivir al corte que se investiga: sin
/// esto, el registro del vuelo se pierde justo en el accidente.
pub fn flush() -> bool {
    if !is_ready() || !write_armed() { return false; }
    match unsafe { bmo_ahci::flush_cache(unsafe { PORT }) } {
        Ok(()) => true,
        Err(e) => {
            crate::ring0::cabina::fault("disk", e.name(), 0);
            false
        }
    }
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

// ── El contrato de bloques, implementado ────────────────────────────────────
//
// `bmo-block` declara la forma (leer / escribir / capacidad / identidad); aquí
// se dice que el AHCI de esta máquina la cumple. Es el paso 3 del orden de
// construcción de ESTRATOS: a partir de ahora, lo que hay por encima habla con
// un dispositivo de bloques y no con SATA.

/// El disco SATA de BMO visto como dispositivo de bloques.
///
/// Sin campos: el estado vive en los estáticos del módulo porque hay UN disco
/// y lo hay durante toda la vida del kernel. Un struct vacío es lo que permite
/// tener un `&'static dyn BlockDevice` sin reservar memoria, que en Ring 0 no
/// se puede.
pub struct AhciDisk;
static AHCI_DISK: AhciDisk = AhciDisk;

impl bmo_block::BlockDevice for AhciDisk {
    fn identity(&self) -> bmo_block::DeviceId {
        let mut id = bmo_block::DeviceId::EMPTY;
        unsafe {
            // Slices pedidos explícitamente: tomar `&(*addr_of!(ARR))[..n]`
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
        // `read` sin `self.` es la función libre del módulo, no este método.
        match read(lba, count, buf) {
            0 => Err(E::Device),
            n => Ok(n),
        }
    }

    fn write(&self, lba: u64, count: u16, data: &[u8]) -> Result<u16, bmo_block::BlockError> {
        use bmo_block::BlockError as E;
        if !is_ready() { return Err(E::NotReady); }
        // El gate primero y con su propio código: "no me han autorizado a
        // escribir" no es lo mismo que "el disco falló", y quien llama va a
        // hacer cosas distintas con cada respuesta.
        if !write_armed() { return Err(E::ReadOnly); }
        if count == 0 { return Ok(0); }
        if data.len() < count as usize * SECTOR { return Err(E::ShortBuffer); }
        let end = lba.checked_add(count as u64).ok_or(E::OutOfRange)?;
        let total = unsafe { TOTAL_SECTORS };
        if total != 0 && end > total { return Err(E::OutOfRange); }
        // La VENTANA sigue mandando por debajo: el contrato de bloques no la
        // sustituye. Un rango fuera de la partición de datos se rechaza aquí
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

// ── Las funciones sueltas que consume bmo-fat32 ─────────────────────────────
//
// `bmo-fat32` recibe punteros a función, no un objeto de trait, y se queda
// así: cambiarlo sería tocar un sistema de ficheros que YA funciona en
// hardware para no ganar nada hoy. Cuando ESTRATOS llegue, hablará con
// `bmo_block::device()` directamente.

/// Lector de bloques del disco montado. LBA ABSOLUTO del dispositivo.
pub fn block_read(lba: u64, count: u16, buf: &mut [u8]) -> bool {
    if count == 0 || buf.len() < count as usize * SECTOR { return false; }
    read(lba, count, buf) == count
}

/// Escritor de bloques. LBA ABSOLUTO del dispositivo.
///
/// La otra mitad del contrato. Se le entrega a `bmo-fat32` **solo** para el
/// volumen de datos: al de arranque se le sigue montando con `None`, y así su
/// inmutabilidad no depende de que nadie se equivoque, sino de que no exista
/// la función.
pub fn block_write(lba: u64, count: u16, data: &[u8]) -> bool {
    if count == 0 || data.len() < count as usize * SECTOR { return false; }
    write(lba, count, data) == count
}

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
