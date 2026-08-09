//! **El VOLUMEN del audifono USB**, por control transfer.
//!
//! ## Por que esto llega antes que oir nada
//!
//! Reproducir muestras por USB pide transferencias **isocronas**, que
//! `bmo-xhci` no tiene: hoy sabe control e interrupt. Eso es la casilla 2.4 de
//! `docs/LIDERES.md` y es una pieza XL.
//!
//! El volumen no. Es un `SET_CUR` sobre el *Feature Unit* de la interfaz de
//! control del aparato, o sea **un control transfer** -- los mismos que enumeran
//! el teclado y el raton en cada arranque. Asi que BMO-X puede mandar sobre el
//! audifono **antes de reproducir una sola muestra**.
//!
//! Y en esta maquina eso importa mas de lo que parece: el altavoz del PC no
//! suena porque la placa no trae zumbador (`aparatos = 1` y silencio, visto en
//! el Ryzen el 2026-08-09). El aparato que el dueno usa de verdad es
//! `VID_1B3F&PID_2008`, un USB Audio Class 1.0. Este es el primer camino de
//! sonido que puede tener efecto audible en esta maquina.
//!
//! ## El reparto con `bmo-uaudio`
//!
//! Aqui se toca el hardware y **nada mas**: pedir el descriptor, mandar las
//! peticiones. Quien decide QUE mandar --como se lee un descriptor, como se
//! monta un `wIndex`, como se convierte un porcentaje en decibelios-- vive en
//! `platform/drivers/usb/uaudio`, que no depende de nada y **se prueba entero en
//! el anfitrion**: diez filas sin encender la maquina.
//!
//! Es el mismo reparto que hizo util al driver del raton: la decision separada
//! del registro.

use core::sync::atomic::{AtomicBool, AtomicI16, AtomicU8, Ordering};

/// El slot xHCI del aparato de audio, o 0 si no se ha encontrado.
static SLOT: AtomicU8 = AtomicU8::new(0);
/// Datos del Feature Unit, ya localizados. Se guardan sueltos porque
/// `AudioControl` no es atomico y aqui no hay cerrojo que valga la pena.
static IFACE: AtomicU8 = AtomicU8::new(0);
static UNIT: AtomicU8 = AtomicU8::new(0);
/// Rango que declaro el aparato, en 1/256 dB. **No hay uno estandar**: se le
/// pregunta con `GET_MIN`/`GET_MAX` y suponerlo es el mismo error que suponer
/// el formato del informe HID.
static VOL_MIN: AtomicI16 = AtomicI16::new(0);
static VOL_MAX: AtomicI16 = AtomicI16::new(0);
static BUSCADO: AtomicBool = AtomicBool::new(false);

/// Hay un aparato de audio USB localizado y con volumen?
pub fn hay() -> bool {
    SLOT.load(Ordering::SeqCst) != 0
}

/// Busca un aparato USB Audio entre los slots enumerados, UNA sola vez.
///
/// Se barren los slots en vez de engancharse a la enumeracion a proposito: el
/// camino de enumeracion de `dev/usb.rs` costo mucho estabilizarse --el bucle
/// que se comia a si mismo, el anillo de eventos compartido-- y meterle un
/// tercer interesado ahora seria tocar lo unico del USB que ya funciona.
///
/// El precio esta dicho: si el aparato se enchufa DESPUES, no se ve hasta que
/// alguien vuelva a llamar a [`olvidar`].
pub fn buscar() {
    if BUSCADO.swap(true, Ordering::SeqCst) {
        return;
    }
    let mut buf = [0u8; 256];
    // Los slots bajos son los que reparte el xHC al enumerar. Ocho sobran para
    // esta maquina y no cuesta nada equivocarse por arriba.
    for slot in 1u8..=8 {
        let n = unsafe { bmo_xhci::get_config_descriptor(slot, 0, &mut buf) };
        if n == 0 {
            continue;
        }
        let cfg = &buf[..n.min(buf.len())];
        if let Some(ac) = bmo_uaudio::find_audio_control(cfg) {
            if !ac.has_volume {
                // Existe y NO deja cambiar el volumen. Es un caso real, y la
                // respuesta correcta es decirlo, no fingir que se puso.
                crate::ring0::cabina::warn(
                    "uaudio",
                    "aparato de audio SIN control de volumen",
                    slot as u64,
                );
                continue;
            }
            SLOT.store(slot, Ordering::SeqCst);
            IFACE.store(ac.interface, Ordering::SeqCst);
            UNIT.store(ac.feature_unit, Ordering::SeqCst);
            leer_rango(&ac);
            crate::ring0::cabina::info("uaudio", "audifono USB con volumen", slot as u64);
            return;
        }
    }
}

