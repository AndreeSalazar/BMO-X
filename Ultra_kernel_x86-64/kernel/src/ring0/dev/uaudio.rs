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
/// Lo que el Feature Unit declaro. Se GUARDA en vez de suponerse en cada
/// llamada: mandar un mute a un aparato que no lo tiene es un STALL, y un
/// STALL contado como fallo esconde el volumen que si llego.
static TIENE_MUTE: AtomicBool = AtomicBool::new(false);
static CANALES: AtomicU8 = AtomicU8::new(0);
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
    let mut buf = [0u8; DESCRIPTOR_MAX];
    // Los slots bajos son los que reparte el xHC al enumerar. Ocho sobran para
    // esta maquina y no cuesta nada equivocarse por arriba.
    for slot in 1u8..=8 {
        let Some(n) = leer_configuracion(slot, &mut buf) else {
            continue;
        };
        let cfg = &buf[..n];
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
            TIENE_MUTE.store(ac.has_mute, Ordering::SeqCst);
            CANALES.store(ac.channels, Ordering::SeqCst);
            leer_rango(&ac);
            crate::ring0::cabina::info("uaudio", "audifono USB con volumen", slot as u64);
            return;
        }
    }
    // Que NO haya nada tambien es una respuesta, y hasta ahora se daba
    // callando. Una linea que no sale no distingue "mire y no habia" de "no
    // llegue a mirar", y son dos sitios distintos donde buscar.
    crate::ring0::cabina::info("uaudio", "ningun aparato de audio en los slots 1..8", 0);
}

/// Cuanto descriptor de configuracion cabe. **512 y no 256**: ver
/// [`leer_configuracion`].
const DESCRIPTOR_MAX: usize = 512;

/// Lee el descriptor de configuracion ENTERO de un slot, en dos pasos.
///
/// # Por que dos pasos, y no una lectura y ya
///
/// Un control transfer entrega **como mucho lo que se le pide**: el `wLength`
/// es `buf.len()`. Con un buffer de 256 bytes, un aparato cuyo descriptor mida
/// mas devuelve los 256 primeros **y el resto no existe para nosotros**.
///
/// Y eso importa justo aqui: un audifono USB corriente trae cuatro interfaces
/// --AudioControl, dos de AudioStreaming y una HID para los botones-- y pasa de
/// los 256 bytes con facilidad. El Feature Unit puede quedar **detras** del
/// corte. El sintoma seria un aparato enchufado, enumerado y funcionando al que
/// este codigo dice que no encuentra: ni una linea, ni una causa.
///
/// Asi que primero se piden los **9 bytes de la cabecera**, que traen
/// `wTotalLength`, y luego se pide exactamente eso. Es lo que hace cualquier
/// pila USB, y por el mismo motivo.
fn leer_configuracion(slot: u8, buf: &mut [u8; DESCRIPTOR_MAX]) -> Option<usize> {
    let mut cab = [0u8; 9];
    let n = unsafe { bmo_xhci::get_config_descriptor(slot, 0, &mut cab) };
    if n < 9 {
        return None;
    }
    // wTotalLength va en los bytes 2 y 3 del descriptor de CONFIGURACION.
    let total = u16::from_le_bytes([cab[2], cab[3]]) as usize;
    if total < 9 {
        return None;
    }
    if total > DESCRIPTOR_MAX {
        // Se lee lo que cabe y se sigue, pero se DICE: si luego no aparece el
        // Feature Unit, esta linea es la diferencia entre "no es de audio" y
        // "no me cupo".
        crate::ring0::cabina::warn(
            "uaudio",
            "el descriptor no cabe entero: puede que el Feature Unit quede fuera",
            total as u64,
        );
    }
    let quiero = total.min(DESCRIPTOR_MAX);
    let n = unsafe { bmo_xhci::get_config_descriptor(slot, 0, &mut buf[..quiero]) };
    if n == 0 {
        return None;
    }
    Some(n.min(quiero))
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
    let min = leer(ac, bmo_uaudio::GET_MIN);
    let max = leer(ac, bmo_uaudio::GET_MAX);
    if let (Some(a), Some(b)) = (min, max) {
        // La validacion vive en `bmo-uaudio` y no aqui: un rango del reves y un
        // minimo que en realidad es el marcador de silencio son decisiones, y
        // las decisiones se prueban en el anfitrion.
        if let Some((a, b)) = bmo_uaudio::rango(a, b) {
            VOL_MIN.store(a, Ordering::SeqCst);
            VOL_MAX.store(b, Ordering::SeqCst);
            return;
        }
    }
    // -60 dB a 0 dB, que es el rango tipico de un aparato de estos.
    VOL_MIN.store(-15360, Ordering::SeqCst);
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
    let ac = actual();
    let plan = bmo_uaudio::plan(
        pct,
        VOL_MIN.load(Ordering::SeqCst),
        VOL_MAX.load(Ordering::SeqCst),
        ac.has_mute,
    );
    match plan {
        bmo_uaudio::Plan::Callar => mandar_mute(slot, &ac, true),
        bmo_uaudio::Plan::Poner { valor, quitar_mute } => {
            // El mute va DELANTE del volumen: si el aparato se quedo callado
            // de la vez anterior, mandar solo el volumen deja un aparato que
            // acepta el numero y no suena -- que parece el camino roto entero.
            if quitar_mute {
                mandar_mute(slot, &ac, false);
            }
            mandar_volumen(slot, &ac, pct, valor)
        }
    }
}

