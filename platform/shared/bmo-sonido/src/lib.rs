//! **QUE HAY DENTRO DE UN FICHERO DE AUDIO**, y si este aparato puede tocarlo.
//!
//! generacion: abuelo -- no depende de nadie
//!
//! # El paso 3 de `AUDIO_MAESTRO`, y su frase
//!
//! > *"Un `.wav` es **PCM en un sobre**: 44 bytes de cabecera y detras las
//! > muestras crudas, que es exactamente lo que come el endpoint. **Cero
//! > decodificador.**"*
//!
//! Por eso WAV es **la base y no una etapa mas**. Si el aparato pide 48 kHz / 16
//! bits / 2 canales y el fichero viene asi, el trabajo entero es **leerlo y
//! darlo**. Todo lo que venga despues --MP3, FLAC, lo que sea-- termina
//! entregando exactamente esto mismo al mismo tubo.
//!
//! ```text
//!    un .wav      -> PCM          -> el endpoint isocrono
//!    un .mp3      -> DECODER -> PCM -> el mismo endpoint
//! ```
//!
//! *** **Y por eso el decodificador va el ultimo.** Empezar por MP3 dejaria un
//! decodificador en verde y sin ejecutar mientras no hay donde soltar las
//! muestras. `AUDIO_MAESTRO` lo llama por su nombre: *"la cicatriz de los nueve
//! tests de coma flotante del frontend de C"*.
//!
//! # [!] ESTE CRATE NO CONVIERTE NADA, Y NO ES UNA CARENCIA
//!
//! La parte 8 del maestro lo declara:
//!
//! > *"Si el aparato pide 48 kHz y el fichero viene a 44,1, aqui **no se
//! > convierte**: se dice y se convierte fuera. Un resampler malo suena peor que
//! > no sonar, y uno bueno es un proyecto."*
//!
//! Asi que [`Pcm::cabe_en`] contesta **si** o **no** con el motivo, y nunca
//! *"casi"*. Un `Casi` habria sido la puerta por la que entra el resampler.
//!
//! # Lo que sabe leer hoy
//!
//! ```text
//!    WAV / PCM     [X]  el sobre. Cero decodificador
//!    MP3           --   se RECONOCE y se dice que no. Ver `Formato`
//!    FLAC, OGG     --   idem
//! ```
//!
//! * Reconocer un formato que no se sabe tocar **no es lo mismo que no
//! soportarlo**: un `.mp3` da un no que dice *"esto es un MP3 y aqui todavia no
//! hay decodificador"*, y no *"esto no es audio"*. Las dos respuestas mandan a
//! sitios distintos.

#![no_std]
#![forbid(unsafe_code)]

/// **Que clase de fichero es esto.**
///
/// Se reconocen mas formatos de los que se saben tocar **a proposito**: la
/// diferencia entre *"no es audio"* y *"es un MP3 y no hay decodificador"* es la
/// diferencia entre buscar el fallo en el fichero o en el sistema.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Formato {
    /// RIFF/WAVE con muestras PCM. **El unico que se puede tocar hoy.**
    Wav,
    /// MPEG audio. Se reconoce por su sincronismo o por una etiqueta ID3.
    Mp3,
    /// `fLaC`.
    Flac,
    /// `OggS`.
    Ogg,
    /// No se parece a ninguno de los de arriba.
    Desconocido,
}

impl Formato {
    /// Se puede entregar al aparato **sin decodificar**?
    pub fn es_crudo(self) -> bool {
        matches!(self, Formato::Wav)
    }

    pub fn nombre(self) -> &'static str {
        match self {
            Formato::Wav => "WAV (PCM en un sobre)",
            Formato::Mp3 => "MP3",
            Formato::Flac => "FLAC",
            Formato::Ogg => "OGG",
            Formato::Desconocido => "no reconocido",
        }
    }
}

/// **Que formato traen estos bytes**, mirando solo el principio.
///
/// [!] Con MP3 se mira **ID3 o el sincronismo**, y el sincronismo (`0xFF Ex`)
/// puede aparecer por casualidad en cualquier fichero. Por eso esto **no es un
/// juicio**: es una pista para poder decir un no util. Lo que decide si algo se
/// toca es [`leer_wav`], que comprueba de verdad.
pub fn reconocer(bytes: &[u8]) -> Formato {
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        return Formato::Wav;
    }
    if bytes.len() >= 4 && &bytes[0..4] == b"fLaC" {
        return Formato::Flac;
    }
    if bytes.len() >= 4 && &bytes[0..4] == b"OggS" {
        return Formato::Ogg;
    }
    if bytes.len() >= 3 && &bytes[0..3] == b"ID3" {
        return Formato::Mp3;
    }
    // Sincronismo de trama MPEG: once unos seguidos.
    if bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0 {
        return Formato::Mp3;
    }
    Formato::Desconocido
}

