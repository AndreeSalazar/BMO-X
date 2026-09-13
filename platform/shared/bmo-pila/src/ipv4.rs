//! **IPv4**, y solo la parte que no tiene historia de agujeros.
//!
//! ```text
//!    se rechaza          por que
//!    ------------------  ------------------------------------------------
//!    OPCIONES (IHL > 5)  source routing, timestamps: nadie las necesita y
//!                        todas se usaron para atacar
//!    FRAGMENTOS          reensamblar es el agujero clasico (teardrop,
//!                        solapes, agotar memoria). DF sale siempre puesto
//!    bit reservado       no existe; si viene puesto, alguien prueba algo
//!    origen imposible    0.0.0.0, 127/8, multidifusion y clase E
//!    TTL cero            no deberia haber llegado
//! ```
//!
//! El relleno Ethernet que venga detras del `largo total` se descarta: la
//! carga es exactamente lo que dice la cabecera.

use crate::{be16, pon16, suma, Rechazo};

pub type Ip = [u8; 4];

pub const CABECERA: usize = 20;
pub const ICMP: u8 = 1;
pub const TCP: u8 = 6;
pub const UDP: u8 = 17;
pub const TTL: u8 = 64;
/// El paquete mas grande que sale: la MTU de Ethernet.
pub const MTU: usize = 1500;

pub fn es_difusion(ip: &Ip) -> bool {
    *ip == [255; 4]
}

pub fn es_grupo(ip: &Ip) -> bool {
    (224..=239).contains(&ip[0])
}

/// Nadie de fuera puede escribir desde aqui.
pub fn origen_imposible(ip: &Ip) -> bool {
    *ip == [0; 4] || ip[0] == 127 || ip[0] >= 224
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paquete<'a> {
    pub origen: Ip,
    pub destino: Ip,
    pub protocolo: u8,
    pub ttl: u8,
    pub id: u16,
    pub carga: &'a [u8],
}

pub fn leer(b: &[u8]) -> Result<Paquete<'_>, Rechazo> {
    if b.len() < CABECERA {
        return Err(Rechazo::Corto);
    }
    if b[0] >> 4 != 4 {
        return Err(Rechazo::Version);
    }
    if b[0] & 0x0F != 5 {
        return Err(Rechazo::Opciones);
    }
    let total = be16(b, 2) as usize;
    if total < CABECERA {
        return Err(Rechazo::Cabecera);
    }
    if total > b.len() {
        return Err(Rechazo::Corto);
    }
    let banderas = be16(b, 6);
    if banderas & 0x8000 != 0 {
        return Err(Rechazo::Banderas);
    }
    // MF (0x2000) o un desplazamiento (0x1FFF): un trozo de algo.
    if banderas & 0x3FFF != 0 {
        return Err(Rechazo::Fragmento);
    }
    if b[8] == 0 {
        return Err(Rechazo::Cabecera);
    }
    if suma::de(&b[..CABECERA]) != 0 {
        return Err(Rechazo::Suma);
    }
    let origen = [b[12], b[13], b[14], b[15]];
    if origen_imposible(&origen) {
        return Err(Rechazo::Direccion);
    }
    Ok(Paquete {
        origen,
        destino: [b[16], b[17], b[18], b[19]],
        protocolo: b[9],
        ttl: b[8],
        id: be16(b, 4),
        carga: &b[CABECERA..total],
    })
}

/// Escribe la cabecera para `largo_carga` bytes que van detras. DF puesto.
pub fn escribir(
    dst: &mut [u8],
    origen: Ip,
    destino: Ip,
    protocolo: u8,
    largo_carga: usize,
    id: u16,
) -> Result<usize, Rechazo> {
    let total = CABECERA + largo_carga;
    if total > MTU {
        return Err(Rechazo::Largo);
    }
    if dst.len() < CABECERA {
        return Err(Rechazo::Corto);
    }
    let b = &mut dst[..CABECERA];
    b[0] = 0x45;
    b[1] = 0;
    pon16(b, 2, total as u16);
    pon16(b, 4, id);
    pon16(b, 6, 0x4000);
    b[8] = TTL;
    b[9] = protocolo;
    pon16(b, 10, 0);
    b[12..16].copy_from_slice(&origen);
    b[16..20].copy_from_slice(&destino);
    let s = suma::de(b);
    pon16(b, 10, s);
    Ok(CABECERA)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const A: Ip = [192, 168, 1, 10];
    const B: Ip = [192, 168, 1, 1];

    fn paquete(carga: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; CABECERA + carga.len()];
        escribir(&mut b, A, B, UDP, carga.len(), 7).unwrap();
        b[CABECERA..].copy_from_slice(carga);
        b
    }

    fn resumar(b: &mut [u8]) {
        pon16(b, 10, 0);
        let s = suma::de(&b[..CABECERA]);
        pon16(b, 10, s);
    }

    #[test]
    fn ida_y_vuelta_y_el_relleno_se_va() {
        let mut b = paquete(b"hola");
        b.extend_from_slice(&[0; 22]);
        let p = leer(&b).unwrap();
        assert_eq!((p.origen, p.destino, p.protocolo, p.carga), (A, B, UDP, &b"hola"[..]));
    }

    #[test]
    fn opciones_fragmentos_y_bit_reservado() {
        let mut b = paquete(b"x");
        b[0] = 0x46;
        assert_eq!(leer(&b), Err(Rechazo::Opciones));
        for (banderas, motivo) in
            [(0x2000u16, Rechazo::Fragmento), (0x0001, Rechazo::Fragmento), (0x8000, Rechazo::Banderas)]
        {
            let mut b = paquete(b"x");
            pon16(&mut b, 6, banderas);
            resumar(&mut b);
            assert_eq!(leer(&b), Err(motivo), "{banderas:#x}");
        }
    }

    #[test]
    fn la_suma_mala_y_el_largo_mentiroso() {
        let mut b = paquete(b"abc");
        b[11] ^= 1;
        assert_eq!(leer(&b), Err(Rechazo::Suma));
        let mut b = paquete(b"abc");
        pon16(&mut b, 2, 200);
        resumar(&mut b);
        assert_eq!(leer(&b), Err(Rechazo::Corto));
        pon16(&mut b, 2, 19);
        resumar(&mut b);
        assert_eq!(leer(&b), Err(Rechazo::Cabecera));
    }

    #[test]
    fn origenes_imposibles() {
        for o in [[0, 0, 0, 0], [127, 0, 0, 1], [224, 0, 0, 251], [255, 255, 255, 255], [240, 1, 2, 3]] {
            let mut b = paquete(b"");
            b[12..16].copy_from_slice(&o);
            resumar(&mut b);
            assert_eq!(leer(&b), Err(Rechazo::Direccion), "{o:?}");
        }
    }

    #[test]
    fn no_sale_nada_mayor_que_la_mtu() {
        let mut b = [0u8; 20];
        assert_eq!(escribir(&mut b, A, B, TCP, 1481, 0), Err(Rechazo::Largo));
    }
}
