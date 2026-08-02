//! **El bus, no los aparatos.**
//!
//! Aquí vive lo que hay que hacer para que un endpoint de interrupción esté
//! listo, y es *idéntico* para un teclado, un ratón o cualquier otro HID:
//! descifrar descriptores, mandar `SET_CONFIGURATION`, configurar el endpoint
//! en el xHC, poner el protocolo boot y reservar el buffer de DMA.
//!
//! Estaba metido en la misma función que la decodificación de informes. Ese es
//! el reparto que se estaba pidiendo a gritos: **esto no sabe qué es un
//! teclado**, y [`crate::teclado`] no sabe qué es un puerto. Cuando entre un
//! tercer aparato —un mando, una tableta— no se toca nada de aquí.


/// Máximo de interfaces que consideramos por dispositivo (fijo, sin alloc: el
/// driver corre dentro de Ring 0 de BMO, que no tiene allocator).
pub const MAX_IFACES: usize = 8;
/// Tamaño máximo aceptado del config descriptor completo (fijo, sin alloc).
pub const MAX_CFG: usize = 512;

/// Clase HID de una interfaz, ya interpretada.
pub const CLASE_HID: u8 = 3;
pub const SUBCLASE_BOOT: u8 = 1;
pub const PROTO_TECLADO: u8 = 1;
pub const PROTO_RATON: u8 = 2;

/// Dos bytes en little-endian.
pub fn le_u16(buf: &[u8], off: usize) -> u16 {
    (buf[off] as u16) | ((buf[off + 1] as u16) << 8)
}

/// Las interfaces de un config descriptor completo:
/// `(numero, clase, subclase, protocolo)`.
pub fn interfaces(cfg: &[u8], out: &mut [(u8, u8, u8, u8); MAX_IFACES]) -> usize {
    let mut n = 0;
    let total = if cfg.len() >= 2 { le_u16(cfg, 2) as usize } else { 0 };
    let limit = if total > 0 && total <= cfg.len() { total } else { cfg.len() };
    let mut off = if !cfg.is_empty() { cfg[0] as usize } else { 9 };
    while off + 3 <= limit && n < MAX_IFACES {
        let len = cfg[off] as usize;
        let dtype = cfg[off + 1];
        if len < 2 || off + len > limit { break; }
        if dtype == 4 && len >= 9 {
            out[n] = (cfg[off + 2], cfg[off + 5], cfg[off + 6], cfg[off + 7]);
            n += 1;
        }
        off += len;
    }
    n
}

/// El endpoint de interrupción IN de UNA interfaz concreta:
/// `(direccion_del_endpoint, max_packet, bInterval, dci)`.
///
/// El seguimiento de `iface_actual` no es un detalle: los descriptores de
/// endpoint vienen sueltos detrás del de su interfaz, sin decir de quién son.
/// Sin llevar la cuenta, un teclado compuesto daría el endpoint de su interfaz
/// de medios para la de teclado — y ésa sólo habla si pulsas subir volumen.
pub fn intr_in(cfg: &[u8], iface_num: u8) -> Option<(u8, u16, u8, u8)> {
    let total = if cfg.len() >= 2 { le_u16(cfg, 2) as usize } else { 0 };
    let limit = if total > 0 && total <= cfg.len() { total } else { cfg.len() };
    let mut off = if !cfg.is_empty() { cfg[0] as usize } else { 9 };
    let mut iface_actual = 0u8;
    while off + 3 <= limit {
        let len = cfg[off] as usize;
        let dtype = cfg[off + 1];
        if len < 2 || off + len > limit { break; }
        if dtype == 4 && len >= 9 { iface_actual = cfg[off + 2]; }
        if dtype == 5 && len >= 7 && iface_actual == iface_num {
            let ep_addr = cfg[off + 2];
            let attr = cfg[off + 3];
            let mps = le_u16(cfg, off + 4);
            let interval = cfg[off + 6];
            // IN + tipo interrupción (bits 1:0 = 3)
            if (ep_addr & 0x80) != 0 && (attr & 3) == 3 {
                let ep_num = ep_addr & 0x0F;
                let dci = if ep_num == 0 { 1 } else { ep_num * 2 + 1 };
                return Some((ep_addr, mps, interval, dci));
            }
        }
        off += len;
    }
    None
}

