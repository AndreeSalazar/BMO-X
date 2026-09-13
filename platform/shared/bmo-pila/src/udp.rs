//! **UDP**: ocho bytes de cabecera y una suma.
//!
//! En IPv4 la suma de UDP es OPCIONAL (un cero dice "no la calcule"). Aqui no:
//! un datagrama sin suma se rechaza. Todos los sistemas actuales la ponen, y el
//! que no la pone es o muy viejo o esta probando que pasa.

use crate::ipv4::{self, Ip};
use crate::{be16, pon16, suma, Rechazo};

pub const CABECERA: usize = 8;
/// Lo mas grande que cabe en un paquete sin fragmentar.
pub const MAX_DATOS: usize = ipv4::MTU - ipv4::CABECERA - CABECERA;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Datagrama<'a> {
    pub origen: u16,
    pub destino: u16,
    pub datos: &'a [u8],
}

pub fn leer(b: &[u8], ip_origen: Ip, ip_destino: Ip) -> Result<Datagrama<'_>, Rechazo> {
    if b.len() < CABECERA {
        return Err(Rechazo::Corto);
    }
    let largo = be16(b, 4) as usize;
    if largo < CABECERA {
        return Err(Rechazo::Cabecera);
    }
    if largo > b.len() {
        return Err(Rechazo::Corto);
    }
    let b = &b[..largo];
    if be16(b, 6) == 0 {
        return Err(Rechazo::Suma);
    }
    let mut s = suma::pseudo(ip_origen, ip_destino, ipv4::UDP, largo as u16);
    s.bytes(b);
    if s.cerrar() != 0 {
        return Err(Rechazo::Suma);
    }
    let (origen, destino) = (be16(b, 0), be16(b, 2));
    if origen == 0 || destino == 0 {
        return Err(Rechazo::Puerto);
    }
    Ok(Datagrama { origen, destino, datos: &b[CABECERA..] })
}

pub fn escribir(
    dst: &mut [u8],
    ip_origen: Ip,
    ip_destino: Ip,
    origen: u16,
    destino: u16,
    datos: &[u8],
) -> Result<usize, Rechazo> {
    if datos.len() > MAX_DATOS {
        return Err(Rechazo::Largo);
    }
    let n = CABECERA + datos.len();
    if dst.len() < n {
        return Err(Rechazo::Corto);
    }
    let b = &mut dst[..n];
    pon16(b, 0, origen);
    pon16(b, 2, destino);
    pon16(b, 4, n as u16);
    pon16(b, 6, 0);
    b[CABECERA..].copy_from_slice(datos);
    let mut s = suma::pseudo(ip_origen, ip_destino, ipv4::UDP, n as u16);
    s.bytes(b);
    // Un cero calculado se escribe como 0xFFFF: el cero significa "sin suma".
    let s = match s.cerrar() {
        0 => 0xFFFF,
        v => v,
    };
    pon16(b, 6, s);
    Ok(n)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const A: Ip = [10, 0, 0, 2];
    const B: Ip = [10, 0, 0, 1];

    #[test]
    fn ida_y_vuelta() {
        let mut b = [0u8; 64];
        let n = escribir(&mut b, A, B, 5353, 53, b"consulta").unwrap();
        let d = leer(&b[..n], A, B).unwrap();
        assert_eq!((d.origen, d.destino, d.datos), (5353, 53, &b"consulta"[..]));
    }

    #[test]
    fn la_suma_ata_las_ip() {
        let mut b = [0u8; 64];
        let n = escribir(&mut b, A, B, 1000, 2000, b"x").unwrap();
        assert_eq!(leer(&b[..n], [10, 0, 0, 9], B), Err(Rechazo::Suma), "otra IP de origen");
    }

    #[test]
    fn sin_suma_y_puertos_cero() {
        let mut b = [0u8; 64];
        let n = escribir(&mut b, A, B, 1000, 2000, b"x").unwrap();
        let mut sin = b;
        pon16(&mut sin, 6, 0);
        assert_eq!(leer(&sin[..n], A, B), Err(Rechazo::Suma));
        let n = escribir(&mut b, A, B, 0, 2000, b"x").unwrap();
        assert_eq!(leer(&b[..n], A, B), Err(Rechazo::Puerto));
    }

    #[test]
    fn largos_que_mienten() {
        let mut b = [0u8; 64];
        let n = escribir(&mut b, A, B, 1, 2, b"abc").unwrap();
        pon16(&mut b, 4, 7);
        assert_eq!(leer(&b[..n], A, B), Err(Rechazo::Cabecera));
        pon16(&mut b, 4, 40);
        assert_eq!(leer(&b[..n], A, B), Err(Rechazo::Corto));
    }
}
