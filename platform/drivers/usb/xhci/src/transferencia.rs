//! **HABLAR CON UN APARATO QUE YA TIENE DIRECCION.**
//!
//! Transferencias de control, descriptores, configurar endpoints, resucitar uno
//! parado, encolar una IN de interrupcion, y el sondeo que no bloquea.
//!
//! ## Por que soy un fichero (L6b)
//!
//! Porque `enumerar.rs` acaba donde este empieza: alli el aparato **consigue un
//! nombre**, y aqui **se le habla**. Son dos preguntas, y la segunda es la que
//! se repite un millon de veces mientras la maquina esta encendida.
//!
//! ## *** Y AQUI VIVE LA LECCION MAS CARA DEL DRIVER
//!
//! **El evento ES EL PERMISO para volver a encolar.** Perder un evento de un
//! endpoint de interrupcion no pierde una pulsacion: **para la bomba**, y el
//! teclado se queda mudo hasta que alguien reinicia. Por eso existe
//! `RESUCITAR UN ENDPOINT PARADO`, y por eso esa seccion esta en este fichero y
//! no en otro: quien lea como se encola tiene que ver, en la misma pagina, que
//! pasa cuando la cola se para.
//!
//! ** El reparto es MOVER TEXTO (L6d): ni una linea cambia de contenido.

use super::*;

// ===================================================================
//  Control Transfer -- uses per-slot EP0 ring
// ===================================================================

pub unsafe fn control_transfer(slot: u8, bm_req_type: u8, b_request: u8,
    w_value: u16, w_index: u16, buf: &mut [u8], data_in: bool) -> usize
{
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return 0 };
    let h = hal();
    let ep0 = match ep0_mut(slot) { Some(e) => e, None => { h.log("no ep0 ring\n"); return 0; } };

    let has_data = !buf.is_empty();
    let data_page = if has_data {
        // Quedarse sin paginas DMA se trataba igual que "el aparato no mando
        // nada": devolver 0. Dos causas opuestas --una es memoria del sistema,
        // la otra es el periferico-- con la misma cara, y sin una linea. La de
        // arriba (`no ep0 ring`) si gritaba; esta no. Ahora las dos.
        let dp = h.alloc_dma_pages(1).unwrap_or(0);
        if dp == 0 {
            h.log("[xhci] control_transfer: SIN PAGINAS DMA (no es el aparato, es la memoria)\n");
            return 0;
        }
        if !data_in {
            let dv = h.phys_to_virt(dp);
            for i in 0..buf.len() { dv.add(i).write_volatile(buf[i]); }
        }
        dp
    } else { 0 };

    let trt = if !has_data { 0u32 } else if data_in { 3u32 } else { 2u32 };

    // Setup Stage. Spec 4.11.2.2: Setup/Data/Status son TDs SEPARADOS --
    // CH=0 en cada stage. Encadenarlos (CH=1) lo tolera QEMU pero el
    // silicio real (AMD) responde con Transaction Error (cc=4).
    let setup = Trb {
        dw0: (bm_req_type as u32) | ((b_request as u32) << 8) | ((w_value as u32) << 16),
        dw1: (w_index as u32) | ((buf.len() as u32) << 16),
        dw2: 8,
        dw3: (TRB_SETUP << 10) | (1 << 6) | (trt << 16), // IDT; sin CH
    };
    let s_idx = ep0.enqueue;
    let sb = s_idx * 4;
    ep0.ring_virt.add(sb).write_volatile(setup.dw0);
    ep0.ring_virt.add(sb + 1).write_volatile(setup.dw1);
    ep0.ring_virt.add(sb + 2).write_volatile(setup.dw2);
    ep0.ring_virt.add(sb + 3).write_volatile(setup.dw3 | if ep0.pcs { 1 } else { 0 });
    ep0.enqueue = s_idx + 1;
    if ep0.enqueue >= LAST_TRB_IDX { ep0.enqueue = 0; ep0.pcs = !ep0.pcs; }

    // Data Stage
    if has_data {
        let d_idx = ep0.enqueue;
        let db = d_idx * 4;
        ep0.ring_virt.add(db).write_volatile((data_page & 0xFFFF_FFFF) as u32);
        ep0.ring_virt.add(db + 1).write_volatile(((data_page >> 32) & 0xFFFF_FFFF) as u32);
        ep0.ring_virt.add(db + 2).write_volatile(buf.len() as u32 & 0x1FFFF);
        let dir = if data_in { 1u32 << 16 } else { 0 };
        // Data Stage de un solo TRB = TD propio: sin CH (ver nota del Setup).
        ep0.ring_virt.add(db + 3).write_volatile(
            (TRB_DATA << 10) | dir | if ep0.pcs { 1 } else { 0 });
        ep0.enqueue = d_idx + 1;
        if ep0.enqueue >= LAST_TRB_IDX { ep0.enqueue = 0; ep0.pcs = !ep0.pcs; }
    }

    // Status Stage
    let st_idx = ep0.enqueue;
    let stb = st_idx * 4;
    ep0.ring_virt.add(stb).write_volatile(0);
    ep0.ring_virt.add(stb + 1).write_volatile(0);
    ep0.ring_virt.add(stb + 2).write_volatile(0);
    let dir_in = if has_data { !data_in } else { true };
    ep0.ring_virt.add(stb + 3).write_volatile(
        (TRB_STATUS << 10) | (if dir_in { 1u32 << 16 } else { 0 }) | (1 << 5)
        | if ep0.pcs { 1 } else { 0 }
    );
    ep0.enqueue = st_idx + 1;
    if ep0.enqueue >= LAST_TRB_IDX { ep0.enqueue = 0; ep0.pcs = !ep0.pcs; }

    // Ring EP0 doorbell
    ring_doorbell(slot, 1);

    // Espera el Transfer Event de ESTE EP0 y de nadie mas.
    //
    // El bucle de antes descartaba todo lo que no fuera suyo -- incluidos los
    // informes de interrupcion de un raton ya enumerado, que es exactamente el
    // camino por el que el teclado y el raton se quedaban mudos los dos.
    let ev = match evt_poll_block(ctrl, Espera::Transferencia { slot, ep: 1 }) {
        Some(e) => e,
        None => return 0,
    };
    let dw2 = ev.2;
    let cc = (dw2 >> 24) & 0xFF;
    if cc != CC_SUCCESS && cc != CC_SHORT {
        h.log_u64(" ctrl_xfer cc=", cc as u64);
        return 0;
    }
    let rem = dw2 & 0xFFFFFF;
    let xfer = buf.len().saturating_sub(rem as usize);
    if data_in && has_data && data_page != 0 {
        let dv = h.phys_to_virt(data_page);
        for i in 0..xfer.min(buf.len()) { buf[i] = dv.add(i).read_volatile(); }
    }
    xfer
}

