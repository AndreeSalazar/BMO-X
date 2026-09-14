//! **EL PASE** -- las preguntas que se hacen UNA vez, en orden, antes de abrir.
//!
//! [carril]  VERDE     un veredicto sobre hechos que ya junto el kernel
//! [cuesta]  NADA      corre cuando alguien pide `RED_OP_ABRIR`
//! [riesgo]  UNICO     es la unica puerta de la red: una pregunta de menos aqui no
//!                     la hace nadie despues
//!
//! # El orden, y por que la autoridad va PRIMERO
//!
//! ```text
//!    1 autoridad    quien pide. Un proceso sin ella no se entera ni de si
//!                   hay tarjeta: el estado del hardware es informacion
//!    2 tarjeta      hay algo que este kernel sepa programar
//!    3 enlace       hay cable AHORA (se lee del aparato, no del arranque)
//!    4 receptor     el anillo de entrada esta armado
//!    5 ocupado      hay otro pase vivo. UNO a la vez: dos duenos del cable
//!                   no se pueden vigilar por separado
//!    6 cerrandose   el anterior aun no termino de soltar
//!    7 lo pedido    cero ms o cero tramas no es un pase, es un error
//! ```
//!
//! # *** Y la firma, donde esta
//!
//! Antes. Un `.bex` que no pasa el ancla no llega a proceso (`task/admitir.rs`,
//! el gate de autoria), asi que todo el que pregunta aqui ya fue juzgado por
//! QUIEN lo escribio. Esto juzga QUIEN LO LANZO: la autoridad `RED` la fija
//! Ring 0 al crear el proceso y **no se delega** (`task/autoridad.rs`).
//!
//! La tercera firma es el handle: lleva generacion, y un pase revocado no
//! vuelve a resolver aunque alguien guarde el numero.

/// El plazo mas largo de una vez: diez minutos. Igual que `bmo_net::tx::DURACION_MAX`.
pub const MS_MAX: u64 = 10 * 60 * 1000;
/// Las tramas mas de una vez. Igual que `bmo_net::tx::CUPO_MAX`.
pub const CUPO_MAX: u32 = 10_000;

/// Lo que el kernel sabe cuando llega la peticion. Lo junta el; esto solo juzga.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Hechos {
    pub autoridad: bool,
    pub hay_tarjeta: bool,
    pub enlace: bool,
    pub receptor: bool,
    pub ocupado: bool,
    pub cerrandose: bool,
    pub pide_ms: u64,
    pub pide_cupo: u32,
}

/// **Por que no.** Viaja en las banderas de `BmoStatus::negado`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum NoPase {
    SinAutoridad = 1,
    SinTarjeta = 2,
    SinEnlace = 3,
    SinReceptor = 4,
    Ocupado = 5,
    Cerrandose = 6,
    PideNada = 7,
    // ** Los tres de abajo NO los devuelve `juzgar`: los pone el kernel DESPUES
    // del juicio, cuando el si ya estaba dado y lo que falla es cumplirlo. Viven
    // en esta lista porque el que pide no distingue de donde salio el no.
    /// No hubo marcos para el buzon, o no se pudo lanzar el latido.
    SinMemoria = 8,
    /// El transmisor de la tarjeta no se pudo armar. CABINA dice por que.
    SalidaNoArma = 9,
    /// El buzon no se pudo mapear en el proceso.
    NoSeMapea = 10,
}

impl NoPase {
    pub const fn codigo(self) -> u32 {
        self as u32
    }

    pub fn desde_codigo(c: u32) -> Option<NoPase> {
        Some(match c {
            1 => NoPase::SinAutoridad,
            2 => NoPase::SinTarjeta,
            3 => NoPase::SinEnlace,
            4 => NoPase::SinReceptor,
            5 => NoPase::Ocupado,
            6 => NoPase::Cerrandose,
            7 => NoPase::PideNada,
            8 => NoPase::SinMemoria,
            9 => NoPase::SalidaNoArma,
            10 => NoPase::NoSeMapea,
            _ => return None,
        })
    }

    pub fn texto(self) -> &'static str {
        match self {
            NoPase::SinAutoridad => "este proceso no tiene autoridad de red (no lo lanzo Ring 0)",
            NoPase::SinTarjeta => "no hay tarjeta que este kernel sepa programar",
            NoPase::SinEnlace => "el enlace esta ABAJO: enchufa el cable",
            NoPase::SinReceptor => "el receptor no esta armado: `red rx` primero",
            NoPase::Ocupado => "otro proceso tiene el pase abierto",
            NoPase::Cerrandose => "el pase anterior todavia se esta cerrando",
            NoPase::PideNada => "cero milisegundos o cero tramas no es un pase",
            NoPase::SinMemoria => "no hubo memoria para el buzon o para el latido",
            NoPase::SalidaNoArma => "el transmisor de la tarjeta no se pudo armar (F11 dice por que)",
            NoPase::NoSeMapea => "el buzon no se pudo mapear en este proceso",
        }
    }
}

