//! AHCI/SATA: el camino de comandos, escrito contra la especificación.
//!
//! ## Cómo se le pide algo a un disco SATA
//!
//! El HBA no recibe órdenes por registros: se las deja escritas en memoria y
//! se le toca una campana. Tres estructuras, todas en RAM y todas leídas por
//! el controlador POR DIRECCIÓN FÍSICA:
//!
//! 1. **Command List** (`PxCLB`): 32 cabeceras de 32 bytes, una por ranura.
//!    Cada cabecera dice cuántos dwords mide el FIS, si es escritura, cuántas
//!    entradas tiene el PRDT y —lo importante— DÓNDE está su command table.
//! 2. **Command Table** (`CTBA` en la cabecera): el FIS de mando (64 B) y, a
//!    partir del byte 0x80, el PRDT.
//! 3. **PRDT**: la lista de trozos de memoria donde van (o de donde salen) los
//!    datos. Cada entrada lleva una dirección FÍSICA y un contador de bytes
//!    MENOS UNO.
//! 4. **FIS Receive Area** (`PxFB`): donde el HBA deja lo que responde el disco.
//!
//! Y luego `PxCI` bit N = "ejecuta la ranura N". El bit se limpia solo cuando
//! el comando termina.
//!
//! ## Reglas que esta versión respeta y la anterior no
//!
//! - **Direcciones físicas, siempre.** El HBA no sabe qué es una dirección
//!   virtual. La versión previa metía en el PRDT el puntero del kernel: el
//!   disco habría escrito sus datos en una dirección al azar de la RAM.
//! - **La cabecera lleva la CTBA.** La versión previa escribía la dirección de
//!   la command table DENTRO de la propia command table, dejando la cabecera
//!   en ceros; el driver leía después ese cero y construía el FIS en la página
//!   física 0.
//! - **Todo espera con límite.** Un disco que no contesta tiene que devolver un
//!   error, nunca colgar la máquina.
//! - **Los errores se miran.** `PxTFD.ERR` y `PxIS.TFES` existen para eso; un
//!   comando que falla no puede devolver "leídos N sectores".

use crate::storage_hal;
use core::sync::atomic::{AtomicBool, Ordering};

// ── Registros del HBA ───────────────────────────────────────────────────────

const HBA_CAP: usize = 0x00;
const HBA_GHC: usize = 0x04;
const HBA_IS:  usize = 0x08;
const HBA_PI:  usize = 0x0C;

const PORT_STRIDE: usize = 0x100;
const PORT_CLB:  usize = 0x00;
const PORT_CLBU: usize = 0x04;
const PORT_FB:   usize = 0x08;
const PORT_FBU:  usize = 0x0C;
const PORT_IS:   usize = 0x10;
const PORT_CMD:  usize = 0x18;
const PORT_TFD:  usize = 0x20;
const PORT_SIG:  usize = 0x24;
const PORT_SSTS: usize = 0x28;
const PORT_SCTL: usize = 0x2C;
const PORT_SERR: usize = 0x30;
const PORT_CI:   usize = 0x38;

const GHC_HR: u32 = 1 << 0;  // HBA Reset
const GHC_IE: u32 = 1 << 1;  // Interrupt Enable
const GHC_AE: u32 = 1 << 31; // AHCI Enable

const CMD_ST:  u32 = 1 << 0;  // Start
const CMD_FRE: u32 = 1 << 4;  // FIS Receive Enable
const CMD_FR:  u32 = 1 << 14; // FIS Receive Running
const CMD_CR:  u32 = 1 << 15; // Command list Running

const SSTS_DET: u32 = 0x0F;

/// Task File Data: el disco está ocupado, o pidiendo/entregando datos.
const TFD_BSY: u32 = 1 << 7;
const TFD_DRQ: u32 = 1 << 3;
const TFD_ERR: u32 = 1 << 0;
/// Interrupt Status: Task File Error.
const IS_TFES: u32 = 1 << 30;

