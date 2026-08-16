//! **El barrido**: comparar lo que el driver CREE con lo que dicen los puertos.
//!
//! === Por que existe este modulo ===
//!
//! Hasta ahora todo el hot-plug colgaba de una sola cosa: **que llegara el
//! aviso**. Enchufar avisa, desenchufar avisa, y con eso se movia el estado. El
//! problema de un sistema asi no es que los avisos fallen poco: es que cuando
//! falla UNO, no hay nada que lo repare. La maquina se queda creyendo algo que el
//! hardware desmiente desde hace rato, y **se queda asi hasta el reinicio**.
//!
//! Esa es la forma exacta del fallo que reporto el dueno: *"es como que al
//! teclado se le olvido, o otras veces mouse y teclado se olvido"*, con la foto de
//! CABINA repitiendo `puerto: ENCHUFADO, nada que adoptar` y, al lado, la linea
//! que delata la mentira: `creo tener teclado:raton =257` -- o sea `0x101`, o sea
//! *"tengo los dos"*, mientras el dueno esta mirando un teclado que no escribe.
//!
//! Con la cola de `bmo_xhci::avisos` se pierden muchos menos avisos. Pero
//! "muchos menos" no es "las puertas siempre abiertas", que es lo que se pidio:
//!
//! > *"mi Kernel tiene que tener siempre abierto las puertas para facilitar"*
//!
//! Y para eso hace falta algo que no dependa de haberse enterado. Un aviso es un
//! ATAJO; **la verdad esta en PORTSC**, y se puede leer cuando se quiera.
//!
//! === Las tres reparaciones, y por que son tres ===
//!
//! 1. **El fantasma.** Un puerto vacio del que yo creo tener un aparato: me
//!    perdi su desconexion. Se suelta. Sin esto, `completo()` miente y el
//!    adoptador se va por su primera linea para siempre.
//! 2. **La puerta cerrada.** Un puerto vacio con intentos gastados. Los intentos
//!    solo se devolvian al recibir el aviso de desconexion -- que es justo el que
//!    se pierde. Un puerto vacio **no tiene nada que reintentar**, asi que sus
//!    tres oportunidades vuelven enteras: lo que se enchufe despues es otro
//!    aparato y merece las suyas.
//! 3. **El que llego mientras nadie miraba.** Algo conectado en un puerto que no
//!    es mio, faltandome un aparato: se intenta.
//!
//! === Y las dos reglas que NO se tocan ===
//!
//! Este modulo puede REPARAR, pero no puede romper lo que ya costo caro. Se
//! respetan enteras las dos reglas de [`crate::puertos`]:
//!
//! * **A un puerto tomado no se le vuelve a tocar**, ni aunque falte algo.
//!   Resetear el puerto de un teclado que esta escribiendo fue el bug del
//!   2026-07-31, y un barrido que corre solo lo repetiria 250 veces por segundo.
//! * **Los intentos siguen siendo finitos** mientras haya algo conectado. Solo
//!   un puerto VACIO los recupera, y un puerto vacio no se enumera.
//!
//! Vive aparte porque es una DECISION, no hardware: entra el estado, sale la
//! accion. Se prueba sin un xHC delante, que es la unica forma de estar seguro de
//! que un barrido automatico no se come al teclado.

use crate::puertos::MAX_INTENTOS;

/// Lo que se sabe de UN puerto en el instante de barrer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vista {
    /// `PORTSC.CCS`: hay algo enchufado AHORA. Es el unico dato que viene del
    /// hardware; los otros tres son lo que el driver cree.
    pub hay_dispositivo: bool,
    /// Alguno de mis aparatos dice haber salido de este puerto.
    pub es_mio: bool,
    /// El libro de puertos lo da por tomado (de aqui salio algo que funciona).
    pub tomado: bool,
    /// Intentos de adopcion gastados en este puerto.
    pub intentos: u8,
    /// Me falta algun aparato? (`!completo()`)
    pub falta_algo: bool,
}

/// Que hacer con un puerto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accion {
    /// Ni mirarlo. Es la respuesta correcta casi siempre.
    Nada,
    /// Es mio y ya no esta: olvidarlo. Reparacion 1.
    Soltar,
    /// Esta vacio y con intentos gastados: devolverselos. Reparacion 2.
    Reabrir,
    /// Hay algo que no es mio y me falta un aparato: intentarlo. Reparacion 3.
    Adoptar,
}

