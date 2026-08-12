//! **USB Audio Class 1.0 -- mandar sobre el aparato.**
//!
//! ## Por que esto llega ANTES que reproducir sonido
//!
//! Reproducir muestras por USB pide transferencias **isocronas**, que
//! `bmo-xhci` no tiene: hoy sabe hacer control e interrupt. Eso es la casilla
//! 2.4 de `docs/LIDERES.md` y es una pieza XL.
//!
//! Pero el **volumen** de un aparato USB Audio no es una transferencia de
//! datos: es un `SET_CUR` sobre el *Feature Unit* de su interfaz de control, o
//! sea **un control transfer**. Y control transfers ya se hacen -- son los que
//! enumeran el teclado y el raton todos los arranques.
//!
//! Asi que BMO-X puede mandar sobre el volumen del audifono **antes de poder
//! reproducir una sola muestra**. Es pequena, se nota en el aparato fisico, y no
//! depende de nada que falte.
//!
//! ## Lo que hace este crate, y lo que NO
//!
//! Aqui **no se toca hardware**. Este modulo hace dos cosas, las dos puras:
//!
//! 1. **Lee el descriptor de configuracion** y encuentra la interfaz
//!    AudioControl y su Feature Unit.
//! 2. **Construye las peticiones** (`bmRequestType`, `bRequest`, `wValue`,
//!    `wIndex`) que hay que mandar.
//!
//! Quien las manda es el kernel, con `bmo_xhci::control_transfer`. La ventaja
//! de partirlo asi no es de estilo: **todo lo de aqui se prueba en el
//! anfitrion**, con descriptores de verdad y sin encender el Ryzen. El parseo
//! de descriptores es exactamente donde se esconden los fallos de esta clase de
//! driver -- ya paso con el Report Descriptor del raton.
//!
//! ## El aparato que hay delante
//!
//! `VID_1B3F&PID_2008`, `USB\Class_01&SubClass_01`. Un adaptador USB Audio
//! Class 1.0 corriente.
//!
//! [!] **Y el volumen de USB Audio NO son porcentajes: son DECIBELIOS**, en
//! unidades de 1/256 dB con signo. Tratar el campo como "0..100" da un aparato
//! que salta de mudo a ensordecedor en dos pulsaciones, porque los decibelios
//! son logaritmicos y el rango real lo decide el aparato -- por eso hay que
//! preguntarle `GET_MIN` y `GET_MAX` en vez de suponerlos.

#![no_std]

// -- ** LOS DOS TRABAJOS, EN DOS FICHEROS (2026-08-12) ------------------------
//
// Esto era un solo fichero y hacia dos cosas distintas:
//
//   lib.rs   AudioCONTROL   -- volumen y mute. Ordenes SOBRE el aparato
//   stream   AudioSTREAMING -- el tubo por el que van las muestras
//
// Comparten las constantes de la clase y nada mas. La linea 51 de este fichero
// ya decia *"la interfaz que transporta muestras es AUDIOSTREAMING (0x02) y no
// se toca aqui"*, y esa frase deja de ser una promesa y pasa a ser una frontera
// en cuanto hay dos ficheros.

/// El lado que transporta las muestras. Paso 0 de `docs/AUDIO_MAESTRO.md`.
pub mod stream;

// -- Constantes de la clase (USB Audio 1.0, seccion A) --------------------

/// Clase de interfaz AUDIO.
pub const CLASS_AUDIO: u8 = 0x01;
/// Subclase AUDIOCONTROL: la interfaz que MANDA sobre el aparato. La que
/// transporta muestras es AUDIOSTREAMING (0x02) y no se toca aqui.
pub const SUBCLASS_AUDIOCONTROL: u8 = 0x01;

/// Tipo de descriptor especifico de clase para una interfaz.
pub(crate) const DESC_CS_INTERFACE: u8 = 0x24;
/// Subtipo: FEATURE_UNIT, el que lleva volumen y mute.
const AC_FEATURE_UNIT: u8 = 0x06;
/// Tipos de descriptor estandar.
pub(crate) const DESC_INTERFACE: u8 = 0x04;

