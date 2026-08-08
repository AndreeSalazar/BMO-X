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

// -- Registros del HBA -------------------------------------------------------

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

/// Task File Data: el disco esta ocupado, o pidiendo/entregando datos.
const TFD_BSY: u32 = 1 << 7;
const TFD_DRQ: u32 = 1 << 3;
const TFD_ERR: u32 = 1 << 0;
/// Interrupt Status: Task File Error.
const IS_TFES: u32 = 1 << 30;

const FIS_TYPE_REG_H2D: u8 = 0x27;
const ATA_CMD_READ_DMA_EX:  u8 = 0x25;
const ATA_CMD_WRITE_DMA_EX: u8 = 0x35;
const ATA_CMD_IDENTIFY:     u8 = 0xEC;
/// FLUSH CACHE EXT: obliga al disco a bajar a la superficie lo que acepto y
/// tiene todavia en su cache. Un `WRITE DMA` que devuelve OK solo promete que
/// el disco se quedo con los datos, no que sobrevivan a un corte.
const ATA_CMD_FLUSH_EXT:    u8 = 0xEA;

/// Firma que deja un disco duro SATA en `PxSIG`. Un 0xEB140101 seria una
/// unidad optica (ATAPI) y un 0xFFFF0000, un puerto sin nada.
pub const SIG_SATA_DISK: u32 = 0x0000_0101;

/// Bytes por sector logico. Todo LBA de este driver es de 512 B.
pub const SECTOR: usize = 512;

/// Espera maxima para que un comando termine, en iteraciones de sondeo. Un
/// disco dormido puede tardar; un SSD contesta en microsegundos. El numero es
/// generoso a proposito: el limite existe para que un puerto MUERTO no cuelgue
/// la maquina, no para cronometrar al disco.
const CMD_TIMEOUT: u32 = 20_000_000;
/// Espera para los cambios de estado del puerto (arranque/parada del motor de
/// comandos), que son inmediatos salvo averia.
const PORT_TIMEOUT: u32 = 1_000_000;


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

static mut CONTROLLER: Option<AhciController> = None;
static INIT_DONE: AtomicBool = AtomicBool::new(false);

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

// -- Arranque del controlador ------------------------------------------------