const FIS_TYPE_REG_H2D: u8 = 0x27;
const ATA_CMD_READ_DMA_EX:  u8 = 0x25;
const ATA_CMD_WRITE_DMA_EX: u8 = 0x35;
const ATA_CMD_IDENTIFY:     u8 = 0xEC;

/// Firma que deja un disco duro SATA en `PxSIG`. Un 0xEB140101 sería una
/// unidad óptica (ATAPI) y un 0xFFFF0000, un puerto sin nada.
pub const SIG_SATA_DISK: u32 = 0x0000_0101;

/// Bytes por sector lógico. Todo LBA de este driver es de 512 B.
pub const SECTOR: usize = 512;

/// Espera máxima para que un comando termine, en iteraciones de sondeo. Un
/// disco dormido puede tardar; un SSD contesta en microsegundos. El número es
/// generoso a propósito: el límite existe para que un puerto MUERTO no cuelgue
/// la máquina, no para cronometrar al disco.
const CMD_TIMEOUT: u32 = 20_000_000;
/// Espera para los cambios de estado del puerto (arranque/parada del motor de
/// comandos), que son inmediatos salvo avería.
const PORT_TIMEOUT: u32 = 1_000_000;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState { Empty, Present, Active, Error }

#[derive(Debug, Clone, Copy)]
pub struct AhciPort {
    pub port_number: u8,
    pub state: PortState,
    pub signature: u32,
    /// `PxSSTS` crudo tal como lo dejó el censo. DET (bits 3:0) es lo que
    /// decide si hay disco: 0=nada, 1=algo conectado sin comunicación,
    /// 3=enlace establecido. Guardarlo permite PINTAR el número en vez de
    /// deducir por qué no aparece el disco.
    pub ssts: u32,
    /// `PxSCTL` y `PxCMD` crudos, para poder VER si el COMRESET se aplico
    /// y en que estado quedaron los motores del puerto.
    pub sctl: u32,
    pub cmd: u32,
    /// Command List (32 cabeceras × 32 B), física.
    pub command_list_phys: u64,
    /// FIS Receive Area, física.
    pub fis_phys: u64,
    /// Command Table de la ranura 0, física.
    pub cmd_table_phys: u64,
}

#[derive(Debug)]
pub struct AhciController {
    pub mmio_base: u64,
    pub port_count: u8,
    pub ports_implemented: u32,
    pub ports: [AhciPort; 32],
}

static mut CONTROLLER: Option<AhciController> = None;
static INIT_DONE: AtomicBool = AtomicBool::new(false);

/// Por qué falló la última operación. Un código que se puede pintar vale más
/// que un cero mudo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskError {
    /// No hay controlador, o el puerto no está preparado.
    NotReady,
    /// El disco no soltó BSY/DRQ: no se le pudo dar la orden.
    Busy,
    /// El comando no terminó dentro del límite.
    Timeout,
    /// El disco respondió con error (`PxTFD.ERR` / `PxIS.TFES`).
    Device(u32),
    /// Petición imposible (0 sectores, o más de los que caben en el PRDT).
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

// ── Acceso MMIO ─────────────────────────────────────────────────────────────

unsafe fn hba_read(mmio: u64, offset: usize) -> u32 {
    core::ptr::read_volatile((mmio + offset as u64) as *const u32)
}
unsafe fn hba_write(mmio: u64, offset: usize, val: u32) {
    core::ptr::write_volatile((mmio + offset as u64) as *mut u32, val);
}
unsafe fn port_read(mmio: u64, port: u8, offset: usize) -> u32 {
    let base = mmio + 0x100 + (port as u64) * PORT_STRIDE as u64;
    core::ptr::read_volatile((base + offset as u64) as *const u32)
}
unsafe fn port_write(mmio: u64, port: u8, offset: usize, val: u32) {
    let base = mmio + 0x100 + (port as u64) * PORT_STRIDE as u64;
    core::ptr::write_volatile((base + offset as u64) as *mut u32, val);
}

