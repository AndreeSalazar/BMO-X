//! `marco` -- donde cae cada valor dentro de una funcion.
//!
//! ## Lo que decide, y lo que NO
//!
//! La IR habla de `Local(3)` y `Temporal(7)`: **indices sin sitio**. Aqui se
//! convierten en desplazamientos dentro del marco, y para eso hace falta saber
//! el ancho de una palabra -- que es justo lo que el frontend tiene prohibido
//! saber.
//!
//! Por eso este reparto vive en el crate de la maquina y no en el otro lado de
//! la frontera.
//!
//! ## OJO: Lo que hoy es simple a proposito
//!
//! **Todo va a la pila**: cada local y cada temporal tienen su ranura. Es lo
//! que hace BMO C, y es exactamente el techo del que habla la seccion 13.6 del
//! maestro -- **2-4x por debajo de lo que se puede**.
//!
//! Se hace asi ahora porque el orden esta escrito y no se cambia: *primero
//! codigo CORRECTO aunque sea lento*. Un asignador de registros sobre un emisor
//! que todavia no se sabe si emite bien es dos bugs mezclados.
//!
//! ** Pero la IR ya trae los temporales, que es lo unico que el asignador
//! necesita. Cuando llegue F3, este fichero es el que cambia -- y solo este.

use bmo_inti_front::ir::{FuncionIr, Local, Temporal};

/// El ancho de una palabra en esta maquina.
///
/// Sale de `arch/x86_64/inti.toml` cuando el compilador corre de verdad; aqui
/// hay una constante porque este crate **ES** el de esa maquina. Es la
/// diferencia entre "el frontend no puede saberlo" y "el emisor de x86-64 lo
/// sabe por definicion".
pub const PALABRA: i32 = 8;

/// Donde vive cada cosa mientras corre una funcion.
#[derive(Debug, Clone)]
pub struct Marco {
    locales: u32,
    temporales: u32,
}

impl Marco {
    pub fn de(f: &FuncionIr) -> Self {
        Self {
            locales: f.locales,
            temporales: f.temporales,
        }
    }

    /// Cuantos bytes hay que reservar, redondeado a 16.
    ///
    /// La alineacion de 16 no es adorno: la ABI la exige antes de una llamada,
    /// y saltarsela da un fallo que aparece **dentro de la funcion llamada**,
    /// que es el peor sitio donde puede aparecer un fallo.
    pub fn tamano(&self) -> i32 {
        let bruto = (self.locales + self.temporales) as i32 * PALABRA;
        (bruto + 15) & !15
    }

    /// El desplazamiento de una local desde `rbp`. Negativo: el marco crece
    /// hacia abajo, que es lo que dice `la_pila_crece` en la tabla.
    pub fn local(&self, l: Local) -> i32 {
        -((l.0 as i32 + 1) * PALABRA)
    }

    /// Y el de un temporal, detras de las locales.
    pub fn temporal(&self, t: Temporal) -> i32 {
        -((self.locales as i32 + t.0 as i32 + 1) * PALABRA)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use bmo_inti_front::ir::{FuncionIr, Instr};

    fn funcion(locales: u32, temporales: u32) -> FuncionIr {
        FuncionIr {
            nombre: "f".into(),
            parametros: 0,
            locales,
            temporales,
            instrucciones: Vec::<Instr>::new(),
        }
    }

    #[test]
    fn cada_local_tiene_su_sitio_y_no_se_pisan() {
        let m = Marco::de(&funcion(3, 0));
        assert_eq!(m.local(Local(0)), -8);
        assert_eq!(m.local(Local(1)), -16);
        assert_eq!(m.local(Local(2)), -24);
    }

    /// Los temporales van DETRAS de las locales. Si empezaran en el mismo
    /// sitio, un temporal pisaria un parametro -- y eso da un programa que
    /// funciona hasta que la expresion se complica.
    #[test]
    fn los_temporales_no_pisan_a_las_locales() {
        let m = Marco::de(&funcion(2, 2));
        assert_eq!(m.local(Local(1)), -16);
        assert_eq!(m.temporal(Temporal(0)), -24);
        assert_eq!(m.temporal(Temporal(1)), -32);
    }

    /// La ABI exige 16 antes de una llamada, y saltarsela da un fallo que
    /// aparece DENTRO de la funcion llamada.
    #[test]
    fn el_marco_se_alinea_a_dieciseis() {
        assert_eq!(Marco::de(&funcion(1, 0)).tamano(), 16);
        assert_eq!(Marco::de(&funcion(2, 0)).tamano(), 16);
        assert_eq!(Marco::de(&funcion(3, 0)).tamano(), 32);
        assert_eq!(Marco::de(&funcion(0, 0)).tamano(), 0);
    }
}
