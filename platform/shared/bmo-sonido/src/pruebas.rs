//! **Un sobre mal leido no da un error: da ruido blanco a todo volumen.**
//!
//! Esa es la razon de que este fichero sea largo para lo que parece un lector de
//! cabeceras. Los fallos de un parser de audio **no se ven, se oyen** -- y se
//! oyen tarde, en un oido, y sin nada en pantalla que diga por que.
//!
//! ## Lo que se prueba con mas insistencia, y por que
//!
//! **Que los trozos se recorran de verdad.** El fallo mas comun de un lector de
//! WAV es saltar 44 bytes a ciegas: funciona con el fichero que uno tiene
//! delante y falla con el primero que traiga un `LIST` con el titulo dentro. Y
//! cuando falla, entrega los metadatos como si fueran muestras.

use super::*;

extern crate std;
use std::vec::Vec;

/// Arma un WAV con los trozos que se le den, en ese orden.
///
/// * A mano y no con una libreria: si lo armara la misma que lo lee, un fallo de
/// formato produciria un fichero que el lector entiende **y que nadie mas**.
fn wav(trozos: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut cuerpo = Vec::new();
    for (nombre, datos) in trozos {
        cuerpo.extend_from_slice(*nombre);
        cuerpo.extend_from_slice(&(datos.len() as u32).to_le_bytes());
        cuerpo.extend_from_slice(datos);
        // El relleno de los impares, que es parte del formato.
        if datos.len() % 2 == 1 {
            cuerpo.push(0);
        }
    }
    let mut v = Vec::new();
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&((cuerpo.len() + 4) as u32).to_le_bytes());
    v.extend_from_slice(b"WAVE");
    v.extend_from_slice(&cuerpo);
    v
}

/// Un `fmt ` de 16 bytes.
fn fmt(codigo: u16, canales: u16, frecuencia: u32, bits: u16) -> Vec<u8> {
    let cuadro = canales as u32 * (bits as u32 / 8);
    let mut v = Vec::new();
    v.extend_from_slice(&codigo.to_le_bytes());
    v.extend_from_slice(&canales.to_le_bytes());
    v.extend_from_slice(&frecuencia.to_le_bytes());
    v.extend_from_slice(&(frecuencia * cuadro).to_le_bytes()); // bytes/s
    v.extend_from_slice(&(cuadro as u16).to_le_bytes());
    v.extend_from_slice(&bits.to_le_bytes());
    v
}

fn bueno() -> Vec<u8> {
    wav(&[
        (b"fmt ", fmt(1, 2, 48000, 16)),
        (b"data", std::vec![0x41u8; 960]),
    ])
}

/// El caso bueno, y **los cuatro numeros del audifono de esta casa**.
#[test]
fn un_wav_de_48k_16_bits_estereo_se_abre() {
    let b = bueno();
    let p = leer_wav(&b).expect("tiene que abrir");
    assert_eq!((p.canales, p.frecuencia, p.bits), (2, 48000, 16));
    assert_eq!(p.datos.len(), 960);
    // *** 192 bytes por milisegundo, que es EXACTAMENTE el `wMaxPacketSize` que
    // AUDIO_MAESTRO predijo para este aparato antes de mirarlo.
    assert_eq!(p.bytes_por_ms(), 192);
    assert_eq!(p.bytes_por_cuadro(), 4);
    assert_eq!(p.dura_ms(), 5);
}

/// *** **LOS TROZOS SE RECORREN: un `LIST` en medio no desplaza las muestras.**
///
/// Es el fallo que este fichero existe para impedir. Un lector que salte 44
/// bytes a ciegas entregaria aqui el titulo de la cancion como si fueran
/// muestras -- y eso no da un error, **da ruido blanco a todo volumen**.
#[test]
fn un_trozo_en_medio_no_corre_las_muestras() {
    let b = wav(&[
        (b"fmt ", fmt(1, 2, 48000, 16)),
        (b"LIST", Vec::from(&b"INFOINAMuna cancion cualquiera"[..])),
        (b"data", std::vec![0x7Fu8; 400]),
    ]);
    let p = leer_wav(&b).expect("tiene que abrir");
    assert_eq!(p.datos.len(), 400);
    assert!(p.datos.iter().all(|&x| x == 0x7F), "las muestras, no el titulo");
}

/// **Un trozo de largo IMPAR lleva un byte de relleno**, y sin contarlo el
/// cursor se desalinea y el `data` no aparece nunca aunque este ahi.
#[test]
fn un_trozo_impar_no_desalinea_el_resto() {
    let b = wav(&[
        (b"fmt ", fmt(1, 1, 44100, 8)),
        (b"fact", std::vec![0xAAu8; 7]), // impar
        (b"data", std::vec![0x33u8; 100]),
    ]);
    let p = leer_wav(&b).expect("el data tiene que aparecer detras del impar");
    assert_eq!(p.datos.len(), 100);
    assert_eq!(p.canales, 1);
}

/// **`data` antes que `fmt `** tambien vale: RIFF no promete un orden.
#[test]
fn el_orden_de_los_trozos_no_esta_prometido() {
    let b = wav(&[
        (b"data", std::vec![1u8; 8]),
        (b"fmt ", fmt(1, 2, 48000, 16)),
    ]);
    assert!(leer_wav(&b).is_ok());
}