// ── Arranque del controlador ────────────────────────────────────────────────

/// Prepara el HBA y hace censo de sus puertos. No toca ningún disco.
pub unsafe fn probe(mmio_base: u64) -> bool {
    if INIT_DONE.swap(true, Ordering::SeqCst) { return true; }
    let hal = storage_hal::hal();
    hal.log("[ahci] probing HBA\n");

    // ★ SIN RESET DEL HBA, a propósito.
    //
    // La versión anterior hacía `GHC.HR` nada más entrar y leía el estado de
    // los puertos justo después: cero puertos con disco, siempre. Normal — un
    // reset del HBA TIRA TODOS LOS ENLACES SATA, y renegociar un enlace lleva
    // decenas de milisegundos. Era preguntar "¿hay alguien?" un microsegundo
    // después de colgar el teléfono.
    //
    // Y el reset no hacía falta para nada: el firmware UEFI ya arrancó este
    // HBA y dejó los enlaces establecidos para poder leer el arranque. Lo
    // único que hay que asegurar es el modo (algunas placas lo dejan en modo
    // compatible IDE, donde los registros no significan lo que creemos) y
    // apagar las interrupciones, porque este driver sondea.
    hba_write(mmio_base, HBA_GHC, hba_read(mmio_base, HBA_GHC) | GHC_AE);
    hba_write(mmio_base, HBA_GHC, hba_read(mmio_base, HBA_GHC) & !GHC_IE);
    hba_write(mmio_base, HBA_IS, hba_read(mmio_base, HBA_IS)); // limpiar pendientes

    let cap = hba_read(mmio_base, HBA_CAP);
    let port_count = ((cap >> 20) & 0x1F) as u8 + 1;
    let pi = hba_read(mmio_base, HBA_PI);
    // Los registros del HBA, dichos en voz alta. Si CAP y PI salen 0x0 o
    // 0xFFFFFFFF, el problema no son los puertos: es que no estamos leyendo
    // el HBA (BAR equivocada, MMIO sin mapear). Sin estos dos números,
    // "ningún puerto tiene disco" es una conclusión sin pruebas.
    hal.log_hex("[ahci] cap=", cap as u64);
    hal.log_hex(" pi=", pi as u64);
    hal.log_hex(" ghc=", hba_read(mmio_base, HBA_GHC) as u64);
    hal.log("\n");

    let sss = cap & (1 << 27) != 0;

    let mut ctrl = AhciController {
        mmio_base, port_count, ports_implemented: pi,
        ports: [AhciPort {
            port_number: 0, state: PortState::Empty, signature: 0, ssts: 0, sctl: 0, cmd: 0,
            command_list_phys: 0, fis_phys: 0, cmd_table_phys: 0,
        }; 32],
    };

    // Primer intento: SUAVE. Se respeta lo que dejó el firmware y solo se
    // renegocia el enlace de los puertos que estén caídos.
    let mut active = census(&mut ctrl, pi, sss);

    // Segundo intento: EL MARTILLO. Si NINGÚN puerto levantó enlace, la
    // hipótesis cambia — no es que los discos no estén, es que el firmware
    // dejó el controlador en un estado del que no sabemos sacarlo puerto a
    // puerto. Ahí sí toca resetear el HBA entero y rehacer el trabajo, esta
    // vez esperando de verdad a que los enlaces vuelvan.
    //
    // Suave primero y martillo después, nunca al revés: el reset destruye el
    // trabajo que el firmware ya hizo, y si ese trabajo servía, mejor no
    // tocarlo.
    if active == 0 {
        hal.log("[ahci] ningun enlace: reset completo del HBA y reintento\n");
        hba_write(mmio_base, HBA_GHC, hba_read(mmio_base, HBA_GHC) | GHC_HR);
        let mut spun = 0u32;
        while hba_read(mmio_base, HBA_GHC) & GHC_HR != 0 && spun < PORT_TIMEOUT {
            spun += 1;
            core::hint::spin_loop();
        }
        // El reset apaga AE: sin él, los registros dejan de significar lo que
        // creemos.
        hba_write(mmio_base, HBA_GHC, hba_read(mmio_base, HBA_GHC) | GHC_AE);
        hba_write(mmio_base, HBA_GHC, hba_read(mmio_base, HBA_GHC) & !GHC_IE);
        hal.delay_ms(100); // que el HBA respire antes de tocarle los puertos
        active = census(&mut ctrl, pi, sss);
        hal.log_hex("[ahci] tras reset, puertos con enlace=", active as u64);
        hal.log("\n");
    }

    CONTROLLER = Some(ctrl);
    hal.log("[ahci] HBA listo\n");
    true
}

