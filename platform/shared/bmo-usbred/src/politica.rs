//! **LA POLITICA USB**: en que CATEGORIA cae cada aparato, y que se le deja hacer.
//!
//! [carril]  VERDE     una tabla y un veredicto: no toca el bus
//! [cuesta]  NADA      unas comparaciones por interfaz
//! [riesgo]  UNICO     es la unica lista de lo que se deja entrar: una fila de mas
//!                     abre una puerta que despues no mira nadie
//!
//! # Por que existe (2026-09-14)
//!
//! Eddi, con su movil enchufado: *"intenta aislar TODO en USB, como siempre en
//! categoria y por que, pero MAS ESTRICTO"*. USB es la puerta por la que un
//! aparato ajeno se sienta dentro de la maquina: un pendrive que dice ser
//! teclado, un movil que ofrece su shell, una camara que se enciende sola.
//!
//! # REGLA 0: TODO NEGADO
//!
//! Un aparato entra SOLO si su categoria lo permite, y cada NO tiene nombre.
//!
//! ```text
//!    categoria  que es                     que se le deja
//!    MANOS      teclado y raton            entra solo, al enchufar
//!    SONIDO     audio USB                  solo cuando el dueno lo pide (`audio`)
//!    RED        RNDIS / NCM / ECM          NUNCA sola: orden explicita, UNA a la vez,
//!                                          y su corral PRESTADO en el titular del DMA
//!    PASO       hubs                       solo hace de paso: no tiene datos propios
//!    ALMACEN    discos                     NEGADO: hoy no hay driver; el dia que lo
//!                                          haya, solo lectura y con orden
//!    MOVIL      MTP y ADB                  NEGADO SIEMPRE: son la puerta a los
//!                                          ficheros y a la shell del movil
//!    OJOS       camaras                    NEGADO: una camara no se enciende sin que
//!                                          su dueno lo sepa
//!    OPACO      del fabricante, o sin      NEGADO: lo que no dice que es, no entra
//!               clase conocida
//! ```
//!
//! [!] Esto DECIDE y no ejecuta. El portero lo dice y `bmo_uhid` sigue adoptando
//! lo suyo; el dia que entre un driver nuevo, este veredicto es el que lo deja.

