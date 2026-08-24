//! **UN COMANDO**: emitirlo, esperarlo, y saber en que quedo.
//!
//! ## Por que soy un fichero (L6b)
//!
//! Porque esto es lo que corre **en cada lectura y en cada escritura** del
//! sistema, y `arranque.rs` corre una vez. Son dos vidas distintas y dos
//! presupuestos distintos.
//!
//! ## ** Y el presupuesto de este camino lo pone el APARATO, no el CPU
//!
//! `OPTIMIZACION_MAESTRO.md`, regla 1: antes de optimizar hay que decir quien
//! manda sobre el tiempo. Aqui manda el disco. Lo que se optimiza no es
//! calcular menos -- es **no esperar**: por eso existen `emitir` y `sondear`
//! separados de `run_command`, que es el bucle que gira.
//!
//! ** El reparto es MOVER TEXTO (L6d): ni una linea cambia de contenido.

// ** Del hermano y no del crate: los nombres del HBA --registros, banderas, el
// `CONTROLLER`-- viven en `controller.rs`, que es donde vive el hecho sobre el
// chip. `use super::*` traeria la raiz del crate, que no los tiene.
use super::controller::*;
use super::*;

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
/// En que va un comando que ya se emitio. Lo contesta [`sondear`].
///
/// == Por que existe: **preguntar no es esperar** ==
///
/// `run_command` armaba el comando, tocaba la campana y **se quedaba dentro**
/// girando hasta que el HBA contestara. Mientras tanto, quien llamo no podia
/// hacer nada -- y lo que es peor, nadie de fuera podia saber si habia algo en
/// vuelo.
///
/// Partirlo en EMITIR y SONDEAR no acelera el disco: lo que hace es que el
/// estado del comando **se pueda mirar desde fuera**. Sin eso no hay E/S
/// asincrona posible, porque "pedir sin esperar" es exactamente poder volver y
/// preguntar despues.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// El HBA sigue con ello.
    EnCurso,
    /// Termino. Lleva los sectores que movio DE VERDAD (`PRDBC`).
    Hecho(u16),
    Fallo(DiskError),
}

/// **Arma el comando y toca la campana. No espera.**
///
/// A partir de aqui la ranura 0 del puerto esta OCUPADA hasta que [`sondear`]
/// diga otra cosa. Quien emita otro comando encima pisa el que estaba en vuelo
/// -- este driver no tiene forma de impedirlo, y no debe tenerla: quien reparte
/// el disco es la capa de arriba (ver `ring0/dev/disk.rs`), igual que quien
/// reparte la pantalla es el compositor y no el framebuffer.
pub unsafe fn emitir(
    port_idx: u8,
    command: u8,
    lba: u64,
    sector_count: u16,
    data: Option<(u64, u32)>,
    write: bool,
) -> Result<(), DiskError> {
    // Features a cero: de los comandos que este driver sabe emitir, el unico que
    // lo usa es `DATA SET MANAGEMENT`, y ese no se pide sin esperar -- recortar
    // es una tanda detras de otra, no una lectura que se pueda solapar con otra
    // cosa. Ver `trim_phys`.
    armar(port_idx, command, 0, lba, sector_count, data, write)
}