/// Lo que de verdad se concede, ya recortado a los topes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Concesion {
    pub ms: u64,
    pub cupo: u32,
}

/// **El juicio.** Una pregunta detras de otra; la primera que falla, contesta.
pub fn juzgar(h: &Hechos) -> Result<Concesion, NoPase> {
    if !h.autoridad {
        return Err(NoPase::SinAutoridad);
    }
    if !h.hay_tarjeta {
        return Err(NoPase::SinTarjeta);
    }
    if !h.enlace {
        return Err(NoPase::SinEnlace);
    }
    if !h.receptor {
        return Err(NoPase::SinReceptor);
    }
    if h.ocupado {
        return Err(NoPase::Ocupado);
    }
    if h.cerrandose {
        return Err(NoPase::Cerrandose);
    }
    if h.pide_ms == 0 || h.pide_cupo == 0 {
        return Err(NoPase::PideNada);
    }
    Ok(Concesion { ms: h.pide_ms.min(MS_MAX), cupo: h.pide_cupo.min(CUPO_MAX) })
}

/// **Plazo y cupo en UN numero**, porque por la puerta cabe uno: el plazo en
/// los 32 bits bajos (10 minutos son 600.000 ms) y el cupo en los altos.
pub const fn empaquetar(ms: u64, cupo: u32) -> u64 {
    let ms = if ms > u32::MAX as u64 { u32::MAX as u64 } else { ms };
    ms | ((cupo as u64) << 32)
}

pub const fn desempaquetar(v: u64) -> (u64, u32) {
    (v & 0xFFFF_FFFF, (v >> 32) as u32)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn todo_bien() -> Hechos {
        Hechos {
            autoridad: true,
            hay_tarjeta: true,
            enlace: true,
            receptor: true,
            ocupado: false,
            cerrandose: false,
            pide_ms: 30_000,
            pide_cupo: 100,
        }
    }

    #[test]
    fn con_todo_en_su_sitio_concede_lo_pedido() {
        assert_eq!(juzgar(&todo_bien()), Ok(Concesion { ms: 30_000, cupo: 100 }));
    }

    #[test]
    fn recorta_a_los_topes() {
        let mut h = todo_bien();
        h.pide_ms = u64::MAX;
        h.pide_cupo = u32::MAX;
        assert_eq!(juzgar(&h), Ok(Concesion { ms: MS_MAX, cupo: CUPO_MAX }));
    }

    /// *** Sin autoridad no se cuenta NADA del hardware, aunque todo falle.
    #[test]
    fn la_autoridad_se_pregunta_antes_que_el_hardware() {
        let h = Hechos { pide_ms: 1, pide_cupo: 1, ..Hechos::default() };
        assert_eq!(juzgar(&h), Err(NoPase::SinAutoridad));
    }

    #[test]
    fn cada_no_por_su_nombre_y_en_su_orden() {
        let casos: [(fn(&mut Hechos), NoPase); 7] = [
            (|h| h.autoridad = false, NoPase::SinAutoridad),
            (|h| h.hay_tarjeta = false, NoPase::SinTarjeta),
            (|h| h.enlace = false, NoPase::SinEnlace),
            (|h| h.receptor = false, NoPase::SinReceptor),
            (|h| h.ocupado = true, NoPase::Ocupado),
            (|h| h.cerrandose = true, NoPase::Cerrandose),
            (|h| h.pide_cupo = 0, NoPase::PideNada),
        ];
        for (romper, esperado) in casos {
            let mut h = todo_bien();
            romper(&mut h);
            assert_eq!(juzgar(&h), Err(esperado));
            assert_eq!(NoPase::desde_codigo(esperado.codigo()), Some(esperado));
        }
        let mut h = todo_bien();
        h.enlace = false;
        h.ocupado = true;
        assert_eq!(juzgar(&h), Err(NoPase::SinEnlace), "el primero que falla contesta");
    }

    #[test]
    fn el_paquete_va_y_vuelve() {
        assert_eq!(desempaquetar(empaquetar(600_000, 10_000)), (600_000, 10_000));
        assert_eq!(desempaquetar(empaquetar(u64::MAX, 7)), (u32::MAX as u64, 7));
    }
}
