//! **bmo-imagen** -- de los bytes de un fichero a pixeles, SIN creerse el fichero.
//!
//! generacion: nieto -- no sabe quien la llama ni donde se pinta; recibe bytes
//! y un bufer, y devuelve medidas o un motivo.
//!
//! ## Por que existe (2026-09-13)
//!
//! Eddi: *"dale con el visor de imagenes"*. El DIRECTOR ya descifraba BICO, pero
//! solo 16x16 y solo iconos. Un visor tiene que abrir lo que haya en el disco
//! --BICO, BMP, QOI-- y ahi entra la clase de fallo mas famosa de las librerias
//! de imagenes: un fichero MALICIOSO declara unas medidas, el decodificador
//! multiplica sin mirar, y escribe fuera del bufer.
//!
//! ** Por eso esto es una crate y no un fichero del DIRECTOR: aqui se prueba en
//! el anfitrion contra ficheros ROTOS. Toda medida que viene del fichero se
//! compara con lo que llego antes de usarla, y un bufer pequeno es `NoCabe`, no
//! una escritura de mas.
//!
//! ## El pixel que sale
//!
//! `0xAARRGGBB`, y **el alfa es un BIT** como en `<bmo/imagen.h>`: `0xFF` o `0`.
//! QOI trae alfa de 8 bits y se corta en 128. No hay mezcla porque no hay quien
//! la pague: mezclar pide leer el fondo, y el framebuffer no se lee barato.
//!
//! [!] Tope de 1024 de lado. Es de quien llama --el bufer son 4 MiB-- y se dice
//! con `Medidas`, no recortando.

#![cfg_attr(not(test), no_std)]

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Formato {
    Bico,
    Bmp,
    Qoi,
}

impl Formato {
    pub fn nombre(self) -> &'static str {
        match self {
            Formato::Bico => "BICO",
            Formato::Bmp => "BMP",
            Formato::Qoi => "QOI",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// Faltan bytes que la cabecera promete.
    Corto,
    /// No empieza como ningun formato conocido.
    NoEsImagen,
    /// Es el formato, pero una variante que no se soporta (BMP comprimido...).
    Variante,
    /// Ancho o alto a cero, o mas de `LADO_MAX`.
    Medidas,
    /// El bufer de quien llama es mas pequeno que la imagen.
    NoCabe,
}

impl Error {
    pub fn motivo(self) -> &'static str {
        match self {
            Error::Corto => "la imagen esta CORTADA: faltan bytes que su cabecera promete",
            Error::NoEsImagen => "no es BICO, BMP ni QOI",
            Error::Variante => "variante no soportada (BMP comprimido, 16 bits...)",
            Error::Medidas => "medidas imposibles: cero, o mas de 1024 de lado",
            Error::NoCabe => "no cabe en el bufer del visor",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Medidas {
    pub ancho: u32,
    pub alto: u32,
    pub formato: Formato,
}

pub const LADO_MAX: u32 = 1024;
const OPACO: u32 = 0xFF00_0000;

fn le16(b: &[u8], i: usize) -> Option<u32> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]) as u32)
}

fn le32(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*b.get(i)?, *b.get(i + 1)?, *b.get(i + 2)?, *b.get(i + 3)?]))
}

fn be32(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_be_bytes([*b.get(i)?, *b.get(i + 1)?, *b.get(i + 2)?, *b.get(i + 3)?]))
}

fn lados(ancho: u32, alto: u32) -> Result<(), Error> {
    if ancho == 0 || alto == 0 || ancho > LADO_MAX || alto > LADO_MAX {
        Err(Error::Medidas)
    } else {
        Ok(())
    }
}