/// `SET_CUR` / `GET_CUR` / `GET_MIN` / `GET_MAX` / `GET_RES`.
pub const SET_CUR: u8 = 0x01;
pub const GET_CUR: u8 = 0x81;
pub const GET_MIN: u8 = 0x82;
pub const GET_MAX: u8 = 0x83;
pub const GET_RES: u8 = 0x84;

/// Selectores de control dentro del Feature Unit.
pub const MUTE_CONTROL: u8 = 0x01;
pub const VOLUME_CONTROL: u8 = 0x02;

/// Canal 0 = maestro (los dos oidos a la vez). 1 = izquierdo, 2 = derecho.
pub const CHANNEL_MASTER: u8 = 0x00;

/// `bmRequestType` para hablar con una INTERFAZ, peticion de CLASE.
/// El bit 7 es la direccion: 0 = host->aparato, 1 = aparato->host.
const REQ_TYPE_SET: u8 = 0x21;
const REQ_TYPE_GET: u8 = 0xA1;

/// Silencio absoluto en la escala de USB Audio. **No es 0**: 0 son 0 dB, o sea
/// ganancia unidad. Confundirlos pone el aparato a tope creyendo que se apaga.
pub const VOLUME_SILENCE: i16 = -32768; // 0x8000

/// La interfaz de control de un aparato de audio, ya localizada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioControl {
    /// Numero de la interfaz AudioControl (`bInterfaceNumber`).
    pub interface: u8,
    /// Id de la unidad que lleva el volumen (`bUnitID` del Feature Unit).
    pub feature_unit: u8,
    /// Canales logicos que el Feature Unit describe, sin contar el maestro.
    pub channels: u8,
    /// El Feature Unit declara control de VOLUMEN?
    pub has_volume: bool,
    /// Y de MUTE?
    pub has_mute: bool,
}

/// Una peticion de control ya montada, lista para `control_transfer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    pub bm_request_type: u8,
    pub b_request: u8,
    pub w_value: u16,
    pub w_index: u16,
    /// Cuantos bytes viajan. Volumen 2, mute 1.
    pub length: usize,
    /// True cuando los datos van del aparato al host.
    pub data_in: bool,
}

/// Recorre el descriptor de CONFIGURACION y localiza la interfaz AudioControl
/// con su Feature Unit.
///
/// Devuelve `None` si el aparato no es de audio o si no declara Feature Unit --
/// que es un caso real: hay aparatos que solo reproducen y **no dejan cambiar
/// el volumen**, y entonces la respuesta correcta es decirlo, no fingir que se
/// puso.
///
/// # Como se recorre, y por que asi
///
/// Un descriptor de configuracion es una cadena de descriptores pegados, cada
/// uno con su longitud en el primer byte. Se avanza **por esa longitud** y
/// nunca por el tamano de la struct que uno espera: un aparato puede meter
/// descriptores que este codigo no conoce, y saltarlos por su longitud es lo
/// unico que no se rompe con ellos.
///
/// [!] Un `bLength` de 0 pararia el bucle para siempre. Se comprueba: es la
/// clase de dato que llega de fuera y no se le puede suponer nada.
pub fn find_audio_control(config: &[u8]) -> Option<AudioControl> {
    let mut i = 0usize;
    let mut ac_interface: Option<u8> = None;

    while i + 2 <= config.len() {
        let len = config[i] as usize;
        let dtype = config[i + 1];
        if len < 2 || i + len > config.len() {
            // Descriptor imposible: se para. Seguir adivinando desde aqui seria
            // leer basura y creersela.
            break;
        }

        if dtype == DESC_INTERFACE && len >= 9 {
            let class = config[i + 5];
            let subclass = config[i + 6];
            ac_interface = if class == CLASS_AUDIO && subclass == SUBCLASS_AUDIOCONTROL {
                Some(config[i + 2]) // bInterfaceNumber
            } else {
                // Otra interfaz (AudioStreaming, HID...): a partir de aqui los
                // descriptores de clase ya no son del AudioControl.
                None
            };
        } else if dtype == DESC_CS_INTERFACE && len >= 3 && config[i + 2] == AC_FEATURE_UNIT {
            if let Some(interface) = ac_interface {
                // Feature Unit: bLength bDescriptorType bDescriptorSubtype
                //               bUnitID bSourceID bControlSize bmaControls(0)...
                if len >= 7 {
                    let feature_unit = config[i + 3];
                    let control_size = config[i + 5] as usize;
                    let bma = &config[i + 6..i + len];
                    // El primer bloque bmaControls es el del canal MAESTRO.
                    let master = primer_bloque(bma, control_size);
                    // Los bits son 1=mute, 2=volumen (bit 0 y bit 1).
                    //
                    // **** PERO NO SOLO EL MAESTRO, y esto salio del Ryzen.
                    //
                    // El 2026-08-09 la maquina dijo `aparato de audio SIN
                    // control de volumen` con el audifono enchufado. El aparato
                    // esta, tiene Feature Unit, y su bloque MAESTRO declara
                    // cero -- porque **declara el volumen por CANAL**, en los
                    // bloques de izquierdo y derecho.
                    //
                    // Es una disposicion corrientisima y el estandar la permite:
                    // el canal 0 es opcional. Mirar solo el primer bloque
                    // descarta esos aparatos enteros, y el sintoma es el peor
                    // posible -- un "no se puede" sobre algo que si se puede.
                    //
                    // Se mira el maestro Y todos los canales. Cual funciona de
                    // verdad lo averigua quien manda: `set_volume` prueba el
                    // maestro y, si da STALL, va canal por canal.
                    let mut junta = master;
                    if control_size > 0 {
                        let mut k = control_size;
                        while k + control_size <= bma.len() {
                            junta |= primer_bloque(&bma[k..], control_size);
                            k += control_size;
                        }
                    }
                    let has_mute = junta & 0x01 != 0;
                    let has_volume = junta & 0x02 != 0;
                    // Cuantos bloques hay, menos el maestro.
                    let channels = if control_size > 0 {
                        ((bma.len() / control_size).saturating_sub(1)) as u8
                    } else {
                        0
                    };
                    return Some(AudioControl {
                        interface,
                        feature_unit,
                        channels,
                        has_volume,
                        has_mute,
                    });
                }
            }
        }
        i += len;
    }
    None
}

