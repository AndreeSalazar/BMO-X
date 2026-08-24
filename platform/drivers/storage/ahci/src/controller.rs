//! AHCI/SATA: el camino de comandos, escrito contra la especificacion.
//!
//! ## Como se le pide algo a un disco SATA
//!
//! El HBA no recibe ordenes por registros: se las deja escritas en memoria y
//! se le toca una campana. Tres estructuras, todas en RAM y todas leidas por
//! el controlador POR DIRECCION FISICA:
//!
//! 1. **Command List** (`PxCLB`): 32 cabeceras de 32 bytes, una por ranura.
//!    Cada cabecera dice cuantos dwords mide el FIS, si es escritura, cuantas
//!    entradas tiene el PRDT y --lo importante-- DONDE esta su command table.
//! 2. **Command Table** (`CTBA` en la cabecera): el FIS de mando (64 B) y, a
//!    partir del byte 0x80, el PRDT.
//! 3. **PRDT**: la lista de trozos de memoria donde van (o de donde salen) los
//!    datos. Cada entrada lleva una direccion FISICA y un contador de bytes
//!    MENOS UNO.
//! 4. **FIS Receive Area** (`PxFB`): donde el HBA deja lo que responde el disco.
//!
//! Y luego `PxCI` bit N = "ejecuta la ranura N". El bit se limpia solo cuando
//! el comando termina.
//!
//! ## Reglas que esta version respeta y la anterior no
//!
//! - **Direcciones fisicas, siempre.** El HBA no sabe que es una direccion
//!   virtual. La version previa metia en el PRDT el puntero del kernel: el
//!   disco habria escrito sus datos en una direccion al azar de la RAM.
//! - **La cabecera lleva la CTBA.** La version previa escribia la direccion de
//!   la command table DENTRO de la propia command table, dejando la cabecera
//!   en ceros; el driver leia despues ese cero y construia el FIS en la pagina
//!   fisica 0.
//! - **Todo espera con limite.** Un disco que no contesta tiene que devolver un
//!   error, nunca colgar la maquina.
//! - **Los errores se miran.** `PxTFD.ERR` y `PxIS.TFES` existen para eso; un
//!   comando que falla no puede devolver "leidos N sectores".

use crate::storage_hal;
use core::sync::atomic::{AtomicBool, Ordering};

// ** Lo que se fue a `arranque.rs` y a `comando.rs` se REEXPORTA desde aqui, y
// no se cambia ni un `use` de quien lo llamaba. El reparto mueve texto: si
// ademas obligara a media docena de ficheros a saber en cual de los tres vive
// cada nombre, seria un reparto que se paga fuera.
pub use super::arranque::*;
pub use super::comando::*;


// ** LAS CONSTANTES DEL MAPA DE REGISTROS SE ABREN AL CRATE.
//
// Son los nombres del HBA --`GHC_AE`, `CMD_ST`, `PORT_IS`...-- y los usan los
// tres ficheros, porque los tres hablan con el mismo aparato. Antes del reparto
// eran privadas porque "privado" significaba "de este fichero" y el fichero era
// uno solo.
//
// [!] Y esto NO las convierte en API: siguen sin salir del crate. Lo que dicen
// es un hecho sobre un chip, y el sitio donde vive ese hecho es este.
// -- Registros del HBA -------------------------------------------------------

pub(crate) const HBA_CAP: usize = 0x00;
pub(crate) const HBA_GHC: usize = 0x04;
pub(crate) const HBA_IS:  usize = 0x08;
pub(crate) const HBA_PI:  usize = 0x0C;

pub(crate) const PORT_STRIDE: usize = 0x100;
pub(crate) const PORT_CLB:  usize = 0x00;
pub(crate) const PORT_CLBU: usize = 0x04;
pub(crate) const PORT_FB:   usize = 0x08;
pub(crate) const PORT_FBU:  usize = 0x0C;
pub(crate) const PORT_IS:   usize = 0x10;
pub(crate) const PORT_CMD:  usize = 0x18;
pub(crate) const PORT_TFD:  usize = 0x20;
pub(crate) const PORT_SIG:  usize = 0x24;
pub(crate) const PORT_SSTS: usize = 0x28;
pub(crate) const PORT_SCTL: usize = 0x2C;
pub(crate) const PORT_SERR: usize = 0x30;
pub(crate) const PORT_CI:   usize = 0x38;