/// Levanta el enlace de cada puerto y anota su estado. Devuelve cuántos
/// quedaron con enlace vivo.
///
/// ★ NO SE CONFÍA EN `PI`. El registro de puertos implementados lo escribe el
/// firmware, y el firmware se equivoca: hay un caso conocido en Linux (Acer
/// Switch Alpha 12) donde la BIOS reporta un mapa que hace al driver SALTARSE
/// justo el puerto donde está el disco, y el arreglo del kernel es ignorar el
/// registro y forzar el valor bueno a mano. Aquí se recorren TODOS los puertos
/// que `CAP.NP` dice que existen y se anota si `PI` los declaraba o no;
/// escribir a un puerto inexistente es inofensivo (el registro no cambia, como
/// se vio con los puertos 4 y 5), y saltarse el puerto del disco no lo es.
unsafe fn census(ctrl: &mut AhciController, pi: u32, sss: bool) -> u32 {
    let hal = storage_hal::hal();
    let mmio_base = ctrl.mmio_base;
    let np = ctrl.port_count.min(32);
    let mut active = 0u32;
    for i in 0..np {
        let declared = pi & (1 << i) != 0;
        let ssts = port_link_up(mmio_base, i, sss);
        // Cada puerto dice su estado CRUDO aquí, en el driver, que es quien lo
        // tiene delante. El `!` marca los que `PI` NO declaraba: si uno de
        // esos trae disco, el firmware estaba mintiendo.
        hal.log(if declared { "[ahci] p" } else { "[ahci] !p" });
        hal.log_hex("", i as u64);
        hal.log_hex(" ssts=", ssts as u64);
        hal.log_hex(" cmd=", port_read(mmio_base, i, PORT_CMD) as u64);
        hal.log_hex(" sctl=", port_read(mmio_base, i, PORT_SCTL) as u64);
        hal.log_hex(" sig=", port_read(mmio_base, i, PORT_SIG) as u64);
        hal.log("\n");
        // DET=3 es "dispositivo presente y comunicación establecida": el único
        // estado en el que tiene sentido hablarle.
        let state = match ssts & SSTS_DET {
            0x03 => { active += 1; PortState::Active }
            0x01 => PortState::Present,
            _ => PortState::Empty,
        };
        ctrl.ports[i as usize] = AhciPort {
            port_number: i, state, signature: port_read(mmio_base, i, PORT_SIG), ssts,
            sctl: port_read(mmio_base, i, PORT_SCTL),
            cmd: port_read(mmio_base, i, PORT_CMD),
            command_list_phys: 0, fis_phys: 0, cmd_table_phys: 0,
        };
    }
    active
}

