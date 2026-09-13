//! **La suma de Internet** (RFC 1071): la misma para IPv4, ICMP, UDP y TCP.
//!
//! Complemento a uno de las palabras de 16 bits. Se puede ir alimentando a
//! trozos --la pseudo-cabecera, la cabecera, los datos-- y los trozos pueden
//! tener cualquier largo: un byte suelto al final de uno se empareja con el
//! primero del siguiente.
//!
//! Comprobar es sumar TODO, suma incluida: el resultado tiene que ser cero.

/// Una suma a medio hacer.
#[derive(Clone, Copy, Debug)]
pub struct Suma {
    acum: u32,
    impar: Option<u8>,
}

impl Suma {
    pub const fn nueva() -> Self {
        Self { acum: 0, impar: None }
    }

    fn anadir(&mut self, v: u16) {
        self.acum += v as u32;
        // Se pliega en cada paso: `acum` nunca pasa de 0xFFFF y no hay forma de
        // desbordarlo con ningun largo de entrada.
        self.acum = (self.acum & 0xFFFF) + (self.acum >> 16);
    }

    pub fn bytes(&mut self, mut b: &[u8]) {
        if let Some(alto) = self.impar.take() {
            match b.split_first() {
                Some((&bajo, resto)) => {
                    self.anadir(u16::from_be_bytes([alto, bajo]));
                    b = resto;
                }
                None => {
                    self.impar = Some(alto);
                    return;
                }
            }
        }
        let mut pares = b.chunks_exact(2);
        for p in &mut pares {
            self.anadir(u16::from_be_bytes([p[0], p[1]]));
        }
        if let [suelto] = pares.remainder() {
            self.impar = Some(*suelto);
        }
    }

    pub fn u16(&mut self, v: u16) {
        self.bytes(&v.to_be_bytes());
    }

    /// El complemento: lo que va en el campo de la suma, o cero al comprobar.
    pub fn cerrar(mut self) -> u16 {
        if let Some(alto) = self.impar.take() {
            self.anadir(u16::from_be_bytes([alto, 0]));
        }
        !(self.acum as u16)
    }
}

/// La suma de un bloque entero.
pub fn de(b: &[u8]) -> u16 {
    let mut s = Suma::nueva();
    s.bytes(b);
    s.cerrar()
}

/// La pseudo-cabecera de UDP y TCP: origen, destino, protocolo y largo.
pub fn pseudo(origen: [u8; 4], destino: [u8; 4], protocolo: u8, largo: u16) -> Suma {
    let mut s = Suma::nueva();
    s.bytes(&origen);
    s.bytes(&destino);
    s.u16(protocolo as u16);
    s.u16(largo);
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El ejemplo del propio RFC 1071, seccion 3.
    #[test]
    fn el_ejemplo_del_rfc_1071() {
        assert_eq!(de(&[0x00, 0x01, 0xF2, 0x03, 0xF4, 0xF5, 0xF6, 0xF7]), !0xDDF2);
    }

    /// Una cabecera IPv4 real con su suma puesta: comprobar da cero.
    #[test]
    fn una_cabecera_buena_suma_cero() {
        let c = [
            0x45, 0x00, 0x00, 0x73, 0x00, 0x00, 0x40, 0x00, 0x40, 0x11, 0xB8, 0x61, 0xC0, 0xA8, 0x00,
            0x01, 0xC0, 0xA8, 0x00, 0xC7,
        ];
        assert_eq!(de(&c), 0);
    }

    /// A trozos o de una vez, y con cortes impares: el mismo numero.
    #[test]
    fn a_trozos_da_lo_mismo() {
        let datos: Vec<u8> = (0..101u32).map(|i| (i * 37 + 11) as u8).collect();
        let entera = de(&datos);
        for corte in 0..datos.len() {
            let mut s = Suma::nueva();
            s.bytes(&datos[..corte]);
            s.bytes(&[]);
            s.bytes(&datos[corte..]);
            assert_eq!(s.cerrar(), entera, "corte en {corte}");
        }
    }
}