/// **Un WAV que lleva MP3 dentro se rechaza CON SU NUMERO.**
///
/// El sobre no ayuda: hay que decodificar igual. Y decirlo asi --con el codigo
/// de formato-- manda a mirar el fichero, no el driver.
#[test]
fn un_wav_que_no_es_pcm_dice_que_no_lo_es() {
    let b = wav(&[
        (b"fmt ", fmt(0x0055, 2, 44100, 16)), // 0x55 = MPEG Layer 3
        (b"data", std::vec![0u8; 10]),
    ]);
    assert_eq!(leer_wav(&b), Err(Falta::NoEsPcm(0x0055)));
}

/// El WAV "extensible" (`0xFFFE`) **si** se acepta: su GUID empieza por el mismo
/// numero de PCM, y rechazarlo dejaria fuera lo que graba media herramienta.
#[test]
fn el_wav_extensible_se_acepta() {
    let b = wav(&[
        (b"fmt ", fmt(0xFFFE, 2, 48000, 16)),
        (b"data", std::vec![0u8; 16]),
    ]);
    assert!(leer_wav(&b).is_ok());
}

/// **Un trozo que dice medir mas de lo que hay.** El largo lo escribe el
/// fichero, o sea otro.
#[test]
fn un_trozo_que_miente_sobre_su_largo_no_lee_fuera() {
    let mut b = bueno();
    // El `data` esta detras del `fmt ` (8 + 16 + relleno 0) -> off 12+8+16 = 36.
    b[36 + 4..36 + 8].copy_from_slice(&0xFFFF_FF00u32.to_le_bytes());
    assert_eq!(leer_wav(&b), Err(Falta::SobreRoto));
}

/// Le falta un trozo, y **se dice CUAL**.
#[test]
fn si_falta_un_trozo_se_dice_cual() {
    let sin_datos = wav(&[(b"fmt ", fmt(1, 2, 48000, 16))]);
    assert!(matches!(leer_wav(&sin_datos), Err(Falta::SinTrozo(_))));
    let sin_fmt = wav(&[(b"data", std::vec![0u8; 4])]);
    assert!(matches!(leer_wav(&sin_fmt), Err(Falta::SinTrozo(_))));
}

/// Cero canales o cero frecuencia: **nada que tocar, y `bytes_por_ms` dividiria
/// entre cero mas abajo.**
#[test]
fn un_formato_vacio_se_para_antes_de_dividir() {
    for f in [fmt(1, 0, 48000, 16), fmt(1, 2, 0, 16), fmt(1, 2, 48000, 0)] {
        let b = wav(&[(b"fmt ", f), (b"data", std::vec![0u8; 8])]);
        assert_eq!(leer_wav(&b), Err(Falta::Vacio));
    }
}

/// **`cabe_en` dice que NO y dice por que.** Y no hay `Casi`: esa variante seria
/// la puerta por la que entra el resampler, que el maestro rechaza por escrito.
#[test]
fn lo_que_no_cabe_dice_por_que_no_cabe() {
    let b = wav(&[
        (b"fmt ", fmt(1, 2, 44100, 16)),
        (b"data", std::vec![0u8; 16]),
    ]);
    let p = leer_wav(&b).unwrap();
    assert_eq!(
        p.cabe_en(48000, 2, 16),
        Err(NoCabe::OtraFrecuencia { pide: 44100, acepta: 48000 })
    );
    assert!(p.cabe_en(44100, 2, 16).is_ok());
    assert_eq!(
        p.cabe_en(44100, 1, 16),
        Err(NoCabe::OtrosCanales { pide: 2, acepta: 1 })
    );
}

/// **Reconocer no es lo mismo que saber tocar.**
///
/// Un `.mp3` da un no que dice *"esto es un MP3"*, no *"esto no es audio"*. Las
/// dos respuestas mandan a sitios distintos.
#[test]
fn se_reconoce_mas_de_lo_que_se_sabe_tocar() {
    assert_eq!(reconocer(&bueno()), Formato::Wav);
    assert!(Formato::Wav.es_crudo());

    assert_eq!(reconocer(b"ID3\x04\x00\x00\x00\x00\x00\x00"), Formato::Mp3);
    assert_eq!(reconocer(&[0xFF, 0xFB, 0x90, 0x00]), Formato::Mp3);
    assert!(!Formato::Mp3.es_crudo(), "reconocido, y no se puede tocar");

    assert_eq!(reconocer(b"fLaC\x00\x00\x00\x22"), Formato::Flac);
    assert_eq!(reconocer(b"OggS\x00\x02\x00\x00"), Formato::Ogg);
    assert_eq!(reconocer(b"cualquier cosa"), Formato::Desconocido);
    assert_eq!(reconocer(b""), Formato::Desconocido);
}

/// **Nada de lo que llegue puede hacer estallar el lector.** En Ring 3 un panico
/// no es un test rojo: es el reproductor caido con el tubo abierto.
#[test]
fn ningun_byte_corrompido_tumba_el_lector() {
    let base = bueno();
    for i in 0..base.len().min(120) {
        for v in [0x00u8, 0x01, 0x7F, 0xFF] {
            let mut b = base.clone();
            b[i] = v;
            let _ = leer_wav(&b);
            let _ = reconocer(&b);
        }
    }
    // Y truncado por todos los sitios.
    for n in 0..base.len().min(120) {
        let _ = leer_wav(&base[..n]);
        let _ = reconocer(&base[..n]);
    }
}