/// Levanta el enlace SATA de un puerto y devuelve su `PxSSTS` final.
///
/// ★ POR QUÉ HACE FALTA: el firmware UEFI usó este disco para arrancarnos y
/// después, al salir, PARÓ los puertos — se ve en `PxCMD` con ST y FRE a
/// cero. Un enlace parado reporta `DET=0`, que es indistinguible de "aquí no
/// hay nada". Encender el disco (SUD) no basta: hay que renegociar el enlace,
/// y eso se pide con un COMRESET por `PxSCTL`.
///
/// La secuencia es la de la especificación, y cada espera es de TIEMPO REAL:
/// contar vueltas de bucle mide la velocidad del CPU, no los milisegundos que
/// el SATA necesita.
unsafe fn port_link_up(mmio: u64, port: u8, sss: bool) -> u32 {
    let hal = storage_hal::hal();

    // 1. Con el motor de comandos andando no se toca el PHY.
    port_stop(mmio, port);

    // 2. Arrancar el disco si el HBA usa spin-up escalonado (CAP.SSS): con él,
    //    un puerto no negocia nada hasta que se le pide. SUD = Spin-Up Device,
    //    POD = Power On Device.
    if sss {
        let cmd = port_read(mmio, port, PORT_CMD);
        port_write(mmio, port, PORT_CMD, cmd | (1 << 1) | (1 << 2));
        hal.delay_ms(10);
    }

    // 3. ¿Ya está? Si el enlace vino vivo del firmware, aquí se acaba.
    let ssts = port_read(mmio, port, PORT_SSTS);
    if ssts & SSTS_DET == 0x03 {
        port_write(mmio, port, PORT_SERR, port_read(mmio, port, PORT_SERR));
        return ssts;
    }

    // 4. COMRESET: DET=1 fuerza la renegociación, y hay que sostenerlo al
    //    menos 1 ms antes de soltarlo a 0.
    let sctl = port_read(mmio, port, PORT_SCTL);
    port_write(mmio, port, PORT_SCTL, (sctl & !0xF) | 0x1);
    hal.delay_ms(2);
    port_write(mmio, port, PORT_SCTL, sctl & !0xF);

    // 5. Esperar el enlace. Un SSD contesta en milisegundos; a un disco
    //    mecánico dormido se le conceden hasta 1,5 s antes de darlo por vacío.
    let mut ssts = 0u32;
    for _ in 0..150 {
        ssts = port_read(mmio, port, PORT_SSTS);
        if ssts & SSTS_DET == 0x03 { break; }
        hal.delay_ms(10);
    }
    // 6. La negociación deja errores de estreno en PxSERR: se limpian, o el
    //    primer comando nacerá con un error que no es suyo.
    port_write(mmio, port, PORT_SERR, port_read(mmio, port, PORT_SERR));
    ssts
}

/// Detiene el motor de comandos del puerto y espera a que pare de verdad.
unsafe fn port_stop(mmio: u64, port: u8) -> bool {
    let cmd = port_read(mmio, port, PORT_CMD);
    port_write(mmio, port, PORT_CMD, cmd & !CMD_ST);
    let mut spun = 0u32;
    while port_read(mmio, port, PORT_CMD) & CMD_CR != 0 && spun < PORT_TIMEOUT {
        spun += 1; core::hint::spin_loop();
    }
    let cmd = port_read(mmio, port, PORT_CMD);
    port_write(mmio, port, PORT_CMD, cmd & !CMD_FRE);
    while port_read(mmio, port, PORT_CMD) & CMD_FR != 0 && spun < PORT_TIMEOUT {
        spun += 1; core::hint::spin_loop();
    }
    spun < PORT_TIMEOUT
}

/// Arranca el puerto en el orden que manda la especificación: primero recibir
/// FIS y, solo cuando eso corre, aceptar comandos.
unsafe fn port_start(mmio: u64, port: u8) -> bool {
    let cmd = port_read(mmio, port, PORT_CMD);
    port_write(mmio, port, PORT_CMD, cmd | CMD_FRE);
    let mut spun = 0u32;
    while port_read(mmio, port, PORT_CMD) & CMD_FR == 0 && spun < PORT_TIMEOUT {
        spun += 1; core::hint::spin_loop();
    }
    if spun >= PORT_TIMEOUT { return false; }
    let cmd = port_read(mmio, port, PORT_CMD);
    port_write(mmio, port, PORT_CMD, cmd | CMD_ST);
    true
}