use crate::clase::Tipo;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Categoria {
    Manos,
    Sonido,
    Red,
    Paso,
    Almacen,
    Movil,
    Ojos,
    Opaco,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motivo {
    /// Hoy no hay driver, y no se adopta lo que no se sabe tratar.
    SinDriver,
    /// MTP o ADB: la puerta a los datos o a la shell de otro aparato.
    PuertaDelMovil,
    /// Una camara.
    Camara,
    /// No dice lo que es.
    NoDiceQueEs,
    /// Ya hay una red USB adoptada: dos no se vigilan por separado.
    YaHayUna,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Adopta,
    /// Solo con una orden explicita del dueno.
    SoloConOrden,
    Niega(Motivo),
}

pub fn categoria(t: Tipo) -> Categoria {
    match t {
        Tipo::Hid => Categoria::Manos,
        Tipo::Audio => Categoria::Sonido,
        Tipo::Rndis | Tipo::Ncm | Tipo::Ecm | Tipo::DatosCdc => Categoria::Red,
        Tipo::Hub => Categoria::Paso,
        Tipo::Almacenamiento => Categoria::Almacen,
        Tipo::Mtp | Tipo::Adb => Categoria::Movil,
        Tipo::Video => Categoria::Ojos,
        Tipo::DelFabricante | Tipo::Otro => Categoria::Opaco,
    }
}

/// **El veredicto.** `orden` = el dueno lo pidio por su nombre; `ya_hay_red` =
/// otra red USB esta adoptada.
pub fn decidir(t: Tipo, orden: bool, ya_hay_red: bool) -> Decision {
    match categoria(t) {
        Categoria::Manos | Categoria::Paso => Decision::Adopta,
        Categoria::Sonido => {
            if orden {
                Decision::Adopta
            } else {
                Decision::SoloConOrden
            }
        }
        Categoria::Red => {
            if !orden {
                Decision::SoloConOrden
            } else if ya_hay_red {
                Decision::Niega(Motivo::YaHayUna)
            } else {
                Decision::Adopta
            }
        }
        Categoria::Almacen => Decision::Niega(Motivo::SinDriver),
        Categoria::Movil => Decision::Niega(Motivo::PuertaDelMovil),
        Categoria::Ojos => Decision::Niega(Motivo::Camara),
        Categoria::Opaco => Decision::Niega(Motivo::NoDiceQueEs),
    }
}

/// **La frase del portero**: que es, en que categoria cae y que se le deja.
/// Con el aparato recien enchufado y sin orden de nadie.
pub fn frase(t: Tipo) -> &'static str {
    match t {
        Tipo::Rndis => "RED por USB (RNDIS, anclaje de un movil): no entra sola -- orden explicita y una a la vez; hoy ademas falta BULK",
        Tipo::Ncm => "RED por USB (NCM): no entra sola -- orden explicita y una a la vez; hoy ademas falta BULK",
        Tipo::Ecm => "RED por USB (ECM): no entra sola -- orden explicita y una a la vez; hoy ademas falta BULK",
        Tipo::DatosCdc => "RED por USB, su mitad de datos: va con su control, nunca suelta",
        Tipo::Mtp => "MOVIL en modo archivos (MTP): NEGADO siempre -- es la puerta a sus ficheros",
        Tipo::Adb => "MOVIL, depuracion (ADB): NEGADO siempre -- es la puerta a su shell",
        Tipo::Almacenamiento => "ALMACEN (disco USB): NEGADO -- no hay driver; el dia que lo haya, solo lectura",
        Tipo::Video => "OJOS (camara USB): NEGADO -- no se enciende sin que su dueno lo sepa",
        Tipo::Hid => "MANOS (HID)",
        Tipo::Audio => "SONIDO (audio USB): entra cuando el dueno lo pide",
        Tipo::Hub => "PASO (hub)",
        Tipo::DelFabricante => "OPACO (clase del fabricante): NEGADO -- lo que no dice que es, no entra",
        Tipo::Otro => "OPACO (clase sin nombre): NEGADO -- lo que no dice que es, no entra",
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::clase::que_es;

    /// *** REGLA 0: de las 256 clases, solo entran solas las MANOS y el PASO.
    #[test]
    fn todo_negado_por_defecto() {
        for c in 0..=255u8 {
            for (s, p) in [(0u8, 0u8), (1, 1), (0xFF, 0xFF)] {
                let t = que_es(c, s, p);
                let d = decidir(t, false, false);
                if d == Decision::Adopta {
                    assert!(matches!(categoria(t), Categoria::Manos | Categoria::Paso), "{c:02X}/{s:02X}/{p:02X} entra solo");
                }
            }
        }
    }

    #[test]
    fn la_red_nunca_entra_sola_y_solo_una() {
        let rndis = que_es(0xE0, 0x01, 0x03);
        assert_eq!(decidir(rndis, false, false), Decision::SoloConOrden);
        assert_eq!(decidir(rndis, true, false), Decision::Adopta);
        assert_eq!(decidir(rndis, true, true), Decision::Niega(Motivo::YaHayUna));
    }

    #[test]
    fn el_movil_y_la_camara_ni_con_orden() {
        assert_eq!(decidir(que_es(0x06, 0x01, 0x01), true, false), Decision::Niega(Motivo::PuertaDelMovil));
        assert_eq!(decidir(que_es(0xFF, 0x42, 0x01), true, false), Decision::Niega(Motivo::PuertaDelMovil));
        assert_eq!(decidir(que_es(0x0E, 0x01, 0x00), true, false), Decision::Niega(Motivo::Camara));
        assert_eq!(decidir(que_es(0xFF, 0x00, 0x00), true, false), Decision::Niega(Motivo::NoDiceQueEs));
    }

    #[test]
    fn cada_tipo_tiene_su_frase() {
        for c in 0..=255u8 {
            assert!(!frase(que_es(c, 0, 0)).is_empty());
        }
    }
}