// ===================================================================
//  Descriptor helpers
// ===================================================================

pub unsafe fn get_device_descriptor(slot: u8, buf: &mut [u8]) -> usize {
    let len = if buf.len() > 18 { 18 } else { buf.len() };
    control_transfer(slot, 0x80, USB_REQ_GET_DESCRIPTOR, USB_DESC_DEVICE << 8, 0, &mut buf[..len], true)
}

pub unsafe fn get_config_descriptor(slot: u8, index: u8, buf: &mut [u8]) -> usize {
    control_transfer(slot, 0x80, USB_REQ_GET_DESCRIPTOR, (USB_DESC_CONFIG << 8) | index as u16, 0, buf, true)
}

// ===================================================================
//  Per-endpoint transfer ring storage (for non-EP0 endpoints)
// ===================================================================

const MAX_DCI: usize = 32;
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct EpRing { valid: bool, ring_phys: u64, ring_virt: *mut u32, pcs: bool, enqueue: usize }
// ** EL MISMO CASO QUE `EP0_RINGS`, y aqui cuesta 255 KiB.
//
// 255 ranuras x 32 endpoints x 32 bytes = 261.120 bytes que vivian en `.data`
// **por un solo `true`**. Todo lo demas ya era cero, asi que la tabla entera
// viajaba dentro de la imagen del kernel para llevar un bit por entrada -- un
// bit que ademas no lee nadie: `ep_ring_mut` filtra por `valid` y
// `ep_ring_set` reescribe la estructura completa al registrar.
//
// [!] Y que quede claro que gana cada cosa: **la RAM reservada es la misma**
// --`.bss` tambien ocupa-- lo que baja 255 KiB es la IMAGEN del kernel, que va
// embebida en BOOTX64.EFI y se lee entera en cada arranque.
static mut EP_RINGS: [[EpRing; MAX_DCI]; MAX_SLOTS] = [[EpRing {
    valid: false, ring_phys: 0, ring_virt: core::ptr::null_mut(), pcs: false, enqueue: 0
}; MAX_DCI]; MAX_SLOTS];
fn ep_ring_mut(slot: u8, dci: u8) -> Option<&'static mut EpRing> {
    if (slot as usize) < MAX_SLOTS && (dci as usize) < MAX_DCI {
        unsafe { let p = &mut EP_RINGS[slot as usize][dci as usize]; if p.valid { Some(p) } else { None } }
    } else { None }
}

// ===================================================================
//  Configure Endpoint
// ===================================================================

/// **EP Type del Endpoint Context (xHCI 6.2.3.4), y NO es el del USB.**
///
/// Vive aqui --y no en cada driver-- porque es la tabla del CONTROLADOR: quien
/// la copie a mano en su fichero acaba teniendo dos, y el dia que no coincidan
/// el endpoint queda configurado del tipo equivocado. Eso no falla al
/// configurarlo: falla al primer TRB, que es el peor sitio donde enterarse.
///
/// ```text
///    1  Isoch OUT      4  Control      5  Isoch IN      7  Interrupt IN
/// ```
pub const EP_TYPE_ISOCH_OUT: u8 = 1;
/// Ver [`EP_TYPE_ISOCH_OUT`].
pub const EP_TYPE_ISOCH_IN: u8 = 5;

/// Convierte el `bInterval` del descriptor de endpoint al campo **Interval**
/// del Endpoint Context.
///
/// * EL BUG QUE MATABA AL TECLADO: ese campo NO es lineal. El xHC sirve el
/// endpoint cada `2^Interval x 125 us`, o sea es un EXPONENTE. Escribiamos el
/// `bInterval` crudo, que en Low/Full Speed viene en MILISEGUNDOS (10, 24,
/// 32...). Un teclado que pide 24 ms terminaba programado como 2^24 x 125 us =
/// **35 minutos** entre sondeos; con 32, 149 horas. El endpoint queda
/// "configurado" y el Configure Endpoint devuelve exito -- el xHC sencillamente
/// no lo consulta jamas. Se ve identico a un driver muerto.
///
/// Reglas (xHCI 6.2.3.6):
///   - Low/Full Speed interrupt: `bInterval` en FRAMES de 1 ms (1..255) ->
///     `Interval = 3 + floor(log2(bInterval))` (125 us x 2^3 = 1 ms).
///   - High/Super Speed: `bInterval` YA es un exponente (1..16) ->
///     `Interval = bInterval - 1`.
/// El campo tiene 4 bits utiles: se acota a 0..15.
pub fn encode_interval(speed: u8, b_interval: u8) -> u8 {
    match speed {
        1 | 2 => {
            // Full (1) / Low (2): milisegundos -> exponente de 125 us.
            let ms = if b_interval == 0 { 1u32 } else { b_interval as u32 };
            let mut e = 0u32;
            while (1u32 << (e + 1)) <= ms { e += 1; } // floor(log2(ms))
            let v = 3 + e;
            if v > 15 { 15 } else { v as u8 }
        }
        _ => {
            // High (3) / Super (4+): ya viene como exponente.
            let b = if b_interval == 0 { 1u8 } else { b_interval };
            let v = b - 1;
            if v > 15 { 15 } else { v }
        }
    }
}

