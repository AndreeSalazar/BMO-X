//! **La ventana del SONIDO** -- F10, el aparato y quien lo tiene.
//!
//! === Lo que ensena, y por que ese orden ===
//!
//! Arriba el APARATO, porque es la pregunta que decide todo lo demas: si no hay
//! camino, el volumen y las notas no significan nada. Debajo el volumen, y
//! abajo un teclado para probarlo -- porque un control de sonido que no deja
//! hacer ruido no se puede comprobar, y entonces no se sabe si funciona.
//!
//! === * EL COMPOSITOR RECLAMA EL SONIDO AL ABRIRLA Y LO SUELTA AL CERRARLA ===
//!
//! Y esto es lo interesante de la ventana, mas que lo que pinta.
//!
//! `KIND_AUDIO` es **exclusivo**: un solo proceso lo tiene a la vez. Si el
//! escritorio lo reclamara al arrancar --como hace con la pantalla y la
//! entrada-- **ningun programa lanzado desde el podria volver a sonar jamas**,
//! y el sintoma seria `c/musica.bex` diciendo "lo tiene otro proceso" para
//! siempre.
//!
//! Eso ya paso una vez, con la pantalla: `gui.bex` la reclamaba al arrancar y no
//! la soltaba nunca, asi que `ray.bex` no podia pintar. Costo escribir
//! `PANTALLA_SOLTAR` despues, con el fallo delante.
//!
//! Aqui se hace al reves desde el primer dia: **se toma al abrir y se devuelve
//! al cerrar**. La ventana es un huesped del aparato, no su dueno. Por eso
//! `Sonido::release` existia antes de que hubiera nadie que lo llamara.
//!
//! === Por que F10 ===
//!
//! Una tecla de funcion no produce caracter en ninguna distribucion, asi que no
//! choca con escribir -- el mismo motivo que F11 y F12. Y va pegada a ellas
//! porque es la misma familia: ventanas que se abren con una tecla y se leen.

use bmo_userland as bmo;

use super::marco::Marco;
use super::*;
use crate::texto::decimal;

// Proporcion de la pantalla y no un tamano fijo, como las demas: ver
// `docs/LIDERES.md`. Los minimos existen para que no se pueda dejar
// inservible con el raton -- el teclado de abajo son siete teclas de 52 px y
// por debajo de eso no se puede tocar.
const SON_PCT_ANCHO: u32 = 48;
const SON_PCT_ALTO: u32 = 40;
const SON_MIN_ANCHO: u32 = 620;
const SON_MIN_ALTO: u32 = 300;

// El ambar es de esta ventana igual que el azul es del kernel y el verde de
// ESTRATOS: el color dice cual es antes de leer el titulo.
const SON_FONDO: u32 = 0x0018_1105;
const SON_TITULO_FONDO: u32 = 0x0026_1A08;
const SON_BORDE: u32 = 0x004A_3418;
const SON_TITULO: u32 = 0x00F0_A860;
const SON_BARRA: u32 = 0x00D8_8C3A;
const SON_BARRA_HUECO: u32 = 0x0032_2410;
const TECLA_BLANCA: u32 = 0x00C8_C0B0;
const TECLA_NEGRA: u32 = 0x0020_1C18;
const TECLA_PULSADA: u32 = 0x00F0_A860;

/// Las siete notas del teclado de abajo, y la letra que las toca.
///
/// Es una octava y no mas: caben siete teclas legibles en el ancho de la
/// ventana, y el objeto de esto es **comprobar que suena**, no tocar una pieza.
/// Para una pieza esta `c/musica.bex`, que es un programa y no una ventana.
pub(crate) const NOTAS: [(u8, u32, &str); 7] = [
    (b'z', 262, "DO"),
    (b'x', 294, "RE"),
    (b'c', 330, "MI"),
    (b'v', 349, "FA"),
    (b'b', 392, "SOL"),
    (b'n', 440, "LA"),
    (b'm', 494, "SI"),
];