/// Reserva las estructuras DMA del puerto y lo deja listo para comandos.
pub unsafe fn init_port_dma(port_idx: u8) -> bool {
    #[allow(static_mut_refs)]
    let ctrl = match CONTROLLER.as_mut() { Some(c) => c, None => return false };
    if port_idx >= 32 { return false; }
    let mmio = ctrl.mmio_base;
    let port = &mut ctrl.ports[port_idx as usize];
    if port.state != PortState::Active { return false; }
    let hal = storage_hal::hal();

    // El puerto tiene que estar PARADO antes de moverle las direcciones de sus
    // estructuras: con el motor corriendo, se las lleva a medio cambiar.
    if !port_stop(mmio, port_idx) {
        hal.log("[ahci] el puerto no se detiene\n");
        return false;
    }

    // Una página por estructura. Sobra sitio (la lista son 1 KiB y el área de
    // FIS 256 B), pero la página es la unidad que entrega el asignador y así
    // las alineaciones que exige el HBA (1 KiB / 256 B / 128 B) salen solas.
    let cl_phys = match hal.alloc_dma_pages(1) { Some(p) => p, None => return false };
    let fis_phys = match hal.alloc_dma_pages(1) { Some(p) => p, None => return false };
    let ct_phys = match hal.alloc_dma_pages(1) { Some(p) => p, None => return false };
    core::ptr::write_bytes(hal.phys_to_virt(cl_phys), 0, 4096);
    core::ptr::write_bytes(hal.phys_to_virt(fis_phys), 0, 4096);
    core::ptr::write_bytes(hal.phys_to_virt(ct_phys), 0, 4096);

    // Cabecera de la ranura 0 → dónde está SU command table. Esto es lo que
    // faltaba: sin CTBA en la cabecera, el HBA busca la orden en la dirección
    // física 0.
    let hdr = hal.phys_to_virt(cl_phys) as *mut u32;
    hdr.add(2).write_volatile((ct_phys & 0xFFFF_FFFF) as u32); // CTBA
    hdr.add(3).write_volatile((ct_phys >> 32) as u32);         // CTBAU

    port.command_list_phys = cl_phys;
    port.fis_phys = fis_phys;
    port.cmd_table_phys = ct_phys;

    port_write(mmio, port_idx, PORT_CLB, (cl_phys & 0xFFFF_FFFF) as u32);
    port_write(mmio, port_idx, PORT_CLBU, (cl_phys >> 32) as u32);
    port_write(mmio, port_idx, PORT_FB, (fis_phys & 0xFFFF_FFFF) as u32);
    port_write(mmio, port_idx, PORT_FBU, (fis_phys >> 32) as u32);
    // Errores heredados del arranque: se limpian antes de empezar (los bits de
    // PxSERR son "escribe 1 para borrar").
    port_write(mmio, port_idx, PORT_SERR, port_read(mmio, port_idx, PORT_SERR));
    port_write(mmio, port_idx, PORT_IS, port_read(mmio, port_idx, PORT_IS));

    if !port_start(mmio, port_idx) {
        hal.log("[ahci] el puerto no arranca\n");
        return false;
    }
    true
}

// ── Un comando ──────────────────────────────────────────────────────────────

/// Espera a que el disco suelte BSY y DRQ: hasta entonces no acepta órdenes.
unsafe fn wait_ready(mmio: u64, port: u8) -> bool {
    let mut spun = 0u32;
    while spun < PORT_TIMEOUT {
        let tfd = port_read(mmio, port, PORT_TFD);
        if tfd & (TFD_BSY | TFD_DRQ) == 0 { return true; }
        spun += 1;
        core::hint::spin_loop();
    }
    false
}

