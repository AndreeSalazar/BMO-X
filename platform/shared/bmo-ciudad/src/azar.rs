//! **EL AZAR** -- xorshift de 64 bits, y nada mas.
//!
//! Hace falta uno porque una skyline con todas las torres iguales no es una
//! skyline. Y **no puede ser el del sistema**: tiene que dar la misma ciudad en
//! cada arranque. Un fondo que cambia solo cada vez que enciendes es un fondo
//! que no puedes usar para notar que algo cambio.
//!
//! Xorshift y no algo mejor porque aqui no se protege nada: son tres
//! desplazamientos, cabe en diez lineas, y sus numeros no se parecen entre si --
//! que es todo lo que hace falta para repartir alturas.

pub struct Azar(u64);

impl Azar {
    pub fn nuevo(semilla: u64) -> Self {
        // [!] El cero es PUNTO FIJO de xorshift: se quedaria en cero para
        // siempre y la ciudad saldria con todas las torres identicas -- que es
        // exactamente el sintoma que este modulo existe para evitar, y saldria
        // solo con la semilla mas facil de pasar por accidente.
        Azar(if semilla == 0 { 0x9E37_79B9_7F4A_7C15 } else { semilla })
    }

    pub fn siguiente(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Un numero en `[desde, hasta]`, los dos incluidos.
    pub fn entre(&mut self, desde: i32, hasta: i32) -> i32 {
        if hasta <= desde {
            return desde;
        }
        let rango = (hasta - desde + 1) as u64;
        desde + (self.siguiente() % rango) as i32
    }
}

/// Un revoltijo determinista de dos coordenadas.
///
/// Da el patron de ventanas, y **tiene que salir de la POSICION** y no de un
/// generador con estado: asi la misma ventana esta siempre igual aunque la torre
/// se redibuje treinta veces por segundo. Con un `Azar` corriente, la ciudad
/// parpadearia entera en cada fotograma.
pub fn mezclador(x: u64, y: u64) -> u64 {
    let mut h = x.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ y.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    h
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_semilla_cero_no_se_queda_clavada() {
        let mut a = Azar::nuevo(0);
        let x = a.siguiente();
        let y = a.siguiente();
        assert_ne!(x, 0);
        assert_ne!(x, y);
    }

    #[test]
    fn entre_respeta_los_dos_extremos() {
        let mut a = Azar::nuevo(7);
        for _ in 0..200 {
            let v = a.entre(3, 9);
            assert!((3..=9).contains(&v), "salio {}", v);
        }
    }

    /// Un rango de un solo valor no puede dividir por cero ni salirse.
    #[test]
    fn un_rango_degenerado_devuelve_el_valor() {
        let mut a = Azar::nuevo(1);
        assert_eq!(a.entre(5, 5), 5);
        assert_eq!(a.entre(9, 2), 9);
    }

    /// La misma posicion da siempre la misma ventana: es lo que impide que la
    /// ciudad parpadee entre fotogramas.
    #[test]
    fn el_mezclador_es_estable_para_la_misma_posicion() {
        assert_eq!(mezclador(31, 77), mezclador(31, 77));
        assert_ne!(mezclador(31, 77), mezclador(32, 77));
    }
}