/// **Que es y cuanto mide**, sin decodificar.
pub fn medir(b: &[u8]) -> Result<Medidas, Error> {
    if b.len() < 4 {
        return Err(Error::Corto);
    }
    if &b[..4] == b"BICO" {
        let (w, h) = (le16(b, 4).ok_or(Error::Corto)?, le16(b, 6).ok_or(Error::Corto)?);
        lados(w, h)?;
        return Ok(Medidas { ancho: w, alto: h, formato: Formato::Bico });
    }
    if &b[..4] == b"qoif" {
        let (w, h) = (be32(b, 4).ok_or(Error::Corto)?, be32(b, 8).ok_or(Error::Corto)?);
        lados(w, h)?;
        return Ok(Medidas { ancho: w, alto: h, formato: Formato::Qoi });
    }
    if b[0] == b'B' && b[1] == b'M' {
        let w = le32(b, 18).ok_or(Error::Corto)?;
        let h = le32(b, 22).ok_or(Error::Corto)?;
        // Un ancho negativo leido sin signo es >= 2^31 y cae en `Medidas`. Un
        // alto negativo es legal: la imagen va de arriba abajo.
        let alto = if h >= 0x8000_0000 { h.wrapping_neg() } else { h };
        lados(w, alto)?;
        return Ok(Medidas { ancho: w, alto, formato: Formato::Bmp });
    }
    Err(Error::NoEsImagen)
}

/// **Decodifica en `dst`** (fila a fila, `ancho * alto` pixeles).
pub fn decodificar(b: &[u8], dst: &mut [u32]) -> Result<Medidas, Error> {
    let m = medir(b)?;
    // Con los lados acotados a 1024 el producto cabe de sobra en u32.
    let n = (m.ancho * m.alto) as usize;
    if dst.len() < n {
        return Err(Error::NoCabe);
    }
    match m.formato {
        Formato::Bico => bico(b, n, &mut dst[..n])?,
        Formato::Bmp => bmp(b, &m, &mut dst[..n])?,
        Formato::Qoi => qoi(b, &mut dst[..n])?,
    }
    Ok(m)
}

fn bit_de_alfa(a: u32) -> u32 {
    if a >= 128 { OPACO } else { 0 }
}

fn bico(b: &[u8], n: usize, dst: &mut [u32]) -> Result<(), Error> {
    if b.len() < 8 + n * 4 {
        return Err(Error::Corto);
    }
    for (k, px) in dst.iter_mut().enumerate() {
        let v = le32(b, 8 + k * 4).ok_or(Error::Corto)?;
        *px = bit_de_alfa(v >> 24) | (v & 0x00FF_FFFF);
    }
    Ok(())
}

fn bmp(b: &[u8], m: &Medidas, dst: &mut [u32]) -> Result<(), Error> {
    let datos = le32(b, 10).ok_or(Error::Corto)? as usize;
    if le32(b, 14).ok_or(Error::Corto)? < 40 {
        return Err(Error::Variante);
    }
    if le16(b, 26).ok_or(Error::Corto)? != 1 {
        return Err(Error::Variante);
    }
    let bits = le16(b, 28).ok_or(Error::Corto)?;
    if bits != 24 && bits != 32 {
        return Err(Error::Variante);
    }
    if le32(b, 30).ok_or(Error::Corto)? != 0 {
        return Err(Error::Variante);
    }
    let de_arriba = le32(b, 22).ok_or(Error::Corto)? >= 0x8000_0000;
    let (w, h) = (m.ancho as usize, m.alto as usize);
    let por_pixel = (bits / 8) as usize;
    let fila = ((bits as usize * w + 31) / 32) * 4;
    // *** LA COMPROBACION QUE LAS LIBRERIAS FAMOSAS NO HICIERON: lo que la
    // cabecera promete cabe en lo que llego. Con los lados acotados, `fila * h`
    // no pasa de 4 MiB y no puede dar la vuelta.
    if datos > b.len() || fila * h > b.len() - datos {
        return Err(Error::Corto);
    }
    for y in 0..h {
        let fuente = if de_arriba { y } else { h - 1 - y };
        for x in 0..w {
            let p = datos + fuente * fila + x * por_pixel;
            let (azul, verde, rojo) = (b[p] as u32, b[p + 1] as u32, b[p + 2] as u32);
            // El cuarto byte de un BMP de 32 sin comprimir es "reservado", no
            // alfa: muchos programas lo dejan a cero. Leerlo daria una imagen
            // invisible. Sale opaco.
            dst[y * w + x] = OPACO | (rojo << 16) | (verde << 8) | azul;
        }
    }
    Ok(())
}

