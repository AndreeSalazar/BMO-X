//! **El segmento TCP**: leerlo sin creerselo y escribirlo.
//!
//! ```text
//!    se rechaza                     por que
//!    -----------------------------  -----------------------------------------
//!    SYN+FIN, SYN+RST               no existen en una conversacion honrada
//!    sin ninguna bandera            el "null scan"
//!    FIN sin ACK                    el "FIN scan" y el "Xmas"
//!    URG                            el puntero urgente: nadie lo usa bien y
//!                                   cada pila lo interpreta distinto
//!    opcion con largo 0, 1 o que    el bucle infinito y la lectura fuera de
//!    se sale de la cabecera         la cabecera, las dos clasicas
//! ```
//!
//! De las opciones solo se LEE el MSS. Escala de ventana, SACK y marcas de
//! tiempo se saltan con su largo comprobado: no se negocian, asi que el otro
//! lado no las usara.

use crate::ipv4::{self, Ip};
use crate::{be16, be32, pon16, pon32, suma, Rechazo};

pub const CABECERA: usize = 20;

pub mod bandera {
    pub const FIN: u8 = 0x01;
    pub const SYN: u8 = 0x02;
    pub const RST: u8 = 0x04;
    pub const PSH: u8 = 0x08;
    pub const ACK: u8 = 0x10;
    pub const URG: u8 = 0x20;
}
use bandera::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segmento<'a> {
    pub origen: u16,
    pub destino: u16,
    pub sec: u32,
    pub ack: u32,
    pub banderas: u8,
    pub ventana: u16,
    pub mss: Option<u16>,
    pub datos: &'a [u8],
}

pub fn leer(b: &[u8], ip_origen: Ip, ip_destino: Ip) -> Result<Segmento<'_>, Rechazo> {
    if b.len() < CABECERA {
        return Err(Rechazo::Corto);
    }
    if b.len() > ipv4::MTU - ipv4::CABECERA {
        return Err(Rechazo::Largo);
    }
    let desp = (b[12] >> 4) as usize * 4;
    if desp < CABECERA {
        return Err(Rechazo::Cabecera);
    }
    if desp > b.len() {
        return Err(Rechazo::Corto);
    }
    let mut s = suma::pseudo(ip_origen, ip_destino, ipv4::TCP, b.len() as u16);
    s.bytes(b);
    if s.cerrar() != 0 {
        return Err(Rechazo::Suma);
    }
    let (origen, destino) = (be16(b, 0), be16(b, 2));
    if origen == 0 || destino == 0 {
        return Err(Rechazo::Puerto);
    }
    // Solo las seis de siempre; ECE y CWR (0xC0) se ignoran sin rechazar.
    let banderas = b[13] & 0x3F;
    let raras = banderas == 0
        || banderas & (SYN | FIN) == SYN | FIN
        || banderas & (SYN | RST) == SYN | RST
        || banderas & URG != 0
        || (banderas & FIN != 0 && banderas & ACK == 0);
    if raras {
        return Err(Rechazo::Banderas);
    }
    let mut mss = None;
    let mut i = CABECERA;
    while i < desp {
        match b[i] {
            0 => break,
            1 => i += 1,
            tipo => {
                if i + 1 >= desp {
                    return Err(Rechazo::Opciones);
                }
                let largo = b[i + 1] as usize;
                if largo < 2 || i + largo > desp {
                    return Err(Rechazo::Opciones);
                }
                if tipo == 2 {
                    if largo != 4 {
                        return Err(Rechazo::Opciones);
                    }
                    mss = Some(be16(b, i + 2));
                }
                i += largo;
            }
        }
    }
    Ok(Segmento {
        origen,
        destino,
        sec: be32(b, 4),
        ack: be32(b, 8),
        banderas,
        ventana: be16(b, 14),
        mss,
        datos: &b[desp..],
    })
}