/// **Cuánto mide el Report Descriptor de una interfaz**, según su descriptor
/// HID.
///
/// Detrás de cada interfaz HID viene un descriptor de clase (`bDescriptorType`
/// = 0x21) que dice qué descriptores subordinados tiene y de qué tamaño. Sin
/// leer esa longitud no se puede pedir el Report Descriptor: a un
/// `GET_DESCRIPTOR` hay que decirle cuántos bytes se esperan, y pedir de más a
/// un endpoint de control no es gratis en todos los aparatos.
///
/// Se busca **dentro de la interfaz pedida**, no el primero que aparezca: un
/// teclado compuesto trae dos descriptores HID y el de su interfaz de medios no
/// describe el mismo informe.
pub fn hid_report_len(cfg: &[u8], iface_num: u8) -> Option<u16> {
    let total = if cfg.len() >= 2 { le_u16(cfg, 2) as usize } else { 0 };
    let limit = if total > 0 && total <= cfg.len() { total } else { cfg.len() };
    let mut off = if !cfg.is_empty() { cfg[0] as usize } else { 9 };
    let mut iface_actual = 0xFFu8;
    while off + 3 <= limit {
        let len = cfg[off] as usize;
        let dtype = cfg[off + 1];
        if len < 2 || off + len > limit {
            break;
        }
        if dtype == 4 && len >= 9 {
            iface_actual = cfg[off + 2];
        }
        // 0x21 = HID. Su cabecera son 6 bytes y luego pares
        // `(bDescriptorType, wDescriptorLength)`; 0x22 es el Report Descriptor.
        if dtype == 0x21 && iface_actual == iface_num && len >= 9 {
            let mut i = 6;
            while i + 3 <= len {
                if cfg[off + i] == 0x22 {
                    return Some(le_u16(cfg, off + i + 1));
                }
                i += 3;
            }
        }
        off += len;
    }
    None
}

/// ¿Este aparato trae interfaz de teclado **y** de ratón?
///
/// Un aparato con las dos es un teclado compuesto: la de ratón son sus teclas
/// de medios. Uno que sólo trae la de ratón es un ratón de verdad. Distinguirlo
/// es lo que impide que las teclas de volumen de un teclado se lleven el puesto
/// del ratón.
pub fn es_compuesto(ifaces: &[(u8, u8, u8, u8)]) -> bool {
    let tiene = |p: u8| {
        ifaces
            .iter()
            .any(|(_, c, s, pr)| *c == CLASE_HID && *s == SUBCLASE_BOOT && *pr == p)
    };
    tiene(PROTO_TECLADO) && tiene(PROTO_RATON)
}

