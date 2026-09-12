//! **CONFIGURE** -- el DIRECTOR le dice a una app el hueco que tiene.
//!
//! generacion: hijo -- relaciona tres hechos que le dan de fuera (el estado del
//! marco, el panel, lo que mide el marco) y devuelve un evento. **No sabe que
//! hara la app con el**: redibujar a otro tamano, escalar o no hacer nada es
//! cosa suya, al otro lado del proceso.
//!
//! ## De donde sale (2026-09-12)
//!
//! El dueno pidio que el DIRECTOR *"genere una ventana o pantalla completa"*. Y
//! el propio `surface.rs` tenia escrito lo que faltaba:
//!
//! > *Lo que falta para que un juego LLENE el panel es que la app sepa el hueco
//! > nuevo y vuelva a ofrecer una superficie mayor -- eso pide avisarla, y no
//! > esta.*
//!
//! Hasta hoy mandaba la APP: DOOM decia 960x600 y a pantalla completa el
//! DIRECTOR solo podia centrarlo con bordes negros. Es el `configure` de Wayland
//! (`xdg_toplevel.configure` + `ack_configure`), sin socket: viaja por el buzon.
//!
//! ## El protocolo entero, en cuatro pasos
//!
//! ```text
//!    1  DIRECTOR   cambia el marco (Alt+Enter, maximizar) y publica un
//!                  CONFIGURE con el hueco nuevo en el buzon de la app
//!    2  app        crea una superficie de ese tamano y la OFRECE, sin soltar
//!                  la vieja
//!    3  DIRECTOR   ve que la oferta es del MISMO tid: la pone en la misma
//!                  ranura, conserva el marco, suelta la vieja y enciende
//!                  TOMADA en el buzon de la nueva
//!    4  app        ve TOMADA en la nueva y libera la vieja
//! ```
//!
//! ** El paso 4 es lo que Wayland llama `wl_buffer.release`, y no es un lujo:
//! si la app soltara la vieja en el paso 2 y el DIRECTOR tardara --o no tuviera
//! ranura-- el compositor seguiria leyendo memoria que ya es de otro `malloc`.
//! Sin TOMADA la app no tiene forma de saber cuando puede.
//!
//! ## Los numeros son un CONTRATO con C
//!
//! Los lee `<bmo/superficie/amarilla.h>`. La prueba `los_numeros_son_los_de_rex`
//! lee esa cabecera: si alguien cambia uno de los dos lados, el banco lo dice.

/// El bit que dice "hay evento". Lo llevan TODOS los eventos del buzon.
///
/// ** Y el CONFIGURE tambien, a proposito: los bucles de las apps drenan con
/// `if ((e & BMO_EVENTO_HAY) == 0) break;`. Un CONFIGURE sin este bit cortaria
/// el drenaje -- y como ya se habria sacado del anillo, se perderia.
pub const HAY: u64 = 0x100;

/// **Esto es un CONFIGURE.** Bit 61: el 63 es el raton y el 62 la letra.
pub const EV_CONFIGURE: u64 = 1 << 61;

/// **"Ya la tengo"**: bit 24 de la palabra de estado del buzon (+12).
///
/// El DIRECTOR lo enciende en la superficie que acaba de ADOPTAR. Los bytes 0,
/// 1 y 2 de esa palabra ya eran botones, dentro y vista; el 3 estaba libre.
///
/// [!] Esa palabra la PISA entera `Surface::puntero` cada vuelta. Por eso este
/// bit va dentro de la misma escritura y no aparte: escrito aparte, el
/// siguiente movimiento del raton lo borraria.
pub const TOMADA: u32 = 1 << 24;

/// El estado del marco que se le comunica a la app.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Estado {
    /// Con marco, del tamano que tenga el marco.
    Ventana = 0,
    /// Con marco, ocupando el hueco bajo la barra.
    Maximizada = 1,
    /// Sin marco y el panel entero.
    PantallaCompleta = 2,
}