/// **Por que no se puede tocar esto.**
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Falta {
    /// No llega ni a la cabecera RIFF.
    NoLlegaNiAlSobre,
    /// No es un RIFF/WAVE.
    NoEsWav,
    /// Le falta el trozo `fmt ` o el `data`.
    SinTrozo(&'static str),
    /// **No es PCM.** Un WAV puede llevar dentro casi cualquier cosa --incluido
    /// MP3-- y entonces el sobre no ayuda: hay que decodificar igual.
    NoEsPcm(u16),
    /// El sobre se contradice: un trozo dice medir mas de lo que hay.
    SobreRoto,
    /// Cero canales, cero frecuencia o cero bits. Nada que tocar.
    Vacio,
}

impl Falta {
    pub fn nombre(self) -> &'static str {
        match self {
            Falta::NoLlegaNiAlSobre => "no llega ni a la cabecera RIFF",
            Falta::NoEsWav => "no es un RIFF/WAVE",
            Falta::SinTrozo(t) => t,
            Falta::NoEsPcm(_) => "el WAV no lleva PCM dentro: pide decodificador",
            Falta::SobreRoto => "un trozo dice medir mas de lo que hay",
            Falta::Vacio => "cero canales, cero frecuencia o cero bits",
        }
    }
}

/// **Muestras crudas, listas para el endpoint.**
///
/// `datos` es una rebanada del fichero: **no se copia nada**. Es la parte 4 del
/// maestro empezando aqui -- las muestras se prestan, no se copian.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pcm<'a> {
    pub canales: u16,
    pub frecuencia: u32,
    pub bits: u16,
    /// Las muestras. Del primer byte al ultimo, sin cabecera.
    pub datos: &'a [u8],
}

/// Por que este PCM no cabe en este aparato.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoCabe {
    /// El aparato no acepta esa frecuencia. **No se resamplea**: ver la cabecera.
    OtraFrecuencia { pide: u32, acepta: u32 },
    OtrosCanales { pide: u16, acepta: u16 },
    OtrosBits { pide: u16, acepta: u16 },
}

impl<'a> Pcm<'a> {
    /// Bytes de una muestra de TODOS los canales.
    pub fn bytes_por_cuadro(&self) -> u32 {
        (self.canales as u32) * ((self.bits as u32 + 7) / 8)
    }

    /// **Cuantos bytes hay que entregar por milisegundo.**
    ///
    /// Es el numero que tiene que cuadrar con el `wMaxPacketSize` del endpoint.
    /// A 48 kHz, 16 bits y 2 canales son **192**, que es exactamente lo que
    /// `AUDIO_MAESTRO` predijo para este audifono antes de mirarlo.
    pub fn bytes_por_ms(&self) -> u32 {
        (self.frecuencia / 1000) * self.bytes_por_cuadro()
    }

    /// Cuanto dura, en milisegundos.
    pub fn dura_ms(&self) -> u32 {
        let por_ms = self.bytes_por_ms();
        if por_ms == 0 {
            return 0;
        }
        (self.datos.len() as u32) / por_ms
    }

    /// **Puede este aparato tocar esto tal cual?**
    ///
    /// Devuelve el primer motivo por el que no. **No hay `Casi`**: esa variante
    /// habria sido la puerta por la que entra el resampler, y el maestro se
    /// niega a traerlo por escrito.
    pub fn cabe_en(&self, frecuencia: u32, canales: u16, bits: u16) -> Result<(), NoCabe> {
        if self.frecuencia != frecuencia {
            return Err(NoCabe::OtraFrecuencia { pide: self.frecuencia, acepta: frecuencia });
        }
        if self.canales != canales {
            return Err(NoCabe::OtrosCanales { pide: self.canales, acepta: canales });
        }
        if self.bits != bits {
            return Err(NoCabe::OtrosBits { pide: self.bits, acepta: bits });
        }
        Ok(())
    }
}

