//! **Asking the USB audio device how it wants its samples.** Nothing else.
//!
//! Step 0 of `docs/maestro/AUDIO_MAESTRO.md`, kernel side. The decision --reading the
//! descriptor-- lives in `bmo_uaudio::stream`, where it is tested; here there is
//! only what no test can cover: touching the bus and printing.
//!
//! # Why its own file, next to `bus` and `rescate`
//!
//! Because it is a fourth job and it does not belong to any of the other three.
//! `mod.rs` is the bridge to the xHC for HID; this is a different device class
//! that happens to hang off the same controller. Putting it in `mod.rs` would be
//! re-doing exactly what was undone on 2026-08-12.
//!
//! # ** IT IS A TYPED COMMAND, NOT A BOOT STEP, AND THAT IS THE WHOLE SAFETY
//!
//! Enumerating a port RESETS it. Doing that to an already-working keyboard kills
//! it -- the mine is documented in `bmo_uhid::puertos`: *"resetear un puerto ES
//! un cambio de puerto, asi que el aviso que la dispara lo genera ella misma,
//! para siempre. Peor: el puerto que giraba era el del teclado ya enumerado, que
//! moria con el primer reset."*
//!
//! So this only ever touches ports **that `bmo_uhid` did not take**. A port with
//! a keyboard on it is never addressed from here, and that is checked and not
//! remembered.
//!
//! And it runs from the `audio` command and not from boot, same as `smp` and
//! `net rx`: if something goes wrong, what hangs is a command and not the
//! machine at power-on.

use crate::ring0::cabina;

/// **Walks the untaken ports and reports the first playback pipe it finds.**
///
/// Returns `true` if it found one. Everything it learns goes to CABINA, because
/// the point of this step is a photograph that can be compared against what the
/// other operating system on this machine says about the same headset -- the
/// same method that answered the July `#GP` and the NIC's MAC.
///
/// # Safety
/// Touches xHC MMIO: has to run with the kernel CR3 loaded. The `audio` command
/// goes through [`super::pump_bus`]'s wrapper for that reason.
pub unsafe fn censar() -> bool {
    let ctrl = match bmo_xhci::controller() {
        Some(c) => c,
        None => {
            cabina::warn("audio", "no hay controlador xHCI: no hay a quien preguntar", 0);
            return false;
        }
    };
    let max_ports = ctrl.max_ports;

    let mut mirados = 0u64;
    for port in 0..max_ports {
        // ** LOS PUERTOS DE OTRO NO SE TOCAN. Ver la cabecera.
        let ocupado = {
            let hid = &*core::ptr::addr_of!(super::HID);
            hid.puertos().tomado(port)
        };
        if ocupado {
            continue;
        }
        let slot = match bmo_uhid::enumera::direccionar_puerto(port) {
            Some(s) => s,
            None => continue,
        };
        mirados += 1;
        let mut cfg = [0u8; bmo_uhid::enumera::MAX_CFG];
        let largo = match bmo_uhid::enumera::leer_descriptores(slot, &mut cfg) {
            Some((_, n)) => n,
            None => {
                cabina::warn("audio", "un aparato no dejo leer su descriptor, puerto", port as u64);
                continue;
            }
        };
        let Some(p) = bmo_uaudio::stream::find_playback(&cfg[..largo]) else {
            continue;
        };

        // -- The four numbers. Each with its unit, so nobody converts by hand.
        cabina::count("audio", "interfaz AudioStreaming, alt", p.alt_setting as u64);
        cabina::count("audio", "canales", p.channels as u64);
        cabina::count("audio", "bits por muestra", p.bits as u64);
        cabina::bytes("audio", "bytes por trama (wMaxPacketSize)", p.max_packet as u64);
        for i in 0..p.rates().len() {
            cabina::count("audio", "frecuencia que acepta", p.rates()[i] as u64);
        }

        // ** Y EL NUMERO QUE DECIDE SI ALGO VA A SONAR.
        //
        // Una trama de la frecuencia elegida tiene que CABER en el paquete. Si
        // no cabe, no hay codigo correcto que lo arregle -- y decirlo aqui evita
        // buscar el fallo en el driver que todavia no existe.
        match p.best_rate(48000) {
            Some(r) => {
                cabina::count("audio", "frecuencia elegida", r as u64);
                cabina::bytes("audio", "y una trama suya ocupa", p.bytes_per_interval(r) as u64);
            }
            None => {
                cabina::fault("audio", "ninguna frecuencia suya cabe en su propio paquete", 0);
            }
        }
        cabina::count("audio", "el endpoint isocrono es el DCI", p.dci as u64);
        // *** Y AQUI SE ABRE EL TUBO (A1, 25-08). Hasta hoy esta funcion
        // terminaba con los seis numeros apuntados y **sin haberle dicho al
        // aparato que se pusiera en su alt**, asi que el endpoint no existia.
        abrir(slot, &p);
        return true;
    }

    // Decir "no hay" y decir "no mire" son cosas distintas, y sin el numero de
    // puertos mirados se ven igual.
    cabina::count("audio", "puertos libres mirados, y ninguno reproduce", mirados);
    false
}