/// **La decision.** Todo el barrido es esto, una vez por puerto.
///
/// El orden de los casos importa: `Soltar` va antes que nada porque un fantasma
/// hace mentir a `falta_algo`, y decidir con un dato que se sabe falso es como se
/// llego hasta aqui.
pub fn decidir(v: &Vista) -> Accion {
    if !v.hay_dispositivo {
        // -- El puerto esta VACIO ------------------------------------------
        if v.es_mio {
            // Reparacion 1: el fantasma. Me perdi su desconexion.
            return Accion::Soltar;
        }
        if v.tomado || v.intentos > 0 {
            // Reparacion 2: la puerta cerrada. Un puerto vacio no tiene nada
            // que reintentar, asi que sus oportunidades vuelven enteras.
            return Accion::Reabrir;
        }
        return Accion::Nada;
    }

    // -- Hay algo enchufado --------------------------------------------
    //
    // ** LA REGLA QUE NO SE TOCA: un puerto tomado no se vuelve a tocar. Da
    // igual que falte algo. El puerto tomado es el del teclado que esta
    // escribiendo, y resetearlo es el bug del 2026-07-31 -- que ahora, con un
    // barrido automatico, se repetiria doscientas cincuenta veces por segundo
    // en vez de una.
    if v.es_mio || v.tomado {
        return Accion::Nada;
    }
    if !v.falta_algo {
        // No falta nada: re-enumerar por gusto mete control transfers en un
        // controlador con dos aparatos bombeando. Lo que se enchufe aqui se
        // atendera por su aviso, que es barato; el barrido no insiste.
        return Accion::Nada;
    }
    if v.intentos >= MAX_INTENTOS {
        // Los intentos siguen siendo finitos mientras haya algo conectado: sin
        // esto, un aparato que no sabemos adoptar giraria para siempre.
        return Accion::Nada;
    }
    Accion::Adoptar
}

/// Lo que hizo un barrido entero. Para CABINA: un barrido que repara algo es
/// noticia, y uno que no repara nada no debe decir ni una linea.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Resumen {
    /// Fantasmas soltados (reparacion 1).
    pub soltados: u8,
    /// Puertos vacios a los que se les devolvieron los intentos (reparacion 2).
    pub reabiertos: u8,
    /// Aparatos adoptados (reparacion 3, con exito).
    pub adoptados: u8,
    /// Intentos de adopcion que no dieron nada.
    pub fallidos: u8,
}