/// La ventana del sonido. **Movible**, como todas las de `gui.bex`.
///
/// Nacio con una caja fija --cuatro numeros y un `contiene`-- y eso ya era una
/// excepcion el dia que se escribio: ESTRATOS llevaba `Marco` desde antes. Una
/// ventana que no se puede apartar tapa justo lo que uno quiere comparar con
/// ella, y aqui eso duele mas que en otras: el volumen se ajusta MIRANDO otra
/// cosa -- lo que suena.
pub(crate) struct CajaSonido {
    pub(crate) marco: Marco,
}

impl CajaSonido {
    pub(crate) fn nueva(p: &bmo::Pantalla) -> Self {
        Self {
            marco: Marco::nuevo(p, SON_PCT_ANCHO, SON_PCT_ALTO, SON_MIN_ANCHO, SON_MIN_ALTO),
        }
    }
}

/// Escribe `s` en `dst` desde `n`, sin salirse. Igual que en `klog`.
fn poner(s: &[u8], dst: &mut [u8], n: &mut usize) {
    for &b in s {
        if *n < dst.len() {
            dst[*n] = b;
            *n += 1;
        }
    }
}

fn num(v: u64, dst: &mut [u8], n: &mut usize) {
    let mut d = [0u8; 10];
    let k = decimal(v, &mut d);
    poner(&d[..k], dst, n);
}