// Diagnostico del ultimo endpoint configurado: que pidio el descriptor y que
// programamos de verdad. Con esto CABINA puede decir "pediste 24 ms, programe
// exponente 7 (16 ms)" en vez de dejarnos adivinando.
static mut LAST_EP_BINTERVAL: u8 = 0;
static mut LAST_EP_INTERVAL: u8 = 0;
static mut LAST_EP_SPEED: u8 = 0;

/// `(bInterval_del_descriptor, Interval_programado, speed_del_slot)`.
pub fn last_ep_timing() -> (u8, u8, u8) {
    unsafe { (LAST_EP_BINTERVAL, LAST_EP_INTERVAL, LAST_EP_SPEED) }
}

/// Completion Code del ultimo Configure Endpoint. `0xFE` = el controlador no
/// contesto nada.
static mut LAST_CFG_EP_CC: u8 = 0xFF;

/// **Por que el Configure Endpoint dijo que no.**
///
/// # Esto existe por la regla del escritorio, no por comodidad
///
/// El codigo se escribia con `h.log`, o sea **por el cable de serie**. El dueno
/// de esta maquina trabaja en el escritorio y al shell de Ring 0 no vuelve: un
/// dato que solo sale por serie, para el, no existe. El 2026-08-25 el audifono
/// se quedo sin tubo y lo unico que llego a la pantalla fue *"el xHC no
/// configuro el endpoint isocrono"* -- la frase, sin el numero que dice cual de
/// las cinco causas fue.
///
/// ```text
///     0  Success
///     4  Transaction Error
///     8  Bandwidth Error     el intervalo pedido no cabe en la agenda periodica
///    11  Trb Error
///    17  Parameter Error     algun campo del contexto no vale  <- CErr en isoch
///    19  Context State Error el endpoint ya estaba configurado y corriendo
///   0xFE  el controlador no contesto
/// ```
///
/// ** Un driver no puede decidir que hacer con esto y no lo intenta: lo APUNTA.
/// Quien llama sabe a que aparato pertenece y lo pone en CABINA con su nombre.
pub fn last_cfg_ep_cc() -> u8 {
    unsafe { LAST_CFG_EP_CC }
}

/// Estado del endpoint leido del **Device Context** (el que mantiene el xHC,
/// no el que le mandamos): 0=Disabled 1=Running 2=Halted 3=Stopped 4=Error.
/// Si tras configurar no esta en Running, el endpoint no esta agendado y
/// ninguna cantidad de doorbells lo va a despertar.
pub unsafe fn ep_state(slot: u8, dci: u8) -> u8 {
    let ctrl = match CTRL.as_ref() { Some(c) => c, None => return 0xFF };
    let cs = ctx_sz(ctrl);
    let dev_phys = match dcbaa_get(slot) { Some(p) => p, None => return 0xFF };
    let dev_virt = hal().phys_to_virt(dev_phys) as *const u32;
    let ep = dev_virt.add((dci as usize) * cs / 4);
    (ep.read_volatile() & 0x7) as u8
}

/// USBSTS crudo. Bit 2 = HSE (Host System Error, tipicamente un DMA a memoria
/// que el xHC no puede tocar) y bit 12 = HCE (Host Controller Error). Si
/// alguno esta encendido el controlador esta muerto y todo lo demas es ruido.
pub unsafe fn usbsts() -> u32 {
    match CTRL.as_ref() { Some(c) => op_r(c.mmio, c.op_base, USBSTS), None => 0 }
}

