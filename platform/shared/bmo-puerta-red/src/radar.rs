//! **EL RADAR DE 4 MS** -- mira el pase desde fuera y decide si sigue.
//!
//! [carril]  VERDE     solo cuenta y compara: no toca memoria ni aparato
//! [cuesta]  NADA      unas restas por latido, y solo con un pase abierto
//! [riesgo]  RELOJ     ve DESPUES, cada 4 ms: una trama que el grifo dejo salir ya salio
//!
//! # RADAR, no rayos X (`docs/identidad/LA_RUTA.md`)
//!
//! No esta en el camino de la trama -- estar ahi seria la burocracia que el
//! pase acaba de quitar. Mira cada 4 ms lo que ya paso: los indices del buzon,
//! lo que el grifo nego y el plazo. Y cuando algo no cuadra no avisa: REVOCA.
//!
//! # *** Blindado agresivamente: que no tiene segunda oportunidad
//!
//! ```text
//!    un indice o un largo imposible   el proceso miente o esta roto   AL MOMENTO
//!    una MAC de origen ajena          suplantacion                    AL MOMENTO
//!    el enlace se cae                 el pase era para ESE cable      AL MOMENTO
//!    el plazo o el cupo se acaban     lo pedido, cumplido             AL MOMENTO
//!    tramas malformadas               un bug tiene 16, no mas         a la 17
//!    ritmo excedido                   una rafaga pasa; una inundacion
//!                                     son 25 latidos SEGUIDOS (100 ms)  al 25
//! ```
//!
//! [!] Lo que el radar NO para: una tarjeta escribiendo por DMA fuera de su
//! corral. Eso lo ve el titular (pisados, choques) y solo lo impide una IOMMU.

use crate::buzon::Mal;

/// Cada cuanto late el radar. Es la resolucion: lo que dure menos, no se ve.
pub const LATIDO_MS: u64 = 4;
/// Latidos SEGUIDOS con el ritmo excedido para llamarlo inundacion.
pub const GOLPES_DE_RITMO: u32 = 25;
/// Tramas malformadas que se perdonan en un pase. La siguiente revoca.
pub const MALFORMADAS_MAX: u64 = 16;

/// **Por que se cerro un pase.** Viaja en `buzon::campo::ESTADO` y en
/// `RED_OP_ESTADO`. El cero no existe: cero es "abierto".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum Motivo {
    CerradoPorElDueno = 1,
    Caducado = 2,
    CupoGastado = 3,
    IndiceImposible = 4,
    LargoImposible = 5,
    Suplantacion = 6,
    Inundacion = 7,
    Malformadas = 8,
    DuenoMurio = 9,
    EnlaceCaido = 10,
    BuzonRoto = 11,
}

impl Motivo {
    pub const fn codigo(self) -> u32 {
        self as u32
    }

    pub fn desde_codigo(c: u32) -> Option<Motivo> {
        Some(match c {
            1 => Motivo::CerradoPorElDueno,
            2 => Motivo::Caducado,
            3 => Motivo::CupoGastado,
            4 => Motivo::IndiceImposible,
            5 => Motivo::LargoImposible,
            6 => Motivo::Suplantacion,
            7 => Motivo::Inundacion,
            8 => Motivo::Malformadas,
            9 => Motivo::DuenoMurio,
            10 => Motivo::EnlaceCaido,
            11 => Motivo::BuzonRoto,
            _ => return None,
        })
    }

    pub fn texto(self) -> &'static str {
        match self {
            Motivo::CerradoPorElDueno => "lo cerro quien lo abrio",
            Motivo::Caducado => "paso el plazo concedido",
            Motivo::CupoGastado => "se gastaron las tramas concedidas",
            Motivo::IndiceImposible => "el proceso escribio un indice imposible en el buzon",
            Motivo::LargoImposible => "el proceso declaro un largo que no es trama",
            Motivo::Suplantacion => "intento salir con una MAC de origen que no es la nuestra",
            Motivo::Inundacion => "100 ms seguidos por encima del ritmo",
            Motivo::Malformadas => "mas de 16 tramas malformadas",
            Motivo::DuenoMurio => "el proceso murio",
            Motivo::EnlaceCaido => "el enlace se cayo",
            Motivo::BuzonRoto => "el buzon del kernel no tiene su forma",
        }
    }

    /// Lo decidio el dueno o el plazo, y no un fallo: no se grita en CABINA.
    pub fn es_normal(self) -> bool {
        matches!(self, Motivo::CerradoPorElDueno | Motivo::Caducado | Motivo::CupoGastado | Motivo::DuenoMurio)
    }
}

/// **Lo que paso en un latido**, contado por el kernel.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Vuelta {
    pub ahora_ms: u64,
    pub hasta_ms: u64,
    pub cupo_restante: u32,
    pub enlace: bool,
    /// Lo que devolvio el buzon este latido, si mintio.
    pub mal: Option<Mal>,
    /// Negadas del grifo ESTE latido, por clase.
    pub origen_ajeno: u32,
    pub ritmo: u32,
    pub malformadas: u32,
}