// ===================================================================
//  A1 -- SET_INTERFACE: lo unico que separaba de que suene
// ===================================================================

/// `bEndpointType` del xHC para un endpoint **isocrono de salida**.
///
/// [!] La tabla del xHCI no es la del USB: aqui `1` es Isoch OUT, `4` Control,
/// `5` Isoch IN y `7` Interrupt IN -- que es el que usa el teclado. Meter el
/// numero del USB da un endpoint configurado del tipo equivocado, y eso no
/// falla al configurarlo: falla al primer TRB.
const EP_ISOCH_OUT: u8 = 1;

/// `SET_INTERFACE`, peticion estandar 0x0B.
const REQ_SET_INTERFACE: u8 = 0x0B;
/// Host -> aparato, estandar, destinada a una INTERFAZ.
const A_LA_INTERFAZ: u8 = 0x01;

/// `SET_CUR`, peticion de clase para audio.
const REQ_SET_CUR: u8 = 0x01;
/// Host -> aparato, de CLASE, destinada a un ENDPOINT.
const AL_ENDPOINT_DE_CLASE: u8 = 0x22;
/// `SAMPLING_FREQ_CONTROL` en el byte alto de `wValue`.
const CTRL_FRECUENCIA: u16 = 0x0100;

/// Lo que quedo abierto, para que el bucle que alimente el tubo lo encuentre.
static mut TUBO: Option<Tubo> = None;

/// **Un tubo de audio abierto.** Todo lo que hace falta para empujar muestras.
#[derive(Clone, Copy)]
pub struct Tubo {
    pub slot: u8,
    pub dci: u8,
    /// La frecuencia que se le pidio al aparato, en Hz.
    pub frecuencia: u32,
    /// Bytes que hay que entregar por intervalo. **Tiene que caber en
    /// `max_packet`**, y eso se comprueba antes de abrir.
    pub bytes_por_trama: u32,
    pub max_packet: u16,
}

/// El tubo abierto, si lo hay.
pub fn tubo() -> Option<Tubo> {
    unsafe { TUBO }
}

