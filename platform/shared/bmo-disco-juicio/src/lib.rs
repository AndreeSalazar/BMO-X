//! **EL NIETO** -- el veredicto sobre un disco. El unico que opina.
//!
//! [eje]     CORRECCION
//! [exige]   R-DISCO6..10, R-CPU8, R-CPU9, L7b
//!
//! Capitulo con los numeros: `docs/componente/EL_DISCO_EXIGE.md`.
//!
//! # Por que vive aqui y no en el kernel (L7b)
//!
//! Por la misma razon que `bmo-juicio`: **aqui se puede PROBAR**. Un veredicto
//! sobre un disco que solo se puede comprobar flasheando el disco no es un
//! veredicto comprobado -- y este componente es el unico donde equivocarse no da
//! un fault en pantalla, se lleva el trabajo de alguien.
//!
//! Las tres generaciones de debajo (`bmo-identify`) tampoco opinan, asi que toda
//! la opinion del sistema sobre el almacenamiento esta **en este fichero**, y
//! entera bajo `cargo test`.
//!
//! # ** LA REGLA QUE ORDENA EL VEREDICTO: EL CAMINO CONSERVADOR SE TOMA SOLO
//!
//! Un juez de rendimiento que se calla deja una cifra sin publicar. **Un juez de
//! almacenamiento que se calla tiene que dejar el sistema en el camino que no
//! pierde datos**, no en el rapido. Por eso ninguna funcion de aqui contesta
//! `true` por defecto: cuando falta un dato, la respuesta es la que no asume.
//!
//! Es R-CPU8 con la consecuencia invertida: alli, sin perfil, el juez se calla y
//! no hay veredicto; aqui, sin perfil, el juez **dice que no** y hay que
//! seguirle.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub mod perfil;

pub use perfil::{Cifra, Identidad, Origen, Perfil, KINGSTON_A400_480};

use bmo_identify::{Contraste, LoQueDiceElDisco, Medio};

/// Por que el perfil no se puede aplicar a este disco.
///
/// **Lleva los dos lados** (R-CPU9): lo que el perfil esperaba y lo que el disco
/// dijo. Un `bool` frenaria el juicio sin decir como arreglarlo -- con los dos
/// delante, el arreglo es cambiar una cifra.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SinPerfil {
    pub modelo_esperado: &'static str,
    pub sectores_esperados: u64,
    pub sectores_leidos: u64,
}

/// Que se le puede pedir a este disco, y con que confianza.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Veredicto {
    /// `None` si el disco de delante no es el del perfil. Todo lo que dependa
    /// de datos declarados contesta entonces en su forma conservadora.
    pub perfil: Option<Perfil>,
    /// Por que no hay perfil, con los dos lados.
    pub por_que_no: Option<SinPerfil>,
    /// Lo que el aparato contesto, tal cual.
    pub dice: LoQueDiceElDisco,
    /// Las restas entre dos hechos.
    pub contraste: Contraste,
}

impl Veredicto {
    /// `modelo` y `sectores` salen del IDENTIFY; `usadas`, del driver.
    pub fn emitir(
        dice: LoQueDiceElDisco,
        modelo: &str,
        sectores: u64,
        usadas: u8,
        candidatos: &[Perfil],
    ) -> Veredicto {
        let contraste = dice.contraste(usadas);
        for p in candidatos {
            if p.identidad.coincide(modelo, sectores) {
                return Veredicto { perfil: Some(*p), por_que_no: None, dice, contraste };
            }
        }
        // Sin coincidencia se dice contra QUE se comparo, para que el arreglo
        // sea una cifra y no una lectura de codigo.
        let por_que_no = candidatos.first().map(|p| SinPerfil {
            modelo_esperado: p.identidad.modelo,
            sectores_esperados: p.identidad.sectores,
            sectores_leidos: sectores,
        });
        Veredicto { perfil: None, por_que_no, dice, contraste }
    }

    /// El atajo para el sistema: el unico perfil que hay hoy.
    pub fn del_sistema(dice: LoQueDiceElDisco, modelo: &str, sectores: u64, usadas: u8)
        -> Veredicto
    {
        Veredicto::emitir(dice, modelo, sectores, usadas, &[KINGSTON_A400_480])
    }

    // -- LAS PREGUNTAS QUE ESTRATOS LE HACE AL DISCO ------------------------

    /// **Se puede confiar en que este medio no paga busqueda de cabezal?**
    ///
    /// Solo si el disco lo dijo (`0001h`). `NoContesta` y los valores reservados
    /// contestan `false`: es R-DISCO6, y es la leccion de Windows 7 --que no se
    /// fio de la palabra sola-- aplicada al reves. Aqui no hay prueba de
    /// rendimiento con la que desempatar, asi que la duda va al lado seguro.
    pub fn medio_solido_confirmado(&self) -> bool {
        self.dice.medio.es_estado_solido()
    }

    /// **Es el `FLUSH CACHE` la unica red que hay?**
    ///
    /// `true` tambien SIN perfil, y eso es lo importante: no saber si el disco
    /// tiene condensadores **no autoriza a suponer que los tiene**. La barrera
    /// se respeta igual, que es lo que no pierde datos.
    pub fn la_barrera_es_lo_unico(&self) -> bool {
        match self.perfil {
            Some(p) => !p.condensadores,
            None => true,
        }
    }

    /// **Puede el recolector avisar al disco de lo que ya no importa?**
    ///
    /// Si contesta `false` con un medio solido confirmado, el recolector de la
    /// section 9 de ESTRATOS **no esta completo** y hay que decirlo (R-DISCO10).
    pub fn el_recolector_puede_avisar(&self) -> bool {
        self.dice.trim.soportado
    }

