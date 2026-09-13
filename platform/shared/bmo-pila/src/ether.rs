//! **Ethernet II**: la trama que entrega el driver.
//!
//! ** LISTA BLANCA, no lista negra. Solo pasan dos tipos --IPv4 y ARP--; VLAN,
//! IPv6, 802.3 con largo y cualquier cosa que invente otro se rechazan con su
//! motivo. Una pila que "ya vera que hace" con un tipo desconocido es una pila
//! con un camino que nadie probo.

use crate::{be16, pon16, Rechazo};

pub type Mac = [u8; 6];

pub const CABECERA: usize = 14;
pub const MAX_CARGA: usize = 1500;
/// Lo minimo que viaja por el cable (sin la suma de la tarjeta).
pub const MIN_TRAMA: usize = 60;
pub const DIFUSION: Mac = [0xFF; 6];
pub const TIPO_IPV4: u16 = 0x0800;
pub const TIPO_ARP: u16 = 0x0806;

/// El bit de GRUPO: difusion o multidifusion. Nadie puede ser un grupo como
/// origen.
pub fn es_grupo(m: &Mac) -> bool {
    m[0] & 1 != 0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trama<'a> {
    pub destino: Mac,
    pub origen: Mac,
    pub tipo: u16,
    pub carga: &'a [u8],
}

fn mac(b: &[u8]) -> Mac {
    [b[0], b[1], b[2], b[3], b[4], b[5]]
}

pub fn leer(b: &[u8]) -> Result<Trama<'_>, Rechazo> {
    if b.len() < CABECERA {
        return Err(Rechazo::Corto);
    }
    if b.len() > CABECERA + MAX_CARGA {
        return Err(Rechazo::Largo);
    }
    let destino = mac(&b[0..6]);
    let origen = mac(&b[6..12]);
    if es_grupo(&origen) || origen == [0; 6] {
        return Err(Rechazo::Direccion);
    }
    let tipo = be16(b, 12);
    if tipo != TIPO_IPV4 && tipo != TIPO_ARP {
        return Err(Rechazo::Tipo);
    }
    Ok(Trama { destino, origen, tipo, carga: &b[CABECERA..] })
}

/// Escribe la cabecera. La carga va detras, desde `CABECERA`.
pub fn escribir(dst: &mut [u8], destino: Mac, origen: Mac, tipo: u16) -> Result<usize, Rechazo> {
    if dst.len() < CABECERA {
        return Err(Rechazo::Corto);
    }
    dst[0..6].copy_from_slice(&destino);
    dst[6..12].copy_from_slice(&origen);
    pon16(dst, 12, tipo);
    Ok(CABECERA)
}

/// Rellena con ceros hasta `MIN_TRAMA` si hace falta. Devuelve el largo final.
/// Con ceros y no con lo que hubiera: el relleno viejo es memoria de otra trama.
pub fn con_relleno(dst: &mut [u8], largo: usize) -> usize {
    if largo >= MIN_TRAMA || dst.len() < MIN_TRAMA {
        return largo;
    }
    dst[largo..MIN_TRAMA].fill(0);
    MIN_TRAMA
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const YO: Mac = [0x02, 0x1A, 0x2B, 0x3C, 0x4D, 0x5E];
    const OTRO: Mac = [0x02, 0x11, 0x22, 0x33, 0x44, 0x55];

    #[test]
    fn ida_y_vuelta() {
        let mut b = [0u8; 64];
        escribir(&mut b, YO, OTRO, TIPO_ARP).unwrap();
        let t = leer(&b[..60]).unwrap();
        assert_eq!((t.destino, t.origen, t.tipo, t.carga.len()), (YO, OTRO, TIPO_ARP, 46));
    }

    #[test]
    fn lo_que_no_esta_en_la_lista_no_pasa() {
        let mut b = [0u8; 60];
        for tipo in [0x86DDu16, 0x8100, 0x0042, 0x88CC] {
            escribir(&mut b, YO, OTRO, tipo).unwrap();
            assert_eq!(leer(&b), Err(Rechazo::Tipo), "{tipo:#x}");
        }
    }

    #[test]
    fn un_grupo_no_puede_ser_origen() {
        let mut b = [0u8; 60];
        escribir(&mut b, YO, DIFUSION, TIPO_IPV4).unwrap();
        assert_eq!(leer(&b), Err(Rechazo::Direccion));
        escribir(&mut b, YO, [0; 6], TIPO_IPV4).unwrap();
        assert_eq!(leer(&b), Err(Rechazo::Direccion));
    }

    #[test]
    fn corta_y_gigante() {
        assert_eq!(leer(&[0u8; 13]), Err(Rechazo::Corto));
        assert_eq!(leer(&[0u8; 1515]), Err(Rechazo::Largo));
    }

    #[test]
    fn el_relleno_es_de_ceros() {
        let mut b = [0xAAu8; 64];
        assert_eq!(con_relleno(&mut b, 42), 60);
        assert!(b[42..60].iter().all(|&x| x == 0));
    }
}