impl Resumen {
    /// Se reparo algo? Si no, el barrido se calla.
    pub fn hubo_algo(&self) -> bool {
        self.soltados != 0 || self.reabiertos != 0 || self.adoptados != 0 || self.fallidos != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un puerto con algo enchufado que no es mio, sin nada que me falte y sin
    /// intentos: el caso corriente. La base sobre la que se cambia un campo.
    fn vista() -> Vista {
        Vista {
            hay_dispositivo: true,
            es_mio: false,
            tomado: false,
            intentos: 0,
            falta_algo: false,
        }
    }

    /// ** LA REPARACION QUE ARREGLA EL FALLO REPORTADO: el fantasma.
    ///
    /// El puerto esta vacio y yo sigo creyendo que de ahi sale mi teclado. Eso es
    /// exactamente lo que deja `completo()` diciendo `true` para siempre, y con
    /// ello el adoptador se va por su primera linea sin tocar el bus: en CABINA,
    /// `nada que adoptar` con un `creo tener teclado:raton =257` al lado.
    #[test]
    fn un_puerto_vacio_del_que_creo_tener_un_aparato_se_suelta() {
        let v = Vista { hay_dispositivo: false, es_mio: true, tomado: true, ..vista() };
        assert_eq!(decidir(&v), Accion::Soltar);
    }

    /// ** Y LA OTRA MITAD: la puerta que se quedo cerrada.
    ///
    /// Los intentos solo se devolvian al recibir el aviso de desconexion, que es
    /// justo el que se pierde. Tres enchufes fallidos y el puerto quedaba
    /// inservible hasta el reinicio -- aunque el dueno lo desenchufara y lo
    /// volviera a enchufar, que es lo primero que hace cualquiera.
    #[test]
    fn un_puerto_vacio_con_los_intentos_gastados_los_recupera() {
        let v = Vista {
            hay_dispositivo: false,
            intentos: MAX_INTENTOS,
            ..vista()
        };
        assert_eq!(decidir(&v), Accion::Reabrir);
    }

    /// Un puerto vacio y limpio no es noticia. Si esto devolviera algo, el
    /// barrido escribiria una linea por puerto vacio, 250 veces por segundo.
    #[test]
    fn un_puerto_vacio_y_limpio_no_da_trabajo() {
        let v = Vista { hay_dispositivo: false, ..vista() };
        assert_eq!(decidir(&v), Accion::Nada);
    }

    /// La reparacion 3: algo llego mientras nadie miraba y me falta un aparato.
    #[test]
    fn algo_conectado_que_no_es_mio_se_adopta_si_falta_algo() {
        let v = Vista { falta_algo: true, ..vista() };
        assert_eq!(decidir(&v), Accion::Adoptar);
    }

    /// ** LA REGLA QUE NO SE PUEDE ROMPER, y ahora con un bucle automatico
    /// detras: **a un puerto tomado no se le toca ni aunque falte algo**.
    ///
    /// Ese puerto es el del teclado que esta escribiendo. Resetearlo fue el bug
    /// del 2026-07-31; con un barrido corriendo solo, seria ese bug 250 veces por
    /// segundo. Esta prueba es el seguro de que el barrido no puede repetirlo.
    #[test]
    fn a_un_puerto_tomado_no_se_le_toca_ni_faltando_algo() {
        let v = Vista { tomado: true, falta_algo: true, ..vista() };
        assert_eq!(decidir(&v), Accion::Nada, "aqui vive el teclado que escribe");
    }

    /// Y lo mismo por la otra puerta: el puerto DE MI APARATO, con el aparato
    /// todavia enchufado, no se re-enumera.
    #[test]
    fn el_puerto_de_mi_propio_aparato_no_se_re_enumera() {
        let v = Vista { es_mio: true, tomado: true, falta_algo: true, ..vista() };
        assert_eq!(decidir(&v), Accion::Nada);
    }

    /// Con teclado y raton puestos, el barrido no toca nada. Re-enumerar por
    /// gusto mete control transfers en un bus con dos aparatos bombeando.
    #[test]
    fn si_no_falta_nada_el_barrido_no_toca_el_bus() {
        let v = Vista { falta_algo: false, ..vista() };
        assert_eq!(decidir(&v), Accion::Nada);
    }

    /// Los intentos siguen siendo finitos MIENTRAS haya algo conectado. Un
    /// aparato que no sabemos adoptar --un pendrive, unos auriculares-- no puede
    /// hacer girar el barrido para siempre.
    #[test]
    fn un_aparato_que_no_sabemos_adoptar_no_gira_para_siempre() {
        let v = Vista { falta_algo: true, intentos: MAX_INTENTOS, ..vista() };
        assert_eq!(decidir(&v), Accion::Nada);
    }

    /// Pero se intenta lo justo: el primer fallo no puede ser el ultimo, porque
    /// un aparato puede tardar en engancharse.
    #[test]
    fn se_reintenta_mientras_queden_intentos() {
        let v = Vista { falta_algo: true, intentos: MAX_INTENTOS - 1, ..vista() };
        assert_eq!(decidir(&v), Accion::Adoptar);
    }

    /// ** EL CICLO COMPLETO, que es lo que el dueno vive con las manos.
    ///
    /// Desenchufar sin que llegue el aviso, y volver a enchufar. Antes esto
    /// terminaba en `nada que adoptar` para siempre; ahora cada paso tiene su
    /// reparacion y el teclado vuelve solo.
    #[test]
    fn el_ciclo_entero_de_un_teclado_que_se_perdio_sin_avisar() {
        // 1. Lo tengo, y esta ahi. No se toca.
        let mut tomado = true;
        let mut es_mio = true;
        let mut intentos = 1u8;
        assert_eq!(
            decidir(&Vista { hay_dispositivo: true, es_mio, tomado, intentos, falta_algo: false }),
            Accion::Nada
        );

        // 2. Lo desenchufa y el aviso se pierde. El barrido ve el puerto vacio.
        assert_eq!(
            decidir(&Vista { hay_dispositivo: false, es_mio, tomado, intentos, falta_algo: false }),
            Accion::Soltar,
            "el fantasma se va, y con el la mentira de completo()"
        );
        // `soltar_puerto` olvida el aparato Y devuelve el puerto con los
        // intentos a cero.
        es_mio = false;
        tomado = false;
        intentos = 0;

        // 3. Sigue vacio: ya no hay nada que reparar.
        assert_eq!(
            decidir(&Vista { hay_dispositivo: false, es_mio, tomado, intentos, falta_algo: true }),
            Accion::Nada
        );

        // 4. Lo vuelve a enchufar. Ahora SI falta algo, y la puerta esta abierta.
        assert_eq!(
            decidir(&Vista { hay_dispositivo: true, es_mio, tomado, intentos, falta_algo: true }),
            Accion::Adoptar,
            "y aqui es donde antes salia 'nada que adoptar'"
        );
    }

    /// Un barrido que no repara nada se calla. Una linea que sale siempre es una
    /// linea que se deja de leer.
    #[test]
    fn un_barrido_sin_trabajo_no_es_noticia() {
        assert!(!Resumen::default().hubo_algo());
        assert!(Resumen { soltados: 1, ..Default::default() }.hubo_algo());
        assert!(Resumen { fallidos: 1, ..Default::default() }.hubo_algo());
    }
}