/// El estado del radar de UN pase. Nace con el pase y muere con el.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Radar {
    golpes: u32,
    malformadas: u64,
    pub latidos: u64,
}

impl Radar {
    pub const fn nuevo() -> Self {
        Self { golpes: 0, malformadas: 0, latidos: 0 }
    }

    /// **Mira un latido.** `None` = sigue abierto. El orden es de lo mas grave a
    /// lo mas normal: si mintio Y caduco, el motivo es que mintio.
    pub fn mirar(&mut self, v: &Vuelta) -> Option<Motivo> {
        self.latidos += 1;
        if let Some(m) = v.mal {
            return Some(match m {
                Mal::IndiceImposible => Motivo::IndiceImposible,
                Mal::LargoImposible => Motivo::LargoImposible,
                Mal::BuzonCorto => Motivo::BuzonRoto,
            });
        }
        if v.origen_ajeno > 0 {
            return Some(Motivo::Suplantacion);
        }
        self.malformadas = self.malformadas.saturating_add(v.malformadas as u64);
        if self.malformadas > MALFORMADAS_MAX {
            return Some(Motivo::Malformadas);
        }
        if v.ritmo > 0 {
            self.golpes += 1;
            if self.golpes >= GOLPES_DE_RITMO {
                return Some(Motivo::Inundacion);
            }
        } else {
            self.golpes = 0;
        }
        if !v.enlace {
            return Some(Motivo::EnlaceCaido);
        }
        if v.ahora_ms >= v.hasta_ms {
            return Some(Motivo::Caducado);
        }
        if v.cupo_restante == 0 {
            return Some(Motivo::CupoGastado);
        }
        None
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn tranquila(ahora: u64) -> Vuelta {
        Vuelta { ahora_ms: ahora, hasta_ms: 10_000, cupo_restante: 5, enlace: true, ..Vuelta::default() }
    }

    #[test]
    fn una_vuelta_tranquila_sigue() {
        let mut r = Radar::nuevo();
        for t in 0..1000 {
            assert_eq!(r.mirar(&tranquila(t * LATIDO_MS)), None);
        }
        assert_eq!(r.latidos, 1000);
    }

    #[test]
    fn mentir_revoca_al_momento_y_gana_a_caducar() {
        let mut v = tranquila(20_000);
        v.mal = Some(Mal::IndiceImposible);
        assert_eq!(Radar::nuevo().mirar(&v), Some(Motivo::IndiceImposible));
        v.mal = Some(Mal::LargoImposible);
        assert_eq!(Radar::nuevo().mirar(&v), Some(Motivo::LargoImposible));
    }

    #[test]
    fn una_sola_suplantacion_basta() {
        let mut v = tranquila(0);
        v.origen_ajeno = 1;
        assert_eq!(Radar::nuevo().mirar(&v), Some(Motivo::Suplantacion));
    }

    #[test]
    fn el_plazo_el_cupo_y_el_cable() {
        assert_eq!(Radar::nuevo().mirar(&tranquila(9_999)), None);
        assert_eq!(Radar::nuevo().mirar(&tranquila(10_000)), Some(Motivo::Caducado), "al ms exacto");
        let mut v = tranquila(0);
        v.cupo_restante = 0;
        assert_eq!(Radar::nuevo().mirar(&v), Some(Motivo::CupoGastado));
        let mut v = tranquila(0);
        v.enlace = false;
        assert_eq!(Radar::nuevo().mirar(&v), Some(Motivo::EnlaceCaido));
    }

    #[test]
    fn dieciseis_malformadas_se_perdonan_la_diecisiete_no() {
        let mut r = Radar::nuevo();
        let mut v = tranquila(0);
        v.malformadas = 1;
        for _ in 0..MALFORMADAS_MAX {
            assert_eq!(r.mirar(&v), None);
        }
        assert_eq!(r.mirar(&v), Some(Motivo::Malformadas));
    }

    /// ** Una rafaga no es una inundacion: los golpes tienen que ser SEGUIDOS.
    #[test]
    fn la_rafaga_pasa_la_inundacion_no() {
        let mut r = Radar::nuevo();
        let mut exceso = tranquila(0);
        exceso.ritmo = 3;
        for _ in 0..10 {
            for _ in 0..GOLPES_DE_RITMO - 1 {
                assert_eq!(r.mirar(&exceso), None);
            }
            assert_eq!(r.mirar(&tranquila(0)), None, "un latido limpio pone los golpes a cero");
        }
        for _ in 0..GOLPES_DE_RITMO - 1 {
            assert_eq!(r.mirar(&exceso), None);
        }
        assert_eq!(r.mirar(&exceso), Some(Motivo::Inundacion));
    }

    #[test]
    fn los_codigos_van_y_vuelven() {
        for c in 1..=11 {
            let m = Motivo::desde_codigo(c).unwrap();
            assert_eq!(m.codigo(), c);
            assert!(!m.texto().is_empty());
        }
        assert_eq!(Motivo::desde_codigo(0), None, "cero es abierto, no un motivo");
        assert_eq!(Motivo::desde_codigo(12), None);
    }
}