/// **ABRIR EL TUBO: poner el aparato en el alt que trae el endpoint.**
///
/// # Por que esto es A1 y no un detalle
///
/// El paso 0 sabe **cual** es el alt setting. `queue_isoch_out` sabe encolar una
/// trama. Y entre los dos faltaba esto: **nadie le habia dicho al aparato que se
/// pusiera en ese alt**, asi que su endpoint isocrono no existia.
///
/// > Una interfaz AudioStreaming declara su endpoint **solo en los alt settings
/// > distintos de cero**. El alt 0 existe para que un aparato de audio pueda
/// > estar enchufado sin reservar ancho de banda isocrono en el bus.
///
/// # *** EL ORDEN DE LOS DOS PASOS, Y ES UNA DECISION
///
/// ```text
///    1. configurar el endpoint en el xHC   el HOST se prepara
///    2. SET_INTERFACE                      el APARATO empieza su reloj
///    3. SET_CUR frecuencia                 solo si declara mas de una
/// ```
///
/// Se hace en ese orden **para que el host este listo antes de que el aparato
/// arranque**. Al reves hay una ventana en la que el aparato ya espera datos en
/// cada microtrama y el xHC todavia no tiene ni anillo donde ponerlos.
///
/// [!] Y con `OUT` esa ventana no rompe nada --el aparato recibe silencio-- pero
/// **cuenta como tramas tarde**, y entonces el primer numero que se mira al
/// depurar estaria sucio desde antes de empezar. Ver `isoch_tarde`.
///
/// # La comprobacion que va antes de tocar el aparato
///
/// Que una trama de la frecuencia elegida **quepa en el paquete**. Si no cabe,
/// no hay codigo correcto que lo arregle -- y decirlo aqui evita buscar el fallo
/// en el bucle que todavia no existe.
pub fn abrir(slot: u8, p: &bmo_uaudio::stream::Playback) -> bool {
    let Some(frecuencia) = p.best_rate(48000) else {
        cabina::fault("audio", "ninguna frecuencia suya cabe en su propio paquete", 0);
        return false;
    };
    let bytes = p.bytes_per_interval(frecuencia);

    // 1. El HOST primero. Ver la cabecera.
    if !unsafe { bmo_xhci::configure_endpoint(slot, p.dci, EP_ISOCH_OUT, p.max_packet, p.interval) } {
        cabina::fault("audio", "el xHC no configuro el endpoint isocrono, dci", p.dci as u64);
        return false;
    }
    cabina::count("audio", "endpoint isocrono configurado, dci", p.dci as u64);

    // 2. Y ahora el aparato. `wValue` = alt, `wIndex` = interfaz.
    let mut vacio: [u8; 0] = [];
    unsafe {
        bmo_xhci::control_transfer(
            slot,
            A_LA_INTERFAZ,
            REQ_SET_INTERFACE,
            p.alt_setting as u16,
            p.interface as u16,
            &mut vacio,
            false,
        );
    }
    cabina::count("audio", "SET_INTERFACE, alt", p.alt_setting as u64);

    // 3. La frecuencia, **solo si hay mas de una que elegir**.
    //
    // ** Un aparato de una sola frecuencia puede contestar STALL a esta
    // peticion, y con razon: no hay nada que fijar. Mandarla igual dejaria un
    // error en el log de cada arranque -- y un error que sale siempre deja de
    // ser un error.
    if p.rates().len() > 1 || p.continuous {
        // Tres bytes, little-endian. UAC1 manda la frecuencia asi y no en
        // cuatro: el cuarto byte no existe en el protocolo, no es relleno.
        let mut hz = [
            (frecuencia & 0xFF) as u8,
            ((frecuencia >> 8) & 0xFF) as u8,
            ((frecuencia >> 16) & 0xFF) as u8,
        ];
        unsafe {
            bmo_xhci::control_transfer(
                slot,
                AL_ENDPOINT_DE_CLASE,
                REQ_SET_CUR,
                CTRL_FRECUENCIA,
                p.endpoint as u16,
                &mut hz,
                false,
            );
        }
        cabina::count("audio", "frecuencia pedida al aparato", frecuencia as u64);
    } else {
        cabina::count("audio", "una sola frecuencia: no hay nada que pedir", frecuencia as u64);
    }

    unsafe {
        TUBO = Some(Tubo {
            slot,
            dci: p.dci,
            frecuencia,
            bytes_por_trama: bytes,
            max_packet: p.max_packet,
        });
    }
    cabina::bytes("audio", "TUBO ABIERTO -- bytes por trama", bytes as u64);
    true
}

// ===================================================================
//  A2 en marcha -- el silencio, que es lo unico que no puede sonar mal
// ===================================================================

/// Marco fisico lleno de ceros. **Uno solo, y lo apuntan todas las tramas.**
///
/// El silencio es el mismo silencio: no hace falta un bufer por trama. Cuando
/// haya musica de verdad esto pasa a ser un anillo de bufers prestados (A4), y
/// **esa es la unica diferencia** entre este bucle y el definitivo.
static mut CEROS: u64 = 0;

/// Esta el tubo empujando?
static mut ARMADO: bool = false;

/// **Cuantas tramas se encolan en cada latido del bus.**
///
/// El bus late cada 4 ms y una trama isocrona dura 1 ms, asi que hacen falta
/// **cuatro** para cubrir el latido -- mas [`bmo_xhci::ISOCH_ADELANTO`] de
/// colchon, porque un latido que llegue tarde no puede dejar el tubo seco.
///
/// [!] Y pasarse tampoco es gratis: cada trama de mas es latencia que el que
/// escucha nota al parar la musica. Cuatro mas cuatro son 8 ms, que es lo que
/// `AUDIO_MAESTRO` llama audio y no un problema.
const TRAMAS_POR_LATIDO: usize = 4 + bmo_xhci::ISOCH_ADELANTO as usize;

/// **Armar o desarmar el empuje de silencio.** `false` deja de alimentar.
///
/// *** ESTO NO SE ENCIENDE SOLO AL ARRANCAR, Y ES A PROPOSITO.
///
/// Abrir el tubo --A1-- configura y no manda nada: es seguro. Empujar tramas
/// es trafico continuo en el bus a 250 latidos por segundo, y eso **no debe
/// pasar en cada arranque mientras no haya nada que reproducir**.
///
/// Es la regla de las hojas de metal: lo que no toca nada va primero, y esto se
/// pide **a proposito** o no ocurre.
pub fn armar_silencio(si: bool) -> bool {
    if unsafe { TUBO.is_none() } {
        cabina::warn("audio", "no hay tubo abierto que armar", 0);
        return false;
    }
    if si && unsafe { CEROS } == 0 {
        let Some(f) = crate::ring0::mm::phys::alloc_frame() else {
            cabina::fault("audio", "sin marco para el bufer de silencio", 0);
            return false;
        };
        crate::ring0::mm::phys::zero_frame(f);
        unsafe { CEROS = f };
    }
    unsafe { ARMADO = si };
    cabina::count("audio", if si { "tubo ARMADO: empujando silencio" } else { "tubo callado" }, 0);
    true
}