pub(crate) const GHC_HR: u32 = 1 << 0;  // HBA Reset
pub(crate) const GHC_IE: u32 = 1 << 1;  // Interrupt Enable
pub(crate) const GHC_AE: u32 = 1 << 31; // AHCI Enable

pub(crate) const CMD_ST:  u32 = 1 << 0;  // Start
pub(crate) const CMD_FRE: u32 = 1 << 4;  // FIS Receive Enable
pub(crate) const CMD_FR:  u32 = 1 << 14; // FIS Receive Running
pub(crate) const CMD_CR:  u32 = 1 << 15; // Command list Running

pub(crate) const SSTS_DET: u32 = 0x0F;

/// Task File Data: el disco esta ocupado, o pidiendo/entregando datos.
pub(crate) const TFD_BSY: u32 = 1 << 7;
pub(crate) const TFD_DRQ: u32 = 1 << 3;
pub(crate) const TFD_ERR: u32 = 1 << 0;
/// Interrupt Status: Task File Error.
pub(crate) const IS_TFES: u32 = 1 << 30;

pub(crate) const FIS_TYPE_REG_H2D: u8 = 0x27;
pub const ATA_CMD_READ_DMA_EX:  u8 = 0x25;
pub(crate) const ATA_CMD_WRITE_DMA_EX: u8 = 0x35;
pub(crate) const ATA_CMD_IDENTIFY:     u8 = 0xEC;
/// FLUSH CACHE EXT: obliga al disco a bajar a la superficie lo que acepto y
/// tiene todavia en su cache. Un `WRITE DMA` que devuelve OK solo promete que
/// el disco se quedo con los datos, no que sobrevivan a un corte.
pub(crate) const ATA_CMD_FLUSH_EXT:    u8 = 0xEA;
/// **DATA SET MANAGEMENT**: la orden que lleva TRIM dentro.
///
/// El comando no es "TRIM": TRIM es **una funcion suya**, y cual se pide lo
/// dice el registro de features ([`DSM_TRIM`]). Por eso este es el unico
/// comando del driver que necesita ese registro -- ver `armar`.
pub(crate) const ATA_CMD_DSM:          u8 = 0x06;
/// Bit 0 de features: la funcion TRIM de `DATA SET MANAGEMENT`.
///
/// ** Con este bit a cero el disco recibe un DSM **sin funcion pedida**. No es
/// un TRIM que no hace nada: es una orden distinta, y lo que haga con el payload
/// depende del aparato. El bit no es un detalle del empaquetado.
pub(crate) const DSM_TRIM: u16 = 1 << 0;

/// Firma que deja un disco duro SATA en `PxSIG`. Un 0xEB140101 seria una
/// unidad optica (ATAPI) y un 0xFFFF0000, un puerto sin nada.
pub const SIG_SATA_DISK: u32 = 0x0000_0101;

/// Bytes por sector logico. Todo LBA de este driver es de 512 B.
pub const SECTOR: usize = 512;

/// Espera maxima para que un comando termine, en iteraciones de sondeo. Un
/// disco dormido puede tardar; un SSD contesta en microsegundos. El numero es
/// generoso a proposito: el limite existe para que un puerto MUERTO no cuelgue
/// la maquina, no para cronometrar al disco.
pub(crate) const CMD_TIMEOUT: u32 = 20_000_000;
/// Espera para los cambios de estado del puerto (arranque/parada del motor de
/// comandos), que son inmediatos salvo averia.
pub(crate) const PORT_TIMEOUT: u32 = 1_000_000;

/// ** LA PACIENCIA DE `DATA SET MANAGEMENT`, que no es la de una lectura.
///
/// Veinte veces [`CMD_TIMEOUT`], y el motivo no es "por si acaso": **una sola
/// orden de TRIM puede cubrir gigabytes**. Lo que el aparato hace con ella --
/// tocar sus tablas de traduccion y marcar bloques enteros como libres-- no se
/// parece a mover 4 KiB, y la especificacion **no acota cuanto puede tardar**.
///
/// Compartir el presupuesto de una lectura no fue una decision: fue que este
/// driver solo sabia pedir cosas parecidas entre si. Un `Timeout` aqui no
/// significaria "el disco esta roto" sino "no le dimos tiempo", que es la peor
/// clase de diagnostico -- el que acusa al aparato de lo que hizo el driver.
///
/// [!] Y sigue siendo un contador de VUELTAS, no un tiempo: quien gira aqui no
/// tiene reloj. El limite existe para que un puerto muerto no cuelgue la
/// maquina, no para cronometrar al disco.
pub(crate) const DSM_TIMEOUT: u32 = CMD_TIMEOUT * 20;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState { Empty, Present, Active, Error }