/// El Feature Unit tal como lo declaro el aparato. **Se lee de lo guardado**, y
/// no se inventa: la version anterior construia esta struct con
/// `channels: 2, has_mute: true` a pelo en cada llamada, o sea que le mandaba
/// un mute a un aparato que podia no tenerlo.
fn actual() -> bmo_uaudio::AudioControl {
    bmo_uaudio::AudioControl {
        interface: IFACE.load(Ordering::SeqCst),
        feature_unit: UNIT.load(Ordering::SeqCst),
        channels: CANALES.load(Ordering::SeqCst),
        has_volume: true,
        has_mute: TIENE_MUTE.load(Ordering::SeqCst),
    }
}

fn mandar_mute(slot: u8, ac: &bmo_uaudio::AudioControl, callar: bool) -> bool {
    let r = bmo_uaudio::set_mute(ac, bmo_uaudio::CHANNEL_MASTER, callar);
    let mut datos = [if callar { 1u8 } else { 0u8 }];
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
    if n == 0 {
        crate::ring0::cabina::warn("uaudio", "el aparato rechazo el mute", callar as u64);
        return false;
    }
    true
}

/// Manda el volumen, y si el canal maestro no vale, **prueba canal por canal**.
///
/// # Por que la segunda vuelta
///
/// El canal 0 (maestro) es opcional. Hay aparatos --sobre todo los que separan
/// izquierdo y derecho-- que **solo** aceptan el volumen por canal y contestan
/// STALL al maestro. Sin esta vuelta, ese aparato queda para siempre como "el
/// aparato rechazo el volumen": un aparato con volumen perfectamente
/// controlable al que le estabamos hablando por el canal que no era.
fn mandar_volumen(slot: u8, ac: &bmo_uaudio::AudioControl, pct: u8, valor: i16) -> bool {
    if escribir_volumen(slot, ac, bmo_uaudio::CHANNEL_MASTER, valor) {
        confirmar(slot, ac, bmo_uaudio::CHANNEL_MASTER, pct, valor);
        return true;
    }
    let mut alguno = false;
    for canal in 1..=ac.channels {
        if escribir_volumen(slot, ac, canal, valor) {
            alguno = true;
        }
    }
    if alguno {
        crate::ring0::cabina::info(
            "uaudio",
            "el maestro no valia: el volumen va por canal",
            ac.channels as u64,
        );
        return true;
    }
    crate::ring0::cabina::warn("uaudio", "el aparato rechazo el volumen", pct as u64);
    false
}

fn escribir_volumen(slot: u8, ac: &bmo_uaudio::AudioControl, canal: u8, valor: i16) -> bool {
    let r = bmo_uaudio::set_volume(ac, canal, valor);
    let mut datos = valor.to_le_bytes();
    // Un STALL devuelve 0 bytes. Se dice: un volumen que no llego y se cuenta
    // como puesto es un control que miente, y esos se descubren girando la
    // rueda sin que pase nada.
    unsafe {
        bmo_xhci::control_transfer(
            slot,
            r.bm_request_type,
            r.b_request,
            r.w_value,
            r.w_index,
            &mut datos,
            false,
        ) != 0
    }
}

/// Le vuelve a preguntar al aparato **que volumen tiene puesto**, y lo dice.
///
/// # Por que no basta con que el `SET_CUR` no diera STALL
///
/// Que la peticion se acepte prueba que llego, no que hiciera algo. Un aparato
/// puede aceptar el valor y recortarlo, redondearlo a su paso (`GET_RES`) o
/// ignorarlo. Con el `GET_CUR` de vuelta, el arranque deja escrito **el numero
/// que el aparato dice tener** al lado del que se le mando: si son distintos,
/// no hay que adivinar por que la oreja no nota el cambio.
///
/// Solo se dice cuando NO coinciden. Una linea por cada pulsacion de flecha
/// llenaria CABINA de ruido y taparia justo lo que hay que ver.
fn confirmar(slot: u8, ac: &bmo_uaudio::AudioControl, canal: u8, pct: u8, mandado: i16) {
    let r = bmo_uaudio::get_volume(ac, canal, bmo_uaudio::GET_CUR);
    let mut buf = [0u8; 2];
    let n = unsafe {
        bmo_xhci::control_transfer(
            slot,
            r.bm_request_type,
            r.b_request,
            r.w_value,
            r.w_index,
            &mut buf,
            true,
        )
    };
    if n < 2 {
        return;
    }
    let tiene = i16::from_le_bytes(buf);
    if tiene != mandado {
        crate::ring0::cabina::info("uaudio", "el aparato guardo OTRO volumen", pct as u64);
    }
}
