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

/// Un color de 32 bits, como los quiere el framebuffer.
///
/// No se declara aqui: **es el de `bmo-dibujo`**. Un `type Color = u32` propio
/// no seria una copia peligrosa hoy --los dos son `u32`-- pero seria dos sitios
/// donde cambiar el formato de pixel el dia que una GPU diga otra cosa, y esa
/// es exactamente la clase de acuerdo que se rompe sin que nadie se entere.
pub use bmo_dibujo::Color;

// ** LA ESCALERA DE VALORES, que es lo que fallaba en el primer metal.
//
// En la foto del Ryzen las torres salian **mas claras que el cielo** y las dos
// capas al mismo brillo: la ciudad se leia como una sola pared con textura, y el
// gato quedaba enredado dentro en vez de delante.
//
// El arreglo no es de color, es de VALOR. Una escena con profundidad tiene los
// planos separados en brillo y **sin solaparse**, y el orden lo dicta la fisica
// de una ciudad de noche: el cielo esta iluminado por el resplandor de abajo, y
// los edificios son masas oscuras recortadas contra el. Cuanto mas lejos, mas se
// acerca un edificio al color del cielo -- eso es la niebla, y es la pista que
// el ojo usa para leer distancia.
//
//     cielo         lo mas claro. Es la fuente de luz.
//     torre fondo   un poco mas oscura que el cielo. Casi se funde: esta lejos.
//     torre frente  MUCHO mas oscura. Es la silueta que recorta.
//     ventanas      lo unico brillante, y solo en el frente.
//
// La escalera tiene prueba (`las_capas_no_se_solapan_en_brillo`): si alguien
// toca un tono y rompe el orden, sale rojo antes de llegar a la maquina.

/// El cielo, arriba del todo.
pub const CIELO_ALTO: Color = 0xFF1A1030;
/// El cielo cerca del horizonte: el resplandor de la ciudad tinendo la niebla.
pub const CIELO_BAJO: Color = 0xFF4A2168;
/// Torres del fondo: casi el cielo, un punto mas oscuras. **Estan lejos.**
pub const TORRE_FONDO: Color = 0xFF241443;
/// Torres delanteras: la silueta que recorta contra todo lo demas.
pub const TORRE_FRENTE: Color = 0xFF0E0820;
/// El borde iluminado de una torre delantera, del lado del neon.
pub const TORRE_BORDE: Color = 0xFF2E1D57;

// ** EL MARCO: el escalon que faltaba en la escalera, y el que mas cambia.
//
// La escalera iba cielo -> torre lejana -> torre cercana, y ahi se acababa. Con
// eso la escena tiene profundidad pero sigue siendo **una vista**: no hay nada
// entre el ojo y la ciudad.
//
// En la referencia que enseno el dueno --y en cualquier plano de callejon-- los
// bordes izquierdo y derecho son masas de edificio casi negras. Eso es lo unico
// que separa "una foto de una ciudad" de "estas de pie en un callejon
// mirandola", y no se consigue con detalle: se consigue con **un valor mas
// oscuro que todo lo demas y sin nada dentro**.
//
// Son dos tonos y no uno porque el marco tambien tiene profundidad: los bloques
// mas cercanos son mas oscuros que los que retroceden hacia el centro. Es la
// misma perspectiva aerea de la niebla, aplicada al primer plano.

/// El bloque del marco mas cercano al ojo. **Lo mas oscuro de la escena.**
pub const MARCO_CERCA: Color = 0xFF050310;
/// El bloque del marco que ya retrocede hacia el centro.
pub const MARCO_LEJOS: Color = 0xFF0A0619;
/// La luz de la calle enganchandose en el canto interior del marco.
///
/// Sin esto el marco es un agujero negro con borde recto y se lee como una
/// barra tapando la pantalla. Con una linea de luz en el canto, se lee como una
/// esquina de edificio -- y cuesta un rectangulo de dos pixeles de ancho.
pub const MARCO_CANTO: Color = 0xFF3A2A6E;

/// Ventana encendida, la mas comun.
pub const VENTANA_CALIDA: Color = 0xFFFFC96B;
/// Ventana encendida en frio.
pub const VENTANA_FRIA: Color = 0xFF7DE3FF;
/// Ventana encendida en magenta.
pub const VENTANA_MAGENTA: Color = 0xFFFF6BD6;
/// Ventana apagada: no es negra, es la fachada un punto mas clara. Una ventana
/// negra del todo desaparece y la torre se queda lisa.
pub const VENTANA_APAGADA: Color = 0xFF150D2C;