/// Lo que viaja: el hueco y en que estado esta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Configure {
    pub ancho: u32,
    pub alto: u32,
    pub estado: Estado,
}

/// **El evento de 64 bits**, con la misma forma que el del raton:
///
/// ```text
///    bit 61        CONFIGURE
///    bit 8         HAY
///    bits 16..31   ancho
///    bits 32..47   alto
///    bits 48..55   estado
///    byte bajo     0  -> una app vieja lo lee como el scancode 0, soltado
/// ```
///
/// `None` si no cabe en 16 bits o es cero: un hueco de 0 no es un hueco, y
/// recortarlo a 65535 seria mandar un tamano que nadie pidio.
pub fn codifica(c: Configure) -> Option<u64> {
    if c.ancho == 0 || c.alto == 0 || c.ancho > 0xFFFF || c.alto > 0xFFFF {
        return None;
    }
    Some(
        EV_CONFIGURE
            | HAY
            | (c.ancho as u64) << 16
            | (c.alto as u64) << 32
            | (c.estado as u64) << 48,
    )
}

/// El inverso, para quien lo lee en Rust y para el banco. `None` si no es un
/// CONFIGURE bien formado.
pub fn descodifica(e: u64) -> Option<Configure> {
    if e & EV_CONFIGURE == 0 || e & HAY == 0 {
        return None;
    }
    // Un evento de raton o de letra con el 61 encendido por error no es un
    // CONFIGURE: se rechaza antes que adivinar cual de los dos quiso ser.
    if e & (1 << 63) != 0 || e & (1 << 62) != 0 {
        return None;
    }
    let estado = match (e >> 48) & 0xFF {
        0 => Estado::Ventana,
        1 => Estado::Maximizada,
        2 => Estado::PantallaCompleta,
        _ => return None,
    };
    let ancho = ((e >> 16) & 0xFFFF) as u32;
    let alto = ((e >> 32) & 0xFFFF) as u32;
    if ancho == 0 || alto == 0 {
        return None;
    }
    Some(Configure { ancho, alto, estado })
}