#[derive(Debug, Clone, Copy)]
pub struct AhciPort {
    pub port_number: u8,
    pub state: PortState,
    pub signature: u32,
    /// `PxSSTS` crudo tal como lo dejo el censo. DET (bits 3:0) es lo que
    /// decide si hay disco: 0=nada, 1=algo conectado sin comunicacion,
    /// 3=enlace establecido. Guardarlo permite PINTAR el numero en vez de
    /// deducir por que no aparece el disco.
    pub ssts: u32,
    /// `PxSCTL` y `PxCMD` crudos, para poder VER si el COMRESET se aplico
    /// y en que estado quedaron los motores del puerto.
    pub sctl: u32,
    pub cmd: u32,
    /// Command List (32 cabeceras x 32 B), fisica.
    pub command_list_phys: u64,
    /// FIS Receive Area, fisica.
    pub fis_phys: u64,
    /// Command Table de la ranura 0, fisica.
    pub cmd_table_phys: u64,
}

#[derive(Debug)]
pub struct AhciController {
    pub mmio_base: u64,
    pub port_count: u8,
    pub ports_implemented: u32,
    pub ports: [AhciPort; 32],
}

pub(crate) static mut CONTROLLER: Option<AhciController> = None;
pub(crate) static INIT_DONE: AtomicBool = AtomicBool::new(false);

/// Por que fallo la ultima operacion. Un codigo que se puede pintar vale mas
/// que un cero mudo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskError {
    /// No hay controlador, o el puerto no esta preparado.
    NotReady,
    /// El disco no solto BSY/DRQ: no se le pudo dar la orden.
    Busy,
    /// El comando no termino dentro del limite.
    Timeout,
    /// El disco respondio con error (`PxTFD.ERR` / `PxIS.TFES`).
    Device(u32),
    /// Peticion imposible (0 sectores, o mas de los que caben en el PRDT).
    BadRequest,
}

impl DiskError {
    pub fn name(self) -> &'static str {
        match self {
            DiskError::NotReady => "puerto no preparado",
            DiskError::Busy => "el disco no acepta ordenes (BSY/DRQ)",
            DiskError::Timeout => "el comando no termino a tiempo",
            DiskError::Device(_) => "el disco respondio con error",
            DiskError::BadRequest => "peticion invalida",
        }
    }
}

// -- Acceso MMIO -------------------------------------------------------------

// ** Los cuatro accesos a MMIO se abren al crate: los tres ficheros hablan con
// el mismo aparato por la misma ventana. Ver la nota de las constantes.
pub(crate) unsafe fn hba_read(mmio: u64, offset: usize) -> u32 {
    core::ptr::read_volatile((mmio + offset as u64) as *const u32)
}
pub(crate) unsafe fn hba_write(mmio: u64, offset: usize, val: u32) {
    core::ptr::write_volatile((mmio + offset as u64) as *mut u32, val);
}
pub(crate) unsafe fn port_read(mmio: u64, port: u8, offset: usize) -> u32 {
    let base = mmio + 0x100 + (port as u64) * PORT_STRIDE as u64;
    core::ptr::read_volatile((base + offset as u64) as *const u32)
}
pub(crate) unsafe fn port_write(mmio: u64, port: u8, offset: usize, val: u32) {
    let base = mmio + 0x100 + (port as u64) * PORT_STRIDE as u64;
    core::ptr::write_volatile((base + offset as u64) as *mut u32, val);
}


// -- ** QUE EL APARATO AVISE ------------------------------------------------

