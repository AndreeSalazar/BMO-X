//! **LO QUE EL DISCO CONTESTA** -- el IDENTIFY DEVICE, repartido por herencia.
//!
//! generacion: varias -- abuelo, padre e hijo viven DENTRO, por modulos
//!
//! [eje]     CORRECCION -- aqui no se optimiza nada, se lee bien
//! [exige]   R-DISCO2, R-DISCO6, R-DISCO7, R-FW2, L5, L7
//!
//! Capitulo con los numeros y su origen: `docs/componente/EL_DISCO_EXIGE.md`.
//!
//! # Por que existe este crate
//!
//! Hasta el 2026-08-17 BMO-X le preguntaba al disco tres cosas --modelo, serie
//! y capacidad-- y **no sabia si su disco giraba**. La palabra que lo dice es la
//! 217 y estaba a una lectura de 16 bits del buffer que ya se pedia.
//!
//! Mientras tanto el arbol **si opinaba**: el diseno de ESTRATOS razona sobre
//! TRIM, y la ley dice que un disco *"da caudal cuando tiene cola"*. Ninguna de
//! las dos frases es falsa. **Ninguna estaba comprobada.** Eso es L5 al reves
//! --*hardcodea contratos, pregunta hechos*-- y este crate es el hecho.
//!
//! # ** EL REPARTO POR HERENCIA (L7), y que compra cada generacion
//!
//! ```text
//!    abuelo   `abuelo::Identify`   la PALABRA n. No sabe que significa ninguna
//!    padre    `padre::*`           NOMBRA una palabra y aplica su sesgo y su
//!                                  guarda. No sabe que tiene hermanos
//!    hijo     `hijo::Contraste`    RELACIONA dos del padre. No sabe si la
//!                                  relacion es grave
//!    nieto    `bmo-disco-juicio`   el VEREDICTO. Vive FUERA y se prueba en un
//!                                  `cargo test` de tres segundos
//! ```
//!
//! **El conocimiento solo baja.** El abuelo no puede nombrar al padre, el padre
//! no puede preguntar por sus hermanos, y el nieto es el unico con opinion.
//!
//! Y no es orden por gusto: es lo que hace **falsable** cada afirmacion sobre
//! este disco. *"El exponente esta mal leido"* no puede ser un fallo del abuelo,
//! porque el abuelo no sabe que existan los exponentes. La busqueda queda
//! acotada por construccion a una funcion con su prueba al lado.
//!
//! # ** Lo que este crate NO hace, y es deliberado
//!
//! **No decide nada.** Ni si el disco es apto, ni si hay que encender la cola,
//! ni si falta un perfil. Todo eso es el nieto, y vive fuera **porque alli se
//! puede probar** (L7b): este crate es `no_std` para un target sin sistema
//! operativo, y un veredicto que solo se puede comprobar flasheando un disco no
//! es un veredicto comprobado.
//!
//! No emite comandos: no manda IDENTIFY, no manda TRIM, no toca un registro.
//! Recibe un buffer que ya existe. Eso lo hace probable entero en el anfitrion
//! con sectores inventados a mano -- que es como se probaron los rangos de la
//! palabra 217 sin tener seis discos distintos delante.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub mod abuelo;
pub mod hijo;
pub mod padre;

pub use abuelo::Identify;
pub use hijo::Contraste;
pub use padre::{Cola, Enlace, Geometria, Medio, Trim};

/// Todo lo que el aparato contesta, ya nombrado. **Ningun juicio.**
///
/// Es la foto del padre entero, para que quien la pase no tenga que ir campo
/// por campo. Se compone aqui y no en el padre a proposito: **un campo del padre
/// no puede saber que tiene hermanos**, y esta estructura los conoce a todos.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LoQueDiceElDisco {
    pub medio: Medio,
    pub cola: Cola,
    pub enlace: Enlace,
    pub geometria: Geometria,
    pub trim: Trim,
}

impl LoQueDiceElDisco {
    pub fn leer(id: &Identify) -> LoQueDiceElDisco {
        LoQueDiceElDisco {
            medio: Medio::de(id),
            cola: Cola::de(id),
            enlace: Enlace::de(id),
            geometria: Geometria::de(id),
            trim: Trim::de(id),
        }
    }

    /// `usadas`: ranuras de comando que el driver emplea de verdad. Hoy, 1.
    pub fn contraste(&self, usadas: u8) -> Contraste {
        Contraste::de(self.medio, self.cola, self.enlace, self.geometria, self.trim, usadas)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El sector que contestaria un SSD SATA moderno bien portado.
    fn un_ssd_sata() -> [u8; 512] {
        let mut s = [0u8; 512];
        let mut w = |n: usize, v: u16| {
            let b = v.to_le_bytes();
            s[n * 2] = b[0];
            s[n * 2 + 1] = b[1];
        };
        w(75, 31); // 32 ranuras (menos uno)
        w(76, (1 << 8) | 0b1110); // NCQ + Gen1/2/3
        w(77, 3 << 1); // negociado Gen3
        w(169, 1); // TRIM
        w(217, 0x0001); // no rota
        s
    }

    #[test]
    fn un_ssd_sata_se_lee_entero_y_sin_contrastes() {
        let s = un_ssd_sata();
        let id = Identify::nuevo(&s).unwrap();
        let d = LoQueDiceElDisco::leer(&id);

        assert_eq!(d.medio, Medio::NoRota);
        assert_eq!(d.cola.profundidad, 32);
        assert!(d.cola.ncq);
        assert_eq!(d.enlace.mejor_soportada(), 3);
        assert_eq!(d.enlace.negociada, 3);
        assert!(d.trim.soportado);

        // Con las 32 usadas no queda nada que senalar.
        let c = d.contraste(32);
        assert!(!c.enlace_por_debajo);
        assert!(!c.solido_sin_trim);
        assert_eq!(c.ranuras_ociosas, 0);
    }

    /// ** El estado REAL de BMO-X hoy, escrito como prueba: el mismo disco, con
    /// el driver usando una sola ranura.
    #[test]
    fn el_mismo_disco_con_la_ranura_0_deja_31_ociosas() {
        let s = un_ssd_sata();
        let id = Identify::nuevo(&s).unwrap();
        let c = LoQueDiceElDisco::leer(&id).contraste(1);
        assert_eq!(c.ranuras_ociosas, 31);
    }

    /// Un disco que no contesta nada: todo tiene que salir en su estado
    /// conservador, y **nada** puede afirmarse.
    #[test]
    fn un_sector_a_ceros_no_afirma_nada() {
        let s = [0u8; 512];
        let id = Identify::nuevo(&s).unwrap();
        let d = LoQueDiceElDisco::leer(&id);

        assert_eq!(d.medio, Medio::NoContesta);
        assert!(!d.medio.es_estado_solido());
        assert_eq!(d.cola.profundidad, 1, "cero en la palabra es UNA ranura");
        assert!(!d.cola.ncq);
        assert_eq!(d.enlace.mejor_soportada(), 0);
        assert!(!d.trim.soportado);
        assert!(!d.geometria.valida);

        let c = d.contraste(1);
        assert!(!c.enlace_por_debajo);
        assert!(!c.solido_sin_trim, "no se sabe el medio: no se afirma");
        assert!(!c.desalineado);
    }
}