/// Arma y ejecuta un comando ATA de DMA sobre la ranura 0.
///
/// `buf_phys` es una dirección **FÍSICA**: es el HBA quien va a leerla o
/// escribirla, y el HBA no conoce el mapa de memoria del kernel.
unsafe fn run_dma_command(
    port_idx: u8,
    command: u8,
    lba: u64,
    sector_count: u16,
    buf_phys: u64,
    bytes: u32,
    write: bool,
) -> Result<u16, DiskError> {
    #[allow(static_mut_refs)]
    let ctrl = match CONTROLLER.as_ref() { Some(c) => c, None => return Err(DiskError::NotReady) };
    if port_idx >= 32 { return Err(DiskError::NotReady); }
    let port = &ctrl.ports[port_idx as usize];
    if port.command_list_phys == 0 || port.cmd_table_phys == 0 {
        return Err(DiskError::NotReady);
    }
    // Una entrada de PRDT admite 4 MiB; con una sola entrada ese es el techo.
    if bytes == 0 || bytes > 4 * 1024 * 1024 { return Err(DiskError::BadRequest); }
    // El buffer de DMA debe estar alineado a 2 bytes. En la práctica siempre
    // llega alineado a página, pero comprobarlo es gratis.
    if buf_phys & 1 != 0 { return Err(DiskError::BadRequest); }

    let mmio = ctrl.mmio_base;
    let hal = storage_hal::hal();

    if !wait_ready(mmio, port_idx) { return Err(DiskError::Busy); }

    let hdr = hal.phys_to_virt(port.command_list_phys) as *mut u32;
    let ct = hal.phys_to_virt(port.cmd_table_phys) as *mut u8;

    // ── Command Table: el FIS de mando (Host to Device, registro) ──
    core::ptr::write_bytes(ct, 0, 0x80 + 16); // FIS + hueco ATAPI + 1 PRDT
    ct.add(0).write_volatile(FIS_TYPE_REG_H2D);
    ct.add(1).write_volatile(0x80); // C=1: esto es un comando, no una actualización
    ct.add(2).write_volatile(command);
    ct.add(3).write_volatile(0);    // features (bajo)
    let l = lba.to_le_bytes();
    ct.add(4).write_volatile(l[0]);
    ct.add(5).write_volatile(l[1]);
    ct.add(6).write_volatile(l[2]);
    // Device: bit 6 = modo LBA. Sin él, el disco interpreta CHS.
    ct.add(7).write_volatile(0x40);
    ct.add(8).write_volatile(l[3]);
    ct.add(9).write_volatile(l[4]);
    ct.add(10).write_volatile(l[5]);
    ct.add(11).write_volatile(0);   // features (alto)
    ct.add(12).write_volatile((sector_count & 0xFF) as u8);
    ct.add(13).write_volatile((sector_count >> 8) as u8);
    ct.add(14).write_volatile(0);   // ICC
    ct.add(15).write_volatile(0);   // control

    // ── PRDT (byte 0x80): a dónde van los datos ──
    let prdt = ct.add(0x80) as *mut u32;
    prdt.add(0).write_volatile((buf_phys & 0xFFFF_FFFF) as u32);
    prdt.add(1).write_volatile((buf_phys >> 32) as u32);
    prdt.add(2).write_volatile(0);
    // DBC es el número de bytes MENOS UNO. Poner el número exacto pide un byte
    // de más — el error clásico de esta estructura.
    prdt.add(3).write_volatile((bytes - 1) & 0x003F_FFFF);

    // ── Cabecera de la ranura 0 ──
    // DW0: CFL (longitud del FIS en dwords) | W (escritura) | PRDTL (entradas)
    let cfl = 20u32 / 4; // el FIS H2D mide 20 bytes = 5 dwords
    let mut dw0 = cfl & 0x1F;
    if write { dw0 |= 1 << 6; }
    dw0 |= 1 << 16; // PRDTL = 1 entrada
    hdr.add(0).write_volatile(dw0);
    hdr.add(1).write_volatile(0); // PRDBC: lo rellena el HBA

    // Limpiar el estado anterior antes de tocar la campana.
    port_write(mmio, port_idx, PORT_IS, port_read(mmio, port_idx, PORT_IS));
    port_write(mmio, port_idx, PORT_SERR, port_read(mmio, port_idx, PORT_SERR));

    // ── Campana: ejecuta la ranura 0 ──
    port_write(mmio, port_idx, PORT_CI, 1);

    let mut spun = 0u32;
    loop {
        let ci = port_read(mmio, port_idx, PORT_CI);
        let is = port_read(mmio, port_idx, PORT_IS);
        if is & IS_TFES != 0 {
            let tfd = port_read(mmio, port_idx, PORT_TFD);
            port_write(mmio, port_idx, PORT_IS, is);
            return Err(DiskError::Device(tfd));
        }
        if ci & 1 == 0 { break; }
        spun += 1;
        if spun >= CMD_TIMEOUT { return Err(DiskError::Timeout); }
        core::hint::spin_loop();
    }
    let tfd = port_read(mmio, port_idx, PORT_TFD);
    if tfd & TFD_ERR != 0 { return Err(DiskError::Device(tfd)); }

    // PRDBC dice cuántos bytes movió DE VERDAD. Devolver "los que pedí" sin
    // mirarlo es la clase de mentira cómoda que este proyecto no admite.
    let moved = hdr.add(1).read_volatile();
    Ok((moved / SECTOR as u32) as u16)
}