/// Offset del registro de interrupciones habilitadas del puerto.
pub(crate) const PORT_IE: usize = 0x14;
/// `DHRS`: llego el FIS de registro Device-to-Host. Es el que marca "termine el
/// comando" en una lectura o escritura DMA -- el unico que hace falta para lo
/// que este driver sabe pedir.
pub(crate) const IE_DHRS: u32 = 1 << 0;

/// Cuantas veces ha avisado el aparato. Lo sube [`atender`] desde el manejador
/// de interrupcion; lo mira [`run_command`] para dejar de leer MMIO en balde.
///
/// Es un `AtomicU32` y no un booleano porque **el contador no se pierde**: un
/// aviso que llega justo antes de que alguien empiece a mirar sigue contando, y
/// comparar contra una marca tomada al emitir dice si hubo aviso DESPUES.
pub static AVISOS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// **Enciende las interrupciones del puerto y del HBA.** `false` si no hay
/// controlador.
///
/// Se llama DESPUES de haber armado MSI, nunca antes: un aparato al que se le
/// dice que avise cuando su aviso no llega a ninguna parte se queda esperando a
/// que le contesten, y el sintoma --lecturas que no terminan-- no se parece en
/// nada a la causa.
pub unsafe fn habilitar_irq(port_idx: u8) -> bool {
    #[allow(static_mut_refs)]
    let ctrl = match CONTROLLER.as_ref() { Some(c) => c, None => return false };
    if port_idx >= 32 { return false; }
    let mmio = ctrl.mmio_base;
    // Lo que hubiera pendiente se limpia ANTES de abrir la puerta: un aviso
    // viejo entrando como si fuera nuevo es un comando que se da por terminado
    // sin haber empezado.
    port_write(mmio, port_idx, PORT_IS, port_read(mmio, port_idx, PORT_IS));
    hba_write(mmio, HBA_IS, hba_read(mmio, HBA_IS));
    port_write(mmio, port_idx, PORT_IE, IE_DHRS);
    hba_write(mmio, HBA_GHC, hba_read(mmio, HBA_GHC) | GHC_IE);
    true
}

/// **Atiende el aviso.** Limpia el estado del aparato y cuenta. Nada mas.
///
/// Devuelve `true` si el aviso era de este puerto. Corre en contexto de
/// interrupcion: aqui no se lee `PRDBC` ni se decide nada -- eso es contestarle
/// a quien pregunte, y preguntar es cosa del que pidio.
pub unsafe fn atender(port_idx: u8) -> bool {
    #[allow(static_mut_refs)]
    let ctrl = match CONTROLLER.as_ref() { Some(c) => c, None => return false };
    if port_idx >= 32 { return false; }
    let mmio = ctrl.mmio_base;
    let is_hba = hba_read(mmio, HBA_IS);
    let mio = is_hba & (1 << port_idx) != 0;
    if mio {
        // ** EL ORDEN IMPORTA: primero el puerto, despues el HBA.
        //
        // El bit del HBA es el OR de los del puerto. Borrarlo primero y que el
        // puerto siguiera con el suyo puesto lo volveria a encender en el acto,
        // y el aparato quedaria pidiendo atencion para siempre -- una tormenta
        // de interrupciones que no deja correr a nadie.
        let is_puerto = port_read(mmio, port_idx, PORT_IS);
        port_write(mmio, port_idx, PORT_IS, is_puerto);
    }
    hba_write(mmio, HBA_IS, is_hba);
    if mio {
        AVISOS.fetch_add(1, core::sync::atomic::Ordering::Release);
    }
    mio
}

/// Lee `sector_count` sectores desde `lba` al buffer FISICO `buf_phys`.
pub unsafe fn read_sectors_phys(port_idx: u8, lba: u64, sector_count: u16, buf_phys: u64)
    -> Result<u16, DiskError>
{
    if sector_count == 0 { return Err(DiskError::BadRequest); }
    let bytes = sector_count as u32 * SECTOR as u32;
    run_command(port_idx, ATA_CMD_READ_DMA_EX, 0, lba, sector_count, Some((buf_phys, bytes)), false)
}