pub unsafe fn configure_endpoint(slot: u8, dci: u8, ep_type: u8, max_pkt: u16, interval: u8) -> bool {
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return false };
    let h = hal();
    let cs = ctx_sz(ctrl);

    let in_phys = match h.alloc_dma_pages(1) { Some(p) => p, None => { h.log("[xhci] cfg_ep: no mem\n"); return false; } };
    let in_virt = h.phys_to_virt(in_phys) as *mut u8;
    core::ptr::write_bytes(in_virt, 0, 4096);

    let in32 = in_virt as *mut u32;
    in32.add(0).write_volatile(0); // Drop
    in32.add(1).write_volatile((1u32 << dci) | 1); // Add Slot(0) + EP(dci)

    // Slot Context: preserve the controller-populated route/speed/root-port
    // fields from the output Device Context; only raise Context Entries.
    let sc = in_virt.add(cs) as *mut u32;
    let dev_phys = match dcbaa_get(slot) { Some(p) => p, None => { h.log("[xhci] cfg_ep: no dev ctx\n"); return false; } };
    let dev_virt = h.phys_to_virt(dev_phys) as *const u32;
    let slot_dwords = cs / 4;
    for i in 0..slot_dwords {
        sc.add(i).write_volatile(dev_virt.add(i).read_volatile());
    }
    let old_dw0 = sc.add(0).read_volatile();
    let old_entries = (old_dw0 >> 27) & 0x1F;
    let new_entries = old_entries.max(dci as u32);
    sc.add(0).write_volatile((old_dw0 & !(0x1F << 27)) | (new_entries << 27));

    // Allocate transfer ring for this endpoint
    let tr_phys = match h.alloc_dma_pages(1) { Some(p) => p, None => { h.log("[xhci] cfg_ep: no ring\n"); return false; } };
    let tr_virt = h.phys_to_virt(tr_phys) as *mut u32;
    core::ptr::write_bytes(tr_virt as *mut u8, 0, 4096);
    // El Link TRB del final del anillo necesita **Toggle Cycle**: sin el, al dar
    // la primera vuelta (255 reportes) el productor invierte su PCS pero el
    // consumidor no, el ciclo deja de coincidir y el endpoint se congela para
    // siempre. Una bomba de tiempo a ~255 pulsaciones de tecla.
    let mut tr = TransferRing::new(tr_virt, tr_phys);
    tr.enable_toggle_cycle();
    if (slot as usize) < MAX_SLOTS && (dci as usize) < MAX_DCI {
        EP_RINGS[slot as usize][dci as usize] = EpRing {
            valid: true, ring_phys: tr_phys & !0xF, ring_virt: tr_virt, pcs: true, enqueue: 0,
        };
    }

    let dq = (tr_phys & !0xF) | 1;
    let ep = in_virt.add((dci as usize + 1) * cs) as *mut u32;
    // La velocidad la sabe el propio Slot Context que acabamos de copiar del
    // Device Context (bits 23:20) -- no hace falta que el caller la adivine ni
    // arrastrarla por media pila de llamadas: se la preguntamos al hardware.
    let speed = ((old_dw0 >> 20) & 0xF) as u8;
    let enc = encode_interval(speed, interval);
    LAST_EP_BINTERVAL = interval;
    LAST_EP_INTERVAL = enc;
    LAST_EP_SPEED = speed;
    ep.add(0).write_volatile((enc as u32) << 16); // DW0: Interval in bits 23:16
    // *** CErr (DW1 bits 2:1) NO ES UNA CONSTANTE, Y ESO COSTO EL AUDIO.
    //
    // xHCI 6.2.3.5: *"CErr ... shall be set to '0' for Isoch endpoints."* No es
    // una recomendacion de estilo: una transferencia isocrona **no se
    // reintenta** --la muestra llega a tiempo o no existe-- asi que un contador
    // de reintentos en un endpoint isocrono es un campo con un valor que el
    // hardware declara imposible.
    //
    // ** Aqui iba `3` fijo para TODOS. En el teclado (interrupcion) es correcto
    // y lleva meses funcionando; en el endpoint isocrono del audifono es un
    // parametro invalido, y un xHC estricto --el de AMD lo es, ya lo demostro
    // con `CH=1` en las etapas de control-- contesta **Parameter Error (cc=17)**
    // al Configure Endpoint. El endpoint no queda configurado, `abrir` devuelve
    // `false`, y el escritorio dice *"el tubo NO esta abierto"* sin poder decir
    // por que.
    let isocrono = ep_type == EP_TYPE_ISOCH_OUT || ep_type == EP_TYPE_ISOCH_IN;
    let cerr: u32 = if isocrono { 0 } else { 3 };
    ep.add(1).write_volatile(
        ((max_pkt as u32) << 16) | ((ep_type as u32) << 3) | (cerr << 1)
    );
    ep.add(2).write_volatile((dq & 0xFFFF_FFFF) as u32);
    ep.add(3).write_volatile(((dq >> 32) & 0xFFFF_FFFF) as u32);
    // DW4: Max ESIT Payload Lo (bits 31:16) | Average TRB Length (bits 15:0).
    // * EL BUG DEL TECLADO: sin Max ESIT Payload, el xHC asigna CERO ancho de
    // banda periodico al endpoint de INTERRUPCION -> nunca lo sirve -> las
    // teclas jamas completan (tev pegado, kev=0). Para un teclado boot el
    // payload por intervalo = max_pkt (8 bytes). Con esto el DCI del teclado
    // deberia empezar a postear Transfer Events al presionar teclas.
    let max_esit = max_pkt as u32; // interrupt LS/FS/HS boot: 1 paquete por ESIT
    // ** Y el Average TRB Length tampoco es una constante.
    //
    // Es lo que el xHC usa para presupuestar el bus, y estaba clavado en `8` --
    // el tamano de un informe de teclado boot. Para un endpoint isocrono que
    // entrega 192 bytes cada milisegundo, declarar 8 es pedir **veinticuatro
    // veces menos ancho de banda del que se va a gastar**. El sintoma no es un
    // error: son tramas que llegan tarde, o sea justo el contador que
    // `AUDIO_MAESTRO` puso en la portada. Un numero mal declarado que se ve
    // como un chasquido en un oido.
    let avg_trb = if isocrono { max_pkt as u32 } else { 8 };
    ep.add(4).write_volatile((max_esit << 16) | avg_trb);

    let trb = Trb {
        dw0: (in_phys & 0xFFFF_FFFF) as u32,
        dw1: ((in_phys >> 32) & 0xFFFF_FFFF) as u32,
        dw2: 0,
        dw3: ((slot as u32) << 24) | (TRB_CONFIGURE << 10),
    };
    ctrl.cmd_ring.enqueue(&trb);
    ring_doorbell(0, 0);
    // Igual que en `address_device`: esto tomaba el primer evento sin mirar el
    // tipo, asi que podia dar por configurado un endpoint leyendo el `cc` del
    // informe de otro aparato.
    let ev = evt_poll_block(ctrl, Espera::Comando);
    match ev {
        Some((_, _, dw2, _)) => {
            let cc = (dw2 >> 24) & 0xFF;
            LAST_CFG_EP_CC = cc as u8;
            if cc != CC_SUCCESS {
                // El CODIGO, no un "FAIL" mudo. Los que importan aqui:
                // 4=Transaction Error, 8=Bandwidth Error (el intervalo pedido
                // no cabe en la agenda periodica), 11=Trb Error,
                // 17=Parameter Error (algun campo del contexto no vale).
                h.log_u64("[xhci] cfg_ep cc=", cc as u64);
                h.log("\n");
            }
            cc == CC_SUCCESS
        }
        None => {
            LAST_CFG_EP_CC = 0xFE;
            h.log("[xhci] cfg_ep sin respuesta del controlador\n");
            false
        }
    }
}

// ===================================================================
//  RESUCITAR UN ENDPOINT PARADO
// ===================================================================
//
// * EL AGUJERO QUE ESTO TAPA: el driver sabia VER que un endpoint estaba
// Halted (`ep_state` lo documenta desde hace tiempo) y no tenia con que
// levantarlo. Los dos comandos que hacen falta --Reset Endpoint (14) y Set TR
// Dequeue Pointer (16)-- sencillamente no estaban escritos.
//
// El sintoma es el que conto el dueno: **el teclado deja de responder al
// pulsar, sin que nadie lo desenchufe**. Un error de transaccion del bus
// --cable, ruido, un paquete que llega mal-- deja el endpoint parado, y a partir
// de ahi `rearmar()` encola y toca el timbre para nada: **el xHC ignora el
// doorbell de un endpoint Halted**. Se veia identico a un aparato desconectado.
//
// La secuencia es de la spec (xHCI 4.6.8) y el ORDEN no es negociable:
//
//   1. Reset Endpoint      Halted -> Stopped. Sin esto, lo demas no vale.
//   2. Set TR Dequeue      decirle POR DONDE seguir. El endpoint parado dejo
//                          el puntero a mitad del anillo; si no se recoloca,
//                          al arrancar lee TRBs viejos con el ciclo cambiado.
//   3. <- el llamante encola y toca el timbre (`rearmar`)
//
// El paso 2 es el que se olvida y el que hace que "el reset no sirviera de
// nada": resetear sin recolocar deja el endpoint listo para leer basura.