/// Deja un endpoint de interrupción LISTO para bombear, y reserva su buffer.
///
/// Devuelve `(buf_phys, buf_virt)`. Lo que NO hace, a propósito, es encolar la
/// primera transferencia: eso lo decide el llamante y va al final de toda la
/// enumeración. Un endpoint que empieza a postear informes mientras todavía se
/// enumera el puerto siguiente mete sus eventos en medio de los control
/// transfers del otro aparato — y ése fue el camino por el que el teclado y el
/// ratón enmudecieron los dos.
pub unsafe fn preparar_endpoint(
    slot: u8,
    dci: u8,
    mps: u16,
    interval: u8,
    iface: u8,
    cfg_val: u8,
) -> Option<(u64, *mut u8, u8)> {
    let h = bmo_xhci::hal();

    h.log_u64(" dci=", dci as u64);
    h.log_u64(" mps=", mps as u64);
    h.log_u64(" bInterval=", interval as u64);

    // SET_CONFIGURATION. Sin esto el firmware del aparato ni arranca — es lo
    // que enciende las luces de un ratón RGB, y por eso el RGB apagado fue la
    // pista de que a uno nunca se le había mandado.
    bmo_xhci::control_transfer(slot, 0x00, 0x09, cfg_val as u16, 0, &mut [], false);

    // `interval` es el bInterval CRUDO del descriptor; la conversión al
    // exponente que espera el Endpoint Context la hace `encode_interval` — el
    // frontend no debe adivinar codificaciones del controlador.
    if !bmo_xhci::configure_endpoint(slot, dci, 7, mps, interval) {
        h.log("[uhid] cfg_ep FAIL\n");
        return None;
    }
    // Lo que dice el xHC, no lo que creemos: 1 = Running.
    h.log_u64(" ep_state=", bmo_xhci::ep_state(slot, dci) as u64);

    // Protocolo BOOT: informes de formato fijo, sin tener que interpretar el
    // HID Report Descriptor (que es un parser entero).
    bmo_xhci::control_transfer(slot, 0x21, 0x0B, 0, iface as u16, &mut [], false);
    // SET_IDLE(0): que sólo informe cuando algo CAMBIE, no periódicamente.
    bmo_xhci::control_transfer(slot, 0x21, 0x0A, 0, iface as u16, &mut [], false);

    // ★ Y AHORA SE LE PREGUNTA EN QUÉ PROTOCOLO SE QUEDÓ.
    //
    // `SET_PROTOCOL` se mandaba y **nadie miraba si sirvió de algo**. Un aparato
    // que lo ignora sigue mandando su informe de protocolo de INFORME, que
    // empieza por un byte de Report ID — y entonces todo va corrido una
    // posición: los botones caen donde el driver espera el desplazamiento en X.
    //
    // Eso es exactamente lo que se vio en el Ryzen: `bot=0b01` fijo (el Report
    // ID, que nunca cambia), `x=0` (los botones, cero mientras no pulses) y la
    // `y` derivando sola al mover en horizontal. Y el síntoma que lo delató, en
    // palabras del dueño: *"muevo y no funciona, pero al hacer clic se mueve"* —
    // porque el byte de botones caía en el campo del movimiento.
    //
    // `GET_PROTOCOL` (0xA1, 0x03) devuelve 0 = Boot, 1 = Informe. Preguntarlo
    // cuesta un control transfer al arrancar y convierte una suposición en un
    // dato. Si el aparato no contesta, `0xFF`: quien decide qué hacer con eso
    // es el que descifra, no el que enumera.
    let mut prot = [0u8; 1];
    let n = bmo_xhci::control_transfer(slot, 0xA1, 0x03, 0, iface as u16, &mut prot, true);
    let protocolo = if n >= 1 { prot[0] } else { 0xFF };
    h.log_u64(" protocolo=", protocolo as u64);
    if protocolo == 1 {
        h.log(" (INFORME: el aparato ignoro el BOOT)");
    }

    let buf_phys = h.alloc_dma_pages(1)?;
    let buf_virt = h.phys_to_virt(buf_phys);
    core::ptr::write_bytes(buf_virt, 0, 4096);
    Some((buf_phys, buf_virt, protocolo))
}

/// **Le pide al aparato su Report Descriptor** y lo descifra.
///
/// Es la respuesta a la pregunta que se quedó abierta cuando el ratón confesó
/// `protocolo=0x1`: *¿los desplazamientos son de 8 o de 16 bits?*. Se contestaba
/// mirando ocho bytes crudos en un log y decidiendo a ojo. Aquí se pregunta.
///
/// `None` con su motivo dicho en cada salida. El llamante vuelve al formato
/// BOOT, que es un formato **correcto** para un aparato que respeta el
/// protocolo — lo que no era correcto es aplicarlo sin preguntar.
///
/// # Safety
/// Toca MMIO del xHC: hay que llamarlo con el CR3 del kernel puesto.
pub unsafe fn leer_formato_raton(
    slot: u8,
    iface: u8,
    cfg: &[u8],
) -> Option<crate::formato::Formato> {
    let h = bmo_xhci::hal();

    let largo = match hid_report_len(cfg, iface) {
        Some(l) if l as usize <= MAX_REPORT => l as usize,
        Some(l) => {
            h.log_u64("[uhid] report desc demasiado grande: ", l as u64);
            h.log("\n");
            return None;
        }
        None => {
            h.log("[uhid] la interfaz no declara Report Descriptor\n");
            return None;
        }
    };

    let mut desc = [0u8; MAX_REPORT];
    // GET_DESCRIPTOR estándar a la INTERFAZ (0x81), tipo 0x22 = Report.
    let n = bmo_xhci::control_transfer(slot, 0x81, 0x06, 0x2200, iface as u16, &mut desc[..largo], true);
    if n < largo {
        h.log_u64("[uhid] report desc corto: ", n as u64);
        h.log_u64(" de ", largo as u64);
        h.log("\n");
        return None;
    }

    match crate::formato::raton(&desc[..largo]) {
        Some(f) => {
            // Lo que decide el reparto de bytes, dicho en el arranque: si esto
            // no cuadra con lo que hace el puntero, la culpa es del parser y no
            // hay que volver a mirar el bus.
            h.log_u64("[uhid] formato del raton: id=", f.report_id as u64);
            h.log_u64(" x=bit", f.x.map_or(0, |c| c.bit) as u64);
            h.log_u64("/", f.x.map_or(0, |c| c.bits) as u64);
            h.log_u64("b  y=bit", f.y.map_or(0, |c| c.bit) as u64);
            h.log_u64("/", f.y.map_or(0, |c| c.bits) as u64);
            h.log_u64("b  informe=", f.bits as u64);
            h.log(" bits\n");
            if f.ejes_anchos() {
                h.log("[uhid] EJES DE MAS DE 8 BITS: el formato BOOT habria leido dy dentro de dx\n");
            }
            Some(f)
        }
        None => {
            h.log("[uhid] no entiendo su Report Descriptor: me quedo con el BOOT\n");
            None
        }
    }
}