/// **El hueco que le toca a la app**, dado su marco.
///
/// ```text
///    pantalla completa   el panel entero
///    maximizada/ventana  el interior del marco: el ancho menos los dos
///                        bordes, el alto menos el titulo y el borde de abajo
/// ```
///
/// ** La resta es la INVERSA exacta de `Chrome::for_content`, que suma `+2` al
/// ancho y `+titulo_h+1` al alto. Si una cambia y la otra no, la app pinta un
/// pixel de mas o de menos -- y eso no da error, da un borde. La prueba
/// `el_hueco_deshace_lo_que_suma_el_marco` lo ata.
pub fn hueco(
    pantalla_completa: bool,
    maximizada: bool,
    panel: (u32, u32),
    marco: (u32, u32),
    titulo_h: u32,
) -> Configure {
    if pantalla_completa {
        return Configure { ancho: panel.0, alto: panel.1, estado: Estado::PantallaCompleta };
    }
    let estado = if maximizada { Estado::Maximizada } else { Estado::Ventana };
    Configure {
        ancho: marco.0.saturating_sub(2),
        alto: marco.1.saturating_sub(titulo_h + 1),
        estado,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn c(ancho: u32, alto: u32, estado: Estado) -> Configure {
        Configure { ancho, alto, estado }
    }

    #[test]
    fn ida_y_vuelta() {
        for estado in [Estado::Ventana, Estado::Maximizada, Estado::PantallaCompleta] {
            let e = codifica(c(1920, 1080, estado)).unwrap();
            assert_eq!(descodifica(e), Some(c(1920, 1080, estado)));
        }
    }

    /// *** LA FILA QUE PROTEGE A LAS APPS QUE YA EXISTEN. Drenan con
    /// `(e & HAY) == 0 -> break`; sin HAY el CONFIGURE cortaria el bucle y se
    /// perderia. Y con el byte bajo a 0 y sin PULSADA, una app que no lo conoce
    /// lo lee como "se solto el scancode 0" -- que no es ninguna tecla.
    #[test]
    fn una_app_vieja_no_se_rompe() {
        let e = codifica(c(360, 360, Estado::Ventana)).unwrap();
        assert_ne!(e & HAY, 0, "sin HAY, el drenaje de las apps se corta");
        assert_eq!(e & 0xFF, 0, "el byte bajo es el scancode de las apps viejas");
        assert_eq!(e & 0x200, 0, "PULSADA apagado: nadie pulso nada");
        assert_eq!(e & (1 << 63), 0, "no se puede confundir con el raton");
        assert_eq!(e & (1 << 62), 0, "ni con una letra");
    }

    #[test]
    fn no_cero_ni_mas_de_16_bits() {
        assert_eq!(codifica(c(0, 100, Estado::Ventana)), None);
        assert_eq!(codifica(c(100, 0, Estado::Ventana)), None);
        assert_eq!(codifica(c(65_536, 100, Estado::Ventana)), None);
    }

    #[test]
    fn no_lo_que_no_es_un_configure() {
        let raton = (1u64 << 63) | HAY | (100 << 16);
        assert_eq!(descodifica(raton), None);
        let mal = codifica(c(10, 10, Estado::Ventana)).unwrap() | (1 << 63);
        assert_eq!(descodifica(mal), None, "raton con el 61: no se adivina");
        let estado_raro = EV_CONFIGURE | HAY | (10 << 16) | (10 << 32) | (7 << 48);
        assert_eq!(descodifica(estado_raro), None);
    }

    #[test]
    fn pantalla_completa_es_el_panel() {
        let h = hueco(true, false, (1920, 1080), (400, 300), 24);
        assert_eq!(h, c(1920, 1080, Estado::PantallaCompleta));
        let h = hueco(true, true, (1920, 1080), (1920, 1040), 24);
        assert_eq!(h.estado, Estado::PantallaCompleta, "gana a maximizada");
    }

    /// ** La inversa de `Chrome::for_content`: +2 y +titulo+1.
    #[test]
    fn el_hueco_deshace_lo_que_suma_el_marco() {
        let (ancho, alto, titulo) = (360u32, 360u32, 24u32);
        let marco = (ancho + 2, alto + titulo + 1);
        assert_eq!(hueco(false, false, (1920, 1080), marco, titulo), c(360, 360, Estado::Ventana));
        assert_eq!(hueco(false, true, (1920, 1080), marco, titulo).estado, Estado::Maximizada);
    }

    fn valor_de_rex(nombre: &str) -> u64 {
        let h = include_str!("../../../../toolchain/forge/sem-asm/tables/bmo/superficie/amarilla.h");
        let linea = h
            .lines()
            .find(|l| l.starts_with("#define ") && l.split_whitespace().nth(1) == Some(nombre))
            .unwrap_or_else(|| panic!("{nombre} no esta en superficie/amarilla.h"));
        let crudo = linea.split_whitespace().nth(2).unwrap();
        let crudo = crudo.trim_end_matches(|ch: char| ch == 'U' || ch == 'L');
        match crudo.strip_prefix("0x") {
            Some(h) => u64::from_str_radix(h, 16).unwrap(),
            None => crudo.parse().unwrap(),
        }
    }

    #[test]
    fn los_numeros_son_los_de_rex() {
        assert_eq!(valor_de_rex("BMO_SUP_EV_CONFIGURE"), EV_CONFIGURE);
        assert_eq!(valor_de_rex("BMO_SUP_TOMADA"), TOMADA as u64);
        assert_eq!(valor_de_rex("BMO_SUP_ESTADO_VENTANA"), Estado::Ventana as u64);
        assert_eq!(valor_de_rex("BMO_SUP_ESTADO_MAXIMIZADA"), Estado::Maximizada as u64);
        assert_eq!(valor_de_rex("BMO_SUP_ESTADO_COMPLETA"), Estado::PantallaCompleta as u64);
    }
}