/// **En que va el comando de la ranura 0?** No espera, no gira: mira y contesta.
///
/// `con_datos` dice si el comando movia bytes. Un FLUSH no mueve ninguno y no
/// tiene `PRDBC` que leer; preguntarselo devolveria cero y se leeria como "no
/// movio nada", que para un FLUSH es cierto y desconcertante.
pub unsafe fn sondear(port_idx: u8, con_datos: bool, write: bool) -> Estado {
    #[allow(static_mut_refs)]
    let ctrl = match CONTROLLER.as_ref() { Some(c) => c, None => return Estado::Fallo(DiskError::NotReady) };
    if port_idx >= 32 { return Estado::Fallo(DiskError::NotReady); }
    let port = &ctrl.ports[port_idx as usize];
    let mmio = ctrl.mmio_base;

    let ci = port_read(mmio, port_idx, PORT_CI);
    let is = port_read(mmio, port_idx, PORT_IS);
    if is & IS_TFES != 0 {
        let tfd = port_read(mmio, port_idx, PORT_TFD);
        port_write(mmio, port_idx, PORT_IS, is);
        return Estado::Fallo(DiskError::Device(tfd));
    }
    if ci & 1 != 0 {
        return Estado::EnCurso;
    }
    let tfd = port_read(mmio, port_idx, PORT_TFD);
    if tfd & TFD_ERR != 0 { return Estado::Fallo(DiskError::Device(tfd)); }
    if !con_datos { return Estado::Hecho(0); }

    let hal = storage_hal::hal();
    let hdr = hal.phys_to_virt(port.command_list_phys) as *mut u32;
    let moved = hdr.add(1).read_volatile();
    let sectors = (moved / SECTOR as u32) as u16;
    if write && sectors == 0 {
        // Ver la nota de `run_command`: no todos los HBA actualizan PRDBC en
        // escritura. Manda TFD.ERR, no un contador opcional.
        hal.log("[ahci] el HBA no reporta PRDBC en escritura; vale el estado del disco\n");
        return Estado::Hecho(u16::MAX);
    }
    Estado::Hecho(sectors)
}

/// Emite y **espera girando** hasta que termine. Es [`emitir`] mas [`sondear`]
/// en un bucle, y se queda porque casi todos sus usuarios --montar, leer la
/// GPT, el arranque-- no tienen a donde ir mientras tanto.
// ** `pub(crate)` al repartir: "privada" dejo de significar "de este driver" y
// paso a significar "de este modulo". Es la unica linea que este reparto no
// pudo dejar igual, y se dice en vez de cambiarla callando (L6d).
pub(crate) unsafe fn run_command(
    port_idx: u8,
    command: u8,
    features: u16,
    lba: u64,
    sector_count: u16,
    data: Option<(u64, u32)>,
    write: bool,
) -> Result<u16, DiskError> {
    run_command_hasta(port_idx, command, features, lba, sector_count, data, write, CMD_TIMEOUT)
}