/// Escribe `sector_count` sectores en `lba` desde el buffer FISICO `buf_phys`.
///
/// Existe porque un driver de disco a medias no es un driver. Que el kernel la
/// exponga o no --y a quien-- es decision suya, no de esta capa.
pub unsafe fn write_sectors_phys(port_idx: u8, lba: u64, sector_count: u16, buf_phys: u64)
    -> Result<u16, DiskError>
{
    if sector_count == 0 { return Err(DiskError::BadRequest); }
    let bytes = sector_count as u32 * SECTOR as u32;
    run_command(port_idx, ATA_CMD_WRITE_DMA_EX, 0, lba, sector_count, Some((buf_phys, bytes)), true)
}

/// **TRIM: decirle al disco que estos sectores ya no le importan a nadie.**
///
/// `buf_phys` apunta al payload que arma `bmo_trim` --descriptores de rango-- y
/// `bloques` es su tamano **en bloques de 512 B**, que es la unidad en la que
/// cuenta este comando. No mueve datos del disco: los mueve HACIA el.
///
/// === Las tres cosas que no se parecen a una escritura ===
///
/// 1. **El contador no son sectores del disco**, son bloques de payload. Un
///    solo bloque puede cubrir 2 GiB de disco.
/// 2. **`features` elige la funcion.** Sin [`DSM_TRIM`] esto es otro comando.
/// 3. **No hay `PRDBC` que creer.** Se manda como escritura --el host entrega
///    bytes-- y ya se sabe que no todos los HBA rellenan ese contador al
///    escribir (ver `sondear`); lo que dice si salio bien es `TFD.ERR`. Por eso
///    esta funcion devuelve `()` y no un numero: **inventarse "cuantos recorto"
///    a partir de un contador opcional seria mentir con un numero**, y lo que
///    cubre cada orden ya lo sabe quien armo el payload.
///
/// [!] Sin NCQ y sin encolar, que es donde esta BMO-X hoy. El historial de
/// corrupcion de TRIM es del TRIM **encolado** (`NO_NCQ_TRIM` de Linux), no de
/// este -- ver `docs/componente/EL_DISCO_EXIGE.md`.
pub unsafe fn trim_phys(port_idx: u8, buf_phys: u64, bloques: u16) -> Result<(), DiskError> {
    if bloques == 0 { return Err(DiskError::BadRequest); }
    let bytes = bloques as u32 * SECTOR as u32;
    run_command_hasta(
        port_idx, ATA_CMD_DSM, DSM_TRIM, 0, bloques, Some((buf_phys, bytes)), true,
        DSM_TIMEOUT,
    )
    .map(|_| ())
}

/// Ordena al disco bajar a la superficie todo lo que acepto y aun tiene en su
/// cache. Sin datos: es una orden, no una transferencia.
///
/// Un `WRITE DMA` que devuelve OK solo promete que el disco se quedo con los
/// bytes. Para una caja negra --que existe justamente para sobrevivir al corte
/// que se esta investigando-- esa promesa no basta: el punto de no retorno es
/// este comando.
pub unsafe fn flush_cache(port_idx: u8) -> Result<(), DiskError> {
    run_command(port_idx, ATA_CMD_FLUSH_EXT, 0, 0, 0, None, false).map(|_| ())
}

/// IDENTIFY DEVICE: 512 bytes con el modelo, el numero de serie y los sectores
/// del disco.
///
/// Es la forma de que BMO sepa A QUE DISCO le esta hablando, en vez de fiarse
/// del orden de enumeracion. Con dos discos en la maquina y el sistema del
/// dueno en uno de ellos, eso no es un lujo.
pub unsafe fn identify_phys(port_idx: u8, buf_phys: u64) -> Result<u16, DiskError> {
    // IDENTIFY entrega exactamente un sector y no usa LBA ni contador.
    run_command(port_idx, ATA_CMD_IDENTIFY, 0, 0, 1, Some((buf_phys, SECTOR as u32)), false)
}

pub fn controller() -> Option<&'static AhciController> {
    #[allow(static_mut_refs)]
    unsafe { CONTROLLER.as_ref() }
}

/// Olvida el controlador actual para poder probar OTRO.
///
/// Una placa puede traer mas de un HBA SATA (el del chipset y alguno anadido),
/// y el disco que buscamos puede estar en el segundo. Sin esto, `probe` se
/// queda con el primero para siempre por su guarda de inicializacion.
pub fn reset_ctrl() {
    unsafe { CONTROLLER = None; }
    INIT_DONE.store(false, Ordering::SeqCst);
}