/// Esta armado?
pub fn armado() -> bool {
    unsafe { ARMADO }
}

/// **Empuja tramas y toca el timbre UNA vez.** La llama el hilo del bus.
///
/// # Por que el timbre va fuera del bucle
///
/// Tocar el timbre es un MMIO. Uno por trama serian 2.000 escrituras por segundo
/// para mover 192 bytes cada una -- **el aviso costaria mas que el dato**. El
/// xHC recorre el anillo entero desde donde estaba, asi que un solo timbre
/// despues de encolar las ocho es exactamente igual de efectivo.
pub fn latido() {
    if !unsafe { ARMADO } {
        return;
    }
    let Some(t) = tubo() else { return };
    let ceros = unsafe { CEROS };
    if ceros == 0 {
        return;
    }
    // ** La trama mide lo que el aparato pidio, no lo que quepa en la pagina.
    // Un `wMaxPacketSize` mas grande que la trama real es legal --el aparato
    // acepta hasta ahi-- y mandarle de mas seria inventar muestras.
    let largo = t.bytes_por_trama.min(t.max_packet as u32) as u16;
    for _ in 0..TRAMAS_POR_LATIDO {
        // *** LAS MUESTRAS DE VERDAD PRIMERO, Y SI NO HAY, SILENCIO **CONTADO**.
        //
        // Un hueco no se deja vacio: el endpoint tiene una cita cada
        // milisegundo y no esperar es todo el trato. Lo que cambia es que **se
        // apunta**, porque "sono un clic" y "el productor no llego a tiempo"
        // son dos cosas distintas y solo este contador las separa.
        let (donde, n) = match siguiente_trama(largo as u64) {
            Some(t) => t,
            None => {
                if unsafe { PRESTADO.is_some() } {
                    unsafe { HUECOS = HUECOS.wrapping_add(1) };
                }
                (ceros, largo)
            }
        };
        unsafe {
            if !bmo_xhci::queue_isoch_out(t.slot, t.dci, donde, n) {
                break;
            }
        }
    }
    unsafe { bmo_xhci::ring_doorbell(t.slot, t.dci) };
}

/// Los dos numeros que dicen si esto va bien. Ver `AUDIO_MAESTRO` parte 7.
pub fn cuentas() -> (u64, u64) {
    (bmo_xhci::isoch_encoladas(), bmo_xhci::isoch_tarde())
}

/// **Tramas que salieron en silencio porque el productor no llego.**
///
/// *** Y NO ES LO MISMO QUE `tramas tarde`, aunque las dos se oigan igual:
///
/// ```text
///    tarde    el xHC no llego a su cita        -> el problema es del BUS
///    huecos   nadie habia escrito la trama     -> el problema es de la APP
/// ```
///
/// Sin separarlas, un audio que chasquea manda a mirar el driver cuando la mitad
/// de las veces el que llega tarde es quien produce las muestras.
static mut HUECOS: u64 = 0;

/// Cuantas tramas salieron en silencio por falta de muestras.
pub fn huecos() -> u64 {
    unsafe { HUECOS }
}

// ===================================================================
//  A4 -- EL BUFER PRESTADO. Cero copias, y por que SMAP no estorba
// ===================================================================
//
// `AUDIO_MAESTRO` parte 4, y hay que decidirlo ANTES de escribir el primer
// `write` porque despues cuesta deshacerlo:
//
//    MAL   `audio_escribir(&muestras)` -> el kernel copia 192 bytes a su
//          anillo. Mil veces por segundo, mil cruces de puerta y mil copias
//
//    BIEN  la app pide un bloque, lo llena de PCM, y lo OFRECE. El aparato
//          lee de ahi. **La app escribe donde el aparato va a leer**
//
// *** Y AQUI HAY UNA COSA QUE SOLO SE VE DESPUES DE SMAP:
//
// Desde el 25-08 Ring 0 **no puede tocar memoria de Ring 3**. Un diseno que
// hiciera al kernel LEER las muestras del bufer de la app estaria muerto desde
// esa manana -- daria `#PF` en la primera trama.
//
// ** Este no lee nada. El TRB isocrono lleva una direccion **FISICA** y quien
// va a buscar los bytes es **el xHC por DMA**, no el CPU. El kernel solo
// traduce una VA a su fisica una vez, al ofrecer.
//
//    el que lee no es el CPU  ->  SMAP no tiene nada que decir
//
// [!] Y eso lo hace posible que `KIND_MEMORIA` entregue marcos **contiguos**:
// `Bloque` guarda una `fisica` y los bytes van seguidos detras. Si fueran
// paginas sueltas haria falta un TRB por pagina y el corte no caeria en la
// frontera de una trama.

