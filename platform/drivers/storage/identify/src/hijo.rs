//! **EL HIJO** -- relaciona dos hechos del padre. No sabe que significa la
//! relacion.
//!
//! [eje]     CORRECCION
//! [exige]   R-DISCO7 (medio, ranura y aparato son tres ejes distintos)
//!
//! # Que hace una generacion que solo resta
//!
//! El padre tiene campos y ninguno mira a otro. Pero **casi todo lo interesante
//! de un disco esta en el par**, no en el campo:
//!
//! ```text
//!    soportado 6 Gb/s   y   negociado 3 Gb/s      -> va por debajo de si mismo
//!    no rota            y   sin TRIM              -> un SSD que no puede
//!                                                    enterarse de lo que sobra
//!    32 ranuras         y   BMO usa 1             -> el aparato espera
//! ```
//!
//! Ninguna de esas tres frases cabe en un campo, y ninguna de las tres es un
//! veredicto: **son restas**. Que ir a 3 Gb/s sea un problema depende de si algo
//! satura el cable, y eso lo decide el nieto -- que vive fuera y se puede
//! probar.
//!
//! ## Por que la separacion vale la pena aqui (L7, punto 1)
//!
//! Porque permite disenar el experimento. `ranuras_ociosas` es exactamente la
//! cifra que hay que mover para saber cuanto cuesta la ranura 0, y esta aislada
//! de todo lo demas: **entre medir con 1 y medir con 32 cambia UNA SOLA COSA**.
//! Si el contraste estuviera mezclado con el veredicto, la resta arrastraria
//! tambien el criterio, y dos tandas darian el mismo numero y la misma duda.

use crate::padre::{Cola, Enlace, Geometria, Medio, Trim};

/// Las relaciones entre dos hechos. Hechos tambien, no opiniones.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Contraste {
    /// El enlace negocio por debajo de lo que el disco declara saber hacer.
    ///
    /// `false` tambien cuando la negociada es 0 -- si el disco no dice a que
    /// velocidad va, **no se puede afirmar que vaya por debajo**. Una ausencia
    /// no es una desigualdad.
    pub enlace_por_debajo: bool,

    /// Generacion que se pierde: `mejor_soportada - negociada`. 0 si no aplica.
    pub enlace_escalones: u8,

    /// El medio es de estado solido **y** el disco no admite TRIM.
    ///
    /// Es la pareja de R-DISCO10: el recolector del sistema de ficheros libera
    /// bloques y el aparato no puede enterarse.
    pub solido_sin_trim: bool,

    /// Ranuras que el disco admite y el sistema no usa: `profundidad - usadas`.
    ///
    /// Se le pasa `usadas` porque **el hijo no sabe cuantas usa el driver**: eso
    /// es un hecho del sistema, no del aparato, y mezclarlos seria decidir aqui
    /// algo que se decide en otro sitio.
    pub ranuras_ociosas: u8,

    /// El disco declara NCQ y sin embargo dice una sola ranura. Se contradice.
    pub ncq_sin_cola: bool,

    /// La geometria dice varios sectores logicos por fisico **y** el LBA 0 no
    /// cae en frontera. Toda escritura de un fisico completo va a caballo.
    pub desalineado: bool,
}