/// **El brillo percibido de un color, 0..255.**
///
/// Los tres canales NO pesan igual: el ojo humano es mucho mas sensible al verde
/// que al azul. Los coeficientes son los de siempre (2126/7152/722), que es lo
/// que usa cualquier conversion a gris que no quiera mentir.
///
/// Existe para poder **probar la escalera de valores**. Sin esto, "el fondo es
/// mas oscuro que el frente" es una opinion sobre dos hexadecimales.
pub fn luminancia(c: Color) -> u32 {
    let r = (c >> 16) & 0xFF;
    let g = (c >> 8) & 0xFF;
    let b = c & 0xFF;
    (r * 2126 + g * 7152 + b * 722) / 10000
}

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
///
/// [!] **Es `bmo_dibujo::mezclar` con los argumentos al derecho.** Habia dos
/// mezcladores en el repo haciendo la misma aritmetica --este y el del
/// rasterizador-- y solo se diferenciaban en cual de los dos colores iba
/// primero. Ahora hay uno; esta funcion se queda porque toda la escena esta
/// escrita en el orden *"sobre `a`, pon `parte` de `b`"*, que es como se piensa
/// un degradado, y darle la vuelta a ochenta llamadas para ahorrar una linea
/// seria cambiar codigo bueno por nada.
pub fn mezcla(a: Color, b: Color, parte: u32, total: u32) -> Color {
    if total == 0 {
        return a;
    }
    bmo_dibujo::mezclar(b, a, parte, total)
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

    /// ** LA ESCALERA DE VALORES, y esta prueba nace de una foto del Ryzen.
    ///
    /// En el primer metal las torres salieron **mas claras que el cielo** y las
    /// dos capas al mismo brillo: la ciudad se leia como una pared con textura y
    /// el gato quedaba enredado dentro. El fallo no era de color sino de VALOR,
    /// y un fallo de valor no se ve leyendo hexadecimales -- por eso hay una
    /// funcion de luminancia y por eso hay esta casilla.
    ///
    /// El orden lo dicta una ciudad de noche: el cielo esta iluminado por el
    /// resplandor de abajo, lo lejano se le acerca por la niebla, y lo cercano
    /// es la masa oscura que recorta.
    #[test]
    fn las_capas_no_se_solapan_en_brillo() {
        let cielo = luminancia(CIELO_BAJO);
        let fondo = luminancia(TORRE_FONDO);
        let frente = luminancia(TORRE_FRENTE);
        assert!(cielo > fondo, "el cielo tiene que ser lo mas claro");
        assert!(fondo > frente, "el fondo esta LEJOS: mas claro que el frente");
        // Y separadas de verdad, no por un punto: dos planos a cinco unidades de
        // distancia se leen como uno solo, que es exactamente lo que pasaba.
        assert!(cielo - fondo >= 12, "cielo y fondo se confunden");
        assert!(fondo - frente >= 12, "las dos capas de torres se confunden");
    }

    /// ** Y EL MARCO CIERRA LA ESCALERA POR ABAJO.
    ///
    /// El plano mas cercano tiene que ser **lo mas oscuro que hay**, incluida la
    /// torre mas cercana. Si el marco quedara al mismo valor que las torres, se
    /// leeria como una torre mas pegada al borde en vez de como la pared que
    /// tienes al lado -- y toda la razon de que exista se pierde sin que nada
    /// falle.
    #[test]
    fn el_marco_es_lo_mas_oscuro_de_la_escena() {
        let frente = luminancia(TORRE_FRENTE);
        let lejos = luminancia(MARCO_LEJOS);
        let cerca = luminancia(MARCO_CERCA);
        assert!(frente > lejos, "el marco tiene que ser mas oscuro que la torre cercana");
        assert!(lejos > cerca, "el bloque de delante es el mas oscuro del marco");
    }

    /// El canto del marco es lo UNICO claro que lleva: es la luz de la calle
    /// enganchandose en la esquina. Sin el, el marco es una barra negra.
    #[test]
    fn el_canto_del_marco_se_ve_contra_el_marco() {
        assert!(
            luminancia(MARCO_CANTO) > luminancia(MARCO_CERCA) + 15,
            "el canto no se distingue de la masa"
        );
    }

    /// Las ventanas encendidas son **lo unico brillante** de la escena. Si una
    /// fachada llegara al brillo de una ventana, la torre se leeria encendida
    /// entera -- que es la otra mitad de lo que salio en la foto.
    #[test]
    fn las_ventanas_son_lo_mas_brillante() {
        let tope = luminancia(CIELO_BAJO).max(luminancia(TORRE_FONDO));
        for v in [VENTANA_CALIDA, VENTANA_FRIA, VENTANA_MAGENTA] {
            assert!(luminancia(v) > tope + 40, "una ventana no destaca del fondo");
        }
        assert!(
            luminancia(VENTANA_APAGADA) < luminancia(TORRE_FONDO),
            "una ventana apagada no puede ser mas clara que una torre lejana"
        );
    }
}
