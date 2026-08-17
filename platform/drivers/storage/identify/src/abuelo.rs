//! **EL ABUELO** -- el sector de IDENTIFY en crudo. No sabe para que se usa.
//!
//! [eje]     CORRECCION -- no hay nada que optimizar aqui
//! [exige]   R-DISCO2 (esta generacion NO aplica sesgos: los deja pasar)
//!
//! # Que sabe hacer, y que tiene PROHIBIDO saber
//!
//! Sabe dos cosas: **devolver la palabra n** y **sacar una cadena** deshaciendo
//! el intercambio de bytes que ATA hace dentro de cada palabra. Nada mas.
//!
//! No sabe que la palabra 217 es la rotacion, ni que la 75 lleva un sesgo de
//! menos uno, ni que la 106 tiene una guarda de validez. **No puede saberlo**,
//! y eso no es modestia: es lo que hace falsable a la generacion de arriba.
//!
//! ## Por que esa ignorancia compra algo (L7, punto 2)
//!
//! Si el abuelo interpretara, un bit mal leido podria estar en dos sitios y
//! habria que mirar los dos. Como no interpreta, **una lectura mal interpretada
//! solo puede estar en el padre** -- que tiene un campo por palabra y una prueba
//! por campo. La busqueda pasa de "en algun sitio del parseo" a "en esta funcion
//! de seis lineas".
//!
//! Y al reves: si el padre saca un valor absurdo, `palabra()` dice **que dio el
//! disco de verdad**, sin opinion encima. Por eso los campos de arriba viajan
//! con su palabra cruda al lado hasta la pantalla.
//!
//! # El intercambio de bytes, que no es un detalle
//!
//! ATA define las cadenas como palabras de 16 bits **con los dos caracteres al
//! reves**: el modelo `"KINGSTON"` llega como `"IKGNTSNO"`. Es herencia de un
//! bus de 16 bits big-endian hablando con un anfitrion little-endian, y esta
//! asi desde ATA-1. Quien lo olvide no obtiene basura: obtiene un texto legible
//! y equivocado, que es peor porque pasa la revision a ojo.

/// Un sector de IDENTIFY DEVICE: 512 bytes = 256 palabras de 16 bits.
pub const PALABRAS: usize = 256;

/// El sector tal como lo dejo el disco.
///
/// Se toma prestado y no se copia: en Ring 0 no hay reservas, y el buffer ya
/// existe donde lo dejo el DMA.
#[derive(Clone, Copy)]
pub struct Identify<'a> {
    crudo: &'a [u8],
}

impl<'a> Identify<'a> {
    /// Envuelve un buffer. `None` si no mide un sector entero -- un IDENTIFY
    /// corto no es un IDENTIFY con menos datos, es una lectura que fallo.
    pub fn nuevo(crudo: &'a [u8]) -> Option<Self> {
        if crudo.len() < PALABRAS * 2 {
            return None;
        }
        Some(Identify { crudo })
    }

    /// La palabra `n`, tal cual. Little-endian, que es como viaja por el bus.
    ///
    /// Fuera de rango devuelve 0 y no entra en panico: esto corre en Ring 0 con
    /// datos que vienen de un aparato, y un aparato puede decir cualquier cosa.
    /// Un 0 aqui lo lee el padre como *"no contesta"*, que es un estado que
    /// todas las palabras de la spec saben expresar.
    pub fn palabra(&self, n: usize) -> u16 {
        if n >= PALABRAS {
            return 0;
        }
        u16::from_le_bytes([self.crudo[n * 2], self.crudo[n * 2 + 1]])
    }