/// Pinta la ventana entera.
///
/// `aparatos` es la mascara que contesto el kernel (0 = no se pudo preguntar
/// porque el sonido es de otro), `volumen` el que esta puesto, y `pulsada` la
/// nota que se esta tocando ahora mismo, si alguna.
///
/// No recuerda nada entre llamadas **a proposito**: el estado de la sesion vive
/// en `main.rs`, igual que el desplazamiento y el filtro del klog. Un modulo que
/// pinta y ademas recuerda acaba teniendo dos verdades sobre lo mismo.
pub(crate) fn pintar(
    p: &bmo::Pantalla,
    c: &CajaSonido,
    tengo: bool,
    aparatos: u64,
    volumen: u8,
    pulsada: Option<usize>,
) {
    if c.marco.minimizada {
        return;
    }
    // El cromo lo pinta el Marco: sombra, esquinas, barra de titulo y los tres
    // botones. Estaba escrito a mano aqui, que es la forma de que un dia el
    // redondeo de esta ventana no case con el de las otras.
    c.marco.pintar_cromo(p, SON_BORDE, SON_FONDO, SON_TITULO_FONDO, SON_TITULO);
    c.marco.pintar_botones(p, SON_TITULO_FONDO);

    let tx = c.marco.x + 16;
    p.rect(tx, c.marco.y + 9, 8, 8, SON_TITULO);
    let px = p.texto(tx + 16, c.marco.y + 8, "Sonido", TEXTO);
    p.texto(px + 2 * bmo::GLIFO_ANCHO, c.marco.y + 8, "KIND_AUDIO", TEXTO_TENUE);

    let mut ty = c.marco.y + TITULO_ALTO + 10;

    // -- 1. EL APARATO -------------------------------------------------
    //
    // Primero porque decide lo demas. Y con la salvedad dicha: un bit puesto
    // significa que hay CAMINO, no que se vaya a oir algo -- el puerto del
    // altavoz existe en todo x86 y el zumbador fisico no.
    if !tengo {
        p.texto(tx, ty, "el sonido lo tiene OTRO proceso.", TEXTO);
        ty += bmo::GLIFO_ALTO + 4;
        p.texto(tx, ty, "cierra esta ventana y vuelve a abrirla cuando lo suelte.", TEXTO_TENUE);
        return;
    }

    let mut lin = [0u8; 80];
    let mut n = 0usize;
    poner(b"aparato   ", &mut lin, &mut n);
    if aparatos & 1 != 0 {
        poner(b"altavoz del PC", &mut lin, &mut n);
    } else {
        poner(b"ninguno", &mut lin, &mut n);
    }
    if aparatos & 2 != 0 {
        poner(b" + HD Audio", &mut lin, &mut n);
    }
    if aparatos & 4 != 0 {
        poner(b" + audifono USB", &mut lin, &mut n);
    }
    p.texto_bytes(tx, ty, &lin[..n], TEXTO);
    ty += bmo::GLIFO_ALTO + 2;

    // La salvedad, en tenue y siempre. Sin ella, un altavoz que no suena parece
    // un sistema roto -- y la mitad de las placas modernas no traen zumbador.
    //
    // Con audifono USB delante la frase cambia, porque **ahi el volumen si
    // manda**: es un `SET_CUR` sobre su Feature Unit y el aparato obedece.
    p.texto(
        tx,
        ty,
        if aparatos & 4 != 0 {
            "el volumen manda sobre el audifono USB de verdad"
        } else {
            "(hay camino; que suene depende de la placa)"
        },
        TEXTO_TENUE,
    );
    ty += bmo::GLIFO_ALTO + 2;
    if aparatos & 2 == 0 {
        p.texto(tx, ty, "HD Audio: sin driver todavia -- casilla 5.1", TEXTO_TENUE);
    }
    ty += bmo::GLIFO_ALTO + 10;

    // -- 2. EL VOLUMEN --------------------------------------------------
    let mut lin = [0u8; 40];
    let mut n = 0usize;
    poner(b"volumen   ", &mut lin, &mut n);
    num(volumen as u64, &mut lin, &mut n);
    poner(b" / 100", &mut lin, &mut n);
    p.texto_bytes(tx, ty, &lin[..n], TEXTO);
    ty += bmo::GLIFO_ALTO + 6;

    let barra_ancho = c.marco.ancho - 32 - 16;
    p.rect(tx, ty, barra_ancho, 10, SON_BARRA_HUECO);
    let lleno = (barra_ancho * volumen as u32) / 100;
    if lleno > 0 {
        p.rect(tx, ty, lleno, 10, SON_BARRA);
    }
    // La marca del escalon: en el altavoz del PC el volumen NO es continuo --
    // son dos modos del temporizador, y el corte esta en 50. Dibujarlo evita
    // que parezca que la barra no hace nada entre 51 y 100.
    p.rect(tx + barra_ancho / 2, ty - 3, 1, 16, SON_TITULO);
    ty += 16;
    p.texto(
        tx,
        ty,
        "el altavoz del PC tiene DOS escalones, no cien: el corte es la marca",
        TEXTO_TENUE,
    );
    ty += bmo::GLIFO_ALTO + 12;

    // -- 3. EL TECLADO --------------------------------------------------
    //
    // Siete teclas. Existe para poder COMPROBAR: un control de sonido que no
    // deja hacer ruido no se puede probar, y entonces no se sabe si funciona.
    let tecla_w = 52u32;
    let tecla_h = 54u32;
    let sep = 4u32;
    let kx = tx;
    for (i, (letra, _, nombre)) in NOTAS.iter().enumerate() {
        let x = kx + i as u32 * (tecla_w + sep);
        let color = if pulsada == Some(i) { TECLA_PULSADA } else { TECLA_BLANCA };
        p.rect(x, ty, tecla_w, tecla_h, color);
        // El nombre de la nota arriba y la letra que la toca abajo: la ventana
        // se explica sola sin una leyenda aparte.
        p.texto(x + 8, ty + 8, nombre, TECLA_NEGRA);
        let mut l = [0u8; 1];
        l[0] = (*letra as char).to_ascii_uppercase() as u8;
        p.texto_bytes(x + 8, ty + tecla_h - bmo::GLIFO_ALTO - 8, &l, TECLA_NEGRA);
    }
    ty += tecla_h + 8;

    // -- 4. La barra de atajos, del mismo estilo que las otras ventanas --
    p.texto(
        tx,
        ty,
        "flechas volumen   Z..M notas   P la frase   arrastra el titulo   ESC devuelve el aparato",
        TEXTO_TENUE,
    );
}