/// Lee `sector_count` sectores desde `lba` al buffer FÍSICO `buf_phys`.
pub unsafe fn read_sectors_phys(port_idx: u8, lba: u64, sector_count: u16, buf_phys: u64)
    -> Result<u16, DiskError>
{
    if sector_count == 0 { return Err(DiskError::BadRequest); }
    let bytes = sector_count as u32 * SECTOR as u32;
    run_dma_command(port_idx, ATA_CMD_READ_DMA_EX, lba, sector_count, buf_phys, bytes, false)
}

/// Escribe `sector_count` sectores en `lba` desde el buffer FÍSICO `buf_phys`.
///
/// Existe porque un driver de disco a medias no es un driver. Que el kernel la
/// exponga o no —y a quién— es decisión suya, no de esta capa.
pub unsafe fn write_sectors_phys(port_idx: u8, lba: u64, sector_count: u16, buf_phys: u64)
    -> Result<u16, DiskError>
{
    if sector_count == 0 { return Err(DiskError::BadRequest); }
    let bytes = sector_count as u32 * SECTOR as u32;
    run_dma_command(port_idx, ATA_CMD_WRITE_DMA_EX, lba, sector_count, buf_phys, bytes, true)
}

/// IDENTIFY DEVICE: 512 bytes con el modelo, el número de serie y los sectores
/// del disco.
///
/// Es la forma de que BMO sepa A QUÉ DISCO le está hablando, en vez de fiarse
/// del orden de enumeración. Con dos discos en la máquina y el sistema del
/// dueño en uno de ellos, eso no es un lujo.
pub unsafe fn identify_phys(port_idx: u8, buf_phys: u64) -> Result<u16, DiskError> {
    // IDENTIFY entrega exactamente un sector y no usa LBA ni contador.
    run_dma_command(port_idx, ATA_CMD_IDENTIFY, 0, 1, buf_phys, SECTOR as u32, false)
}

pub fn controller() -> Option<&'static AhciController> {
    #[allow(static_mut_refs)]
    unsafe { CONTROLLER.as_ref() }
}

/// Olvida el controlador actual para poder probar OTRO.
///
/// Una placa puede traer más de un HBA SATA (el del chipset y alguno añadido),
/// y el disco que buscamos puede estar en el segundo. Sin esto, `probe` se
/// queda con el primero para siempre por su guarda de inicialización.
pub fn reset_ctrl() {
    unsafe { CONTROLLER = None; }
    INIT_DONE.store(false, Ordering::SeqCst);
}