    /// **A que frontera hay que alinear el frente del log**, en bytes.
    ///
    /// `None` sin perfil: no hay forma de preguntarlo (R-DISCO8), asi que sin
    /// perfil **no se puede alinear**, y quien escriba tiene que saberlo en vez
    /// de alinear a un numero inventado.
    pub fn frontera_de_escritura(&self) -> Option<u64> {
        self.perfil.map(|p| p.bloque_de_borrado.valor)
    }

    /// **Se puede publicar una cifra de rendimiento de este disco como medida?**
    ///
    /// R-DISCO9. Sin perfil no; con perfil, solo si su sostenido no es de
    /// catalogo.
    pub fn tiene_rendimiento_medido(&self) -> bool {
        self.perfil.map(|p| p.sostenido_mb_s.es_medida()).unwrap_or(false)
    }

    /// Las RPM, si el medio gira. Para el informe.
    pub fn rpm(&self) -> u16 {
        match self.dice.medio {
            Medio::Rota { rpm } => rpm,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use bmo_identify::Identify;

    fn sector(pares: &[(usize, u16)]) -> [u8; 512] {
        let mut s = [0u8; 512];
        for &(n, v) in pares {
            let b = v.to_le_bytes();
            s[n * 2] = b[0];
            s[n * 2 + 1] = b[1];
        }
        s
    }

    fn dice(pares: &[(usize, u16)]) -> LoQueDiceElDisco {
        let s = sector(pares);
        let id = Identify::nuevo(&s).unwrap();
        LoQueDiceElDisco::leer(&id)
    }

    /// El disco de esta maquina, con su perfil puesto.
    fn kingston() -> Veredicto {
        let d = dice(&[(75, 31), (76, (1 << 8) | 0b1110), (77, 3 << 1), (169, 1), (217, 1)]);
        Veredicto::del_sistema(d, "KINGSTON SA400S37480G", 937_703_088, 1)
    }

    #[test]
    fn el_disco_de_la_casa_encuentra_su_perfil() {
        let v = kingston();
        assert!(v.perfil.is_some());
        assert!(v.por_que_no.is_none());
        assert!(v.medio_solido_confirmado());
        assert!(v.el_recolector_puede_avisar());
        assert_eq!(v.frontera_de_escritura(), Some(2 * 1024 * 1024));
    }

    /// ** Y lo que su perfil obliga a decir: la barrera es lo unico que hay.
    #[test]
    fn este_disco_no_tiene_red_debajo_de_la_barrera() {
        assert!(kingston().la_barrera_es_lo_unico());
    }

    /// ** LA PRUEBA QUE MAS IMPORTA: sin perfil, el juez no se relaja.
    #[test]
    fn sin_perfil_se_toma_el_camino_conservador() {
        let d = dice(&[(217, 1), (169, 1)]);
        let v = Veredicto::del_sistema(d, "UN DISCO CUALQUIERA", 123_456, 1);

        assert!(v.perfil.is_none());
        assert!(v.la_barrera_es_lo_unico(), "no saberlo NO autoriza a suponer que los tiene");
        assert_eq!(v.frontera_de_escritura(), None, "no se alinea a un numero inventado");
        assert!(!v.tiene_rendimiento_medido());
    }

    /// R-CPU9: el "no coincide" trae los dos lados.
    #[test]
    fn el_sin_perfil_dice_contra_que_se_comparo() {
        let d = dice(&[(217, 1)]);
        let v = Veredicto::del_sistema(d, "OTRO", 999, 1);
        let p = v.por_que_no.expect("tiene que decir por que");
        assert_eq!(p.modelo_esperado, "SA400S37480G");
        assert_eq!(p.sectores_leidos, 999);
        assert_ne!(p.sectores_esperados, p.sectores_leidos);
    }

    /// R-DISCO6: un disco mudo no autoriza el camino de SSD, aunque el perfil
    /// diga que ese modelo es un SSD. **Gana el aparato** (R-FW2).
    #[test]
    fn un_medio_mudo_no_se_da_por_solido_ni_con_perfil() {
        let d = dice(&[(75, 31), (76, 1 << 8), (169, 1)]); // sin palabra 217
        let v = Veredicto::del_sistema(d, "KINGSTON SA400S37480G", 937_703_088, 1);
        assert!(v.perfil.is_some(), "el perfil SI coincide");
        assert!(!v.medio_solido_confirmado(), "y aun asi no se afirma el medio");
    }

    /// R-DISCO10: un SSD sin TRIM deja el recolector incompleto, y se ve.
    #[test]
    fn un_solido_sin_trim_lo_dice_el_contraste_y_el_veredicto() {
        let d = dice(&[(217, 1), (169, 0)]);
        let v = Veredicto::del_sistema(d, "KINGSTON SA400S37480G", 937_703_088, 1);
        assert!(!v.el_recolector_puede_avisar());
        assert!(v.contraste.solido_sin_trim);
    }

    /// El estado de hoy, escrito como prueba para que cambie cuando cambie.
    #[test]
    fn hoy_el_sistema_deja_31_ranuras_paradas() {
        assert_eq!(kingston().contraste.ranuras_ociosas, 31);
    }

    /// Un plato que gira: nada del camino de SSD se enciende.
    #[test]
    fn un_hdd_se_lee_como_lo_que_es() {
        let d = dice(&[(217, 0x1C20), (75, 31), (76, 1 << 8)]);
        let v = Veredicto::del_sistema(d, "WDC WD10EZEX", 1_953_525_168, 1);
        assert!(!v.medio_solido_confirmado());
        assert_eq!(v.rpm(), 7200);
        assert!(!v.contraste.solido_sin_trim, "un HDD sin TRIM no es una carencia");
        assert!(v.la_barrera_es_lo_unico(), "sin perfil, la barrera manda igual");
    }
}