fn qoi(b: &[u8], dst: &mut [u32]) -> Result<(), Error> {
    if b.len() < 14 + 8 {
        return Err(Error::Corto);
    }
    if b[12] != 3 && b[12] != 4 {
        return Err(Error::Variante);
    }
    if b[13] > 1 {
        return Err(Error::Variante);
    }
    // Los ocho ultimos bytes son la marca de fin y no son datos.
    let fin = b.len() - 8;
    let mut tabla = [[0u8; 4]; 64];
    let mut px = [0u8, 0, 0, 255];
    let mut tanda = 0u32;
    let mut i = 14usize;
    for out in dst.iter_mut() {
        if tanda > 0 {
            tanda -= 1;
        } else {
            // ** Cada chunk se mide contra `fin` ANTES de leerlo: un QOI que
            // promete 1024x1024 y trae diez bytes se para aqui con `Corto`.
            let b1 = *b.get(i).filter(|_| i < fin).ok_or(Error::Corto)?;
            let largo = match b1 {
                0xFE => 4,
                0xFF => 5,
                _ if b1 >> 6 == 2 => 2,
                _ => 1,
            };
            if i + largo > fin {
                return Err(Error::Corto);
            }
            match b1 {
                0xFE => px = [b[i + 1], b[i + 2], b[i + 3], px[3]],
                0xFF => px = [b[i + 1], b[i + 2], b[i + 3], b[i + 4]],
                _ => match b1 >> 6 {
                    0 => px = tabla[(b1 & 63) as usize],
                    1 => {
                        px[0] = px[0].wrapping_add((b1 >> 4) & 3).wrapping_sub(2);
                        px[1] = px[1].wrapping_add((b1 >> 2) & 3).wrapping_sub(2);
                        px[2] = px[2].wrapping_add(b1 & 3).wrapping_sub(2);
                    }
                    2 => {
                        let dv = (b1 & 63).wrapping_sub(32);
                        let b2 = b[i + 1];
                        px[0] = px[0].wrapping_add(dv).wrapping_add(b2 >> 4).wrapping_sub(8);
                        px[1] = px[1].wrapping_add(dv);
                        px[2] = px[2].wrapping_add(dv).wrapping_add(b2 & 15).wrapping_sub(8);
                    }
                    _ => tanda = (b1 & 63) as u32,
                },
            }
            i += largo;
            let h = (px[0] as usize * 3 + px[1] as usize * 5 + px[2] as usize * 7 + px[3] as usize * 11) % 64;
            tabla[h] = px;
        }
        *out = bit_de_alfa(px[3] as u32) | ((px[0] as u32) << 16) | ((px[1] as u32) << 8) | px[2] as u32;
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn u16le(v: &mut Vec<u8>, x: u16) {
        v.extend_from_slice(&x.to_le_bytes());
    }
    fn u32le(v: &mut Vec<u8>, x: u32) {
        v.extend_from_slice(&x.to_le_bytes());
    }

    fn bico(w: u16, h: u16, px: &[u32]) -> Vec<u8> {
        let mut v = b"BICO".to_vec();
        u16le(&mut v, w);
        u16le(&mut v, h);
        for p in px {
            u32le(&mut v, *p);
        }
        v
    }

    /// Un BMP con los pixeles YA ordenados como van en el fichero.
    fn bmp(w: i32, h: i32, bits: u16, compresion: u32, datos: &[u8]) -> Vec<u8> {
        let mut v = b"BM".to_vec();
        u32le(&mut v, (54 + datos.len()) as u32);
        u32le(&mut v, 0);
        u32le(&mut v, 54);
        u32le(&mut v, 40);
        u32le(&mut v, w as u32);
        u32le(&mut v, h as u32);
        u16le(&mut v, 1);
        u16le(&mut v, bits);
        u32le(&mut v, compresion);
        v.extend_from_slice(&[0u8; 20]);
        v.extend_from_slice(datos);
        v
    }

    fn qoi(w: u32, h: u32, chunks: &[u8]) -> Vec<u8> {
        let mut v = b"qoif".to_vec();
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v.push(4);
        v.push(0);
        v.extend_from_slice(chunks);
        v.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
        v
    }

    fn dec(b: &[u8]) -> Result<(Medidas, Vec<u32>), Error> {
        let mut dst = vec![0u32; 64];
        let m = decodificar(b, &mut dst)?;
        dst.truncate((m.ancho * m.alto) as usize);
        Ok((m, dst))
    }

    #[test]
    fn bico_y_su_alfa_de_un_bit() {
        let (m, px) = dec(&bico(2, 1, &[0xFF11_2233, 0x0044_5566])).unwrap();
        assert_eq!((m.ancho, m.alto, m.formato), (2, 1, Formato::Bico));
        assert_eq!(px, vec![0xFF11_2233, 0x0044_5566]);
    }

    /// De abajo arriba, con el relleno de cuatro de cada fila.
    #[test]
    fn bmp_de_24_bits_de_abajo_arriba() {
        // 2x2: fila de ABAJO primero. BGR por pixel, 6 bytes + 2 de relleno.
        let datos = [
            1, 2, 3, 4, 5, 6, 0, 0, // abajo: (3,2,1) (6,5,4)
            7, 8, 9, 10, 11, 12, 0, 0, // arriba: (9,8,7) (12,11,10)
        ];
        let (_, px) = dec(&bmp(2, 2, 24, 0, &datos)).unwrap();
        assert_eq!(px, vec![0xFF09_0807, 0xFF0C_0B0A, 0xFF03_0201, 0xFF06_0504]);
    }

    /// De arriba abajo (alto negativo), y el cuarto byte NO es alfa.
    #[test]
    fn bmp_de_32_bits_de_arriba_abajo_sale_opaco() {
        let datos = [1, 2, 3, 0, 4, 5, 6, 0];
        let (m, px) = dec(&bmp(2, -1, 32, 0, &datos)).unwrap();
        assert_eq!(m.alto, 1);
        assert_eq!(px, vec![0xFF03_0201, 0xFF06_0504]);
    }

    /// Las seis operaciones de QOI, con los numeros escritos a mano.
    #[test]
    fn qoi_con_todas_sus_operaciones() {
        let chunks = [
            0xFE, 10, 20, 30, // RGB
            0xC1, // RUN de 2
            0x76, // DIFF +1 -1 0
            0xA4, 0x69, // LUMA verde +4, rojo -2, azul +1 respecto al verde
            0x09, // INDEX: el primer color (hash 9)
        ];
        let (m, px) = dec(&qoi(3, 2, &chunks)).unwrap();
        assert_eq!(m.formato, Formato::Qoi);
        assert_eq!(
            px,
            vec![0xFF0A_141E, 0xFF0A_141E, 0xFF0A_141E, 0xFF0B_131E, 0xFF0D_1723, 0xFF0A_141E]
        );
    }

    #[test]
    fn un_qoi_con_alfa_bajo_es_transparente() {
        let (_, px) = dec(&qoi(1, 1, &[0xFF, 1, 2, 3, 100])).unwrap();
        assert_eq!(px, vec![0x0001_0203]);
    }

    // == ** LOS ROTOS: la mitad de la crate que importa ======================

    #[test]
    fn corto_y_desconocido() {
        assert_eq!(dec(b"BI").unwrap_err(), Error::Corto);
        assert_eq!(dec(b"GIF89a").unwrap_err(), Error::NoEsImagen);
    }

    #[test]
    fn un_bico_que_promete_mas_pixeles_de_los_que_trae() {
        let mut b = bico(4, 4, &[0; 3]);
        b.truncate(b.len());
        assert_eq!(dec(&b).unwrap_err(), Error::Corto);
    }

    /// *** EL MALICIOSO CLASICO: la cabecera dice 1000x1000 y trae 8 bytes.
    #[test]
    fn un_bmp_que_miente_sobre_sus_medidas() {
        let mut dst = vec![0u32; 1_000_000];
        let b = bmp(1000, 1000, 24, 0, &[0; 8]);
        assert_eq!(decodificar(&b, &mut dst).unwrap_err(), Error::Corto);
    }

    #[test]
    fn medidas_imposibles() {
        assert_eq!(dec(&bico(0, 4, &[])).unwrap_err(), Error::Medidas);
        assert_eq!(dec(&bmp(2000, 1, 24, 0, &[])).unwrap_err(), Error::Medidas);
        // Ancho negativo: leido sin signo es enorme.
        assert_eq!(dec(&bmp(-2, 1, 24, 0, &[0; 8])).unwrap_err(), Error::Medidas);
        assert_eq!(dec(&qoi(5000, 1, &[])).unwrap_err(), Error::Medidas);
    }

    #[test]
    fn variantes_que_no_se_soportan() {
        assert_eq!(dec(&bmp(1, 1, 24, 1, &[0; 4])).unwrap_err(), Error::Variante);
        assert_eq!(dec(&bmp(1, 1, 16, 0, &[0; 4])).unwrap_err(), Error::Variante);
    }

    #[test]
    fn un_qoi_cortado_a_media_imagen() {
        // Promete 4 pixeles y trae uno.
        assert_eq!(dec(&qoi(2, 2, &[0xFE, 1, 2, 3])).unwrap_err(), Error::Corto);
        // Un RGB al que le faltan bytes justo antes de la marca de fin.
        assert_eq!(dec(&qoi(1, 1, &[0xFE, 1])).unwrap_err(), Error::Corto);
    }

    #[test]
    fn un_bufer_pequeno_es_no_cabe_y_no_una_escritura_de_mas() {
        let mut dst = vec![0u32; 3];
        let b = bico(2, 2, &[1, 2, 3, 4]);
        assert_eq!(decodificar(&b, &mut dst).unwrap_err(), Error::NoCabe);
        assert_eq!(dst, vec![0, 0, 0], "no toco el bufer");
    }

    /// ** HOSTILE PASS (2026-09-17): an image can come from the antenna, so its
    /// bytes are a stranger's. Every format, mutated, decoded into a buffer
    /// that is sometimes big enough and sometimes one pixel short. Checked:
    /// nothing panics, and a decode never claims more pixels than it was given.
    #[test]
    fn hostile_images_never_panic() {
        let a = bico(2, 2, &[0xFF11_2233, 0, 0x8044_5566, 0xFFFF_FFFF]);
        let b = bmp(2, 2, 24, 0, &[1, 2, 3, 4, 5, 6, 0, 0, 7, 8, 9, 10, 11, 12, 0, 0]);
        let c = qoi(2, 2, &[0xFE, 10, 20, 30, 0xC0 | 2]);
        for (name, good) in [("bico", &a), ("bmp", &b), ("qoi", &c)] {
            assert!(dec(good).is_ok(), "the {} sample must be a GOOD image, or the mutations never pass the header: {:?}", name, dec(good));
        }
        bmo_hostile::attack("imagen", bmo_hostile::DEFAULT_SEED, 30_000, &[&a, &b, &c], 512, |x| {
            let _ = medir(x);
            for cap in [0usize, 3, 4, 64] {
                let mut dst = vec![0u32; cap];
                if let Ok(m) = decodificar(x, &mut dst) {
                    assert!((m.ancho as usize) * (m.alto as usize) <= cap, "claimed more pixels than the buffer holds");
                }
            }
        });
    }
}