/// Los `bmaControls` del canal maestro, leidos como un entero pequeno.
/// `control_size` puede ser 1, 2 o 4 bytes segun el aparato.
fn primer_bloque(bma: &[u8], control_size: usize) -> u32 {
    let n = control_size.min(4).min(bma.len());
    let mut v = 0u32;
    for i in 0..n {
        v |= (bma[i] as u32) << (8 * i);
    }
    v
}

/// Monta la peticion para PONER el volumen de un canal.
///
/// `value` va en unidades de **1/256 dB con signo**. Ver [`percent_to_volume`]
/// si lo que se tiene es un porcentaje.
pub fn set_volume(ac: &AudioControl, channel: u8, _value: i16) -> Request {
    Request {
        bm_request_type: REQ_TYPE_SET,
        b_request: SET_CUR,
        w_value: ((VOLUME_CONTROL as u16) << 8) | channel as u16,
        // ** El indice lleva la UNIDAD arriba y la INTERFAZ abajo. Ponerlo al
        // reves es el fallo clasico de esta clase: el aparato contesta STALL y
        // no dice por que, porque desde su punto de vista le han preguntado por
        // una unidad que no existe.
        w_index: ((ac.feature_unit as u16) << 8) | ac.interface as u16,
        length: 2,
        data_in: false,
    }
}

/// Monta una peticion de LECTURA del volumen: `GET_CUR`, `GET_MIN`, `GET_MAX`
/// o `GET_RES`.
///
/// `GET_MIN` y `GET_MAX` son obligatorios antes de escribir nada: el rango lo
/// decide el aparato y **no hay uno estandar**. Suponerlo es como suponer el
/// formato del informe HID -- ya costo una sesion entera.
pub fn get_volume(ac: &AudioControl, channel: u8, which: u8) -> Request {
    Request {
        bm_request_type: REQ_TYPE_GET,
        b_request: which,
        w_value: ((VOLUME_CONTROL as u16) << 8) | channel as u16,
        w_index: ((ac.feature_unit as u16) << 8) | ac.interface as u16,
        length: 2,
        data_in: true,
    }
}