#[allow(clippy::too_many_arguments)]
pub fn escribir(
    dst: &mut [u8],
    ip_origen: Ip,
    ip_destino: Ip,
    origen: u16,
    destino: u16,
    sec: u32,
    ack: u32,
    banderas: u8,
    ventana: u16,
    mss: Option<u16>,
    datos: &[u8],
) -> Result<usize, Rechazo> {
    let desp = CABECERA + if mss.is_some() { 4 } else { 0 };
    let n = desp + datos.len();
    if n > ipv4::MTU - ipv4::CABECERA {
        return Err(Rechazo::Largo);
    }
    if dst.len() < n {
        return Err(Rechazo::Corto);
    }
    let b = &mut dst[..n];
    pon16(b, 0, origen);
    pon16(b, 2, destino);
    pon32(b, 4, sec);
    pon32(b, 8, ack);
    b[12] = ((desp / 4) as u8) << 4;
    b[13] = banderas;
    pon16(b, 14, ventana);
    pon16(b, 16, 0);
    pon16(b, 18, 0);
    if let Some(m) = mss {
        b[20] = 2;
        b[21] = 4;
        pon16(b, 22, m);
    }
    b[desp..].copy_from_slice(datos);
    let mut s = suma::pseudo(ip_origen, ip_destino, ipv4::TCP, n as u16);
    s.bytes(b);
    let v = s.cerrar();
    pon16(b, 16, v);
    Ok(n)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const A: Ip = [10, 0, 0, 2];
    const B: Ip = [10, 0, 0, 1];

    fn resumar(b: &mut [u8]) {
        pon16(b, 16, 0);
        let mut s = suma::pseudo(A, B, ipv4::TCP, b.len() as u16);
        s.bytes(b);
        let v = s.cerrar();
        pon16(b, 16, v);
    }

    #[test]
    fn ida_y_vuelta_con_mss() {
        let mut b = [0u8; 64];
        let n = escribir(&mut b, A, B, 50000, 80, 1, 2, SYN, 8192, Some(1460), b"").unwrap();
        let s = leer(&b[..n], A, B).unwrap();
        assert_eq!((s.origen, s.destino, s.sec, s.ack, s.banderas, s.ventana, s.mss), (50000, 80, 1, 2, SYN, 8192, Some(1460)));
    }

    #[test]
    fn las_combinaciones_de_los_escaneres() {
        for banderas in [0u8, SYN | FIN, SYN | RST, FIN, FIN | PSH | URG, ACK | URG] {
            let mut b = [0u8; 20];
            escribir(&mut b, A, B, 1, 2, 0, 0, banderas, 0, None, b"").unwrap();
            assert_eq!(leer(&b, A, B), Err(Rechazo::Banderas), "{banderas:#x}");
        }
    }

    #[test]
    fn opciones_que_harian_dano() {
        // Largo 0: el bucle infinito. Largo que se sale: leer fuera.
        for opciones in [[3u8, 0, 0, 0], [3, 9, 0, 0], [2, 3, 5, 0], [8, 1, 0, 0]] {
            let mut b = [0u8; 24];
            escribir(&mut b, A, B, 1, 2, 0, 0, ACK, 0, Some(0), b"").unwrap();
            b[20..24].copy_from_slice(&opciones);
            resumar(&mut b);
            assert_eq!(leer(&b, A, B), Err(Rechazo::Opciones), "{opciones:?}");
        }
        // NOP, NOP y fin de lista: bien, y sin MSS.
        let mut b = [0u8; 24];
        escribir(&mut b, A, B, 1, 2, 0, 0, ACK, 0, Some(0), b"").unwrap();
        b[20..24].copy_from_slice(&[1, 1, 0, 0]);
        resumar(&mut b);
        assert_eq!(leer(&b, A, B).unwrap().mss, None);
    }

    #[test]
    fn el_desplazamiento_mentiroso() {
        let mut b = [0u8; 20];
        escribir(&mut b, A, B, 1, 2, 0, 0, ACK, 0, None, b"").unwrap();
        b[12] = 0x40;
        resumar(&mut b);
        assert_eq!(leer(&b, A, B), Err(Rechazo::Cabecera));
        b[12] = 0xF0;
        resumar(&mut b);
        assert_eq!(leer(&b, A, B), Err(Rechazo::Corto));
    }
}