/// Prepara el HBA y hace censo de sus puertos. No toca ningun disco.
pub unsafe fn probe(mmio_base: u64) -> bool {
    if INIT_DONE.swap(true, Ordering::SeqCst) { return true; }
    let hal = storage_hal::hal();
    hal.log("[ahci] probing HBA\n");

    // * SIN RESET DEL HBA, a proposito.
    //
    // La version anterior hacia `GHC.HR` nada mas entrar y leia el estado de
    // los puertos justo despues: cero puertos con disco, siempre. Normal -- un
    // reset del HBA TIRA TODOS LOS ENLACES SATA, y renegociar un enlace lleva
    // decenas de milisegundos. Era preguntar "hay alguien?" un microsegundo
    // despues de colgar el telefono.
    //
    // Y el reset no hacia falta para nada: el firmware UEFI ya arranco este
    // HBA y dejo los enlaces establecidos para poder leer el arranque. Lo
    // unico que hay que asegurar es el modo (algunas placas lo dejan en modo
    // compatible IDE, donde los registros no significan lo que creemos) y
    // apagar las interrupciones, porque este driver sondea.
    hba_write(mmio_base, HBA_GHC, hba_read(mmio_base, HBA_GHC) | GHC_AE);
    hba_write(mmio_base, HBA_GHC, hba_read(mmio_base, HBA_GHC) & !GHC_IE);
    hba_write(mmio_base, HBA_IS, hba_read(mmio_base, HBA_IS)); // limpiar pendientes

    let cap = hba_read(mmio_base, HBA_CAP);
    // * `CAP.NP` son los bits [4:0]. NO los [24:20].
    //
    // Esto leia `(cap >> 20) & 0x1F`, que en este HBA (cap=0xEF36FF27) cae
    // encima de `ISS` --la velocidad de interfaz soportada-- y daba 0x13: el
    // driver se creia 20 puertos en un controlador de 8. Los puertos 8..19 no
    // existen, y su espacio MMIO **alias-ea sobre los reales**: por eso el
    // puerto 0x12 reportaba exactamente lo mismo que el 0x2 (ssts=0x133,
    // sig=0x101). No eran dos discos ni un doble volcado del log -- era el
    // MISMO registro leido dos veces por dos direcciones distintas.
    //
    // Y no era solo ruido en pantalla: `port_link_up` ESCRIBE. Cada puerto
    // fantasma le mandaba un COMRESET por `PxSCTL` a un puerto real que ya
    // estaba levantado, despues del censo. Tirar el enlace del disco justo
    // despues de encontrarlo es una forma perfecta de que "a veces arranca".
    //
    // Windows nunca ensena esto porque lee `NP` de donde toca y ademas solo
    // toca los puertos que `PI` declara.
    let port_count = (cap & 0x1F) as u8 + 1;
    let pi = hba_read(mmio_base, HBA_PI);
    // Los registros del HBA, dichos en voz alta. Si CAP y PI salen 0x0 o
    // 0xFFFFFFFF, el problema no son los puertos: es que no estamos leyendo
    // el HBA (BAR equivocada, MMIO sin mapear). Sin estos dos numeros,
    // "ningun puerto tiene disco" es una conclusion sin pruebas.
    hal.log_hex("[ahci] cap=", cap as u64);
    hal.log_hex(" pi=", pi as u64);
    hal.log_hex(" ghc=", hba_read(mmio_base, HBA_GHC) as u64);
    // `np` explicito: es el numero que decide cuantos puertos se tocan, y
    // haberlo leido mal costo doce puertos fantasma. Si vuelve a salir raro,
    // sale ANTES que sus consecuencias.
    hal.log_hex(" np=", port_count as u64);
    hal.log("\n");

    let sss = cap & (1 << 27) != 0;

    let mut ctrl = AhciController {
        mmio_base, port_count, ports_implemented: pi,
        ports: [AhciPort {
            port_number: 0, state: PortState::Empty, signature: 0, ssts: 0, sctl: 0, cmd: 0,
            command_list_phys: 0, fis_phys: 0, cmd_table_phys: 0,
        }; 32],
    };

    // Primer intento: SUAVE. Se respeta lo que dejo el firmware y solo se
    // renegocia el enlace de los puertos que esten caidos.
    let mut active = census(&mut ctrl, pi, sss);

    // Segundo intento: EL MARTILLO. Si NINGUN puerto levanto enlace, la
    // hipotesis cambia -- no es que los discos no esten, es que el firmware
    // dejo el controlador en un estado del que no sabemos sacarlo puerto a
    // puerto. Ahi si toca resetear el HBA entero y rehacer el trabajo, esta
    // vez esperando de verdad a que los enlaces vuelvan.
    //
    // Suave primero y martillo despues, nunca al reves: el reset destruye el
    // trabajo que el firmware ya hizo, y si ese trabajo servia, mejor no
    // tocarlo.
    if active == 0 {
        hal.log("[ahci] ningun enlace: reset completo del HBA y reintento\n");
        hba_write(mmio_base, HBA_GHC, hba_read(mmio_base, HBA_GHC) | GHC_HR);
        let mut spun = 0u32;
        while hba_read(mmio_base, HBA_GHC) & GHC_HR != 0 && spun < PORT_TIMEOUT {
            spun += 1;
            core::hint::spin_loop();
        }
        // El reset apaga AE: sin el, los registros dejan de significar lo que
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

/// Levanta el enlace de cada puerto y anota su estado. Devuelve cuantos
/// quedaron con enlace vivo.
///
/// * NO SE CONFIA EN `PI`. El registro de puertos implementados lo escribe el
/// firmware, y el firmware se equivoca: hay un caso conocido en Linux (Acer
/// Switch Alpha 12) donde la BIOS reporta un mapa que hace al driver SALTARSE
/// justo el puerto donde esta el disco, y el arreglo del kernel es ignorar el
/// registro y forzar el valor bueno a mano. Aqui se recorren TODOS los puertos
/// que `CAP.NP` dice que existen y se anota si `PI` los declaraba o no:
/// saltarse el puerto del disco es peor que mirar uno de mas. En esta maquina
/// eso se gana el pan -- el disco aparece en el puerto 2, que `PI=0x33` no
/// declara.
///
/// * PERO EL LIMITE ES `CAP.NP`, Y ES UN LIMITE DURO. Lo que si es danino es
/// pasarse de ahi: el espacio de puertos alias-ea, asi que un puerto que no
/// existe no devuelve ceros, devuelve **otro puerto**, y escribirle es
/// escribirle a ese. Desconfiar de `PI` es una decision; desconfiar de `NP`
/// seria mandarle COMRESET a un disco vivo creyendo que se le habla al vacio.
unsafe fn census(ctrl: &mut AhciController, pi: u32, sss: bool) -> u32 {
    let hal = storage_hal::hal();
    let mmio_base = ctrl.mmio_base;
    let np = ctrl.port_count.min(32);
    let mut active = 0u32;
    let mut vacios = 0u32;
    for i in 0..np {
        let declared = pi & (1 << i) != 0;
        let ssts = port_link_up(mmio_base, i, sss);
        // Cada puerto dice su estado CRUDO aqui, en el driver, que es quien lo
        // tiene delante. El `!` marca los que `PI` NO declaraba: si uno de
        // esos trae disco, el firmware estaba mintiendo.
        //
        // Solo se imprimen los puertos que tienen ALGO que contar. Un HBA con
        // 32 puertos y un disco escupia treinta lineas de ceros identicas que
        // barrian el arranque entero fuera del panel -- y el panel es la unica
        // ventana que hay, porque aqui no se puede hacer scroll hacia atras.
        // Los vacios se cuentan y se resumen en una sola linea al final: el
        // numero sigue estando, que es lo que importaba.
        let algo = ssts != 0
            || port_read(mmio_base, i, PORT_CMD) != 0
            || port_read(mmio_base, i, PORT_SIG) != 0;
        if algo {
            hal.log(if declared { "[ahci] p" } else { "[ahci] !p" });
            hal.log_hex("", i as u64);
            hal.log_hex(" ssts=", ssts as u64);
            hal.log_hex(" cmd=", port_read(mmio_base, i, PORT_CMD) as u64);
            hal.log_hex(" sctl=", port_read(mmio_base, i, PORT_SCTL) as u64);
            hal.log_hex(" sig=", port_read(mmio_base, i, PORT_SIG) as u64);
            hal.log("\n");
        } else {
            vacios += 1;
        }
        // DET=3 es "dispositivo presente y comunicacion establecida": el unico
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
    // El resumen de los callados. El dato no se pierde: si algun dia "faltan"
    // puertos, este numero dice cuantos se miraron y estaban en cero.
    if vacios > 0 {
        hal.log_hex("[ahci] puertos vacios (no se listan): ", vacios as u64);
        hal.log("\n");
    }
    active
}

/// Levanta el enlace SATA de un puerto y devuelve su `PxSSTS` final.
///
/// * POR QUE HACE FALTA: el firmware UEFI uso este disco para arrancarnos y
/// despues, al salir, PARO los puertos -- se ve en `PxCMD` con ST y FRE a
/// cero. Un enlace parado reporta `DET=0`, que es indistinguible de "aqui no
/// hay nada". Encender el disco (SUD) no basta: hay que renegociar el enlace,
/// y eso se pide con un COMRESET por `PxSCTL`.
///
/// La secuencia es la de la especificacion, y cada espera es de TIEMPO REAL:
/// contar vueltas de bucle mide la velocidad del CPU, no los milisegundos que
/// el SATA necesita.
unsafe fn port_link_up(mmio: u64, port: u8, sss: bool) -> u32 {
    let hal = storage_hal::hal();

    // 1. Con el motor de comandos andando no se toca el PHY.
    port_stop(mmio, port);

    // 2. Arrancar el disco si el HBA usa spin-up escalonado (CAP.SSS): con el,
    //    un puerto no negocia nada hasta que se le pide. SUD = Spin-Up Device,
    //    POD = Power On Device.
    if sss {
        let cmd = port_read(mmio, port, PORT_CMD);
        port_write(mmio, port, PORT_CMD, cmd | (1 << 1) | (1 << 2));
        hal.delay_ms(10);
    }

    // 3. Ya esta? Si el enlace vino vivo del firmware, aqui se acaba.
    let ssts = port_read(mmio, port, PORT_SSTS);
    if ssts & SSTS_DET == 0x03 {
        port_write(mmio, port, PORT_SERR, port_read(mmio, port, PORT_SERR));
        return ssts;
    }

    // 4. COMRESET: DET=1 fuerza la renegociacion, y hay que sostenerlo al
    //    menos 1 ms antes de soltarlo a 0.
    let sctl = port_read(mmio, port, PORT_SCTL);
    port_write(mmio, port, PORT_SCTL, (sctl & !0xF) | 0x1);
    hal.delay_ms(2);
    port_write(mmio, port, PORT_SCTL, sctl & !0xF);

    // 5. Esperar el enlace. Un SSD contesta en milisegundos; a un disco
    //    mecanico dormido se le conceden hasta 1,5 s antes de darlo por vacio.
    let mut ssts = 0u32;
    for _ in 0..150 {
        ssts = port_read(mmio, port, PORT_SSTS);
        if ssts & SSTS_DET == 0x03 { break; }
        hal.delay_ms(10);
    }
    // 6. La negociacion deja errores de estreno en PxSERR: se limpian, o el
    //    primer comando nacera con un error que no es suyo.
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

/// Arranca el puerto en el orden que manda la especificacion: primero recibir
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

    // Una pagina por estructura. Sobra sitio (la lista son 1 KiB y el area de
    // FIS 256 B), pero la pagina es la unidad que entrega el asignador y asi
    // las alineaciones que exige el HBA (1 KiB / 256 B / 128 B) salen solas.
    let cl_phys = match hal.alloc_dma_pages(1) { Some(p) => p, None => return false };
    let fis_phys = match hal.alloc_dma_pages(1) { Some(p) => p, None => return false };
    let ct_phys = match hal.alloc_dma_pages(1) { Some(p) => p, None => return false };
    core::ptr::write_bytes(hal.phys_to_virt(cl_phys), 0, 4096);
    core::ptr::write_bytes(hal.phys_to_virt(fis_phys), 0, 4096);
    core::ptr::write_bytes(hal.phys_to_virt(ct_phys), 0, 4096);

    // Cabecera de la ranura 0 -> donde esta SU command table. Esto es lo que
    // faltaba: sin CTBA en la cabecera, el HBA busca la orden en la direccion
    // fisica 0.
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

// -- Un comando --------------------------------------------------------------

/// Espera a que el disco suelte BSY y DRQ: hasta entonces no acepta ordenes.
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

/// Arma y ejecuta un comando ATA sobre la ranura 0.
///
/// `data` es `Some((direccion FISICA, bytes))` para los comandos que mueven
/// datos y `None` para los que no mueven ninguno (FLUSH CACHE). La direccion
/// es fisica porque es el HBA quien va a leerla o escribirla, y el HBA no
/// conoce el mapa de memoria del kernel.
unsafe fn run_command(
    port_idx: u8,
    command: u8,
    lba: u64,
    sector_count: u16,
    data: Option<(u64, u32)>,
    write: bool,
) -> Result<u16, DiskError> {
    #[allow(static_mut_refs)]
    let ctrl = match CONTROLLER.as_ref() { Some(c) => c, None => return Err(DiskError::NotReady) };
    if port_idx >= 32 { return Err(DiskError::NotReady); }
    let port = &ctrl.ports[port_idx as usize];
    if port.command_list_phys == 0 || port.cmd_table_phys == 0 {
        return Err(DiskError::NotReady);
    }
    if let Some((buf_phys, bytes)) = data {
        // Una entrada de PRDT admite 4 MiB; con una sola entrada ese es el techo.
        if bytes == 0 || bytes > 4 * 1024 * 1024 { return Err(DiskError::BadRequest); }
        // El buffer de DMA debe estar alineado a 2 bytes. En la practica siempre
        // llega alineado a pagina, pero comprobarlo es gratis.
        if buf_phys & 1 != 0 { return Err(DiskError::BadRequest); }
    }

    let mmio = ctrl.mmio_base;
    let hal = storage_hal::hal();

    if !wait_ready(mmio, port_idx) { return Err(DiskError::Busy); }

    let hdr = hal.phys_to_virt(port.command_list_phys) as *mut u32;
    let ct = hal.phys_to_virt(port.cmd_table_phys) as *mut u8;

    // -- Command Table: el FIS de mando (Host to Device, registro) --
    core::ptr::write_bytes(ct, 0, 0x80 + 16); // FIS + hueco ATAPI + 1 PRDT
    ct.add(0).write_volatile(FIS_TYPE_REG_H2D);
    ct.add(1).write_volatile(0x80); // C=1: esto es un comando, no una actualizacion
    ct.add(2).write_volatile(command);
    ct.add(3).write_volatile(0);    // features (bajo)
    let l = lba.to_le_bytes();
    ct.add(4).write_volatile(l[0]);
    ct.add(5).write_volatile(l[1]);
    ct.add(6).write_volatile(l[2]);
    // Device: bit 6 = modo LBA. Sin el, el disco interpreta CHS.
    ct.add(7).write_volatile(0x40);
    ct.add(8).write_volatile(l[3]);
    ct.add(9).write_volatile(l[4]);
    ct.add(10).write_volatile(l[5]);
    ct.add(11).write_volatile(0);   // features (alto)
    ct.add(12).write_volatile((sector_count & 0xFF) as u8);
    ct.add(13).write_volatile((sector_count >> 8) as u8);
    ct.add(14).write_volatile(0);   // ICC
    ct.add(15).write_volatile(0);   // control

    // -- PRDT (byte 0x80): a donde van los datos --
    // Un comando sin datos (FLUSH) no lleva ninguna entrada: PRDTL = 0 y aqui
    // no se escribe nada. Dejar un PRDT con direcciones viejas y decirle al HBA
    // que hay 0 entradas es correcto, pero dejarlo apuntando a algo Y declarar
    // una entrada seria mandarle a mover datos que nadie pidio.
    if let Some((buf_phys, bytes)) = data {
        let prdt = ct.add(0x80) as *mut u32;
        prdt.add(0).write_volatile((buf_phys & 0xFFFF_FFFF) as u32);
        prdt.add(1).write_volatile((buf_phys >> 32) as u32);
        prdt.add(2).write_volatile(0);
        // DBC es el numero de bytes MENOS UNO. Poner el numero exacto pide un
        // byte de mas -- el error clasico de esta estructura.
        prdt.add(3).write_volatile((bytes - 1) & 0x003F_FFFF);
    }

    // -- Cabecera de la ranura 0 --
    // DW0: CFL (longitud del FIS en dwords) | W (escritura) | PRDTL (entradas)
    let cfl = 20u32 / 4; // el FIS H2D mide 20 bytes = 5 dwords
    let mut dw0 = cfl & 0x1F;
    if write { dw0 |= 1 << 6; }
    if data.is_some() { dw0 |= 1 << 16; } // PRDTL = 1 entrada
    hdr.add(0).write_volatile(dw0);
    hdr.add(1).write_volatile(0); // PRDBC: lo rellena el HBA

    // Limpiar el estado anterior antes de tocar la campana.
    port_write(mmio, port_idx, PORT_IS, port_read(mmio, port_idx, PORT_IS));
    port_write(mmio, port_idx, PORT_SERR, port_read(mmio, port_idx, PORT_SERR));

    // -- Campana: ejecuta la ranura 0 --
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

    // Un comando sin datos no mueve sectores y no tiene nada que contar.
    if data.is_none() { return Ok(0); }

    // PRDBC dice cuantos bytes movio DE VERDAD. Devolver "los que pedi" sin
    // mirarlo es la clase de mentira comoda que este proyecto no admite.
    let moved = hdr.add(1).read_volatile();
    let sectors = (moved / SECTOR as u32) as u16;

    // Matiz honesto de la ESCRITURA: no todos los HBA actualizan PRDBC cuando
    // los datos van de la memoria AL disco (la spec lo pide, el silicio no
    // siempre obedece; Linux tampoco se fia de el en ese sentido). Si el disco
    // no reporto ERROR y el comando termino, la orden se cumplio: quien manda
    // aqui es TFD.ERR, no un contador opcional. Se dice en voz alta en vez de
    // devolver 0 y dejar creer que la escritura fallo.
    if write && sectors == 0 {
        storage_hal::hal().log("[ahci] el HBA no reporta PRDBC en escritura; vale el estado del disco\n");
        return Ok(sector_count);
    }
    Ok(sectors)
}

/// Lee `sector_count` sectores desde `lba` al buffer FISICO `buf_phys`.
pub unsafe fn read_sectors_phys(port_idx: u8, lba: u64, sector_count: u16, buf_phys: u64)
    -> Result<u16, DiskError>
{
    if sector_count == 0 { return Err(DiskError::BadRequest); }
    let bytes = sector_count as u32 * SECTOR as u32;
    run_command(port_idx, ATA_CMD_READ_DMA_EX, lba, sector_count, Some((buf_phys, bytes)), false)
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
    run_command(port_idx, ATA_CMD_WRITE_DMA_EX, lba, sector_count, Some((buf_phys, bytes)), true)
}

/// Ordena al disco bajar a la superficie todo lo que acepto y aun tiene en su
/// cache. Sin datos: es una orden, no una transferencia.
///
/// Un `WRITE DMA` que devuelve OK solo promete que el disco se quedo con los
/// bytes. Para una caja negra --que existe justamente para sobrevivir al corte
/// que se esta investigando-- esa promesa no basta: el punto de no retorno es
/// este comando.
pub unsafe fn flush_cache(port_idx: u8) -> Result<(), DiskError> {
    run_command(port_idx, ATA_CMD_FLUSH_EXT, 0, 0, None, false).map(|_| ())
}

/// IDENTIFY DEVICE: 512 bytes con el modelo, el numero de serie y los sectores
/// del disco.
///
/// Es la forma de que BMO sepa A QUE DISCO le esta hablando, en vez de fiarse
/// del orden de enumeracion. Con dos discos en la maquina y el sistema del
/// dueno en uno de ellos, eso no es un lujo.
pub unsafe fn identify_phys(port_idx: u8, buf_phys: u64) -> Result<u16, DiskError> {
    // IDENTIFY entrega exactamente un sector y no usa LBA ni contador.
    run_command(port_idx, ATA_CMD_IDENTIFY, 0, 1, Some((buf_phys, SECTOR as u32)), false)
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