/// Formato de compresion dentro de un WAV. `1` es PCM sin comprimir.
const WAVE_FORMAT_PCM: u16 = 1;
/// El WAV "extensible": la cabecera es mas larga y el formato real vive en un
/// GUID al final. Los primeros dos bytes de ese GUID **son el mismo numero**, y
/// por eso se acepta cuando dice PCM.
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

fn u16_en(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]))
}
fn u32_en(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *b.get(i)?,
        *b.get(i + 1)?,
        *b.get(i + 2)?,
        *b.get(i + 3)?,
    ]))
}

/// **Abrir un WAV.** Devuelve las muestras SIN COPIARLAS.
///
/// # El sobre no es de tamano fijo, y ahi esta el fallo facil
///
/// Se lee mucho que *"un WAV son 44 bytes de cabecera"*. Es cierto en el caso
/// mas comun y **falso en general**: RIFF es una lista de trozos con nombre, y
/// entre `fmt ` y `data` puede haber un `LIST` con el titulo, un `fact`, o
/// relleno. Un lector que salte 44 bytes a ciegas **entrega metadatos como si
/// fueran muestras** -- y eso no da un error: da ruido blanco a todo volumen.
///
/// [!] Por eso esto **recorre los trozos**. Es diez lineas mas y es la diferencia
/// entre que suene musica y que suene un fallo directamente a un oido.
///
/// # Lo que se comprueba, y por que cada cosa
///
/// ```text
///    1. RIFF....WAVE          o no es esto
///    2. cada trozo CABE       el largo lo escribe el fichero, o sea otro
///    3. `fmt ` dice PCM       un WAV puede llevar MP3 dentro
///    4. ni canales ni bits    un cero aqui divide por cero mas abajo
///       ni frecuencia a cero
/// ```
pub fn leer_wav(bytes: &[u8]) -> Result<Pcm<'_>, Falta> {
    if bytes.len() < 12 {
        return Err(Falta::NoLlegaNiAlSobre);
    }
    if &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(Falta::NoEsWav);
    }

    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    let mut datos: Option<&[u8]> = None;

    // ** EL RECORRIDO. Cada trozo son 8 bytes de cabecera --nombre y largo-- y
    // su contenido, y los impares llevan un byte de relleno detras.
    let mut off = 12usize;
    while off + 8 <= bytes.len() {
        let nombre = &bytes[off..off + 4];
        let largo = u32_en(bytes, off + 4).ok_or(Falta::SobreRoto)? as usize;
        let ini = off + 8;
        // ** En `u64`: `largo` lo escribe el fichero, o sea otro. En 32 bits
        // `ini + largo` da la vuelta y apunta DENTRO del fichero, que es el
        // mismo fallo que `LasCuentasNoCaben` en la cara que viaja.
        if ini as u64 + largo as u64 > bytes.len() as u64 {
            return Err(Falta::SobreRoto);
        }
        let cuerpo = &bytes[ini..ini + largo];

        if nombre == b"fmt " {
            if largo < 16 {
                return Err(Falta::SobreRoto);
            }
            let codigo = u16_en(cuerpo, 0).ok_or(Falta::SobreRoto)?;
            let canales = u16_en(cuerpo, 2).ok_or(Falta::SobreRoto)?;
            let frecuencia = u32_en(cuerpo, 4).ok_or(Falta::SobreRoto)?;
            let bits = u16_en(cuerpo, 14).ok_or(Falta::SobreRoto)?;
            if codigo != WAVE_FORMAT_PCM && codigo != WAVE_FORMAT_EXTENSIBLE {
                return Err(Falta::NoEsPcm(codigo));
            }
            fmt = Some((codigo, canales, frecuencia, bits));
        } else if nombre == b"data" {
            datos = Some(cuerpo);
        }

        // ** El relleno de los impares. Sin esto, un trozo de largo impar deja
        // el cursor desalineado y el SIGUIENTE nombre se lee corrido -- y
        // entonces el `data` no aparece nunca aunque este ahi.
        off = ini + largo + (largo & 1);
    }

    let (_, canales, frecuencia, bits) = fmt.ok_or(Falta::SinTrozo("le falta el trozo `fmt `"))?;
    let datos = datos.ok_or(Falta::SinTrozo("le falta el trozo `data`"))?;
    if canales == 0 || frecuencia == 0 || bits == 0 {
        return Err(Falta::Vacio);
    }
    Ok(Pcm { canales, frecuencia, bits, datos })
}

#[cfg(test)]
mod pruebas;