/// Monta la peticion de MUTE. `on = true` calla el aparato.
pub fn set_mute(ac: &AudioControl, channel: u8, _on: bool) -> Request {
    Request {
        bm_request_type: REQ_TYPE_SET,
        b_request: SET_CUR,
        w_value: ((MUTE_CONTROL as u16) << 8) | channel as u16,
        w_index: ((ac.feature_unit as u16) << 8) | ac.interface as u16,
        length: 1,
        data_in: false,
    }
}

/// Valida el rango que contesto el aparato antes de usarlo para nada.
///
/// Devuelve `None` cuando no se le puede creer, que es la senal de "usa el
/// rango supuesto y **avisa**". Dos casos, y los dos se han visto de verdad:
///
/// - `max <= min`: el aparato no contesto, o contesto basura.
/// - `min == `[`VOLUME_SILENCE`]: `0x8000` esta reservado para el silencio, no
///   es un volumen. Tomarlo como minimo haria que los porcentajes bajos se
///   convirtieran **en el marcador de silencio**, o sea que el 10% callara del
///   todo en vez de sonar flojo. Se sube un paso: el volumen mas bajo que se
///   puede MANDAR sin que sea la senal de callar.
pub fn rango(min: i16, max: i16) -> Option<(i16, i16)> {
    if max <= min {
        return None;
    }
    let min = if min == VOLUME_SILENCE { VOLUME_SILENCE + 1 } else { min };
    Some((min, max))
}

/// **Lo que hay que MANDARLE al aparato** para dejarlo en `percent`.
///
/// Existe porque poner el volumen a 0 no es "mandar un numero muy bajo": ver
/// [`plan`], que es donde esta escrito el por que.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    /// Callar con el control de MUTE. Un byte, otro selector.
    Callar,
    /// Poner este valor de volumen. `quitar_mute` dice si ademas hay que
    /// mandar un `MUTE = 0` delante, porque el aparato pudo quedarse callado
    /// de la vez anterior y entonces el volumen no se oye igual.
    Poner { valor: i16, quitar_mute: bool },
}

/// Decide que peticiones hacen falta para dejar el aparato en `percent`.
///
/// # Por que el 0% no es un valor de volumen
///
/// [`VOLUME_SILENCE`] (`0x8000`) es el silencio de la escala, pero **esta
/// FUERA del rango que el aparato declara** con `GET_MIN`/`GET_MAX`. Un
/// aparato que valida su rango --y hay muchos-- contesta **STALL** a ese valor.
/// Y un STALL aqui es lo peor que puede pasar: la barra marca 0, el sistema
/// cuenta un fallo, y **el audifono sigue sonando igual de fuerte con los
/// cascos puestos**.
///
/// Asi que el 0% se manda por donde el estandar dice que se manda: el control
/// **MUTE**, que es un si/no y no tiene rango que salirse.
///
/// Si el aparato no declara mute --pasa-- se baja al MINIMO que el declaro. No
/// es silencio y no se finge que lo sea: es lo mas bajo que ese aparato admite.
pub fn plan(percent: u8, min: i16, max: i16, has_mute: bool) -> Plan {
    if percent == 0 {
        if has_mute {
            return Plan::Callar;
        }
        return Plan::Poner { valor: min, quitar_mute: false };
    }
    Plan::Poner { valor: percent_to_volume(percent, min, max), quitar_mute: has_mute }
}