/// Igual, pero con **el presupuesto de espera dicho por quien manda el comando**.
///
/// === Por que un comando elige su propia paciencia ===
///
/// [`CMD_TIMEOUT`] se eligio para que **un puerto MUERTO no cuelgue la maquina**,
/// no para cronometrar al disco -- y con eso en la cabeza, el mismo numero valia
/// para todo lo que este driver sabia pedir: leer, escribir, IDENTIFY y FLUSH.
/// Los cuatro le piden al aparato un trabajo acotado y parecido.
///
/// ** `DATA SET MANAGEMENT` rompe esa familia. Una sola orden puede cubrir
/// **GIGABYTES** de disco, y lo que el aparato hace con ella --tocar sus tablas
/// de traduccion, marcar bloques enteros como libres-- no se parece en nada a
/// mover 4 KiB. La spec **no acota cuanto puede tardar**.
///
/// Compartir el presupuesto de una lectura no era una decision: era que nadie
/// habia tenido que pensarlo todavia. Cada clase de comando dice el suyo.
///
/// [!] Sigue siendo un CONTADOR DE VUELTAS y no un tiempo. No se traduce a
/// milisegundos a proposito: el que gira aqui no tiene reloj, y meterle uno
/// seria darle al driver una dependencia nueva para una espera que casi nunca
/// se agota.
#[allow(clippy::too_many_arguments)]
// ** `pub(crate)` al repartir: ver la nota de `run_command`.
pub(crate) unsafe fn run_command_hasta(
    port_idx: u8,
    command: u8,
    features: u16,
    lba: u64,
    sector_count: u16,
    data: Option<(u64, u32)>,
    write: bool,
    limite: u32,
) -> Result<u16, DiskError> {
    use core::sync::atomic::Ordering;
    // La marca ANTES de emitir: lo que cuenta es un aviso posterior a esto.
    // Tomarla despues perderia el aviso de un disco rapido que contesta entre
    // la campana y la primera vuelta.
    let marca = AVISOS.load(Ordering::Acquire);
    armar(port_idx, command, features, lba, sector_count, data, write)?;
    let mut spun = 0u32;
    loop {
        // ** ESCUCHAR ES BARATO; PREGUNTAR, NO.
        //
        // `sondear` lee tres registros por MMIO, y **el MMIO no pasa por
        // cache**: cada lectura es un viaje al chipset. Girar sobre eso son
        // millones de viajes para averiguar algo que el aparato sabia desde el
        // primer microsegundo.
        //
        // `AVISOS` es memoria normal: leerlo sale de cache y no molesta a nadie.
        // Asi que se mira eso, y solo se pregunta de verdad cuando el aparato ha
        // dicho algo.
        if AVISOS.load(Ordering::Acquire) != marca {
            match sondear(port_idx, data.is_some(), write) {
                Estado::Hecho(n) => return Ok(if n == u16::MAX { sector_count } else { n }),
                Estado::Fallo(e) => return Err(e),
                // Aviso de otra cosa: se sigue esperando el nuestro.
                Estado::EnCurso => {}
            }
        }
        spun += 1;
        if spun >= limite { return Err(DiskError::Timeout); }
        // ** Y LA RED DE SEGURIDAD, que es lo que permite encender todo esto.
        //
        // Cada tantas vueltas se pregunta por MMIO **aunque no haya habido
        // aviso**. Si la placa no enruta MSI, si el firmware dejo el vector
        // enmascarado, o si el aviso se perdio, el disco sigue funcionando
        // exactamente como antes -- mas lento en esa vuelta y nada mas.
        //
        // Un camino nuevo que solo funciona cuando el hardware colabora no puede
        // ser el UNICO camino: la placa que no colabore se quedaria sin disco, o
        // sea sin arrancar, y el sintoma no se pareceria a la causa.
        if spun % 4096 == 0 {
            match sondear(port_idx, data.is_some(), write) {
                Estado::Hecho(n) => return Ok(if n == u16::MAX { sector_count } else { n }),
                Estado::Fallo(e) => return Err(e),
                Estado::EnCurso => {}
            }
        }
        core::hint::spin_loop();
    }
}

/// Lo que hay que escribir para que el comando exista. Sin esperar a nada.
///
/// ** `features` estuvo escrito a cero durante toda la vida de este driver, y
/// era cierto para los cuatro comandos que sabia mandar (leer, escribir, FLUSH,
/// IDENTIFY: ninguno lo usa). `DATA SET MANAGEMENT` es el primero que **elige
/// que hace** por ese registro -- sin el bit 0, la orden no es un TRIM.
unsafe fn armar(
    port_idx: u8,
    command: u8,
    features: u16,
    lba: u64,
    sector_count: u16,
    data: Option<(u64, u32)>,
    write: bool,
) -> Result<(), DiskError> {
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
    ct.add(3).write_volatile((features & 0xFF) as u8); // features (bajo)
    let l = lba.to_le_bytes();
    ct.add(4).write_volatile(l[0]);
    ct.add(5).write_volatile(l[1]);
    ct.add(6).write_volatile(l[2]);
    // Device: bit 6 = modo LBA. Sin el, el disco interpreta CHS.
    ct.add(7).write_volatile(0x40);
    ct.add(8).write_volatile(l[3]);
    ct.add(9).write_volatile(l[4]);
    ct.add(10).write_volatile(l[5]);
    ct.add(11).write_volatile((features >> 8) as u8); // features (alto)
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
    //
    // A partir de esta escritura hay un comando EN VUELO, y el resultado se
    // recoge con `sondear`. Lo que valga `PRDBC` --cuantos bytes movio DE
    // VERDAD, que es lo que se contesta en vez de "los que pedi"-- lo lee esa
    // funcion desde la misma cabecera.
    port_write(mmio, port_idx, PORT_CI, 1);
    let _ = hdr;
    Ok(())
}
