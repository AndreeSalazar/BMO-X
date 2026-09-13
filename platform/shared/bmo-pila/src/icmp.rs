//! **ICMP**: solo el eco (`ping`). Todo lo demas se rechaza con su motivo.
//!
//! "Destino inalcanzable", redirecciones y compania son la forma clasica de
//! que alguien de fuera cambie las rutas o corte una conexion con un paquete
//! falso. Una pila que no los lee no se deja mandar por ellos; el precio es que
//! TCP tarda sus reintentos en rendirse, y ese precio se ACEPTA.

use crate::{be16, pon16, suma, Rechazo};

pub const CABECERA: usize = 8;
pub const ECO_RESPUESTA: u8 = 0;
pub const ECO_PETICION: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Eco<'a> {
    pub tipo: u8,
    pub id: u16,
    pub secuencia: u16,
    pub datos: &'a [u8],
}

pub fn leer(b: &[u8]) -> Result<Eco<'_>, Rechazo> {
    if b.len() < CABECERA {
        return Err(Rechazo::Corto);
    }
    if suma::de(b) != 0 {
        return Err(Rechazo::Suma);
    }
    if (b[0] != ECO_PETICION && b[0] != ECO_RESPUESTA) || b[1] != 0 {
        return Err(Rechazo::Tipo);
    }
    Ok(Eco { tipo: b[0], id: be16(b, 4), secuencia: be16(b, 6), datos: &b[CABECERA..] })
}

pub fn escribir(dst: &mut [u8], tipo: u8, id: u16, secuencia: u16, datos: &[u8]) -> Result<usize, Rechazo> {
    let n = CABECERA + datos.len();
    if dst.len() < n {
        return Err(Rechazo::Corto);
    }
    let b = &mut dst[..n];
    b[0] = tipo;
    b[1] = 0;
    pon16(b, 2, 0);
    pon16(b, 4, id);
    pon16(b, 6, secuencia);
    b[CABECERA..].copy_from_slice(datos);
    let s = suma::de(b);
    pon16(b, 2, s);
    Ok(n)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn ida_y_vuelta() {
        let mut b = [0u8; 64];
        let n = escribir(&mut b, ECO_PETICION, 0xBEEF, 3, b"abcdefgh").unwrap();
        let e = leer(&b[..n]).unwrap();
        assert_eq!((e.tipo, e.id, e.secuencia, e.datos), (ECO_PETICION, 0xBEEF, 3, &b"abcdefgh"[..]));
    }

    #[test]
    fn inalcanzable_y_redireccion_no_se_leen() {
        for tipo in [3u8, 5, 11, 13] {
            let mut b = [0u8; 16];
            let n = escribir(&mut b, tipo, 0, 0, &[]).unwrap();
            assert_eq!(leer(&b[..n]), Err(Rechazo::Tipo), "tipo {tipo}");
        }
    }

    #[test]
    fn la_suma_se_comprueba() {
        let mut b = [0u8; 16];
        let n = escribir(&mut b, ECO_PETICION, 1, 1, b"xy").unwrap();
        b[n - 1] ^= 0x40;
        assert_eq!(leer(&b[..n]), Err(Rechazo::Suma));
    }
}