/// Los `cc` de un Transfer Event que dejan el endpoint **parado**, y por tanto
/// exigen recuperarlo en vez de reintentar.
///
/// `3` Babble (el aparato mando mas de lo que cabia), `4` USB Transaction Error
/// (el bus fallo), `6` Stall (el aparato dijo que no). Cualquier otro `cc` malo
/// es informativo: molesta, pero el endpoint sigue agendado.
pub fn cc_halta_endpoint(cc: u8) -> bool { matches!(cc, 3 | 4 | 6) }

static mut RECUPERACIONES: u32 = 0;
static mut RECUPERACIONES_FALLIDAS: u32 = 0;

/// `(endpoints resucitados, intentos que no salieron)`.
///
/// El segundo numero es el que hay que mirar: si sube, el aparato no vuelve con
/// un reset y el problema esta mas abajo (el puerto o el propio cable).
pub fn recuperaciones() -> (u32, u32) {
    unsafe { (RECUPERACIONES, RECUPERACIONES_FALLIDAS) }
}

/// Manda un comando y devuelve su `cc` -- a diferencia de `send_cmd`, que
/// convierte cualquier fallo en `None` y se lleva el numero por delante. Aqui
/// el numero ES el diagnostico: `19` (Context State Error) significa "el
/// endpoint no estaba en el estado que este comando espera", que es distinto de
/// "el controlador no contesto".
unsafe fn cmd_cc(trb: Trb) -> Option<u32> {
    let ctrl = CTRL.as_mut()?;
    ctrl.cmd_ring.enqueue(&trb);
    ring_doorbell(0, 0);
    let ev = evt_poll_block(ctrl, Espera::Comando)?;
    Some((ev.2 >> 24) & 0xFF)
}

/// Levanta un endpoint parado y lo deja listo para que el llamante encole.
///
/// Devuelve `true` si el endpoint quedo en condiciones de volver a bombear. **No
/// encola ni toca el timbre**: eso es trabajo del dueno del endpoint, que es
/// quien sabe que buffer y que largo le tocan.
///
/// [!] Bloquea, porque espera la complecion de dos comandos. Es aceptable por lo
/// mismo que la adopcion en caliente: ocurre **solo cuando algo ya ha fallado**,
/// no en el camino normal. El dia que haya un hilo de kernel para el bus, esto
/// se muda ahi con lo demas.
pub unsafe fn recuperar_endpoint(slot: u8, dci: u8) -> bool {
    let h = hal();

    // El estado lo dice el xHC, no nosotros. Si ya esta Running no hay nada que
    // resetear y hacerlo daria Context State Error: un `cc=19` en el log que
    // pareceria un fallo cuando en realidad no habia averia.
    let estado = ep_state(slot, dci);
    if estado == 1 {
        return true;
    }
    if estado == 0 || estado == 0xFF {
        // Disabled: el endpoint no esta configurado. Un reset no lo arregla --
        // esto es re-enumerar, y no se decide aqui.
        h.log_u64("[xhci] recuperar: endpoint sin configurar, dci=", dci as u64);
        h.log("\n");
        RECUPERACIONES_FALLIDAS = RECUPERACIONES_FALLIDAS.wrapping_add(1);
        return false;
    }

    let campos = ((slot as u32) << 24) | ((dci as u32) << 16);

    // -- 1. Reset Endpoint: Halted -> Stopped --------------------------
    //
    // El bit TSP (9) se deja a 0 a proposito: preservar el estado de
    // transferencia es justo lo que NO se quiere aqui. Lo que habia en vuelo
    // cuando el endpoint se paro es exactamente lo que fallo.
    if estado == 2 {
        match cmd_cc(Trb { dw0: 0, dw1: 0, dw2: 0, dw3: campos | (TRB_RESET_EP << 10) }) {
            Some(CC_SUCCESS) => {}
            Some(cc) => {
                h.log_u64("[xhci] reset_ep cc=", cc as u64);
                h.log("\n");
                RECUPERACIONES_FALLIDAS = RECUPERACIONES_FALLIDAS.wrapping_add(1);
                return false;
            }
            None => {
                h.log("[xhci] reset_ep sin respuesta del controlador\n");
                RECUPERACIONES_FALLIDAS = RECUPERACIONES_FALLIDAS.wrapping_add(1);
                return false;
            }
        }
    }

    // -- 2. Set TR Dequeue Pointer: volver al principio del anillo -----
    //
    // Se recoloca al TRB 0 con ciclo 1 y **la contabilidad nuestra se pone a
    // juego** (`enqueue = 0`, `pcs = true`). Las dos mitades tienen que decir lo
    // mismo: el xHC leera donde le decimos, y nosotros escribiremos ahi con el
    // ciclo que le hemos declarado. Descuadrarlas es congelar el endpoint de la
    // forma mas dificil de ver -- el mismo fallo que el Toggle Cycle del Link.
    let ring_phys = match ep_ring_mut(slot, dci) {
        Some(r) => {
            r.enqueue = 0;
            r.pcs = true;
            r.ring_phys
        }
        None => {
            h.log("[xhci] recuperar: no hay anillo para ese endpoint\n");
            RECUPERACIONES_FALLIDAS = RECUPERACIONES_FALLIDAS.wrapping_add(1);
            return false;
        }
    };
    let dq = (ring_phys & !0xFu64) | 1; // DCS = 1, a juego con pcs = true
    let trb = Trb {
        dw0: (dq & 0xFFFF_FFFF) as u32,
        dw1: ((dq >> 32) & 0xFFFF_FFFF) as u32,
        dw2: 0, // Stream ID 0: este endpoint no usa streams
        dw3: campos | (TRB_SET_TR_DEQ << 10),
    };
    match cmd_cc(trb) {
        Some(CC_SUCCESS) => {}
        Some(cc) => {
            h.log_u64("[xhci] set_tr_deq cc=", cc as u64);
            h.log("\n");
            RECUPERACIONES_FALLIDAS = RECUPERACIONES_FALLIDAS.wrapping_add(1);
            return false;
        }
        None => {
            h.log("[xhci] set_tr_deq sin respuesta del controlador\n");
            RECUPERACIONES_FALLIDAS = RECUPERACIONES_FALLIDAS.wrapping_add(1);
            return false;
        }
    }

    RECUPERACIONES = RECUPERACIONES.wrapping_add(1);
    h.log_u64("[xhci] endpoint RESUCITADO dci=", dci as u64);
    h.log_u64(" slot=", slot as u64);
    h.log("\n");
    true
}