    /// Una cadena ATA de `desde` (inclusive) a `hasta` (exclusive), en palabras.
    ///
    /// Deshace el intercambio de bytes y recorta los espacios de relleno por la
    /// derecha, que la spec exige que existan. Devuelve cuantos bytes escribio.
    ///
    /// **No valida que sean ASCII imprimibles**: eso es una opinion sobre el
    /// contenido y esta generacion no opina. Quien la use decide.
    pub fn cadena(&self, desde: usize, hasta: usize, salida: &mut [u8]) -> usize {
        let mut n = 0;
        for w in desde..hasta {
            let p = self.palabra(w);
            // ** El intercambio: el byte ALTO va primero. Ver la cabecera.
            let par = [(p >> 8) as u8, (p & 0xFF) as u8];
            for b in par {
                if n < salida.len() {
                    salida[n] = b;
                    n += 1;
                }
            }
        }
        while n > 0 && (salida[n - 1] == b' ' || salida[n - 1] == 0) {
            n -= 1;
        }
        n
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Construye un sector con las palabras que se le den.
    fn sector(pares: &[(usize, u16)]) -> [u8; 512] {
        let mut s = [0u8; 512];
        for &(n, v) in pares {
            let b = v.to_le_bytes();
            s[n * 2] = b[0];
            s[n * 2 + 1] = b[1];
        }
        s
    }

    #[test]
    fn un_buffer_corto_no_es_un_identify() {
        assert!(Identify::nuevo(&[0u8; 511]).is_none());
        assert!(Identify::nuevo(&[0u8; 512]).is_some());
    }

    #[test]
    fn devuelve_la_palabra_en_little_endian() {
        let s = sector(&[(217, 0x1C20)]);
        let id = Identify::nuevo(&s).unwrap();
        assert_eq!(id.palabra(217), 0x1C20);
    }

    #[test]
    fn fuera_de_rango_contesta_cero_y_no_entra_en_panico() {
        let s = sector(&[]);
        let id = Identify::nuevo(&s).unwrap();
        assert_eq!(id.palabra(256), 0);
        assert_eq!(id.palabra(usize::MAX), 0);
    }

    /// La prueba que caza el fallo clasico de ATA: sin el intercambio, el
    /// resultado es legible y esta mal.
    ///
    /// ** El texto "KING" ocupa dos palabras, y el PRIMER caracter de cada una
    /// va en el byte ALTO. O sea:
    ///
    /// ```text
    ///    "KI"  ->  ('K' << 8) | 'I'  =  0x4B49   y en el buffer: 49 4B
    ///    "NG"  ->  ('N' << 8) | 'G'  =  0x4E47   y en el buffer: 47 4E
    /// ```
    ///
    /// Los bytes del buffer salen al reves que el texto, y esa es justamente la
    /// razon de que exista el intercambio. (Esta prueba fallo la primera vez por
    /// tener el dato escrito al reves -- el fallo que vigila, cometido al
    /// escribirla.)
    #[test]
    fn la_cadena_deshace_el_intercambio_de_bytes() {
        let s = sector(&[(27, 0x4B49), (28, 0x4E47)]);
        let id = Identify::nuevo(&s).unwrap();
        let mut buf = [0u8; 8];
        let n = id.cadena(27, 29, &mut buf);
        assert_eq!(&buf[..n], b"KING");
    }

    /// Y la contraria, que es la que demuestra que el intercambio HACE algo:
    /// leer las mismas palabras sin intercambiar da un texto **legible y
    /// equivocado**, que es lo que pasa la revision a ojo.
    #[test]
    fn sin_intercambio_saldria_legible_y_mal() {
        let s = sector(&[(27, 0x4B49), (28, 0x4E47)]);
        let id = Identify::nuevo(&s).unwrap();
        let crudo = [
            (id.palabra(27) & 0xFF) as u8, (id.palabra(27) >> 8) as u8,
            (id.palabra(28) & 0xFF) as u8, (id.palabra(28) >> 8) as u8,
        ];
        assert_eq!(&crudo, b"IKGN", "asi se veria el modelo si nadie intercambiara");
    }

    #[test]
    fn la_cadena_recorta_el_relleno_de_la_derecha() {
        let s = sector(&[(27, 0x4142), (28, 0x2020)]); // "AB" + dos espacios
        let id = Identify::nuevo(&s).unwrap();
        let mut buf = [0u8; 8];
        let n = id.cadena(27, 29, &mut buf);
        assert_eq!(&buf[..n], b"AB");
    }

    #[test]
    fn la_cadena_no_desborda_una_salida_pequena() {
        let s = sector(&[(27, 0x4142), (28, 0x4344)]);
        let id = Identify::nuevo(&s).unwrap();
        let mut buf = [0u8; 2];
        let n = id.cadena(27, 29, &mut buf);
        assert_eq!(n, 2);
        assert_eq!(&buf[..n], b"AB");
    }
}