/// Tope del Report Descriptor que se acepta. Los de ratón rondan los 60-200
/// bytes; 512 cubre cualquiera con macros sin dejar que un descriptor absurdo
/// se lleve la pila de Ring 0.
pub const MAX_REPORT: usize = 512;

/// Lee los descriptores de un dispositivo recién direccionado.
///
/// Devuelve `(cfg_val, longitud_util)` habiendo llenado `cfg`. Los reintentos
/// no son paranoia: la enumeración demostró ser inestable entre arranques — un
/// mismo binario da "no dev desc" en un encendido y enumera bien en el
/// siguiente. Un dispositivo recién reseteado puede no estar listo para el
/// primer control transfer.
pub unsafe fn leer_descriptores(slot: u8, cfg: &mut [u8; MAX_CFG]) -> Option<(u8, usize)> {
    let h = bmo_xhci::hal();

    let mut dev_desc = [0u8; 18];
    let mut n = 0usize;
    for _ in 0..3 {
        n = bmo_xhci::get_device_descriptor(slot, &mut dev_desc);
        if n >= 8 { break; }
        h.delay_ms(50);
    }
    if n < 8 {
        h.log("[uhid] no dev desc\n");
        return None;
    }
    h.log_u64(" class=", dev_desc[4] as u64);

    let mut cfg_hdr = [0u8; 9];
    let mut n2 = 0usize;
    for _ in 0..3 {
        n2 = bmo_xhci::get_config_descriptor(slot, 0, &mut cfg_hdr);
        if n2 >= 9 { break; }
        h.delay_ms(50);
    }
    if n2 < 9 {
        h.log("[uhid] no cfg hdr\n");
        return None;
    }
    let total_len = le_u16(&cfg_hdr, 2) as usize;
    let cfg_val = cfg_hdr[5];
    h.log_u64(" cfg_val=", cfg_val as u64);
    h.log_u64(" total_len=", total_len as u64);

    if total_len > MAX_CFG {
        h.log("[uhid] cfg too big\n");
        return None;
    }
    let n3 = bmo_xhci::get_config_descriptor(slot, 0, &mut cfg[..total_len]);
    if n3 < total_len {
        h.log("[uhid] cfg short\n");
        return None;
    }
    Some((cfg_val, total_len))
}

/// Enciende un puerto y direcciona lo que haya. `None` = ahí no hay nada, o no
/// se pudo.
///
/// ★ Los tres caminos de salida HABLAN. Eran `continue` mudos, y por eso hizo
/// falta más de una ronda de fotos para entender por qué el ratón no aparecía:
/// un puerto que falla al resetear, uno vacío y uno que no acepta dirección se
/// veían **exactamente igual**, o sea nada. El vacío sigue callado porque no es
/// un fallo.
pub unsafe fn direccionar_puerto(port: u8) -> Option<u8> {
    let h = bmo_xhci::hal();
    bmo_xhci::port_power_on(port);
    // Margen tras encender (chipset AMD).
    for _ in 0..50000 {
        core::hint::spin_loop();
    }
    if !bmo_xhci::port_reset(port) {
        h.log_u64("[uhid] puerto sin reset: ", port as u64);
        return None;
    }
    let speed = bmo_xhci::port_speed(port);
    if speed == 0 {
        return None;
    }
    h.log_u64("[uhid] puerto con algo: ", port as u64);
    match bmo_xhci::address_device(port, speed) {
        Some(s) => {
            h.log_u64("[uhid] slot=", s as u64);
            Some(s)
        }
        None => {
            h.log_u64("[uhid] NO acepta direccion, puerto ", port as u64);
            None
        }
    }
}