// ===================================================================
//  Queue interrupt IN transfer
// ===================================================================

pub unsafe fn queue_interrupt_in(slot: u8, dci: u8, buf_phys: u64, len: u16) -> bool {
    let ring = match ep_ring_mut(slot, dci) { Some(r) => r, None => return false };
    let idx = ring.enqueue;
    let b = idx * 4;
    ring.ring_virt.add(b).write_volatile((buf_phys & 0xFFFF_FFFF) as u32);
    ring.ring_virt.add(b + 1).write_volatile(((buf_phys >> 32) & 0xFFFF_FFFF) as u32);
    ring.ring_virt.add(b + 2).write_volatile(len as u32);
    let ctl = (TRB_NORMAL << 10) | (1 << 5); // IOC
    ring.ring_virt.add(b + 3).write_volatile(ctl | if ring.pcs { 1 } else { 0 });
    ring.enqueue = idx + 1;
    if ring.enqueue >= LAST_TRB_IDX {
        // Al dar la vuelta hay que dejar el Link TRB con el ciclo ACTUAL antes
        // de invertir el nuestro; si no, el xHC llega al Link, ve un ciclo que
        // no es el suyo, y se detiene ahi para siempre. (El bit Toggle Cycle lo
        // pusimos al crear el anillo en configure_endpoint.)
        let lb = LAST_TRB_IDX * 4;
        let dw3 = ring.ring_virt.add(lb + 3).read_volatile();
        let dw3 = (dw3 & !1) | if ring.pcs { 1 } else { 0 };
        ring.ring_virt.add(lb + 3).write_volatile(dw3);
        ring.enqueue = 0;
        ring.pcs = !ring.pcs;
    }
    true
}

// ===================================================================
//  Queue ISOCHRONOUS OUT transfer -- las muestras
// ===================================================================

/// **Cuantas tramas se dejan sin entregar antes de decir que se llego tarde.**
///
/// Una isocrona tiene una cita: el xHC la entrega en SU microtrama y si no hay
/// datos, **no espera** -- manda silencio y sigue. Ese es el trato entero: se
/// pierde la muestra, no el tiempo.
pub const ISOCH_ADELANTO: u16 = 4;

/// **Encolar una trama de muestras a un endpoint isocrono OUT.**
///
/// El hermano de [`queue_interrupt_in`], y **no es el mismo con otro numero**.
/// Estas son las cuatro diferencias, que son justo donde se equivoca quien copia
/// la funcion de al lado:
///
/// ```text
///    1. el tipo es 5 (Isoch), no 1 (Normal)
///    2. lleva FRAME ID: en que microtrama quiere el aparato estos bytes
///    3. lleva SIA -- *Start Isoch ASAP*-- que le dice al xHC que lo ponga en
///       la primera microtrama libre en vez de exigir un frame id exacto
///    4. TBC/TLBC: cuantas rafagas lleva la trama. Con un paquete por
///       intervalo son cero, y **cero no es "no aplica": es el valor**
/// ```
///
/// # *** POR QUE SIA Y NO UN FRAME ID CALCULADO
///
/// Calcular el frame id exige leer el `MFINDEX` del controlador, sumarle un
/// adelanto y acertar antes de que el reloj avance. **Si se falla el numero, el
/// xHC contesta `Isoch Buffer Overrun` o tira la trama** -- y las dos se oyen
/// igual: un clic.
///
/// Con `SIA` la cita la elige el controlador, que es el que tiene el reloj
/// delante. Se pierde control sobre la latencia exacta y se gana no fallar la
/// cita, y `AUDIO_MAESTRO` ya decidio ese cambio por escrito:
///
/// > *"Primero que suene sin huecos. Un audio puntual con 40 ms de retardo es
/// > audio; uno con 5 ms y clics, no."*
///
/// [!] Y el dia que la latencia importe, esto es lo que hay que cambiar --
/// **no antes**, y midiendo `tramas tarde` para saber si se gano algo.
///
/// # Lo que esta funcion NO hace
///
/// **No toca el timbre.** Encolar y avisar son dos cosas: quien alimenta el
/// tubo quiere poner varias tramas de adelanto y tocar UNA vez, y un timbre por
/// trama seria un MMIO por cada 192 bytes de audio.
pub unsafe fn queue_isoch_out(slot: u8, dci: u8, buf_phys: u64, len: u16) -> bool {
    let ring = match ep_ring_mut(slot, dci) { Some(r) => r, None => return false };
    let idx = ring.enqueue;
    let b = idx * 4;
    ring.ring_virt.add(b).write_volatile((buf_phys & 0xFFFF_FFFF) as u32);
    ring.ring_virt.add(b + 1).write_volatile(((buf_phys >> 32) & 0xFFFF_FFFF) as u32);
    // dw2: los 17 bits bajos son el largo. TD Size (21:17) va a CERO -- con una
    // sola trama por TD no hay paquetes pendientes que declarar.
    ring.ring_virt.add(b + 2).write_volatile(len as u32);
    // dw3: tipo en 15:10, IOC en el 5, SIA en el 31, frame id en 30:20 (que con
    // SIA puesto el xHC ignora), TBC en 8:7 y TLBC en 17:16 -- los dos a cero.
    let ctl = (TRB_ISOCH << 10) | (1 << 5) | (1 << 31);
    ring.ring_virt.add(b + 3).write_volatile(ctl | if ring.pcs { 1 } else { 0 });
    ring.enqueue = idx + 1;
    if ring.enqueue >= LAST_TRB_IDX {
        // El mismo cierre del anillo que el Normal, y por el mismo motivo: si el
        // Link TRB se queda con el ciclo viejo, el xHC llega ahi, ve un ciclo
        // que no es el suyo, y **se para para siempre**. En audio eso no es un
        // fallo visible: es que deja de sonar y nadie sabe por que.
        let lb = LAST_TRB_IDX * 4;
        let dw3 = ring.ring_virt.add(lb + 3).read_volatile();
        let dw3 = (dw3 & !1) | if ring.pcs { 1 } else { 0 };
        ring.ring_virt.add(lb + 3).write_volatile(dw3);
        ring.enqueue = 0;
        ring.pcs = !ring.pcs;
    }
    ISOCH_ENCOLADAS = ISOCH_ENCOLADAS.wrapping_add(1);
    true
}