/// Vuelve a mirar en la proxima llamada. Para cuando se enchufa algo despues.
pub fn olvidar() {
    BUSCADO.store(false, Ordering::SeqCst);
    SLOT.store(0, Ordering::SeqCst);
}

/// Le pregunta al aparato **su** rango. Si no contesta, se queda un rango
/// conservador -- pero se avisa, porque un rango inventado da un volumen que
/// salta de mudo a ensordecedor.
fn leer_rango(ac: &bmo_uaudio::AudioControl) {
    let mut min = leer(ac, bmo_uaudio::GET_MIN);
    let max = leer(ac, bmo_uaudio::GET_MAX);
    match (min, max) {
        (Some(a), Some(b)) if b > a => {
            VOL_MIN.store(a, Ordering::SeqCst);
            VOL_MAX.store(b, Ordering::SeqCst);
            return;
        }
        _ => {}
    }
    // -60 dB a 0 dB, que es el rango tipico de un aparato de estos.
    min = Some(-15360);
    VOL_MIN.store(min.unwrap_or(-15360), Ordering::SeqCst);
    VOL_MAX.store(0, Ordering::SeqCst);
    crate::ring0::cabina::warn("uaudio", "el aparato no dijo su rango: se supone", 0);
}

fn leer(ac: &bmo_uaudio::AudioControl, cual: u8) -> Option<i16> {
    let r = bmo_uaudio::get_volume(ac, bmo_uaudio::CHANNEL_MASTER, cual);
    let mut buf = [0u8; 2];
    let n = unsafe {
        bmo_xhci::control_transfer(
            SLOT.load(Ordering::SeqCst),
            r.bm_request_type,
            r.b_request,
            r.w_value,
            r.w_index,
            &mut buf,
            true,
        )
    };
    if n < 2 {
        return None;
    }
    Some(i16::from_le_bytes(buf))
}

/// Pone el volumen del audifono, de 0 a 100. Devuelve si el aparato lo acepto.
///
/// [!] El porcentaje NO se manda tal cual: se convierte a decibelios con la
/// curva de `bmo_uaudio::percent_to_volume` y se recorta al rango que declaro el
/// aparato. El campo va en 1/256 dB con signo, y **el 0% no es el valor 0** --
/// ese es 0 dB, o sea el maximo. Confundirlos pone el audifono a tope creyendo
/// que se apaga, con los cascos puestos.
pub fn set_volume(pct: u8) -> bool {
    buscar();
    let slot = SLOT.load(Ordering::SeqCst);
    if slot == 0 {
        return false;
    }
    let ac = bmo_uaudio::AudioControl {
        interface: IFACE.load(Ordering::SeqCst),
        feature_unit: UNIT.load(Ordering::SeqCst),
        channels: 2,
        has_volume: true,
        has_mute: true,
    };
    let valor = bmo_uaudio::percent_to_volume(
        pct,
        VOL_MIN.load(Ordering::SeqCst),
        VOL_MAX.load(Ordering::SeqCst),
    );
    let r = bmo_uaudio::set_volume(&ac, bmo_uaudio::CHANNEL_MASTER, valor);
    let mut datos = valor.to_le_bytes();
    let n = unsafe {
        bmo_xhci::control_transfer(
            slot,
            r.bm_request_type,
            r.b_request,
            r.w_value,
            r.w_index,
            &mut datos,
            false,
        )
    };
    // Un STALL devuelve 0 bytes. Se dice: un volumen que no llego y se cuenta
    // como puesto es un control que miente, y esos se descubren girando la
    // rueda sin que pase nada.
    if n == 0 {
        crate::ring0::cabina::warn("uaudio", "el aparato rechazo el volumen", pct as u64);
        return false;
    }
    true
}