impl Contraste {
    /// `usadas`: cuantas ranuras de comando usa de verdad el driver. Hoy, 1.
    pub fn de(
        medio: Medio,
        cola: Cola,
        enlace: Enlace,
        geom: Geometria,
        trim: Trim,
        usadas: u8,
    ) -> Contraste {
        let mejor = enlace.mejor_soportada();
        // ** La negociada a 0 no es "va lentisimo": es "no lo dice".
        let por_debajo = enlace.negociada != 0 && mejor != 0 && enlace.negociada < mejor;

        Contraste {
            enlace_por_debajo: por_debajo,
            enlace_escalones: if por_debajo { mejor - enlace.negociada } else { 0 },
            solido_sin_trim: medio.es_estado_solido() && !trim.soportado,
            ranuras_ociosas: cola.profundidad.saturating_sub(usadas),
            ncq_sin_cola: cola.ncq && cola.profundidad <= 1,
            desalineado: geom.desplazamiento_valido && geom.desplazamiento != 0,
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn c(medio: Medio, cola: Cola, enlace: Enlace, geom: Geometria, trim: bool, usadas: u8)
        -> Contraste
    {
        // `bloques_max` no entra en ningun contraste: dice cuanto cabe en una
        // orden, no si el disco esta bien portado. Se pone el minimo que ACS-3
        // garantiza para que la casilla hable de lo que esta probando.
        Contraste::de(medio, cola, enlace, geom, Trim { soportado: trim, bloques_max: 1 }, usadas)
    }

    const GEOM_LLANA: Geometria = Geometria {
        exponente: 0, valida: true, desplazamiento: 0, desplazamiento_valido: false,
    };

    #[test]
    fn el_enlace_va_por_debajo_y_dice_cuantos_escalones() {
        let e = Enlace { soportadas: 0b111, negociada: 1 };
        let r = c(Medio::NoRota, Cola { profundidad: 32, ncq: true }, e, GEOM_LLANA, true, 1);
        assert!(r.enlace_por_debajo);
        assert_eq!(r.enlace_escalones, 2, "de Gen3 a Gen1 son dos escalones");
    }

    /// ** La que separa una ausencia de una desigualdad.
    #[test]
    fn no_decir_la_velocidad_no_es_ir_por_debajo() {
        let e = Enlace { soportadas: 0b111, negociada: 0 };
        let r = c(Medio::NoRota, Cola { profundidad: 32, ncq: true }, e, GEOM_LLANA, true, 1);
        assert!(!r.enlace_por_debajo);
        assert_eq!(r.enlace_escalones, 0);
    }

    #[test]
    fn al_maximo_no_hay_nada_que_restar() {
        let e = Enlace { soportadas: 0b111, negociada: 3 };
        let r = c(Medio::NoRota, Cola { profundidad: 32, ncq: true }, e, GEOM_LLANA, true, 1);
        assert!(!r.enlace_por_debajo);
    }

    /// La pareja de R-DISCO10, y su contraria: un HDD sin TRIM **no** es el
    /// mismo caso -- ahi TRIM no hace falta.
    #[test]
    fn solido_sin_trim_solo_cuando_el_medio_es_solido() {
        let e = Enlace { soportadas: 0b111, negociada: 3 };
        let cola = Cola { profundidad: 32, ncq: true };
        let ssd = c(Medio::NoRota, cola, e, GEOM_LLANA, false, 1);
        assert!(ssd.solido_sin_trim);

        let hdd = c(Medio::Rota { rpm: 7200 }, cola, e, GEOM_LLANA, false, 1);
        assert!(!hdd.solido_sin_trim, "un plato que gira no necesita TRIM");

        let mudo = c(Medio::NoContesta, cola, e, GEOM_LLANA, false, 1);
        assert!(!mudo.solido_sin_trim, "sin saber el medio no se afirma nada");
    }

    /// El numero de esta maquina: 32 declaradas, 1 usada.
    #[test]
    fn las_ranuras_ociosas_son_la_resta() {
        let e = Enlace { soportadas: 0b111, negociada: 3 };
        let r = c(Medio::NoRota, Cola { profundidad: 32, ncq: true }, e, GEOM_LLANA, true, 1);
        assert_eq!(r.ranuras_ociosas, 31);
    }

    #[test]
    fn usar_todas_no_deja_ociosas_y_no_se_va_por_debajo_de_cero() {
        let e = Enlace { soportadas: 0b111, negociada: 3 };
        let cola = Cola { profundidad: 32, ncq: true };
        assert_eq!(c(Medio::NoRota, cola, e, GEOM_LLANA, true, 32).ranuras_ociosas, 0);
        assert_eq!(c(Medio::NoRota, cola, e, GEOM_LLANA, true, 40).ranuras_ociosas, 0);
    }

    #[test]
    fn un_disco_que_se_contradice_se_nota() {
        let e = Enlace { soportadas: 0b111, negociada: 3 };
        let r = c(Medio::NoRota, Cola { profundidad: 1, ncq: true }, e, GEOM_LLANA, true, 1);
        assert!(r.ncq_sin_cola);
    }

    #[test]
    fn el_desalineado_pide_que_la_209_fuera_valida() {
        let e = Enlace { soportadas: 0b111, negociada: 3 };
        let cola = Cola { profundidad: 32, ncq: true };

        let malo = Geometria {
            exponente: 3, valida: true, desplazamiento: 1, desplazamiento_valido: true,
        };
        assert!(c(Medio::NoRota, cola, e, malo, true, 1).desalineado);

        // Mismo desplazamiento, pero el disco no lo declaro valido: no se afirma.
        let dudoso = Geometria { desplazamiento_valido: false, ..malo };
        assert!(!c(Medio::NoRota, cola, e, dudoso, true, 1).desalineado);
    }
}