/// Cuantas tramas isocronas se han encolado desde el arranque.
///
/// ** Tiene que SUBIR SOLA mientras algo suene. Un numero quieto con el tubo
/// abierto significa que quien lo alimenta dejo de hacerlo, que es distinto de
/// que el aparato no acepte -- y eso ultimo lo dice `ISOCH_TARDE`.
pub fn isoch_encoladas() -> u64 {
    unsafe { ISOCH_ENCOLADAS }
}

/// **Tramas que el controlador no pudo entregar a tiempo.**
///
/// *** LA CIFRA DE TODA LA PAGINA DE AUDIO. `AUDIO_MAESTRO` lo dice: un audio que
/// va bien y uno que chasquea **se distinguen por este contador y por nada
/// mas** -- a oido son "suena raro" y "suena bien", que no es un diagnostico.
pub fn isoch_tarde() -> u64 {
    unsafe { ISOCH_TARDE }
}

/// Lo apunta el bucle de eventos cuando el xHC contesta `Isoch Buffer Overrun`
/// (`CC 31`) o `Missed Service Error` (`CC 10`).
pub fn apunta_isoch_tarde() {
    unsafe { ISOCH_TARDE = ISOCH_TARDE.wrapping_add(1) };
}

static mut ISOCH_ENCOLADAS: u64 = 0;
static mut ISOCH_TARDE: u64 = 0;

// ===================================================================
//  Public non-blocking event poll
// ===================================================================

/// Returns (slot, endpoint_id, cc) for the next transfer event, or None.
/// Ring doorbell for a slot+endpoint. EP0=1, EP1 OUT=2, EP1 IN=3, etc.
pub unsafe fn ring_doorbell(slot: u8, endpoint_id: u8) {
    if let Some(c) = CTRL.as_ref() {
        let db_addr = c.mmio + c.db_base as u64 + (slot as u64) * 4;
        w32(db_addr, endpoint_id as u32);
    }
}

/// Diagnostico: cuantos Transfer Events ha posteado el xHC (cualquier slot/ep)
/// y cuantos eventos crudos de cualquier tipo. Si al presionar teclas TEV no
/// sube, el controlador no esta completando la transferencia de interrupcion
/// (endpoint/ring/doorbell), no el parseo. Ojos en metal desnudo.
static mut XFER_EVENTS: u32 = 0;
static mut RAW_EVENTS: u32 = 0;
// Ultimo Transfer Event: slot, endpoint_id, completion_code. Para comparar con
// el dci del teclado y ver si el evento matchea (si no, no se re-encola).
static mut LAST_SLOT: u8 = 0;
static mut LAST_EP: u8 = 0;
static mut LAST_CC: u8 = 0;

pub fn xfer_events() -> u32 { unsafe { XFER_EVENTS } }
pub fn raw_events() -> u32 { unsafe { RAW_EVENTS } }
pub fn last_event() -> (u8, u8, u8) { unsafe { (LAST_SLOT, LAST_EP, LAST_CC) } }

