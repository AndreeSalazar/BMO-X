//! **LO QUE NO SE VE, NO SE PINTA** -- el juez de si una ventana se ve (R-APP8).
//!
//! generacion: hijo -- relaciona tres hechos que le dan de fuera (la caja
//! visible, si esta minimizada, si la pantalla esta prestada) y devuelve un
//! veredicto. **No sabe que hara la app con el**: saltarse el dibujo es cosa
//! suya, al otro lado del proceso.
//!
//! ## Por que el juez vive aqui y no en el compositor
//!
//! Por lo mismo que la resta: el compositor es un `.bex` y ahi no corre un
//! test (L7b). Y por una razon mas, que es de este fichero: la caja con la que
//! se decide **es la misma `Visible` con la que se golpea**. Decidir con otra
//! seria el `[riesgo] ESPEJO` que esta casa caza -- una ventana que se puede
//! pulsar y "no se ve", o que se ve y no se pinta.
//!
//! ## ** HACIA QUE LADO SE EQUIVOCA, y es lo que lo hace seguro
//!
//! Los dos errores posibles no cuestan lo mismo:
//!
//! ```text
//!    decir "se ve" de mas     la app pinta para nadie: se gastan vatios
//!    decir "no se ve" de mas  el usuario mira un juego CONGELADO
//! ```
//!
//! El segundo es un fallo que se ve; el primero, un gasto que no. Asi que
//! **ante la duda, se ve**: un solo pixel a la vista es verse, y todo lo que
//! este juez no sabe decidir --una ventana tapada por otra, por ejemplo-- se
//! contesta `SeVe`. Ahorrar menos es aceptable; congelar lo que se mira, no.
//!
//! ## Los numeros son un CONTRATO con C
//!
//! Viajan en el byte 2 del estado del buzon, y los lee `<bmo/superficie.h>`
//! (`BMO_SUP_VISTA_*`). La prueba `los_numeros_son_los_de_rex` lee esa
//! cabecera: si alguien cambia uno de los dos lados, el banco lo dice.

use super::Visible;

/// **Por que una superficie NO se ve**, o que si. Es el byte que el DIRECTOR
/// deja en el buzon; `0` = se ve, y es tambien lo que lee una app vieja o lo
/// que escribe un DIRECTOR viejo. El cero es el comportamiento de siempre.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vista {
    /// Se ve, aunque sea un pixel. Pinta.
    SeVe = 0,
    /// Esta en la barra. El DIRECTOR ya no la compone.
    Minimizada = 1,
    /// Arrastrada fuera del lienzo, o encogida hasta no dejar interior.
    FueraDePantalla = 2,
    /// El DIRECTOR presto la pantalla entera a otro programa.
    PantallaPrestada = 3,
    /// Otra ventana esta a PANTALLA COMPLETA y la tapa entera (2026-09-11).
    ///
    /// ** Es el unico solape que el DIRECTOR sabe calcular sin geometria, y
    /// es el que importa: un juego llenando el panel con tres ventanas
    /// detras dibujando para nadie. El solape general --una ventana encima
    /// de otra-- sigue contestando `SeVe`.
    Tapada = 4,
}

