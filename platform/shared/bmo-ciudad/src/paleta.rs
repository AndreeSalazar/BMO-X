//! **LA PALETA** -- los colores, y solo los colores.
//!
//! Sale de las dos capturas que enseno el dueno: fondo casi negro con tinte
//! violeta, torres en morados frios, y el neon repartido en cian, magenta y
//! ambar.
//!
//! ** Pocos tonos y muy separados**, que es lo que hace que el pixel art se lea.
//! Una paleta de treinta grises no es pixel art: es una foto pequena.
//!
//! Vive en su propio fichero porque es lo unico de este crate que se toca **a
//! ojo**. Todo lo demas se juzga con una prueba; esto se juzga mirando, y
//! mezclarlo con la aritmetica obligaria a releer trescientas lineas para
//! cambiar un azul.

/// Un color BGRA de 32 bits, como los quiere el framebuffer.
pub type Color = u32;

/// El cielo, arriba del todo. Casi negro con violeta.
pub const CIELO_ALTO: Color = 0xFF0B0714;
/// El cielo cerca del horizonte: el resplandor de la ciudad tinendo la niebla.
pub const CIELO_BAJO: Color = 0xFF2A1140;
/// Torres del fondo: apenas siluetas.
pub const TORRE_FONDO: Color = 0xFF191033;
/// Torres delanteras.
pub const TORRE_FRENTE: Color = 0xFF241847;
/// El borde iluminado de una torre delantera, del lado del neon.
pub const TORRE_BORDE: Color = 0xFF3B2A6B;

/// Ventana encendida, la mas comun.
pub const VENTANA_CALIDA: Color = 0xFFFFC96B;
/// Ventana encendida en frio.
pub const VENTANA_FRIA: Color = 0xFF7DE3FF;
/// Ventana encendida en magenta.
pub const VENTANA_MAGENTA: Color = 0xFFFF6BD6;
/// Ventana apagada: no es negra, es la torre un poco mas oscura. Una ventana
/// negra del todo desaparece y la fachada se queda lisa.
pub const VENTANA_APAGADA: Color = 0xFF120B26;

/// El cian de la marca. Es el mismo que el de los ojos del gato.
pub const NEON_CIAN: Color = 0xFF00E5FF;
/// El magenta de los letreros.
pub const NEON_MAGENTA: Color = 0xFFFF3DAE;

/// Negro puro: el fondo del logo, y donde acaba la funcion.
pub const NEGRO: Color = 0xFF000000;

/// Mezcla dos colores por canal. Entera, sin coma flotante.
///
/// `parte` de `total` es cuanto de `b` se pone encima de `a`. Con `parte = 0`
/// sale `a`; con `parte = total`, `b`.
pub fn mezcla(a: Color, b: Color, parte: u32, total: u32) -> Color {
    if total == 0 {
        return a;
    }
    let parte = parte.min(total);
    let inv = total - parte;
    let canal = |desp: u32| {
        let ca = (a >> desp) & 0xFF;
        let cb = (b >> desp) & 0xFF;
        ((ca * inv + cb * parte) / total) & 0xFF
    };
    0xFF00_0000 | (canal(16) << 16) | (canal(8) << 8) | canal(0)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn mezclar_a_los_extremos_devuelve_los_extremos() {
        assert_eq!(mezcla(CIELO_ALTO, CIELO_BAJO, 0, 10), CIELO_ALTO);
        assert_eq!(mezcla(CIELO_ALTO, CIELO_BAJO, 10, 10), CIELO_BAJO);
    }

    /// Pasarse de `total` no desborda: se recorta. Un color desbordado se ve
    /// como un pixel de otro color en mitad de un degradado.
    #[test]
    fn pasarse_de_total_se_recorta() {
        assert_eq!(mezcla(NEGRO, NEON_CIAN, 99, 10), NEON_CIAN);
    }

    /// El canal alfa sale siempre opaco: el framebuffer no mezcla, y un alfa a
    /// cero se veria negro sin motivo.
    #[test]
    fn el_alfa_siempre_sale_opaco() {
        assert_eq!(mezcla(NEGRO, NEON_CIAN, 3, 10) & 0xFF00_0000, 0xFF00_0000);
    }
}