/// El bufer que una app ofrecio, ya traducido a fisica.
#[derive(Clone, Copy)]
struct Prestado {
    /// El pid que lo ofrecio. Si muere, se suelta.
    pid: u32,
    /// La base FISICA. Los `bytes` siguientes van seguidos.
    fisica: u64,
    bytes: u64,
    /// **Hasta donde ha escrito la app.** Lo mueve ella.
    escrito: u64,
    /// **Por donde va el tubo.** Lo mueve el latido.
    leido: u64,
}

static mut PRESTADO: Option<Prestado> = None;

/// **Adoptar el bufer de una app.** `va` y `bytes` son del bloque que ella pidio.
///
/// Devuelve `false` si esa VA no es suya -- que es lo que impide que una app
/// ofrezca la memoria de otra: `fisica_de` busca en SUS bloques y en ninguno mas.
pub fn ofrecer(pid: u32, va: u64, bytes: u64) -> bool {
    let Some(fisica) = crate::ring0::obj::memory::fisica_de(pid, va, bytes) else {
        cabina::warn("audio", "esa memoria no es de quien la ofrece, pid", pid as u64);
        return false;
    };
    if unsafe { TUBO.is_none() } {
        cabina::warn("audio", "no hay tubo abierto al que ofrecer", 0);
        return false;
    }
    unsafe { PRESTADO = Some(Prestado { pid, fisica, bytes, escrito: 0, leido: 0 }) };
    cabina::bytes("audio", "bufer PRESTADO al tubo, bytes", bytes);
    true
}

/// La app dice **hasta donde ha escrito**. Es uno de los dos numeros que cruzan.
///
/// [!] Solo puede CRECER dentro de la vuelta. Un `escrito` que retroceda seria
/// la app pisando lo que el aparato todavia no ha leido, y eso se oye.
pub fn escrito(pid: u32, hasta: u64) -> bool {
    unsafe {
        match PRESTADO.as_mut() {
            Some(p) if p.pid == pid && hasta <= p.bytes => {
                p.escrito = hasta;
                true
            }
            _ => false,
        }
    }
}

/// Y el tubo dice **por donde va**. El otro numero.
pub fn leido() -> u64 {
    unsafe { PRESTADO.map(|p| p.leido).unwrap_or(0) }
}

/// Cuantos bytes hay listos y sin entregar.
pub fn pendientes() -> u64 {
    unsafe {
        match PRESTADO {
            Some(p) => p.escrito.saturating_sub(p.leido),
            None => 0,
        }
    }
}

/// Soltar el prestamo. Lo llama tambien la muerte del proceso.
pub fn soltar(pid: u32) {
    unsafe {
        if let Some(p) = PRESTADO {
            if p.pid == pid {
                PRESTADO = None;
                cabina::count("audio", "bufer prestado SOLTADO, pid", pid as u64);
            }
        }
    }
}

/// **Una trama del bufer prestado**, o `None` si no hay nada listo.
///
/// Devuelve la fisica de donde empieza y cuantos bytes son, y **avanza el
/// indice**. No copia ni un byte: lo que se devuelve es una direccion.
fn siguiente_trama(largo: u64) -> Option<(u64, u16)> {
    unsafe {
        let p = PRESTADO.as_mut()?;
        let hay = p.escrito.checked_sub(p.leido)?;
        if hay < largo {
            // ** MEDIA TRAMA NO SE MANDA. Entregar los bytes que hay y rellenar
            // con lo que fuera es inventar muestras -- y lo que se inventa en
            // audio no se ve, se OYE. Mejor una trama de silencio.
            return None;
        }
        let desde = p.fisica + p.leido;
        p.leido += largo;
        // La vuelta al principio: el bufer es circular por acuerdo con la app,
        // que reinicia su `escrito` al mismo tiempo.
        if p.leido >= p.bytes {
            p.leido = 0;
        }
        Some((desde, largo as u16))
    }
}