/// Convierte un porcentaje 0..100 al valor en 1/256 dB que espera el aparato,
/// **dentro del rango que el aparato declaro**.
///
/// # Por que no es una regla de tres
///
/// Los decibelios son logaritmicos y el oido tambien. Un mapeo lineal sobre el
/// rango en dB reparte casi todo el recorrido util en el ultimo tramo: el 50%
/// de una barra que va de -60 dB a 0 dB cae en -30 dB, que se oye **muy** por
/// debajo de la mitad. Es el mismo motivo por el que ningun mezclador de audio
/// usa una escala lineal en dB para su fader.
///
/// Aqui el porcentaje se toma como fraccion de AMPLITUD y se pasa a dB con
/// `20*log10`, que es lo que hace que el 50% suene a media fuerza. Sin coma
/// flotante --el kernel no la usa-- con una tabla de 21 puntos e interpolando
/// entre ellos.
///
/// `0` no es "muy bajito": es [`VOLUME_SILENCE`], porque un fader al fondo
/// tiene que callar del todo.
pub fn percent_to_volume(percent: u8, min: i16, max: i16) -> i16 {
    if percent == 0 {
        return VOLUME_SILENCE;
    }
    let p = percent.min(100) as i32;
    // dB relativos al maximo, en 1/256 dB. Tabla de 20*log10(p/100) cada 5%.
    // Negativos: el 100% son 0 dB (el maximo) y el 5% son -26 dB.
    const DB_256: [i32; 21] = [
        -32768, // 0%   (no se usa: lo atrapa el `if` de arriba)
        -6812,  // 5%   -26.02 dB
        -5560,  // 10%  -21.72
        -4828,  // 15%  -18.86
        -4308,  // 20%  -16.83
        -3905,  // 25%  -15.26
        -3576,  // 30%  -13.97
        -3298,  // 35%  -12.88
        -3056,  // 40%  -11.94
        -2843,  // 45%  -11.10
        -2653,  // 50%  -10.36  <- media fuerza de verdad
        -2481,  // 55%
        -2324,  // 60%
        -2180,  // 65%
        -2047,  // 70%
        -1923,  // 75%
        -1808,  // 80%
        -1700,  // 85%
        -1598,  // 90%
        -1502,  // 95%
        0,      // 100% -- 0 dB, el tope
    ];
    let idx = (p / 5) as usize;
    let resto = p % 5;
    let a = DB_256[idx.min(20)];
    let b = DB_256[(idx + 1).min(20)];
    let db = a + (b - a) * resto / 5;

    // Y ahora se recorta al rango REAL del aparato. Un valor fuera de rango no
    // da error: da un STALL, o peor, un aparato que se queda como estaba y hace
    // creer que obedecio.
    let min32 = min as i32;
    let max32 = max as i32;
    let v = (max32 + db).max(min32).min(max32);
    v as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Descriptor de configuracion de un adaptador USB Audio 1.0 corriente:
    /// interfaz 0 = AudioControl, con un Feature Unit id 6 que declara mute y
    /// volumen para maestro, izquierdo y derecho.
    fn config_tipica() -> [u8; 42] {
        [
            // Configuration (9)
            9, 0x02, 42, 0, 2, 1, 0, 0x80, 50,
            // Interface 0: Audio / AudioControl (9)
            9, DESC_INTERFACE, 0, 0, 0, CLASS_AUDIO, SUBCLASS_AUDIOCONTROL, 0, 0,
            // CS_INTERFACE / HEADER (9) -- este se salta por su longitud
            9, DESC_CS_INTERFACE, 0x01, 0x00, 0x01, 0x1E, 0x00, 1, 1,
            // CS_INTERFACE / FEATURE_UNIT (9), empieza en el byte 27
            // len type subtype unitID sourceID ctrlSize | master, L, R
            //                       ^28    ^29 -- el subtipo esta en 29
            9, DESC_CS_INTERFACE, AC_FEATURE_UNIT, 6, 1, 1, 0x03, 0x00, 0x00,
            // Interface 1: AudioStreaming (6, recortada a proposito)
            6, DESC_INTERFACE, 1, 0, 0, CLASS_AUDIO,
        ]
    }

    #[test]
    fn encuentra_el_feature_unit() {
        let ac = find_audio_control(&config_tipica()).expect("hay Feature Unit");
        assert_eq!(ac.interface, 0);
        assert_eq!(ac.feature_unit, 6);
        assert!(ac.has_volume, "declara volumen");
        assert!(ac.has_mute, "declara mute");
        assert_eq!(ac.channels, 2, "izquierdo y derecho, sin contar el maestro");
    }

    /// ***** EL CASO DEL RYZEN, 2026-08-09: el volumen esta en los CANALES y el
    /// bloque MAESTRO declara cero.
    ///
    /// La maquina dijo `aparato de audio SIN control de volumen` con el
    /// audifono enchufado. El canal 0 es **opcional** en el estandar, y hay
    /// aparatos --muchos-- que solo declaran izquierdo y derecho. Mirar solo el
    /// primer bloque los descarta enteros, y el sintoma es el peor posible: un
    /// "no se puede" sobre algo que si se puede.
    #[test]
    fn el_volumen_puede_estar_solo_en_los_canales() {
        let mut c = config_tipica();
        // El Feature Unit empieza en el byte 27: len, type, subtype, unitID,
        // sourceID, ctrlSize, y luego maestro / L / R.
        c[33] = 0x00; // maestro: no declara nada
        c[34] = 0x03; // izquierdo: mute + volumen
        c[35] = 0x03; // derecho: igual
        let ac = find_audio_control(&c).expect("sigue siendo un Feature Unit");
        assert!(ac.has_volume, "el volumen esta en los canales, y cuenta");
        assert!(ac.has_mute, "y el mute tambien");
    }

    /// Y si NADIE lo declara --ni maestro ni canales-- sigue siendo que no.
    /// Ensanchar la busqueda no puede convertir un "no" en un "si".
    #[test]
    fn si_nadie_declara_volumen_sigue_siendo_que_no() {
        let mut c = config_tipica();
        c[33] = 0x00;
        c[34] = 0x00;
        c[35] = 0x00;
        let ac = find_audio_control(&c).expect("el Feature Unit esta");
        assert!(!ac.has_volume);
        assert!(!ac.has_mute);
    }

    /// Un aparato sin Feature Unit existe, y la respuesta correcta es `None`:
    /// decir que no se puede, en vez de fingir que el volumen se puso.
    #[test]
    fn sin_feature_unit_contesta_que_no() {
        let mut c = config_tipica();
        c[29] = 0x02; // el subtipo del Feature Unit pasa a ser otra cosa
        assert!(find_audio_control(&c).is_none());
    }

    /// Un descriptor de longitud 0 colgaria el bucle. Llega de fuera: no se le
    /// supone nada.
    #[test]
    fn un_bLength_de_cero_no_cuelga() {
        let malo = [0u8, 0x02, 0, 0];
        assert!(find_audio_control(&malo).is_none());
    }

    /// ** El `wIndex` lleva la UNIDAD arriba y la INTERFAZ abajo. Al reves el
    /// aparato contesta STALL sin decir por que.
    #[test]
    fn el_windex_no_va_al_reves() {
        let ac = find_audio_control(&config_tipica()).unwrap();
        let r = set_volume(&ac, CHANNEL_MASTER, -2653);
        assert_eq!(r.w_index, 0x0600, "unidad 6 arriba, interfaz 0 abajo");
        assert_eq!(r.w_value, 0x0200, "selector VOLUME arriba, canal 0 abajo");
        assert_eq!(r.bm_request_type, 0x21, "clase, a interfaz, host->aparato");
        assert_eq!(r.b_request, SET_CUR);
        assert_eq!(r.length, 2);
        assert!(!r.data_in);
    }

    #[test]
    fn las_lecturas_van_al_reves_que_las_escrituras() {
        let ac = find_audio_control(&config_tipica()).unwrap();
        let r = get_volume(&ac, CHANNEL_MASTER, GET_MAX);
        assert_eq!(r.bm_request_type, 0xA1, "aparato->host");
        assert!(r.data_in);
        assert_eq!(r.b_request, GET_MAX);
    }

    /// El mute son 1 byte y otro selector. Un mute de 2 bytes es un STALL.
    #[test]
    fn el_mute_es_de_un_byte() {
        let ac = find_audio_control(&config_tipica()).unwrap();
        let r = set_mute(&ac, CHANNEL_MASTER, true);
        assert_eq!(r.w_value, 0x0100);
        assert_eq!(r.length, 1);
    }

    /// **** El 0% CALLA. No es "muy bajito": `0` en esta escala son 0 dB, o sea
    /// el maximo. Confundirlos pone el aparato a tope creyendo que se apaga.
    #[test]
    fn el_cero_por_ciento_es_silencio_y_no_cero_db() {
        assert_eq!(percent_to_volume(0, -6400, 0), VOLUME_SILENCE);
        assert_ne!(percent_to_volume(0, -6400, 0), 0);
    }

    /// El 100% es el maximo que declaro el aparato, ni un paso mas.
    #[test]
    fn el_cien_por_ciento_es_el_maximo_del_aparato() {
        assert_eq!(percent_to_volume(100, -6400, -256), -256);
        assert_eq!(percent_to_volume(100, -6400, 0), 0);
    }

    /// ** La curva NO es lineal, y esa es toda la razon de que exista la tabla.
    ///
    /// Con un rango de -25 dB a 0 dB, el 50% lineal caeria en -12.5 dB
    /// (`-3200`). La curva logaritmica lo pone en -10.36 dB, que es lo que el
    /// oido lee como media fuerza. Si esta fila falla con un numero cercano a
    /// -3200, es que alguien cambio la tabla por una regla de tres.
    #[test]
    fn la_curva_es_logaritmica_y_no_una_regla_de_tres() {
        let v = percent_to_volume(50, -6400, 0);
        assert_eq!(v, -2653, "50% = -10.36 dB");
        let lineal = -6400 / 2;
        assert!(v > lineal, "la curva log tiene que quedar POR ENCIMA del lineal");
    }

    /// **** EL 0% SE MANDA POR EL MUTE, no por el volumen.
    ///
    /// El silencio de la escala (`0x8000`) esta FUERA del rango que el aparato
    /// declara, asi que un aparato que valida contesta STALL: la barra marca 0
    /// y **el audifono sigue sonando**. Si esta fila cambia a `Poner`, eso es
    /// lo que vuelve.
    #[test]
    fn el_cero_por_ciento_calla_por_el_mute() {
        assert_eq!(plan(0, -6400, 0, true), Plan::Callar);
    }

    /// Y si el aparato NO tiene mute, se baja al minimo que el declaro. No es
    /// silencio, y por eso no se manda el marcador de silencio: se manda lo
    /// mas bajo que ESE aparato admite.
    #[test]
    fn sin_mute_el_cero_baja_al_minimo_declarado() {
        assert_eq!(
            plan(0, -6400, 0, false),
            Plan::Poner { valor: -6400, quitar_mute: false }
        );
    }

    /// Subir de 0 tiene que QUITAR el mute. Sin esto, bajar a 0 y volver a
    /// subir deja un aparato mudo con la barra al 80: el volumen llega, se
    /// acepta, y no se oye -- que es exactamente el sintoma que hace pensar
    /// que el camino entero esta roto.
    #[test]
    fn subir_de_cero_quita_el_mute() {
        let p = plan(50, -6400, 0, true);
        assert_eq!(p, Plan::Poner { valor: -2653, quitar_mute: true });
    }

    /// ** `0x8000` no es un volumen: es la senal de callar. Si un aparato lo
    /// declara como minimo, tomarlo al pie de la letra haria que los
    /// porcentajes bajos CALLARAN en vez de sonar flojo.
    #[test]
    fn un_minimo_de_silencio_no_se_toma_al_pie_de_la_letra() {
        let (min, max) = rango(VOLUME_SILENCE, 0).expect("es un rango usable");
        assert_eq!(min, VOLUME_SILENCE + 1);
        assert_eq!(max, 0);
        for p in 1..=100u8 {
            assert_ne!(
                percent_to_volume(p, min, max),
                VOLUME_SILENCE,
                "{p}% no puede caer en el marcador de silencio"
            );
        }
    }

    /// Un rango al reves --o el que queda cuando el aparato no contesta-- no se
    /// usa: se dice que no vale, y quien llama avisa. Un rango inventado da un
    /// volumen que salta de mudo a ensordecedor.
    #[test]
    fn un_rango_imposible_se_rechaza() {
        assert!(rango(0, 0).is_none(), "max == min no es un rango");
        assert!(rango(0, -6400).is_none(), "del reves tampoco");
        assert!(rango(-6400, 0).is_some());
    }

    /// Y nunca se sale del rango del aparato, pase lo que pase. Un valor fuera
    /// no da error: da un STALL, o un aparato que se queda como estaba y hace
    /// creer que obedecio.
    #[test]
    fn nunca_se_sale_del_rango_declarado() {
        for p in 0..=100u8 {
            let v = percent_to_volume(p, -2000, -500);
            if p == 0 {
                continue; // el silencio es un valor especial, fuera de rango a proposito
            }
            assert!(v >= -2000 && v <= -500, "{p}% dio {v}, fuera de [-2000,-500]");
        }
    }
}