pub unsafe fn poll_transfer_event() -> Option<(u8, u8, u8)> {
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return None };
    loop {
        // * Lo APARCADO primero, y en orden.
        //
        // Un evento que llego mientras la enumeracion esperaba otra cosa es
        // tan valido como uno recien posteado -- y es justamente el primer
        // informe de cada aparato, el que arranca la bomba. Si el aparcadero
        // no se drenara aqui, se habria cambiado tirar eventos por guardarlos
        // donde nadie los mira, que es el mismo silencio con otra cara.
        let ev = match desaparcar_cualquiera() {
            Some(e) => e,
            None => evt_poll_nb(ctrl)?,
        };
        RAW_EVENTS = RAW_EVENTS.wrapping_add(1);
        let typ = (ev.3 >> 10) & 0x3F;
        if typ == TRB_TRANSFER {
            XFER_EVENTS = XFER_EVENTS.wrapping_add(1);
            let slot = ((ev.3 >> 24) & 0xFF) as u8;
            let ep = ((ev.3 >> 16) & 0x1F) as u8;
            let cc = (ev.2 >> 24) as u8;
            LAST_SLOT = slot; LAST_EP = ep; LAST_CC = cc;
            // *** LAS DOS FORMAS DE LLEGAR TARDE A UNA CITA ISOCRONA.
            //
            //    10  Missed Service Error   el xHC no llego a servir el TD en
            //                               su microtrama
            //    31  Isoch Buffer Overrun   el aparato pidio y no habia
            //
            // Se cuentan aqui --el unico sitio por donde pasan TODOS los
            // eventos-- y no en el que alimenta el tubo, porque ese no los ve.
            // Es la cifra que separa "suena bien" de "suena raro", y sin ella
            // esas dos frases son todo el diagnostico que hay.
            if cc == 10 || cc == 31 {
                apunta_isoch_tarde();
            }
            return Some((slot, ep, cc));
        }
        // -- Cambio de puerto: enchufaron o desenchufaron algo --
        //
        // Esto se estaba DESCARTANDO junto con las compleciones, y por eso no
        // habia hot-plug: el xHC avisa de que un puerto cambio de estado, y
        // nadie escuchaba. Al desenchufar el teclado no se enteraba nadie, y al
        // volver a enchufarlo tampoco.
        //
        // El Port ID viene en los bits 31:24 del primer dword del TRB, y es
        // 1-based (el puerto 1 del xHC es el indice 0 de PORTSC).
        //
        // * Hay que limpiar CSC SI O SI. Es write-1-to-clear: mientras siga
        // puesto, el xHC no vuelve a avisar de ese puerto -- el segundo
        // enchufe pasaria en silencio. Se escribe preservando PP y poniendo
        // SOLO el bit que se quiere limpiar: los demas bits de estado son
        // RW1C y escribirles un 1 limpiaria cambios que no hemos atendido.
        if typ == TRB_PORT_STATUS {
            let port_id = ((ev.0 >> 24) & 0xFF) as u8;
            if port_id >= 1 {
                let idx = port_id - 1;
                if let Some(c) = CTRL.as_ref() {
                    let pb = c.op_base as u64 + 0x400 + idx as u64 * 0x10;
                    let sc = r32(c.mmio + pb + PORTSC as u64);
                    if sc & PORTSC_CSC != 0 {
                        w32(c.mmio + pb + PORTSC as u64, (sc & PORTSC_PP) | PORTSC_CSC);
                    }
                    PORT_EVENTS = PORT_EVENTS.wrapping_add(1);
                    LAST_PORT = port_id;
                    LAST_PORT_CCS = sc & PORTSC_CCS != 0;
                    // * SE ENCOLA, no se sobrescribe. Este bucle drena el anillo
                    // entero hasta dar con un Transfer Event, asi que aqui se
                    // cruzan VARIOS cambios de puerto en la misma vuelta -- y
                    // con el buzon de una plaza solo sobrevivia el ultimo. El
                    // par desenchufe/enchufe se fundia en "conectado" y el
                    // teclado que se habia ido seguia contando como presente.
                    // Ver la cabecera de `avisos`.
                    PORT_COLA.anotar(port_id, LAST_PORT_CCS);
                }
            }
            continue;
        }
        if typ == TRB_COMPLETION { continue; }
    }
}

// -- Hot-plug: lo que el driver ve, para que otro decida --------------
//
// El driver NO re-enumera solo. Reconstruir un dispositivo es asignar slot,
// direccionarlo y configurar endpoints -- decisiones que toma la capa de
// arriba (`uhid` + `dev::usb`), que es la que sabe si lo que se enchufo es un
// teclado, un raton o un disco. Aqui solo se anota el hecho.

static mut PORT_EVENTS: u32 = 0;
static mut LAST_PORT: u8 = 0;
static mut LAST_PORT_CCS: bool = false;
/// **La cola de avisos.** Antes esto eran los dos estaticos de arriba mas un
/// `PORT_PENDIENTE: bool`, es decir un buzon de UNA plaza -- ver la cabecera de
/// [`avisos`] para el bug que eso costo. `LAST_PORT`/`LAST_PORT_CCS` siguen
/// existiendo, pero ya solo para [`port_stats`]: son diagnostico, no el canal.
static mut PORT_COLA: Avisos = Avisos::nueva();

/// `(cuantos cambios de puerto, ultimo puerto, hay dispositivo ahora)`.
/// Para diagnostico: si esto no sube al desenchufar, el xHC no esta avisando.
pub fn port_stats() -> (u32, u8, bool) {
    unsafe { (PORT_EVENTS, LAST_PORT, LAST_PORT_CCS) }
}

/// `(avisos esperando, avisos que no cupieron)`. Para el panel.
///
/// Un desborde no es fatal --el barrido de `bmo_uhid` compara con los puertos de
/// verdad-- pero es la senal de que los avisos por si solos ya no bastan.
pub fn avisos_stats() -> (usize, u32) {
    unsafe { (PORT_COLA.largo(), PORT_COLA.desbordes()) }
}

/// Consume UN aviso, el mas antiguo: `Some((puerto, conectado))`.
///
/// Devuelve `None` si no hay nada nuevo, para que el llamante pueda sondear en
/// su bucle sin re-enumerar cien veces el mismo enchufe.
///
/// * Quien llama tiene que **insistir hasta el `None`**, no atender uno por
/// vuelta. Un `if let` aqui deja los demas avisos esperando, y eso reintroduce
/// por arriba justo el retraso que la cola quita por abajo.
pub fn tomar_cambio_puerto() -> Option<(u8, bool)> {
    unsafe { PORT_COLA.tomar() }
}

/// **Hay algo enchufado en este puerto AHORA MISMO?** (PORTSC.CCS, 0-based).
///
/// La pregunta que no se podia hacer, y por eso todo colgaba de los avisos: un
/// aviso perdido dejaba al driver creyendo algo que el hardware desmiente desde
/// hace rato, sin forma de comprobarlo. Es la base del barrido de `bmo_uhid`.
///
/// # Safety
/// Lee MMIO del xHC: hay que llamarlo con el CR3 del kernel puesto.
pub unsafe fn hay_dispositivo(port: u8) -> bool {
    let c = match CTRL.as_ref() { Some(c) => c, None => return false };
    if port >= c.max_ports { return false; }
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    r32(c.mmio + pb + PORTSC as u64) & PORTSC_CCS != 0
}

/// Cuantos puertos raiz declara el controlador. 0 si aun no hay controlador.
pub fn puertos_totales() -> u8 {
    unsafe { CTRL.as_ref().map_or(0, |c| c.max_ports) }
}