/// **El veredicto.** El orden de las preguntas va de la causa mas ancha a la
/// mas estrecha, y solo importa para el MOTIVO: las cuatro dicen que no se ve.
///
/// ** `minimizada` gana a `tapada` a proposito: las dos son ciertas a la vez
/// cuando hay un juego a pantalla completa y una ventana en la barra, y la que
/// explica lo que pasa es la que hizo el dueno.
pub fn vista(v: Visible, minimizada: bool, prestada: bool, tapada: bool) -> Vista {
    if prestada {
        return Vista::PantallaPrestada;
    }
    if minimizada {
        return Vista::Minimizada;
    }
    if tapada {
        return Vista::Tapada;
    }
    if v.ancho == 0 || v.alto == 0 {
        return Vista::FueraDePantalla;
    }
    Vista::SeVe
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn caja() -> Visible {
        Visible { x: 100, y: 50, ancho: 960, alto: 600 }
    }

    #[test]
    fn una_ventana_normal_se_ve() {
        assert_eq!(vista(caja(), false, false, false), Vista::SeVe);
    }

    #[test]
    fn minimizada_no_se_ve() {
        assert_eq!(vista(caja(), true, false, false), Vista::Minimizada);
    }

    #[test]
    fn sin_ancho_o_sin_alto_no_se_ve() {
        let mut v = caja();
        v.ancho = 0;
        assert_eq!(vista(v, false, false, false), Vista::FueraDePantalla);
        let mut v = caja();
        v.alto = 0;
        assert_eq!(vista(v, false, false, false), Vista::FueraDePantalla);
    }

    /// *** La que hace seguro al juez: casi fuera NO es fuera. Un pixel a la
    /// vista y el usuario puede estar mirandolo -- congelarlo seria el error
    /// caro de los dos.
    #[test]
    fn un_solo_pixel_a_la_vista_es_verse() {
        let v = Visible { x: 1919, y: 1079, ancho: 1, alto: 1 };
        assert_eq!(vista(v, false, false, false), Vista::SeVe);
    }

    /// Con la pantalla prestada no se ve NINGUNA, ni las que estaban delante.
    #[test]
    fn la_pantalla_prestada_gana_a_todo() {
        assert_eq!(vista(caja(), false, true, false), Vista::PantallaPrestada);
        assert_eq!(vista(caja(), true, true, false), Vista::PantallaPrestada);
    }

    /// Otra ventana a pantalla completa tapa a esta, aunque su caja este
    /// entera dentro del panel: la geometria no lo dice, el estado si.
    #[test]
    fn tapada_por_una_a_pantalla_completa() {
        assert_eq!(vista(caja(), false, false, true), Vista::Tapada);
    }

    /// Y el orden de los motivos: lo que hizo el dueno gana a lo que hizo
    /// otra ventana, y la pantalla prestada gana a todo.
    #[test]
    fn minimizada_gana_a_tapada_y_prestada_gana_a_las_dos() {
        assert_eq!(vista(caja(), true, false, true), Vista::Minimizada);
        assert_eq!(vista(caja(), true, true, true), Vista::PantallaPrestada);
    }

    /// El cero es lo que ya escribia el DIRECTOR en ese byte antes de que
    /// existiera: una app vieja y un DIRECTOR viejo siguen diciendo "se ve".
    #[test]
    fn el_cero_es_verse_y_eso_es_la_compatibilidad() {
        assert_eq!(Vista::SeVe as u8, 0);
    }

    /// *** EL ESPEJO CON C, comprobado y no prometido. `<bmo/superficie.h>`
    /// escribe los mismos cuatro numeros; si uno de los dos lados cambia sin
    /// el otro, una app leeria "minimizada" donde el DIRECTOR dijo "fuera".
    #[test]
    fn los_numeros_son_los_de_rex() {
        let h = include_str!("../../../../toolchain/forge/sem-asm/tables/bmo/superficie/amarilla.h");
        let valor = |nombre: &str| -> u8 {
            let linea = h
                .lines()
                .find(|l| l.starts_with("#define ") && l.split_whitespace().nth(1) == Some(nombre))
                .unwrap_or_else(|| panic!("{nombre} no esta en superficie/amarilla.h"));
            linea.split_whitespace().nth(2).unwrap().parse().unwrap()
        };
        assert_eq!(valor("BMO_SUP_VISTA_SE_VE"), Vista::SeVe as u8);
        assert_eq!(valor("BMO_SUP_VISTA_MINIMIZADA"), Vista::Minimizada as u8);
        assert_eq!(valor("BMO_SUP_VISTA_FUERA"), Vista::FueraDePantalla as u8);
        assert_eq!(valor("BMO_SUP_VISTA_PRESTADA"), Vista::PantallaPrestada as u8);
        assert_eq!(valor("BMO_SUP_VISTA_TAPADA"), Vista::Tapada as u8);
    }
}
